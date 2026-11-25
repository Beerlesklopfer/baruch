-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Runde 34 „Mindmap wie XMind“ (mindmap_daten, Vertrag §1): Einfälle als Baum.
--
-- Nur ADD COLUMN, kein Tabellenumbau (darum nicht in FOREIGN_KEYS_OFF und kein Umbau von uuid_tombstone):
--   title       Titel des Knotens (höchstens 200 Zeichen, darf leer sein – dann zeigen Karte und Liste die
--               Schlagwörter bzw. den Anfang des Gedankens).
--   parent_id   Elternknoten (ein Einfall desselben Werks; die Regeln – gleiches Werk, kein Kreis, Tiefe höchstens
--               32 – prüft der Store). Hartes Löschen des Elternknotens löst nur die Bindung.
--   position    Reihenfolge unter Geschwistern (dann nach id). Bestehende Einfälle: 0, also nach id.
--   main_topic  Nachtrag des Nutzers vom 2026-11-25 (Wolke als Ablage neben der Mindmap): Ein Einfall ohne
--               Elternknoten und ohne Bezug liegt in der Wolke (0, Vorgabe – auch alle bestehenden) oder ist ein
--               freies Hauptthema direkt am Werk (1). Mit Elternknoten oder Bezug ist die Spalte ohne Bedeutung.
--
-- Bestehende Zeilen bleiben unverändert (title '', parent_id NULL, position 0, main_topic 0); der Volltextindex
-- idea_fts wird nicht nachgefüllt, weil alle Titel leer sind (der Store schreibt den Titel künftig in die
-- FTS-Spalte words mit).

ALTER TABLE idea ADD COLUMN title TEXT NOT NULL DEFAULT '' CHECK (length(title) <= 200);
ALTER TABLE idea ADD COLUMN parent_id INTEGER REFERENCES idea(id) ON DELETE SET NULL;
ALTER TABLE idea ADD COLUMN position INTEGER NOT NULL DEFAULT 0;
ALTER TABLE idea ADD COLUMN main_topic INTEGER NOT NULL DEFAULT 0 CHECK (main_topic IN (0, 1));
CREATE INDEX idea_parent ON idea(parent_id, position);
