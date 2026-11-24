-- SPDX-License-Identifier: AGPL-3.0-or-later
-- SWORD-Bibeln (Wunsch 2026-11-06): Modulordner (global, in dieser Reihenfolge durchsucht) und die gewählte
-- Bibel je Werk (Modulname). Ohne Zeile in `store_meta` mit dem Schlüssel `bible_dirs` gelten die Vorgaben
-- ($SWORD_PATH, ~/.sword, /usr/share/sword); mit der Zeile gilt genau `bible_dir` (auch leer). Ohne Zeile in
-- `work_bible` hat das Werk keine Bibel (Wortlaut ungeprüft). Keine Datenänderung.
CREATE TABLE bible_dir (
  position INTEGER PRIMARY KEY CHECK (position >= 1),          -- 1..n lückenlos, vom Store gepflegt
  path TEXT NOT NULL UNIQUE CHECK (length(path) BETWEEN 1 AND 1000 AND instr(path, char(10)) = 0
                                   AND instr(path, char(13)) = 0)
);

CREATE TABLE work_bible (
  work_id INTEGER PRIMARY KEY REFERENCES work(id) ON DELETE CASCADE,
  module TEXT NOT NULL CHECK (length(module) BETWEEN 1 AND 100 AND instr(module, char(10)) = 0),
  updated TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
