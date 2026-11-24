-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Glossar, eigene Register und Registervorschläge je Werk (Runde 31, Vertrag §1; Aufgabe glossar-daten).
--
-- - `glossary_term`: ein Begriff je Zeile, angenommen (`accepted`, vom Menschen) oder vorgeschlagen (`proposed`, von
--   einer Persona bzw. einem MCP-Client; `origin` = Handle). `term_key` = `baruch_typst::index::term_key(term)` (im Code
--   gesetzt, ab S1 unveränderlich); unter den angenommenen Begriffen eines Werks ist er eindeutig (Teilindex).
--   `bible_refs` ist eine JSON-Liste normalisierter Stellen (Prüfung im Server mit baruch_scripture). Die genauen Grenzen
--   (MAX_GLOSSARY_*) prüft der Code; die CHECKs sind die äußere Grenze.
-- - `glossary_variant`: Varianten/Beugungen eines Begriffs in ihrer Reihenfolge (`form_key` wie term_key). Ohne uuid:
--   gehört zum Begriff, geht mit ihm (Fremdschlüssel mit Kaskade).
-- - `index_register`: eigene Register eines Werks; die eingebauten Kennungen sind gesperrt.
-- - `index_proposal`: Registervorschläge (Arbeitsablauf der Instanz, nicht im Austausch, darum ohne uuid – wie offene
--   Anträge). `mark` ist das JSON einer `baruch_typst::index::IndexMark`, `based_on` die Fassungsnummer, auf die sich der
--   Vorschlag bezieht.
-- - uuid für glossary_term und index_register wie 0041/0045/0046/0048: Spalte, eindeutiger Index, Fülltrigger,
--   Grabstein-Trigger, Kaskaden-Trigger BEFORE DELETE ON work (so kennt der Grabstein die Werk-uuid).
-- - `uuid_tombstone` bekommt die Arten 'glossary_term' und 'index_register'. SQLite kann einen CHECK nicht ändern,
--   darum wie in 0048: (1) die siebzehn Grabstein-Trigger droppen, die uuid_tombstone nennen (sechzehn aus 0048 und
--   work_typeset_uuid_tombstone), (2) Zeilen in eine temporäre Kopie, Tabelle neu mit erweitertem CHECK, Zeilen zurück,
--   Index neu, (3) die siebzehn Trigger wörtlich neu. Die Kaskaden-Trigger nennen uuid_tombstone nicht und bleiben.
--
-- Kein FOREIGN_KEYS_OFF: Umgebaut wird nur uuid_tombstone (kein Fremdschlüssel zeigt darauf); das implizite DELETE
-- von DROP TABLE löst darum keine Aktion aus (Beleg in tests/migration_0049.rs).
--
-- Reserviert (Runde 31): Store 0050 register-satz, 0051 kontextmenue, 0052 Integrator; Bibliothek 0108 glossar-daten,
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
DROP TRIGGER work_typeset_uuid_tombstone;

-- (2) uuid_tombstone mit den neuen Arten umbauen (temporäre Kopie wie in 0048).
CREATE TEMP TABLE uuid_tombstone_copy AS SELECT uuid, kind, work_uuid, deleted FROM uuid_tombstone;
DROP TABLE uuid_tombstone;
CREATE TABLE uuid_tombstone (
  uuid      TEXT PRIMARY KEY,
  kind      TEXT NOT NULL CHECK (kind IN ('work','node','revision','template_revision','post','idea','thread',
                                          'thread_post','asset','asset_revision','idea_link','idea_map_pos',
                                          'cover_block',
                                          'finding','finding_status','post_version',
                                          'work_typeset',
                                          'glossary_term','index_register')),
  work_uuid TEXT,
  deleted   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
INSERT INTO uuid_tombstone (uuid, kind, work_uuid, deleted) SELECT uuid, kind, work_uuid, deleted FROM temp.uuid_tombstone_copy;
DROP TABLE temp.uuid_tombstone_copy;
CREATE INDEX uuid_tombstone_work ON uuid_tombstone(work_uuid);

-- (3) Die siebzehn Grabstein-Trigger wörtlich wie in 0041, 0043, 0045, 0046 bzw. 0048.
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

-- (4) Glossarbegriffe.
CREATE TABLE glossary_term (
  id         INTEGER PRIMARY KEY,
  work_id    INTEGER NOT NULL REFERENCES work(id) ON DELETE CASCADE,
  uuid       TEXT CHECK (uuid IS NULL OR (length(uuid) = 36 AND uuid = lower(uuid))),
  term       TEXT NOT NULL CHECK (length(trim(term)) >= 1 AND length(term) <= 200),
  term_key   TEXT NOT NULL CHECK (length(term_key) >= 1),
  short      TEXT NOT NULL DEFAULT '' CHECK (length(short) <= 500),
  long       TEXT NOT NULL DEFAULT '' CHECK (length(long) <= 10000),
  source     TEXT NOT NULL DEFAULT '' CHECK (length(source) <= 500),
  bible_refs TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(bible_refs) AND json_type(bible_refs) = 'array'),
  status     TEXT NOT NULL CHECK (status IN ('accepted','proposed')),
  origin     TEXT NOT NULL CHECK (length(origin) <= 200),
  created    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated    TEXT,
  updated_by TEXT NOT NULL DEFAULT '' CHECK (length(updated_by) <= 200)
);
CREATE UNIQUE INDEX glossary_term_uuid ON glossary_term(uuid);
CREATE UNIQUE INDEX glossary_term_accepted_key ON glossary_term(work_id, term_key) WHERE status = 'accepted';
CREATE INDEX glossary_term_work ON glossary_term(work_id, status);

-- (5) Varianten eines Begriffs.
CREATE TABLE glossary_variant (
  term_id  INTEGER NOT NULL REFERENCES glossary_term(id) ON DELETE CASCADE,
  position INTEGER NOT NULL CHECK (position >= 0),
  form     TEXT NOT NULL CHECK (length(trim(form)) >= 1 AND length(form) <= 200),
  form_key TEXT NOT NULL CHECK (length(form_key) >= 1),
  PRIMARY KEY (term_id, position)
);
CREATE INDEX glossary_variant_form_key ON glossary_variant(form_key);

-- (6) Eigene Register.
CREATE TABLE index_register (
  id       INTEGER PRIMARY KEY,
  work_id  INTEGER NOT NULL REFERENCES work(id) ON DELETE CASCADE,
  uuid     TEXT CHECK (uuid IS NULL OR (length(uuid) = 36 AND uuid = lower(uuid))),
  slug     TEXT NOT NULL CHECK (slug NOT IN ('stichwort','personen','bibel') AND length(slug) BETWEEN 1 AND 32),
  title    TEXT NOT NULL CHECK (length(trim(title)) >= 1 AND length(title) <= 80),
  position INTEGER NOT NULL,
  created  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  UNIQUE (work_id, slug)
);
CREATE UNIQUE INDEX index_register_uuid ON index_register(uuid);

-- (7) Registervorschläge (ohne uuid, nicht im Austausch).
CREATE TABLE index_proposal (
  id         INTEGER PRIMARY KEY,
  work_id    INTEGER NOT NULL REFERENCES work(id) ON DELETE CASCADE,
  node_id    INTEGER NOT NULL REFERENCES node(id) ON DELETE CASCADE,
  based_on   INTEGER NOT NULL,
  word       TEXT NOT NULL CHECK (length(word) >= 1),
  occurrence TEXT NOT NULL,
  mark       TEXT NOT NULL CHECK (json_valid(mark) AND json_type(mark) = 'object'),
  reason     TEXT NOT NULL DEFAULT '' CHECK (length(reason) <= 1000),
  origin     TEXT NOT NULL,
  status     TEXT NOT NULL CHECK (status IN ('open','accepted','rejected','stale')),
  created    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  decided    TEXT,
  decided_by TEXT
);
CREATE INDEX index_proposal_work_status ON index_proposal(work_id, status);

-- (8) Fülltrigger und Grabsteine (Arten 'glossary_term' und 'index_register'; Werk des Grabsteins = work_id).
CREATE TRIGGER glossary_term_uuid_fill AFTER INSERT ON glossary_term WHEN NEW.uuid IS NULL
BEGIN UPDATE glossary_term SET uuid = lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' || substr(lower(hex(randomblob(2))),2) || '-' || substr('89ab', 1 + (abs(random()) % 4), 1) || substr(lower(hex(randomblob(2))),2) || '-' || lower(hex(randomblob(6))) WHERE rowid = NEW.rowid; END;
CREATE TRIGGER glossary_term_uuid_tombstone AFTER DELETE ON glossary_term WHEN OLD.uuid IS NOT NULL
BEGIN INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid) VALUES (OLD.uuid, 'glossary_term', (SELECT uuid FROM work WHERE id = OLD.work_id)); END;
CREATE TRIGGER index_register_uuid_fill AFTER INSERT ON index_register WHEN NEW.uuid IS NULL
BEGIN UPDATE index_register SET uuid = lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' || substr(lower(hex(randomblob(2))),2) || '-' || substr('89ab', 1 + (abs(random()) % 4), 1) || substr(lower(hex(randomblob(2))),2) || '-' || lower(hex(randomblob(6))) WHERE rowid = NEW.rowid; END;
CREATE TRIGGER index_register_uuid_tombstone AFTER DELETE ON index_register WHEN OLD.uuid IS NOT NULL
BEGIN INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid) VALUES (OLD.uuid, 'index_register', (SELECT uuid FROM work WHERE id = OLD.work_id)); END;

-- Begriffe und Register vor ihrem Werk löschen (wie 0042/0045/0046/0048), damit der Grabstein die Werk-uuid kennt.
CREATE TRIGGER glossary_term_cascade_uuid_tombstone BEFORE DELETE ON work
BEGIN DELETE FROM glossary_term WHERE work_id = OLD.id; END;
CREATE TRIGGER index_register_cascade_uuid_tombstone BEFORE DELETE ON work
BEGIN DELETE FROM index_register WHERE work_id = OLD.id; END;
