// SPDX-License-Identifier: AGPL-3.0-or-later
//! Tokens für den MCP-Zugang im Store (Migration 0016, Runde 10): Klartext genau einmal, in SQLite nur
//! der SHA-256-Hash, Prüfen, Widerrufen (idempotent), Namen eindeutig unter gültigen Tokens.

use baruch_store::{MCP_TOKEN_PREFIX, Store, StoreError};

async fn open() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    (dir, store)
}

/// Alle Bytes der Datenbankdateien (Hauptdatei und WAL) als Text (verlustbehaftet).
fn database_bytes(dir: &std::path::Path) -> String {
    let mut all = Vec::new();
    for name in ["baruch.sqlite", "baruch.sqlite-wal"] {
        if let Ok(bytes) = std::fs::read(dir.join(name)) {
            all.extend(bytes);
        }
    }
    String::from_utf8_lossy(&all).into_owned()
}

#[tokio::test]
async fn a_new_token_is_returned_once_and_only_its_hash_is_stored() {
    let (dir, store) = open().await;
    let (token, plain) = store.mcp_token_create("  Arbeitszimmer  ").await.unwrap();
    assert!(plain.starts_with(MCP_TOKEN_PREFIX), "Klartext beginnt mit baruch_: {}", &plain[..12]);
    assert_eq!(plain.len(), MCP_TOKEN_PREFIX.len() + 43, "32 Bytes Base64url ohne Füllung = 43 Zeichen");
    assert!(
        plain[MCP_TOKEN_PREFIX.len()..].bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
        "Base64url-Alphabet"
    );
    assert_eq!(token.name, "Arbeitszimmer", "Name getrimmt");
    assert_eq!(token.prefix, plain[..12], "prefix = erste 12 Zeichen");
    assert!(token.revoked.is_none() && token.last_used.is_none(), "neu: weder widerrufen noch benutzt");

    let json = serde_json::to_string(&token).unwrap();
    assert!(!json.contains(&plain) && !json.contains("hash"), "McpToken trägt weder Klartext noch Hash: {json}");
    let listed = serde_json::to_string(&store.mcp_token_list().await.unwrap()).unwrap();
    assert!(!listed.contains(&plain), "die Liste zeigt den Klartext nicht");

    let stored: String = sqlx::query_scalar("SELECT hash FROM mcp_token WHERE id = ?")
        .bind(token.id)
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(stored.len(), 64, "SHA-256 hex in der Spalte hash");
    assert_ne!(stored, plain, "kein Klartext in der Spalte hash");
    sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)").execute(store.pool()).await.unwrap();
    assert!(!database_bytes(dir.path()).contains(&plain), "der Klartext steht nirgends in der Datenbankdatei");
    assert!(database_bytes(dir.path()).contains(&stored), "der Hash steht in der Datei (Gegenprobe)");

    let (_, second) = store.mcp_token_create("Laptop").await.unwrap();
    assert_ne!(second, plain, "jeder Token ist neu gewürfelt");
}

#[tokio::test]
async fn verify_accepts_only_valid_tokens_and_notes_the_use() {
    let (_dir, store) = open().await;
    let (token, plain) = store.mcp_token_create("Laptop").await.unwrap();
    let seen = store.mcp_token_verify(&plain).await.unwrap().expect("gültiger Token");
    assert_eq!(seen.id, token.id, "derselbe Token");
    let first_use = seen.last_used.clone().expect("last_used nach der ersten Prüfung gesetzt");
    let again = store.mcp_token_verify(&plain).await.unwrap().unwrap();
    assert_eq!(again.last_used.as_deref(), Some(first_use.as_str()), "höchstens einmal je Minute geschrieben");

    for wrong in ["", "baruch_", "baruch_falsch", "kein-präfix", &plain[..plain.len() - 1], &format!("{plain}x")] {
        assert!(store.mcp_token_verify(wrong).await.unwrap().is_none(), "„{wrong}“ ist kein gültiger Token");
    }

    let revoked = store.mcp_token_revoke(token.id).await.unwrap();
    let when = revoked.revoked.clone().expect("revoked gesetzt");
    assert!(store.mcp_token_verify(&plain).await.unwrap().is_none(), "widerrufen gilt nicht mehr");
    let twice = store.mcp_token_revoke(token.id).await.unwrap();
    assert_eq!(twice.revoked.as_deref(), Some(when.as_str()), "zweites Widerrufen ändert nichts (idempotent)");
    assert_eq!(store.mcp_token_list().await.unwrap().len(), 1, "die Zeile bleibt nach dem Widerruf");
    assert!(matches!(store.mcp_token_revoke(9999).await, Err(StoreError::Refused(_))), "unbekannter Token → Refused");
}

#[tokio::test]
async fn names_are_unique_among_valid_tokens_and_bounded() {
    let (_dir, store) = open().await;
    let (first, _) = store.mcp_token_create("Laptop").await.unwrap();
    let doubled = store.mcp_token_create(" Laptop ").await;
    assert!(matches!(doubled, Err(StoreError::Refused(_))), "gleicher Name unter gültigen: {doubled:?}");
    store.mcp_token_revoke(first.id).await.unwrap();
    let (reused, _) = store.mcp_token_create("Laptop").await.expect("nach dem Widerruf ist der Name frei");
    assert_ne!(reused.id, first.id, "neuer Eintrag");
    for bad in ["", "   ", "zwei\nZeilen", &"x".repeat(65)] {
        assert!(matches!(store.mcp_token_create(bad).await, Err(StoreError::Refused(_))), "Name „{bad}“ abgelehnt");
    }
    assert!(store.mcp_token_create(&"ü".repeat(64)).await.is_ok(), "64 Zeichen (auch mehrere Bytes je Zeichen) gehen");
    assert_eq!(store.mcp_token_list().await.unwrap().len(), 3, "nur die gültigen Anlagen zählen");
}
