// SPDX-License-Identifier: AGPL-3.0-or-later
//! Back matter (index pages and glossary page) and overview per index (Round 31b).
//!
//! **Sorting and grouping in Rust:** keywords by [`super::sort_key`] (DIN 5007-1), grouped by the first letter; in
//! the index of scripture references by the sort key of the marks (biblical order of the books, then chapter and
//! verse). Equal entries = same index, same keyword and same sub-keyword (literally).
//!
//! **Page numbers in Typst:** `query(<baruch-idx>)` delivers all marks; per mark `counter(page).at(m.location())` –
//! the same counting as footer and cross-reference „S. n“, also with the page counting from the first chapter on
//! (the reset stands in the page background of the first chapter page, `typeset::rules`). Marks with „siehe“ carry
//! no page. Consecutive pages are merged to „12–14“, duplicate pages are dropped.
//!
//! **Standard Typst only:** helper functions with the prefix `baruch-idx-` or `baruch-gl-` in the back matter
//! itself, no package, no network; all data (keywords, terms, explanations) as escaped string literals.

use std::collections::BTreeMap;

use super::project::label_safe;
use super::{
    BackMatter, GLOSSARY_ANCHOR_PREFIX, GlossaryEntry, IndexMark, IndexTerm, Register, RegisterKind, RegisterView,
    SCRIPTURE_REGISTER, sort_key, term_key, typst_string,
};
use crate::typeset::GlossaryPosition;

/// A collected entry of an index.
#[derive(Debug, Clone, Default)]
pub(super) struct Entry {
    pub term: String,
    pub sub: String,
    /// Smallest sort key of automatic marks (Bible), otherwise empty.
    pub sort: String,
    /// Sections with marks that carry a page (without „siehe“).
    pub keys: Vec<i64>,
    pub see: Vec<String>,
    pub also: Vec<String>,
    /// All marks of the entry are automatic.
    pub auto: bool,
}

/// Entries per index: (index, keyword, sub-keyword) → entry.
pub(super) type Collected = BTreeMap<(String, String, String), Entry>;

/// Add a mark to the collection.
pub(super) fn collect(into: &mut Collected, key: i64, mark: &IndexMark, auto: bool) {
    let entry = into.entry((mark.register.clone(), mark.term.clone(), mark.sub.clone())).or_insert_with(|| Entry {
        term: mark.term.clone(),
        sub: mark.sub.clone(),
        auto: true,
        ..Entry::default()
    });
    entry.auto &= auto;
    if !mark.sort.is_empty() && (entry.sort.is_empty() || mark.sort < entry.sort) {
        entry.sort = mark.sort.clone();
    }
    if mark.see.is_empty() {
        if !entry.keys.contains(&key) {
            entry.keys.push(key);
        }
    } else if !entry.see.contains(&mark.see) {
        entry.see.push(mark.see.clone());
    }
    if !mark.also.is_empty() && !entry.also.contains(&mark.also) {
        entry.also.push(mark.also.clone());
    }
}

/// Comparison key of a text for the sorting: DIN 5007-1, then literally (unambiguous order).
fn text_order(text: &str) -> (String, String, String) {
    (sort_key(text), term_key(text), text.to_owned())
}

/// A group of entries with the same keyword: keyword, its own entry (without sub-keyword, if present) and the
/// sub-keywords in order.
pub(super) struct TermGroup<'a> {
    pub term: &'a str,
    pub main: Option<&'a Entry>,
    pub subs: Vec<&'a Entry>,
}

/// The entries of an index sorted and grouped by keyword.
pub(super) fn ordered<'a>(collected: &'a Collected, register: &Register) -> Vec<TermGroup<'a>> {
    let mut by_term: BTreeMap<&str, Vec<&Entry>> = BTreeMap::new();
    for ((slug, term, _), entry) in collected {
        if *slug == register.slug {
            by_term.entry(term.as_str()).or_default().push(entry);
        }
    }
    let scripture = register.kind == RegisterKind::Scripture;
    let mut groups: Vec<TermGroup> = by_term
        .into_iter()
        .map(|(term, entries)| {
            let main = entries.iter().copied().find(|e| e.sub.is_empty());
            let mut subs: Vec<&Entry> = entries.into_iter().filter(|e| !e.sub.is_empty()).collect();
            if scripture {
                subs.sort_by(|a, b| {
                    (a.sort.is_empty(), &a.sort, text_order(&a.sub)).cmp(&(
                        b.sort.is_empty(),
                        &b.sort,
                        text_order(&b.sub),
                    ))
                });
            } else {
                subs.sort_by_key(|e| text_order(&e.sub));
            }
            TermGroup { term, main, subs }
        })
        .collect();
    if scripture {
        // Book by the smallest sort key (the first two digits are the position of the book).
        let book = |g: &TermGroup| -> String {
            g.main
                .iter()
                .chain(&g.subs)
                .filter(|e| !e.sort.is_empty())
                .map(|e| e.sort.clone())
                .min()
                .unwrap_or_default()
        };
        groups.sort_by(|a, b| {
            let (ka, kb) = (book(a), book(b));
            (ka.is_empty(), ka.get(..2).unwrap_or("").to_owned(), text_order(a.term)).cmp(&(
                kb.is_empty(),
                kb.get(..2).unwrap_or("").to_owned(),
                text_order(b.term),
            ))
        });
    } else {
        groups.sort_by_key(|g| text_order(g.term));
    }
    groups
}

/// Letter of the group of a keyword („A“ for „Ägypten“, „0–9“ for digits, otherwise „#“).
pub(super) fn group_letter(term: &str) -> String {
    match sort_key(term).chars().next() {
        Some(c) if c.is_alphabetic() => c.to_uppercase().collect(),
        Some(c) if c.is_numeric() => "0–9".to_owned(),
        _ => "#".to_owned(),
    }
}

/// Overview per index.
pub(super) fn views(back: &BackMatter, collected: &Collected) -> Vec<RegisterView> {
    back.registers
        .iter()
        .map(|register| {
            let mut terms = Vec::new();
            for group in ordered(collected, register) {
                for entry in group.main.iter().copied().chain(group.subs.iter().copied()) {
                    let mut keys = entry.keys.clone();
                    keys.sort_unstable();
                    terms.push(IndexTerm {
                        term: entry.term.clone(),
                        sub: entry.sub.clone(),
                        keys,
                        see: entry.see.clone(),
                        also: entry.also.clone(),
                        auto: entry.auto,
                    });
                }
            }
            RegisterView { register: register.clone(), terms }
        })
        .collect()
}

fn lit(text: &str) -> String {
    let mut out = String::new();
    typst_string(text, &mut out);
    out
}

/// Typst array of string literals (`()`, `("a",)`, `("a", "b")`).
fn array(items: &[String]) -> String {
    match items {
        [] => "()".to_owned(),
        [one] => format!("({},)", lit(one)),
        many => format!("({})", many.iter().map(|s| lit(s)).collect::<Vec<_>>().join(", ")),
    }
}

/// Key of a mark for the page numbers in the Typst code (index, keyword, sub-keyword with U+001F).
fn entry_key(register: &str, term: &str, sub: &str) -> String {
    format!("{register}\u{1f}{term}\u{1f}{sub}")
}

/// Helper functions of the back matter.
pub(super) const HELPERS: &str = r#"#let baruch-idx-ranges(ns) = {
  let ns = ns.sorted().dedup()
  let out = ()
  let i = 0
  while i < ns.len() {
    let a = ns.at(i)
    let b = a
    while i + 1 < ns.len() and ns.at(i + 1) == b + 1 {
      i += 1
      b = ns.at(i)
    }
    out.push(if b > a { str(a) + "–" + str(b) } else { str(a) })
    i += 1
  }
  out.join(", ")
}
#let baruch-idx-pages() = {
  let d = (:)
  for m in query(<baruch-idx>) {
    let v = m.value
    if type(v) == dictionary and v.at("see", default: "") == "" {
      let k = v.at("register", default: "") + "\u{1f}" + v.at("term", default: "") + "\u{1f}" + v.at("sub", default: "")
      d.insert(k, d.at(k, default: ()) + (counter(page).at(m.location()).first(),))
    }
  }
  d
}
#let baruch-idx-group(letter) = block(above: 0.9em, below: 0.35em, sticky: true, strong(letter))
#let baruch-idx-entry(d, k, term, level, see, also) = {
  let ns = d.at(k, default: ())
  let parts = ()
  if ns.len() > 0 { parts.push([#baruch-idx-ranges(ns)]) }
  if see.len() > 0 { parts.push([#emph[siehe] #see.join("; ")]) }
  if also.len() > 0 { parts.push([#emph[siehe auch] #also.join("; ")]) }
  block(above: 0.3em, below: 0.3em, inset: (left: level * 1.2em), breakable: true,
    par(first-line-indent: 0em, hanging-indent: 1.2em, justify: false,
      [#term] + if parts.len() > 0 { [ ] + parts.join([; ]) } else { [] }))
}
#let baruch-gl-entry(anchor, term, variants, short, long, refs) = block(above: 0.6em, below: 0.9em, breakable: true, {
  set par(first-line-indent: 0em, justify: false)
  par[#anchor#strong(term)#if variants.len() > 0 [ (#variants.join(", "))]#if short != "" [ – #short]]
  for p in long { par(p) }
  if refs.len() > 0 { par[#emph[Bibelstellen:] #refs.join("; ")] }
})
"#;

/// An index page.
fn register_page(out: &mut String, register: &Register, collected: &Collected, columns: u8) {
    out.push_str("#pagebreak(weak: true)\n");
    out.push_str(&format!("#heading(level: 1, numbering: none)[#{}]\n", lit(&register.title)));
    let groups = ordered(collected, register);
    if groups.is_empty() {
        out.push_str("#emph(\"Keine Einträge.\")\n\n");
        return;
    }
    out.push_str("#context {\n  let d = baruch-idx-pages()\n  let body = {\n");
    let mut letter = String::new();
    let scripture = register.kind == RegisterKind::Scripture;
    for group in &groups {
        if !scripture {
            let l = group_letter(group.term);
            if l != letter {
                out.push_str(&format!("    baruch-idx-group({})\n", lit(&l)));
                letter = l;
            }
        }
        let line = |out: &mut String, entry: Option<&Entry>, text: &str, sub: &str, level: u8| {
            let (see, also) = entry.map_or((Vec::new(), Vec::new()), |e| (e.see.clone(), e.also.clone()));
            out.push_str(&format!(
                "    baruch-idx-entry(d, {}, {}, {level}, {}, {})\n",
                lit(&entry_key(&register.slug, group.term, sub)),
                lit(text),
                array(&see),
                array(&also)
            ));
        };
        line(out, group.main, group.term, "", 0);
        for entry in &group.subs {
            line(out, Some(entry), &entry.sub, &entry.sub, 1);
        }
    }
    out.push_str("  }\n");
    if columns > 1 {
        out.push_str(&format!("  columns({columns}, gutter: 1.5em, body)\n"));
    } else {
        out.push_str("  body\n");
    }
    out.push_str("}\n\n");
}

/// Glossary page.
fn glossary_page(out: &mut String, glossary: &[GlossaryEntry]) {
    out.push_str("#pagebreak(weak: true)\n");
    out.push_str("#heading(level: 1, numbering: none)[#\"Glossar\"]\n");
    if glossary.is_empty() {
        out.push_str("#emph(\"Keine Begriffe.\")\n\n");
        return;
    }
    let mut entries: Vec<&GlossaryEntry> = glossary.iter().collect();
    entries.sort_by_key(|e| text_order(&e.term));
    for entry in entries {
        let anchor = if label_safe(&entry.uuid) {
            format!("[#metadata(\"baruch-glossar\")<{GLOSSARY_ANCHOR_PREFIX}{}>]", entry.uuid)
        } else {
            "[]".to_owned()
        };
        let paragraphs: Vec<String> = entry
            .long
            .replace("\r\n", "\n")
            .split("\n\n")
            .map(|p| p.split_whitespace().collect::<Vec<_>>().join(" "))
            .filter(|p| !p.is_empty())
            .collect();
        let variants: Vec<String> =
            entry.variants.iter().map(|v| v.trim().to_owned()).filter(|v| !v.is_empty()).collect();
        let refs: Vec<String> =
            entry.bible_refs.iter().map(|v| v.trim().to_owned()).filter(|v| !v.is_empty()).collect();
        out.push_str(&format!(
            "#baruch-gl-entry({anchor}, {}, {}, {}, {}, {})\n",
            lit(entry.term.trim()),
            array(&variants),
            lit(&entry.short.split_whitespace().collect::<Vec<_>>().join(" ")),
            array(&paragraphs),
            array(&refs)
        ));
    }
    out.push('\n');
}

/// The back matter for the marks in `collected`.
pub(super) fn render(back: &BackMatter, collected: &Collected) -> String {
    let pages = back.settings.index_pages.as_ref();
    let glossary = back.settings.glossary_page;
    if pages.is_none() && glossary.is_none() {
        return String::new();
    }
    let mut out = format!("{}\n{HELPERS}\n", super::BACK_MATTER_MARK);
    let columns = pages.map_or(1, |p| p.columns.clamp(1, 3));
    let registers: Vec<&Register> = pages
        .map(|p| {
            let mut list: Vec<&Register> = Vec::new();
            for slug in &p.registers {
                if let Some(r) = back.registers.iter().find(|r| &r.slug == slug)
                    && !list.iter().any(|x| x.slug == r.slug)
                {
                    list.push(r);
                }
            }
            list
        })
        .unwrap_or_default();
    if glossary == Some(GlossaryPosition::BeforeIndexes) {
        glossary_page(&mut out, &back.glossary);
    }
    for register in registers {
        register_page(&mut out, register, collected, columns);
    }
    if glossary == Some(GlossaryPosition::AfterIndexes) {
        glossary_page(&mut out, &back.glossary);
    }
    out
}

/// Identifier of the index of scripture references (for callers that recognise automatic marks).
pub(super) fn is_auto(mark: &IndexMark) -> bool {
    mark.register == SCRIPTURE_REGISTER && !mark.sort.is_empty()
}
