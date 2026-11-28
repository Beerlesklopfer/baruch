// SPDX-License-Identifier: AGPL-3.0-or-later
//! Ribbon for editing (Round 18a, interface R18-S1): the formats of the work template
//! (`#let szene`, `#let arbeit`) and the built-in ones (footnote, quotation, list …) as buttons.
//!
//! - RPC `ribbon.formats {work?}` → `{formats: [RibbonFormat]}` (`docs/RPC.md`).
//! - In the typeset view a button sends the command `cmd` through `baruch:ws-command` (`web/src/ws_command.ts`),
//!   in the source text `web/src/ribbon.ts` inserts the Typst text `snippet` at the cursor position.
//! - Round 19b: „Bibelstelle“ (`scripture`) if the template defines `#let bibelstelle(…)` or `#let bibel(…)`
//!   (detection through [`defined_functions`]; snippet `#bibelstelle[]` with the name found, assumption: the function
//!   takes the passage as content), and „Literaturverweis“ (`cite`, snippet `@`) if the template contains
//!   `#bibliography(`. Without that the buttons do not exist (otherwise typesetting would be an error).

use baruch_store::WorkId;
use serde_json::{Value, json};

use crate::AppState;
use crate::i18n::tc;
use crate::rpc::{METHOD_NOT_FOUND, RpcError};

/// One format of the work template as a button of the ribbon.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct RibbonFormat {
    /// Command to the typeset view (`baruch:ws-command`, see `web/src/ws_command.ts`):
    /// scene|work|scripture|cite|footnote|quote|list|enum|terms|break|link|xref
    pub cmd: String,
    pub label: String,
    pub title: String,
    /// Typst text for the source text (inserted at the cursor position), e.g. "#szene[\n\n]".
    pub snippet: String,
}

/// Buttons of the ribbon for a work.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct Ribbon {
    pub formats: Vec<RibbonFormat>,
}

/// Typst text of the button „Zitat“: only the quotation block. Up to Round 35 the line „– “ for the
/// attribution stood below it (Round 23a); if it stayed unfilled, a single dash stood in the book. Since Round 36
/// the button no longer inserts a following line: the author writes the attribution himself into the line directly
/// below the block (the typeset view reads it since Round 23 as a paragraph of its own, no island) – the hint of the
/// button says so.
pub const QUOTE_SNIPPET: &str = "#quote(block: true)[]\n";

/// Function names for Bible passages in the work template, in this order of precedence (Round 19b).
pub const SCRIPTURE_FUNCTIONS: [&str; 2] = ["bibelstelle", "bibel"];

/// Formats the work template provides (`#let szene`, `#let arbeit`, since Round 19b `#let bibelstelle`/`#let bibel`
/// and `#bibliography(` for literature references) plus the built-in ones (footnote, quotation, list …). Order:
/// first those of the template (scene, work, Bible passage, literature reference), then footnote, quotation, list,
/// enumeration, term, break, link, cross-reference (Round 30a). A template that does not define a function does not
/// get the button: without the function `#szene[…]` would be an error in typesetting, without a bibliography a
/// `@schlüssel`.
pub fn formats(template: &str) -> Vec<RibbonFormat> {
    let defined = defined_functions(template);
    let mut out = Vec::new();
    let mut push = |cmd: &str, label: &str, title: &str, snippet: &str| {
        out.push(RibbonFormat {
            cmd: cmd.to_owned(),
            label: label.to_owned(),
            title: title.to_owned(),
            snippet: snippet.to_owned(),
        });
    };
    if defined.iter().any(|name| name == "szene") {
        push("scene", &tc("view_msg.ribbon.scene", &[]), &tc("view_msg.ribbon.scene_title", &[]), "#szene[\n\n]\n");
    }
    if defined.iter().any(|name| name == "arbeit") {
        push("work", &tc("view_msg.ribbon.work", &[]), &tc("view_msg.ribbon.work_title", &[]), "#arbeit[\n\n]\n");
    }
    if let Some(name) = SCRIPTURE_FUNCTIONS.iter().find(|f| defined.iter().any(|name| name == *f)) {
        push(
            "scripture",
            &tc("view_msg.ribbon.scripture", &[]),
            &tc("view_msg.ribbon.scripture_title", &[("name", (*name).into())]),
            &format!("#{name}[]"),
        );
    }
    if cites_literature(template) {
        push("cite", &tc("view_msg.ribbon.cite", &[]), &tc("view_msg.ribbon.cite_title", &[]), "@");
    }
    push("footnote", &tc("view_msg.ribbon.footnote", &[]), &tc("view_msg.ribbon.footnote_title", &[]), "#footnote[]");
    // Round 23a (finding of the user): room for the attribution below the block. Round 36: without a preset line
    // „– “ (it stayed unfilled as a single dash); the hint names the form. An empty
    // `attribution: []` is out of the question: Typst sets a line with the dash for it (measured in
    // `crates/baruch-render/tests/quote_attribution.rs`). The empty lines before and after are set by the widget
    // depending on the surroundings (`web/src/ribbon.ts`, `insertion`), the cursor stands in the body.
    push("quote", &tc("view_msg.ribbon.quote", &[]), &tc("view_msg.ribbon.quote_title", &[]), QUOTE_SNIPPET);
    push("list", &tc("view_msg.ribbon.list", &[]), &tc("view_msg.ribbon.list_title", &[]), "- \n");
    push("enum", &tc("view_msg.ribbon.enum", &[]), &tc("view_msg.ribbon.enum_title", &[]), "1. \n");
    push("terms", &tc("view_msg.ribbon.terms", &[]), &tc("view_msg.ribbon.terms_title", &[]), "/ Begriff: \n");
    push("break", &tc("view_msg.ribbon.break", &[]), &tc("view_msg.ribbon.break_title", &[]), "\\\n");
    push("link", &tc("view_msg.ribbon.link", &[]), &tc("view_msg.ribbon.link_title", &[]), "#link(\"https://\")[]");
    // Round 30a: cross-reference to an outline item; the button opens the dialog (web/src/xref.ts).
    push(
        "xref",
        &tc("view_msg.ribbon.xref", &[]),
        &tc("view_msg.ribbon.xref_title", &[("lt", "<".into()), ("gt", ">".into())]),
        "#link(<k>)[]",
    );
    out
}

/// Does the template have a bibliography (`#bibliography(` outside of comments)? Then a
/// `@schlüssel` in the text works (Round 19b).
fn cites_literature(template: &str) -> bool {
    code_lines(template).iter().any(|(_, line)| line.contains("#bibliography("))
}

/// Lines of the template without comments (`// …` and – also nested – `/* … */`), each with the depth of open
/// brackets `(`, `[`, `{` at the start of the line (0 = top level). Strings `"…"` stay whole (a `//` or a
/// bracket inside does not count). Round 23a: this way a `#let szene` in a block comment or in the body of
/// another function no longer counts as a definition of the template. Not a Typst parser, but its rules for
/// comments, strings and brackets.
fn code_lines(template: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let (mut depth, mut comment, mut string) = (0usize, 0usize, false);
    for line in template.lines() {
        let start_depth = depth;
        let mut code = String::new();
        let chars: Vec<char> = line.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let (c, next) = (chars[i], chars.get(i + 1).copied());
            if comment > 0 {
                match (c, next) {
                    ('*', Some('/')) => {
                        comment -= 1;
                        i += 2;
                    }
                    ('/', Some('*')) => {
                        comment += 1;
                        i += 2;
                    }
                    _ => i += 1,
                }
                continue;
            }
            if string {
                code.push(c);
                if c == '\\' && next.is_some() {
                    code.push(chars[i + 1]);
                    i += 2;
                    continue;
                }
                if c == '"' {
                    string = false;
                }
                i += 1;
                continue;
            }
            match (c, next) {
                ('/', Some('/')) => break,
                ('/', Some('*')) => {
                    comment += 1;
                    i += 2;
                    continue;
                }
                ('"', _) => string = true,
                ('(' | '[' | '{', _) => depth += 1,
                (')' | ']' | '}', _) => depth = depth.saturating_sub(1),
                _ => {}
            }
            code.push(c);
            i += 1;
        }
        // In Typst a string does not necessarily end at the end of the line; close it here anyway to be safe.
        string = false;
        out.push((start_depth, code));
    }
    out
}

/// Names of the functions the template provides on the top level: defined with `#let name(…)` or `#let name =`
/// (start of the line, also indented) or – Round 23a – introduced with `#import "datei.typ": name, anderer`
/// (`*` and renamings `a as b` → `b`). Comments and definitions in the body of other functions do not count
/// ([`code_lines`]).
fn defined_functions(template: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    let mut add = |name: &str| {
        let name = name.trim();
        let valid = !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '-');
        if valid && !names.iter().any(|n| n == name) {
            names.push(name.to_owned());
        }
    };
    for (depth, line) in code_lines(template) {
        if depth > 0 {
            continue;
        }
        let trimmed = line.trim_start();
        if let Some(rest) = trimmed.strip_prefix("#import") {
            if !rest.starts_with([' ', '\t']) {
                continue;
            }
            // Behind the path (string) stands `: name, anderer as neu`; without a colon only the module comes.
            let after_path = rest.trim_start().strip_prefix('"').and_then(|r| r.split_once('"')).map(|(_, r)| r);
            if let Some(items) = after_path.and_then(|r| r.trim_start().strip_prefix(':')) {
                for item in items.trim().trim_start_matches('(').trim_end_matches(')').split(',') {
                    let item = item.trim();
                    add(item.rsplit_once(" as ").map_or(item, |(_, renamed)| renamed));
                }
            }
            continue;
        }
        let Some(rest) = trimmed.strip_prefix("#let") else { continue };
        if !rest.starts_with([' ', '\t']) {
            continue; // „#letter…“: no whitespace after #let
        }
        let rest = rest.trim_start();
        let name: String = rest.chars().take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '-').collect();
        let after = rest[name.len()..].trim_start();
        if after.starts_with('(') || after.starts_with('=') {
            add(&name);
        }
    }
    names
}

/// Ribbon of the work from its work template.
pub async fn ribbon(state: &AppState, work: WorkId) -> Result<Ribbon, RpcError> {
    let template = state.store.work_template(work).await?;
    Ok(Ribbon { formats: formats(&template) })
}

/// Does this module serve the RPC method? `ribbon.formats`.
pub fn handles(method: &str) -> bool {
    method == "ribbon.formats"
}

/// RPC `ribbon.formats {work?}` → `{formats}`; `work` is already resolved (missing → default work).
pub async fn call(state: &AppState, work: WorkId, method: &str, params: Value) -> Result<Value, RpcError> {
    let _ = params;
    match method {
        "ribbon.formats" => Ok(json!({ "formats": ribbon(state, work).await?.formats })),
        _ => Err(RpcError::new(METHOD_NOT_FOUND, format!("unknown method {method}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cmds(template: &str) -> Vec<String> {
        formats(template).into_iter().map(|f| f.cmd).collect()
    }

    #[test]
    fn scene_and_work_appear_only_when_the_template_defines_them() {
        let built_in = ["footnote", "quote", "list", "enum", "terms", "break", "link", "xref"];
        assert_eq!(cmds(""), built_in, "ohne Vorlage nur die eingebauten Formate, Fußnote, Zitat und Liste immer");
        let both = "#set text(lang: \"de\")\n#let szene(body) = block(inset: 4pt, body)\n  #let arbeit = rect\n";
        let mut expected = vec!["scene", "work"];
        expected.extend(built_in);
        assert_eq!(cmds(both), expected, "Szene und Arbeit aus der Vorlage zuerst, dann die eingebauten");
        assert_eq!(
            cmds("#let szenen(body) = body\n// #let arbeit(body) = body\n#letter\n#let szene_neu(x) = x\n"),
            built_in,
            "ähnliche Namen, auskommentierte Zeilen und „#letter“ zählen nicht"
        );
    }

    /// Round 23a (review R18): comments, function bodies and `#import`.
    #[test]
    fn only_top_level_code_defines_a_format_and_imports_count() {
        let built_in = ["footnote", "quote", "list", "enum", "terms", "break", "link", "xref"];
        assert_eq!(
            cmds(
                "/* alter Entwurf:\n#let szene(body) = body\n*/\n#let rahmen(body) = {\n  #let arbeit(x) = x\n  body\n}\n"
            ),
            built_in,
            "`#let szene` im Blockkommentar und `#let arbeit` im Rumpf einer anderen Funktion ergeben keinen Knopf \
             (der Satz scheiterte sonst an der fehlenden Funktion)"
        );
        assert_eq!(
            cmds(
                "/* a /* geschachtelt */ noch Kommentar\n#let szene(b) = b */\n#let arbeit(b) = b // #let szene(b) = b\n"
            ),
            [vec!["work"], built_in.to_vec()].concat(),
            "geschachtelte Blockkommentare enden erst mit ihrem letzten `*/`; ein Zeilenkommentar zählt nicht"
        );
        assert_eq!(
            cmds("#import \"formate.typ\": szene, arbeit\n"),
            [vec!["scene", "work"], built_in.to_vec()].concat(),
            "aus einer Datei eingeführte Funktionen ergeben die Knöpfe"
        );
        assert_eq!(
            cmds("#import \"formate.typ\": kasten as szene, anderes\n#import \"mehr.typ\"\n"),
            [vec!["scene"], built_in.to_vec()].concat(),
            "eine Umbenennung zählt mit ihrem neuen Namen; ein Import ohne Namen bringt keine Funktion"
        );
        assert_eq!(
            cmds("#let hinweis = \"// kein Kommentar: #let x\"\n#let szene(body) = block(body) // Szene\n"),
            [vec!["scene"], built_in.to_vec()].concat(),
            "ein `//` in einer Zeichenkette beginnt keinen Kommentar; die Definition danach zählt"
        );
        assert_eq!(
            cmds("/* #bibliography(\"alt.bib\") */\n"),
            built_in,
            "ein Literaturverzeichnis im Blockkommentar ergibt keinen Literaturverweis"
        );
    }

    #[test]
    fn scripture_and_cite_appear_only_when_the_template_gives_them() {
        crate::i18n::sync_scope(crate::i18n::Lang::De, || {
            let built_in = ["footnote", "quote", "list", "enum", "terms", "break", "link", "xref"];
            let with = "#let bibelstelle(stelle) = text(style: \"italic\", stelle)\n#bibliography(\"quellen.bib\")\n";
            let mut expected = vec!["scripture", "cite"];
            expected.extend(built_in);
            assert_eq!(cmds(with), expected, "Bibelstelle und Literaturverweis aus der Vorlage vor den eingebauten");
            let all = formats(with);
            let scripture = all.iter().find(|f| f.cmd == "scripture").unwrap();
            assert_eq!(
                scripture.snippet, "#bibelstelle[]",
                "Snippet mit dem Namen der Vorlage, Inhalt in den Klammern"
            );
            assert_eq!(scripture.label, "Bibelstelle", "deutsche Beschriftung");
            let cite = all.iter().find(|f| f.cmd == "cite").unwrap();
            assert_eq!((cite.label.as_str(), cite.snippet.as_str()), ("Literaturverweis", "@"), "@cite als Knopf");
            let bibel = formats("#let bibel(body) = emph(body)\n");
            assert_eq!(
                bibel.iter().find(|f| f.cmd == "scripture").map(|f| f.snippet.as_str()),
                Some("#bibel[]"),
                "auch #let bibel ergibt den Knopf, mit ihrem Namen"
            );
            assert_eq!(cmds(""), built_in, "ohne Vorlage weder Bibelstelle noch Literaturverweis");
            assert_eq!(
                cmds("// #bibliography(\"x.bib\")\n#let bibelstellen(x) = x\n#let bibel_neu(x) = x\n"),
                built_in,
                "auskommentiertes Verzeichnis und ähnliche Namen zählen nicht"
            );
        })
    }

    #[test]
    fn snippets_are_valid_typst_shapes_with_german_labels() {
        crate::i18n::sync_scope(crate::i18n::Lang::De, || {
            let all = formats("#let szene(body) = body\n#let arbeit(body) = body\n");
            let get = |cmd: &str| all.iter().find(|f| f.cmd == cmd).unwrap_or_else(|| panic!("{cmd} fehlt"));
            assert_eq!(get("scene").snippet, "#szene[\n\n]\n", "Szene als Block mit leerer Zeile für den Text");
            assert_eq!(get("footnote").snippet, "#footnote[]", "Fußnote mitten im Absatz, ohne Zeilenumbruch");
            assert_eq!(get("break").snippet, "\\\n", "Zeilenumbruch ist ein Backslash am Zeilenende");
            assert_eq!(
                get("quote").snippet,
                "#quote(block: true)[]\n",
                "Zitat als Block ohne `attribution` (eine leere setzte in Typst eine Strichzeile) und seit Runde 36 ohne \
             Folgezeile „– “; die Leerzeilen davor und danach setzt das Widget (Runde 23a)"
            );
            assert_eq!(get("list").label, "Liste", "deutsche Beschriftung");
            assert!(
                all.iter().all(|f| !f.title.is_empty() && !f.label.is_empty()),
                "jeder Knopf hat Titel und Beschriftung"
            );
        })
    }
}
