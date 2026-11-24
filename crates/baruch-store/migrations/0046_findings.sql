-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Befunde, ihr Statusverlauf und alte Fassungen bearbeiteter Beiträge (Runde 28, Vertrag Abschnitt 2; S1 schreibt
-- sie, danach gehört sie befunde).
--
-- - `finding`: ein Befund aus der Antwort einer Persona (Art Lob/Hinweis/kritisch, Status offen/übernommen/
--   verworfen/verschoben/erledigt). Herkunft: Beitrag im Abschnitts-Chat (`post_id`) oder Fadenbeitrag
--   (`thread_post_id`), nie beides; Stelle in der Antwort `position` (eindeutig je Beitrag). Fassung, Ziel und
--   Ähnlichkeit werden über uuid bzw. id gemerkt (Fassung, Ziel-Abschnitt und Ziel-Einfall ohne Fremdschlüssel, damit
--   ein Löschen dort den Verlauf nicht bricht). CHECKs: verworfen nur mit Begründung, übernommen nur mit Fassung,
--   verschoben mit genau einem Ziel.
-- - `finding_status`: jede Änderung des Status (nicht die Erstanlage). Der Mensch setzt (`proposed = 0`,
--   `actor_kind = 'human'`); Assistenten stellen nur Vorschläge (`proposed = 1`), über die der Mensch entscheidet
--   (`decision` confirmed/rejected/stale, mit `decided`).
-- - `post_version`: die alte Fassung eines bearbeiteten Beitrags (Abschnitts-Chat oder Faden, genau eins), dazu
--   `post.edited` und `chat_thread_post.edited` (NULL = nie bearbeitet).
-- - Alle drei Tabellen tragen `uuid` wie in 0041/0043/0045 (Spalte, eindeutiger Index, Fülltrigger,
--   Grabstein-Trigger); nur `exchange_apply` setzt eine.
-- - `uuid_tombstone` bekommt die Arten 'finding', 'finding_status' und 'post_version'. SQLite kann einen CHECK
--   nicht ändern, darum wie in 0045: (1) die dreizehn Grabstein-Trigger droppen, die uuid_tombstone nennen (zehn
--   aus 0041, idea_link und idea_map_pos aus 0043, cover_block aus 0045), (2) Zeilen in eine temporäre Kopie,
--   Tabelle neu mit erweitertem CHECK, Zeilen zurück, Index neu, (3) die dreizehn Trigger wörtlich neu. Die Trigger
--   aus 0042 und die Kaskaden-Trigger aus 0043/0045 nennen uuid_tombstone nicht und bleiben.
-- - Kaskaden-Trigger (wie 0042/0043/0045), damit jeder Grabstein die Werk-uuid kennt: vor einem Werk seine Befunde,
--   vor einem Befund seinen Verlauf, vor einem Beitrag bzw. Fadenbeitrag seine alten Fassungen löschen, solange die
--   Elternzeile noch steht.
--
-- Kein FOREIGN_KEYS_OFF: Umgebaut wird nur uuid_tombstone (kein Fremdschlüssel zeigt darauf); das implizite DELETE
-- von DROP TABLE löst darum keine Aktion aus (Beleg in tests/migration_0046.rs).
--
-- Reserviert (Runde 28): 0047 befunde, 0048 chat-auftrag, 0049 befund-uebersicht, 0050 Integrator (alle erwartet
-- unbenutzt); Bibliothek 0107 chat-auftrag (Volltextindex bei bearbeiteten Beiträgen).

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

-- (2) uuid_tombstone mit den neuen Arten umbauen (temporäre Kopie wie in 0045).
CREATE TEMP TABLE uuid_tombstone_copy AS SELECT uuid, kind, work_uuid, deleted FROM uuid_tombstone;
DROP TABLE uuid_tombstone;
CREATE TABLE uuid_tombstone (
  uuid      TEXT PRIMARY KEY,
  kind      TEXT NOT NULL CHECK (kind IN ('work','node','revision','template_revision','post','idea','thread',
                                          'thread_post','asset','asset_revision','idea_link','idea_map_pos',
                                          'cover_block',
                                          'finding','finding_status','post_version')),
  work_uuid TEXT,
  deleted   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
INSERT INTO uuid_tombstone (uuid, kind, work_uuid, deleted) SELECT uuid, kind, work_uuid, deleted FROM temp.uuid_tombstone_copy;
DROP TABLE temp.uuid_tombstone_copy;
CREATE INDEX uuid_tombstone_work ON uuid_tombstone(work_uuid);

-- (3) Die dreizehn Grabstein-Trigger wörtlich wie in 0041, 0043 bzw. 0045.
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

-- (4) Befunde.
CREATE TABLE finding (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  work_id INTEGER NOT NULL REFERENCES work(id) ON DELETE CASCADE,
  node_id INTEGER REFERENCES node(id) ON DELETE SET NULL,          -- NULL: Faden ohne Abschnitt bzw. Abschnitt fort
  post_id INTEGER REFERENCES post(id) ON DELETE SET NULL,
  thread_post_id INTEGER REFERENCES chat_thread_post(id) ON DELETE SET NULL,
  position INTEGER NOT NULL CHECK (position >= 0),                 -- Stelle in der Antwort
  persona TEXT NOT NULL CHECK (length(persona) BETWEEN 1 AND 200), -- Autor der Antwort (@handle)
  kind TEXT NOT NULL CHECK (kind IN ('praise','hint','critical')),
  body TEXT NOT NULL CHECK (length(trim(body)) > 0 AND length(body) <= 4000),
  quote TEXT NOT NULL DEFAULT '' CHECK (length(quote) <= 2000),
  quote_check TEXT NOT NULL DEFAULT '' CHECK (length(quote_check) <= 200),
  status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open','adopted','rejected','deferred','done')),
  reason TEXT NOT NULL DEFAULT '' CHECK (length(reason) <= 2000),
  adopted_revision TEXT,                                           -- uuid der Fassung (ohne Fremdschlüssel)
  deferred_node TEXT,                                              -- uuid des Ziel-Abschnitts (ohne Fremdschlüssel)
  deferred_idea TEXT,                                              -- uuid des Ziel-Einfalls (ohne Fremdschlüssel)
  similar_to INTEGER REFERENCES finding(id) ON DELETE SET NULL,    -- angehängt an diesen Befund
  summary_of TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(summary_of) AND json_type(summary_of) = 'array'),
  created TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  uuid TEXT CHECK (uuid IS NULL OR (length(uuid) = 36 AND uuid = lower(uuid))),
  CHECK (status <> 'rejected' OR length(trim(reason)) > 0),
  CHECK (status <> 'adopted' OR adopted_revision IS NOT NULL),
  CHECK (status <> 'deferred' OR ((deferred_node IS NULL) <> (deferred_idea IS NULL))),
  CHECK (post_id IS NULL OR thread_post_id IS NULL)
);
CREATE UNIQUE INDEX finding_uuid ON finding(uuid);
CREATE UNIQUE INDEX finding_post_position ON finding(post_id, position) WHERE post_id IS NOT NULL;
CREATE UNIQUE INDEX finding_thread_post_position ON finding(thread_post_id, position) WHERE thread_post_id IS NOT NULL;
CREATE INDEX finding_work_node_status ON finding(work_id, node_id, status);

-- (5) Verlauf des Status (nur Änderungen; die Erstanlage hat keinen Eintrag).
CREATE TABLE finding_status (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  finding_id INTEGER NOT NULL REFERENCES finding(id) ON DELETE CASCADE,
  status TEXT NOT NULL CHECK (status IN ('open','adopted','rejected','deferred','done')),
  reason TEXT NOT NULL DEFAULT '' CHECK (length(reason) <= 2000),
  adopted_revision TEXT,
  deferred_node TEXT,
  deferred_idea TEXT,
  actor_kind TEXT NOT NULL CHECK (actor_kind IN ('human','persona','mcp')),
  actor TEXT NOT NULL,                                             -- Name des Menschen, @handle bzw. @mcp:<name>
  proposed INTEGER NOT NULL DEFAULT 0 CHECK (proposed IN (0,1)),   -- 1 = Vorschlag eines Assistenten
  note TEXT NOT NULL DEFAULT '' CHECK (length(note) <= 2000),
  decision TEXT CHECK (decision IS NULL OR decision IN ('confirmed','rejected','stale')),
  decided TEXT,
  decided_by TEXT,
  created TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  uuid TEXT CHECK (uuid IS NULL OR (length(uuid) = 36 AND uuid = lower(uuid))),
  CHECK (proposed = 1 OR actor_kind = 'human'),
  CHECK (proposed = 1 OR decision IS NULL),
  CHECK ((decided IS NULL) = (decision IS NULL)),
  CHECK (status <> 'rejected' OR length(trim(reason)) > 0),
  CHECK (status <> 'adopted' OR adopted_revision IS NOT NULL),
  CHECK (status <> 'deferred' OR ((deferred_node IS NULL) <> (deferred_idea IS NULL)))
);
CREATE UNIQUE INDEX finding_status_uuid ON finding_status(uuid);
CREATE INDEX finding_status_finding ON finding_status(finding_id);

-- (6) Alte Fassungen bearbeiteter Beiträge.
CREATE TABLE post_version (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  post_id INTEGER REFERENCES post(id) ON DELETE CASCADE,
  thread_post_id INTEGER REFERENCES chat_thread_post(id) ON DELETE CASCADE,
  body TEXT NOT NULL,
  valid_from TEXT NOT NULL,                                        -- seit wann diese Fassung galt
  replaced TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  edited_by TEXT NOT NULL,
  uuid TEXT CHECK (uuid IS NULL OR (length(uuid) = 36 AND uuid = lower(uuid))),
  CHECK ((post_id IS NULL) <> (thread_post_id IS NULL))
);
CREATE UNIQUE INDEX post_version_uuid ON post_version(uuid);
CREATE INDEX post_version_post ON post_version(post_id);
CREATE INDEX post_version_thread_post ON post_version(thread_post_id);
ALTER TABLE post ADD COLUMN edited TEXT;
ALTER TABLE chat_thread_post ADD COLUMN edited TEXT;

-- (7) Fülltrigger und Grabsteine.
-- finding (finding); Werk des Grabsteins = work_id
CREATE TRIGGER finding_uuid_fill AFTER INSERT ON finding WHEN NEW.uuid IS NULL
BEGIN UPDATE finding SET uuid = lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' || substr(lower(hex(randomblob(2))),2) || '-' || substr('89ab', 1 + (abs(random()) % 4), 1) || substr(lower(hex(randomblob(2))),2) || '-' || lower(hex(randomblob(6))) WHERE rowid = NEW.rowid; END;
CREATE TRIGGER finding_uuid_tombstone AFTER DELETE ON finding WHEN OLD.uuid IS NOT NULL
BEGIN INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid) VALUES (OLD.uuid, 'finding', (SELECT uuid FROM work WHERE id = OLD.work_id)); END;

-- finding_status (finding_status); Werk über den Befund
CREATE TRIGGER finding_status_uuid_fill AFTER INSERT ON finding_status WHEN NEW.uuid IS NULL
BEGIN UPDATE finding_status SET uuid = lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' || substr(lower(hex(randomblob(2))),2) || '-' || substr('89ab', 1 + (abs(random()) % 4), 1) || substr(lower(hex(randomblob(2))),2) || '-' || lower(hex(randomblob(6))) WHERE rowid = NEW.rowid; END;
CREATE TRIGGER finding_status_uuid_tombstone AFTER DELETE ON finding_status WHEN OLD.uuid IS NOT NULL
BEGIN INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid) VALUES (OLD.uuid, 'finding_status', (SELECT w.uuid FROM finding f JOIN work w ON w.id = f.work_id WHERE f.id = OLD.finding_id)); END;

-- post_version (post_version); Werk über Beitrag → Abschnitt → Werk bzw. Fadenbeitrag → Faden → Werk
CREATE TRIGGER post_version_uuid_fill AFTER INSERT ON post_version WHEN NEW.uuid IS NULL
BEGIN UPDATE post_version SET uuid = lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' || substr(lower(hex(randomblob(2))),2) || '-' || substr('89ab', 1 + (abs(random()) % 4), 1) || substr(lower(hex(randomblob(2))),2) || '-' || lower(hex(randomblob(6))) WHERE rowid = NEW.rowid; END;
CREATE TRIGGER post_version_uuid_tombstone AFTER DELETE ON post_version WHEN OLD.uuid IS NOT NULL
BEGIN INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid) VALUES (OLD.uuid, 'post_version', coalesce((SELECT w.uuid FROM post p JOIN node n ON n.id = p.node_id JOIN work w ON w.id = n.work_id WHERE p.id = OLD.post_id), (SELECT w.uuid FROM chat_thread_post tp JOIN chat_thread t ON t.id = tp.thread_id JOIN work w ON w.id = t.work_id WHERE tp.id = OLD.thread_post_id))); END;

-- Kinder vor ihren Eltern löschen (wie 0042/0043/0045), damit deren Grabsteine die Werk-uuid kennen.
CREATE TRIGGER work_finding_cascade_uuid_tombstone BEFORE DELETE ON work
BEGIN DELETE FROM finding WHERE work_id = OLD.id; END;
CREATE TRIGGER finding_status_cascade_uuid_tombstone BEFORE DELETE ON finding
BEGIN DELETE FROM finding_status WHERE finding_id = OLD.id; END;
CREATE TRIGGER post_version_cascade_uuid_tombstone BEFORE DELETE ON post
BEGIN DELETE FROM post_version WHERE post_id = OLD.id; END;
CREATE TRIGGER thread_post_version_cascade_uuid_tombstone BEFORE DELETE ON chat_thread_post
BEGIN DELETE FROM post_version WHERE thread_post_id = OLD.id; END;
