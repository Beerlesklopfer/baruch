// SPDX-License-Identifier: AGPL-3.0-or-later
//! Menüband für das Bearbeiten (Runde 18a, Schnittstelle R18-S1): die Formate der Werkvorlage
//! (`#let szene`, `#let arbeit`) und die eingebauten (Fußnote, Zitat, Liste …) als Knöpfe.
//!
//! - RPC `ribbon.formats {work?}` → `{formats: [RibbonFormat]}` (`docs/RPC.md`).
//! - Ein Knopf schickt im Satzbild den Befehl `cmd` über `baruch:ws-command` (`web/src/ws_command.ts`),
//!   im Quelltext fügt `web/src/ribbon.ts` den Typst-Text `snippet` an der Cursorstelle ein.
//! - Runde 19b: „Bibelstelle“ (`scripture`), wenn die Vorlage `#let bibelstelle(…)` bzw. `#let bibel(…)` definiert
//!   (Erkennung über [`defined_functions`]; Snippet `#bibelstelle[]` mit dem gefundenen Namen, Annahme: die Funktion
//!   nimmt die Stelle als Inhalt), und „Literaturverweis“ (`cite`, Snippet `@`), wenn die Vorlage `#bibliography(`
//!   enthält. Ohne das gibt es die Knöpfe nicht (sonst wäre der Satz ein Fehler).

use baruch_store::WorkId;
use serde_json::{Value, json};

use crate::AppState;
use crate::rpc::{METHOD_NOT_FOUND, RpcError};

/// Ein Format der Werkvorlage als Knopf des Menübands.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct RibbonFormat {
    /// Befehl an das Satzbild (`baruch:ws-command`, siehe `web/src/ws_command.ts`):
    /// scene|work|scripture|cite|footnote|quote|list|enum|terms|break|link|xref
    pub cmd: String,
    pub label: String,
    pub title: String,
    /// Typst-Text für den Quelltext (an der Cursorstelle eingefügt), z. B. "#szene[\n\n]".
    pub snippet: String,
}

/// Knöpfe des Menübands für ein Werk.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct Ribbon {
    pub formats: Vec<RibbonFormat>,
}

/// Typst-Text des Knopfs „Zitat“: nur der Zitatblock. Bis Runde 35 stand darunter die Zeile „– “ für die
/// Stellenangabe (Runde 23a); blieb sie ungefüllt, stand ein einzelner Gedankenstrich im Buch. Seit Runde 36 fügt
/// der Knopf keine Folgezeile mehr ein: Die Stellenangabe schreibt der Autor selbst in die Zeile direkt unter den
/// Block (das Satzbild liest sie seit Runde 23 als eigenen Absatz, keine Insel) – der Hinweis des Knopfs sagt es.
pub const QUOTE_SNIPPET: &str = "#quote(block: true)[]\n";

/// Funktionsnamen für Bibelstellen in der Werkvorlage, in dieser Vorrangfolge (Runde 19b).
pub const SCRIPTURE_FUNCTIONS: [&str; 2] = ["bibelstelle", "bibel"];

/// Formate, die die Werkvorlage hergibt (`#let szene`, `#let arbeit`, seit Runde 19b `#let bibelstelle`/`#let bibel`
/// und `#bibliography(` für Literaturverweise) plus die eingebauten (Fußnote, Zitat, Liste …). Reihenfolge: erst die
/// der Vorlage (Szene, Arbeit, Bibelstelle, Literaturverweis), dann Fußnote, Zitat, Liste, Aufzählung, Begriff,
/// Umbruch, Link, Verweis (Runde 30a). Eine Vorlage, die eine Funktion nicht definiert, bekommt den Knopf nicht: Ohne
/// die Funktion wäre `#szene[…]` im Satz ein Fehler, ohne Literaturverzeichnis ein `@schlüssel`.
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
        push("scene", "Szene", "Szene nach der Werkvorlage (#szene)", "#szene[\n\n]\n");
    }
    if defined.iter().any(|name| name == "arbeit") {
        push("work", "Arbeit", "Arbeitsauftrag nach der Werkvorlage (#arbeit)", "#arbeit[\n\n]\n");
    }
    if let Some(name) = SCRIPTURE_FUNCTIONS.iter().find(|f| defined.iter().any(|name| name == *f)) {
        push(
            "scripture",
            "Bibelstelle",
            &format!("Bibelstelle nach der Werkvorlage (#{name}[…], z. B. Joh 3,16)"),
            &format!("#{name}[]"),
        );
    }
    if cites_literature(template) {
        push(
            "cite",
            "Literaturverweis",
            "Verweis auf einen Eintrag des Literaturverzeichnisses der Werkvorlage (@schlüssel)",
            "@",
        );
    }
    push("footnote", "Fußnote", "Fußnote an der Cursorstelle (#footnote)", "#footnote[]");
    // Runde 23a (Befund des Nutzers): Platz für die Stellenangabe unter dem Block. Runde 36: ohne vorgegebene Zeile
    // „– “ (sie blieb ungefüllt als einzelner Gedankenstrich stehen); der Hinweis nennt die Form. Eine leere
    // `attribution: []` kommt nicht in Frage: Typst setzt dafür eine Zeile mit dem Gedankenstrich (gemessen in
    // `crates/baruch-render/tests/quote_attribution.rs`). Die Leerzeilen davor und danach setzt das Widget je nach
    // Umgebung (`web/src/ribbon.ts`, `insertion`), der Cursor steht im Rumpf.
    push(
        "quote",
        "Zitat",
        "Zitat als eigener Block (#quote); die Stellenangabe kommt als eigene Zeile direkt darunter (– Stelle)",
        QUOTE_SNIPPET,
    );
    push("list", "Liste", "Liste mit Spiegelstrichen (- …)", "- \n");
    push("enum", "Aufzählung", "Nummerierte Aufzählung (1. …)", "1. \n");
    push("terms", "Begriff", "Begriffsliste (/ Begriff: Erklärung)", "/ Begriff: \n");
    push("break", "Umbruch", "Zeilenumbruch im Absatz (\\)", "\\\n");
    push("link", "Link", "Verweis auf eine Adresse (#link)", "#link(\"https://\")[]");
    // Runde 30a: Querverweis auf einen Gliederungspunkt; der Knopf öffnet den Dialog (web/src/xref.ts).
    push(
        "xref",
        "Verweis",
        "Querverweis auf einen Gliederungspunkt (#link(<kN>)[…], wahlweise mit Seitenzahl)",
        "#link(<k>)[]",
    );
    out
}

/// Hat die Vorlage ein Literaturverzeichnis (`#bibliography(` außerhalb von Kommentaren)? Dann trägt ein
/// `@schlüssel` im Text (Runde 19b).
fn cites_literature(template: &str) -> bool {
    code_lines(template).iter().any(|(_, line)| line.contains("#bibliography("))
}

/// Zeilen der Vorlage ohne Kommentare (`// …` und – auch geschachtelte – `/* … */`), je mit der Tiefe offener
/// Klammern `(`, `[`, `{` am Zeilenanfang (0 = oberste Ebene). Zeichenketten `"…"` bleiben ganz (ein `//` oder eine
/// Klammer darin zählt nicht). Runde 23a: So zählt ein `#let szene` in einem Blockkommentar oder im Rumpf einer
/// anderen Funktion nicht mehr als Definition der Vorlage. Kein Typst-Parser, aber dessen Regeln für Kommentare,
/// Zeichenketten und Klammern.
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
        // Eine Zeichenkette endet in Typst spätestens nicht am Zeilenende; zur Sicherheit hier trotzdem schließen.
        string = false;
        out.push((start_depth, code));
    }
    out
}

/// Namen der Funktionen, die die Vorlage auf oberster Ebene bereitstellt: mit `#let name(…)` bzw. `#let name =`
/// definiert (Zeilenanfang, auch eingerückt) oder – Runde 23a – mit `#import "datei.typ": name, anderer` eingeführt
/// (`*` und Umbenennungen `a as b` → `b`). Kommentare und Definitionen im Rumpf anderer Funktionen zählen nicht
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
            // Hinter dem Pfad (Zeichenkette) steht `: name, anderer as neu`; ohne Doppelpunkt kommt nur das Modul.
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
            continue; // „#letter…“: kein Leerraum nach #let
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

/// Menüband des Werks aus seiner Werkvorlage.
pub async fn ribbon(state: &AppState, work: WorkId) -> Result<Ribbon, RpcError> {
    let template = state.store.work_template(work).await?;
    Ok(Ribbon { formats: formats(&template) })
}

/// Bedient dieses Modul die RPC-Methode? `ribbon.formats`.
pub fn handles(method: &str) -> bool {
    method == "ribbon.formats"
}

/// RPC `ribbon.formats {work?}` → `{formats}`; `work` ist schon aufgelöst (fehlt → Vorgabewerk).
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

    /// Runde 23a (Gutachten R18): Kommentare, Funktionsrümpfe und `#import`.
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
        let built_in = ["footnote", "quote", "list", "enum", "terms", "break", "link", "xref"];
        let with = "#let bibelstelle(stelle) = text(style: \"italic\", stelle)\n#bibliography(\"quellen.bib\")\n";
        let mut expected = vec!["scripture", "cite"];
        expected.extend(built_in);
        assert_eq!(cmds(with), expected, "Bibelstelle und Literaturverweis aus der Vorlage vor den eingebauten");
        let all = formats(with);
        let scripture = all.iter().find(|f| f.cmd == "scripture").unwrap();
        assert_eq!(scripture.snippet, "#bibelstelle[]", "Snippet mit dem Namen der Vorlage, Inhalt in den Klammern");
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
    }

    #[test]
    fn snippets_are_valid_typst_shapes_with_german_labels() {
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
    }
}
