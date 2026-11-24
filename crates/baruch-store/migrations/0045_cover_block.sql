-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Umschlag-Anordnung, Anschriften und Antrag „Anordnung“ (Runde 27, Vertrag Abschnitt 3; S1 schreibt sie, danach
-- gehört sie umschlag-daten).
--
-- - `cover_block`: je Werk und Art (die 15 Namen aus `baruch_typst::cover_layout::CoverBlockKind`) höchstens ein
--   frei gesetzter Block des Umschlags; Koordinaten in mm relativ zur Schnittkante der eigenen Fläche. Grenzen wie
--   die Konstanten in `cover_layout` (POS_MM, WIDTH_MM, SIZE_PT, LEADING_EM, MAX_TEXT_CHARS, MAX_FONT_CHARS).
--   `direction` gibt es genau bei den Rücken-Blöcken. Die Tabelle trägt `uuid` wie in 0041/0043 (Spalte,
--   eindeutiger Index, Fülltrigger, Grabstein-Trigger); nur `exchange_apply` setzt eine.
-- - `uuid_tombstone` bekommt die Art 'cover_block'. SQLite kann einen CHECK nicht ändern, darum wie in 0043:
--   (1) die zwölf Grabstein-Trigger droppen, die uuid_tombstone nennen (zehn aus 0041, zwei aus 0043), (2) Zeilen
--   in eine temporäre Kopie, Tabelle neu mit erweitertem CHECK, Zeilen zurück, Index neu, (3) die zwölf Trigger
--   wörtlich neu. Die Trigger aus 0042 und die Kaskaden-Trigger aus 0043 nennen uuid_tombstone nicht und bleiben.
-- - `work_cover_block_cascade_uuid_tombstone` (wie 0042/0043): Vor einem Werk seine Blöcke löschen, solange die
--   Werkzeile noch steht, damit der Grabstein des Blocks die Werk-uuid kennt.
-- - `work_book` bekommt `publisher_address` und `printer_address` (Pflichtangaben im Impressum, mehrzeilig).
-- - `asset_request` bekommt die Art 'cover_layout' (Antrag eines Assistenten, die Anordnung zu ändern): Umbau über
--   eine temporäre Kopie mit allen Spalten samt id; der Folgezähler in sqlite_sequence bleibt beim Höchstwert.
--
-- Kein FOREIGN_KEYS_OFF: Umgebaut werden nur uuid_tombstone (kein Fremdschlüssel zeigt darauf) und asset_request
-- (niemandes Elterntabelle); das implizite DELETE von DROP TABLE löst darum keine Aktion aus (Beleg in
-- tests/migration_0045.rs).
--
-- Reserviert (Runde 27): 0046 umschlag-daten, 0047 umschlag-satz, 0048 Integrator (erwartet unbenutzt).

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

-- (2) uuid_tombstone mit der neuen Art umbauen (temporäre Kopie wie in 0043).
CREATE TEMP TABLE uuid_tombstone_copy AS SELECT uuid, kind, work_uuid, deleted FROM uuid_tombstone;
DROP TABLE uuid_tombstone;
CREATE TABLE uuid_tombstone (
  uuid      TEXT PRIMARY KEY,
  kind      TEXT NOT NULL CHECK (kind IN ('work','node','revision','template_revision','post','idea','thread',
                                          'thread_post','asset','asset_revision','idea_link','idea_map_pos',
                                          'cover_block')),
  work_uuid TEXT,
  deleted   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
INSERT INTO uuid_tombstone (uuid, kind, work_uuid, deleted) SELECT uuid, kind, work_uuid, deleted FROM temp.uuid_tombstone_copy;
DROP TABLE temp.uuid_tombstone_copy;
CREATE INDEX uuid_tombstone_work ON uuid_tombstone(work_uuid);

-- (3) Die zwölf Grabstein-Trigger wörtlich wie in 0041 bzw. 0043.
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

-- (4) Blöcke der Umschlag-Anordnung.
CREATE TABLE cover_block (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  work_id INTEGER NOT NULL REFERENCES work(id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK (kind IN ('front_title','front_subtitle','front_author','front_publisher','front_free',
                                 'spine_title','spine_author','spine_publisher','spine_free',
                                 'back_motto','back_blurb','back_bio','back_barcode','back_publisher_price','back_free')),
  x_mm REAL NOT NULL CHECK (x_mm BETWEEN -50 AND 1000),
  y_mm REAL NOT NULL CHECK (y_mm BETWEEN -50 AND 1000),
  width_mm REAL NOT NULL CHECK (width_mm BETWEEN 1 AND 1000),
  align TEXT NOT NULL DEFAULT 'center' CHECK (align IN ('left','center','right','justify')),
  size_pt REAL NOT NULL CHECK (size_pt BETWEEN 4 AND 200),
  leading_em REAL NOT NULL DEFAULT 0.65 CHECK (leading_em BETWEEN 0 AND 3),
  color TEXT NOT NULL DEFAULT 'light' CHECK (color IN ('light','dark') OR (length(color) = 7 AND substr(color,1,1) = '#')),
  font TEXT CHECK (font IS NULL OR length(font) BETWEEN 1 AND 200),
  style TEXT NOT NULL DEFAULT 'normal' CHECK (style IN ('normal','bold','italic')),
  backdrop TEXT NOT NULL DEFAULT 'none' CHECK (backdrop IN ('none','shadow','gradient')),
  visible INTEGER NOT NULL DEFAULT 1 CHECK (visible IN (0,1)),
  direction TEXT CHECK (direction IS NULL OR direction IN ('down','up')),
  text TEXT CHECK (text IS NULL OR length(text) <= 1000),
  updated TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  uuid TEXT CHECK (uuid IS NULL OR (length(uuid) = 36 AND uuid = lower(uuid))),
  UNIQUE (work_id, kind),
  CHECK ((direction IS NULL) = (kind NOT LIKE 'spine_%'))
);

-- cover_block (cover_block); Werk des Grabsteins = work_id
CREATE UNIQUE INDEX cover_block_uuid ON cover_block(uuid);
CREATE TRIGGER cover_block_uuid_fill AFTER INSERT ON cover_block WHEN NEW.uuid IS NULL
BEGIN UPDATE cover_block SET uuid = lower(hex(randomblob(4))) || '-' || lower(hex(randomblob(2))) || '-4' || substr(lower(hex(randomblob(2))),2) || '-' || substr('89ab', 1 + (abs(random()) % 4), 1) || substr(lower(hex(randomblob(2))),2) || '-' || lower(hex(randomblob(6))) WHERE rowid = NEW.rowid; END;
CREATE TRIGGER cover_block_uuid_tombstone AFTER DELETE ON cover_block WHEN OLD.uuid IS NOT NULL
BEGIN INSERT OR IGNORE INTO uuid_tombstone (uuid, kind, work_uuid) VALUES (OLD.uuid, 'cover_block', (SELECT uuid FROM work WHERE id = OLD.work_id)); END;

-- Vor einem Werk seine Blöcke löschen (wie 0042/0043), damit deren Grabsteine die Werk-uuid kennen.
CREATE TRIGGER work_cover_block_cascade_uuid_tombstone BEFORE DELETE ON work
BEGIN DELETE FROM cover_block WHERE work_id = OLD.id; END;

-- (5) Anschriften für das Impressum.
ALTER TABLE work_book ADD COLUMN publisher_address TEXT NOT NULL DEFAULT '';
ALTER TABLE work_book ADD COLUMN printer_address TEXT NOT NULL DEFAULT '';

-- (6) asset_request mit der Art 'cover_layout' umbauen: CREATE-Text wörtlich wie in 0018 bis auf den CHECK von op
-- (bei cover_layout bleibt asset_name ''), alle Spalten in derselben Reihenfolge.
CREATE TEMP TABLE asset_request_seq AS SELECT seq FROM sqlite_sequence WHERE name = 'asset_request';
CREATE TEMP TABLE asset_request_copy AS
  SELECT id, work_id, op, asset_name, params, origin_kind, origin_name, note, created, decided, decision, decided_by,
         message FROM asset_request;
DROP TABLE asset_request;
CREATE TABLE asset_request (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  work_id INTEGER NOT NULL REFERENCES work(id) ON DELETE CASCADE,
  op TEXT NOT NULL CHECK (op IN ('replace','set_cover','delete','cover_layout')),
  asset_name TEXT NOT NULL DEFAULT '',             -- '' nur bei set_cover = Umschlag entfernen
  params TEXT NOT NULL CHECK (json_valid(params)),
  origin_kind TEXT NOT NULL CHECK (origin_kind IN ('persona','mcp')),
  origin_name TEXT NOT NULL,                       -- @handle bzw. @mcp:<client>
  note TEXT NOT NULL DEFAULT '',                   -- Begründung des Antragstellers
  created TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  decided TEXT,                                    -- NULL = offen
  decision TEXT CHECK (decision IN ('confirmed','rejected','stale')),
  decided_by TEXT,                                 -- der Mensch, der entschieden hat
  message TEXT,                                    -- bei „stale“: warum veraltet bzw. nicht ausführbar
  CHECK ((decided IS NULL) = (decision IS NULL))
);
INSERT INTO asset_request (id, work_id, op, asset_name, params, origin_kind, origin_name, note, created, decided,
                           decision, decided_by, message)
  SELECT id, work_id, op, asset_name, params, origin_kind, origin_name, note, created, decided, decision, decided_by,
         message FROM temp.asset_request_copy;
DROP TABLE temp.asset_request_copy;
CREATE INDEX asset_request_open ON asset_request(work_id, decided);
DELETE FROM sqlite_sequence WHERE name = 'asset_request';
INSERT INTO sqlite_sequence (name, seq)
  SELECT 'asset_request', max(coalesce((SELECT max(seq) FROM temp.asset_request_seq), 0),
                              coalesce((SELECT max(id) FROM asset_request), 0))
  WHERE EXISTS (SELECT 1 FROM temp.asset_request_seq) OR EXISTS (SELECT 1 FROM asset_request);
DROP TABLE temp.asset_request_seq;
