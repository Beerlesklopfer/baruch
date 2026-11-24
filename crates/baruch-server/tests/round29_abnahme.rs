// SPDX-License-Identifier: AGPL-3.0-or-later
//! Abnahme Runde 29 „Buchsatz in den Einstellungen“ (Vertrag §8.4): erfundenes Werk mit Teilen, Kapiteln, Fußnoten,
//! Querverweis, Titelei und Umschlag – ohne Einstellungen unverändert, Formular speichern, PDF gemessen, Umschlag,
//! Vorlagenregel übernehmen, Konflikt, MCP, Austausch, Verweis-Dialog, Spiegel.
//!
//! Besitz: Integrator. Von der Schnittstelle R29-S1 leer angelegt.
//!
//! Hier steht, was erst nach dem Zusammenführen der drei Zweige zusammen geht: Die Einstellungen speichert das Formular
//! der Oberfläche (satz-oberflaeche) über `typeset_settings::save` in die Tabelle `work_typeset` (satz-daten), der Haken
//! in `snapshot::head` hängt den Regelblock aus `baruch_typst::typeset::rules` (satz-typst) an – und erst dann lässt
//! sich im Buch-PDF messen, was die Einstellungen bewirken. Gemessen wird mit `baruch_render::page_texts` auf genau der
//! Quelle, aus der das PDF entsteht (`typeset::prepare`, gleicher Stand), dazu `pdfinfo`/`pdftotext` am PDF selbst.
//! Das „Fährhausbuch“ ist frei erfunden: Titelei, zwei Teile, vier Kapitel mit Abschnitten, eine Fußnote je Kapitel und
//! ein Querverweis mit Seitenzahl. Kein Netz, kein Modell.

use crate::common;

use std::collections::HashMap;
use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_render::{PlacedText, Source};
use baruch_server::typeset_settings::ADOPT_NOTE;
use baruch_typst::typeset::{
    CANONICAL_CHAPTER_BREAK, ChapterStart, FootnoteNumbering, FootnoteRestart, Footnotes, Margins, PageFormat,
    PageNumberPosition, PageNumberStart, PageNumbers, Paragraph, RULES_MARK, RunningHead, TypesetSettings, rules,
};
use serde_json::{Value, json};

const PT_PER_MM: f64 = 72.0 / 25.4;

fn mm(x: f64) -> f64 {
    x * PT_PER_MM
}

/// Frei erfundene Sätze für die Absätze.
const SENTENCES: [&str; 6] = [
    "Die Fähre kam bei Nebel, und der Fährmann rief seinen Gruß über das graue Wasser zum Anleger hinüber.",
    "Im Fährhaus brannte schon Licht, die Wirtin stellte Becher auf den Tresen und legte Holz in den Ofen.",
    "Zwei Fischer saßen am Fenster und stritten leise darüber, ob der Wind am Abend nach Westen drehen würde.",
    "Draußen zogen Möwen über die Bucht, und die Boote schaukelten an ihren Leinen wie müde Pferde im Stall.",
    "Als die Glocke vom Kirchturm sechs schlug, öffnete sich die Tür, und ein Fremder trat mit nassem Mantel ein.",
    "Er bestellte Tee, setzte sich an den Ofen und erzählte von einer Insel, die auf keiner Karte verzeichnet war.",
];

/// Ein erfundenes langes Wort für die Silbentrennung.
const LONG_WORD: &str = "Fährhauswirtinnenversammlungsprotokollbuch";

fn paragraph(seed: usize) -> String {
    (0..5).map(|i| SENTENCES[(seed + i) % SENTENCES.len()]).collect::<Vec<_>>().join(" ")
}

/// Kapitel mit Überschrift (eigenes Label), `paragraphs` Absätzen, einer Fußnote im ersten Absatz und einem Abschnitt.
fn chapter(title: &str, label: &str, paragraphs: usize, extra: &str) -> String {
    let mut out = format!("= {title} <{label}>\n\n");
    for p in 0..paragraphs {
        out.push_str(&paragraph(p));
        if p == 0 {
            out.push_str(&format!("#footnote[Erfundene Anmerkung zum Kapitel {title}.]"));
        }
        out.push_str("\n\n");
        if p == paragraphs / 2 {
            out.push_str(&format!("== Unterwegs im Kapitel {title}\n\n"));
        }
    }
    out.push_str(extra);
    out
}

/// Werkvorlage mit Titel (Werktitel) und Autorin; `extra` hängt weitere Vorlagenzeilen an.
fn template(title: &str, extra: &str) -> String {
    format!("#set document(title: \"{title}\", author: \"Mara Holm\")\n{extra}")
}

/// Das Fährhausbuch: zwei Teile, vier Kapitel; im dritten Kapitel ein Querverweis mit Seitenzahl auf „Der Hafen“ und
/// ein langes Wort.
fn book(template: &str) -> String {
    let xref = format!(
        "Wie im Kapitel #link(<hafen>)[Der Hafen, S.~#context counter(page).at(<hafen>).first()] erzählt, kam der \
         Fremde wieder. Im {LONG_WORD} und im {LONG_WORD} stand sein Name, ebenso im {LONG_WORD} des Vorjahres.\n\n"
    );
    let mut out = format!("{template}\n\n");
    out.push_str(&baruch_typst::part_line("Am Wasser"));
    out.push('\n');
    out.push_str(&chapter("Einleitung", "einl", 10, ""));
    out.push_str(&chapter("Der Hafen", "hafen", 7, ""));
    out.push_str(&baruch_typst::part_line("Im Land"));
    out.push('\n');
    out.push_str(&chapter("Die Mühle", "muehle", 5, &xref));
    out.push_str(&chapter("Heimkehr", "heim", 5, ""));
    out
}

const CHAPTERS: [&str; 4] = ["Einleitung", "Der Hafen", "Die Mühle", "Heimkehr"];

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC-Fehler: {answer}");
    &answer["result"]
}

struct Ferry {
    server: common::Server,
    work: i64,
    slug: String,
    keys: HashMap<String, i64>,
}

/// Neuer Datenordner mit dem Fährhausbuch (Titel je Aufruf eigen, damit Stände im prozessweiten Zwischenspeicher nicht
/// zwischen Tests geteilt werden) und Titelei aus den Buchangaben.
async fn ferry(title: &str, extra_template: &str) -> Ferry {
    let server = common::open().await;
    let report = server.import(&book(&template(title, extra_template)), "faehrhaus.typ").await;
    let work = report.work;
    result(
        &server
            .rpc("work.meta.set", json!({ "work": work, "front_matter": true, "subtitle": "Geschichten vom Anleger" }))
            .await,
    );
    let keys =
        server.state.store.load_work(work).await.unwrap().nodes.iter().map(|n| (n.title.clone(), n.key)).collect();
    Ferry { server, work, slug: report.slug, keys }
}

impl Ferry {
    fn key(&self, title: &str) -> i64 {
        *self.keys.get(title).unwrap_or_else(|| panic!("kein Punkt {title}"))
    }

    fn commits(&self) -> usize {
        common::git(&self.server.mirror_dir_of(&self.slug), &["rev-list", "--count", "HEAD"])
            .map_or(0, |n| n.trim().parse().unwrap())
    }

    fn last_commit(&self) -> String {
        common::git(&self.server.mirror_dir_of(&self.slug), &["log", "-1", "--format=%an|%cn%n%B"]).unwrap_or_default()
    }

    async fn typeset_get(&self) -> Value {
        result(&self.server.rpc("work.typeset_get", json!({ "work": self.work })).await).clone()
    }

    async fn set(&self, settings: &TypesetSettings) {
        let value = serde_json::to_value(settings).unwrap();
        result(&self.server.rpc("work.typeset_set", json!({ "work": self.work, "settings": value })).await);
    }

    async fn post(&self, uri: &str, form: &str) -> (StatusCode, Option<String>, String) {
        let request = Request::post(uri)
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(Body::from(form.to_owned()))
            .unwrap();
        let (status, headers, body) = self.server.send(request).await;
        (status, headers.get(header::LOCATION).map(|v| v.to_str().unwrap().to_owned()), body)
    }

    /// Das Buch gesetzt wie für Satz-Ansicht und PDF: Seitenzahl, Kapitelseiten, Seitenmaße und Textstücke je Seite.
    async fn measure(&self) -> Measured {
        let typeset = baruch_server::typeset::typeset(&self.server.state, self.work).await.unwrap();
        assert!(!typeset.rendered().has_errors(), "Satz ohne Fehler: {:?}", typeset.rendered().diagnostics);
        let chapters = CHAPTERS.iter().map(|t| typeset.page_of(Some(self.key(t)))).collect();
        let sizes = baruch_render::page_sizes_pt(typeset.rendered());
        let source = typeset.prepared.source.clone();
        let texts = page_texts(&source).await;
        assert_eq!(texts.len(), typeset.page_count(), "page_texts zählt dieselben Seiten wie die Satz-Ansicht");
        Measured {
            pages: typeset.page_count(),
            stand: typeset.prepared.stand.clone(),
            main: source.main.clone(),
            chapters,
            sizes,
            texts,
        }
    }
}

async fn page_texts(source: &Source) -> Vec<Vec<PlacedText>> {
    let source = source.clone();
    tokio::task::spawn_blocking(move || baruch_render::page_texts(&source))
        .await
        .unwrap()
        .expect("ohne Satzfehler liefert page_texts die Seiten")
}

struct Measured {
    pages: usize,
    stand: String,
    main: String,
    /// Seite (1-basiert) je Kapitel in der Reihenfolge von [`CHAPTERS`].
    chapters: Vec<usize>,
    sizes: Vec<(f64, f64)>,
    texts: Vec<Vec<PlacedText>>,
}

impl Measured {
    fn page(&self, n: usize) -> &[PlacedText] {
        &self.texts[n - 1]
    }

    /// Textstücke einer Seite oberhalb von `y` pt (Kopfzeile).
    fn above(&self, n: usize, y: f64) -> Vec<&PlacedText> {
        self.page(n).iter().filter(|t| t.y_pt < y).collect()
    }

    /// Textstücke einer Seite unterhalb von `y` pt (Fußzeile).
    fn below(&self, n: usize, y: f64) -> Vec<&PlacedText> {
        self.page(n).iter().filter(|t| t.y_pt > y).collect()
    }

    /// Kleinstes x der Textstücke zwischen `top` und `bottom` pt (linke Kante des Satzspiegels).
    fn left_edge(&self, n: usize, top: f64, bottom: f64) -> f64 {
        self.page(n).iter().filter(|t| t.y_pt > top && t.y_pt < bottom).map(|t| t.x_pt).fold(f64::INFINITY, f64::min)
    }

    /// Häufigste Schriftgröße der Seiten (Grundschrift) und ihr Familienname.
    fn body_font(&self) -> (f64, String) {
        let mut counts: HashMap<(i64, String), usize> = HashMap::new();
        for t in self.texts.iter().flatten() {
            *counts.entry(((t.size_pt * 10.0).round() as i64, t.font.clone())).or_default() += t.text.chars().count();
        }
        let ((size, font), _) = counts.into_iter().max_by_key(|(_, n)| *n).unwrap();
        (size as f64 / 10.0, font)
    }
}

/// Zeilen einer Seite: Textstücke mit gleicher Grundlinie (auf 0,1 pt), von oben nach unten, je Zeile von links.
fn lines(page: &[PlacedText]) -> Vec<Vec<&PlacedText>> {
    let mut ys: Vec<i64> = page.iter().map(|t| (t.y_pt * 10.0).round() as i64).collect();
    ys.sort_unstable();
    ys.dedup();
    ys.into_iter()
        .map(|y| {
            let mut line: Vec<&PlacedText> = page.iter().filter(|t| (t.y_pt * 10.0).round() as i64 == y).collect();
            line.sort_by(|a, b| a.x_pt.total_cmp(&b.x_pt));
            line
        })
        .collect()
}

fn line_text(line: &[&PlacedText]) -> String {
    line.iter().map(|t| t.text.as_str()).collect::<Vec<_>>().join(" ")
}

fn digits(texts: &[&PlacedText]) -> Vec<String> {
    texts
        .iter()
        .map(|t| t.text.trim().to_owned())
        .filter(|t| !t.is_empty() && t.chars().all(|c| c.is_ascii_digit()))
        .collect()
}

/// Seitenmaße aller Seiten des PDFs in pt (pdfinfo); `None`, wenn pdfinfo fehlt.
fn pdf_page_sizes(pdf: &[u8], pages: usize) -> Option<Vec<(f64, f64)>> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("buch.pdf");
    std::fs::write(&path, pdf).unwrap();
    let out =
        std::process::Command::new("pdfinfo").args(["-f", "1", "-l", &pages.to_string()]).arg(&path).output().ok()?;
    assert!(out.status.success(), "pdfinfo: {}", String::from_utf8_lossy(&out.stderr));
    let text = String::from_utf8_lossy(&out.stdout).to_string();
    Some(
        text.lines()
            .filter(|l| l.starts_with("Page") && l.contains("size:"))
            .map(|l| {
                let size = l.split("size:").nth(1).unwrap().trim();
                let mut it = size.split_whitespace();
                let w: f64 = it.next().unwrap().parse().unwrap();
                it.next(); // „x“
                let h: f64 = it.next().unwrap().parse().unwrap();
                (w, h)
            })
            .collect(),
    )
}

/// Text des PDFs (pdftotext, geschütztes Leerzeichen als Leerzeichen); `None`, wenn pdftotext fehlt.
fn pdf_text(pdf: &[u8]) -> Option<String> {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("buch.pdf");
    std::fs::write(&path, pdf).unwrap();
    let out = std::process::Command::new("pdftotext").arg(&path).arg("-").output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).replace('\u{a0}', " "))
}

async fn book_pdf(f: &Ferry) -> baruch_server::pdf::PdfOut {
    let _serial = common::PDF_SERIAL.lock().await;
    baruch_server::pdf::book_pdf(&f.server.state, f.work).await.expect("Buch-PDF entsteht")
}

/// Die Formularfelder der Abnahme (Vertrag §8.4 (2)): A5, Ränder mit Bundsteg, neue rechte Seite, Seitenzahlen unten
/// außen ab dem ersten Kapitel, Kolumnentitel Kapitel, Fußnoten je Kapitel, Verweis mit Seitenzahl, Dezimalkomma.
fn abnahme_form(work: i64) -> String {
    format!(
        "work={work}&action=save&chapter_start=right_page&part_side=&page_format=a5&page_width_mm=&page_height_mm=\
         &margin_inside_mm=20&margin_outside_mm=15&margin_top_mm=18&margin_bottom_mm=22&binding_mm=5&font=\
         &font_size_pt=10%2C5&leading_em=&paragraph=indent&paragraph_em=1&justify=on&hyphenate=on&widows_orphans=\
         &xref_with_page=on&lang=de&page_number_position=bottom_outside&page_number_start=first_chapter\
         &running_head=chapter&footnote_numbering=arabic&footnote_restart=per_chapter"
    )
}

fn abnahme_settings() -> Value {
    json!({"chapter_start":"right_page","page_format":{"kind":"a5"},
           "margins":{"inside_mm":20.0,"outside_mm":15.0,"top_mm":18.0,"bottom_mm":22.0,"binding_mm":5.0},
           "font_size_pt":10.5,"paragraph":{"kind":"indent","em":1.0},"justify":true,"hyphenate":true,"lang":"de",
           "page_numbers":{"position":"bottom_outside","start":"first_chapter"},"running_head":"chapter",
           "footnotes":{"numbering":"arabic","restart":"per_chapter"},"xref_with_page":true})
}

// ---- (1) ohne Einstellungen ---------------------------------------------------------------------

#[tokio::test]
async fn without_settings_export_source_and_pages_stay_as_before_and_reset_restores_them() {
    let f = ferry("Das Fährhausbuch eins", "").await;
    let export = f.server.state.store.export_typst(f.work).await.unwrap();
    let before = f.measure().await;
    assert!(!export.contains(RULES_MARK), "ohne Einstellungen kein Regelblock im Export");
    assert!(!before.main.contains(RULES_MARK), "und keiner in der Satzquelle");
    let main = std::fs::read_to_string(f.server.mirror_dir_of(&f.slug).join("main.typ")).unwrap();
    assert!(!main.contains(RULES_MARK), "und keiner in main.typ des Spiegels");

    // Setzen und Zurücksetzen über das Formular (ohne JavaScript) stellt Export, Quelle und Seiten byte-gleich wieder
    // her – der Stand ohne Einstellungen ist derselbe wie vor Runde 29 (Goldstände dazu: Store-Test `typeset.rs`,
    // `round29_typst.rs`).
    let (status, location, body) = f.post("/einstellungen/werk/satz", &abnahme_form(f.work)).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Speichern leitet weiter: {body}");
    assert_eq!(location.as_deref(), Some(format!("/einstellungen/werk?work={}&ok=satz#satz", f.work).as_str()));
    let with = f.measure().await;
    assert_ne!(with.stand, before.stand, "mit Einstellungen ein anderer Stand");
    let (status, location, body) =
        f.post("/einstellungen/werk/satz", &format!("work={}&action=reset&confirm=1", f.work)).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Zurücksetzen leitet weiter: {body}");
    assert_eq!(location.as_deref(), Some(format!("/einstellungen/werk?work={}&ok=satz_zurueck#satz", f.work).as_str()));
    assert_eq!(f.server.state.store.export_typst(f.work).await.unwrap(), export, "Export byte-gleich wie vorher");
    let after = f.measure().await;
    assert_eq!(after.stand, before.stand, "Satzquelle byte-gleich wie vorher (gleicher Stand)");
    assert_eq!(after.pages, before.pages, "gleiche Seitenzahl");
    assert_eq!(after.chapters, before.chapters, "Kapitel auf denselben Seiten");
}

// ---- (2) Formular → Einstellungen → Export → PDF gemessen, (3) Umschlag, (8) Verweis, (9) Spiegel ------------

#[tokio::test]
async fn the_saved_form_sets_the_book_pdf_in_a5_with_right_pages_numbers_and_running_heads() {
    let f = ferry("Das Fährhausbuch zwei", "").await;
    result(&f.server.rpc("work.meta.set", json!({ "work": f.work, "front_matter": true, "paper_mm": 0.1 })).await);
    let before = f.measure().await;
    let wrap_before = baruch_server::cover::build_wrap(&f.server.state, f.work).await.unwrap();

    // Speichern über das Formular ohne JavaScript: 303, ein Commit vom Menschen mit Trailer.
    let commits = f.commits();
    let (status, location, body) = f.post("/einstellungen/werk/satz", &abnahme_form(f.work)).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Speichern: {body}");
    assert_eq!(location.as_deref(), Some(format!("/einstellungen/werk?work={}&ok=satz#satz", f.work).as_str()));
    assert_eq!(f.commits(), commits + 1, "genau ein Spiegel-Commit");
    let message = f.last_commit();
    assert!(message.starts_with("Autor|Autor\n"), "Autor und Committer der Mensch: {message}");
    assert!(message.contains("Baruch-Action: typeset-set"), "Trailer: {message}");
    assert!(message.contains("Kapitelanfang: aus der Werkvorlage → neue rechte Seite"), "Rumpf: {message}");

    // Vorschau ohne JavaScript: mit den gespeicherten Werten kein Hinweis „Ungespeicherte Werte“, mit einem
    // geänderten Wert schon (Integrator R29); beide Male Vorschaubilder.
    let (status, _, page) =
        f.post("/einstellungen/werk/satz", &abnahme_form(f.work).replace("action=save", "action=preview")).await;
    assert_eq!(status, StatusCode::OK, "Vorschau: {page}");
    assert!(page.contains("/einstellungen/werk/satz/vorschau/"), "Vorschaubilder");
    assert!(!page.contains("Ungespeicherte Werte"), "gleiche Werte wie gespeichert: kein Hinweis");
    let changed =
        abnahme_form(f.work).replace("action=save", "action=preview").replace("font_size_pt=10%2C5", "font_size_pt=11");
    let (status, _, page) = f.post("/einstellungen/werk/satz", &changed).await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains("Ungespeicherte Werte"), "geänderter Wert: Hinweis");

    // work.typeset_get liest, was das Formular gespeichert hat (Dezimalkomma 10,5 → 10.5).
    let view = f.typeset_get().await;
    assert_eq!(view["settings"], abnahme_settings(), "Formular → gespeicherte Einstellungen: {view}");
    let settings: TypesetSettings = serde_json::from_value(abnahme_settings()).unwrap();
    let block = rules(&settings);
    assert!(block.starts_with(RULES_MARK), "Regelblock beginnt mit „// Baruch: Satz“");
    assert_eq!(view["rules"], block, "derselbe Block in work.typeset_get");

    // (8) Verweis-Dialog: Das Häkchen startet mit der Werk-Einstellung (xref.ts liest work.typeset_get).
    assert_eq!(view["settings"]["xref_with_page"], true, "Vorgabe „mit Seitenzahl“ als Werk-Einstellung");
    let xref_js = baruch_server::SCRIPTS.iter().find(|(name, _)| *name == "xref.js").expect("xref.js ausgeliefert").1;
    assert!(
        xref_js.contains("work.typeset_get") && xref_js.contains("xref_with_page"),
        "der Dialog liest die Werk-Einstellung (übersetztes xref.ts)"
    );

    // Export, Export-Karte und main.typ tragen den Block hinter der Werkvorlage (snapshot::head).
    let export = f.server.state.store.export_typst(f.work).await.unwrap();
    assert!(export.contains(&block), "Export mit Regelblock");
    assert!(
        export.find("#set document(").unwrap() < export.find(RULES_MARK).unwrap(),
        "der Block steht hinter der Werkvorlage"
    );
    let main = std::fs::read_to_string(f.server.mirror_dir_of(&f.slug).join("main.typ")).unwrap();
    assert!(main.contains(&block), "main.typ des Spiegels mit Regelblock");

    // Satz und PDF aus derselben Quelle: gleicher Stand, gleiche Seitenzahl (Satz-Vorschau = PDF).
    let m = f.measure().await;
    let pdf = book_pdf(&f).await;
    assert_eq!(pdf.stand, m.stand, "PDF aus derselben Quelle wie die Satz-Ansicht");
    assert_eq!(pdf.pages, m.pages, "PDF und Satz-Ansicht haben gleich viele Seiten");
    assert!(m.main.contains(&block), "die Satzquelle trägt den Block");
    assert_ne!(m.pages, before.pages, "die Einstellungen ändern die Seitenzahl ({} → {})", before.pages, m.pages);

    // A5 auf jeder Seite (±0,5 pt), im Satz und im PDF selbst.
    let (w, h) = (mm(148.0), mm(210.0));
    for (i, (pw, ph)) in m.sizes.iter().enumerate() {
        assert!((pw - w).abs() < 0.5 && (ph - h).abs() < 0.5, "Seite {}: A5 erwartet, {pw} × {ph} pt", i + 1);
    }
    match pdf_page_sizes(&pdf.bytes, pdf.pages) {
        Some(sizes) => {
            assert_eq!(sizes.len(), pdf.pages, "pdfinfo nennt jede Seite");
            for (i, (pw, ph)) in sizes.iter().enumerate() {
                assert!((pw - w).abs() < 0.5 && (ph - h).abs() < 0.5, "PDF-Seite {}: {pw} × {ph} pt", i + 1);
            }
        }
        None => eprintln!("ÜBERSPRUNGEN: pdfinfo fehlt (Seitenmaß nur im Satz gemessen)"),
    }

    // Neue rechte Seite: jedes Kapitel beginnt auf einer ungeraden Seite.
    for (title, page) in CHAPTERS.iter().zip(&m.chapters) {
        assert_eq!(page % 2, 1, "Kapitel „{title}“ auf einer rechten (ungeraden) Seite: {page}");
    }
    let first = m.chapters[0];
    assert!(first > 3, "vor dem ersten Kapitel stehen Titelei und Teil-Seite: {first}");

    let top = mm(18.0);
    let bottom = h - mm(22.0);
    // Seitenzahlen unten außen ab dem ersten Kapitel: davor keine Zahl, auf der ersten Kapitelseite „1“ rechts,
    // auf der folgenden (geraden) Seite „2“ links.
    for page in 1..first {
        let footer = digits(&m.below(page, bottom));
        assert!(footer.is_empty(), "vor dem ersten Kapitel keine Seitenzahl (Seite {page}): {footer:?}");
    }
    let number = |page: usize| -> (String, f64) {
        let footer = m.below(page, bottom);
        let n: Vec<&&PlacedText> =
            footer.iter().filter(|t| t.text.trim().chars().all(|c| c.is_ascii_digit())).collect();
        assert_eq!(n.len(), 1, "genau eine Seitenzahl unten auf Seite {page}: {footer:?}");
        (n[0].text.trim().to_owned(), n[0].x_pt + n[0].width_pt / 2.0)
    };
    // Jede Seite mit Text ab dem ersten Kapitel trägt ihre Zahl außen; Teil- und Leerseiten nicht. Der Kolumnentitel
    // „Kapitel“ steht auf jeder solchen Seite außer den Kapitelanfängen und nennt das laufende Kapitel.
    let mut numbered = 0;
    for page in first..=m.pages {
        if m.page(page).len() <= 3 {
            assert!(digits(&m.below(page, bottom)).is_empty(), "Teil- oder Leerseite {page} ohne Zahl");
            assert!(m.above(page, top).is_empty(), "Teil- oder Leerseite {page} ohne Kolumnentitel");
            continue;
        }
        let (n, x) = number(page);
        assert_eq!(n, (page - first + 1).to_string(), "Seite {page}: Zählung ab dem ersten Kapitel mit 1");
        if page % 2 == 1 {
            assert!(x > w / 2.0 + mm(30.0), "Seite {page} (rechts): Zahl außen = rechts, x = {x}");
        } else {
            assert!(x < w / 2.0 - mm(30.0), "Seite {page} (links): Zahl außen = links, x = {x}");
        }
        let head: String = m.above(page, top).iter().map(|t| t.text.as_str()).collect::<Vec<_>>().join(" ");
        match m.chapters.iter().position(|&c| c == page) {
            Some(_) => assert!(head.is_empty(), "kein Kolumnentitel auf der Kapitelanfangsseite {page}: {head:?}"),
            None => {
                let current = m.chapters.iter().rposition(|&c| c < page).unwrap();
                assert_eq!(head, CHAPTERS[current], "Kolumnentitel auf Seite {page} = laufendes Kapitel");
            }
        }
        numbered += 1;
    }
    assert!(numbered > 6, "viele Seiten geprüft ({numbered})");
    assert_eq!(number(first).0, "1", "die erste Kapitelseite trägt die 1");

    // Ränder mit Bundsteg, gespiegelt: rechte Seite innen (links) 20 + 5 mm, linke Seite außen (links) 15 mm.
    let odd = m.left_edge(first, top, bottom);
    let even = m.left_edge(first + 1, top, bottom);
    assert!((odd - mm(25.0)).abs() < 0.5, "ungerade Seite: Innenrand + Bundsteg = 25 mm, gemessen {odd} pt");
    assert!((even - mm(15.0)).abs() < 0.5, "gerade Seite: Außenrand 15 mm links, gemessen {even} pt");

    // Querverweis „S. n“ = die auf der Zielseite gedruckte Zahl (Zählung ab dem ersten Kapitel, Titelei ohne Zahl).
    let target = m.chapters[1];
    let printed = (target - first + 1).to_string();
    let xref_line = m
        .texts
        .iter()
        .flat_map(|p| lines(p))
        .map(|l| line_text(&l))
        .find(|l| l.contains("Hafen, S."))
        .expect("Zeile mit dem Querverweis");
    let shown: String = xref_line
        .split("Hafen, S.")
        .nth(1)
        .unwrap()
        .trim_start_matches(|c: char| c.is_whitespace())
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    assert_eq!(shown, printed, "„S. n“ = angezeigte Zahl der Seite mit „Der Hafen“: {xref_line}");
    if target > first && !digits(&m.below(target, bottom)).is_empty() {
        assert_eq!(digits(&m.below(target, bottom)), vec![printed.clone()], "die Zielseite druckt dieselbe Zahl");
    }
    match pdf_text(&pdf.bytes) {
        Some(text) => assert!(text.contains(&format!("Der Hafen, S. {printed}")), "„S. {printed}“ im PDF-Text"),
        None => eprintln!("ÜBERSPRUNGEN: pdftotext fehlt"),
    }

    // (3) Umschlag: Seitenmaß = Satz-Format, Rücken aus der neuen Seitenzahl.
    let wrap = baruch_server::cover::build_wrap(&f.server.state, f.work).await.unwrap();
    assert!(
        (wrap.page_width_mm - 148.0).abs() < 0.2 && (wrap.page_height_mm - 210.0).abs() < 0.2,
        "Seitenmaß des Umschlags folgt dem Satz-Format A5: {} × {}",
        wrap.page_width_mm,
        wrap.page_height_mm
    );
    assert_eq!(wrap.pages, m.pages, "Rücken aus der neuen Seitenzahl (Leerseiten zählen mit)");
    assert_ne!(wrap.pages, wrap_before.pages, "vorher andere Seitenzahl");
    assert!(
        (wrap.spine_mm - baruch_typst::cover::spine_mm(wrap.pages - wrap.cover_pages, 0.1)).abs() < 1e-9,
        "Rücken = (Seiten − Umschlagseiten) × Papierdicke: {} mm",
        wrap.spine_mm
    );
    assert!(wrap.spine_mm > wrap_before.spine_mm, "mehr Seiten, breiterer Rücken");
}

// ---- jede Einstellung einzeln über den Server ----------------------------------------------------

#[tokio::test]
async fn each_setting_alone_shows_in_the_typeset_book() {
    let f = ferry("Das Fährhausbuch drei", "").await;
    let plain = f.measure().await;
    let (plain_size, plain_font) = plain.body_font();

    // Seitenformat 12 × 19 und eigenes Format.
    for (format, (fw, fh)) in [
        (PageFormat::Mm120x190, (120.0, 190.0)),
        (PageFormat::Custom { width_mm: 100.0, height_mm: 160.0 }, (100.0, 160.0)),
    ] {
        f.set(&TypesetSettings { page_format: Some(format), ..Default::default() }).await;
        let m = f.measure().await;
        for (pw, ph) in &m.sizes {
            assert!((pw - mm(fw)).abs() < 0.5 && (ph - mm(fh)).abs() < 0.5, "{format:?}: {pw} × {ph} pt");
        }
    }

    // Kapitelanfang „neue Seite“: Über jeder Kapitelüberschrift steht auf ihrer Seite kein Text.
    f.set(&TypesetSettings { chapter_start: Some(ChapterStart::NewPage), ..Default::default() }).await;
    let m = f.measure().await;
    for (title, page) in CHAPTERS.iter().zip(&m.chapters) {
        let first_line = lines(m.page(*page)).into_iter().map(|l| line_text(&l)).next().unwrap_or_default();
        assert!(first_line.contains(title), "„{title}“ ist die erste Zeile seiner Seite {page}: {first_line:?}");
    }

    // Grundschrift und Größe.
    let families = tokio::task::spawn_blocking(baruch_render::font_families).await.unwrap();
    let other = families
        .iter()
        .find(|fam| fam.embedded && !fam.name.eq_ignore_ascii_case(&plain_font) && !fam.name.contains("Math"))
        .expect("eine zweite eingebettete Schrift")
        .name
        .clone();
    f.set(&TypesetSettings { font: Some(other.clone()), font_size_pt: Some(12.0), ..Default::default() }).await;
    let (size, font) = f.measure().await.body_font();
    assert!((size - 12.0).abs() < 0.05, "Grundschrift 12 pt (vorher {plain_size}): {size}");
    assert!(font.eq_ignore_ascii_case(&other), "Grundschrift {other} (vorher {plain_font}): {font}");

    // Einzug vs. Abstand: mit Einzug beginnen Zeilen 2 em rechts der Kante, mit Abstand keine.
    let indented = |m: &Measured| -> usize {
        let (size, _) = m.body_font();
        m.texts
            .iter()
            .map(|page| {
                let body: Vec<&PlacedText> = page.iter().filter(|t| (t.size_pt - size).abs() < 0.05).collect();
                let edge = body.iter().map(|t| t.x_pt).fold(f64::INFINITY, f64::min);
                lines(page)
                    .iter()
                    .filter(|l| (l[0].size_pt - size).abs() < 0.05 && (l[0].x_pt - edge - 2.0 * size).abs() < 0.5)
                    .count()
            })
            .sum()
    };
    f.set(&TypesetSettings { paragraph: Some(Paragraph::Indent { em: 2.0 }), ..Default::default() }).await;
    let with_indent = indented(&f.measure().await);
    f.set(&TypesetSettings { paragraph: Some(Paragraph::Spacing { em: 1.0 }), ..Default::default() }).await;
    let with_spacing = indented(&f.measure().await);
    assert!(with_indent > 10, "Einzug 2 em: viele Absatzanfänge eingerückt ({with_indent})");
    assert_eq!(with_spacing, 0, "Abstand statt Einzug: keine eingerückte Zeile");

    // Zeilenabstand: 1,0 em statt 0,5 em rückt Grundlinien um 0,5 em weiter auseinander.
    let pitch = |m: &Measured| -> f64 {
        let (size, _) = m.body_font();
        let page = m.page(m.chapters[0]);
        let ys: Vec<f64> = lines(page)
            .iter()
            .filter(|l| l.iter().all(|t| (t.size_pt - size).abs() < 0.05))
            .map(|l| l[0].y_pt)
            .collect();
        let mut gaps: Vec<f64> = ys.windows(2).map(|w| w[1] - w[0]).collect();
        gaps.sort_by(f64::total_cmp);
        gaps[0]
    };
    f.set(&TypesetSettings { leading_em: Some(0.5), ..Default::default() }).await;
    let narrow = pitch(&f.measure().await);
    f.set(&TypesetSettings { leading_em: Some(1.0), ..Default::default() }).await;
    let wide = pitch(&f.measure().await);
    assert!((wide - narrow - 0.5 * plain_size).abs() < 0.3, "Zeilenabstand +0,5 em: {narrow} → {wide} pt");

    // Silbentrennung (de) bei schmaler Spalte: Trennstrich am Zeilenende, ohne Trennung keiner.
    let narrow_page = |hyphenate: bool| TypesetSettings {
        hyphenate: Some(hyphenate),
        lang: Some("de".into()),
        justify: Some(true),
        page_format: Some(PageFormat::Custom { width_mm: 80.0, height_mm: 160.0 }),
        margins: Some(Margins { inside_mm: 10.0, outside_mm: 10.0, top_mm: 10.0, bottom_mm: 10.0, binding_mm: 0.0 }),
        ..Default::default()
    };
    let hyphens = |m: &Measured| -> usize {
        m.texts
            .iter()
            .flat_map(|p| lines(p))
            .filter(|l| line_text(l).trim_end().ends_with(['-', '\u{ad}', '\u{2010}']))
            .count()
    };
    f.set(&narrow_page(true)).await;
    let on = hyphens(&f.measure().await);
    f.set(&narrow_page(false)).await;
    let off = hyphens(&f.measure().await);
    assert!(on > 0, "Silbentrennung: mindestens ein Trennstrich am Zeilenende ({on})");
    assert_eq!(off, 0, "ohne Silbentrennung keiner");

    // Seitenzahlen verborgen: keine Zahl in Kopf- oder Fußzeile.
    f.set(&TypesetSettings {
        page_numbers: Some(PageNumbers { position: PageNumberPosition::Hidden, start: PageNumberStart::FirstPage }),
        ..Default::default()
    })
    .await;
    let m = f.measure().await;
    let (_, ph) = m.sizes[0];
    for page in 1..=m.pages {
        let edges: Vec<&PlacedText> =
            m.page(page).iter().filter(|t| t.y_pt < mm(20.0) || t.y_pt > ph - mm(20.0)).collect();
        assert!(digits(&edges).is_empty(), "verborgen: keine Seitenzahl auf Seite {page}: {edges:?}");
    }

    // Kolumnentitel „Werktitel“ auf einer Folgeseite des ersten Kapitels.
    f.set(&TypesetSettings { running_head: Some(RunningHead::WorkTitle), ..Default::default() }).await;
    let m = f.measure().await;
    let mut shown = 0;
    for page in m.chapters[0] + 1..=m.pages {
        if m.chapters.contains(&page) || m.page(page).len() <= 3 {
            continue;
        }
        let head: String = m.above(page, mm(22.0)).iter().map(|t| t.text.as_str()).collect::<Vec<_>>().join(" ");
        assert_eq!(head, "Das Fährhausbuch drei", "Kolumnentitel = Werktitel auf Seite {page} (ohne Kapitelregel)");
        shown += 1;
    }
    assert!(shown >= 2, "auf mehreren Folgeseiten (A4, elf Seiten: {shown})");
    assert!(f.typeset_get().await["warnings"].as_array().unwrap().is_empty(), "die Vorlage setzt einen Titel");

    // Fußnoten mit Buchstaben: fortlaufend a–d, je Kapitel neu immer „a“.
    // Ein Fußnotenzeichen (hochgestellt, eigene Schriftmerkmale) ist ein eigenes Textstück aus einem Zeichen – im Text
    // und am Fuß (wie in `baruch-render/tests/typeset_measure.rs`).
    let marks = |m: &Measured| -> Vec<String> {
        let mut found: Vec<String> = m
            .texts
            .iter()
            .flatten()
            .filter(|t| ["a", "b", "c", "d"].contains(&t.text.as_str()))
            .map(|t| t.text.trim().to_owned())
            .collect();
        found.sort();
        found.dedup();
        found
    };
    let notes = |restart| TypesetSettings {
        footnotes: Some(Footnotes { numbering: FootnoteNumbering::Letters, restart }),
        ..Default::default()
    };
    f.set(&notes(FootnoteRestart::Continuous)).await;
    let continuous = marks(&f.measure().await);
    f.set(&notes(FootnoteRestart::PerChapter)).await;
    let per_chapter = marks(&f.measure().await);
    assert_eq!(continuous, ["a", "b", "c", "d"], "vier Fußnoten fortlaufend a bis d");
    assert_eq!(per_chapter, ["a"], "je Kapitel neu: jede Fußnote „a“");
}

#[tokio::test]
async fn a_work_title_head_without_a_title_in_the_template_is_named_in_the_warnings() {
    let server = common::open().await;
    let work = server.import("= Ohne Titel\n\nEin erfundener Satz.\n", "ohne-titel.typ").await.work;
    let settings = json!({ "running_head": "work_title" });
    result(&server.rpc("work.typeset_set", json!({ "work": work, "settings": settings })).await);
    let view = result(&server.rpc("work.typeset_get", json!({ "work": work })).await).clone();
    assert_eq!(
        view["warnings"],
        json!([baruch_server::typeset_settings::NO_DOCUMENT_TITLE]),
        "Kolumnentitel „Werktitel“ ohne #set document(title: …) bliebe leer: {view}"
    );
}

// ---- (4) Vorlagenregel übernehmen ------------------------------------------------------------------

#[tokio::test]
async fn the_template_rule_is_offered_adopted_with_one_commit_and_chapters_stay_on_their_pages() {
    let f = ferry("Das Fährhausbuch vier", &format!("{CANONICAL_CHAPTER_BREAK}\n")).await;
    let before = f.measure().await;
    for (title, page) in CHAPTERS.iter().zip(&before.chapters) {
        assert_eq!(page % 2, 1, "die Vorlagenregel setzt „{title}“ auf eine rechte Seite");
    }

    // Angebot im Block „Satz“ (ohne JavaScript) und auf der Satz-Seite.
    let (status, page) = f.server.get(&format!("/einstellungen/werk?work={}", f.work)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains("data-typeset-adopt"), "Angebot zur Vorlagenregel im Block „Satz“");
    assert!(page.contains("pagebreak(weak: true, to: &quot;odd&quot;)"), "mit dem Text der Zeile");
    let (_, satz) = f.server.get(&format!("/werke/{}/satz", f.work)).await;
    assert!(satz.contains("typeset-template-rule"), "Hinweis auf der Satz-Seite");

    // Ohne Häkchen: 422, nichts geändert.
    let commits = f.commits();
    let revisions = f.server.state.store.template_revisions(f.work).await.unwrap().len();
    let (status, _, body) = f.post("/einstellungen/werk/satz/vorlage", &format!("work={}", f.work)).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "ohne Bestätigung abgelehnt: {body}");
    assert_eq!(f.commits(), commits, "kein Commit ohne Bestätigung");

    // Bestätigt: 303, neue Vorlagenfassung ohne die Zeile, chapter_start right_page, ein Commit.
    let (status, location, body) =
        f.post("/einstellungen/werk/satz/vorlage", &format!("work={}&confirm=1", f.work)).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Übernahme: {body}");
    assert_eq!(location.as_deref(), Some(format!("/einstellungen/werk?work={}&ok=satz_vorlage#satz", f.work).as_str()));
    assert_eq!(f.commits(), commits + 1, "Einstellungen und Vorlage in genau einem Commit");
    let message = f.last_commit();
    assert!(message.contains("Satz: Kapitelanfang aus der Werkvorlage übernommen"), "Betreff: {message}");
    assert!(message.contains("Baruch-Action: typeset-adopt"), "Trailer: {message}");
    let all = f.server.state.store.template_revisions(f.work).await.unwrap();
    assert_eq!(all.len(), revisions + 1, "eine neue Vorlagenfassung");
    assert_eq!(all.last().unwrap().note, ADOPT_NOTE, "mit der Notiz des Vertrags");
    assert!(
        !f.server.state.store.work_template(f.work).await.unwrap().contains("pagebreak"),
        "die Zeile ist aus der Werkvorlage entfernt"
    );
    let view = f.typeset_get().await;
    assert_eq!(view["settings"], json!({ "chapter_start": "right_page" }), "Kapitelanfang in den Einstellungen");
    assert!(view["template_rule"].is_null(), "kein Angebot mehr");

    let after = f.measure().await;
    assert_eq!(after.chapters, before.chapters, "Kapitelanfänge vorher = nachher");
    assert_eq!(after.pages, before.pages, "gleiche Seitenzahl");
}

// ---- (5) Konflikt Vorlage/Einstellung ---------------------------------------------------------------

#[tokio::test]
async fn a_conflicting_template_line_is_named_in_block_and_typeset_page_and_the_setting_wins() {
    let f = ferry("Das Fährhausbuch fünf", "#set page(paper: \"a4\")\n").await;
    let form = format!("work={}&action=save&page_format=a5", f.work);
    let (status, _, body) = f.post("/einstellungen/werk/satz", &form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Speichern: {body}");
    let view = f.typeset_get().await;
    let conflicts = view["conflicts"].as_array().unwrap();
    let page = conflicts.iter().find(|c| c["field"] == "page_format").unwrap_or_else(|| panic!("Konflikt: {view}"));
    assert_eq!(page["effect"], "overrides", "die Einstellung steht dahinter und gilt");
    assert_eq!(page["line"], 2, "Zeile der Werkvorlage");

    let (_, settings_page) = f.server.get(&format!("/einstellungen/werk?work={}", f.work)).await;
    assert!(settings_page.contains("data-typeset-hint=\"page_format\""), "Hinweis beim Feld Seitenformat im Block");
    let (_, satz) = f.server.get(&format!("/werke/{}/satz", f.work)).await;
    let section = satz.split("id=\"druckpruefung\"").nth(1).expect("Druckprüfung");
    assert!(
        section.contains("typeset-conflict\" data-field=\"page_format\" data-effect=\"overrides\""),
        "Konflikt auf der Satz-Seite: {section}"
    );

    let m = f.measure().await;
    let pdf = book_pdf(&f).await;
    assert_eq!(pdf.stand, m.stand, "PDF aus derselben Quelle");
    for (pw, ph) in &m.sizes {
        assert!((pw - mm(148.0)).abs() < 0.5 && (ph - mm(210.0)).abs() < 0.5, "A5 trotz A4 in der Vorlage");
    }
    if let Some(sizes) = pdf_page_sizes(&pdf.bytes, pdf.pages) {
        assert!(sizes.iter().all(|(w, h)| (w - mm(148.0)).abs() < 0.5 && (h - mm(210.0)).abs() < 0.5), "PDF A5");
    }
}

// ---- (6) MCP nur lesend --------------------------------------------------------------------------------

async fn mcp(server: &common::Server, session: Option<&str>, message: Value) -> (axum::http::HeaderMap, Value) {
    let mut request = Request::post("/mcp").header(header::CONTENT_TYPE, "application/json");
    if let Some(id) = session {
        request = request.header("mcp-session-id", id);
    }
    let mut request = request.body(Body::from(message.to_string())).unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (status, headers, body) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "MCP antwortet: {body}");
    (headers, serde_json::from_str(&body).unwrap())
}

#[tokio::test]
async fn mcp_reads_the_settings_and_has_no_way_to_write_them() {
    let f = ferry("Das Fährhausbuch sechs", "").await;
    let (status, _, _) = f.post("/einstellungen/werk/satz", &abnahme_form(f.work)).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let (headers, _) = mcp(
        &f.server,
        None,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                            "clientInfo": { "name": "Abnahme29", "version": "1.0" } } }),
    )
    .await;
    let session = headers.get("mcp-session-id").unwrap().to_str().unwrap().to_owned();
    let (_, list) = mcp(&f.server, Some(&session), json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" })).await;
    let names: Vec<&str> =
        list["result"]["tools"].as_array().unwrap().iter().filter_map(|t| t["name"].as_str()).collect();
    let typeset: Vec<&&str> = names.iter().filter(|n| n.contains("typeset") || n.contains("satz")).collect();
    assert_eq!(typeset, [&"typeset_settings"], "nur das lesende Werkzeug");
    let (_, read) = mcp(
        &f.server,
        Some(&session),
        json!({ "jsonrpc": "2.0", "id": 3, "method": "tools/call",
                "params": { "name": "typeset_settings", "arguments": { "work": f.work } } }),
    )
    .await;
    assert_eq!(read["result"]["structuredContent"]["settings"], abnahme_settings(), "MCP liest: {read}");
    let commits = f.commits();
    let (_, write) = mcp(
        &f.server,
        Some(&session),
        json!({ "jsonrpc": "2.0", "id": 4, "method": "tools/call",
                "params": { "name": "typeset_set", "arguments": { "work": f.work, "settings": {} } } }),
    )
    .await;
    assert!(
        write["error"].is_object() || write["result"]["isError"] == true,
        "ein schreibendes Werkzeug gibt es nicht: {write}"
    );
    assert_eq!(f.typeset_get().await["settings"], abnahme_settings(), "nichts geändert");
    assert_eq!(f.commits(), commits, "kein Commit");
}

// ---- (7) Austausch u6 und Zurücksetzen mit Grabstein über zwei Instanzen -----------------------------

async fn export_xml(server: &common::Server, work: i64) -> String {
    let answer = server.rpc("work.exchange_export", json!({ "work": work, "format": "xml" })).await;
    result(&answer)["xml"].as_str().unwrap().to_owned()
}

async fn import_xml(server: &common::Server, xml: &str, mode: &str) -> Value {
    let answer = server
        .rpc(
            "work.exchange_import",
            json!({ "data": baruch_store::encode_base64(xml.as_bytes()),
                    "sha256": baruch_exchange::sha256_hex(xml.as_bytes()), "mode": mode }),
        )
        .await;
    result(&answer).clone()
}

#[tokio::test]
async fn the_settings_travel_in_u6_and_a_reset_leaves_a_tombstone_across_two_instances() {
    let a = ferry("Das Fährhausbuch sieben", "").await;
    let (status, _, _) = a.post("/einstellungen/werk/satz", &abnahme_form(a.work)).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let xml = export_xml(&a.server, a.work).await;
    // Runde 22 (Vertrag §6.5, spiegel-entwurf): Der Schreiber schreibt jetzt Unterversion 8.
    assert!(xml.contains("version=\"8\"") && xml.contains("<satz uuid="), "Unterversion 8 (seit Runde 22) mit <satz>");
    let (_, exchange_page) = a.server.get(&format!("/werke/{}/austausch", a.work)).await;
    assert!(exchange_page.contains("Satz-Einstellungen: Kapitelanfang"), "Austauschseite nennt die Satz-Einstellungen");

    // B liest als neues Werk ein: dieselben Einstellungen, derselbe Satz.
    let b = common::open().await;
    let imported = import_xml(&b, &xml, "new").await;
    let work_b = imported["work"].as_i64().unwrap();
    let view_a = a.typeset_get().await;
    let view_b = result(&b.rpc("work.typeset_get", json!({ "work": work_b })).await).clone();
    for key in ["settings", "uuid", "rules"] {
        assert_eq!(view_b[key], view_a[key], "{key} auf B wie auf A");
    }
    let stand_a = baruch_server::typeset::prepare(&a.server.state, a.work).await.unwrap().source.main;
    let stand_b = baruch_server::typeset::prepare(&b.state, work_b).await.unwrap().source.main;
    assert_eq!(stand_b, stand_a, "B setzt dieselbe Quelle wie A (Regelblock aus den mitgereisten Einstellungen)");

    // A setzt über das Formular zurück: ein Commit, Grabstein in der Ausgabe.
    let commits = a.commits();
    let (status, _, _) = a.post("/einstellungen/werk/satz", &format!("work={}&action=reset&confirm=1", a.work)).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(a.commits(), commits + 1, "ein Commit");
    assert!(a.last_commit().contains("Baruch-Action: typeset-reset"), "Trailer typeset-reset");
    let xml = export_xml(&a.server, a.work).await;
    let uuid = view_a["uuid"].as_str().unwrap();
    assert!(xml.contains(&format!("<eintrag uuid=\"{uuid}\" art=\"satz\"")), "Grabstein satz in der Ausgabe");

    // B gleicht ab: Regel „nie still überschreiben“ (docs/Austauschformat.md) – B behält seine Einstellungen.
    let merged = import_xml(&b, &xml, "merge").await;
    assert!(merged["refusals"].as_array().unwrap().is_empty(), "nichts abgelehnt: {merged}");
    let after_b = result(&b.rpc("work.typeset_get", json!({ "work": work_b })).await).clone();
    assert_eq!(after_b["settings"], view_a["settings"], "B behält seine Einstellungen (nichts still gelöscht)");

    // Eine dritte Instanz übernimmt die Ausgabe ohne Einstellungen und kennt den Grabstein.
    let c = common::open().await;
    let work_c = import_xml(&c, &xml, "new").await["work"].as_i64().unwrap();
    assert_eq!(
        result(&c.rpc("work.typeset_get", json!({ "work": work_c })).await)["set"],
        false,
        "C ohne Einstellungen"
    );
    let stones: Vec<String> = sqlx::query_scalar("SELECT uuid FROM uuid_tombstone WHERE kind = 'work_typeset'")
        .fetch_all(c.state.store.pool())
        .await
        .unwrap();
    assert_eq!(stones, vec![uuid.to_owned()], "Grabstein work_typeset auf C");
}
