-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Fortschreiben der aktuellen Fassung, Einstellung „Fassungen“ je Werk, Stand übernommener Befunde (Runde 22,
-- Vertrag §2/§3.1; Schnittstelle R22-S1, danach Besitz fassungen; Spalten und Arten ab S1 eingefroren).
--
-- - `revision.amend_seq`: Stand der Fassung (0 = nie fortgeschrieben, jede Fortschreibung bzw. Rücknahme +1).
--   `revision.based_on_amend`: Stand der Basis, auf dem ein Vorschlag beruht (NULL = ohne bzw. vor Runde 22).
-- - `revision_amend`: je Schritt eine Zeile (Art 'amend' = Fortschreibung des Menschen, 'undo' = Rücknahme über den
--   Verlauf mit `undo_of` = Schritt, dessen Stand vorher wiederhergestellt wurde), Text vorher und nachher, Mensch,
--   Zeit. Verlaufszeilen werden nie einzeln gelöscht, nur mit ihrer Fassung. uuid wie 0041/0045/0046/0048/0049:
--   Spalte, eindeutiger Index, Fülltrigger, Grabstein-Trigger (Art 'revision_amend', Werk über revision→node→work)
--   und Kaskaden-Trigger BEFORE DELETE ON revision (so kennt der Grabstein die Werk-uuid noch).
-- - `work_revision_policy`: Einstellung je Werk (fehlt die Zeile → Vorgabe: an, 20 %, 30 Minuten). Arbeitsweise der
--   Instanz wie Anträge: ohne uuid, nicht im Austauschformat.
-- - `finding.adopted_amend`, `finding_status.adopted_amend`: Stand der Fassung, als ein Befund übernommen wurde.
-- - `uuid_tombstone` bekommt die Art 'revision_amend'. SQLite kann einen CHECK nicht ändern, darum wie in 0049:
--   (1) die neunzehn Grabstein-Trigger droppen, die uuid_tombstone nennen (sechzehn aus 0048, work_typeset,
--   glossary_term, index_register), (2) Zeilen in eine temporäre Kopie, Tabelle neu mit erweitertem CHECK, Zeilen
--   zurück, Index neu, (3) die neunzehn Trigger wörtlich neu. Die Kaskaden-Trigger nennen uuid_tombstone nicht und
--   bleiben.
--
-- Kein FOREIGN_KEYS_OFF: Umgebaut wird nur uuid_tombstone (kein Fremdschlüssel zeigt darauf); das implizite DELETE
-- von DROP TABLE löst darum keine Aktion aus. Die übrigen Änderungen sind ADD COLUMN bzw. neue Tabellen.
--
-- Reserviert (Runde 22): Store 0051 fassungen, 0052 befund-rueckfrage, 0053 spiegel-entwurf, 0054 Integrator;
-- Bibliothek 0109 Integrator (alle erwartet unbenutzt).

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
DROP TRIGGER work_typeset_uuid_tombstone;
DROP TRIGGER glossary_term_uuid_tombstone;
DROP TRIGGER index_register_uuid_tombstone;

-- (2) uuid_tombstone mit der neuen Art umbauen (temporäre Kopie wie in 0049).
CREATE TEMP TABLE uuid_tombstone_copy AS SELECT uuid, kind, work_uuid, deleted FROM uuid_tombstone;
DROP TABLE uuid_tombstone;
CREATE TABLE uuid_tombstone (
  uuid      TEXT PRIMARY KEY,
  kind      TEXT NOT NULL CHECK (kind IN ('work','node','revision','template_revision','post','idea','thread',
                                          'thread_post','asset','asset_revision','idea_link','idea_map_pos',
                                          'cover_block',
                                          'finding','finding_status','post_version',
                                          'work_typeset',
                                          'glossary_term','index_register',
                                          'revision_amend')),
  work_uuid TEXT,
  deleted   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
INSERT INTO uuid_tombstone (uuid, kind, work_uuid, deleted) SELECT uuid, kind, work_uuid, deleted FROM temp.uuid_tombstone_copy;
DROP TABLE temp.uuid_tombstone_copy;
CREATE INDEX uuid_tombstone_work ON uuid_tombstone(work_uuid);

-- (3) Die neunzehn Grabstein-Trigger wörtlich wie in 0041, 0043, 0045, 0046, 0048 bzw. 0049.
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
CREATE TRIGGER work_typeset_uuid_tombstone AFTER DELETE ON work_typeset WHEN OLD.uuid IS NOT NULL
BEGIN INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid) VALUES (OLD.uuid, 'work_typeset', (SELECT uuid FROM work WHERE id = OLD.work_id)); END;
CREATE TRIGGER glossary_term_uuid_tombstone AFTER DELETE ON glossary_term WHEN OLD.uuid IS NOT NULL
BEGIN INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid) VALUES (OLD.uuid, 'glossary_term', (SELECT uuid FROM work WHERE id = OLD.work_id)); END;
CREATE TRIGGER index_register_uuid_tombstone AFTER DELETE ON index_register WHEN OLD.uuid IS NOT NULL
BEGIN INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid) VALUES (OLD.uuid, 'index_register', (SELECT uuid FROM work WHERE id = OLD.work_id)); END;

-- (4) Stand der Fassung und Stand der Basis eines Vorschlags.
ALTER TABLE revision ADD COLUMN amend_seq INTEGER NOT NULL DEFAULT 0 CHECK (amend_seq >= 0);
ALTER TABLE revision ADD COLUMN based_on_amend INTEGER CHECK (based_on_amend IS NULL OR based_on_amend >= 0);

-- (5) Verlauf der Fortschreibungen.
CREATE TABLE revision_amend (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  revision_id INTEGER NOT NULL REFERENCES revision(id) ON DELETE CASCADE,
  seq         INTEGER NOT NULL CHECK (seq >= 1),
  kind        TEXT NOT NULL CHECK (kind IN ('amend','undo')),
  undo_of     INTEGER CHECK (undo_of IS NULL OR undo_of >= 1),
  body_before TEXT NOT NULL,
  body_after  TEXT NOT NULL,
  author_name TEXT NOT NULL CHECK (length(author_name) BETWEEN 1 AND 200),
  created     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  uuid        TEXT CHECK (uuid IS NULL OR (length(uuid) = 36 AND uuid = lower(uuid))),
  UNIQUE (revision_id, seq),
  CHECK ((kind = 'undo') = (undo_of IS NOT NULL))
);
CREATE UNIQUE INDEX revision_amend_uuid ON revision_amend(uuid);

CREATE TRIGGER revision_amend_uuid_fill AFTER INSERT ON revision_amend WHEN NEW.uuid IS NULL
BEGIN UPDATE revision_amend SET uuid = lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' || substr(lower(hex(randomblob(2))),2) || '-' || substr('89ab', 1 + (abs(random()) % 4), 1) || substr(lower(hex(randomblob(2))),2) || '-' || lower(hex(randomblob(6))) WHERE rowid = NEW.rowid; END;
CREATE TRIGGER revision_amend_uuid_tombstone AFTER DELETE ON revision_amend WHEN OLD.uuid IS NOT NULL
BEGIN INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid) VALUES (OLD.uuid, 'revision_amend', (SELECT w.uuid FROM revision r JOIN node n ON n.id = r.node_id JOIN work w ON w.id = n.work_id WHERE r.id = OLD.revision_id)); END;

-- Schritte vor ihrer Fassung löschen (wie 0042/0046), damit der Grabstein die Werk-uuid kennt.
CREATE TRIGGER revision_amend_before_revision_delete BEFORE DELETE ON revision
BEGIN DELETE FROM revision_amend WHERE revision_id = OLD.id; END;

-- (6) Einstellung „Fassungen“ je Werk (ohne uuid, nicht im Austausch).
CREATE TABLE work_revision_policy (
  work_id            INTEGER PRIMARY KEY REFERENCES work(id) ON DELETE CASCADE,
  amend              INTEGER NOT NULL DEFAULT 1 CHECK (amend IN (0,1)),
  max_change_percent INTEGER NOT NULL DEFAULT 20 CHECK (max_change_percent BETWEEN 1 AND 100),
  new_after_minutes  INTEGER NOT NULL DEFAULT 30 CHECK (new_after_minutes BETWEEN 0 AND 1440),
  updated            TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated_by         TEXT NOT NULL DEFAULT '' CHECK (length(updated_by) <= 200)
);

-- (7) Stand der Fassung beim Übernehmen eines Befunds.
ALTER TABLE finding ADD COLUMN adopted_amend INTEGER CHECK (adopted_amend IS NULL OR adopted_amend >= 0);
ALTER TABLE finding_status ADD COLUMN adopted_amend INTEGER CHECK (adopted_amend IS NULL OR adopted_amend >= 0);
