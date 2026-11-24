// SPDX-License-Identifier: AGPL-3.0-or-later
//! HTML des Satzbilds und des Satzbild-Diffs: alles maskiert, Inseln nicht bearbeitbar, Blöcke mit
//! Index und Art.

use baruch_wysiwyg::{ImageRef, diff_html, diff_html_with, image_path, parse, render_html, render_html_with};

#[test]
fn every_text_is_escaped() {
    let doc = parse(
        "= <script>alert(1)</script> & \"Anführung\"\n\nText \\<b\\> & 'x' \"y\"#footnote[<img src=x onerror=alert(1)>]\n\n#zeig(\"</div><script>\")\n",
    );
    let html = render_html(&doc);
    assert!(!html.contains("<script"), "kein roher script-Tag: {html}");
    assert!(!html.contains("<img"), "kein roher img-Tag: {html}");
    assert!(!html.contains("<b>"), "kein roher b-Tag: {html}");
    assert!(html.contains("&lt;script&gt;"), "Tag als Text: {html}");
    assert!(html.contains("&amp;"), "& maskiert: {html}");
    assert!(html.contains("&quot;Anführung&quot;"), "Anführungszeichen maskiert: {html}");
    assert!(html.contains("&#39;x&#39;"), "Apostroph maskiert: {html}");
    assert!(
        html.contains("data-raw=\"#zeig(&quot;&lt;/div&gt;&lt;script&gt;&quot;)"),
        "Rohtext im Attribut maskiert: {html}"
    );
}

#[test]
fn islands_are_not_editable_and_blocks_carry_index_and_kind() {
    let doc = parse("= Titel <t>\n\nEine Formel $x$ im Satz.\n\n#let zahl = 3\n\n- a\n- b\n");
    let html = render_html(&doc);
    assert!(
        html.contains("<div class=\"ws-block\" data-b=\"0\" data-kind=\"heading\" data-level=\"1\""),
        "Überschrift: {html}"
    );
    assert!(html.contains("data-b=\"1\" data-kind=\"paragraph\""), "Absatz mit Index 1: {html}");
    assert!(
        html.contains("data-b=\"2\" data-kind=\"island\" contenteditable=\"false\""),
        "Insel nicht bearbeitbar: {html}"
    );
    assert!(html.contains("<code class=\"ws-raw\">#let zahl = 3</code>"), "Rohtext der Insel sichtbar: {html}");
    assert!(
        html.contains(
            "<span class=\"ws-island\" contenteditable=\"false\" title=\"Typst-Insel\" data-raw=\"$x$\">$x$</span>"
        ),
        "Formel als Insel im Satz: {html}"
    );
    assert!(
        html.contains("<span class=\"ws-label\" contenteditable=\"false\" data-v=\"t\">&lt;t&gt;</span>"),
        "Label: {html}"
    );
    assert!(html.contains("data-kind=\"list\" data-ordered=\"false\"><ul><li>a</li><li>b</li></ul>"), "Liste: {html}");
}

#[test]
fn markup_becomes_elements() {
    let html = render_html(&parse("Ein _leiser_ und *fester* Satz#footnote[Anm.]\nweiter.~Ende #quote[Zitat]\n"));
    assert!(html.contains("<em>leiser</em>"), "Hervorhebung als em: {html}");
    assert!(html.contains("<strong>fester</strong>"), "Fett als strong: {html}");
    assert!(html.contains("<span class=\"ws-footnote\">Anm.</span>"), "Fußnote: {html}");
    assert!(html.contains("<span class=\"ws-soft\"> </span>"), "Zeilenumbruch als Leerzeichen: {html}");
    assert!(html.contains("<span class=\"ws-nbsp\">&nbsp;</span>"), "geschütztes Leerzeichen bleibt erkennbar: {html}");
    assert!(html.contains("<q class=\"ws-quote\">Zitat</q>"), "Zitat im Satz: {html}");
}

#[test]
fn the_diff_marks_removed_and_added_blocks() {
    let base = "= Titel\n\nErster Absatz.\n\nZweiter Absatz.\n";
    let proposal = "= Titel\n\nErster Absatz.\n\nZweiter Absatz, *neu*.\n\nDritter <x>.\n";
    let html = diff_html(base, proposal);
    assert_eq!(html.matches("class=\"ws-same\"").count(), 2, "Titel und erster Absatz gleich: {html}");
    assert_eq!(html.matches("class=\"ws-del\"").count(), 1, "der alte zweite Absatz entfernt: {html}");
    assert_eq!(html.matches("class=\"ws-ins\"").count(), 2, "neuer zweiter und dritter Absatz: {html}");
    assert!(html.contains("<strong>neu</strong>"), "Diff im Satzbild, nicht als Quelltext: {html}");
    assert!(!html.contains("data-b="), "im Diff ist nichts bearbeitbar: {html}");
    let same = diff_html("Absatz.\n", "Absatz.\n\n");
    assert!(!same.contains("ws-del") && !same.contains("ws-ins"), "Leerraum am Ende zählt nicht: {same}");
}

/// Bilder für die Tests: jeder Pfad unter `/bilder/` hat ein Bild, mit Zeichen, die maskiert werden.
fn images(path: &str) -> Option<ImageRef> {
    path.starts_with("/bilder/").then(|| ImageRef {
        src: format!("/werke/1/bilder/pfad?p={}&x=\"1\"", path.replace('/', "%2F")),
        alt: "Grundriss <Werkstatt> & Hof".into(),
        missing: false,
    })
}

#[test]
fn an_image_island_shows_the_picture_and_stays_an_island() {
    // Runde 7: eine Insel mit Bild erscheint als Bild (ws-figure), nicht als Typst-Rohtext.
    let source = "Davor.\n\n#figure(image(\"/bilder/erfunden.svg\"), caption: [Ein Grundriss.])\n\nIm Satz #image(\"/bilder/klein.png\") und // #image(\"/bilder/kommentar.png\")\nweiter.\n";
    let doc = parse(source);
    let html = render_html_with(&doc, &images);
    let island = html.find("<div class=\"ws-block ws-figure\"").expect("die Abbildung ist eine Block-Insel mit Bild");
    let block = &html[island..island + html[island..].find("</div>").unwrap()];
    assert!(
        block.starts_with(
            "<div class=\"ws-block ws-figure\" data-b=\"1\" data-kind=\"island\" data-image=\"1\" contenteditable=\"false\" data-raw=\""
        ),
        "Insel mit Index, Art, Bildmarke, nicht bearbeitbar: {block}"
    );
    assert!(
        block.contains("data-raw=\"#figure(image(&quot;/bilder/erfunden.svg&quot;), caption: [Ein Grundriss.])"),
        "der Rohtext bleibt in data-raw (byte-gleich zurück): {block}"
    );
    assert!(
        block.contains("title=\"Bild – Doppelklick öffnet den Quelltext an Ort und Stelle\""),
        "Hinweis im title: {block}"
    );
    assert!(
        block.contains(
            "<img class=\"ws-image\" src=\"/werke/1/bilder/pfad?p=%2Fbilder%2Ferfunden.svg&amp;x=&quot;1&quot;\" alt=\"Grundriss &lt;Werkstatt&gt; &amp; Hof\" loading=\"eager\">"
        ),
        "Bild mit maskierten Attributen in der Insel: {block}"
    );
    assert!(block.contains("<span class=\"ws-caption\">Ein Grundriss.</span>"), "Unterschrift aus caption: {block}");
    assert!(!block.contains("<code"), "kein sichtbarer Rohtext: {block}");
    assert!(
        html.contains(
            "<span class=\"ws-island ws-inline-image\" contenteditable=\"false\" title=\"Bild – Doppelklick öffnet den Quelltext an Ort und Stelle\" data-raw=\"#image(&quot;/bilder/klein.png&quot;)\"><img class=\"ws-image\""
        ),
        "auch eine Insel im Satz zeigt ihr Bild statt des Rohtexts: {html}"
    );
    assert!(!html.contains("kommentar.png\" alt"), "ein Bild im Kommentar wird nicht gezeigt: {html}");
    assert_eq!(html.matches("<img").count(), 2, "genau zwei Bilder: {html}");
    let plain = render_html(&doc);
    assert!(!plain.contains("<img") && !plain.contains("ws-figure"), "render_html zeigt keine Bilder: {plain}");
    assert!(plain.contains("<code class=\"ws-raw\">#figure"), "ohne Bild wie bisher der Rohtext: {plain}");
    assert_eq!(plain, render_html_with(&doc, &|_| None), "render_html ist render_html_with ohne Bilder");
    let unknown = render_html_with(&parse("#image(\"/anderswo/x.png\")\n"), &images);
    assert!(!unknown.contains("<img"), "ohne Bild vom Rückruf bleibt die Insel wie bisher: {unknown}");
}

#[test]
fn a_figure_without_caption_shows_the_alt_text_and_attributes_stay_escaped() {
    let tricky =
        |_: &str| Some(ImageRef { src: "/x?p=\"><script>".into(), alt: "Anna's \"Laube\" <&>".into(), missing: false });
    let html = render_html_with(&parse("#image(\"/bilder/laube.svg\")\n"), &tricky);
    assert!(!html.contains("<script"), "nichts Rohes aus src: {html}");
    assert!(html.contains("src=\"/x?p=&quot;&gt;&lt;script&gt;\""), "src maskiert: {html}");
    assert!(
        html.contains("<span class=\"ws-caption\">Anna&#39;s &quot;Laube&quot; &lt;&amp;&gt;</span>"),
        "ohne caption steht der Alternativtext maskiert darunter: {html}"
    );
    let empty = render_html_with(&parse("#image(\"/bilder/laube.svg\")\n"), &|_| Some(ImageRef::default()));
    assert!(empty.contains("<span class=\"ws-caption\">/bilder/laube.svg</span>"), "sonst der Pfad: {empty}");
}

#[test]
fn saving_without_changes_keeps_an_image_island_byte_for_byte() {
    let source = "= Laube\n\n#figure(image(\"./bilder/laube.svg\"), caption: [Anna\\'s Laube.])\n\nText.\n";
    let doc = parse(source);
    let keep: Vec<baruch_wysiwyg::Edit> =
        (0..doc.blocks.len()).map(|keep| baruch_wysiwyg::Edit::Keep { keep }).collect();
    assert_eq!(baruch_wysiwyg::serialize(&doc, &keep).unwrap(), source, "alle Blöcke keep → byte-gleich");
    let html = render_html_with(&doc, &|path| {
        Some(ImageRef { src: format!("/b?p={path}"), alt: String::new(), missing: false })
    });
    assert!(html.contains("ws-figure"), "die Insel erscheint als Bild: {html}");
    assert!(html.contains("Anna\\&#39;s Laube."), "Unterschrift wörtlich aus dem Quelltext: {html}");
}

#[test]
fn image_path_finds_the_path_of_a_figure_but_not_in_comments() {
    assert_eq!(
        image_path("#figure(image(\"/bilder/erfunden.svg\"), caption: [Ein Grundriss.])").as_deref(),
        Some("/bilder/erfunden.svg"),
        "Pfad aus der Abbildung"
    );
    assert_eq!(image_path("// #figure(image(\"/bilder/erfunden.svg\"))"), None, "nicht im Kommentar");
    assert_eq!(image_path("#figure(image(\"/bilder/\" + name))"), None, "berechnete Pfade nicht");
}

#[test]
fn heading_labels_and_scene_arguments_are_shown_but_not_editable() {
    let html = render_html(&parse("= Werkstatt\n<werkstatt>\n\n#szene(ort: [Hof])[\n  Text.\n]\n"));
    assert!(
        html.contains(
            "<span class=\"ws-label ws-heading-label\" contenteditable=\"false\" data-heading-label=\"1\" data-v=\"werkstatt\">&lt;werkstatt&gt;</span></div>"
        ),
        "Label am Überschriftsblock: {html}"
    );
    assert!(
        html.contains(
            "data-kind=\"scene\"><div class=\"ws-args\" contenteditable=\"false\" title=\"Typst-Insel: nur im Quelltext bearbeitbar\" data-raw=\"ort: [Hof]\">(ort: [Hof])</div><div class=\"ws-body\">"
        ),
        "Argumente der Szene als Insel vor dem Rumpf: {html}"
    );
    let plain = render_html(&parse("= Titel <t>\n\n#szene[Text.]\n"));
    assert!(!plain.contains("ws-heading-label") && !plain.contains("ws-args"), "ohne beides wie bisher: {plain}");
}

#[test]
fn the_diff_shows_images_too() {
    let html = diff_html_with("Text.\n", "Text.\n\n#image(\"/bilder/neu.png\")\n", &images);
    assert!(
        html.contains("class=\"ws-ins\"") && html.contains("<img class=\"ws-image\""),
        "neues Bild im Diff: {html}"
    );
    assert_eq!(diff_html("a\n", "b\n"), diff_html_with("a\n", "b\n", &|_| None), "diff_html ohne Bilder");
}

#[test]
fn a_missing_image_is_reported_and_the_alt_text_is_visible() {
    // Runde 11: Zum Pfad gibt es kein Bild → deutliche Meldung statt eines leeren Rahmens.
    let gone = |_: &str| Some(ImageRef { src: "/x".into(), alt: String::new(), missing: true });
    let html = render_html_with(&parse("#figure(image(\"bilder/<weg>.png\"), caption: [Weg])\n"), &gone);
    assert!(
        html.contains(
            "<span class=\"ws-missing\" role=\"note\">Bild fehlt: <code>bilder/&lt;weg&gt;.png</code></span>"
        ),
        "Meldung mit maskiertem Pfad: {html}"
    );
    assert!(!html.contains("<img"), "kein Bild, das ohnehin 404 brächte: {html}");
    assert!(html.contains("data-raw=\"#figure(image("), "die Insel bleibt mit ihrem Rohtext: {html}");
    let inline = render_html_with(&parse("Im Satz #image(\"a.png\") weiter.\n"), &gone);
    assert!(inline.contains("Bild fehlt: <code>a.png</code>"), "auch ein Bild im Satz: {inline}");

    let with_alt = |_: &str| Some(ImageRef { src: "/b".into(), alt: "Zwei <Bäume>".into(), missing: false });
    let html = render_html_with(&parse("#figure(image(\"b.png\"), caption: [Der Hof])\n"), &with_alt);
    assert!(
        html.contains(
            "<span class=\"ws-caption\">Der Hof</span><span class=\"ws-alt\">Alternativtext: Zwei &lt;Bäume&gt;</span>"
        ),
        "Unterschrift und darunter der Alternativtext, maskiert: {html}"
    );
    let only_alt = render_html_with(&parse("#image(\"b.png\")\n"), &with_alt);
    assert!(
        !only_alt.contains("ws-alt"),
        "steht der Alternativtext schon als Unterschrift da, nicht doppelt: {only_alt}"
    );
}

/// Runde 21b (Sichtprüfung R18): Nach einem erzwungenen Umbruch `\` verschluckt Typst das Zeilenende; das Satzbild darf
/// die nächste Zeile darum nicht mit einem Leerzeichen einrücken. Das Element bleibt (das Widget liest `soft` zurück).
#[test]
fn after_a_forced_break_the_next_line_starts_without_a_space() {
    let html = render_html(&parse("Erste Zeile \\\nzweite Zeile.\n"));
    assert!(html.contains("data-linebreak=\"1\""), "Testvoraussetzung: der Umbruch ist ein Umbruch-Element: {html}");
    assert!(
        html.contains("<span class=\"ws-soft\" data-after-break=\"1\"></span>zweite Zeile"),
        "das Zeilenende hinter dem Umbruch ist leer (keine Einrückung), steht aber noch da: {html}"
    );
    let plain = render_html(&parse("Erste Zeile\nzweite Zeile.\n"));
    assert!(plain.contains("<span class=\"ws-soft\"> </span>zweite"), "ohne Umbruch bleibt das Leerzeichen: {plain}");
}

/// Runde 21b (Sichtprüfung R18): Eine Bildinsel mit mehreren Bildern nennt alle Pfade, nicht nur den ersten.
#[test]
fn an_image_island_with_two_images_names_both_paths() {
    let plain = |path: &str| {
        path.starts_with("/bilder/").then(|| ImageRef {
            src: format!("/b?p={path}"),
            alt: String::new(),
            missing: false,
        })
    };
    let source = "#stack(dir: ltr, image(\"/bilder/links.png\"), image(\"/bilder/rechts.png\"))\n";
    let html = render_html_with(&parse(source), &plain);
    assert_eq!(html.matches("<img").count(), 2, "beide Bilder erscheinen: {html}");
    assert!(
        html.contains("<span class=\"ws-caption\">/bilder/links.png · /bilder/rechts.png</span>"),
        "ohne caption nennt die Unterschrift jedes Bild (ohne Alternativtext: den Pfad): {html}"
    );
    assert!(
        html.contains("<span class=\"ws-alt ws-paths\">Bilder: /bilder/links.png · /bilder/rechts.png</span>"),
        "die Pfadzeile nennt beide Pfade: {html}"
    );
    let captioned = render_html_with(
        &parse("#figure(stack(image(\"/bilder/a.png\"), image(\"/bilder/b.png\")), caption: [Zwei Fenster])\n"),
        &images,
    );
    assert!(
        captioned.contains("<span class=\"ws-caption\">Zwei Fenster</span>"),
        "eine geschriebene Unterschrift gilt"
    );
    assert!(captioned.contains("Bilder: /bilder/a.png · /bilder/b.png"), "auch dann beide Pfade darunter: {captioned}");
}
