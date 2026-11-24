-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Satz-Einstellungen je Werk (Runde 29, Vertrag §3; Aufgabe satz-daten).
--
-- - `work_typeset`: höchstens eine Zeile je Werk (Primärschlüssel `work_id`); keine Zeile = keine
--   Satz-Einstellungen (alles aus der Werkvorlage). `settings` ist das kanonische JSON von
--   `baruch_typst::typeset::TypesetSettings::to_json` (nur gesetzte Felder). Die genaue Werteprüfung macht
--   `TypesetSettings::validate` im Code; die CHECKs sind die äußere Grenze (JSON-Objekt, höchstens 16384 Zeichen).
--   `updated`/`updated_by`: letzte Änderung (Zeitpunkt, Name des Menschen).
-- - `uuid` wie in 0041/0045/0046 (Spalte, eindeutiger Index, Fülltrigger, Grabstein-Trigger); nur `exchange_apply`
--   setzt eine. Ein Ändern der Einstellungen behält die Zeile und damit die uuid; Zurücksetzen löscht sie
--   (Grabstein der Art 'work_typeset').
-- - `uuid_tombstone` bekommt die Art 'work_typeset'. SQLite kann einen CHECK nicht ändern, darum wie in 0046:
--   (1) die sechzehn Grabstein-Trigger droppen, die uuid_tombstone nennen (zehn aus 0041, idea_link und idea_map_pos
--   aus 0043, cover_block aus 0045, finding, finding_status und post_version aus 0046), (2) Zeilen in eine temporäre
--   Kopie, Tabelle neu mit erweitertem CHECK, Zeilen zurück, Index neu, (3) die sechzehn Trigger wörtlich neu. Die
--   Trigger aus 0042 und die Kaskaden-Trigger aus 0043/0045/0046 nennen uuid_tombstone nicht und bleiben.
-- - Kaskaden-Trigger (wie 0042/0045/0046): vor einem Werk seine Satz-Einstellungen löschen, solange die Werkzeile
--   noch steht – so kennt der Grabstein die Werk-uuid.
--
-- Kein FOREIGN_KEYS_OFF: Umgebaut wird nur uuid_tombstone (kein Fremdschlüssel zeigt darauf); das implizite DELETE
-- von DROP TABLE löst darum keine Aktion aus (Beleg in tests/migration_0048.rs).
--
-- Reserviert (Runde 29): Store 0049 satz-typst, 0050 satz-oberflaeche, 0051 Integrator; Bibliothek 0108 satz-daten,
-- 0109 Integrator (alle erwartet unbenutzt).

-- (1) Grabstein-Trigger, die uuid_tombstone nennen, vorübergehend entfernen.
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
DROP TRIGGER idea_link_uuid_tombstone;
DROP TRIGGER idea_map_pos_uuid_tombstone;
DROP TRIGGER cover_block_uuid_tombstone;
DROP TRIGGER finding_uuid_tombstone;
DROP TRIGGER finding_status_uuid_tombstone;
DROP TRIGGER post_version_uuid_tombstone;

-- (2) uuid_tombstone mit der neuen Art umbauen (temporäre Kopie wie in 0046).
CREATE TEMP TABLE uuid_tombstone_copy AS SELECT uuid, kind, work_uuid, deleted FROM uuid_tombstone;
DROP TABLE uuid_tombstone;
CREATE TABLE uuid_tombstone (
  uuid      TEXT PRIMARY KEY,
  kind      TEXT NOT NULL CHECK (kind IN ('work','node','revision','template_revision','post','idea','thread',
                                          'thread_post','asset','asset_revision','idea_link','idea_map_pos',
                                          'cover_block',
                                          'finding','finding_status','post_version',
                                          'work_typeset')),
  work_uuid TEXT,
  deleted   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
INSERT INTO uuid_tombstone (uuid, kind, work_uuid, deleted) SELECT uuid, kind, work_uuid, deleted FROM temp.uuid_tombstone_copy;
DROP TABLE temp.uuid_tombstone_copy;
CREATE INDEX uuid_tombstone_work ON uuid_tombstone(work_uuid);

-- (3) Die sechzehn Grabstein-Trigger wörtlich wie in 0041, 0043, 0045 bzw. 0046.
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
CREATE TRIGGER idea_link_uuid_tombstone AFTER DELETE ON idea_link WHEN OLD.uuid IS NOT NULL
BEGIN INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid) VALUES (OLD.uuid, 'idea_link', (SELECT w.uuid FROM idea i JOIN work w ON w.id = i.work_id WHERE i.id = OLD.from_id)); END;
CREATE TRIGGER idea_map_pos_uuid_tombstone AFTER DELETE ON idea_map_pos WHEN OLD.uuid IS NOT NULL
BEGIN INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid) VALUES (OLD.uuid, 'idea_map_pos', (SELECT uuid FROM work WHERE id = OLD.work_id)); END;
CREATE TRIGGER cover_block_uuid_tombstone AFTER DELETE ON cover_block WHEN OLD.uuid IS NOT NULL
BEGIN INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid) VALUES (OLD.uuid, 'cover_block', (SELECT uuid FROM work WHERE id = OLD.work_id)); END;
CREATE TRIGGER finding_uuid_tombstone AFTER DELETE ON finding WHEN OLD.uuid IS NOT NULL
BEGIN INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid) VALUES (OLD.uuid, 'finding', (SELECT uuid FROM work WHERE id = OLD.work_id)); END;
CREATE TRIGGER finding_status_uuid_tombstone AFTER DELETE ON finding_status WHEN OLD.uuid IS NOT NULL
BEGIN INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid) VALUES (OLD.uuid, 'finding_status', (SELECT w.uuid FROM finding f JOIN work w ON w.id = f.work_id WHERE f.id = OLD.finding_id)); END;
CREATE TRIGGER post_version_uuid_tombstone AFTER DELETE ON post_version WHEN OLD.uuid IS NOT NULL
BEGIN INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid) VALUES (OLD.uuid, 'post_version', coalesce((SELECT w.uuid FROM post p JOIN node n ON n.id = p.node_id JOIN work w ON w.id = n.work_id WHERE p.id = OLD.post_id), (SELECT w.uuid FROM chat_thread_post tp JOIN chat_thread t ON t.id = tp.thread_id JOIN work w ON w.id = t.work_id WHERE tp.id = OLD.thread_post_id))); END;

-- (4) Satz-Einstellungen je Werk.
CREATE TABLE work_typeset (
  work_id    INTEGER PRIMARY KEY REFERENCES work(id) ON DELETE CASCADE,
  uuid       TEXT CHECK (uuid IS NULL OR (length(uuid) = 36 AND uuid = lower(uuid))),
  settings   TEXT NOT NULL CHECK (json_valid(settings) AND json_type(settings) = 'object' AND length(settings) <= 16384),
  updated    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated_by TEXT NOT NULL DEFAULT '' CHECK (length(updated_by) <= 200)
);
CREATE UNIQUE INDEX work_typeset_uuid ON work_typeset(uuid);

-- (5) Fülltrigger und Grabstein (Art 'work_typeset'; Werk des Grabsteins = work_id).
CREATE TRIGGER work_typeset_uuid_fill AFTER INSERT ON work_typeset WHEN NEW.uuid IS NULL
BEGIN UPDATE work_typeset SET uuid = lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' || substr(lower(hex(randomblob(2))),2) || '-' || substr('89ab', 1 + (abs(random()) % 4), 1) || substr(lower(hex(randomblob(2))),2) || '-' || lower(hex(randomblob(6))) WHERE rowid = NEW.rowid; END;
CREATE TRIGGER work_typeset_uuid_tombstone AFTER DELETE ON work_typeset WHEN OLD.uuid IS NOT NULL
BEGIN INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid) VALUES (OLD.uuid, 'work_typeset', (SELECT uuid FROM work WHERE id = OLD.work_id)); END;

-- Satz-Einstellungen vor ihrem Werk löschen (wie 0042/0045/0046), damit der Grabstein die Werk-uuid kennt.
CREATE TRIGGER work_typeset_cascade_uuid_tombstone BEFORE DELETE ON work
BEGIN DELETE FROM work_typeset WHERE work_id = OLD.id; END;
