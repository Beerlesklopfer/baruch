// SPDX-License-Identifier: AGPL-3.0-or-later
//! Migration 0054 (Runde 35 „Konten und Rollen“, fachlich: rollen – Tabelle `work_member`) auf einer nachgebauten
//! Datenbank im Stand 0053 mit frei erfundenen Werken und Konten, dazu das Store-Modul `members`.
//!
//! Belegt: Die Datei 0054 ist gegenüber der Schnittstelle S1 unverändert und steht nicht in `FOREIGN_KEYS_OFF`; jede
//! Zeile der alten Tabellen und jedes alte Schemaobjekt bleibt byte-gleich; `work_member` ist danach leer; die CHECKs
//! und Fremdschlüssel lehnen ab, was der Vertrag verbietet; das Löschen eines Werks oder eines Kontos entfernt seine
//! Freigaben; `member_grant` gibt nur Lektor-Konten und nur vorhandene Werke frei und ist idempotent; ein gesperrtes
//! Konto hat keine wirksame Freigabe.

use std::borrow::Cow;
use std::path::Path;
use std::str::FromStr;

use baruch_store::{Store, StoreError};
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};

/// Die Tabelle wörtlich aus dem Vertrag (§S1.2), ohne die Kommentarzeilen der Datei.
const CONTRACT_SQL: &str = "CREATE TABLE work_member (
  work_id INTEGER NOT NULL REFERENCES work(id) ON DELETE CASCADE,
  account_id INTEGER NOT NULL REFERENCES account(id) ON DELETE CASCADE,
  role TEXT NOT NULL CHECK (role IN ('lektor')),
  granted TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  PRIMARY KEY (work_id, account_id)
);
CREATE INDEX work_member_account ON work_member(account_id);
";

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
    sqlx::query(sqlx::AssertSqlSafe(sql)).execute(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"));
}

async fn scalar(pool: &SqlitePool, sql: &str) -> i64 {
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql)).fetch_one(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"))
}

async fn rejected(pool: &SqlitePool, sql: &str) -> bool {
    sqlx::query(sqlx::AssertSqlSafe(sql)).execute(pool).await.is_err()
}

/// Store bis 0053 (0040 wie in `baruch_store::migrate` mit ausgeschalteten Fremdschlüsseln): der Stand vor 0054.
async fn migrate_before(pool: &SqlitePool) {
    assert_eq!(baruch_store::FOREIGN_KEYS_OFF, &[40], "Vorbedingung des Nachbaus: nur 0040 baut Tabellen um");
    store_migrator(|v| v < 40).run(pool).await.unwrap();
    {
        let mut conn = pool.acquire().await.unwrap();
        sqlx::query("PRAGMA foreign_keys = OFF").execute(&mut *conn).await.unwrap();
        store_migrator(|v| v == 40).run(&mut *conn).await.unwrap();
        sqlx::query("PRAGMA foreign_keys = ON").execute(&mut *conn).await.unwrap();
    }
    store_migrator(|v| (41..=53).contains(&v)).run(pool).await.unwrap();
    assert_eq!(scalar(pool, "SELECT max(version) FROM _sqlx_migrations").await, 53, "Stand davor: Store bis 0053");
}

async fn insert_work(pool: &SqlitePool, slug: &str, title: &str) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO work (slug, kind, title, author, template, source_name, source_sha256, title_key) \
         VALUES (?, 'book', ?, 'Tilda Rohr', '', ?, ?, ?) RETURNING id",
    )
    .bind(slug)
    .bind(title)
    .bind(format!("{slug}.typ"))
    .bind(format!("sha-{slug}"))
    .bind(baruch_store::title_key(title))
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn insert_account(pool: &SqlitePool, name: &str, kind: &str, active: bool) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO account (name, name_key, slug, display, kind, password, active) \
         VALUES (?, lower(?), lower(?), ?, ?, '$argon2id$test', ?) RETURNING id",
    )
    .bind(name)
    .bind(name)
    .bind(name)
    .bind(format!("{name} Beispiel"))
    .bind(kind)
    .bind(active)
    .fetch_one(pool)
    .await
    .unwrap()
}

fn grant_sql(work: i64, account: i64, role: &str) -> String {
    format!("INSERT INTO work_member (work_id, account_id, role) VALUES ({work}, {account}, '{role}')")
}

/// Alle Zeilen einer Tabelle als Text (jede Spalte über `quote()`), nach rowid.
async fn dump(pool: &SqlitePool, table: &str) -> Vec<String> {
    let columns: Vec<String> = sqlx::query(sqlx::AssertSqlSafe(format!("PRAGMA table_info({table})")))
        .fetch_all(pool)
        .await
        .unwrap()
        .iter()
        .map(|row| format!("quote(\"{}\")", row.get::<String, _>("name")))
        .collect();
    assert!(!columns.is_empty(), "Tabelle {table} gibt es");
    let sql = format!("SELECT {} FROM {table} ORDER BY rowid", columns.join(" || '|' || "));
    sqlx::query_scalar(sqlx::AssertSqlSafe(sql.as_str())).fetch_all(pool).await.unwrap_or_else(|e| panic!("{sql}: {e}"))
}

/// Namen aller gewöhnlichen Tabellen (ohne die Buchführung von sqlx, Volltextindizes und ihre Schattentabellen).
async fn tables(pool: &SqlitePool) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' \
         AND name <> '_sqlx_migrations' AND sql NOT LIKE 'CREATE VIRTUAL%' AND name NOT LIKE '%fts%' ORDER BY name",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

async fn schema(pool: &SqlitePool) -> Vec<String> {
    sqlx::query_scalar(
        "SELECT type || ' ' || name || ': ' || coalesce(sql, '') FROM sqlite_master \
         WHERE name NOT LIKE 'sqlite_%' ORDER BY type, name",
    )
    .fetch_all(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn the_migration_file_is_the_one_from_the_interface_and_only_creates() {
    let migration = baruch_store::MIGRATOR.iter().find(|m| m.version == 54).expect("Migration 0054 ist eingebunden");
    let statements: String = migration
        .sql
        .as_str()
        .lines()
        .filter(|l| !l.trim_start().starts_with("--"))
        .map(|l| format!("{l}\n"))
        .collect();
    assert_eq!(statements, CONTRACT_SQL, "0054 steht wörtlich im Vertrag und ist nach S1 unveränderlich");
    assert!(
        !baruch_store::FOREIGN_KEYS_OFF.contains(&54),
        "0054 legt nur eine Tabelle an und baut keine um: nicht in FOREIGN_KEYS_OFF"
    );
}

#[tokio::test]
async fn existing_rows_and_schema_survive_and_the_new_table_starts_empty() {
    let dir = tempfile::tempdir().unwrap();
    let pool = raw_pool(dir.path()).await;
    migrate_before(&pool).await;
    let kahn = insert_work(&pool, "der-kahn-am-wehr", "Der Kahn am Wehr").await;
    let moor = insert_work(&pool, "das-moor-im-nebel", "Das Moor im Nebel").await;
    let author = insert_account(&pool, "Tilda", "author", true).await;
    let lektor = insert_account(&pool, "Erika", "lektor", true).await;
    let token = "a".repeat(64);
    exec(&pool, &format!("INSERT INTO account_session (token_hash, account_id) VALUES ('{token}', {lektor})")).await;
    exec(&pool, "INSERT INTO login_attempt (scope, failures) VALUES ('konto:erika', 2)").await;
    assert_eq!(
        scalar(&pool, "SELECT count(*) FROM sqlite_master WHERE name = 'work_member'").await,
        0,
        "Vorbedingung: im Stand 0053 gibt es die Tabelle noch nicht"
    );
    let mut rows_before = Vec::new();
    for table in tables(&pool).await {
        let rows = dump(&pool, &table).await;
        rows_before.push((table, rows));
    }
    let schema_before = schema(&pool).await;
    for (table, expected) in [("work", 2), ("account", 2), ("account_session", 1), ("login_attempt", 1)] {
        let rows = rows_before.iter().find(|(t, _)| t == table).map(|(_, rows)| rows.len());
        assert_eq!(rows, Some(expected), "Vorbedingung: erfundene Zeilen in {table} liegen in der alten Datenbank");
    }

    baruch_store::migrate(&pool).await.expect("0054 läuft auf dem Stand 0053 durch");

    assert!(scalar(&pool, "SELECT max(version) FROM _sqlx_migrations").await >= 54, "0054 ist eingetragen");
    for (table, before) in &rows_before {
        let mut after = dump(&pool, table).await;
        if table == "work" {
            // Round 38: the same call also runs 0055, which appends `work.language` ('de' for every existing work).
            after = after
                .iter()
                .map(|row| row.strip_suffix("|'de'").expect("0055 makes every existing work German").to_owned())
                .collect();
        }
        assert_eq!(&after, before, "jede Zeile von {table} bleibt byte-gleich (nur CREATE)");
    }
    let schema_after = schema(&pool).await;
    assert!(
        schema_before.iter().filter(|line| !line.starts_with("table work: ")).all(|line| schema_after.contains(line)),
        "kein altes Schemaobjekt ist geändert oder entfernt"
    );
    // Round 38: `ADD COLUMN` of 0055 extends the stored definition of `work` and changes nothing else in it.
    let work_before = schema_before.iter().find(|line| line.starts_with("table work: ")).expect("work exists");
    let work_after = schema_after.iter().find(|line| line.starts_with("table work: ")).expect("work still exists");
    assert!(
        work_after.starts_with(work_before.strip_suffix(')').expect("the definition ends with a bracket"))
            && work_after.contains("language TEXT NOT NULL DEFAULT 'de'"),
        "the definition of work is the old one plus the column language of 0055: {work_after}"
    );
    let added: Vec<&str> = schema_after
        .iter()
        .filter(|line| !schema_before.contains(line))
        .map(|line| line.split(':').next().unwrap())
        .filter(|name| *name == "table work_member" || *name == "index work_member_account")
        .collect();
    assert_eq!(added, ["index work_member_account", "table work_member"], "0054 bringt die Tabelle und ihren Index");
    assert_eq!(scalar(&pool, "SELECT count(*) FROM work_member").await, 0, "ohne Zutun ist kein Werk freigegeben");

    // Was die Tabelle ablehnt (Fremdschlüssel sind eingeschaltet wie im Store).
    assert!(
        rejected(&pool, &grant_sql(kahn, lektor, "author")).await,
        "eine andere Rolle als lektor lehnt der CHECK ab (der Autor braucht keine Zeile)"
    );
    assert!(rejected(&pool, &grant_sql(9999, lektor, "lektor")).await, "unbekanntes Werk: Fremdschlüssel");
    assert!(rejected(&pool, &grant_sql(kahn, 9999, "lektor")).await, "unbekanntes Konto: Fremdschlüssel");
    exec(&pool, &grant_sql(kahn, lektor, "lektor")).await;
    assert!(
        rejected(&pool, &grant_sql(kahn, lektor, "lektor")).await,
        "je Werk und Konto höchstens eine Freigabe (Primärschlüssel)"
    );
    let granted: String = sqlx::query_scalar("SELECT granted FROM work_member").fetch_one(&pool).await.unwrap();
    assert!(
        granted.ends_with('Z') && granted.contains('T'),
        "der Zeitpunkt der Freigabe steht in der Zeile: {granted}"
    );

    // Löschen eines Werks bzw. Kontos nimmt die Freigaben mit.
    exec(&pool, &grant_sql(moor, lektor, "lektor")).await;
    exec(&pool, &format!("DELETE FROM work WHERE id = {kahn}")).await;
    assert_eq!(
        scalar(&pool, "SELECT count(*) FROM work_member").await,
        1,
        "mit dem Werk verschwindet seine Freigabe, die des anderen Werks bleibt"
    );
    assert_eq!(scalar(&pool, "SELECT work_id FROM work_member").await, moor, "geblieben ist die Freigabe des Moors");
    exec(&pool, &format!("DELETE FROM account WHERE id = {lektor}")).await;
    assert_eq!(
        scalar(&pool, "SELECT count(*) FROM work_member").await,
        0,
        "mit dem Konto verschwinden seine Freigaben"
    );
    assert_eq!(
        scalar(&pool, &format!("SELECT count(*) FROM account WHERE id = {author}")).await,
        1,
        "das Autor-Konto ist davon unberührt"
    );
}

/// Ein Store mit zwei erfundenen Werken, einem Autor und zwei Lektoren (einer gesperrt).
async fn store_with_accounts(dir: &Path) -> (Store, [i64; 2], [i64; 3]) {
    let store = Store::open(dir).await.unwrap();
    let pool = store.pool();
    let kahn = insert_work(pool, "der-kahn-am-wehr", "Der Kahn am Wehr").await;
    let moor = insert_work(pool, "das-moor-im-nebel", "Das Moor im Nebel").await;
    let author = insert_account(pool, "Tilda", "author", true).await;
    let erika = insert_account(pool, "Erika", "lektor", true).await;
    let locked = insert_account(pool, "Gustav", "lektor", false).await;
    (store, [kahn, moor], [author, erika, locked])
}

#[tokio::test]
async fn grant_takes_only_lektor_accounts_and_known_works_and_is_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let (store, [kahn, moor], [author, erika, _]) = store_with_accounts(dir.path()).await;
    assert!(store.member_list(kahn).await.unwrap().is_empty(), "am Anfang ist nichts freigegeben");
    assert!(store.member_works(erika).await.unwrap().is_empty(), "und Erika sieht kein Werk");

    store.member_grant(moor, erika, "lektor").await.expect("ein Lektor-Konto bekommt die Freigabe");
    store.member_grant(kahn, erika, "lektor").await.unwrap();
    let first = store.member_list(kahn).await.unwrap();
    assert_eq!(first.len(), 1, "genau eine Freigabe im Werk");
    assert_eq!(
        (first[0].work, first[0].account, first[0].role.as_str()),
        (kahn, erika, "lektor"),
        "Werk, Konto, Rolle"
    );
    assert_eq!(store.member_works(erika).await.unwrap(), [kahn, moor], "freigegebene Werke aufsteigend nach id");

    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    store.member_grant(kahn, erika, "lektor").await.expect("noch einmal freigeben ist kein Fehler");
    assert_eq!(
        store.member_list(kahn).await.unwrap(),
        first,
        "idempotent: weder eine zweite Zeile noch ein neuer Zeitpunkt"
    );

    for (account, why) in [(author, "ein Autor-Konto ist Autor in jedem Werk"), (9999, "ein unbekanntes Konto")] {
        let refused = store.member_grant(kahn, account, "lektor").await;
        assert!(matches!(refused, Err(StoreError::Refused(_))), "{why}: abgelehnt, nicht {refused:?}");
    }
    assert!(
        matches!(store.member_grant(9999, erika, "lektor").await, Err(StoreError::UnknownWork(9999))),
        "ein unbekanntes Werk lässt sich nicht freigeben"
    );
    assert!(
        matches!(store.member_grant(kahn, erika, "author").await, Err(StoreError::Refused(_))),
        "eine andere Rolle als lektor gibt es nicht zu vergeben"
    );
    assert!(matches!(store.member_list(9999).await, Err(StoreError::UnknownWork(9999))), "unbekanntes Werk beim Lesen");
    assert_eq!(store.member_list(kahn).await.unwrap(), first, "die abgelehnten Versuche haben nichts geschrieben");
}

#[tokio::test]
async fn revoke_and_lock_take_effect_and_deleting_removes_the_grants() {
    let dir = tempfile::tempdir().unwrap();
    let (store, [kahn, moor], [_, erika, locked]) = store_with_accounts(dir.path()).await;
    store.member_grant(kahn, erika, "lektor").await.unwrap();
    store.member_grant(moor, erika, "lektor").await.unwrap();
    store.member_grant(kahn, locked, "lektor").await.expect("auch ein gesperrtes Lektor-Konto kann eine Zeile haben");
    assert_eq!(store.member_list(kahn).await.unwrap().len(), 2, "die Liste des Werks zeigt beide Freigaben");
    assert!(
        store.member_works(locked).await.unwrap().is_empty(),
        "ein gesperrtes Konto hat keine wirksame Freigabe (die Sperre wirkt bei der nächsten Anfrage)"
    );

    assert!(store.member_revoke(kahn, erika).await.unwrap(), "die Freigabe gab es: entzogen");
    assert!(!store.member_revoke(kahn, erika).await.unwrap(), "ein zweites Mal gibt es nichts zu entziehen");
    assert_eq!(store.member_works(erika).await.unwrap(), [moor], "nur das andere Werk bleibt freigegeben");

    sqlx::query("UPDATE account SET active = 0 WHERE id = ?").bind(erika).execute(store.pool()).await.unwrap();
    assert!(store.member_works(erika).await.unwrap().is_empty(), "nach dem Sperren sieht Erika kein Werk mehr");
    sqlx::query("UPDATE account SET active = 1 WHERE id = ?").bind(erika).execute(store.pool()).await.unwrap();
    assert_eq!(store.member_works(erika).await.unwrap(), [moor], "nach dem Entsperren gilt die alte Freigabe wieder");

    let deleted = store.delete_work(moor, "Das Moor im Nebel").await.expect("Werk löschen wie der Autor");
    assert_eq!(deleted.work, moor, "gelöscht ist das Moor");
    assert!(store.member_works(erika).await.unwrap().is_empty(), "mit dem Werk verschwindet seine Freigabe");
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM work_member").fetch_one(store.pool()).await.unwrap(),
        1,
        "übrig ist nur die Zeile des gesperrten Kontos im anderen Werk"
    );
}
