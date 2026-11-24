// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 27, Aufgabe umschlag-satz (R27b): Umschlag-Anordnung, Titelei aus der Werkvorlage und Goldstände.
//!
//! - **Goldstände** (`tests/data/cover_r26/`): `frame_parts` und `wrap_source` ohne Anordnung und ohne
//!   Titelei-Funktionen, gesichert aus dem Stand der Schnittstelle S1 (= af07921 für diese Ausgaben) vor der ersten
//!   Änderung von R27b; der Test vergleicht byte-genau.
//! - Gesetzt (Typst) wird in `baruch-render/tests/cover_layout.rs` und in den Server-Tests (`round27_satz.rs`).

use std::path::PathBuf;

use baruch_typst::cover::{BookInfo, CoverRef, FrameOptions, TitleArea, WrapOptions, frame_parts, wrap_source};

/// Ordner der Goldstände.
fn golden_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/cover_r26")
}

/// Erfundene, voll ausgefüllte Buchangaben (ohne die Anschriften, die es in af07921 noch nicht gab).
fn full_info() -> BookInfo {
    BookInfo {
        title: "Das Wolkenhaus".into(),
        subtitle: "Geschichten vom Dachboden".into(),
        author: "Mira Mondschein".into(),
        publisher: "Lampenverlag".into(),
        place: "Kieselbach".into(),
        year: "2026".into(),
        edition: "2. Auflage".into(),
        isbn: "978-3-16-148410-0".into(),
        price: "18,00 €".into(),
        series: "Reihe Dachkammer".into(),
        printer: "Druckhaus Feder".into(),
        bible: "Erfundene Übersetzung".into(),
        bible_notice: "Mit freundlicher Genehmigung.".into(),
        image_credit: "Wolke, Zeichnung: erfunden".into(),
        dedication: "Für alle, die nach oben schauen.\n\nUnd für Tante Ilse.".into(),
        blurb: "Ein Haus aus Wolken.\nZweite Zeile.\n\nEin zweiter Absatz.".into(),
        bio: "Mira Mondschein wohnt unterm Dach.".into(),
        ..BookInfo::default()
    }
}

/// Nur ein Titel.
fn bare_info() -> BookInfo {
    BookInfo { title: "Nur ein Titel".into(), ..BookInfo::default() }
}

/// Angaben voller Zeichen, die in Typst etwas bedeuten.
fn tricky_info() -> BookInfo {
    let nasty = "Zitat \" Rückstrich \\ #raute ] klammer ) $mathe$ // kommentar /* block */ *fett* _x_\nzweite Zeile";
    BookInfo {
        title: nasty.into(),
        subtitle: nasty.into(),
        author: nasty.into(),
        publisher: nasty.into(),
        place: nasty.into(),
        year: "20\"26".into(),
        edition: nasty.into(),
        isbn: "kein ) ISBN".into(),
        price: nasty.into(),
        series: nasty.into(),
        printer: nasty.into(),
        bible: nasty.into(),
        bible_notice: nasty.into(),
        image_credit: nasty.into(),
        dedication: nasty.into(),
        blurb: nasty.into(),
        bio: nasty.into(),
        ..BookInfo::default()
    }
}

fn cover() -> Option<CoverRef> {
    Some(CoverRef { path: ".baruch/wolke.png".into(), alt: "Eine \"Wolke\" ]".into() })
}

/// Die Fälle für `frame_parts`: (Name, Angaben, Optionen).
pub fn frame_cases() -> Vec<(&'static str, BookInfo, FrameOptions)> {
    let with_cover = |area| FrameOptions {
        cover: cover(),
        cover_pages: true,
        title_area: area,
        barcode: Some(".baruch/isbn.svg".into()),
        author_photo: Some(".baruch/foto.png".into()),
        ..FrameOptions::default()
    };
    vec![
        ("ohne", full_info(), FrameOptions::default()),
        ("umschlag_ohne_seiten", full_info(), FrameOptions { cover: cover(), ..FrameOptions::default() }),
        ("umschlag_oben", full_info(), with_cover(TitleArea::Top)),
        ("umschlag_mitte", full_info(), with_cover(TitleArea::Middle)),
        ("umschlag_unten", full_info(), with_cover(TitleArea::Bottom)),
        (
            "umschlag_schlicht",
            bare_info(),
            FrameOptions { cover: cover(), cover_pages: true, ..FrameOptions::default() },
        ),
        ("titelei", full_info(), FrameOptions { front_matter: true, ..FrameOptions::default() }),
        ("titelei_schlicht", bare_info(), FrameOptions { front_matter: true, ..FrameOptions::default() }),
        ("beides", full_info(), FrameOptions { front_matter: true, ..with_cover(TitleArea::Bottom) }),
        ("beides_zeichen", tricky_info(), FrameOptions { front_matter: true, ..with_cover(TitleArea::Middle) }),
    ]
}

/// Die Fälle für `wrap_source`: Rücken unter und über 6 mm, mit und ohne Bild.
pub fn wrap_cases() -> Vec<(&'static str, BookInfo, WrapOptions)> {
    let base = |spine_mm, area| WrapOptions {
        page_width_mm: 148.0,
        page_height_mm: 210.0,
        spine_mm,
        bleed_mm: 3.0,
        title_area: area,
        cover: cover(),
        barcode: Some(".baruch/isbn.svg".into()),
        author_photo: Some(".baruch/foto.png".into()),
        layout: None,
    };
    vec![
        ("ruecken_duenn", full_info(), base(4.5, TitleArea::Top)),
        ("ruecken_dick", full_info(), base(14.0, TitleArea::Bottom)),
        ("ruecken_grenze", full_info(), base(6.0, TitleArea::Middle)),
        (
            "ohne_bild",
            bare_info(),
            WrapOptions { cover: None, barcode: None, author_photo: None, ..base(9.0, TitleArea::Top) },
        ),
        ("ohne_beschnitt", full_info(), WrapOptions { bleed_mm: 0.0, ..base(20.0, TitleArea::Top) }),
        ("zeichen", tricky_info(), base(12.0, TitleArea::Middle)),
    ]
}

fn read_golden(name: &str) -> String {
    let path = golden_dir().join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("Goldstand {} fehlt: {e}", path.display()))
}

/// Schreibt die Goldstände neu. Nur einmal aus dem Stand S1 vor der ersten Änderung ausgeführt
/// (`cargo test -p baruch-typst --test cover_layout -- --ignored`); danach nie wieder, sonst verlöre der
/// Vergleich seinen Sinn.
#[test]
#[ignore = "schreibt die Goldstände; nur einmal aus dem Stand S1"]
fn write_goldens_from_the_interface_state() {
    let dir = golden_dir();
    std::fs::create_dir_all(&dir).unwrap();
    for (name, info, opts) in frame_cases() {
        let parts = frame_parts(&info, &opts);
        std::fs::write(dir.join(format!("frame_{name}.front.typ")), &parts.front).unwrap();
        std::fs::write(dir.join(format!("frame_{name}.back.typ")), &parts.back).unwrap();
        std::fs::write(dir.join(format!("frame_{name}.pages")), format!("{}\n", parts.added_cover_pages)).unwrap();
    }
    for (name, info, opts) in wrap_cases() {
        std::fs::write(dir.join(format!("wrap_{name}.typ")), wrap_source(&info, &opts)).unwrap();
    }
}

#[test]
fn without_layout_and_template_functions_frame_parts_match_the_goldens_byte_for_byte() {
    for (name, info, opts) in frame_cases() {
        let parts = frame_parts(&info, &opts);
        assert_eq!(
            parts.front,
            read_golden(&format!("frame_{name}.front.typ")),
            "Fall {name}: Vorderteil muss byte-gleich zum Stand vor R27 bleiben (bestehende Werke setzen gleich)"
        );
        assert_eq!(
            parts.back,
            read_golden(&format!("frame_{name}.back.typ")),
            "Fall {name}: Rückseite muss byte-gleich bleiben"
        );
        assert_eq!(
            format!("{}\n", parts.added_cover_pages),
            read_golden(&format!("frame_{name}.pages")),
            "Fall {name}: Zahl der Umschlagseiten unverändert"
        );
    }
}

#[test]
fn without_layout_wrap_source_matches_the_goldens_byte_for_byte() {
    for (name, info, opts) in wrap_cases() {
        assert_eq!(
            wrap_source(&info, &opts),
            read_golden(&format!("wrap_{name}.typ")),
            "Fall {name}: Umschlag für die Druckerei muss ohne Anordnung byte-gleich bleiben"
        );
    }
}

// ---- Anordnung -------------------------------------------------------------------------------------

use baruch_typst::cover::{
    BOOK_DICT, FRAME_MARK, IMPRINT_COMPLETE, TemplateFunctions, book_dict, imprint_lines, imprint_template,
    imprint_template_missing, template_functions,
};
use baruch_typst::cover_layout::{
    CoverBlockKind, CoverFace, CoverGeometry, CoverLayout, LEADING_EM, POS_MM, SIZE_PT, SpineDirection, WIDTH_MM,
    default_blocks, effective_blocks,
};

#[test]
fn default_blocks_cover_all_kinds_within_the_limits_and_follow_the_title_area() {
    for spine in [0.0, 4.0, 6.0, 30.0] {
        let geometry = CoverGeometry::new(148.0, 210.0, spine, 3.0);
        for area in [TitleArea::Top, TitleArea::Middle, TitleArea::Bottom] {
            let blocks = default_blocks(area, &geometry);
            let kinds: Vec<CoverBlockKind> = blocks.iter().map(|b| b.kind).collect();
            assert_eq!(kinds, CoverBlockKind::ALL, "alle 15 Arten in ALL-Reihenfolge");
            for b in &blocks {
                let ok = (POS_MM.0..=POS_MM.1).contains(&b.x_mm)
                    && (POS_MM.0..=POS_MM.1).contains(&b.y_mm)
                    && (WIDTH_MM.0..=WIDTH_MM.1).contains(&b.width_mm)
                    && (SIZE_PT.0..=SIZE_PT.1).contains(&b.size_pt)
                    && (LEADING_EM.0..=LEADING_EM.1).contains(&b.leading_em);
                assert!(ok, "Vorgabeblock innerhalb der CHECKs der Migration 0045: {b:?}");
                assert_eq!(b.direction.is_some(), b.kind.face() == CoverFace::Spine, "direction nur am Rücken: {b:?}");
                assert_eq!(b.text.is_some(), b.kind.own_text(), "eigener Text nur bei Freitext und Motto: {b:?}");
                if b.kind.own_text() {
                    assert!(!b.visible, "leere Freitexte und Motto sind ausgeblendet: {b:?}");
                }
            }
            let spine_title = &blocks[5];
            assert_eq!(spine_title.visible, spine >= 6.0, "Rückentext erst ab 6 mm wie bisher (Rücken {spine} mm)");
            assert_eq!(spine_title.direction, Some(SpineDirection::Down), "von oben nach unten wie bisher");
        }
        let top = default_blocks(TitleArea::Top, &geometry)[0].y_mm;
        let middle = default_blocks(TitleArea::Middle, &geometry)[0].y_mm;
        let bottom = default_blocks(TitleArea::Bottom, &geometry)[0].y_mm;
        assert!(top < middle && middle < bottom, "Titel folgt dem Titelbereich: {top} < {middle} < {bottom}");
        assert!(middle > 80.0 && middle < 110.0, "Mitte liegt um die halbe Höhe (210 mm): {middle}");
    }
}

#[test]
fn wrap_source_with_layout_places_blocks_relative_to_the_trim_edge_of_their_face() {
    let geometry = CoverGeometry::new(148.0, 210.0, 12.0, 3.0);
    let mut title = default_blocks(TitleArea::Top, &geometry)[0].clone();
    title.x_mm = -2.0;
    title.y_mm = 15.5;
    let layout = CoverLayout { blocks: vec![title] };
    let (_, info, base) = wrap_cases().into_iter().next().unwrap();
    let opts = WrapOptions { spine_mm: 12.0, layout: Some(layout.clone()), ..base.clone() };
    let source = wrap_source(&info, &opts);
    // Vorderseite ab b + W + S = 3 + 148 + 12 = 163 mm, Block bei x = −2 mm (in den Beschnitt) → 161 mm.
    assert!(
        source.contains("#place(top + left, dx: 161.000mm, dy: 18.500mm, block(width: "),
        "Titel bei Schnittkante + (x, y), negatives x ragt in den Beschnitt:\n{source}"
    );
    let effective = effective_blocks(&layout, TitleArea::Top, &geometry);
    assert_eq!(effective.iter().filter(|(_, stored)| *stored).count(), 1, "nur der Titel ist gespeichert");
    assert!(source.contains("// back_blurb"), "fehlende Arten kommen aus den Vorgabeblöcken (Klappentext)");
    assert!(
        !source.contains("rotate(90deg, reflow: true, text("),
        "das alte Rückenetikett entfällt im Anordnungsmodus"
    );
    let empty = WrapOptions { layout: Some(CoverLayout::default()), ..base };
    assert_eq!(
        wrap_source(&info, &empty),
        read_golden("wrap_ruecken_duenn.typ"),
        "eine leere Anordnung ist keine Anordnung: byte-gleich zum Goldstand"
    );
}

#[test]
fn frame_parts_with_layout_set_only_the_given_blocks_on_cover_pages() {
    let geometry = CoverGeometry::new(148.0, 210.0, 12.0, 3.0);
    let mut blocks = default_blocks(TitleArea::Top, &geometry);
    blocks.retain(|b| b.kind.face() != CoverFace::Spine);
    let layout = CoverLayout { blocks };
    let (_, info, opts) = frame_cases().into_iter().find(|(n, ..)| *n == "beides").unwrap();
    let parts = frame_parts(&info, &FrameOptions { layout: Some(layout), ..opts });
    let cover_page = &parts.front[..parts.front.find("// Umschlag, Innenseite").unwrap()];
    assert!(cover_page.contains("place(top + left, dx: 14.800mm, dy: "), "Titel bei x = 10 % von 148 mm: {cover_page}");
    assert!(cover_page.contains("\"Das Wolkenhaus\""), "Titel als String-Literal: {cover_page}");
    assert!(!cover_page.contains("inset: (x: 10%, y: 9%)"), "der alte Titelblock entfällt: {cover_page}");
    assert!(
        parts.back.contains("// back_barcode") && parts.back.contains(".baruch/isbn.svg"),
        "Strichcode-Block hinten"
    );
    assert!(parts.back.contains(".baruch/foto.png"), "Kurzbiografie mit Foto hinten");
    assert_eq!(parts.added_cover_pages, 3, "weiter drei Umschlagseiten");
}

// ---- Titelei aus der Werkvorlage -----------------------------------------------------------------------

#[test]
fn template_functions_are_recognised_only_as_functions_at_line_start() {
    let template = "#set text(lang: \"de\")\n#let impressum(buch) = [x]\n  #let widmung (buch) = [y]\n\
                    #let haupttitel = [kein Aufruf]\n// #let schmutztitel(buch) = []\n#letschmutztitel(b) = []\n";
    assert_eq!(
        template_functions(template),
        TemplateFunctions { half_title: false, main_title: false, imprint: true, dedication: true },
        "impressum und widmung (auch eingerückt, mit Leerraum vor der Klammer); keine Variable, kein Kommentar"
    );
    assert_eq!(template_functions(""), TemplateFunctions::default(), "leere Vorlage: keine Funktion");
}

#[test]
fn template_functions_get_their_own_pages_at_fixed_places_and_fall_back_one_by_one() {
    let (_, info, opts) = frame_cases().into_iter().find(|(n, ..)| *n == "titelei").unwrap();
    let only_imprint = TemplateFunctions { imprint: true, ..TemplateFunctions::default() };
    let parts = frame_parts(&info, &FrameOptions { template_functions: only_imprint, ..opts.clone() });
    let golden = read_golden("frame_titelei.front.typ");
    assert!(
        parts.front.starts_with(&format!("{FRAME_MARK}\n#let {BOOK_DICT} = (\n")),
        "Wörterbuch direkt nach der Marke"
    );
    assert_eq!(parts.front.matches(&format!("#let {BOOK_DICT}")).count(), 1, "das Wörterbuch genau einmal");
    let pages: Vec<&str> = parts.front.split("#page(").skip(1).collect();
    assert!(pages[2].contains("text(size: 26pt"), "Seite 3: eingebauter Haupttitel bleibt (Rückfall)");
    assert!(
        pages[3].starts_with("header: none, footer: none, numbering: none, {\n  impressum(baruch_buch)\n})\n"),
        "Seite 4 = Rückseite des Haupttitels: eigene Seite mit impressum(baruch_buch): {}",
        pages[3]
    );
    assert!(
        !parts.front.contains("text(\"Bibliografische Information der Deutschen Nationalbibliothek\""),
        "die eingebaute Impressumsseite entfällt (der Text steht nur noch im Wörterbuch)"
    );
    assert!(golden.contains("Bibliografische Information"), "der Goldstand hat sie (Rückfall ohne Funktion)");
    // Der Rest (Schmutztitel, Seite 2, Haupttitel, Widmung) bleibt wörtlich wie im Goldstand.
    let golden_pages: Vec<&str> = golden.split("#page(").skip(1).collect();
    for i in [0usize, 1, 2, 4, 5] {
        assert_eq!(pages[i], golden_pages[i], "Seite {}: Rückfall byte-gleich zur eingebauten Seite", i + 1);
    }

    let all = TemplateFunctions { half_title: true, main_title: true, imprint: true, dedication: true };
    let parts = frame_parts(&info, &FrameOptions { template_functions: all, ..opts.clone() });
    let at = |needle: &str| parts.front.find(needle).unwrap_or_else(|| panic!("{needle} fehlt:\n{}", parts.front));
    assert!(
        at("  schmutztitel(baruch_buch)") < at("\"Reihe Dachkammer\")")
            && at("\"Reihe Dachkammer\")") < at("  haupttitel(baruch_buch)")
            && at("  haupttitel(baruch_buch)") < at("  impressum(baruch_buch)")
            && at("  impressum(baruch_buch)") < at("  widmung(baruch_buch)"),
        "feste Reihenfolge: Schmutztitel, Seite 2 (Reihe), Haupttitel, Impressum, Widmung"
    );
    assert_eq!(
        parts.front.split("#page(").count() - 1,
        6,
        "sechs Seiten: vier Funktionen, Seite 2, Leerseite nach der Widmung"
    );

    // Ohne Titelei wirken die Funktionen nicht, ohne Widmung keine Widmungsseite.
    let off = frame_parts(&info, &FrameOptions { template_functions: all, front_matter: false, ..opts.clone() });
    assert!(off.front.is_empty(), "ohne Titelei (und ohne Umschlag) bleibt alles leer");
    let mut no_dedication = info.clone();
    no_dedication.dedication.clear();
    let parts = frame_parts(&no_dedication, &FrameOptions { template_functions: all, ..opts });
    assert!(
        !parts.front.contains("widmung(baruch_buch)"),
        "keine Widmung angegeben → keine Widmungsseite (wie Rückfall)"
    );
}

/// Liest `#let baruch_buch = (…)` zeilenweise: jede Zeile `  schlüssel: "…",` mit einem einzigen String-Literal
/// (nur `\\`, `\"`, `\n`, `\t` als Maskierung) bzw. die Zeile `impressum_zeilen` mit einem Array solcher Literale.
fn dict_only_strings(dict: &str) -> Result<Vec<(String, Vec<String>)>, String> {
    fn literals(mut rest: &str) -> Result<(Vec<String>, &str), String> {
        let mut out = Vec::new();
        loop {
            rest = rest.trim_start_matches([' ', ',']);
            let Some(body) = rest.strip_prefix('"') else { return Ok((out, rest)) };
            let mut value = String::new();
            let mut chars = body.char_indices();
            let end = loop {
                let Some((i, c)) = chars.next() else { return Err(format!("Literal ohne Ende: {rest}")) };
                match c {
                    '"' => break i,
                    '\\' => match chars.next() {
                        Some((_, 'n')) => value.push('\n'),
                        Some((_, 't')) => value.push('\t'),
                        Some((_, c @ ('\\' | '"'))) => value.push(c),
                        other => return Err(format!("unerlaubte Maskierung {other:?}")),
                    },
                    c => value.push(c),
                }
            };
            out.push(value);
            rest = &body[end + 1..];
        }
    }
    let mut lines = dict.lines();
    if lines.next() != Some("#let baruch_buch = (") {
        return Err("Kopfzeile fehlt".into());
    }
    let mut entries = Vec::new();
    for line in lines {
        if line == ")" {
            return Ok(entries);
        }
        let line = line.strip_prefix("  ").ok_or("Einrückung")?;
        let (key, value) = line.split_once(": ").ok_or("Schlüssel")?;
        if !key.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
            return Err(format!("Schlüssel {key:?}"));
        }
        // `literals` schluckt die Kommas zwischen und nach den Literalen; danach darf nichts mehr kommen (bzw. beim
        // Array nur `),`).
        let (values, rest) = if let Some(array) = value.strip_prefix('(') {
            let (values, rest) = literals(array)?;
            (values, rest.strip_prefix("),").ok_or(format!("Array ohne Ende: {rest}"))?)
        } else {
            let (values, rest) = literals(value)?;
            if values.len() != 1 || !value.ends_with("\",") {
                return Err(format!("{key}: genau ein Literal mit Komma erwartet"));
            }
            (values, rest)
        };
        if !rest.is_empty() {
            return Err(format!("{key}: Rest nach dem Literal: {rest:?}"));
        }
        entries.push((key.to_owned(), values));
    }
    Err("Schlusszeile fehlt".into())
}

#[test]
fn the_book_dictionary_has_the_contract_keys_and_only_string_literals_even_for_hostile_input() {
    let mut info = tricky_info();
    info.publisher_address = "Weg \" 1 ) # ]\n12345 $Ort$ // x /* y".into();
    info.printer_address = "Druck\\gasse\n9".into();
    let dict = book_dict(&info);
    let entries =
        dict_only_strings(&dict).unwrap_or_else(|e| panic!("Wörterbuch nicht nur aus Literalen ({e}):\n{dict}"));
    let keys: Vec<&str> = entries.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(
        keys,
        [
            "titel",
            "untertitel",
            "autor",
            "verlag",
            "verlag_anschrift",
            "ort",
            "jahr",
            "auflage",
            "isbn",
            "preis",
            "reihe",
            "druck",
            "druck_anschrift",
            "bibel",
            "bibel_hinweis",
            "bildnachweis",
            "widmung",
            "klappentext",
            "vita",
            "copyright",
            "dnb_titel",
            "dnb_text",
            "gesetzt",
            "impressum_zeilen"
        ],
        "Schlüssel nach Vertrag 1.6"
    );
    let get = |key: &str| entries.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone()).unwrap();
    assert_eq!(get("verlag_anschrift"), [info.publisher_address.clone()], "Anschrift kommt wörtlich zurück");
    assert_eq!(get("titel"), [info.title.trim().to_owned()], "Titel wörtlich");
    assert_eq!(get("gesetzt"), ["Gesetzt mit Baruch"], "Satzvermerk");
    assert_eq!(get("impressum_zeilen"), imprint_lines(&info), "impressum_zeilen = imprint_lines");
    let empty = book_dict(&BookInfo::default());
    let entries = dict_only_strings(&empty).unwrap();
    assert!(entries.iter().any(|(k, v)| k == "untertitel" && v == &[String::new()]), "leer = \"\": {empty}");
}

#[test]
fn imprint_template_missing_appends_only_what_is_missing_and_refuses_when_complete() {
    let template = "#set text(lang: \"de\")\n#let impressum(buch) = [eigenes Impressum]";
    let extended = imprint_template_missing(template).expect("drei Funktionen fehlen");
    assert!(extended.starts_with(template), "die Vorlage bleibt vorn unverändert");
    assert_eq!(extended.matches("#let impressum(").count(), 1, "impressum nicht doppelt");
    for name in ["schmutztitel", "haupttitel", "widmung"] {
        assert!(extended.contains(&format!("#let {name}(buch)")), "{name} angehängt");
    }
    assert_eq!(
        template_functions(&extended),
        TemplateFunctions { half_title: true, main_title: true, imprint: true, dedication: true },
        "danach sind alle vier da"
    );
    assert_eq!(imprint_template_missing(&extended), Err(IMPRINT_COMPLETE), "alles da → ehrliche Ablehnung");
    let fresh = imprint_template_missing("").unwrap();
    assert!(fresh.contains(imprint_template().trim()), "leere Vorlage bekommt die ganze Impressum-Vorlage");
    assert!(!imprint_template().is_empty() && !imprint_template().starts_with('\n'), "Vorlage beginnt mit dem Kopf");
}
