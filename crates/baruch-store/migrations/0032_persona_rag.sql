-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Runde 12 (multigpt): Treffer der Bedeutungssuche als Belege für eine Persona (nutzen dürfen sie nur @belege
-- und der Lektor; das prüft der Server). Keine Zeile = aus. Keine Datenänderung; die Tabelle `persona` bleibt.
CREATE TABLE persona_rag (
  persona_id INTEGER PRIMARY KEY REFERENCES persona(id) ON DELETE CASCADE,
  enabled INTEGER NOT NULL CHECK (enabled IN (0,1)),
  updated TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
