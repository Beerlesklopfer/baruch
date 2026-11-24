-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Gedankenkarte (Runde 25, Vertrag Abschnitt 1; S1 schreibt sie, danach gehört sie karte-daten):
-- Verbindungen zwischen Einfällen (`idea_link`) und gespeicherte Lagen auf der Karte (`idea_map_pos`).
--
-- - `idea_link`: gerichtete Verbindung Einfall → Einfall mit Art und Notiz; „widerspricht“ ist ungerichtet und
--   normalisiert (from_id < to_id). Weich gelöscht über `deleted` (wiederherstellbar), nie hart – darum entsteht
--   ein Grabstein 'idea_link' praktisch nur über die Kaskade.
-- - `idea_map_pos`: je (Werk, Einfall) höchstens eine Lage in SVG-Benutzereinheiten der Karte (Werk bei 0,0);
--   reine Anzeige, kein Spiegel-Commit. „Anordnung zurücksetzen“ löscht hart → Grabstein 'idea_map_pos'.
-- - Beide Tabellen tragen `uuid` wie in 0041 (Spalte, eindeutiger Index, Fülltrigger, Grabstein-Trigger); neue
--   Zeilen bekommen ihre uuid nur über den Fülltrigger, nur `exchange_apply` setzt eine.
-- - `uuid_tombstone` bekommt die Arten 'idea_link' und 'idea_map_pos'. SQLite kann einen CHECK nicht ändern,
--   darum Umbau der Tabelle: (1) die zehn Grabstein-Trigger aus 0041 droppen (sie nennen uuid_tombstone, das
--   zwischendurch fehlt), (2) Zeilen in eine temporäre Kopie, alte Tabelle droppen, neu mit erweitertem CHECK
--   anlegen, Zeilen zurück, Index neu, (3) die zehn Trigger wörtlich wie in 0041 neu, (4) die neuen Tabellen und
--   Trigger. Die Trigger aus 0042 nennen uuid_tombstone nicht und bleiben. Auf uuid_tombstone zeigt kein
--   Fremdschlüssel, darum nicht in FOREIGN_KEYS_OFF.
-- - `work_map_pos_cascade_uuid_tombstone` (wie 0042): Vor einem Werk seine Lagen löschen, solange die Werkzeile
--   noch steht, damit der Grabstein der Lage die Werk-uuid kennt.
-- - `idea_link_cascade_uuid_tombstone` (Integrator bei der Zusammenführung, Befund karte-widget): Vor einem hart
--   gelöschten Einfall seine Verbindungen löschen, solange die Einfallzeile noch steht; sonst fände der
--   Grabstein-Trigger einer Verbindung, deren `from_id` der Einfall ist, sein Werk nicht mehr (work_uuid NULL).
--
-- Reserviert (Runde 25): 0044 karte-widget, 0045 karte-bild, 0046 Integrator (erwartet unbenutzt).

-- (1) Grabstein-Trigger aus 0041 vorübergehend entfernen.
DROP TRIGGER work_uuid_tombstone;
DROP TRIGGER node_uuid_tombstone;
DROP TRIGGER revision_uuid_tombstone;
DROP TRIGGER work_template_revision_uuid_tombstone;
DROP TRIGGER post_uuid_tombstone;
DROP TRIGGER idea_uuid_tombstone;
DROP TRIGGER chat_thread_uuid_tombstone;
DROP TRIGGER chat_thread_post_uuid_tombstone;
DROP TRIGGER asset_uuid_tombstone;
DROP TRIGGER asset_revision_uuid_tombstone;

-- (2) uuid_tombstone mit den neuen Arten umbauen.
-- Über eine temporäre Kopie statt ALTER TABLE … RENAME: Das Umbenennen schriebe den Namen in Anführungszeichen
-- in sqlite_master (CREATE TABLE "uuid_tombstone"); so bleibt der Text bis auf den CHECK wie in 0041.
CREATE TEMP TABLE uuid_tombstone_copy AS SELECT uuid, kind, work_uuid, deleted FROM uuid_tombstone;
DROP TABLE uuid_tombstone;
CREATE TABLE uuid_tombstone (
  uuid      TEXT PRIMARY KEY,
  kind      TEXT NOT NULL CHECK (kind IN ('work','node','revision','template_revision','post','idea','thread',
                                          'thread_post','asset','asset_revision','idea_link','idea_map_pos')),
  work_uuid TEXT,
  deleted   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
INSERT INTO uuid_tombstone (uuid, kind, work_uuid, deleted) SELECT uuid, kind, work_uuid, deleted FROM temp.uuid_tombstone_copy;
DROP TABLE temp.uuid_tombstone_copy;
CREATE INDEX uuid_tombstone_work ON uuid_tombstone(work_uuid);

-- (3) Die zehn Grabstein-Trigger wörtlich wie in 0041.
CREATE TRIGGER work_uuid_tombstone AFTER DELETE ON work WHEN OLD.uuid IS NOT NULL
BEGIN INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid) VALUES (OLD.uuid, 'work', OLD.uuid); END;
CREATE TRIGGER node_uuid_tombstone AFTER DELETE ON node WHEN OLD.uuid IS NOT NULL
BEGIN INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid) VALUES (OLD.uuid, 'node', (SELECT uuid FROM work WHERE id = OLD.work_id)); END;
CREATE TRIGGER revision_uuid_tombstone AFTER DELETE ON revision WHEN OLD.uuid IS NOT NULL
BEGIN INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid) VALUES (OLD.uuid, 'revision', (SELECT w.uuid FROM node n JOIN work w ON w.id = n.work_id WHERE n.id = OLD.node_id)); END;
CREATE TRIGGER work_template_revision_uuid_tombstone AFTER DELETE ON work_template_revision WHEN OLD.uuid IS NOT NULL
BEGIN INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid) VALUES (OLD.uuid, 'template_revision', (SELECT uuid FROM work WHERE id = OLD.work_id)); END;
CREATE TRIGGER post_uuid_tombstone AFTER DELETE ON post WHEN OLD.uuid IS NOT NULL
BEGIN INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid) VALUES (OLD.uuid, 'post', (SELECT w.uuid FROM node n JOIN work w ON w.id = n.work_id WHERE n.id = OLD.node_id)); END;
CREATE TRIGGER idea_uuid_tombstone AFTER DELETE ON idea WHEN OLD.uuid IS NOT NULL
BEGIN INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid) VALUES (OLD.uuid, 'idea', (SELECT uuid FROM work WHERE id = OLD.work_id)); END;
CREATE TRIGGER chat_thread_uuid_tombstone AFTER DELETE ON chat_thread WHEN OLD.uuid IS NOT NULL
BEGIN INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid) VALUES (OLD.uuid, 'thread', (SELECT uuid FROM work WHERE id = OLD.work_id)); END;
CREATE TRIGGER chat_thread_post_uuid_tombstone AFTER DELETE ON chat_thread_post WHEN OLD.uuid IS NOT NULL
BEGIN INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid) VALUES (OLD.uuid, 'thread_post', (SELECT w.uuid FROM chat_thread t JOIN work w ON w.id = t.work_id WHERE t.id = OLD.thread_id)); END;
CREATE TRIGGER asset_uuid_tombstone AFTER DELETE ON asset WHEN OLD.uuid IS NOT NULL
BEGIN INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid) VALUES (OLD.uuid, 'asset', (SELECT uuid FROM work WHERE id = OLD.work_id)); END;
CREATE TRIGGER asset_revision_uuid_tombstone AFTER DELETE ON asset_revision WHEN OLD.uuid IS NOT NULL
BEGIN INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid) VALUES (OLD.uuid, 'asset_revision', (SELECT w.uuid FROM asset a JOIN work w ON w.id = a.work_id WHERE a.id = OLD.asset_id)); END;

-- (4) Verbindungen und Lagen.
CREATE TABLE idea_link (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  from_id INTEGER NOT NULL REFERENCES idea(id) ON DELETE CASCADE,
  to_id   INTEGER NOT NULL REFERENCES idea(id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK (kind IN ('leads_to','belongs_to','example_of','contradicts')),
  note TEXT NOT NULL DEFAULT '' CHECK (length(note) <= 500),
  origin_kind TEXT NOT NULL CHECK (origin_kind IN ('human','persona','mcp')),
  origin_name TEXT NOT NULL,
  created TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  deleted TEXT,                                   -- weich gelöscht (wiederherstellbar)
  uuid TEXT CHECK (uuid IS NULL OR (length(uuid) = 36 AND uuid = lower(uuid))),
  CHECK (from_id <> to_id),
  CHECK (kind <> 'contradicts' OR from_id < to_id),   -- „widerspricht“ ist ungerichtet, normalisiert
  UNIQUE (from_id, to_id, kind)
);
CREATE INDEX idea_link_to ON idea_link(to_id);
CREATE TABLE idea_map_pos (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  work_id INTEGER NOT NULL REFERENCES work(id) ON DELETE CASCADE,
  idea_id INTEGER NOT NULL REFERENCES idea(id) ON DELETE CASCADE,
  x REAL NOT NULL CHECK (x BETWEEN -1000000 AND 1000000),
  y REAL NOT NULL CHECK (y BETWEEN -1000000 AND 1000000),
  updated TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  uuid TEXT CHECK (uuid IS NULL OR (length(uuid) = 36 AND uuid = lower(uuid))),
  UNIQUE (work_id, idea_id)
);

-- idea_link (idea_link); Werk des Grabsteins = Werk des Einfalls from_id
CREATE UNIQUE INDEX idea_link_uuid ON idea_link(uuid);
CREATE TRIGGER idea_link_uuid_fill AFTER INSERT ON idea_link WHEN NEW.uuid IS NULL
BEGIN UPDATE idea_link SET uuid = lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' || substr(lower(hex(randomblob(2))),2) || '-' || substr('89ab', 1 + (abs(random()) % 4), 1) || substr(lower(hex(randomblob(2))),2) || '-' || lower(hex(randomblob(6))) WHERE rowid = NEW.rowid; END;
CREATE TRIGGER idea_link_uuid_tombstone AFTER DELETE ON idea_link WHEN OLD.uuid IS NOT NULL
BEGIN INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid) VALUES (OLD.uuid, 'idea_link', (SELECT w.uuid FROM idea i JOIN work w ON w.id = i.work_id WHERE i.id = OLD.from_id)); END;

-- idea_map_pos (idea_map_pos); Werk des Grabsteins = work_id
CREATE UNIQUE INDEX idea_map_pos_uuid ON idea_map_pos(uuid);
CREATE TRIGGER idea_map_pos_uuid_fill AFTER INSERT ON idea_map_pos WHEN NEW.uuid IS NULL
BEGIN UPDATE idea_map_pos SET uuid = lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' || substr(lower(hex(randomblob(2))),2) || '-' || substr('89ab', 1 + (abs(random()) % 4), 1) || substr(lower(hex(randomblob(2))),2) || '-' || lower(hex(randomblob(6))) WHERE rowid = NEW.rowid; END;
CREATE TRIGGER idea_map_pos_uuid_tombstone AFTER DELETE ON idea_map_pos WHEN OLD.uuid IS NOT NULL
BEGIN INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid) VALUES (OLD.uuid, 'idea_map_pos', (SELECT uuid FROM work WHERE id = OLD.work_id)); END;

-- Vor einem Werk seine Lagen löschen (wie 0042), damit deren Grabsteine die Werk-uuid kennen.
CREATE TRIGGER work_map_pos_cascade_uuid_tombstone BEFORE DELETE ON work
BEGIN DELETE FROM idea_map_pos WHERE work_id = OLD.id; END;

CREATE TRIGGER idea_link_cascade_uuid_tombstone BEFORE DELETE ON idea
BEGIN DELETE FROM idea_link WHERE from_id = OLD.id OR to_id = OLD.id; END;
