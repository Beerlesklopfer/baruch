// SPDX-License-Identifier: AGPL-3.0-or-later
//! Anzeigename zur Herkunft `@lektorat:<slug>` (Runde 36, Rest aus Runde 35).
//!
//! Fassungen, Beiträge und Befunde eines Lektors tragen im Werk und im Spiegel nur den Handle. Auf der Arbeitsseite
//! steht dazu der Anzeigename des Kontos: „Erika Beispiel (@lektorat:erika)“. Das ist reine Darstellung – gespeichert
//! und verglichen wird weiter nur der Handle (Attribute wie `data-post-author` bleiben unverändert).
//!
//! [`show`] ersetzt den Handle nur im **Text** von Elementen, die eine Herkunft zeigen ([`ORIGIN_CLASSES`]), nie in
//! Attributen und nie im übrigen Text der Seite: Buchtext, Beitragstext und Eingabefelder, in denen jemand einen
//! Handle erwähnt, bleiben wörtlich (das Satzbild wird aus dem DOM zurückgelesen – eine Ersetzung dort stünde beim
//! nächsten Speichern im Buch). Ohne Konten gibt es keine Namen, die Seite bleibt Byte für Byte gleich.

use std::collections::HashMap;

use crate::AppState;

/// Klassen der Elemente, deren Text eine Herkunft nennt: Kopf des Editors (`rev-meta`), Beitrag (`post-author`),
/// Revisionsbaum (`revtree-author`), Befund (`fd-persona`), Zeile „Weitere Fassungen“ (`rev-alt`) und die Zeile
/// einer Fassung in der Lektor-Spalte (`rev-origin`).
pub const ORIGIN_CLASSES: &[&str] = &["rev-meta", "post-author", "revtree-author", "fd-persona", "rev-alt", "rev-origin"];

const PREFIX: &str = "@lektorat:";

/// Elemente ohne Endtag.
const VOID: &[&str] =
    &["area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source", "track", "wbr"];

/// Handle → Anzeigename aller Lektor-Konten mit einem Anzeigenamen. Ohne Konten leer.
pub async fn display_names(state: &AppState) -> HashMap<String, String> {
    if !crate::auth::accounts_exist(state).await {
        return HashMap::new();
    }
    match state.store.account_list().await {
        Ok(accounts) => accounts
            .into_iter()
            .filter(|account| account.kind == "lektor" && !account.display.trim().is_empty())
            .map(|account| (crate::auth::lektor_handle(&account.name), account.display.trim().to_owned()))
            .collect(),
        Err(e) => {
            eprintln!("Konten: Anzeigenamen nicht lesbar: {e}");
            HashMap::new()
        }
    }
}

/// [`show`] mit den Namen der Instanz; liest die Konten nur, wenn im HTML überhaupt ein Handle steht.
pub async fn show_for(state: &AppState, html: String) -> String {
    if !html.contains(PREFIX) {
        return html;
    }
    let names = display_names(state).await;
    if names.is_empty() { html } else { show(&html, &names) }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Ersetzt in einem Stück Text jeden bekannten Handle durch „Anzeigename (Handle)“.
fn in_text(text: &str, names: &HashMap<String, String>, out: &mut String) {
    let mut rest = text;
    while let Some(at) = rest.find(PREFIX) {
        out.push_str(&rest[..at]);
        let tail = &rest[at + PREFIX.len()..];
        let end = tail.find(|c: char| !(c.is_alphanumeric() || c == '-' || c == '_')).unwrap_or(tail.len());
        let handle = &rest[at..at + PREFIX.len() + end];
        match names.get(handle) {
            Some(display) => {
                out.push_str(&escape(display));
                out.push_str(" (");
                out.push_str(handle);
                out.push(')');
            }
            None => out.push_str(handle),
        }
        rest = &rest[at + PREFIX.len() + end..];
    }
    out.push_str(rest);
}

/// Ende des Tags, das bei `html[0] == '<'` beginnt (Index hinter `>`); `>` in Attributwerten zählt nicht.
fn tag_end(html: &str) -> usize {
    let mut quote = None;
    for (i, c) in html.char_indices() {
        match (quote, c) {
            (None, '"' | '\'') => quote = Some(c),
            (Some(q), c) if q == c => quote = None,
            (None, '>') => return i + 1,
            _ => {}
        }
    }
    html.len()
}

/// Trägt das Tag eine der Klassen aus [`ORIGIN_CLASSES`]?
fn marks_origin(tag: &str) -> bool {
    let Some(at) = tag.find(" class=\"") else { return false };
    let value = &tag[at + 8..];
    let value = &value[..value.find('"').unwrap_or(value.len())];
    value.split_ascii_whitespace().any(|class| ORIGIN_CLASSES.contains(&class))
}

/// Setzt im Text der Herkunfts-Elemente ([`ORIGIN_CLASSES`]) zu jedem bekannten Handle den Anzeigenamen davor:
/// `@lektorat:erika` → `Erika Beispiel (@lektorat:erika)`. Alles andere bleibt Zeichen für Zeichen gleich.
pub fn show(html: &str, names: &HashMap<String, String>) -> String {
    if names.is_empty() || !html.contains(PREFIX) {
        return html.to_owned();
    }
    let mut out = String::with_capacity(html.len() + 64);
    // Offene Elemente innerhalb eines Herkunfts-Elements (das unterste ist das Herkunfts-Element selbst).
    let mut open: Vec<&str> = Vec::new();
    let mut rest = html;
    while !rest.is_empty() {
        let Some(lt) = rest.find('<') else {
            if open.is_empty() { out.push_str(rest) } else { in_text(rest, names, &mut out) }
            break;
        };
        let (text, from_tag) = rest.split_at(lt);
        if open.is_empty() { out.push_str(text) } else { in_text(text, names, &mut out) }
        // Kommentare und Rohtext (Skript, Stil, Textfeld) wörtlich übernehmen.
        let raw_end = if from_tag.starts_with("<!--") {
            Some(from_tag.find("-->").map_or(from_tag.len(), |i| i + 3))
        } else {
            ["script", "style", "textarea"].iter().find_map(|name| {
                let opens = from_tag[1..].starts_with(name)
                    && from_tag[1 + name.len()..].starts_with(|c: char| c == '>' || c.is_ascii_whitespace());
                opens.then(|| {
                    let close = format!("</{name}");
                    from_tag.find(&close).map_or(from_tag.len(), |i| i + tag_end(&from_tag[i..]))
                })
            })
        };
        if let Some(end) = raw_end {
            out.push_str(&from_tag[..end]);
            rest = &from_tag[end..];
            continue;
        }
        let end = tag_end(from_tag);
        let tag = &from_tag[..end];
        out.push_str(tag);
        rest = &from_tag[end..];
        let inner = tag.trim_start_matches('<').trim_end_matches('>');
        if let Some(name) = inner.strip_prefix('/') {
            // Endtag: bis zum passenden offenen Element schließen (verzeiht vergessene Endtags im Inneren).
            let name = name.trim();
            if let Some(at) = open.iter().rposition(|o| o.eq_ignore_ascii_case(name)) {
                open.truncate(at);
            }
            continue;
        }
        let name = inner.split(|c: char| c.is_ascii_whitespace() || c == '/').next().unwrap_or("");
        let is_void = inner.ends_with('/') || VOID.iter().any(|v| v.eq_ignore_ascii_case(name));
        if name.is_empty() || name.starts_with('!') || is_void {
            continue;
        }
        if !open.is_empty() || marks_origin(tag) {
            open.push(name);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names() -> HashMap<String, String> {
        HashMap::from([("@lektorat:erika".to_owned(), "Erika <Beispiel>".to_owned())])
    }

    #[test]
    fn the_name_appears_only_in_the_text_of_origin_elements() {
        let html = "<div class=\"post-item\" data-post-author=\"@lektorat:erika\">\
                    <div class=\"post-author\"><span class=\"pc-swatch\"></span>@lektorat:erika <span>10:00</span></div>\
                    <div class=\"post-body\">Frag @lektorat:erika.</div></div>";
        let shown = show(html, &names());
        assert_eq!(
            shown,
            "<div class=\"post-item\" data-post-author=\"@lektorat:erika\">\
             <div class=\"post-author\"><span class=\"pc-swatch\"></span>Erika &lt;Beispiel&gt; (@lektorat:erika) \
             <span>10:00</span></div><div class=\"post-body\">Frag @lektorat:erika.</div></div>",
            "nur der Text des Elements post-author bekommt den (maskierten) Namen; Attribut und Beitragstext bleiben"
        );
    }

    #[test]
    fn unknown_handles_longer_handles_and_raw_text_stay() {
        let html = "<span class=\"muted small rev-meta\">Fassung 2 · @lektorat:erika-m · @lektorat:gustav · \
                    @lektorat:erika</span><textarea class=\"rev-meta\">@lektorat:erika</textarea>\
                    <p>@lektorat:erika</p><!-- <b class=\"rev-meta\"> -->@lektorat:erika";
        let shown = show(html, &names());
        assert_eq!(
            shown,
            "<span class=\"muted small rev-meta\">Fassung 2 · @lektorat:erika-m · @lektorat:gustav · \
             Erika &lt;Beispiel&gt; (@lektorat:erika)</span><textarea class=\"rev-meta\">@lektorat:erika</textarea>\
             <p>@lektorat:erika</p><!-- <b class=\"rev-meta\"> -->@lektorat:erika",
            "ein längerer Handle ist ein anderes Konto; Textfeld, Kommentar und Text nach dem Element bleiben wörtlich"
        );
    }

    #[test]
    fn without_names_or_handles_the_page_is_returned_unchanged() {
        let html = "<span class=\"rev-meta\">@lektorat:erika</span>";
        assert_eq!(show(html, &HashMap::new()), html, "ohne Konten bleibt die Seite Byte für Byte gleich");
        let plain = "<span class=\"rev-meta\">Autor</span><input value=\"a>b\"><br>";
        assert_eq!(show(plain, &names()), plain, "ohne Handle wird nichts angefasst");
    }

    #[test]
    fn a_forgotten_end_tag_inside_does_not_leak_into_the_rest_of_the_page() {
        let html = "<li class=\"rev-alt\"><span>@lektorat:erika<input name=\"a\"></li><p>@lektorat:erika</p>";
        assert_eq!(
            show(html, &names()),
            "<li class=\"rev-alt\"><span>Erika &lt;Beispiel&gt; (@lektorat:erika)<input name=\"a\"></li>\
             <p>@lektorat:erika</p>",
            "das Endtag des Herkunfts-Elements beendet die Ersetzung, auch wenn innen ein Endtag fehlt"
        );
    }
}
