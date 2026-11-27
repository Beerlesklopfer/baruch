// SPDX-License-Identifier: AGPL-3.0-or-later
//! Shared interface of the RAG ways (contract Round 12, appendix A): names, reports, snippets and error codes. In
//! addition the test checks the file itself against the contract (SHA-256 of `src/lib.rs` and `Cargo.toml`), so
//! that nobody changes it unnoticed.

use baruch_rag::{
    EmbedPrefixes, Hit, MAX_PREFIX_CHARS, MAX_SNIPPET_CHARS, RagError, WEAK_LABEL, WEAK_SCORE, Way, WayReport,
    WayState, check_prefix, clean_snippet, is_weak,
};
use serde_json::json;

#[test]
fn ways_have_fixed_names_and_parse_back() {
    assert_eq!(Way::ALL, [Way::Lancedb, Way::Pgvector, Way::Multigpt], "feste Reihenfolge (auch bei Gleichstand)");
    for way in Way::ALL {
        assert_eq!(Way::parse(way.as_str()), Some(way), "{way}: Name → Weg");
        assert_eq!(way.to_string(), way.as_str(), "Display = as_str");
        assert_eq!(json!(way), json!(way.as_str()), "JSON klein geschrieben");
    }
    assert_eq!(Way::parse(" pgvector "), Some(Way::Pgvector), "Rand zählt nicht");
    assert_eq!(Way::parse("LanceDB"), None, "Groß geschrieben ist kein Name");
    assert_eq!(Way::parse("fts"), None, "unbekannt → None");
    assert_eq!(Way::Lancedb.title(), "LanceDB (lokal)", "Anzeigename");
}

#[test]
fn a_report_carries_the_label_of_its_state() {
    let report = WayReport::new(Way::Multigpt, WayState::Failed, "Zeitlimit", 0, 20_000);
    assert_eq!(report.label, "ausgefallen", "label folgt dem Zustand");
    assert_eq!(WayState::Connected.label(), "verbunden", "verbunden");
    assert_eq!(WayState::Off.label(), "aus", "aus");
    let value = serde_json::to_value(&report).unwrap();
    assert_eq!(
        value,
        json!({ "way": "multigpt", "state": "failed", "label": "ausgefallen", "message": "Zeitlimit",
                "hits": 0, "millis": 20000 }),
        "JSON-Form des Berichts"
    );
}

#[test]
fn snippets_lose_control_characters_and_are_cut_with_an_ellipsis() {
    assert_eq!(clean_snippet("  a\tb\u{7}c\r\nd  "), "a b c \nd", "Steuerzeichen → Leerzeichen, Zeilenumbruch bleibt");
    let long = "x".repeat(MAX_SNIPPET_CHARS + 50);
    let cut = clean_snippet(&long);
    assert_eq!(cut.chars().count(), MAX_SNIPPET_CHARS, "auf 600 Zeichen gekürzt");
    assert!(cut.ends_with('…'), "mit Auslassungszeichen: {cut}");
    let exact = "y".repeat(MAX_SNIPPET_CHARS);
    assert_eq!(clean_snippet(&exact), exact, "genau 600 bleiben unverändert");
}

#[test]
fn errors_have_fixed_codes_and_only_not_configured_means_off() {
    let all = [
        (RagError::Invalid("a".into()), "invalid"),
        (RagError::NotConfigured("b".into()), "not_configured"),
        (RagError::NoEmbedder("c".into()), "no_embedder"),
        (RagError::ModelMismatch("d".into()), "model_mismatch"),
        (RagError::Unreachable("e".into()), "unreachable"),
        (RagError::Denied("f".into()), "denied"),
        (RagError::Timeout("g".into()), "timeout"),
        (RagError::Schema("h".into()), "schema"),
        (RagError::Failed("i".into()), "failed"),
    ];
    for (error, code) in &all {
        assert_eq!(error.code(), *code, "Code von {error:?}");
        assert_eq!(error.is_off(), *code == "not_configured", "nur NotConfigured heißt „aus“: {error:?}");
        assert_eq!(error.to_string(), error.message(), "Display = Satz");
    }
}

#[test]
fn the_identity_of_a_hit_ignores_case_and_margins() {
    let hit = |document: &str, section: Option<&str>| Hit {
        way: Way::Pgvector,
        collection: "c".into(),
        document: document.into(),
        section: section.map(str::to_owned),
        title: String::new(),
        snippet: String::new(),
        score: 0.5,
        url: None,
        meta: json!({}),
    };
    assert_eq!(hit(" Doc-1 ", Some("A ")).identity(), hit("doc-1", Some(" a")).identity(), "Rand und Groß egal");
    assert_eq!(hit("d", None).identity(), ("d".to_owned(), String::new()), "ohne Stelle: leer");
    assert_ne!(hit("d", Some("1")).identity(), hit("d", Some("2")).identity(), "andere Stelle, anderer Treffer");
}

/// SHA-256 of the files fixed verbatim (contract Round 12, appendix A; `src/lib.rs` extended by Round 19, S1;
/// `Cargo.toml` extended by Round 24, S1: `publish = false`; `src/lib.rs` extended in Round 24 by an `#[allow]`
/// with a reason before `is_weak`, code unchanged; comments of `src/lib.rs` translated into English by the
/// integrator of Round 38b, code and strings unchanged), the same in all branches.
const LIB_SHA256: &str = "4ea249117202614e13a7e255376db6827adc11af819de3b2663bf5dd420bcdb0";
const CARGO_SHA256: &str = "a034c2f423452dbf2f8caca79317c13921620cec85c6262fac622ca7ddd5eafe";

fn sha256_hex(bytes: &[u8]) -> String {
    // Small SHA-256 of its own (the crate deliberately has only serde and serde_json).
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5, 0xd807aa98,
        0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786,
        0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8,
        0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
        0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819,
        0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a,
        0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] =
        [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19];
    let mut data = bytes.to_vec();
    let bits = (bytes.len() as u64).wrapping_mul(8);
    data.push(0x80);
    while data.len() % 64 != 56 {
        data.push(0);
    }
    data.extend_from_slice(&bits.to_be_bytes());
    for block in data.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([block[4 * i], block[4 * i + 1], block[4 * i + 2], block[4 * i + 3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ (!v[4] & v[6]);
            let t1 = v[7].wrapping_add(s1).wrapping_add(ch).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v = [t1.wrapping_add(t2), v[0], v[1], v[2], v[3].wrapping_add(t1), v[4], v[5], v[6]];
        }
        for (a, b) in h.iter_mut().zip(v) {
            *a = a.wrapping_add(b);
        }
    }
    h.iter().map(|x| format!("{x:08x}")).collect()
}

#[test]
fn the_contract_files_are_byte_identical_to_annex_a() {
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        "SHA-256 selbst geprüft (Testvektor „abc“)"
    );
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib = std::fs::read(dir.join("src/lib.rs")).unwrap();
    let cargo = std::fs::read(dir.join("Cargo.toml")).unwrap();
    assert_eq!(sha256_hex(&lib), LIB_SHA256, "src/lib.rs weicht vom Vertrag (Anhang A) ab");
    assert_eq!(sha256_hex(&cargo), CARGO_SHA256, "Cargo.toml weicht vom Vertrag (Anhang A) ab");
    assert!(!lib.contains(&b'\r') && lib.ends_with(b"}\n"), "LF und Zeilenende am Schluss");
}

#[test]
fn weak_hits_are_below_the_threshold_or_not_a_number() {
    assert!(is_weak(0.0), "null Ähnlichkeit ist schwach");
    assert!(is_weak(WEAK_SCORE - 0.01), "knapp unter der Schwelle ist schwach");
    assert!(!is_weak(WEAK_SCORE), "genau auf der Schwelle gilt nicht mehr als schwach (< statt <=)");
    assert!(!is_weak(1.0), "volle Ähnlichkeit ist nicht schwach");
    assert!(is_weak(f32::NAN), "keine Zahl ist schwach: lieber anzeigen als verstecken, aber gekennzeichnet");
    assert!(is_weak(f32::NEG_INFINITY), "minus unendlich liegt unter jeder Schwelle");
    assert_eq!(WEAK_LABEL, "schwach", "einheitlicher Anzeigetext auf allen Seiten");
}

#[test]
fn prefixes_keep_spaces_and_refuse_control_characters_and_overlength() {
    assert_eq!(check_prefix("query_prefix", "search_query: ").unwrap(), "search_query: ", "Leerzeichen am Ende bleibt");
    assert_eq!(check_prefix("query_prefix", "").unwrap(), "", "leer = kein Präfix ist erlaubt");
    let max: String = "ä".repeat(MAX_PREFIX_CHARS);
    assert_eq!(check_prefix("document_prefix", &max).unwrap(), max, "genau die Grenze in Zeichen (nicht Bytes) geht");
    let long = "a".repeat(MAX_PREFIX_CHARS + 1);
    let err = check_prefix("document_prefix", &long).unwrap_err();
    assert!(matches!(err, RagError::Invalid(_)), "zu lang → Invalid: {err:?}");
    assert!(err.message().contains("document_prefix"), "der Satz nennt das Feld: {}", err.message());
    for bad in ["a\nb", "a\tb", "\u{7}x", "x\r"] {
        let err = check_prefix("query_prefix", bad).unwrap_err();
        assert!(matches!(err, RagError::Invalid(_)), "Steuerzeichen in {bad:?} → Invalid");
        assert!(err.message().contains("query_prefix"), "der Satz nennt das Feld: {}", err.message());
    }
}

#[test]
fn prefixes_are_put_in_front_of_the_text_and_empty_means_unchanged() {
    let p = EmbedPrefixes { query: "search_query: ".into(), document: "search_document: ".into() };
    assert_eq!(p.query_text("Psalm"), "search_query: Psalm", "Anfrage mit Anfrage-Präfix");
    assert_eq!(p.document_text("Psalm"), "search_document: Psalm", "Dokument mit Dokument-Präfix");
    let none = EmbedPrefixes::default();
    assert_eq!(none.query_text("Psalm"), "Psalm", "ohne Präfix bleibt der Text wie bisher");
    assert_eq!(none.document_text("Psalm"), "Psalm", "ohne Präfix bleibt der Text wie bisher");
}
