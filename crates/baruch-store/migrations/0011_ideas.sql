-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Einfälle (Vertrag Runde 7, Abschnitt 6a; Backlog „Einfälle“, Entscheidung IDEA_CRUD): Schlagwörter
-- und Gedanken, je Werk oder ohne Werk, mit Herkunft (Mensch, Persona, MCP), offen oder verarbeitet,
-- gelöscht = wiederherstellbar markiert. Nur neue Tabellen; bestehende Zeilen bleiben unberührt.
-- Ein Werk zu löschen macht seine Einfälle zu Einfällen ohne Werk (ON DELETE SET NULL), ebenso
-- verliert ein Einfall beim Löschen seines Abschnitts nur den Bezug.

CREATE TABLE idea (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  work_id INTEGER REFERENCES work(id) ON DELETE SET NULL,     -- NULL = Einfall ohne Werk
  node_id INTEGER REFERENCES node(id) ON DELETE SET NULL,     -- Bezug Abschnitt (= Node::key)
  note TEXT NOT NULL DEFAULT '',                              -- Gedanke
  quote TEXT NOT NULL DEFAULT '',                             -- markierter Text
  source TEXT NOT NULL DEFAULT '',                            -- Quelle/Hervorhebung
  bible TEXT NOT NULL DEFAULT '',                             -- Bibelstelle
  origin_kind TEXT NOT NULL CHECK (origin_kind IN ('human','persona','mcp')),
  origin_name TEXT NOT NULL,                                  -- Name bzw. @handle bzw. @mcp:<client>
  status TEXT NOT NULL DEFAULT 'open' CHECK (status IN ('open','done')),
  done_as TEXT CHECK (done_as IN ('outline','post','persona','note')),
  done_ref TEXT,
  created TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  deleted TEXT,                                               -- NULL = vorhanden; sonst Zeitpunkt (wiederherstellbar)
  CHECK ((status = 'done') = (done_as IS NOT NULL))
);

CREATE TABLE idea_word (
  idea_id INTEGER NOT NULL REFERENCES idea(id) ON DELETE CASCADE,
  position INTEGER NOT NULL,
  word TEXT NOT NULL CHECK (length(trim(word)) > 0),
  word_key TEXT NOT NULL,                                     -- klein geschrieben, Leerraum zusammengefasst
  PRIMARY KEY (idea_id, position),
  UNIQUE (idea_id, word_key)
);

CREATE INDEX idea_work ON idea(work_id, status);
CREATE INDEX idea_word_key ON idea_word(word_key);

-- rowid = idea.id, gepflegt in Rust in derselben Transaktion; gelöschte stehen nicht im Index.
CREATE VIRTUAL TABLE idea_fts USING fts5(words, note, quote, source, bible, tokenize = 'unicode61 remove_diacritics 2');
