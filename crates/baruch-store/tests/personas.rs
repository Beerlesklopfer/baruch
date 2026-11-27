// SPDX-License-Identifier: AGPL-3.0-or-later
//! Colour per persona in the store (migration 0010, contract Round 7, „proposals“): default from the palette by order,
//! setting and resetting, rejection of unknown personas and invalid colours without a change, guard in the database
//! (CHECK).

use baruch_core::sample;
use baruch_store::{PERSONA_PALETTE, Store, StoreError};

async fn open() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    (dir, store)
}

#[tokio::test]
async fn every_persona_starts_with_the_palette_color_of_its_position() {
    let (_dir, store) = open().await;
    let colors = store.persona_colors().await.unwrap();
    let expected: Vec<String> = sample::work().personas.iter().map(|p| p.handle.clone()).collect();
    let handles: Vec<String> = colors.iter().map(|c| c.handle.clone()).collect();
    assert_eq!(handles, expected, "Reihenfolge wie die Personas (position, id)");
    for (index, c) in colors.iter().enumerate() {
        assert_eq!(c.color, PERSONA_PALETTE[index], "{} hat die Palettenfarbe Nr. {index}", c.handle);
        assert!(!c.chosen, "{}: Vorgabe, nicht gewählt", c.handle);
        assert!(!c.name.is_empty(), "{}: Name mitgeliefert", c.handle);
    }
}

#[tokio::test]
async fn more_personas_than_colors_wrap_around_the_palette() {
    let (_dir, store) = open().await;
    for i in 0..4 {
        sqlx::query(
            "INSERT INTO persona (handle, name, role, model, context_length, position) VALUES (?, ?, 'Probe', 'm', 1, ?)",
        )
        .bind(format!("@gaertner{i}"))
        .bind(format!("Gärtner {i}"))
        .bind(100 + i)
        .execute(store.pool())
        .await
        .unwrap();
    }
    let colors = store.persona_colors().await.unwrap();
    assert_eq!(colors.len(), 10, "6 aus dem Beispiel und 4 neue");
    assert_eq!(colors[8].color, PERSONA_PALETTE[0], "die neunte Persona beginnt die Palette von vorn");
    assert_eq!(colors[9].color, PERSONA_PALETTE[1], "die zehnte bekommt die zweite Farbe");
}

#[tokio::test]
async fn a_chosen_color_is_stored_normalised_and_can_be_reset() {
    let (_dir, store) = open().await;
    let set = store.set_persona_color("@nathan", Some("#C0FFEE")).await.unwrap();
    assert_eq!(
        (set.handle.as_str(), set.color.as_str(), set.chosen),
        ("@nathan", "#c0ffee", true),
        "gesetzt und klein"
    );
    let stored: Option<String> =
        sqlx::query_scalar("SELECT color FROM persona WHERE handle = '@nathan'").fetch_one(store.pool()).await.unwrap();
    assert_eq!(stored.as_deref(), Some("#c0ffee"), "in der Spalte steht #rrggbb klein");
    let short = store.set_persona_color("exeget", Some("#0f0")).await.unwrap();
    assert_eq!((short.handle.as_str(), short.color.as_str()), ("@exeget", "#00ff00"), "Handle ohne @ und #rgb");

    let colors = store.persona_colors().await.unwrap();
    let lektor = colors.iter().find(|c| c.handle == "@lektor").unwrap();
    assert_eq!(lektor.color, PERSONA_PALETTE[0], "andere Personas behalten ihre Vorgabe");

    let reset = store.set_persona_color("@nathan", None).await.unwrap();
    let position = colors.iter().position(|c| c.handle == "@nathan").unwrap();
    assert_eq!(reset.color, PERSONA_PALETTE[position], "zurück zur Vorgabe nach Reihenfolge");
    assert!(!reset.chosen, "nach dem Zurücksetzen nicht mehr gewählt");
}

#[tokio::test]
async fn unknown_personas_and_invalid_colors_change_nothing() {
    let (_dir, store) = open().await;
    let before = store.persona_colors().await.unwrap();
    let unknown = store.set_persona_color("@gibtsnicht", Some("#123456")).await;
    assert!(
        matches!(&unknown, Err(StoreError::UnknownPersona(h)) if h == "@gibtsnicht"),
        "unbekannte Persona: {unknown:?}"
    );
    assert_eq!(unknown.unwrap_err().to_string(), "Persona „@gibtsnicht“ gibt es nicht.", "deutsche Meldung");
    for bad in ["rot", "#12", "#1234567", "", "#abc\"><script>"] {
        let refused = store.set_persona_color("@lektor", Some(bad)).await;
        assert!(
            matches!(&refused, Err(StoreError::Color(m)) if m == "Farbe als #rrggbb angeben."),
            "{bad:?}: {refused:?}"
        );
    }
    assert_eq!(store.persona_colors().await.unwrap(), before, "nichts geändert");
}

#[tokio::test]
async fn the_database_refuses_colors_that_bypass_the_store() {
    let (_dir, store) = open().await;
    for bad in ["#ABCDEF", "red", "#abcdeg", "#abc"] {
        let refused =
            sqlx::query("UPDATE persona SET color = ? WHERE handle = '@lektor'").bind(bad).execute(store.pool()).await;
        assert!(refused.is_err(), "CHECK der Spalte lehnt {bad:?} ab");
    }
    sqlx::query("UPDATE persona SET color = '#abcdef' WHERE handle = '@lektor'").execute(store.pool()).await.unwrap();
    let lektor = store.persona_colors().await.unwrap().remove(0);
    assert_eq!((lektor.color.as_str(), lektor.chosen), ("#abcdef", true), "ein gültiger Wert gilt als gewählt");
}
