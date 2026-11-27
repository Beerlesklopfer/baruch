// SPDX-License-Identifier: AGPL-3.0-or-later
//! Editor in separate areas (wish 2026-11-06, „die Elemente voneinander entkoppeln“): header and toolbar fixed at the
//! top, the sheet fills the rest and scrolls alone, hints and „Speichern“ in the status line; the toolbar as a fragment
//! of its own; revision list, switching, changing and deleting in the accordion „Fassungen“ on the right. Without
//! JavaScript everything stays usable, with JavaScript the redraw (ui.render) hits the new containers. Sample work:
//! introduction (position 2) with revision 1, 2 (current) and proposal 3.

use crate::common;

use axum::http::StatusCode;
use serde_json::json;

fn script(name: &str) -> &'static str {
    baruch_server::SCRIPTS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, body)| *body)
        .unwrap_or_else(|| panic!("{name} fehlt"))
}

/// Position of `needle` in `html`, otherwise abort with a message.
fn at(html: &str, needle: &str) -> usize {
    html.find(needle).unwrap_or_else(|| panic!("{needle} fehlt: {html}"))
}

/// Head of a `<details data-section="…">` up to `</summary>`.
fn summary_of<'a>(html: &'a str, section: &str) -> &'a str {
    let start = at(html, &format!("data-section=\"{section}\""));
    &html[start..start + html[start..].find("</summary>").expect("summary endet")]
}

#[tokio::test]
async fn the_editor_has_head_toolbar_sheet_and_status_line_in_this_order() {
    let server = common::open().await;
    let ui = server.rpc("ui.render", json!({ "node": 2, "rev": 1 })).await;
    let editor = ui["result"]["editor"].as_str().unwrap();
    let head = at(editor, "<div class=\"editor-head\" data-zoom-zone=\"head\">");
    let toolbar = at(editor, "data-zoom-zone=\"toolbar\"");
    let body = at(editor, "<div class=\"editor-body\" data-zoom-zone=\"sheet\">");
    let form = at(editor, "<form class=\"editor-form\" id=\"editor-form\"");
    let status = at(editor, "<div class=\"editor-status\" data-zoom-zone=\"status\">");
    assert!(
        head < toolbar && toolbar < body && body < form && form < status,
        "Kopf und Werkzeugleiste stehen über dem Blatt, das Formular im Blatt, die Statuszeile darunter"
    );
    // The header keeps only the revision display, paging and the link to the current revision.
    let head_html = &editor[head..toolbar];
    assert!(head_html.contains("Fassung 1 · "), "Fassungsanzeige im Kopf: {head_html}");
    assert!(head_html.contains("data-key=\"next\""), "„›“ im Kopf, Alt+→ findet ihn dort");
    assert!(head_html.contains(">aktuell: Fassung 2</a>"), "Link zur aktuellen Fassung im Kopf");
    // Status line: hints and „Speichern“, which belongs to the form in the sheet through form=.
    let status_html = &editor[status..];
    assert!(status_html.contains("wechselt die Fassung"), "Hinweis in der Statuszeile");
    assert!(
        status_html.contains("<button class=\"btn primary\" type=\"submit\" form=\"editor-form\""),
        "„Speichern“ in der Statuszeile schickt das Editor-Formular ab, auch ohne JavaScript"
    );
}

#[test]
fn the_toolbar_is_its_own_fragment_so_it_can_become_a_ribbon_later() {
    let editor = include_str!("../templates/fragments/editor.html");
    let toolbar = include_str!("../templates/fragments/edit_toolbar.html");
    assert!(
        editor.contains("{% include \"fragments/edit_toolbar.html\" %}"),
        "der Editor bindet die Werkzeugleiste ein, statt sie selbst zu tragen (später Menüband, auch im Preprint)"
    );
    assert!(!editor.contains("data-ws-cmd"), "keine Werkzeugknöpfe mehr direkt im Editor");
    for needle in ["data-ws-mode=\"satz\"", "data-ws-mode=\"quelltext\"", "data-ws-format", "data-ws-cmd=\"italic\""] {
        assert!(toolbar.contains(needle), "{needle} steht im Fragment der Werkzeugleiste");
    }
    assert!(toolbar.contains("data-edit-toolbar hidden"), "ohne JavaScript verborgen (nur das Textfeld bleibt)");
}

#[tokio::test]
async fn the_editor_no_longer_carries_the_version_list_or_the_version_forms() {
    let server = common::open().await;
    let ui = server.rpc("ui.render", json!({ "node": 2, "rev": 1 })).await;
    let editor = ui["result"]["editor"].as_str().unwrap();
    for gone in ["revtree", "revision.delete", "revision.update", "revision.set_current", "Fassungen dieses Abschnitts"]
    {
        assert!(!editor.contains(gone), "{gone} steht jetzt im Akkordeon „Fassungen“, nicht im Editor: {editor}");
    }
    assert_eq!(
        editor.matches("<textarea name=\"body\"").count(),
        1,
        "nur noch ein Textfeld im Editor: Selektoren wie #pane-editor textarea[name=body] treffen eindeutig"
    );
}

#[tokio::test]
async fn the_versions_accordion_sits_on_the_right_with_counts_in_its_head() {
    let server = common::open().await;
    let (status, page) = server.get("/?work=1&node=2&rev=1").await;
    assert_eq!(status, StatusCode::OK, "Arbeitsseite lädt");
    let right = &page[at(&page, "id=\"pane-assistants\"")..];
    let personas = at(right, "data-section=\"personas\"");
    let revisions = at(right, "data-section=\"revisions\"");
    let chat = at(right, "data-section=\"chat\"");
    assert!(personas < revisions && revisions < chat, "Akkordeon rechts: Assistenten, Fassungen, Abschnitts-Chat");
    let head = summary_of(right, "revisions");
    assert!(head.contains("<span class=\"acc-title\">Fassungen</span>"), "Titel des Abschnitts: {head}");
    assert!(head.contains(">3</span>"), "Abzeichen: drei Fassungen, auch eingeklappt sichtbar: {head}");
    assert!(head.contains(">1 Vorschlag</span>"), "Abzeichen: ein offener Vorschlag: {head}");
    let body = &right[revisions..chat];
    assert!(body.contains("data-zoom-zone=\"revisions\""), "der Fassungsbereich ist eine eigene Zoom-Zone");
    assert!(
        body.contains("Fassungen dieses Abschnitts") && body.contains("revtree-chapter"),
        "Baumansicht im Akkordeon"
    );
    assert!(body.contains("Diese Fassung verwenden"), "Umschalten der gezeigten (nicht aktuellen) Fassung");
    assert!(body.contains("<summary class=\"btn\">Fassung ändern</summary>"), "Ändern als <details>, ohne JavaScript");

    let chat_head = &right[chat..chat + right[chat..].find('>').unwrap()];
    assert!(chat_head.contains(" open"), "ohne Cookie bleibt der Chat offen: {chat_head}");
}

#[tokio::test]
async fn the_cookie_opens_the_versions_accordion_after_reloading() {
    let server = common::open().await;
    let request = axum::http::Request::get("/?work=1&node=2&rev=1")
        .header(axum::http::header::COOKIE, "baruch_accordion=revisions")
        .body(axum::body::Body::empty())
        .unwrap();
    let (status, _, page) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "Seite lädt");
    let tag_at = at(&page, "data-section=\"revisions\"");
    let tag = &page[tag_at..tag_at + page[tag_at..].find('>').unwrap()];
    assert!(tag.ends_with(" open"), "das zuletzt geöffnete Akkordeon „Fassungen“ ist wieder offen: {tag}");
    let chat = at(&page, "data-section=\"chat\"");
    assert!(!page[chat..chat + page[chat..].find('>').unwrap()].contains(" open"), "nur ein Abschnitt offen");
}

#[tokio::test]
async fn deleting_a_version_works_without_javascript_from_the_accordion() {
    let server = common::open().await;
    let (_, page) = server.get("/?work=1&node=2&rev=1").await;
    let right = &page[at(&page, "id=\"pane-assistants\"")..];
    let form = &right[at(right, "<form class=\"rev-delete\" method=\"post\" action=\"/act\" data-rpc>")..];
    let form = &form[..form.find("</form>").unwrap()];
    assert!(form.contains("value=\"revision.delete\""), "Löschen-Formular im Akkordeon: {form}");
    assert!(
        form.contains("name=\"confirm\" value=\"1\" required"),
        "nur mit angehaktem Kästchen (Mensch, Bestätigung)"
    );
    assert!(form.contains("<input type=\"hidden\" name=\"rev\" value=\"1\">"), "löscht die gezeigte Fassung");

    // Without the checkbox the server rejects; with the checkbox revision 1 is gone, and it goes back to the section.
    let key = server.rpc("node.get", json!({ "node": 2 })).await["result"]["key"].as_i64().unwrap();
    let fields = format!("method=revision.delete&work=1&node=2&key={key}&rev=1&view=edit");
    let (refused, _) = server.post_form("/act", &fields).await;
    assert_ne!(refused, StatusCode::SEE_OTHER, "ohne Bestätigung wird nichts gelöscht");
    let (status, location) = server.post_form("/act", &format!("{fields}&confirm=1")).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Löschen per Formular ohne JavaScript");
    assert!(location.unwrap_or_default().contains(&format!("key={key}")), "zurück zum Abschnitt");
    let node = server.rpc("node.get", json!({ "node": 2 })).await["result"].clone();
    let numbers: Vec<i64> =
        node["revisions"].as_array().unwrap().iter().map(|r| r["number"].as_i64().unwrap()).collect();
    assert!(!numbers.contains(&1), "Fassung 1 ist gelöscht: {numbers:?}");

    // The current revision stays locked, in the accordion too.
    let (_, current) = server.get("/?work=1&node=2&rev=2").await;
    let right = &current[at(&current, "id=\"pane-assistants\"")..];
    assert!(
        right.contains("<button class=\"btn danger\" type=\"submit\" disabled>Löschen</button>"),
        "die aktuelle Fassung lässt sich nicht löschen"
    );
}

#[tokio::test]
async fn the_preview_has_head_sheet_and_status_line_but_no_toolbar() {
    let server = common::open().await;
    let ui = server.rpc("ui.render", json!({ "node": 2, "view": "preview" })).await;
    let editor = ui["result"]["editor"].as_str().unwrap();
    let head = at(editor, "data-zoom-zone=\"head\"");
    let body = at(editor, "<div class=\"editor-body\" data-zoom-zone=\"sheet\">");
    let sheet = at(editor, "<div class=\"sheet\">");
    let status = at(editor, "data-zoom-zone=\"status\"");
    assert!(head < body && body < sheet && sheet < status, "Vorschau: Kopf, Blatt (scrollt allein), Statuszeile");
    assert!(editor[status..].contains("Vorschau im Browser (Näherung)"), "Hinweis der Vorschau in der Statuszeile");
    assert!(!editor.contains("data-edit-toolbar"), "die Vorschau bearbeitet nichts und hat keine Werkzeugleiste");
}

#[test]
fn the_sheet_fills_the_editor_height_and_scrolls_alone() {
    let css = include_str!("../static/app.css");
    let rule = |selector: &str| -> &str {
        let at = css.find(&format!("{selector} {{")).unwrap_or_else(|| panic!("Regel {selector} fehlt"));
        &css[at..at + css[at..].find('}').unwrap()]
    };
    assert!(
        rule("#pane-editor").contains("overflow: hidden"),
        "der Editor scrollt nicht als Ganzes: Kopf, Werkzeugleiste und Statuszeile bleiben stehen"
    );
    let grows = rule("#pane-editor > .wysiwyg, #pane-editor > .editor-body");
    assert!(
        grows.contains("flex: 1 1 0") && grows.contains("min-height: 0"),
        "das Blatt nimmt die restliche Höhe und darf kleiner als sein Inhalt sein (sonst schöbe es die Statuszeile hinaus): {grows}"
    );
    assert!(rule("\n.editor-body").contains("overflow: auto"), "nur das Blatt scrollt");
    assert!(
        !include_str!("../static/proposals.css").contains("#pane-editor > * { flex-shrink: 0; }"),
        "die alte Regel, nach der kein Kind des Editors schrumpfte (Ursache des niedrigen Blatts), ist fort"
    );
}

#[test]
fn redrawing_with_javascript_finds_the_new_containers() {
    let wysiwyg = script("wysiwyg.js");
    assert!(
        wysiwyg.contains("root.querySelector(\"form[data-widget=editor]\")"),
        "das Widget findet das Formular in seinem Bereich (früher lag der Bereich im Formular)"
    );
    assert!(wysiwyg.contains("[data-edit-toolbar]"), "das Widget zeigt die ohne JavaScript verborgene Werkzeugleiste");
    let widgets = script("widgets.js");
    assert!(
        widgets.contains("form.elements"),
        "„Speichern“ wird über form.elements gefunden, auch mit form= außerhalb"
    );
    assert!(script("panes.js").contains("submitButton(form)"), "das Editor-Widget nutzt denselben Weg");
    let workspace = script("workspace.js");
    assert!(
        workspace.contains("[\"tree\", \"editor\", \"assistants\"]"),
        "ui.render ersetzt weiter die drei Spalten; das Akkordeon „Fassungen“ kommt mit der rechten"
    );
    let panes = script("panes.js");
    assert!(panes.contains("details[data-section]"), "das Akkordeon-Widget kennt jeden Abschnitt, auch „Fassungen“");
}

#[test]
fn the_sheet_fills_the_rest_also_inside_the_editor_so_the_status_line_stays_at_the_bottom() {
    let css = include_str!("../static/app.css");
    assert!(
        css.contains(".wysiwyg > .editor-body { flex: 1 1 0; min-height: 0; }"),
        "im Editor liegt das Blatt in div.wysiwyg; ohne diese Regel ist es nur so hoch wie der Text und die \
         Statuszeile springt beim Wechsel des Abschnitts (Befund des Nutzers 2026-11-06)"
    );
}

#[test]
fn the_editor_head_stays_on_one_line_so_long_titles_do_not_push_the_toolbar_down() {
    let css = include_str!("../static/app.css");
    assert!(
        css.contains(".editor-head { display: flex; align-items: center; gap: 12px; flex-wrap: nowrap;"),
        "ein umbrechender Kopf verschiebt Werkzeugleiste und Blatt je nach Titellänge (Befund 2026-11-06)"
    );
    assert!(css.contains("text-overflow: ellipsis; white-space: nowrap; }"), "lange Titel kürzen sich mit …");
    let page = include_str!("../templates/fragments/editor.html");
    assert!(page.contains("class=\"node-title\" title="), "der volle Titel bleibt im title lesbar");
}
