// SPDX-License-Identifier: AGPL-3.0-or-later
//! Display name for the origin `@lektorat:<slug>` (Round 36, remainder from Round 35).
//!
//! Revisions, posts and findings of a Lektor carry only the handle in the work and in the mirror. On the work page
//! the display name of the account stands next to it: „Erika Beispiel (@lektorat:erika)“. That is pure presentation
//! – only the handle keeps being stored and compared (attributes such as `data-post-author` stay unchanged).
//!
//! [`show`] replaces the handle only in the **text** of elements that show an origin ([`ORIGIN_CLASSES`]), never in
//! attributes and never in the remaining text of the page: book text, post text and input fields in which somebody
//! mentions a handle stay verbatim (the typeset view is read back from the DOM – a replacement there would be in
//! the book at the next save). Without accounts there are no names, the page stays the same byte for byte.

use std::collections::HashMap;

use crate::AppState;

/// Classes of the elements whose text names an origin: head of the editor (`rev-meta`), post (`post-author`),
/// revision tree (`revtree-author`), finding (`fd-persona`), line „Weitere Fassungen“ (`rev-alt`), the line
/// of a revision in the Lektor column (`rev-origin`) and the head line of a proposal in the reading flow
/// (`pv-label`, visual check of the integration: that was the only place showing just the handle; the typeset
/// view skips the pieces `.pv` when saving).
pub const ORIGIN_CLASSES: &[&str] =
    &["rev-meta", "post-author", "revtree-author", "fd-persona", "rev-alt", "rev-origin", "pv-label"];

const PREFIX: &str = "@lektorat:";

/// Elements without an end tag.
const VOID: &[&str] =
    &["area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "source", "track", "wbr"];

/// Handle → display name of all Lektor accounts with a display name. Empty without accounts.
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

/// [`show`] with the names of the instance; reads the accounts only if the HTML contains a handle at all.
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

/// Replaces every known handle in a piece of text by „Anzeigename (Handle)“.
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

/// End of the tag that begins at `html[0] == '<'` (index after `>`); `>` in attribute values does not count.
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

/// Does the tag carry one of the classes from [`ORIGIN_CLASSES`]?
fn marks_origin(tag: &str) -> bool {
    let Some(at) = tag.find(" class=\"") else { return false };
    let value = &tag[at + 8..];
    let value = &value[..value.find('"').unwrap_or(value.len())];
    value.split_ascii_whitespace().any(|class| ORIGIN_CLASSES.contains(&class))
}

/// In the text of the origin elements ([`ORIGIN_CLASSES`]) puts the display name before every known handle:
/// `@lektorat:erika` → `Erika Beispiel (@lektorat:erika)`. Everything else stays the same character for character.
pub fn show(html: &str, names: &HashMap<String, String>) -> String {
    if names.is_empty() || !html.contains(PREFIX) {
        return html.to_owned();
    }
    let mut out = String::with_capacity(html.len() + 64);
    // Open elements inside an origin element (the bottom one is the origin element itself).
    let mut open: Vec<&str> = Vec::new();
    let mut rest = html;
    while !rest.is_empty() {
        let Some(lt) = rest.find('<') else {
            if open.is_empty() {
                out.push_str(rest)
            } else {
                in_text(rest, names, &mut out)
            }
            break;
        };
        let (text, from_tag) = rest.split_at(lt);
        if open.is_empty() {
            out.push_str(text)
        } else {
            in_text(text, names, &mut out)
        }
        // Take over comments and raw text (script, style, text field) verbatim.
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
            // End tag: close up to the matching open element (forgives forgotten end tags inside).
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
