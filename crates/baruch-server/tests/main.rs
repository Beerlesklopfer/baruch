// SPDX-License-Identifier: AGPL-3.0-or-later
//! Alle Integrationstests von `baruch-server` in **einer** Test-Binärdatei (Runde 13, Vorlauf „bau“).
//!
//! Jede Test-Binärdatei linkt den ganzen Server samt LanceDB statisch mit (≈ 500 MB); mit einer Datei je
//! Fachgebiet wuchs `target/` auf ≈ 40 GB. Darum ist `autotests = false` gesetzt (`Cargo.toml`) und jede
//! bisherige Datei `tests/<gebiet>.rs` ein Modul dieses Bündels. **Neue Testdatei:** `tests/<gebiet>.rs`
//! anlegen und unten `mod <gebiet>;` eintragen (der Test [`every_test_file_is_bundled`] erinnert daran);
//! gemeinsame Hilfen über `use crate::common;` statt `mod common;`.
//!
//! Alle Tests laufen in einem Prozess: Prozessweite Zustände (Markierung „PDF entsteht“ je Werk-id,
//! Warteschlange der Modelle, MCP-Sitzungen, Satz-Semaphore) teilen sich jetzt alle Module, nicht mehr nur
//! die Tests einer Datei. PDF-Erzeugung nur unter [`common::PDF_SERIAL`]; Umgebungsvariablen nie setzen.
//! Einzeln laufen lassen: `cargo test -p baruch-server --test server <gebiet>::`.

mod common;
mod pgvector_support;

mod assets;
mod concurrency;
mod editor_layout;
mod fassungen_ausblenden;
mod git_mirror;
mod headings;
mod ideas;
mod import;
mod kritik_zyklus;
mod library_search;
mod literature;
mod markdown_headings;
mod mcp;
mod mcp_assets;
mod mcp_ideas;
mod node_keys;
mod numbering;
mod origin;
mod outline;
mod outline_consent;
mod outline_mcp;
mod pages;
mod personas;
mod proposals;
mod quote_find;
mod revisions;
mod round10;
mod round10_models;
mod round10_personas;
mod round10_scripture;
mod round10_tokens;
mod round11;
mod round11_cover;
mod round11_images;
mod round11_mcp_assets;
mod round11_pdf;
mod round11_personas;
mod round12;
mod round12_lancedb;
mod round12_multigpt;
mod round12_personas_rag;
mod round12_pgvector;
mod round12_search_ui;
mod round13;
mod round13_abgleich;
mod round13_quellen;
mod round13_websuche;
mod round14;
mod round14_conversion;
mod round14_numbering;
mod round14_outline;
mod round14_parts;
mod round15;
mod round15_anbieter;
mod round15_auftraege;
mod round15_bibel_literatur;
mod round15_lektor;
mod round15_mcp_chat;
mod round15_nachbesserung;
mod round15_sword;
mod round16;
mod round16_antraege;
mod round16_lektor;
mod round16_predigt;
mod round16_stichwort;
mod round16_zeit;
mod round17;
mod round17_lektor;
mod round17_pruefanzeige;
mod round17_stellen;
mod round17_verszaehlung;
mod round18_abnahme;
mod round18_konstrukte;
mod round18_lektor;
mod round18_menueband;
mod round18_satzbild;
mod round19_abnahme;
mod round19_index;
mod round19_pgvector;
mod round19_treffer;
mod round20_abnahme;
mod round20_epub;
mod round20_pruefung;
mod round20_reader;
mod round21_abnahme;
mod round21_bilder;
mod round21_markdown_import;
mod round21_nachbesserung;
mod round21_pdf_import;
mod round22_abgleich;
mod round22_abnahme;
mod round22_archiv;
mod round22_befunde;
mod round22_entwurf;
mod round22_fassungen;
mod round22_seitenzweige;
mod round23_abnahme;
mod round23_geheimnisse;
mod round23_mcp_namen;
mod round23_web;
mod round24_abnahme;
mod round24_betrieb;
mod round24_pakete;
mod round25_abnahme;
mod round25_bild;
mod round25_daten;
mod round25_widget;
mod round26_export;
mod round26_ids;
mod round26_import;
mod round26_roundtrip;
mod round27_abnahme;
mod round27_daten;
mod round27_satz;
mod round27_widget;
mod round28_abnahme;
mod round28_befunde;
mod round28_chat;
mod round28_uebersicht;
mod round29_abnahme;
mod round29_daten;
mod round29_oberflaeche;
mod round29_typst;
mod round3;
mod round30_abnahme;
mod round30_bedienung;
mod round30_live;
mod round30_loesungen;
mod round30_suchen;
mod round30_varianten;
mod round30_verweise;
mod round31_abnahme;
mod round31_glossar;
mod round31_kontextmenue;
mod round31_register;
mod round32_abnahme;
mod round32_beenden;
mod round32_docx_export;
mod round32_docx_import;
mod round33_abnahme;
mod round33_laptop;
mod round33_menueband;
mod round33_seiten;
mod round34_abnahme;
mod round34_austausch;
mod round34_daten;
mod round34_widget;
mod round35_abnahme;
mod round35_konten;
mod round35_oberflaeche;
mod round35_rollen;
mod round35_schreibwege;
mod round36_abnahme;
mod round36_lektor;
mod round36_mindmap;
mod round36_reste;
mod round36_sqlx;
mod round36_typst;
mod round37_bilder;
mod round37_hilfe;
mod round38_sprache;
mod round4;
mod round5;
mod round5_fixes;
mod round6;
mod round7;
mod round7_titles;
mod round8;
mod round8_inline;
mod round8_posts;
mod round8_typst;
mod round9_connections;
mod round9_integration;
mod round9_menu;
mod round9_settings;
mod rpc;
mod satzbild;
mod sword_bibles;
mod tree;
mod work_crud;
mod works;
mod wysiwyg;
mod zoom;

/// Jede Datei `tests/*.rs` (außer dieser) ist hier als Modul eingetragen; sonst würde sie wegen
/// `autotests = false` stillschweigend nicht gebaut und ihre Tests gingen verloren.
#[test]
fn every_test_file_is_bundled() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests");
    let main = include_str!("main.rs");
    let mut missing = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_some_and(|e| e == "rs") {
            let stem = path.file_stem().unwrap().to_str().unwrap().to_owned();
            if stem != "main" && !main.lines().any(|l| l == format!("mod {stem};")) {
                missing.push(stem);
            }
        }
    }
    assert!(missing.is_empty(), "Testdateien ohne `mod …;` in tests/main.rs (würden nicht gebaut): {missing:?}");
}
