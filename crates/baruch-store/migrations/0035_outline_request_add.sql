-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Nutzerfrage 12 (Entscheidung des Nutzers 2026-11-07: „es braucht eine aktive Zustimmung (Popup)“):
-- Legt ein Assistent einen Teil so an, dass vorhandene Kapitel unter ihn wandern, wird daraus ein
-- Antrag mit der Art 'add', dem der Mensch zustimmt. Die Prüfung der Spalte `op` lässt sich in SQLite
-- nicht ändern; deshalb wird `outline_request` mit derselben Spaltenfolge neu angelegt, alle Zeilen
-- werden unverändert (samt id) übernommen und der Index neu angelegt. Auf `outline_request` zeigt kein
-- Fremdschlüssel. Die Zählung für AUTOINCREMENT (sqlite_sequence) wird mitgenommen, damit die id eines
-- gelöschten Antrags nicht neu vergeben wird.

CREATE TABLE outline_request_new (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  work_id INTEGER NOT NULL REFERENCES work(id) ON DELETE CASCADE,
  op TEXT NOT NULL CHECK (op IN ('rename','move','set_level','delete','set_numbered','add')),
  params TEXT NOT NULL CHECK (json_valid(params)),
  origin_kind TEXT NOT NULL CHECK (origin_kind IN ('human','persona','mcp')),
  origin_name TEXT NOT NULL,                       -- Name bzw. @handle bzw. @mcp:<client>
  note TEXT NOT NULL DEFAULT '',                   -- Begründung des Antragstellers
  created TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  decided TEXT,                                    -- NULL = offen
  decision TEXT CHECK (decision IN ('confirmed','rejected','stale')),
  decided_by TEXT,                                 -- der Mensch, der entschieden hat
  message TEXT,                                    -- bei „stale“: warum veraltet bzw. nicht ausführbar
  CHECK ((decided IS NULL) = (decision IS NULL))
);

INSERT INTO outline_request_new (id, work_id, op, params, origin_kind, origin_name, note, created, decided,
                                 decision, decided_by, message)
  SELECT id, work_id, op, params, origin_kind, origin_name, note, created, decided, decision, decided_by, message
  FROM outline_request;

INSERT INTO sqlite_sequence (name, seq)
  SELECT 'outline_request_new', 0
  WHERE EXISTS (SELECT 1 FROM sqlite_sequence WHERE name = 'outline_request')
    AND NOT EXISTS (SELECT 1 FROM sqlite_sequence WHERE name = 'outline_request_new');
UPDATE sqlite_sequence
  SET seq = max(seq, (SELECT seq FROM sqlite_sequence WHERE name = 'outline_request'))
  WHERE name = 'outline_request_new' AND EXISTS (SELECT 1 FROM sqlite_sequence WHERE name = 'outline_request');

DROP TABLE outline_request;
ALTER TABLE outline_request_new RENAME TO outline_request;
CREATE INDEX outline_request_open ON outline_request(work_id, decided);
-- INSERT … SELECT legt für eine AUTOINCREMENT-Tabelle auch ohne Zeilen einen Zähler 0 an; gab es vorher keinen
-- (nie ein Antrag), bleibt sqlite_sequence wie vorher.
DELETE FROM sqlite_sequence WHERE name = 'outline_request' AND seq = 0;
