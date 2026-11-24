-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Weltweit eindeutige Kennungen für das Austauschformat (Runde 26, Vertrag Abschnitt 3; S1 schreibt sie, danach
-- gehört sie ids). Kein Umbau einer Tabelle, darum nicht in FOREIGN_KEYS_OFF.
--
-- - Je Tabelle mit Werkinhalt eine Spalte `uuid` (kanonisch klein, 36 Zeichen), vorhandene Zeilen bekommen
--   eine zufällige v4, neue Zeilen über den Trigger `<T>_uuid_fill` ebenso, falls der Code keine setzt.
-- - Eindeutiger Index `<T>_uuid` je Tabelle.
-- - Grabsteine: Wer eine Zeile mit uuid löscht, hinterlässt einen Eintrag in `uuid_tombstone` (Trigger
--   `<T>_uuid_tombstone`) mit Art und Werk-uuid (darf NULL sein, z. B. wenn das Werk schon gelöscht ist oder
--   ein Einfall ohne Werk).
-- - Instanzkennung in `store_meta.instance_uuid` (der Name `instance_name` ist leer, solange er fehlt).
--
-- Keine Sperre gegen das Ändern der uuid per Trigger: Trigger im Hauptschema sehen keine temp-Tabellen, und das
-- Einlesen muss die uuid eines eben angelegten Punkts in derselben Transaktion setzen. Regel im Code: Nur
-- `exchange_apply` setzt eine uuid, und nur auf einer Zeile, die es in derselben Transaktion angelegt hat.
-- Spätere Migrationen, die eine dieser Tabellen umbauen, müssen Spalte, Index und beide Trigger mitnehmen.
--
-- V4 (zufällige UUID Version 4, Variante 10):
--   lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' || substr(lower(hex(randomblob(2))),2)
--   || '-' || substr('89ab', 1 + (abs(random()) % 4), 1) || substr(lower(hex(randomblob(2))),2) || '-'
--   || lower(hex(randomblob(6)))
--
-- Reservierte Nummern der Runde 26: 0042 (ids, nur Nachbesserung), 0043 (import), 0044 (export).

CREATE TABLE uuid_tombstone (
  uuid      TEXT PRIMARY KEY,
  kind      TEXT NOT NULL CHECK (kind IN ('work','node','revision','template_revision','post','idea','thread',
                                          'thread_post','asset','asset_revision')),
  work_uuid TEXT,
  deleted   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX uuid_tombstone_work ON uuid_tombstone(work_uuid);

-- work (work)
ALTER TABLE work ADD COLUMN uuid TEXT CHECK (uuid IS NULL OR (length(uuid) = 36 AND uuid = lower(uuid)));
UPDATE work SET uuid = lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' || substr(lower(hex(randomblob(2))),2) || '-' || substr('89ab', 1 + (abs(random()) % 4), 1) || substr(lower(hex(randomblob(2))),2) || '-' || lower(hex(randomblob(6))) WHERE uuid IS NULL;
CREATE UNIQUE INDEX work_uuid ON work(uuid);
CREATE TRIGGER work_uuid_fill AFTER INSERT ON work WHEN NEW.uuid IS NULL
BEGIN UPDATE work SET uuid = lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' || substr(lower(hex(randomblob(2))),2) || '-' || substr('89ab', 1 + (abs(random()) % 4), 1) || substr(lower(hex(randomblob(2))),2) || '-' || lower(hex(randomblob(6))) WHERE rowid = NEW.rowid; END;
CREATE TRIGGER work_uuid_tombstone AFTER DELETE ON work WHEN OLD.uuid IS NOT NULL
BEGIN INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid) VALUES (OLD.uuid, 'work', OLD.uuid); END;

-- node (node)
ALTER TABLE node ADD COLUMN uuid TEXT CHECK (uuid IS NULL OR (length(uuid) = 36 AND uuid = lower(uuid)));
UPDATE node SET uuid = lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' || substr(lower(hex(randomblob(2))),2) || '-' || substr('89ab', 1 + (abs(random()) % 4), 1) || substr(lower(hex(randomblob(2))),2) || '-' || lower(hex(randomblob(6))) WHERE uuid IS NULL;
CREATE UNIQUE INDEX node_uuid ON node(uuid);
CREATE TRIGGER node_uuid_fill AFTER INSERT ON node WHEN NEW.uuid IS NULL
BEGIN UPDATE node SET uuid = lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' || substr(lower(hex(randomblob(2))),2) || '-' || substr('89ab', 1 + (abs(random()) % 4), 1) || substr(lower(hex(randomblob(2))),2) || '-' || lower(hex(randomblob(6))) WHERE rowid = NEW.rowid; END;
CREATE TRIGGER node_uuid_tombstone AFTER DELETE ON node WHEN OLD.uuid IS NOT NULL
BEGIN INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid) VALUES (OLD.uuid, 'node', (SELECT uuid FROM work WHERE id = OLD.work_id)); END;

-- revision (revision)
ALTER TABLE revision ADD COLUMN uuid TEXT CHECK (uuid IS NULL OR (length(uuid) = 36 AND uuid = lower(uuid)));
UPDATE revision SET uuid = lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' || substr(lower(hex(randomblob(2))),2) || '-' || substr('89ab', 1 + (abs(random()) % 4), 1) || substr(lower(hex(randomblob(2))),2) || '-' || lower(hex(randomblob(6))) WHERE uuid IS NULL;
CREATE UNIQUE INDEX revision_uuid ON revision(uuid);
CREATE TRIGGER revision_uuid_fill AFTER INSERT ON revision WHEN NEW.uuid IS NULL
BEGIN UPDATE revision SET uuid = lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' || substr(lower(hex(randomblob(2))),2) || '-' || substr('89ab', 1 + (abs(random()) % 4), 1) || substr(lower(hex(randomblob(2))),2) || '-' || lower(hex(randomblob(6))) WHERE rowid = NEW.rowid; END;
CREATE TRIGGER revision_uuid_tombstone AFTER DELETE ON revision WHEN OLD.uuid IS NOT NULL
BEGIN INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid) VALUES (OLD.uuid, 'revision', (SELECT w.uuid FROM node n JOIN work w ON w.id = n.work_id WHERE n.id = OLD.node_id)); END;

-- work_template_revision (template_revision)
ALTER TABLE work_template_revision ADD COLUMN uuid TEXT CHECK (uuid IS NULL OR (length(uuid) = 36 AND uuid = lower(uuid)));
UPDATE work_template_revision SET uuid = lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' || substr(lower(hex(randomblob(2))),2) || '-' || substr('89ab', 1 + (abs(random()) % 4), 1) || substr(lower(hex(randomblob(2))),2) || '-' || lower(hex(randomblob(6))) WHERE uuid IS NULL;
CREATE UNIQUE INDEX work_template_revision_uuid ON work_template_revision(uuid);
CREATE TRIGGER work_template_revision_uuid_fill AFTER INSERT ON work_template_revision WHEN NEW.uuid IS NULL
BEGIN UPDATE work_template_revision SET uuid = lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' || substr(lower(hex(randomblob(2))),2) || '-' || substr('89ab', 1 + (abs(random()) % 4), 1) || substr(lower(hex(randomblob(2))),2) || '-' || lower(hex(randomblob(6))) WHERE rowid = NEW.rowid; END;
CREATE TRIGGER work_template_revision_uuid_tombstone AFTER DELETE ON work_template_revision WHEN OLD.uuid IS NOT NULL
BEGIN INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid) VALUES (OLD.uuid, 'template_revision', (SELECT uuid FROM work WHERE id = OLD.work_id)); END;

-- post (post)
ALTER TABLE post ADD COLUMN uuid TEXT CHECK (uuid IS NULL OR (length(uuid) = 36 AND uuid = lower(uuid)));
UPDATE post SET uuid = lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' || substr(lower(hex(randomblob(2))),2) || '-' || substr('89ab', 1 + (abs(random()) % 4), 1) || substr(lower(hex(randomblob(2))),2) || '-' || lower(hex(randomblob(6))) WHERE uuid IS NULL;
CREATE UNIQUE INDEX post_uuid ON post(uuid);
CREATE TRIGGER post_uuid_fill AFTER INSERT ON post WHEN NEW.uuid IS NULL
BEGIN UPDATE post SET uuid = lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' || substr(lower(hex(randomblob(2))),2) || '-' || substr('89ab', 1 + (abs(random()) % 4), 1) || substr(lower(hex(randomblob(2))),2) || '-' || lower(hex(randomblob(6))) WHERE rowid = NEW.rowid; END;
CREATE TRIGGER post_uuid_tombstone AFTER DELETE ON post WHEN OLD.uuid IS NOT NULL
BEGIN INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid) VALUES (OLD.uuid, 'post', (SELECT w.uuid FROM node n JOIN work w ON w.id = n.work_id WHERE n.id = OLD.node_id)); END;

-- idea (idea)
ALTER TABLE idea ADD COLUMN uuid TEXT CHECK (uuid IS NULL OR (length(uuid) = 36 AND uuid = lower(uuid)));
UPDATE idea SET uuid = lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' || substr(lower(hex(randomblob(2))),2) || '-' || substr('89ab', 1 + (abs(random()) % 4), 1) || substr(lower(hex(randomblob(2))),2) || '-' || lower(hex(randomblob(6))) WHERE uuid IS NULL;
CREATE UNIQUE INDEX idea_uuid ON idea(uuid);
CREATE TRIGGER idea_uuid_fill AFTER INSERT ON idea WHEN NEW.uuid IS NULL
BEGIN UPDATE idea SET uuid = lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' || substr(lower(hex(randomblob(2))),2) || '-' || substr('89ab', 1 + (abs(random()) % 4), 1) || substr(lower(hex(randomblob(2))),2) || '-' || lower(hex(randomblob(6))) WHERE rowid = NEW.rowid; END;
CREATE TRIGGER idea_uuid_tombstone AFTER DELETE ON idea WHEN OLD.uuid IS NOT NULL
BEGIN INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid) VALUES (OLD.uuid, 'idea', (SELECT uuid FROM work WHERE id = OLD.work_id)); END;

-- chat_thread (thread)
ALTER TABLE chat_thread ADD COLUMN uuid TEXT CHECK (uuid IS NULL OR (length(uuid) = 36 AND uuid = lower(uuid)));
UPDATE chat_thread SET uuid = lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' || substr(lower(hex(randomblob(2))),2) || '-' || substr('89ab', 1 + (abs(random()) % 4), 1) || substr(lower(hex(randomblob(2))),2) || '-' || lower(hex(randomblob(6))) WHERE uuid IS NULL;
CREATE UNIQUE INDEX chat_thread_uuid ON chat_thread(uuid);
CREATE TRIGGER chat_thread_uuid_fill AFTER INSERT ON chat_thread WHEN NEW.uuid IS NULL
BEGIN UPDATE chat_thread SET uuid = lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' || substr(lower(hex(randomblob(2))),2) || '-' || substr('89ab', 1 + (abs(random()) % 4), 1) || substr(lower(hex(randomblob(2))),2) || '-' || lower(hex(randomblob(6))) WHERE rowid = NEW.rowid; END;
CREATE TRIGGER chat_thread_uuid_tombstone AFTER DELETE ON chat_thread WHEN OLD.uuid IS NOT NULL
BEGIN INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid) VALUES (OLD.uuid, 'thread', (SELECT uuid FROM work WHERE id = OLD.work_id)); END;

-- chat_thread_post (thread_post)
ALTER TABLE chat_thread_post ADD COLUMN uuid TEXT CHECK (uuid IS NULL OR (length(uuid) = 36 AND uuid = lower(uuid)));
UPDATE chat_thread_post SET uuid = lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' || substr(lower(hex(randomblob(2))),2) || '-' || substr('89ab', 1 + (abs(random()) % 4), 1) || substr(lower(hex(randomblob(2))),2) || '-' || lower(hex(randomblob(6))) WHERE uuid IS NULL;
CREATE UNIQUE INDEX chat_thread_post_uuid ON chat_thread_post(uuid);
CREATE TRIGGER chat_thread_post_uuid_fill AFTER INSERT ON chat_thread_post WHEN NEW.uuid IS NULL
BEGIN UPDATE chat_thread_post SET uuid = lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' || substr(lower(hex(randomblob(2))),2) || '-' || substr('89ab', 1 + (abs(random()) % 4), 1) || substr(lower(hex(randomblob(2))),2) || '-' || lower(hex(randomblob(6))) WHERE rowid = NEW.rowid; END;
CREATE TRIGGER chat_thread_post_uuid_tombstone AFTER DELETE ON chat_thread_post WHEN OLD.uuid IS NOT NULL
BEGIN INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid) VALUES (OLD.uuid, 'thread_post', (SELECT w.uuid FROM chat_thread t JOIN work w ON w.id = t.work_id WHERE t.id = OLD.thread_id)); END;

-- asset (asset)
ALTER TABLE asset ADD COLUMN uuid TEXT CHECK (uuid IS NULL OR (length(uuid) = 36 AND uuid = lower(uuid)));
UPDATE asset SET uuid = lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' || substr(lower(hex(randomblob(2))),2) || '-' || substr('89ab', 1 + (abs(random()) % 4), 1) || substr(lower(hex(randomblob(2))),2) || '-' || lower(hex(randomblob(6))) WHERE uuid IS NULL;
CREATE UNIQUE INDEX asset_uuid ON asset(uuid);
CREATE TRIGGER asset_uuid_fill AFTER INSERT ON asset WHEN NEW.uuid IS NULL
BEGIN UPDATE asset SET uuid = lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' || substr(lower(hex(randomblob(2))),2) || '-' || substr('89ab', 1 + (abs(random()) % 4), 1) || substr(lower(hex(randomblob(2))),2) || '-' || lower(hex(randomblob(6))) WHERE rowid = NEW.rowid; END;
CREATE TRIGGER asset_uuid_tombstone AFTER DELETE ON asset WHEN OLD.uuid IS NOT NULL
BEGIN INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid) VALUES (OLD.uuid, 'asset', (SELECT uuid FROM work WHERE id = OLD.work_id)); END;

-- asset_revision (asset_revision)
ALTER TABLE asset_revision ADD COLUMN uuid TEXT CHECK (uuid IS NULL OR (length(uuid) = 36 AND uuid = lower(uuid)));
UPDATE asset_revision SET uuid = lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' || substr(lower(hex(randomblob(2))),2) || '-' || substr('89ab', 1 + (abs(random()) % 4), 1) || substr(lower(hex(randomblob(2))),2) || '-' || lower(hex(randomblob(6))) WHERE uuid IS NULL;
CREATE UNIQUE INDEX asset_revision_uuid ON asset_revision(uuid);
CREATE TRIGGER asset_revision_uuid_fill AFTER INSERT ON asset_revision WHEN NEW.uuid IS NULL
BEGIN UPDATE asset_revision SET uuid = lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' || substr(lower(hex(randomblob(2))),2) || '-' || substr('89ab', 1 + (abs(random()) % 4), 1) || substr(lower(hex(randomblob(2))),2) || '-' || lower(hex(randomblob(6))) WHERE rowid = NEW.rowid; END;
CREATE TRIGGER asset_revision_uuid_tombstone AFTER DELETE ON asset_revision WHEN OLD.uuid IS NOT NULL
BEGIN INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid) VALUES (OLD.uuid, 'asset_revision', (SELECT w.uuid FROM asset a JOIN work w ON w.id = a.work_id WHERE a.id = OLD.asset_id)); END;

-- Kennung dieser Instanz (der Name bleibt leer, bis der Mensch einen setzt).
INSERT OR IGNORE INTO store_meta (key, value) VALUES ('instance_uuid', lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' || substr(lower(hex(randomblob(2))),2) || '-' || substr('89ab', 1 + (abs(random()) % 4), 1) || substr(lower(hex(randomblob(2))),2) || '-' || lower(hex(randomblob(6))));
