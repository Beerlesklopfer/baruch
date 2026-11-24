// SPDX-License-Identifier: AGPL-3.0-or-later
//! Personas im Store, Runde 10: Wahl des Lektors in `store_meta` (fest eine Persona oder automatisch;
//! Vorgabe des Versuchs fest `@lektor`) und eigener Rollen-Prompt je Persona (`persona.prompt`, Migration
//! 0015). Jede abgelehnte Änderung lässt alles, wie es ist.

use baruch_core::LectorChoice;
use baruch_store::{MAX_PROMPT_CHARS, PersonaEdit, Store, StoreError};

async fn open() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    (dir, store)
}

async fn lector(store: &Store) -> String {
    let work = store.default_work().await.unwrap();
    store.load_work(work).await.unwrap().lector().map(|p| p.handle.clone()).unwrap_or_default()
}

#[tokio::test]
async fn the_lector_is_fixed_lektor_by_default_even_when_another_has_more_context() {
    let (_dir, store) = open().await;
    assert_eq!(store.lector_choice().await.unwrap(), LectorChoice::Fixed("@lektor".into()), "Vorgabe des Versuchs");
    store
        .update_persona("@nathan", &PersonaEdit { context_length: Some(500_000), ..Default::default() })
        .await
        .unwrap();
    assert_eq!(lector(&store).await, "@lektor", "fest @lektor: der größere Kontext von @nathan zählt nicht");
    let work = store.default_work().await.unwrap();
    assert_eq!(
        store.load_work(work).await.unwrap().lector_choice,
        LectorChoice::Fixed("@lektor".into()),
        "load_work füllt die Wahl aus store_meta (hier: fehlt → Vorgabe)"
    );
}

#[tokio::test]
async fn a_fixed_lector_that_is_inactive_falls_back_to_the_largest_context() {
    let (_dir, store) = open().await;
    let stored = store.set_lector_choice(&LectorChoice::Fixed("hoerer".into())).await.unwrap();
    assert_eq!(stored, LectorChoice::Fixed("@hoerer".into()), "Handle ohne @ angenommen, mit @ gespeichert");
    assert_eq!(lector(&store).await, "@hoerer", "fest gewählt trotz kleinem Kontext");
    store.update_persona("@hoerer", &PersonaEdit { active: Some(false), ..Default::default() }).await.unwrap();
    assert_eq!(lector(&store).await, "@lektor", "inaktive feste Persona: Regel automatisch (größter Kontext)");
    let value: String =
        sqlx::query_scalar("SELECT value FROM store_meta WHERE key = 'lector'").fetch_one(store.pool()).await.unwrap();
    assert_eq!(value, "@hoerer", "die Wahl bleibt gespeichert, auch wenn sie gerade nicht greift");
}

#[tokio::test]
async fn automatic_takes_the_largest_context_and_the_first_on_a_tie() {
    let (_dir, store) = open().await;
    store.set_lector_choice(&LectorChoice::Auto).await.unwrap();
    assert_eq!(store.lector_choice().await.unwrap(), LectorChoice::Auto, "automatisch gespeichert");
    // Beispiel: @lektor, @schrift und @belege haben 32768 (wie Eliezer Gemma lädt), @exeget weniger.
    store.update_persona("@exeget", &PersonaEdit { context_length: Some(32_768), ..Default::default() }).await.unwrap();
    assert_eq!(lector(&store).await, "@lektor", "Gleichstand mit @lektor (32768): die erste in der Reihenfolge");
    store.update_persona("@exeget", &PersonaEdit { context_length: Some(32_769), ..Default::default() }).await.unwrap();
    assert_eq!(lector(&store).await, "@exeget", "automatisch: größter Kontext gewinnt");
}

#[tokio::test]
async fn an_unknown_fixed_lector_is_refused_and_changes_nothing() {
    let (_dir, store) = open().await;
    store.set_lector_choice(&LectorChoice::Auto).await.unwrap();
    let refused = store.set_lector_choice(&LectorChoice::Fixed("@niemand".into())).await;
    assert!(matches!(refused, Err(StoreError::UnknownPersona(ref h)) if h == "@niemand"), "{refused:?}");
    assert_eq!(store.lector_choice().await.unwrap(), LectorChoice::Auto, "die alte Wahl bleibt");
}

#[tokio::test]
async fn a_role_prompt_is_saved_trimmed_reset_and_bounded() {
    let (_dir, store) = open().await;
    assert_eq!(store.persona_prompt("@lektor").await.unwrap(), None, "Vorgabe: kein eigener Prompt");
    let row = store.set_persona_prompt("lektor", Some("  Du bist streng.\r\nKritisch: immer.  ")).await.unwrap();
    assert_eq!(row.prompt.as_deref(), Some("Du bist streng.\nKritisch: immer."), "getrimmt, Zeilenenden \\n");
    assert_eq!(store.persona_prompt("@lektor").await.unwrap(), row.prompt, "gespeichert");
    let listed = store.persona_list().await.unwrap();
    assert_eq!(listed[0].prompt, row.prompt, "persona_list liest den Prompt mit");

    let longest = "ä".repeat(MAX_PROMPT_CHARS);
    let row = store.set_persona_prompt("@lektor", Some(&longest)).await.unwrap();
    assert_eq!(row.prompt.as_deref().map(|p| p.chars().count()), Some(MAX_PROMPT_CHARS), "8000 Zeichen (nicht Bytes)");
    for (bad, why) in [("   ".to_owned(), "nur Leerraum"), ("ä".repeat(MAX_PROMPT_CHARS + 1), "über 8000 Zeichen")] {
        let refused = store.set_persona_prompt("@lektor", Some(&bad)).await;
        assert!(matches!(refused, Err(StoreError::Refused(_))), "{why} → Refused: {refused:?}");
    }
    assert_eq!(store.persona_prompt("@lektor").await.unwrap().map(|p| p.len()), Some(longest.len()), "unverändert");
    let unknown = store.set_persona_prompt("@niemand", Some("x")).await;
    assert!(matches!(unknown, Err(StoreError::UnknownPersona(_))), "unbekannte Persona: {unknown:?}");

    let row = store.set_persona_prompt("@lektor", None).await.unwrap();
    assert_eq!(row.prompt, None, "zurück zur Vorgabe");
    assert_eq!(store.persona_prompt("@lektor").await.unwrap(), None, "NULL in der Tabelle");
}
