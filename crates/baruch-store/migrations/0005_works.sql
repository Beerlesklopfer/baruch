-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Werke durch den Menschen verwalten (Vertrag Runde 4, Abschnitt 1): eindeutige Titel über einen
-- Schlüssel, Metadaten und Fassungen der Werkvorlage.
--
-- `title_key` = `baruch_store::title_key(title)` (getrimmt, Leerraum zusammengefasst, Unicode-klein).
-- Hier bleibt er NULL: Den Schlüssel trägt `Store::open` nach dieser Migration in Rust nach
-- (`works::ensure_title_keys`) und löst dabei vorhandene Doppel nur über den Titel auf.
ALTER TABLE work ADD COLUMN title_key TEXT;
ALTER TABLE work ADD COLUMN subtitle TEXT NOT NULL DEFAULT '';
ALTER TABLE work ADD COLUMN audience TEXT NOT NULL DEFAULT '';   -- Zielgruppe
ALTER TABLE work ADD COLUMN bible TEXT NOT NULL DEFAULT '';      -- Bibelübersetzung
ALTER TABLE work ADD COLUMN updated TEXT;                        -- letzte Änderung von Metadaten/Vorlage (ISO wie created)
CREATE UNIQUE INDEX work_title_key ON work(title_key) WHERE title_key IS NOT NULL;

-- Fassungen der Werkvorlage; `work.template` bleibt die aktuelle (= Fassung mit der höchsten Nummer).
CREATE TABLE work_template_revision (
  id          INTEGER PRIMARY KEY,
  work_id     INTEGER NOT NULL REFERENCES work(id),
  number      INTEGER NOT NULL,
  body        TEXT NOT NULL,
  author_name TEXT NOT NULL,
  note        TEXT NOT NULL DEFAULT '',
  created     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  UNIQUE (work_id, number)
);
INSERT INTO work_template_revision (work_id, number, body, author_name, note)
  SELECT id, 1, template, 'Autor', 'Stand vor Migration 0005' FROM work;
