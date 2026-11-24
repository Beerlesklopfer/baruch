-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Kommentare und Lexika je Werk und der Schalter „Bibel-Belege“ je Persona (Runde 15, Punkte 5a/5c). Nur
-- zwei neue Tabellen; bestehende bleiben unberührt.
--
-- work_sword_source: je Werk mehrere SWORD-Module der Art commentary bzw. lexicon in einer Reihenfolge.
-- persona_bible: Schalter je Persona; keine Zeile = Vorgabe des Servers (@belege an, Lektor aus).

CREATE TABLE work_sword_source (
  work_id INTEGER NOT NULL REFERENCES work(id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK (kind IN ('commentary','lexicon')),
  module TEXT NOT NULL CHECK (length(trim(module)) BETWEEN 1 AND 200 AND instr(module, char(10)) = 0),
  position INTEGER NOT NULL,
  PRIMARY KEY (work_id, kind, module)
);

CREATE TABLE persona_bible (
  persona_id INTEGER PRIMARY KEY REFERENCES persona(id) ON DELETE CASCADE,
  enabled INTEGER NOT NULL CHECK (enabled IN (0,1)),
  updated TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
