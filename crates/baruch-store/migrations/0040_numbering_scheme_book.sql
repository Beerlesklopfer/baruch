-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Gliederungsschema „Buch“ (`book`): Der CHECK von work.numbering_scheme (Migration 0008) lässt den
-- neuen Wert zu. SQLite kann einen CHECK nicht ändern, darum wird `work` nach dem offiziellen Verfahren
-- umgebaut (https://sqlite.org/lang_altertable.html, „Making Other Kinds Of Table Schema Changes“):
-- neue Tabelle anlegen, Zeilen kopieren, alte löschen, neue umbenennen, Indizes neu, Fremdschlüssel prüfen.
--
-- Fremdschlüssel AUS: Viele Tabellen verweisen mit ON DELETE CASCADE bzw. SET NULL auf work(id)
-- (outline_request, asset_request, work_book, work_rag, work_rag_collection, work_bible, sync_work,
-- work_sword_source, chat_thread, work_timing; idea mit SET NULL). Mit eingeschalteten Fremdschlüsseln
-- führte `DROP TABLE work` ein implizites DELETE aus, und diese Aktionen löschten bzw. leerten die Zeilen –
-- auch `PRAGMA defer_foreign_keys` (Weg von 0007) verhindert Aktionen nicht. In der Transaktion, in der
-- sqlx jede Migration ausführt, ist `PRAGMA foreign_keys=OFF` wirkungslos; darum schaltet
-- `baruch_store::migrate` die Fremdschlüssel für genau diese Migration auf einer eigenen Verbindung vor
-- der Transaktion aus und danach wieder ein (`FOREIGN_KEYS_OFF`). Die Wache (a) bricht ab, falls die
-- Migration doch mit eingeschalteten Fremdschlüsseln liefe (z. B. ein Migrator direkt auf einem Pool).
--
-- Was an `work` hängt (sqlite_master im Stand 0039, Beleg in tests/migration_0040.rs): die Indizes
-- work_source_sha256 und work_title_key (beide partiell eindeutig) und der automatische Index von
-- UNIQUE(slug); kein Trigger auf `work`, keine View, kein FTS-Bezug (search_fts zählt Fassungen,
-- Beiträge und Bibliothek). Die Verweise der anderen Tabellen nennen `work` beim Namen und gelten nach
-- dem Umbenennen wieder. `sqlite_sequence` (AUTOINCREMENT, 0007: ids nie wiederverwenden) behält den
-- größten je vergebenen Wert, auch den eines gelöschten Werks.

-- (a) Wache: Fremdschlüssel müssen aus sein, sonst nichts anfassen.
CREATE TEMP TABLE fk_off_guard (n INTEGER CHECK (n = 0));
INSERT INTO fk_off_guard SELECT foreign_keys FROM pragma_foreign_keys;
DROP TABLE temp.fk_off_guard;

-- (b) Größte vergebene id merken (DROP TABLE löscht die Zeile in sqlite_sequence).
CREATE TEMP TABLE work_seq AS SELECT seq FROM sqlite_sequence WHERE name = 'work';

-- (c) Neue Tabelle: alle Spalten aus 0001, 0002, 0005, 0006, 0007 und 0008 in derselben Reihenfolge,
-- der CHECK mit 'book' hinter 'sermon'.
CREATE TABLE work_new (
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
  numbering     TEXT,                              -- 0006: eigenes Muster (Schema 'custom')
  numbering_scheme TEXT NOT NULL DEFAULT 'template' -- 0008, 0040: Gliederungsschema
    CHECK (numbering_scheme IN ('template','decimal','decimal_dot','alphanumeric','chapter_roman',
                                'chapter_arabic','parts_chapters','chapter_words','legal','sermon','book',
                                'none','custom'))
);
INSERT INTO work_new (id, slug, kind, title, author, created, template, source_name, source_sha256,
                      title_key, subtitle, audience, bible, updated, numbering, numbering_scheme)
  SELECT id, slug, kind, title, author, created, template, source_name, source_sha256,
         title_key, subtitle, audience, bible, updated, numbering, numbering_scheme FROM work ORDER BY id;

-- (d) Alte Tabelle weg (ihre Indizes gehen mit), neue umbenennen, Indizes wie in 0007.
DROP TABLE work;
ALTER TABLE work_new RENAME TO work;
CREATE UNIQUE INDEX work_source_sha256 ON work(source_sha256) WHERE source_sha256 IS NOT NULL;
CREATE UNIQUE INDEX work_title_key ON work(title_key) WHERE title_key IS NOT NULL;

-- (e) sqlite_sequence: der größere Wert aus altem Zähler und größter vorhandener id.
DELETE FROM sqlite_sequence WHERE name = 'work';
INSERT INTO sqlite_sequence (name, seq)
  SELECT 'work', max(coalesce((SELECT max(seq) FROM temp.work_seq), 0), coalesce((SELECT max(id) FROM work), 0));
DROP TABLE temp.work_seq;

-- (f) Wache: Gibt es einen Verstoß gegen einen Fremdschlüssel, scheitert der CHECK, und die ganze
-- Migration wird zurückgerollt (auch kein Eintrag in _sqlx_migrations).
CREATE TEMP TABLE fk_guard (n INTEGER CHECK (n = 0));
INSERT INTO fk_guard SELECT count(*) FROM pragma_foreign_key_check;
DROP TABLE temp.fk_guard;
