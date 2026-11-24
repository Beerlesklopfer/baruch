// SPDX-License-Identifier: AGPL-3.0-or-later
//! Satz-Einstellungen im Satz (Runde 29b, Vertrag §8.2): Werk ohne Einstellungen byte-gleich, Hinweis zur Vorlagenregel
//! auf der Satz-Seite, Umschlag-Seitenmaß und Rücken.
//!
//! Besitz: satz-typst. Das Speichern der Einstellungen gehört satz-daten (in diesem Zweig noch ehrlich abgelehnt); die
//! Wirkung gespeicherter Einstellungen im Server prüft die Abnahme (`round29_abnahme.rs`), die Regeln selbst messen
//! `baruch-typst/tests/typeset_*.rs` und `baruch-render/tests/typeset_measure.rs`. Alle Texte frei erfunden.

use crate::common;

use baruch_typst::typeset::{CANONICAL_CHAPTER_BREAK, PageFormat, RULES_MARK, TypesetSettings};
use serde_json::{Value, json};

/// Erfundenes Küstenbuch: Werkvorlage mit eigenem Format, ein Teil, drei Kapitel mit Fußnoten.
fn coast_book(template: &str) -> String {
    let para = "Die Fähre legte bei Flut an, und die Fahrgäste trugen ihre Taschen über den nassen Steg. \
                Am Kiosk gab es Tee in dicken Bechern, und der Wind riss an den Fahnen über dem Anleger.";
    let mut out = format!("{template}\n");
    out.push_str(&baruch_typst::part_line("Die Insel"));
    for (i, title) in ["Die Überfahrt", "Der Leuchtturm", "Die Heimfahrt"].iter().enumerate() {
        out.push_str(&format!("\n= {title}\n\n"));
        for p in 0..6 {
            out.push_str(para);
            if p == 0 {
                out.push_str(&format!("#footnote[Erfundene Anmerkung {}.]", i + 1));
            }
            out.push_str("\n\n");
        }
    }
    out
}

/// Werkvorlage mit eigenem Titel (der Import nimmt ihn als Werktitel; Titel sind je Datenordner eindeutig).
fn template(title: &str) -> String {
    format!("#set document(title: \"{title}\")\n#set page(width: 120mm, height: 190mm)")
}

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC-Fehler: {answer}");
    &answer["result"]
}

/// Goldstände aus dem Stand der Schnittstelle R29-S1 (09f4f9d, `typeset.rs` und `cover.rs` von dort, gemessen mit
/// diesem Test): Stand der Satzquelle und Seitenzahl des Küstenbuchs ohne Satz-Einstellungen.
const GOLDEN_STAND: &str = "a6c2e714edd37269adb73b103f2f2acfdd1d034616692c4e1c8c5a667f978023";
const GOLDEN_PAGES: usize = 4;

#[tokio::test]
async fn a_work_without_settings_prepares_byte_identically() {
    let server = common::open().await;
    let work = server.import(&coast_book(&template("Küstenbuch")), "kuestenbuch.typ").await.work;
    let prepared = baruch_server::typeset::prepare(&server.state, work).await.unwrap();
    let typeset = baruch_server::typeset::typeset(&server.state, work).await.unwrap();
    println!("gemessen: Stand {} Seiten {}", prepared.stand, typeset.page_count());
    assert!(!prepared.source.main.contains(RULES_MARK), "ohne Einstellungen kein Regelblock in der Satzquelle");
    let map = server.state.store.export_map(work).await.unwrap();
    let labels = baruch_server::typeset::part_labels(&map, &prepared.work);
    assert!(
        prepared.source.main.contains(&baruch_typst::part_page_rule(&labels)),
        "die Teil-Regel von Runde 14 steht unverändert in der Quelle"
    );
    assert_eq!(prepared.stand, GOLDEN_STAND, "Stand der Satzquelle wie vor Runde 29 (S1)");
    assert_eq!(typeset.page_count(), GOLDEN_PAGES, "Seitenzahl wie vor Runde 29 (S1)");
}

#[tokio::test]
async fn the_typeset_page_offers_to_adopt_a_detected_chapter_rule() {
    let server = common::open().await;
    let with_rule = format!("{}\n{CANONICAL_CHAPTER_BREAK}", template("Küstenbuch mit Regel"));
    let work = server.import(&coast_book(&with_rule), "kuestenbuch-regel.typ").await.work;
    let (status, html) = server.get(&format!("/werke/{work}/satz")).await;
    assert_eq!(status, 200, "Satz-Seite: {html}");
    let section = html.split("id=\"druckpruefung\"").nth(1).expect("Abschnitt Druckprüfung");
    assert!(section.contains("typeset-template-rule"), "Hinweis zur erkannten Vorlagenregel: {section}");
    assert!(section.contains("data-chapter-start=\"right_page\""), "to: \"odd\" = neue rechte Seite: {section}");
    assert!(section.contains("Zeile 3"), "Zeile der Regel in der Werkvorlage (nach document und page): {section}");
    assert!(
        section.contains(&format!("href=\"/einstellungen/werk?work={work}#satz\"")),
        "Link zu Einstellungen → Werk → Satz: {section}"
    );
    assert!(section.contains("pagebreak(weak: true, to: &quot;odd&quot;)"), "Regeltext maskiert gezeigt: {section}");
    // Ohne gespeicherte Einstellungen gibt es keinen Konflikt (kein Feld gesetzt).
    assert!(!section.contains("typeset-conflict"), "ohne Einstellungen keine Konflikte: {section}");

    let plain = server.import(&coast_book(&template("Küstenbuch ohne Regel")), "kuestenbuch-ohne.typ").await.work;
    let (_, html) = server.get(&format!("/werke/{plain}/satz")).await;
    assert!(!html.contains("typeset-template-rule"), "ohne Regel in der Vorlage kein Hinweis");
}

#[tokio::test]
async fn the_cover_follows_the_first_book_page_and_counts_blank_pages_for_the_spine() {
    let server = common::open().await;
    // Die Vorlage setzt Kapitel auf rechte Seiten: Leerseiten entstehen und zählen für den Rücken mit.
    let with_rule = format!("{}\n{CANONICAL_CHAPTER_BREAK}", template("Küstenbuch Umschlag"));
    let work = server.import(&coast_book(&with_rule), "kuestenbuch-umschlag.typ").await.work;
    let set = server.rpc("work.meta.set", json!({ "work": work, "paper_mm": 0.1 })).await;
    result(&set);
    let typeset = baruch_server::typeset::typeset(&server.state, work).await.unwrap();
    let blanks = typeset.rendered().page_lines.iter().filter(|l| l.is_empty()).count();
    assert!(blanks > 0, "mit Kapiteln auf rechten Seiten gibt es Leerseiten ({} Seiten)", typeset.page_count());
    let wrap = baruch_server::cover::build_wrap(&server.state, work).await.unwrap();
    assert_eq!(wrap.pages, typeset.page_count(), "Rücken aus der Seitenzahl des Buchsatzes samt Leerseiten");
    assert_eq!(wrap.cover_pages, 0, "ohne Umschlagbild keine Umschlagseiten im Satz");
    assert!(
        (wrap.spine_mm - baruch_typst::cover::spine_mm(wrap.pages, 0.1)).abs() < 1e-9,
        "Rücken = (Seiten − Umschlagseiten) × Papierdicke: {} mm",
        wrap.spine_mm
    );
    assert!(
        (wrap.page_width_mm - 120.0).abs() < 0.2 && (wrap.page_height_mm - 190.0).abs() < 0.2,
        "Seitenmaß des Umschlags = erste Buchseite (120 × 190 mm): {} × {}",
        wrap.page_width_mm,
        wrap.page_height_mm
    );
}

#[test]
fn the_page_template_of_the_frame_carries_the_typeset_rules() {
    let a4 = "#set page(paper: \"a4\")";
    assert_eq!(
        baruch_server::cover::page_template(a4, &TypesetSettings::default()),
        a4,
        "ohne Einstellungen misst der Rahmen die Werkvorlage unverändert (gleicher Stand wie vor Runde 29)"
    );
    let a5 = TypesetSettings { page_format: Some(PageFormat::A5), ..Default::default() };
    let measured = baruch_server::cover::page_template(a4, &a5);
    let rendered =
        baruch_render::compile(&baruch_render::Source { main: format!("{measured}\n#hide[x]\n"), files: vec![] });
    let (w, h) = baruch_render::page_sizes_pt(&rendered)[0];
    let pt = |mm: f64| mm * 72.0 / 25.4;
    assert!(
        (w - pt(148.0)).abs() < 0.5 && (h - pt(210.0)).abs() < 0.5,
        "Seitenmaß des Rahmens = Satz-Format A5, obwohl die Vorlage A4 setzt: {w} × {h} pt"
    );
    assert_eq!(
        baruch_server::cover::book_font("#set text(font: \"New Computer Modern\")", &TypesetSettings::default())
            .as_deref(),
        Some("New Computer Modern"),
        "ohne Einstellung: Schrift der Werkvorlage"
    );
    let font = TypesetSettings { font: Some("DejaVu Sans Mono".into()), ..Default::default() };
    assert_eq!(
        baruch_server::cover::book_font("#set text(font: \"New Computer Modern\")", &font).as_deref(),
        Some("DejaVu Sans Mono"),
        "mit Einstellung: die Grundschrift des Buchs aus den Satz-Einstellungen"
    );
}
