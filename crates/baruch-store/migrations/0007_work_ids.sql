-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Werk-ids werden nie wiederverwendet, die Saat merkt sich, dass sie gelaufen ist, und slugs
-- gelöschter Werke bleiben reserviert (Vertrag Runde 5, Abschnitt 2).
--
-- Weg (Abweichung vom Vertrag, Beleg in tests/migration_0007.rs): sqlx-sqlite 0.8.6 führt jede
-- Migration in einer eigenen Transaktion aus (`migrate.rs`, `apply`: `self.begin()`), auch mit
-- `-- no-transaction` am Anfang; das Feld `no_tx` liest dort niemand. In einer Transaktion ist
-- `PRAGMA foreign_keys=OFF` wirkungslos, und ein eigenes `BEGIN IMMEDIATE` scheitert („cannot start
-- a transaction within a transaction“). Darum läuft der Neubau in der Transaktion von sqlx mit
-- `PRAGMA defer_foreign_keys=ON` (wirkt auch in einer Transaktion, endet mit ihr): Das Löschen der
-- alten Tabelle zählt die Verweise aus `node` und `work_template_revision` als offene Verstöße, das
-- Einfügen in die neue Tabelle `work` zählt sie wieder ab; bleibt beim COMMIT etwas offen, scheitert
-- die Migration, und nichts wird übernommen (auch kein Eintrag in `_sqlx_migrations`). Die neue
-- Tabelle heißt gleich `work` (kein RENAME), damit die Verweise der anderen Tabellen sie finden.
-- Vorher prüft eine Wache mit `pragma_foreign_key_check` ausdrücklich, dass es keinen Verstoß gibt.

PRAGMA defer_foreign_keys = ON;

-- (a) Zeilen sichern: alle Spalten aus 0001, 0002, 0005 und 0006 in derselben Reihenfolge.
CREATE TABLE work_new (
  id            INTEGER PRIMARY KEY,
  slug          TEXT NOT NULL UNIQUE,
  kind          TEXT NOT NULL CHECK (kind IN ('book','sermon')),
  title         TEXT NOT NULL,
  author        TEXT NOT NULL,
  created       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  template      TEXT NOT NULL DEFAULT '',
  source_name   TEXT,
  source_sha256 TEXT,
  title_key     TEXT,
  subtitle      TEXT NOT NULL DEFAULT '',
  audience      TEXT NOT NULL DEFAULT '',
  bible         TEXT NOT NULL DEFAULT '',
  updated       TEXT,
  numbering     TEXT
);
INSERT INTO work_new (id, slug, kind, title, author, created, template, source_name, source_sha256,
                      title_key, subtitle, audience, bible, updated, numbering)
  SELECT id, slug, kind, title, author, created, template, source_name, source_sha256,
         title_key, subtitle, audience, bible, updated, numbering FROM work;

-- (b) Alte Tabelle weg (ihre Indizes gehen mit), neue mit AUTOINCREMENT: SQLite merkt sich die
-- größte je vergebene id in `sqlite_sequence` und vergibt sie nie wieder.
DROP TABLE work;
CREATE TABLE work (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  slug          TEXT NOT NULL UNIQUE,              -- Ordnername des Typst-Spiegels
  kind          TEXT NOT NULL CHECK (kind IN ('book','sermon')),
  title         TEXT NOT NULL,
  author        TEXT NOT NULL,
  created       TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  template      TEXT NOT NULL DEFAULT '',          -- 0002: Präambel bis zur ersten Überschrift
  source_name   TEXT,                              -- 0002: Dateiname beim Import
  source_sha256 TEXT,                              -- 0002: Hex, SHA-256 des Quelltexts nach CRLF→LF
  title_key     TEXT,                              -- 0005: baruch_store::title_key(title)
  subtitle      TEXT NOT NULL DEFAULT '',          -- 0005
  audience      TEXT NOT NULL DEFAULT '',          -- 0005: Zielgruppe
  bible         TEXT NOT NULL DEFAULT '',          -- 0005: Bibelübersetzung
  updated       TEXT,                              -- 0005: letzte Änderung von Metadaten/Vorlage
  numbering     TEXT                               -- 0006: Nummerierungsmuster, NULL = aus der Vorlage
);
INSERT INTO work (id, slug, kind, title, author, created, template, source_name, source_sha256,
                  title_key, subtitle, audience, bible, updated, numbering)
  SELECT id, slug, kind, title, author, created, template, source_name, source_sha256,
         title_key, subtitle, audience, bible, updated, numbering FROM work_new ORDER BY id;
DROP TABLE work_new;
CREATE UNIQUE INDEX work_source_sha256 ON work(source_sha256) WHERE source_sha256 IS NOT NULL;
CREATE UNIQUE INDEX work_title_key ON work(title_key) WHERE title_key IS NOT NULL;

-- (c) Merkzettel des Stores. `seeded` = das Beispielwerk ist gesät (bzw. die Datenbank hatte schon
-- Werke oder Personas); danach sät `seed_if_empty` nie wieder, auch wenn alle Werke gelöscht sind.
CREATE TABLE store_meta (
  key   TEXT PRIMARY KEY,
  value TEXT NOT NULL
);
INSERT INTO store_meta (key, value)
  SELECT 'seeded', '1' WHERE EXISTS (SELECT 1 FROM work) OR EXISTS (SELECT 1 FROM persona);

-- (d) Gelöschte Werke: id, slug und Titel. Der slug bleibt reserviert (`free_slug` meidet ihn), weil
-- der Spiegel unter works/.geloescht/ liegt und die Datenbank den Archivordner nicht kennt.
CREATE TABLE work_retired (
  id      INTEGER PRIMARY KEY,
  slug    TEXT NOT NULL UNIQUE,
  title   TEXT NOT NULL,
  deleted TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

-- (e) Wache: Gibt es einen Verstoß gegen einen Fremdschlüssel, scheitert der CHECK, und die ganze
-- Migration wird zurückgerollt.
CREATE TEMP TABLE fk_guard (n INTEGER CHECK (n = 0));
INSERT INTO fk_guard SELECT count(*) FROM pragma_foreign_key_check;
DROP TABLE temp.fk_guard;
