-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Runde 15 (auftraege): Fäden ohne Abschnitt je Werk. MCP `chat` ohne `key` legt einen Faden an bzw. setzt ihn
-- fort; der Mensch liest und antwortet auf /faeden und /faden. Ein Faden ist kein Buchtext und kein
-- Abschnitts-Chat: Er hängt nur am Werk (mit dem Werk gelöscht). Nur neue Tabellen, keine Datenänderung.
CREATE TABLE chat_thread (
  id         INTEGER PRIMARY KEY AUTOINCREMENT,
  work_id    INTEGER NOT NULL REFERENCES work(id) ON DELETE CASCADE,
  title      TEXT NOT NULL DEFAULT '',
  created_by TEXT NOT NULL,                         -- „Autor“, @handle oder @mcp:<client>
  created    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX chat_thread_work ON chat_thread(work_id, updated);

CREATE TABLE chat_thread_post (
  id        INTEGER PRIMARY KEY AUTOINCREMENT,
  thread_id INTEGER NOT NULL REFERENCES chat_thread(id) ON DELETE CASCADE,
  position  INTEGER NOT NULL,
  author    TEXT NOT NULL,
  body      TEXT NOT NULL CHECK (length(trim(body)) > 0),
  sources   TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(sources)),
  created   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  UNIQUE (thread_id, position)
);
