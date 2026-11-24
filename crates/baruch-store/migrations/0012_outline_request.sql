-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Anträge zur Gliederung (Wunsch 2026-11-06, „UD nach Bestätigung“): Assistenten und MCP-Clients dürfen
-- Gliederungspunkte anlegen, aber nicht umbenennen, verschieben, umstufen, löschen oder mit/ohne
-- Nummer zählen. Stattdessen legen
-- sie einen Antrag an; der Mensch bestätigt (dann führt Baruch ihn über die Gliederungsfunktionen aus)
-- oder lehnt ab. Nur eine neue Tabelle; bestehende Zeilen bleiben unberührt.
--
-- `params` ist JSON: `{"op": {...}, "expected": {...}}` – der Auftrag mit festen Schlüsseln (node.id)
-- und der Stand beim Antrag (Titel, Ebene, Titel des Ziels), an dem die Bestätigung erkennt, ob der
-- Antrag veraltet ist. Ein Werk zu löschen löscht seine Anträge mit.

CREATE TABLE outline_request (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  work_id INTEGER NOT NULL REFERENCES work(id) ON DELETE CASCADE,
  op TEXT NOT NULL CHECK (op IN ('rename','move','set_level','delete','set_numbered')),
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

CREATE INDEX outline_request_open ON outline_request(work_id, decided);
