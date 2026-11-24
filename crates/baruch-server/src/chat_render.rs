// SPDX-License-Identifier: AGPL-3.0-or-later
//! Gegliederte Beiträge im Chat (Runde 30c, chat-bedienung).
//!
//! Serverseitiges, sicheres Markdown-Teilset ([`markdown_html`]): **kein rohes HTML** aus Modell oder Nutzer – jeder
//! Text wird maskiert, Auszeichnung und Links baut nur Baruch.
//! - Absätze (Leerzeile), einfache Umbrüche als `<br>`;
//! - Listen „- “/„* “/„• “/„– “/„+ “ als `<ul class="post-list">`, „1. “/„1) “ als `<ol class="post-list">`
//!   (Abstand zwischen den Punkten in `static/chat.css`), eingerückte Unterpunkte als Liste im Punkt, eingerückte
//!   Folgezeilen gehören zum Punkt;
//! - `**fett**`, `*kursiv*`;
//! - Zwischenüberschriften: „Was trägt:“, „Was fehlt:“, „Was stört:“ am Zeilenanfang (Text dahinter wird ein Absatz),
//!   eine ganze Zeile `**…**` und „# …“ bis „###### …“ als `<h4 class="post-h">`;
//! - `> Zitat` als `<blockquote>`;
//! - Schlusshinweise von Baruch am Ende (z. B. „Hinweis: Bibelstellen und Wortlaut prüft @schrift; hier ungeprüft.“,
//!   abgeschnitten, gekürzt, komprimiert, fehlender Pflichtbefund) klein als `<footer class="post-foot">`.
//!
//! Links im Beitrag ([`post_html`], [`render_post`]), alle geprüft und gebaut von Baruch:
//! - Sprungmarken `[[key:N]]` bzw. `[[key:N|„Zitat“]]` über [`crate::xref::check_marker`]: geprüft →
//!   `<a class="jump" href="…" data-jump data-jump-origin="chat">`, sonst `<span class="jump-unchecked"
//!   title="Sprungmarke ungeprüft: …">` mit dem Text (bzw. „key:N“);
//! - wörtliche Zitate „…“ (ab [`MIN_QUOTE_CHARS`] Zeichen), die die Belegprüfung im aktuellen Text findet: zuerst im
//!   Abschnitt des Beitrags, sonst nur, wenn genau ein Abschnitt sie enthält (Sprung mit `find=`);
//! - Gliederungsnennungen über [`crate::xref::outline_mentions`] (eindeutig oder gar nicht);
//! - Bibelstellen (gültige Stellen aus `baruch_scripture::scan`) → `/literatur/bibel?work=W&ref=…` – die Bibelseite
//!   schlägt ohne `module` die Bibel des Werks auf;
//! - „Einfall n“ → `/einfaelle/n`, „Befund n“ und „[Bn]“ → Befundseite (nur Befunde des Werks, wenn die Liste bekannt
//!   ist).
//!
//! Befunde als eigene Blöcke: [`render_post`] teilt den Text an den Spannen von
//! [`crate::findings::parse::finding_spans`] ([`Segment::Finding`] mit der id des gespeicherten Befunds derselben
//! Stelle, `finding_cards` mit dessen Karte [`crate::findings::view::card`] für das Makro `finding_block`).
//! „Kopieren“ liefert [`PostExtra::plain`]: Klartext mit „- “-Zeilen, ohne Auszeichnung, Sprungmarken als Text.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use baruch_store::{Finding, PostMeta, WorkId};
use regex::Regex;

use crate::bibles::html_escape;

/// Zitate kürzer als so viele Zeichen werden nicht verlinkt (zu viele Zufallstreffer).
pub const MIN_QUOTE_CHARS: usize = 12;
/// Zitate länger als so viele Zeichen werden nicht gesucht.
const MAX_QUOTE_CHARS: usize = 400;

/// Zusammenhang eines Beitrags auf der Arbeitsseite.
pub struct PostCtx<'a> {
    pub work_id: WorkId,
    pub work: &'a baruch_core::Work,
    pub key: i64,
    pub shown_rev: u32,
    pub extras: &'a crate::view::ViewExtras,
}

/// Ein Stück eines gegliederten Beitrags: Markdown-Text oder ein Befund.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Segment {
    Markdown { html: String },
    Finding { finding: i64, html: String },
}

/// Was die Vorlage zu einem Beitrag über `post.body` hinaus bekommt (`post.extra`).
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct PostExtra {
    /// Der ganze Beitrag als sicheres HTML (Befunde in `div.post-finding`).
    pub html: String,
    pub segments: Vec<Segment>,
    /// Klartext für „Kopieren“ (mit „- “-Zeilen).
    pub plain: String,
    /// Kopfzeile „geprüft: …“ (aus `PostMeta::basis_label`); `None` = keine Angabe.
    pub basis: Option<String>,
    pub reply_to: Option<i64>,
    pub repeat_of: Option<i64>,
    pub relay_from: Option<String>,
    /// Lässt sich wiederholen (der Auftrag ist bekannt oder der Rückfall greift, siehe [`RenderInput::fallback`]).
    pub can_repeat: bool,
    /// Runde 30c: geprüfte Fassung (für „Wiederholung von Beitrag n nach Fassung m“).
    pub basis_rev: Option<u32>,
    /// Runde 30c: Karten der Befunde dieses Beitrags (id als Text → [`crate::findings::view::card`]).
    pub finding_cards: BTreeMap<String, serde_json::Value>,
}

/// Was [`render_post`] braucht (Abschnitts-Chat, Faden, `/beitraege`, `/beitrag`).
pub struct RenderInput<'a> {
    pub work_id: WorkId,
    pub work: &'a baruch_core::Work,
    /// Abschnitt des Beitrags (`None` im Faden).
    pub key: Option<i64>,
    /// Gezeigte bzw. geprüfte Fassung des Abschnitts (für Sprungmarken in denselben Abschnitt).
    pub rev: Option<u32>,
    pub body: &'a str,
    pub meta: Option<&'a PostMeta>,
    /// Gespeicherte Befunde **dieses** Beitrags (Stelle → Befund über `Finding::position`).
    pub own_findings: Vec<&'a Finding>,
    /// Alle Befunde des Werks (für Karten und „Befund n“); `None` = unbekannt (dann wird „Befund n“ immer verlinkt).
    pub all_findings: Option<&'a [Finding]>,
    /// Rückfall für „wiederholen“ ohne gespeicherten Auftrag: Persona-Beitrag mit vorangehender Nachricht des
    /// Menschen bzw. eines MCP-Clients.
    pub fallback: bool,
}

// ---- Markdown-Teilset ------------------------------------------------------------------------------------------

/// Anfänge der Schlusshinweise von Baruch (Fußzeile).
fn footer_prefixes() -> [&'static str; 8] {
    [
        crate::scripture::MARK_OTHERS,
        crate::scripture::MARK_SCRIPTURE,
        crate::scripture::MARK_BIBLE,
        crate::assist::CUT_NOTE,
        crate::assist::TRUNCATED_NOTE,
        crate::assist::prompts::CRITICAL_MISSING_NOTE,
        "Komprimiert:",
        "(Antwort abgeschnitten",
    ]
}

fn is_footer_line(line: &str) -> bool {
    let t = line.trim();
    !t.is_empty() && footer_prefixes().iter().any(|p| t.starts_with(p))
}

/// Ein Listenpunkt mit Unterpunkten.
#[derive(Debug, Default)]
struct Item {
    lines: Vec<String>,
    sub: Vec<String>,
}

#[derive(Debug)]
enum Block {
    Para(Vec<String>),
    List { ordered: bool, indent: usize, items: Vec<Item> },
    Quote(Vec<String>),
    Heading(String),
}

static LIST_ITEM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\s*)([-*•–+]|\d{1,3}[.)])\s+(.*)$").expect("Muster Listenpunkt"));
static HASH_HEADING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s{0,3}#{1,6}\s+(.+?)\s*#*\s*$").expect("Muster Überschrift"));
static OUTLINE_HEADING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*(?:\*\*)?(was\s+trägt|was\s+fehlt|was\s+stört)\s*:\s*(?:\*\*)?\s*(.*)$")
        .expect("Muster Gliederung")
});

/// Eine ganze Zeile `**…**` (ohne weiteres `**` darin)?
fn bold_line(line: &str) -> Option<&str> {
    let t = line.trim();
    let inner = t.strip_prefix("**")?.strip_suffix("**")?;
    (!inner.trim().is_empty() && !inner.contains("**")).then_some(inner.trim())
}

fn indent_of(line: &str) -> usize {
    line.chars().take_while(|c| *c == ' ' || *c == '\t').map(|c| if c == '\t' { 4 } else { 1 }).sum()
}

/// Text in Blöcke zerlegen (ohne Fußzeile).
fn blocks(text: &str) -> Vec<Block> {
    let mut out: Vec<Block> = Vec::new();
    let mut open = false; // Darf die nächste Zeile an den letzten Block anschließen?
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            open = false;
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix('>') {
            let rest = rest.strip_prefix(' ').unwrap_or(rest).to_owned();
            match out.last_mut() {
                Some(Block::Quote(lines)) if open => lines.push(rest),
                _ => out.push(Block::Quote(vec![rest])),
            }
            open = true;
            continue;
        }
        if let Some(c) = OUTLINE_HEADING.captures(line) {
            let word = c.get(1).map_or("", |m| m.as_str());
            let mut title: String = word.chars().take(1).flat_map(char::to_uppercase).collect();
            title.push_str(&word.chars().skip(1).collect::<String>());
            out.push(Block::Heading(format!("{title}:")));
            let rest = c.get(2).map_or("", |m| m.as_str()).trim();
            if rest.is_empty() {
                open = false;
            } else {
                out.push(Block::Para(vec![rest.to_owned()]));
                open = true;
            }
            continue;
        }
        if let Some(inner) = bold_line(line) {
            out.push(Block::Heading(inner.to_owned()));
            open = false;
            continue;
        }
        if let Some(c) = HASH_HEADING.captures(line) {
            out.push(Block::Heading(c[1].to_owned()));
            open = false;
            continue;
        }
        if let Some(c) = LIST_ITEM.captures(line) {
            let indent = indent_of(&c[1]);
            let ordered = c[2].chars().next().is_some_and(|ch| ch.is_ascii_digit());
            let text = c[3].to_owned();
            if let Some(Block::List { ordered: o, indent: base, items }) = out.last_mut()
                && (open || indent > *base)
            {
                if indent > *base + 1 && !items.is_empty() {
                    items.last_mut().expect("nicht leer").sub.push(text);
                    open = true;
                    continue;
                }
                if *o == ordered {
                    items.push(Item { lines: vec![text], sub: Vec::new() });
                    open = true;
                    continue;
                }
            }
            out.push(Block::List { ordered, indent, items: vec![Item { lines: vec![text], sub: Vec::new() }] });
            open = true;
            continue;
        }
        // Gewöhnliche Zeile: Folgezeile eines Punkts (eingerückt), eines Absatzes oder ein neuer Absatz.
        match out.last_mut() {
            Some(Block::List { items, indent, .. }) if open && indent_of(line) > *indent => {
                let item = items.last_mut().expect("Liste hat Punkte");
                match item.sub.last_mut() {
                    Some(sub) if indent_of(line) > *indent + 2 => {
                        sub.push(' ');
                        sub.push_str(trimmed);
                    }
                    _ => item.lines.push(trimmed.to_owned()),
                }
            }
            Some(Block::Para(lines)) if open => lines.push(trimmed.to_owned()),
            _ => out.push(Block::Para(vec![trimmed.to_owned()])),
        }
        open = true;
    }
    out
}

/// Text vorne und Fußzeile (Schlusshinweise am Ende) trennen.
fn split_footer(text: &str) -> (String, Vec<String>) {
    let lines: Vec<&str> = text.lines().collect();
    let mut end = lines.len();
    let mut footer: Vec<String> = Vec::new();
    while end > 0 {
        let line = lines[end - 1];
        if line.trim().is_empty() {
            end -= 1;
            continue;
        }
        if is_footer_line(line) {
            footer.push(line.trim().to_owned());
            end -= 1;
            continue;
        }
        break;
    }
    footer.reverse();
    (lines[..end].join("\n"), footer)
}

/// Sicheres Markdown-Teilset ohne Links (Befund-Akkordeon, Matrix; Signatur fest seit R30-S1).
pub fn markdown_html(text: &str) -> String {
    render_markdown(text, None)
}

fn render_markdown(text: &str, links: Option<&Links<'_>>) -> String {
    let text = text.replace("\r\n", "\n");
    let (front, footer) = split_footer(&text);
    let mut out = String::new();
    for block in blocks(&front) {
        match block {
            Block::Para(lines) => {
                out.push_str("<p>");
                out.push_str(&lines.iter().map(|l| inline(l, links)).collect::<Vec<_>>().join("<br>"));
                out.push_str("</p>");
            }
            Block::Heading(text) => {
                out.push_str("<h4 class=\"post-h\">");
                out.push_str(&inline(&text, links));
                out.push_str("</h4>");
            }
            Block::Quote(lines) => {
                out.push_str("<blockquote>");
                out.push_str(&lines.iter().map(|l| inline(l, links)).collect::<Vec<_>>().join("<br>"));
                out.push_str("</blockquote>");
            }
            Block::List { ordered, items, .. } => {
                let tag = if ordered { "ol" } else { "ul" };
                out.push_str(&format!("<{tag} class=\"post-list\">"));
                for item in items {
                    out.push_str("<li>");
                    out.push_str(&item.lines.iter().map(|l| inline(l, links)).collect::<Vec<_>>().join("<br>"));
                    if !item.sub.is_empty() {
                        out.push_str("<ul class=\"post-list\">");
                        for sub in &item.sub {
                            out.push_str("<li>");
                            out.push_str(&inline(sub, links));
                            out.push_str("</li>");
                        }
                        out.push_str("</ul>");
                    }
                    out.push_str("</li>");
                }
                out.push_str(&format!("</{tag}>"));
            }
        }
    }
    if !footer.is_empty() {
        out.push_str("<footer class=\"post-foot small muted\">");
        for line in &footer {
            out.push_str("<p>");
            out.push_str(&inline(line, links));
            out.push_str("</p>");
        }
        out.push_str("</footer>");
    }
    out
}

/// `**fett**` und `*kursiv*` in einem Stück ohne Links; alles andere maskiert.
fn emphasis(raw: &str) -> String {
    let parts: Vec<&str> = raw.split("**").collect();
    // Gerade Zahl von Teilen = ein „**“ ohne Gegenstück: das letzte bleibt wörtlich.
    let paired = if parts.len() % 2 == 1 { parts.len() } else { parts.len() - 1 };
    let mut out = String::new();
    for (i, part) in parts.iter().enumerate() {
        if i >= paired {
            out.push_str("**");
            out.push_str(&italic(part));
            continue;
        }
        if i % 2 == 1 && !part.trim().is_empty() {
            out.push_str("<strong>");
            out.push_str(&italic(part));
            out.push_str("</strong>");
        } else if i % 2 == 1 {
            out.push_str("****");
            out.push_str(&html_escape(part));
        } else {
            out.push_str(&italic(part));
        }
    }
    out
}

/// `*kursiv*` (Stern direkt am Wort, innen kein Stern); sonst maskiert.
fn italic(raw: &str) -> String {
    let mut out = String::new();
    let mut rest = raw;
    while let Some(start) = rest.find('*') {
        let after = &rest[start + 1..];
        let closing = after.find('*');
        let ok = closing.is_some_and(|end| {
            end > 0 && !after[..end].starts_with(char::is_whitespace) && !after[..end].ends_with(char::is_whitespace)
        });
        match closing {
            Some(end) if ok => {
                out.push_str(&html_escape(&rest[..start]));
                out.push_str("<em>");
                out.push_str(&html_escape(&after[..end]));
                out.push_str("</em>");
                rest = &after[end + 1..];
            }
            _ => {
                out.push_str(&html_escape(&rest[..=start]));
                rest = after;
            }
        }
    }
    out.push_str(&html_escape(rest));
    out
}

// ---- Links -----------------------------------------------------------------------------------------------------

/// Zusammenhang für Links in einem Beitrag.
struct Links<'a> {
    work_id: WorkId,
    work: &'a baruch_core::Work,
    key: Option<i64>,
    rev: Option<u32>,
    findings: Option<&'a [Finding]>,
}

/// Ein Link-Bereich in einer Zeile (Bytes) mit fertigem HTML.
struct Span {
    start: usize,
    end: usize,
    html: String,
}

static MARKER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[\[\s*key\s*:\s*(\d{1,12})\s*(?:\|([^\]\n]{0,400}))?\]\]").expect("Muster Marke"));
static IDEA_OR_FINDING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(Einfall|Befund)(?:\s+Nr\.)?\s+(\d{1,9})\b|\[B(\d{1,9})\]").expect("Muster Einfall/Befund")
});
static QUOTE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"„([^„“”\n]+)[“”]").expect("Muster Zitat"));

fn squash(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn attr(text: &str) -> String {
    html_escape(text)
}

/// Sprung-Link (geprüft), mit `data-jump` für das Öffnen ohne Seitenwechsel.
fn jump_link(target: &crate::xref::JumpTarget, text: &str, title: &str) -> String {
    format!(
        "<a class=\"jump\" href=\"{}\" data-jump data-jump-origin=\"chat\" title=\"{}\">{}</a>",
        attr(&crate::xref::jump_url(target)),
        attr(title),
        html_escape(text)
    )
}

fn marker_span(links: &Links<'_>, key: i64, quote: Option<&str>) -> String {
    // Die Fassungsnummer gilt nur im eigenen Abschnitt; in fremden Abschnitten zählt die aktuelle.
    let rev = if Some(key) == links.key { links.rev } else { None };
    let quote = quote.map(str::trim).filter(|q| !q.is_empty());
    match crate::xref::check_marker(links.work_id, links.work, key, quote, rev) {
        crate::xref::MarkerCheck::Linked { target, label } => {
            let node = links.work.nodes.iter().find(|n| n.key == key);
            let title = node.map_or(label.clone(), |n| format!("{} {}", n.number, n.title).trim().to_owned());
            let text = match quote {
                Some(q) => q.to_owned(),
                None => title.clone(),
            };
            jump_link(&target, &text, &title)
        }
        crate::xref::MarkerCheck::UnknownKey { key } => format!(
            "<span class=\"jump-unchecked\" title=\"{}\">{}</span>",
            attr(&format!("Sprungmarke ungeprüft: Abschnitt {key} gibt es in diesem Werk nicht")),
            html_escape(quote.unwrap_or(&format!("key:{key}")))
        ),
        crate::xref::MarkerCheck::QuoteMissing { key, quote } => format!(
            "<span class=\"jump-unchecked\" title=\"{}\">{}</span>",
            attr(&format!("Sprungmarke ungeprüft: das Zitat steht nicht in Abschnitt {key}")),
            html_escape(&quote)
        ),
    }
}

/// Abschnitt, in dem das Zitat steht: zuerst der eigene, sonst genau einer.
fn quote_target(links: &Links<'_>, quote: &str) -> Option<(i64, Option<u32>)> {
    let wanted = squash(quote);
    if wanted.chars().count() < MIN_QUOTE_CHARS || wanted.chars().count() > MAX_QUOTE_CHARS {
        return None;
    }
    let body_of = |node: &baruch_core::Node, rev: Option<u32>| {
        rev.or_else(|| node.current_number()).and_then(|n| node.revision(n)).map(|r| squash(&r.body))
    };
    if let Some(key) = links.key
        && let Some(node) = links.work.nodes.iter().find(|n| n.key == key)
        && body_of(node, links.rev).is_some_and(|b| b.contains(&wanted))
    {
        return Some((key, links.rev));
    }
    let mut found = links.work.nodes.iter().filter(|n| body_of(n, None).is_some_and(|b| b.contains(&wanted)));
    let first = found.next()?;
    found.next().is_none().then_some((first.key, None))
}

/// Link-Bereiche einer Zeile (ohne Überschneidung; Marken vor Einfall/Befund vor Bibelstellen vor Zitaten vor
/// Gliederungsnennungen).
fn spans(line: &str, links: &Links<'_>) -> Vec<Span> {
    let mut found: Vec<Span> = Vec::new();
    let push = |found: &mut Vec<Span>, span: Span| {
        if !found.iter().any(|s| span.start < s.end && s.start < span.end) {
            found.push(span);
        }
    };
    for c in MARKER.captures_iter(line) {
        let whole = c.get(0).expect("Treffer");
        let Ok(key) = c[1].parse::<i64>() else { continue };
        let html = marker_span(links, key, c.get(2).map(|m| m.as_str()));
        push(&mut found, Span { start: whole.start(), end: whole.end(), html });
    }
    for c in IDEA_OR_FINDING.captures_iter(line) {
        let whole = c.get(0).expect("Treffer");
        let (kind, number) = match (c.get(1), c.get(2), c.get(3)) {
            (Some(kind), Some(n), _) => (kind.as_str(), n.as_str()),
            (_, _, Some(n)) => ("Befund", n.as_str()),
            _ => continue,
        };
        let Ok(id) = number.parse::<i64>() else { continue };
        let html = if kind == "Einfall" {
            format!(
                "<a class=\"idea-ref\" href=\"/einfaelle/{id}\" title=\"Einfall {id} öffnen\">{}</a>",
                html_escape(whole.as_str())
            )
        } else {
            let known = links.findings.is_none_or(|all| all.iter().any(|f| f.id == id));
            if !known {
                continue;
            }
            format!(
                "<a class=\"finding-ref\" href=\"/werke/{}/befunde?befund={id}\" title=\"Befund {id} öffnen\">{}</a>",
                links.work_id,
                html_escape(whole.as_str())
            )
        };
        push(&mut found, Span { start: whole.start(), end: whole.end(), html });
    }
    for f in baruch_scripture::scan(line) {
        let (Some(normalized), baruch_scripture::RefStatus::Valid) = (f.normalized.as_ref(), &f.status) else {
            continue;
        };
        let href = format!("/literatur/bibel?work={}&ref={}", links.work_id, crate::view::query_escape(normalized));
        let html = format!(
            "<a class=\"bible-ref\" href=\"{}\" title=\"{} in der Bibel des Werks aufschlagen\">{}</a>",
            attr(&href),
            attr(normalized),
            html_escape(&f.input)
        );
        push(&mut found, Span { start: f.span.0, end: f.span.1, html });
    }
    for c in QUOTE.captures_iter(line) {
        let whole = c.get(0).expect("Treffer");
        let quote = c[1].trim();
        let Some((key, rev)) = quote_target(links, quote) else { continue };
        let target = crate::xref::JumpTarget { work: links.work_id, key, rev, find: Some(quote.to_owned()), at: None };
        let title = links
            .work
            .nodes
            .iter()
            .find(|n| n.key == key)
            .map_or("Zitat".to_owned(), |n| format!("Zitat in {} {}", n.number, n.title).replace("  ", " "));
        push(
            &mut found,
            Span { start: whole.start(), end: whole.end(), html: jump_link(&target, whole.as_str(), &title) },
        );
    }
    for m in crate::xref::outline_mentions(links.work, line) {
        if m.end > line.len() || m.start >= m.end || !line.is_char_boundary(m.start) || !line.is_char_boundary(m.end) {
            continue;
        }
        let target = crate::xref::JumpTarget { work: links.work_id, key: m.key, rev: None, find: None, at: None };
        let html = jump_link(&target, &line[m.start..m.end], &m.text);
        push(&mut found, Span { start: m.start, end: m.end, html });
    }
    found.sort_by_key(|s| s.start);
    found
}

/// Eine Zeile: Links an ihren Stellen, dazwischen Auszeichnung und Maskierung.
fn inline(line: &str, links: Option<&Links<'_>>) -> String {
    let Some(links) = links else { return emphasis(line) };
    let mut out = String::new();
    let mut at = 0;
    for span in spans(line, links) {
        out.push_str(&emphasis(&line[at..span.start]));
        out.push_str(&span.html);
        at = span.end;
    }
    out.push_str(&emphasis(&line[at..]));
    out
}

/// Markdown plus geprüfte Links (Sprungmarken, Zitate, Gliederung, Bibelstellen, Einfälle/Befunde).
pub fn post_html(work_id: WorkId, work: &baruch_core::Work, body: &str, rev: Option<u32>) -> String {
    let links = Links { work_id, work, key: None, rev, findings: None };
    render_markdown(body, Some(&links))
}

// ---- Klartext ----------------------------------------------------------------------------------------------------

/// Klartext für „Kopieren“: Listenpunkte als „- “ (Nummern bleiben), ohne `**`/`*`, Sprungmarken als Text.
pub fn plain_text(work: Option<&baruch_core::Work>, body: &str) -> String {
    let text = body.replace("\r\n", "\n");
    let text = MARKER.replace_all(&text, |c: &regex::Captures<'_>| {
        if let Some(q) = c.get(2).map(|m| m.as_str().trim()).filter(|q| !q.is_empty()) {
            return q.to_owned();
        }
        let key: i64 = c[1].parse().unwrap_or_default();
        work.and_then(|w| w.nodes.iter().find(|n| n.key == key))
            .map_or(format!("key:{key}"), |n| format!("{} {}", n.number, n.title).trim().to_owned())
    });
    let mut out: Vec<String> = Vec::new();
    for line in text.lines() {
        let line = match LIST_ITEM.captures(line) {
            Some(c) if !c[2].chars().next().is_some_and(|ch| ch.is_ascii_digit()) => format!("{}- {}", &c[1], &c[3]),
            _ => line.to_owned(),
        };
        let line = bold_line(&line).map_or(line.clone(), str::to_owned);
        out.push(strip_emphasis(&line));
    }
    out.join("\n").trim_end().to_owned()
}

fn strip_emphasis(line: &str) -> String {
    let no_bold = line.replace("**", "");
    let mut out = String::new();
    let mut rest = no_bold.as_str();
    while let Some(start) = rest.find('*') {
        let after = &rest[start + 1..];
        match after.find('*') {
            Some(end) if end > 0 && !after.starts_with(' ') && !after[..end].ends_with(' ') => {
                out.push_str(&rest[..start]);
                out.push_str(&after[..end]);
                rest = &after[end + 1..];
            }
            _ => {
                out.push_str(&rest[..=start]);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

// ---- Beiträge ----------------------------------------------------------------------------------------------------

/// Text in Markdown- und Befund-Stücke teilen: `spans` (Bytes, aufsteigend) und je Stelle die id des Befunds.
fn split_segments(
    body: &str,
    spans: &[crate::findings::parse::FindingSpan],
    finding_at: &dyn Fn(u32) -> Option<i64>,
    render: &dyn Fn(&str) -> String,
) -> Vec<Segment> {
    let mut out: Vec<Segment> = Vec::new();
    let mut at = 0usize;
    for span in spans {
        let (start, end) = (span.range.start, span.range.end.min(body.len()));
        if start < at || start >= end || !body.is_char_boundary(start) || !body.is_char_boundary(end) {
            continue;
        }
        let Some(id) = finding_at(span.position) else { continue };
        let before = &body[at..start];
        if !before.trim().is_empty() {
            out.push(Segment::Markdown { html: render(before) });
        }
        out.push(Segment::Finding { finding: id, html: render(&body[start..end]) });
        at = end;
    }
    let rest = &body[at..];
    if !rest.trim().is_empty() || out.is_empty() {
        out.push(Segment::Markdown { html: render(rest) });
    }
    out
}

/// Ganzes HTML aus den Stücken (Befunde in `div.post-finding`).
fn joined(segments: &[Segment]) -> String {
    segments
        .iter()
        .map(|s| match s {
            Segment::Markdown { html } => html.clone(),
            Segment::Finding { finding, html } => {
                format!("<div class=\"post-finding\" data-finding=\"{finding}\">{html}</div>")
            }
        })
        .collect()
}

/// Beitrag setzen (Abschnitts-Chat, Faden, Seiten): Stücke, Links, Klartext, Kopf und Karten der Befunde.
pub fn render_post(input: &RenderInput<'_>) -> PostExtra {
    let links = Links {
        work_id: input.work_id,
        work: input.work,
        key: input.key,
        rev: input.rev,
        findings: input.all_findings,
    };
    let render = |text: &str| render_markdown(text, Some(&links));
    let spans = crate::findings::parse::finding_spans(input.body);
    let finding_at = |position: u32| input.own_findings.iter().find(|f| f.position == position).map(|f| f.id);
    let segments = split_segments(input.body, &spans, &finding_at, &render);
    let mut finding_cards = BTreeMap::new();
    for segment in &segments {
        if let Segment::Finding { finding, .. } = segment
            && let Some(f) = input.own_findings.iter().find(|f| f.id == *finding)
        {
            let all = input.all_findings.unwrap_or_default();
            finding_cards.insert(finding.to_string(), crate::findings::view::card(input.work_id, input.work, f, all));
        }
    }
    let meta = input.meta;
    PostExtra {
        html: joined(&segments),
        segments,
        plain: plain_text(Some(input.work), input.body),
        basis: meta.map(|m| m.basis_label.clone()).filter(|b| !b.is_empty()),
        reply_to: meta.and_then(|m| m.reply_to),
        repeat_of: meta.and_then(|m| m.repeat_of),
        relay_from: meta.and_then(|m| m.relay_from.clone()),
        can_repeat: meta.is_some_and(|m| m.request.is_some()) || input.fallback,
        basis_rev: meta.and_then(|m| m.basis_rev),
        finding_cards,
    }
}

/// Ist `author` eine Persona (`@handle`, nicht `@mcp:<name>`)?
pub fn is_persona(author: &str) -> bool {
    author.starts_with('@') && !author.starts_with("@mcp:")
}

/// Beitrag eines Schreibauftrags („Vorschlag Fassung n“, Runde 28)? Den wiederholt der Rückfall nicht.
pub fn is_order_note(sources: &[String]) -> bool {
    sources.iter().any(|s| s.starts_with("Vorschlag Fassung"))
}

/// Rückfall für „wiederholen“ (Abschnitts-Chat): der Beitrag `index` stammt von einer Persona, ist kein Beitrag zu
/// einem Schreibauftrag, und davor steht eine Nachricht des Menschen bzw. eines MCP-Clients.
pub fn repeat_fallback(posts: &[baruch_core::Post], index: usize) -> Option<&baruch_core::Post> {
    let post = posts.get(index)?;
    if !is_persona(&post.author) || is_order_note(&post.sources) {
        return None;
    }
    posts[..index].iter().rev().find(|p| !is_persona(&p.author))
}

/// Zusätze eines Beitrags für die Vorlage auf der Arbeitsseite (Haken aus `view::posts`).
pub fn post_extra(ctx: &PostCtx<'_>, post: &baruch_core::Post) -> PostExtra {
    let meta = ctx.extras.post_meta.get(&post.id);
    let all = ctx.extras.findings.findings.as_slice();
    let own: Vec<&Finding> = all.iter().filter(|f| f.post == Some(post.id)).collect();
    let fallback = meta.is_none_or(|m| m.request.is_none())
        && ctx.work.nodes.iter().find(|n| n.key == ctx.key).is_some_and(|node| {
            node.posts.iter().position(|p| p.id == post.id).is_some_and(|i| repeat_fallback(&node.posts, i).is_some())
        });
    render_post(&RenderInput {
        work_id: ctx.work_id,
        work: ctx.work,
        key: Some(ctx.key),
        rev: Some(ctx.shown_rev).filter(|r| *r >= 1),
        body: &post.body,
        meta,
        own_findings: own,
        all_findings: Some(all),
        fallback,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_escapes_html_and_keeps_paragraphs_and_breaks() {
        assert_eq!(
            markdown_html("Erste <b>Zeile</b>\nzweite\n\nNeuer Absatz"),
            "<p>Erste &lt;b&gt;Zeile&lt;/b&gt;<br>zweite</p><p>Neuer Absatz</p>",
            "kein rohes HTML; Leerzeile trennt Absätze, einfacher Umbruch wird <br>"
        );
    }

    #[test]
    fn lists_become_real_lists_with_sub_items_and_continuations() {
        let html =
            markdown_html("Vorab:\n- erster Punkt\n  geht weiter\n- zweiter\n    - Unterpunkt\n1. eins\n2. zwei");
        assert_eq!(
            html,
            "<p>Vorab:</p><ul class=\"post-list\"><li>erster Punkt<br>geht weiter</li><li>zweiter<ul class=\"post-list\">\
             <li>Unterpunkt</li></ul></li></ul><ol class=\"post-list\"><li>eins</li><li>zwei</li></ol>",
            "„- “ wird <ul>, „1. “ wird <ol>; eingerückte Folgezeile gehört zum Punkt, Unterpunkt in eigener Liste"
        );
        assert_eq!(
            markdown_html("* Stern\n• Punkt"),
            "<ul class=\"post-list\"><li>Stern</li><li>Punkt</li></ul>",
            "auch „* “ und „• “ sind Aufzählungen (Bildschirmfoto des Nutzers: @exeget mit „- …“)"
        );
    }

    #[test]
    fn bold_italic_headings_and_quotes_are_set_and_everything_else_escaped() {
        assert_eq!(
            markdown_html("**Kurz:** das *Licht* und 2*3*4 <i>"),
            "<p><strong>Kurz:</strong> das <em>Licht</em> und 2<em>3</em>4 &lt;i&gt;</p>",
            "fett und kursiv; Spitzklammern maskiert"
        );
        assert_eq!(markdown_html("ein * Stern * allein"), "<p>ein * Stern * allein</p>", "Stern mit Leerraum bleibt");
        assert_eq!(
            markdown_html("Was trägt: das Bild\nWas fehlt:\n- der Schluss\n**Mein Fazit**\n> zitiert <x>"),
            "<h4 class=\"post-h\">Was trägt:</h4><p>das Bild</p><h4 class=\"post-h\">Was fehlt:</h4>\
             <ul class=\"post-list\"><li>der Schluss</li></ul><h4 class=\"post-h\">Mein Fazit</h4>\
             <blockquote>zitiert &lt;x&gt;</blockquote>",
            "Gliederung als kleine Überschrift (Text dahinter ein Absatz), ganze **Zeile** ebenso, „> “ als Zitat"
        );
    }

    #[test]
    fn the_closing_note_of_baruch_becomes_a_small_footer() {
        let body = format!("Das trägt.\n\n{}", crate::scripture::MARK_OTHERS);
        let html = markdown_html(&body);
        assert!(
            html.ends_with(&format!(
                "<footer class=\"post-foot small muted\"><p>{}</p></footer>",
                html_escape(crate::scripture::MARK_OTHERS)
            )),
            "Schlusshinweis klein als Fußzeile: {html}"
        );
        assert!(html.starts_with("<p>Das trägt.</p>"), "der Text davor bleibt Absatz: {html}");
    }

    #[test]
    fn xss_probes_are_only_escaped() {
        let html = markdown_html(
            "<script>alert(1)</script> [Klick](javascript:alert(1)) <a href=\"https://böse.example\">x</a> **<img src=x onerror=1>**",
        );
        assert!(!html.contains("<script") && !html.contains("<a ") && !html.contains("<img"), "kein Element: {html}");
        assert!(html.contains("&lt;script&gt;"), "Skript nur als Text: {html}");
        assert!(html.contains("[Klick](javascript:alert(1))"), "Markdown-Links werden nicht zu Links: {html}");
        assert!(html.contains("href=&quot;https://böse.example&quot;"), "Anführungszeichen maskiert: {html}");
    }

    #[test]
    fn plain_text_for_copying_uses_dash_lines_and_no_markup() {
        assert_eq!(
            plain_text(None, "**Kurz:**\n* eins *betont*\n• zwei\n1. drei\n[[key:7|„Licht“]] und [[key:9]]"),
            "Kurz:\n- eins betont\n- zwei\n1. drei\n„Licht“ und key:9",
            "„- “-Zeilen, Nummern bleiben, keine Sterne, Sprungmarken als Text"
        );
    }

    #[test]
    fn segments_split_the_text_at_finding_spans_that_have_a_saved_finding() {
        use crate::findings::parse::FindingSpan;
        let body = "Vorab ein Satz.\n- Kritisch: „Kisten“ wirkt pedantisch.\n- Hinweis: Nebel hörbar machen.\nSchluss.";
        let first = body.find("- Kritisch").unwrap();
        let second = body.find("- Hinweis").unwrap();
        let end = body.find("Schluss").unwrap();
        let spans =
            vec![FindingSpan { position: 0, range: first..second }, FindingSpan { position: 1, range: second..end }];
        let saved = |p: u32| (p == 0).then_some(41);
        let segments = split_segments(body, &spans, &saved, &|t: &str| markdown_html(t));
        assert_eq!(segments.len(), 3, "Text davor, Befund 41, Rest (die Stelle ohne Befund bleibt Text): {segments:?}");
        assert!(matches!(&segments[1], Segment::Finding { finding: 41, html } if html.contains("pedantisch")));
        assert!(
            matches!(&segments[2], Segment::Markdown { html } if html.contains("Nebel") && html.contains("Schluss"))
        );
        assert!(joined(&segments).contains("<div class=\"post-finding\" data-finding=\"41\">"), "eigener Block");
        let none = split_segments("", &[], &saved, &|t: &str| markdown_html(t));
        assert_eq!(none.len(), 1, "auch ein leerer Beitrag hat ein Stück");
    }
}
