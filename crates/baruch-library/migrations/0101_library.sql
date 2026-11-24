-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Autorenbibliothek: Titel (Zitierschlüssel für Typst-@cite) und Zitate, dazu der gemeinsame
-- Volltextindex `search_fts` für Werk (revision, post; Trigger in 0102) und Bibliothek.

CREATE TABLE library_title (
  id        INTEGER PRIMARY KEY,
  key       TEXT NOT NULL UNIQUE,     -- Zitierschlüssel für Typst-@cite, [a-z0-9_-:]+
  kind      TEXT NOT NULL CHECK (kind IN ('book','article','chapter','bible','web','other')),
  title     TEXT NOT NULL,
  authors   TEXT NOT NULL DEFAULT '', -- „Nachname, Vorname; Nachname, Vorname“
  year      INTEGER,
  publisher TEXT NOT NULL DEFAULT '',
  place     TEXT NOT NULL DEFAULT '',
  url       TEXT NOT NULL DEFAULT '',
  note      TEXT NOT NULL DEFAULT '',
  created   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE TABLE library_quote (
  id        INTEGER PRIMARY KEY,
  title_id  INTEGER NOT NULL REFERENCES library_title(id) ON DELETE CASCADE,
  position  INTEGER NOT NULL,
  text      TEXT NOT NULL,
  page      TEXT NOT NULL DEFAULT '', -- „S. 12“, „13,4“
  note      TEXT NOT NULL DEFAULT '',
  created   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  UNIQUE (title_id, position)
);
CREATE VIRTUAL TABLE search_fts USING fts5(
  kind UNINDEXED,     -- 'revision' | 'post' | 'title' | 'quote'
  row_id UNINDEXED,   -- id in revision / post / library_title / library_quote
  work_id UNINDEXED,  -- NULL bei Bibliothek
  title,              -- Abschnittstitel bzw. Titel des Werks in der Bibliothek
  body,
  tokenize = 'unicode61 remove_diacritics 2'
);

-- Titel: title = Titel, body = Autoren, Jahr, Verlag, Ort, Schlüssel.
CREATE TRIGGER library_title_fts_insert AFTER INSERT ON library_title
BEGIN
  INSERT INTO search_fts (kind, row_id, work_id, title, body)
  VALUES ('title', new.id, NULL, new.title,
          trim(new.authors || ' ' || coalesce(new.year, '') || ' ' || new.publisher || ' ' || new.place || ' ' || new.key));
END;
CREATE TRIGGER library_title_fts_update AFTER UPDATE ON library_title
BEGIN
  UPDATE search_fts
     SET title = new.title,
         body = trim(new.authors || ' ' || coalesce(new.year, '') || ' ' || new.publisher || ' ' || new.place || ' ' || new.key),
         row_id = new.id
   WHERE kind = 'title' AND row_id = old.id;
  -- Zitate tragen den Titel des Werks mit.
  UPDATE search_fts SET title = new.title
   WHERE kind = 'quote' AND row_id IN (SELECT id FROM library_quote WHERE title_id = new.id);
END;
CREATE TRIGGER library_title_fts_delete AFTER DELETE ON library_title
BEGIN
  DELETE FROM search_fts WHERE kind = 'title' AND row_id = old.id;
  -- Zitate, die per ON DELETE CASCADE mit verschwunden sind (unabhängig davon, ob dabei
  -- der Lösch-Trigger der Zitate feuert).
  DELETE FROM search_fts WHERE kind = 'quote' AND row_id NOT IN (SELECT id FROM library_quote);
END;

-- Zitate: title = Titel des Werks, body = Zitattext.
CREATE TRIGGER library_quote_fts_insert AFTER INSERT ON library_quote
BEGIN
  INSERT INTO search_fts (kind, row_id, work_id, title, body)
  VALUES ('quote', new.id, NULL, (SELECT title FROM library_title WHERE id = new.title_id), new.text);
END;
CREATE TRIGGER library_quote_fts_update AFTER UPDATE ON library_quote
BEGIN
  UPDATE search_fts
     SET title = (SELECT title FROM library_title WHERE id = new.title_id),
         body = new.text,
         row_id = new.id
   WHERE kind = 'quote' AND row_id = old.id;
END;
CREATE TRIGGER library_quote_fts_delete AFTER DELETE ON library_quote
BEGIN
  DELETE FROM search_fts WHERE kind = 'quote' AND row_id = old.id;
END;
