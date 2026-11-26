// SPDX-License-Identifier: AGPL-3.0-or-later
//! Migration 0053 (Runde 35 „Konten und Rollen“, Vertrag §S1.2: `account`, `account_session`, `login_attempt`) auf
//! einer nachgebauten Datenbank im Stand von development vor Runde 35 (Store bis 0052, Bibliothek vollständig) mit
//! frei erfundenen Daten (ein Werk mit zwei Kapiteln, Beiträge, Einfälle, Einträge in `store_meta`).
//!
//! Belegt: 0053 legt nur an (nicht in `FOREIGN_KEYS_OFF`); jede Zeile jeder vorhandenen Tabelle bleibt byte-gleich,
//! kein vorhandenes Schemaobjekt ändert sich; neu sind genau die drei Tabellen samt Index, alle leer; die CHECKs
//! lehnen ab, was der Vertrag verbietet (Klartext statt PHC-Text, unbekannte Kontoart, doppelter Name oder Slug,
//! Token-Hash falscher Länge); die Datei ist gegenüber der Schnittstelle S1 unverändert; der Store arbeitet danach
//! auf der migrierten Datenbank (Konten, Sitzungen, Sperre, Schalter) – ohne Konten zählt er 0.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::Path;
use std::str::FromStr;

use baruch_store::{NewAccount, Store, StoreError};
use sha2::{Digest, Sha256};
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

const BOOK: &str = "#set document(title: \"Die Mühle am Hang\", author: \"Arvid Lohse\")\n\n\
= Mahlgang\n\nDer Stein dreht sich schwer.\n\n= Wehrgraben\n\nDas Wasser kommt spät im Jahr.\n";

/// SHA-256 der Datei `0053_accounts.sql`, wie die Schnittstelle S1 (Commit e56c3f3) sie festgelegt hat.
const S1_SHA256: &str = "178570808ad20234925e7d50f17ed51aee9af6b8cb8ad734f137577cf30a5faf";

/// Ein Hash-Text, wie ihn die Spalte verlangt (kein echtes Passwort dahinter).
const PHC: &str = "$argon2id$test";

async fn raw_pool(dir: &Path) -> SqlitePool {
    let options = SqliteConnectOptions::from_str("sqlite:")
        .unwrap()
        .filename(dir.join("baruch.sqlite"))
        .create_if_missing(true)
        .foreign_keys(true);
    SqlitePoolOptions::new().max_connections(1).connect_with(options).await.unwrap()
}

fn store_migrator(keep: impl Fn(i64) -> bool) -> Migrator {
    let list: Vec<_> = baruch_store::MIGRATOR.iter().filter(|m| keep(m.version)).cloned().collect();
    Migrator { migrations: Cow::Owned(list), ignore_missing: true, ..Migrator::DEFAULT }
}

async fn exec(pool: &SqlitePool, sql: &str) {
    sqlx::query(sql).execute(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"));
}

async fn scalar(pool: &SqlitePool, sql: &str) -> i64 {
    sqlx::query_scalar(sql).fetch_one(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"))
}

async fn rejected(pool: &SqlitePool, sql: &str) -> bool {
    sqlx::query(sql).execute(pool).await.is_err()
}

/// Store bis 0052 (0040 wie in `baruch_store::migrate` mit ausgeschalteten Fremdschlüsseln) und die Bibliothek: der
/// Stand von development vor Runde 35.
async fn migrate_before(pool: &SqlitePool) {
    assert!(
        baruch_store::MIGRATOR.iter().any(|m| m.version == 53),
        "die Migration 0053 gehört zum Store (sonst prüfte dieser Test nichts)"
    );
    assert!(
        !baruch_store::FOREIGN_KEYS_OFF.contains(&53),
        "0053 legt nur Tabellen an und baut keine um: nicht in FOREIGN_KEYS_OFF"
    );
    store_migrator(|v| v < 40).run(pool).await.unwrap();
    {
        let mut conn = pool.acquire().await.unwrap();
        sqlx::query("PRAGMA foreign_keys = OFF").execute(&mut *conn).await.unwrap();
        store_migrator(|v| v == 40).run(&mut *conn).await.unwrap();
        sqlx::query("PRAGMA foreign_keys = ON").execute(&mut *conn).await.unwrap();
    }
    store_migrator(|v| (41..=52).contains(&v)).run(pool).await.unwrap();
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../baruch-library/migrations");
    let mut library = Migrator::new(dir).await.unwrap();
    library.ignore_missing = true;
    library.run(pool).await.unwrap();
    assert_eq!(
        scalar(pool, "SELECT max(version) FROM _sqlx_migrations WHERE version < 100").await,
        52,
        "Stand davor: Store bis 0052"
    );
}

/// Ein Werk mit zwei Kapiteln, zwei Beiträgen, drei Einfällen (einer gelöscht, einer ohne Werk) und zwei eigenen
/// Einträgen in `store_meta`, direkt in SQLite.
async fn insert_rows(pool: &SqlitePool) {
    let doc = baruch_typst::parse(BOOK).unwrap();
    let work: i64 = sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, source_name, source_sha256, title_key) \
         VALUES ('die-muehle-am-hang', 'book', 'Die Mühle am Hang', 'Arvid Lohse', ?, 'muehle.typ', 'sha-muehle', ?) \
         RETURNING id",
    )
    .bind(&doc.template)
    .bind(baruch_store::title_key("Die Mühle am Hang"))
    .fetch_one(pool)
    .await
    .unwrap();
    let mut nodes = Vec::new();
    for (seq, heading) in doc.headings.iter().enumerate() {
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO node (work_id, seq, parent_seq, position, number, title, kind) \
             VALUES (?, ?, NULL, ?, ?, ?, 'chapter') RETURNING id",
        )
        .bind(work)
        .bind(seq as i64)
        .bind(seq as i64)
        .bind((seq + 1).to_string())
        .bind(&heading.title)
        .fetch_one(pool)
        .await
        .unwrap();
        nodes.push(id);
    }
    assert_eq!(nodes.len(), 2, "Vorbedingung: zwei Kapitel");
    for (position, (author, body)) in
        [("Autor", "Hier fehlt noch der Müller."), ("@lektor", "Der zweite Satz trägt den Absatz.")].iter().enumerate()
    {
        sqlx::query("INSERT INTO post (node_id, position, author, body) VALUES (?, ?, ?, ?)")
            .bind(nodes[0])
            .bind(position as i64)
            .bind(author)
            .bind(body)
            .execute(pool)
            .await
            .unwrap();
    }
    for (work_id, node, note, deleted) in [
        (Some(work), Some(nodes[0]), "Mehlstaub im Gegenlicht.", None),
        (Some(work), None, "Verworfen: der Esel.", Some("2026-04-02T09:00:00.000Z")),
        (None, None, "Ohne Werk: ein Kranich.", None),
    ] {
        sqlx::query(
            "INSERT INTO idea (work_id, node_id, note, origin_kind, origin_name, status, created, updated, deleted) \
             VALUES (?, ?, ?, 'human', 'Autor', 'open', '2026-04-01T08:00:00.000Z', '2026-04-01T08:00:00.000Z', ?)",
        )
        .bind(work_id)
        .bind(node)
        .bind(note)
        .bind(deleted)
        .execute(pool)
        .await
        .unwrap();
    }
    // Eine Datenbank im Betrieb hat der Store schon einmal geöffnet: Die Markierung „gesät“ steht.
    exec(pool, "INSERT OR IGNORE INTO store_meta (key, value) VALUES ('seeded', '1')").await;
    exec(pool, "INSERT INTO store_meta (key, value) VALUES ('instance_name', 'Mühlenrechner')").await;
    exec(pool, "INSERT INTO store_meta (key, value) VALUES ('probe.r35', 'bleibt')").await;
}

async fn tables(pool: &SqlitePool) -> BTreeMap<String, Vec<String>> {
    let names: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND sql NOT LIKE 'CREATE VIRTUAL%' ORDER BY name",
    )
    .fetch_all(pool)
    .await
    .unwrap();
    let mut out = BTreeMap::new();
    for table in names {
        let cols: Vec<String> = sqlx::query_scalar("SELECT name FROM pragma_table_info(?) ORDER BY cid")
            .bind(&table)
            .fetch_all(pool)
            .await
            .unwrap();
        out.insert(table, cols);
    }
    out
}

/// Alle Schemaobjekte: Name → (Typ, Tabelle, SQL).
async fn schema(pool: &SqlitePool) -> BTreeMap<String, (String, String, Option<String>)> {
    let rows =
        sqlx::query("SELECT name, type, tbl_name, sql FROM sqlite_master ORDER BY name").fetch_all(pool).await.unwrap();
    rows.into_iter().map(|r| (r.get(0), (r.get(1), r.get(2), r.get(3)))).collect()
}

/// Inhalt jeder Tabelle in den Spalten aus `layout`, Werte per `quote()` (Typ und Bytes sichtbar).
async fn contents(pool: &SqlitePool, layout: &BTreeMap<String, Vec<String>>) -> BTreeMap<String, Vec<String>> {
    let mut out = BTreeMap::new();
    for (table, cols) in layout {
        let select = cols.iter().map(|c| format!("quote(\"{c}\")")).collect::<Vec<_>>().join(" || char(31) || ");
        let mut rows: Vec<String> =
            sqlx::query_scalar(&format!("SELECT {select} FROM \"{table}\"")).fetch_all(pool).await.unwrap();
        rows.sort();
        out.insert(table.clone(), rows);
    }
    out
}

#[test]
fn the_migration_file_is_unchanged_since_the_interface_s1() {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations/0053_accounts.sql");
    let bytes = std::fs::read(&file).unwrap();
    let digest: String = Sha256::digest(&bytes).iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(
        digest, S1_SHA256,
        "0053_accounts.sql ist nach S1 unveränderlich (Vertrag §S1.2) – eine geänderte Datei bräche jede Datenbank, \
         die sie schon ausgeführt hat"
    );
}

#[tokio::test]
async fn migration_0053_keeps_every_row_and_adds_three_empty_tables() {
    let dir = tempfile::tempdir().unwrap();
    let pool = raw_pool(dir.path()).await;
    migrate_before(&pool).await;
    insert_rows(&pool).await;
    let layout = tables(&pool).await;
    let schema_before = schema(&pool).await;
    let before = contents(&pool, &layout).await;
    assert_eq!(scalar(&pool, "SELECT count(*) FROM work").await, 1, "Vorbedingung: ein Werk");
    assert_eq!(scalar(&pool, "SELECT count(*) FROM post").await, 2, "Vorbedingung: zwei Beiträge");
    assert_eq!(scalar(&pool, "SELECT count(*) FROM idea").await, 3, "Vorbedingung: drei Einfälle (einer ohne Werk)");
    assert!(
        rejected(&pool, "SELECT count(*) FROM account").await,
        "vor 0053 gibt es die Tabelle account nicht (sonst prüfte dieser Test nichts)"
    );

    // Nur 0053, wie `baruch_store::migrate` sie ausführt (Fremdschlüssel bleiben an).
    store_migrator(|v| v == 53).run(&pool).await.expect("0053 läuft auf dem Stand von development");

    // Daten: jede Zeile jeder vorhandenen Tabelle byte-gleich, gleiche Spalten in gleicher Reihenfolge.
    let layout_after = tables(&pool).await;
    let after = contents(&pool, &layout).await;
    for (table, old) in &before {
        assert_eq!(layout_after.get(table), layout.get(table), "{table}: Spalten unverändert");
        if table != "_sqlx_migrations" {
            assert_eq!(after.get(table), Some(old), "Tabelle {table}: alle Zeilen unverändert");
        }
    }

    // Schema: kein vorhandenes Objekt ändert sich; neu sind genau die drei Tabellen, ihr Index und die Indizes, die
    // SQLite für PRIMARY KEY und UNIQUE selbst anlegt.
    let schema_after = schema(&pool).await;
    for (name, old) in &schema_before {
        assert_eq!(schema_after.get(name), Some(old), "{name}: unverändert");
    }
    let new: Vec<(&str, &str, &str)> = schema_after
        .iter()
        .filter(|(name, _)| !schema_before.contains_key(*name))
        .map(|(name, (kind, table, _))| (name.as_str(), kind.as_str(), table.as_str()))
        .collect();
    assert_eq!(
        new,
        [
            ("account", "table", "account"),
            ("account_session", "table", "account_session"),
            ("account_session_account", "index", "account_session"),
            ("login_attempt", "table", "login_attempt"),
            ("sqlite_autoindex_account_1", "index", "account"),
            ("sqlite_autoindex_account_2", "index", "account"),
            ("sqlite_autoindex_account_session_1", "index", "account_session"),
            ("sqlite_autoindex_login_attempt_1", "index", "login_attempt"),
        ],
        "neu sind genau die Tabellen und Indizes der Konten"
    );
    for table in ["account", "account_session", "login_attempt"] {
        assert_eq!(scalar(&pool, &format!("SELECT count(*) FROM {table}")).await, 0, "{table} ist nach 0053 leer");
    }
    assert_eq!(
        layout_after["account"],
        [
            "id",
            "name",
            "name_key",
            "slug",
            "display",
            "kind",
            "password",
            "active",
            "created",
            "password_changed",
            "last_login"
        ],
        "account: Spalten wörtlich nach Vertrag §S1.2"
    );
    let violations = sqlx::query("PRAGMA foreign_key_check").fetch_all(&pool).await.unwrap();
    assert!(violations.is_empty(), "kein verletzter Fremdschlüssel nach 0053");

    // CHECKs und Eindeutigkeit.
    let insert = |name: &str, key: &str, slug: &str, kind: &str, password: &str| {
        format!(
            "INSERT INTO account (name, name_key, slug, kind, password) \
             VALUES ('{name}', '{key}', '{slug}', '{kind}', '{password}')"
        )
    };
    assert!(
        rejected(&pool, &insert("Erika", "erika", "erika", "author", "klartext-passwort")).await,
        "ein Klartext in der Spalte password ist abgelehnt (nur $argon2id$…)"
    );
    assert!(
        rejected(&pool, &insert("Erika", "erika", "erika", "admin", PHC)).await,
        "nur die Kontoarten author und lektor"
    );
    assert!(rejected(&pool, &insert("   ", "leer", "leer", "lektor", PHC)).await, "ein Name aus Leerraum ist keiner");
    assert!(
        rejected(&pool, &insert(&"n".repeat(41), "lang", "lang", "lektor", PHC)).await,
        "ein Name hat höchstens 40 Zeichen"
    );
    exec(&pool, &insert("Erika", "erika", "erika", "author", PHC)).await;
    assert!(
        rejected(&pool, &insert("ERIKA", "erika", "erika-2", "lektor", PHC)).await,
        "derselbe Namensschlüssel ein zweites Mal ist abgelehnt"
    );
    assert!(
        rejected(&pool, &insert("Erika B.", "erika b.", "erika", "lektor", PHC)).await,
        "derselbe Slug (Handle) ein zweites Mal ist abgelehnt"
    );
    assert_eq!(
        scalar(&pool, "SELECT active FROM account WHERE name = 'Erika'").await,
        1,
        "ein neues Konto ist aktiv (Vorgabe)"
    );
    assert!(
        rejected(&pool, "INSERT INTO account_session (token_hash, account_id) VALUES ('zu-kurz', 1)").await,
        "ein Token-Hash hat genau 64 Zeichen (SHA-256 hex) – ein Token im Klartext passt nicht hinein"
    );
    let hash = "a".repeat(64);
    assert!(
        rejected(&pool, &format!("INSERT INTO account_session (token_hash, account_id) VALUES ('{hash}', 999)")).await,
        "eine Sitzung braucht ein vorhandenes Konto"
    );
    exec(&pool, &format!("INSERT INTO account_session (token_hash, account_id) VALUES ('{hash}', 1)")).await;
    exec(&pool, "DELETE FROM account WHERE id = 1").await;
    assert_eq!(
        scalar(&pool, "SELECT count(*) FROM account_session").await,
        0,
        "mit dem Konto verschwinden seine Sitzungen (ON DELETE CASCADE)"
    );
    assert!(
        rejected(&pool, "INSERT INTO login_attempt (scope, failures) VALUES ('konto:x', -1)").await,
        "Fehlversuche sind nie negativ"
    );
    assert!(rejected(&pool, "INSERT INTO login_attempt (scope) VALUES ('')").await, "ein Bereich hat einen Namen");
    exec(&pool, "DELETE FROM sqlite_sequence WHERE name = 'account'").await;
    pool.close().await;

    // Der Store öffnet die migrierte Datenbank (führt die späteren Migrationen aus) und findet die alten Daten.
    let store = Store::open(dir.path()).await.expect("Store öffnet die migrierte Datenbank");
    assert_eq!(store.account_count().await.unwrap(), 0, "ohne Konten zählt der Store 0 – Baruch bleibt offen");
    assert!(store.account_list().await.unwrap().is_empty(), "die Liste der Konten ist leer");
    assert!(!store.auth_local_open().await.unwrap(), "der Schalter „eigener Rechner“ steht in der Vorgabe auf aus");
    let titles: Vec<String> =
        sqlx::query_scalar("SELECT title FROM work ORDER BY id").fetch_all(store.pool()).await.unwrap();
    assert_eq!(titles, ["Die Mühle am Hang"], "das alte Werk ist da, und kein Beispielwerk kam dazu");
    let meta: Option<String> = sqlx::query_scalar("SELECT value FROM store_meta WHERE key = 'probe.r35'")
        .fetch_optional(store.pool())
        .await
        .unwrap();
    assert_eq!(meta.as_deref(), Some("bleibt"), "ein fremder Eintrag in store_meta bleibt");
}

async fn open_store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().unwrap();
    let store = Store::open(dir.path()).await.unwrap();
    (dir, store)
}

fn new_account<'a>(name: &'a str, slug: &'a str, kind: &'a str) -> NewAccount<'a> {
    NewAccount { name, slug, display: "", kind, password_phc: PHC }
}

fn refused(result: Result<impl Sized, StoreError>) -> &'static str {
    match result {
        Err(StoreError::Refused(text)) => text,
        Err(other) => panic!("fachliche Ablehnung erwartet, bekommen: {other}"),
        Ok(_) => panic!("fachliche Ablehnung erwartet, aber es ging"),
    }
}

#[tokio::test]
async fn the_first_account_exists_only_once_even_when_two_arrive_together() {
    let (_dir, store) = open_store().await;
    assert_eq!(
        refused(store.account_create(&new_account("Lea", "lea", "lektor")).await),
        Store::ACCOUNT_NEEDS_AUTHOR,
        "ohne Autor-Konto entsteht kein Lektor – sonst gäbe es Konten, aber niemanden mit vollen Rechten"
    );
    let first = new_account("Erika", "erika", "author");
    let second = new_account("Jonas", "jonas", "author");
    let (a, b) = tokio::join!(store.account_create_first(&first), store.account_create_first(&second));
    assert_eq!(
        a.is_ok() as u8 + b.is_ok() as u8,
        1,
        "von zwei gleichzeitigen Einrichtungen gewinnt genau eine (Prüfung und Einfügen sind eine Anweisung)"
    );
    let loser = if a.is_ok() { b } else { a };
    assert_eq!(refused(loser), Store::ACCOUNT_NOT_FIRST, "die andere erfährt, dass es schon ein Konto gibt");
    assert_eq!(store.account_count().await.unwrap(), 1, "es gibt genau ein Konto");
    let author = store.account_list().await.unwrap().remove(0);
    assert_eq!(author.kind, "author", "das erste Konto ist der Autor");
    assert!(author.active && author.last_login.is_none(), "aktiv und noch nie angemeldet");
    assert_eq!(
        refused(store.account_create_first(&new_account("Später", "spaeter", "author")).await),
        Store::ACCOUNT_NOT_FIRST,
        "auch später gibt es kein zweites „erstes“ Konto"
    );
}

#[tokio::test]
async fn names_and_handles_are_unique_and_the_last_active_author_stays() {
    let (_dir, store) = open_store().await;
    let author =
        store.account_create_first(&new_account(" Erika  Beispiel ", "erika-beispiel", "author")).await.unwrap();
    assert_eq!(author.name, "Erika  Beispiel", "der Name steht ohne Rand in der Datenbank");
    assert_eq!(Store::account_name_key(" Erika  Beispiel "), "erika beispiel", "Schlüssel: klein, Leerraum einfach");
    let lektor = store.account_create(&new_account("Lea", "lea", "lektor")).await.unwrap();
    assert_eq!(
        refused(store.account_create(&new_account("erika beispiel", "anders", "lektor")).await),
        Store::ACCOUNT_TAKEN,
        "derselbe Name in anderer Schreibweise ist vergeben"
    );
    assert_eq!(
        refused(store.account_create(&new_account("Léa", "lea", "lektor")).await),
        Store::ACCOUNT_TAKEN,
        "ein anderer Name mit demselben Slug hätte dasselbe Handle – vergeben"
    );
    let found = store.account_for_login("ERIKA BEISPIEL").await.unwrap().expect("Anmelden findet den Namen");
    assert_eq!(
        (found.0.id, found.1.as_str()),
        (author.id, PHC),
        "Konto und sein PHC-Text, ohne Rücksicht auf Groß/Klein"
    );
    assert!(store.account_for_login("niemand").await.unwrap().is_none(), "ein unbekannter Name hat kein Konto");

    assert_eq!(
        refused(store.account_set_active(author.id, false).await),
        Store::ACCOUNT_LAST_AUTHOR,
        "das letzte aktive Autor-Konto lässt sich nicht sperren – sonst käme niemand mehr hinein"
    );
    assert!(store.account_get(author.id).await.unwrap().unwrap().active, "es ist weiter aktiv");
    assert!(store.account_set_active(lektor.id, false).await.unwrap(), "ein Lektor lässt sich sperren");
    assert!(!store.account_get(lektor.id).await.unwrap().unwrap().active, "er ist gesperrt");
    assert_eq!(
        refused(store.account_create(&new_account("Lea", "lea-2", "lektor")).await),
        Store::ACCOUNT_TAKEN,
        "auch ein gesperrtes Konto behält Namen und Handle"
    );
    assert!(store.account_set_active(lektor.id, true).await.unwrap(), "und wieder entsperren");
    assert!(!store.account_set_active(999, false).await.unwrap(), "ein unbekanntes Konto meldet false");
    // Mit einem zweiten aktiven Autor lässt sich der erste sperren, der letzte dann wieder nicht.
    let second = store.account_create(&new_account("Jonas", "jonas", "author")).await.unwrap();
    assert!(store.account_set_active(author.id, false).await.unwrap(), "mit zwei Autoren darf einer gesperrt werden");
    assert_eq!(
        refused(store.account_set_active(second.id, false).await),
        Store::ACCOUNT_LAST_AUTHOR,
        "der nun letzte aktive Autor bleibt"
    );
}

#[tokio::test]
async fn sessions_store_only_a_hash_expire_and_end_with_password_or_lock() {
    let (_dir, store) = open_store().await;
    let author = store.account_create_first(&new_account("Erika", "erika", "author")).await.unwrap();
    let token = "ein-token-das-nie-in-der-datenbank-stehen-darf";
    let hash = Store::session_token_hash(token);
    assert_eq!(hash.len(), 64, "SHA-256 als 64 Hex-Zeichen");
    assert_eq!(hash, Store::session_token_hash(token), "derselbe Token ergibt denselben Hash");
    store.session_create(&hash, author.id).await.unwrap();
    let dump: Vec<String> = sqlx::query_scalar("SELECT token_hash || '|' || account_id FROM account_session")
        .fetch_all(store.pool())
        .await
        .unwrap();
    assert_eq!(dump, [format!("{hash}|{}", author.id)], "gespeichert ist der Hash");
    assert!(!dump[0].contains(token), "das Token selbst steht nicht in der Datenbank");
    let (account, idle) = store.session_account(&hash).await.unwrap().expect("die Sitzung gilt");
    assert_eq!(account.id, author.id, "sie gehört dem Konto");
    assert!(idle < 60, "gerade benutzt: {idle} s");
    assert!(account.last_login.is_some(), "die Anmeldung ist am Konto vermerkt");
    assert!(store.session_account(token).await.unwrap().is_none(), "mit dem Token statt dem Hash findet sich nichts");

    // last_seen höchstens einmal je Minute: 30 s alt bleibt stehen, 5 min alt wird aufgefrischt.
    let set = |column: &str, modifier: &str| {
        format!("UPDATE account_session SET {column} = strftime('%Y-%m-%dT%H:%M:%fZ','now','{modifier}')")
    };
    exec(store.pool(), &set("last_seen", "-30 seconds")).await;
    let seen_before: String =
        sqlx::query_scalar("SELECT last_seen FROM account_session").fetch_one(store.pool()).await.unwrap();
    store.session_account(&hash).await.unwrap().unwrap();
    let seen_after: String =
        sqlx::query_scalar("SELECT last_seen FROM account_session").fetch_one(store.pool()).await.unwrap();
    assert_eq!(seen_after, seen_before, "innerhalb einer Minute wird last_seen nicht neu geschrieben");
    exec(store.pool(), &set("last_seen", "-5 minutes")).await;
    let (_, idle) = store.session_account(&hash).await.unwrap().unwrap();
    assert!((295..=310).contains(&idle), "die Pause seit der vorigen Benutzung: {idle} s");
    let (_, idle) = store.session_account(&hash).await.unwrap().unwrap();
    assert!(idle < 60, "danach ist last_seen aufgefrischt: {idle} s");

    // Fristen: 30 Tage ohne Benutzung, 90 Tage insgesamt.
    exec(store.pool(), &set("last_seen", "-29 days")).await;
    assert!(store.session_account(&hash).await.unwrap().is_some(), "29 Tage ohne Benutzung: gilt noch");
    exec(store.pool(), &set("last_seen", "-31 days")).await;
    assert!(store.session_account(&hash).await.unwrap().is_none(), "31 Tage ohne Benutzung: abgelaufen");
    exec(store.pool(), &set("last_seen", "-1 minutes")).await;
    exec(store.pool(), &set("created", "-89 days")).await;
    assert!(store.session_account(&hash).await.unwrap().is_some(), "89 Tage alt und benutzt: gilt noch");
    exec(store.pool(), &set("created", "-91 days")).await;
    assert!(store.session_account(&hash).await.unwrap().is_none(), "91 Tage alt: abgelaufen, auch wenn benutzt");
    // Eine neue Sitzung räumt die abgelaufene weg.
    let fresh = Store::session_token_hash("zweites-token");
    store.session_create(&fresh, author.id).await.unwrap();
    assert_eq!(scalar(store.pool(), "SELECT count(*) FROM account_session").await, 1, "die abgelaufene ist gelöscht");

    // Passwortwechsel: alle anderen Sitzungen enden, die genannte bleibt.
    let other = Store::session_token_hash("drittes-token");
    store.session_create(&other, author.id).await.unwrap();
    let changed_before: String =
        sqlx::query_scalar("SELECT password_changed FROM account").fetch_one(store.pool()).await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    assert!(store.account_set_password(author.id, "$argon2id$neu", Some(&fresh)).await.unwrap(), "Passwort gesetzt");
    assert!(store.session_account(&fresh).await.unwrap().is_some(), "die Sitzung, die das Passwort ändert, bleibt");
    assert!(store.session_account(&other).await.unwrap().is_none(), "jede andere Sitzung endet sofort");
    assert_eq!(store.account_password(author.id).await.unwrap().as_deref(), Some("$argon2id$neu"), "neuer PHC-Text");
    let changed_after: String =
        sqlx::query_scalar("SELECT password_changed FROM account").fetch_one(store.pool()).await.unwrap();
    assert!(changed_after > changed_before, "der Zeitpunkt des Wechsels ist vermerkt");
    assert!(!store.account_set_password(999, "$argon2id$x", None).await.unwrap(), "unbekanntes Konto: false");
    assert!(
        store.account_set_password(author.id, "klartext", None).await.is_err(),
        "auch über den Store kommt kein Klartext in die Spalte"
    );
    assert!(store.session_account(&fresh).await.unwrap().is_some(), "ein gescheiterter Wechsel beendet keine Sitzung");

    // Abmelden und Sperren.
    assert!(store.session_delete(&fresh).await.unwrap(), "Abmelden löscht die Sitzung");
    assert!(!store.session_delete(&fresh).await.unwrap(), "ein zweites Mal gibt es sie nicht mehr");
    let lektor = store.account_create(&new_account("Lea", "lea", "lektor")).await.unwrap();
    let (one, two) = (Store::session_token_hash("lea-eins"), Store::session_token_hash("lea-zwei"));
    store.session_create(&one, lektor.id).await.unwrap();
    store.session_create(&two, lektor.id).await.unwrap();
    assert_eq!(store.session_delete_all(lektor.id, Some(&one)).await.unwrap(), 1, "alle außer der genannten");
    assert!(store.session_account(&one).await.unwrap().is_some(), "die genannte bleibt");
    store.account_set_active(lektor.id, false).await.unwrap();
    assert!(store.session_account(&one).await.unwrap().is_none(), "Sperren beendet die Sitzung sofort");
    assert_eq!(
        scalar(store.pool(), &format!("SELECT count(*) FROM account_session WHERE account_id = {}", lektor.id)).await,
        0,
        "die Sitzungen eines gesperrten Kontos sind gelöscht, nicht nur unwirksam"
    );
}

fn doubling(failures: i64) -> u64 {
    if failures <= 3 { 0 } else { (1u64 << (failures - 3).min(10)).min(900) }
}

#[tokio::test]
async fn failures_block_a_scope_and_the_table_never_grows_past_a_thousand_rows() {
    let (_dir, store) = open_store().await;
    assert_eq!(store.login_wait(&["konto:erika"]).await.unwrap(), None, "ohne Fehlversuch keine Wartezeit");
    for attempt in 1..=3 {
        assert_eq!(store.login_failure("konto:erika", doubling).await.unwrap(), 0, "Fehlversuch {attempt}: frei");
    }
    assert_eq!(store.login_wait(&["konto:erika"]).await.unwrap(), None, "drei Fehlversuche sperren nicht");
    assert_eq!(store.login_failure("konto:erika", doubling).await.unwrap(), 2, "der vierte: 2 Sekunden");
    let wait = store.login_wait(&["konto:erika"]).await.unwrap().expect("jetzt gesperrt");
    assert!((1..=2).contains(&wait), "verbleibende Wartezeit, aufgerundet: {wait}");
    assert_eq!(store.login_wait(&["konto:jonas"]).await.unwrap(), None, "ein anderer Bereich ist nicht gesperrt");
    assert_eq!(
        store.login_wait(&["konto:jonas", "konto:erika"]).await.unwrap(),
        Some(wait),
        "von mehreren Bereichen zählt der gesperrte"
    );
    assert_eq!(store.login_failure("konto:erika", doubling).await.unwrap(), 4, "der fünfte: 4 Sekunden");
    exec(store.pool(), "UPDATE login_attempt SET failures = 20 WHERE scope = 'konto:erika'").await;
    assert_eq!(store.login_failure("konto:erika", doubling).await.unwrap(), 900, "nie mehr als 900 Sekunden");
    let wait = store.login_wait(&["konto:erika"]).await.unwrap().unwrap();
    assert!((899..=900).contains(&wait), "die Sperre läuft 900 Sekunden: {wait}");
    // Abgelaufene Sperre: keine Wartezeit mehr, die Zählung bleibt bis zum Erfolg.
    exec(store.pool(), "UPDATE login_attempt SET blocked_until = strftime('%Y-%m-%dT%H:%M:%fZ','now','-1 seconds')")
        .await;
    assert_eq!(store.login_wait(&["konto:erika"]).await.unwrap(), None, "nach Ablauf ist der Bereich frei");
    store.login_reset("konto:erika").await.unwrap();
    assert_eq!(scalar(store.pool(), "SELECT count(*) FROM login_attempt").await, 0, "Erfolg setzt den Bereich zurück");
    assert_eq!(store.login_failure("konto:erika", doubling).await.unwrap(), 0, "danach zählt es wieder von vorn");

    // Älter als 24 Stunden: fällt beim nächsten Schreiben weg.
    exec(
        store.pool(),
        "INSERT INTO login_attempt (scope, failures, last) \
         VALUES ('konto:alt', 9, strftime('%Y-%m-%dT%H:%M:%fZ','now','-25 hours'))",
    )
    .await;
    store.login_failure("konto:neu", doubling).await.unwrap();
    assert_eq!(
        scalar(store.pool(), "SELECT count(*) FROM login_attempt WHERE scope = 'konto:alt'").await,
        0,
        "ein Eintrag, der älter als 24 Stunden ist, wird beim Schreiben entfernt"
    );

    // Obergrenze: 999 Bereiche und der Bereich `alle`.
    exec(store.pool(), "DELETE FROM login_attempt").await;
    exec(
        store.pool(),
        "WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 998) \
         INSERT INTO login_attempt (scope, failures) SELECT 'konto:flut' || i, 1 FROM n",
    )
    .await;
    assert_eq!(store.login_failure("konto:letzter", doubling).await.unwrap(), 0, "die 999. Zeile entsteht noch");
    assert_eq!(scalar(store.pool(), "SELECT count(*) FROM login_attempt").await, 999, "999 Bereiche");
    for extra in 0..5 {
        let wait = store.login_failure(&format!("konto:zuviel{extra}"), doubling).await.unwrap();
        assert_eq!(wait, Store::LOGIN_FULL_WAIT, "volle Tabelle: ein neuer Bereich wartet fest 2 Sekunden");
    }
    assert_eq!(
        scalar(store.pool(), "SELECT count(*) FROM login_attempt").await,
        Store::LOGIN_ATTEMPT_ROWS,
        "die Tabelle wächst nicht über 1000 Zeilen (999 Bereiche und `alle`)"
    );
    assert_eq!(
        scalar(store.pool(), "SELECT failures FROM login_attempt WHERE scope = 'alle'").await,
        5,
        "die überzähligen Fehlversuche zählt der Bereich `alle`"
    );
    assert!(
        store.login_wait(&["konto:ganz-neu"]).await.unwrap().is_some(),
        "solange `alle` wartet, wartet jeder Anmeldeversuch"
    );
    assert_eq!(
        store.login_failure("konto:flut7", doubling).await.unwrap(),
        0,
        "ein Bereich, der schon eine Zeile hat, zählt auch bei voller Tabelle für sich weiter"
    );
}

#[tokio::test]
async fn the_switch_for_the_own_machine_is_off_until_set() {
    let (_dir, store) = open_store().await;
    assert!(!store.auth_local_open().await.unwrap(), "Vorgabe: auch am eigenen Rechner anmelden");
    store.set_auth_local_open(true).await.unwrap();
    assert!(store.auth_local_open().await.unwrap(), "eingeschaltet");
    let value: String = sqlx::query_scalar("SELECT value FROM store_meta WHERE key = 'auth.local_open'")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(value, "1", "der Schlüssel auth.local_open trägt '1' (Vertrag 0.2)");
    store.set_auth_local_open(true).await.unwrap();
    store.set_auth_local_open(false).await.unwrap();
    assert!(!store.auth_local_open().await.unwrap(), "wieder aus");
    exec(store.pool(), "INSERT INTO store_meta (key, value) VALUES ('auth.local_open', 'ja')").await;
    assert!(!store.auth_local_open().await.unwrap(), "nur genau '1' schaltet ein – jeder andere Wert bleibt aus");
}
