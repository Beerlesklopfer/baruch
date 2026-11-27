// SPDX-License-Identifier: AGPL-3.0-or-later
//! Personas in the store (Round 9, settings „Assistenten“): `Store::persona_list` reads all fields from the table
//! (without a work), `Store::update_persona` changes role, model, context length and active atomically, checks the
//! limits and leaves everything as it is on every error.

use baruch_core::sample;
use baruch_store::{PERSONA_PALETTE, PersonaEdit, Store, StoreError};

async fn open() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    (dir, store)
}

#[tokio::test]
async fn the_persona_list_reads_every_field_from_the_table_without_a_work() {
    let (_dir, store) = open().await;
    let rows = store.persona_list().await.unwrap();
    let sample = sample::work().personas;
    assert_eq!(rows.len(), sample.len(), "alle gesäten Personas: {rows:?}");
    for (index, (row, p)) in rows.iter().zip(&sample).enumerate() {
        assert_eq!(
            (&row.handle, &row.name, &row.role, &row.model, row.context_length, row.active),
            (&p.handle, &p.name, &p.role, &p.model, p.context_length, p.active),
            "Felder der Persona {} wie im Beispielwerk gesät",
            p.handle
        );
        assert_eq!(row.color, PERSONA_PALETTE[index], "{}: Vorgabefarbe nach Reihenfolge", row.handle);
        assert!(!row.chosen, "{}: keine eigene Farbe gewählt", row.handle);
    }
    // Without works the list stays complete: personas are global.
    sqlx::query("UPDATE persona SET context_length = 4096 WHERE handle = '@exeget'")
        .execute(store.pool())
        .await
        .unwrap();
    let again = store.persona_list().await.unwrap();
    assert_eq!(again[1].context_length, 4096, "die Liste liest die Tabelle, nicht das Beispielwerk");
}

#[tokio::test]
async fn updating_a_persona_changes_only_the_given_fields() {
    let (_dir, store) = open().await;
    let before = store.persona_list().await.unwrap();
    let edit = PersonaEdit {
        role: Some("  Liest gegen  ".into()),
        model: None,
        context_length: Some(131_072),
        active: Some(false),
    };
    let row = store.update_persona("exeget", &edit).await.unwrap();
    assert_eq!(row.handle, "@exeget", "Handle ohne @ gefunden");
    assert_eq!(row.role, "Liest gegen", "Rolle getrimmt gespeichert");
    assert_eq!(row.model, before[1].model, "Modell nicht genannt → bleibt");
    assert_eq!((row.context_length, row.active), (131_072, false), "Kontext und aktiv geändert");
    let after = store.persona_list().await.unwrap();
    assert_eq!(after[1], row, "die Antwort zeigt den gespeicherten Stand");
    for (i, (a, b)) in after.iter().zip(&before).enumerate() {
        if i != 1 {
            assert_eq!(a, b, "andere Personas unverändert: {}", a.handle);
        }
    }
    let empty = store.update_persona("@exeget", &PersonaEdit::default()).await.unwrap();
    assert_eq!(empty, row, "eine leere Änderung ändert nichts");
}

#[tokio::test]
async fn limits_and_unknown_personas_are_refused_without_change() {
    let (_dir, store) = open().await;
    let before = store.persona_list().await.unwrap();
    let long = "x".repeat(201);
    for (edit, why) in [
        (PersonaEdit { role: Some("   ".into()), ..Default::default() }, "leere Rolle"),
        (PersonaEdit { model: Some(String::new()), ..Default::default() }, "leeres Modell"),
        (PersonaEdit { role: Some(long.clone()), ..Default::default() }, "Rolle über 200 Zeichen"),
        (PersonaEdit { model: Some(long.clone()), ..Default::default() }, "Modell über 200 Zeichen"),
        (PersonaEdit { context_length: Some(0), ..Default::default() }, "Kontext 0"),
        (PersonaEdit { context_length: Some(10_000_001), ..Default::default() }, "Kontext über 10 Mio."),
    ] {
        let refused = store.update_persona("@lektor", &edit).await;
        assert!(matches!(refused, Err(StoreError::Refused(_))), "{why} abgelehnt: {refused:?}");
    }
    let ok = PersonaEdit {
        role: Some("ä".repeat(200)),
        model: Some("m".into()),
        context_length: Some(10_000_000),
        ..Default::default()
    };
    assert!(ok.check().is_ok(), "200 Zeichen (auch Umlaute) und 10 Mio. Tokens sind gerade noch erlaubt");
    let unknown = store.update_persona("@niemand", &PersonaEdit { active: Some(false), ..Default::default() }).await;
    assert!(matches!(unknown, Err(StoreError::UnknownPersona(ref h)) if h == "@niemand"), "unbekannt: {unknown:?}");
    assert_eq!(store.persona_list().await.unwrap(), before, "abgelehnte Änderungen lassen alles, wie es ist");
}

#[tokio::test]
async fn the_lector_follows_context_and_active_in_the_loaded_work() {
    let (_dir, store) = open().await;
    let work = store.default_work().await.unwrap();
    assert_eq!(store.load_work(work).await.unwrap().lector().unwrap().handle, "@lektor", "Vorgabe: fest @lektor");
    // Round 10: the lector follows the context only in the choice „automatisch“.
    store.set_lector_choice(&baruch_core::LectorChoice::Auto).await.unwrap();
    store
        .update_persona("@nathan", &PersonaEdit { context_length: Some(200_000), ..Default::default() })
        .await
        .unwrap();
    assert_eq!(
        store.load_work(work).await.unwrap().lector().unwrap().handle,
        "@nathan",
        "größerer Kontext macht Nathan zum Lektor (die Regel steht in baruch_core::Work::lector)"
    );
    store.update_persona("@nathan", &PersonaEdit { active: Some(false), ..Default::default() }).await.unwrap();
    assert_eq!(
        store.load_work(work).await.unwrap().lector().unwrap().handle,
        "@lektor",
        "inaktiv zählt nicht: wieder der Lektor"
    );
}
