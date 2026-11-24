-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Hervorhebungen in Quellen der Bibliothek: eine markierte Stelle eines Titels (optional in einem
-- Zitat, mit Zeichenbereich), Art, Notiz, Urheber. Personas legen nur Vorschläge an.
-- Im Volltextindex als kind 'highlight' (title = Titel der Quelle, body = Text und Notiz).

CREATE TABLE library_highlight (
  id          INTEGER PRIMARY KEY,
  title_id    INTEGER NOT NULL REFERENCES library_title(id) ON DELETE CASCADE,
  quote_id    INTEGER REFERENCES library_quote(id) ON DELETE SET NULL,
  locator     TEXT NOT NULL DEFAULT '',  -- Abschnitt bzw. Seite: „S. 12“, „13,4“
  start_char  INTEGER,                   -- Zeichenbereich ab 0 im Zitat bzw. Dokument
  end_char    INTEGER,
  text        TEXT NOT NULL CHECK (length(trim(text, ' ' || char(9, 10, 13))) > 0),
  kind        TEXT NOT NULL CHECK (kind IN ('quote','core','contradiction','check')),
  note        TEXT NOT NULL DEFAULT '',
  author_kind TEXT NOT NULL CHECK (author_kind IN ('human','assistant')),
  author_name TEXT NOT NULL,             -- Mensch: Name; Persona: Handle „@lektor“
  model       TEXT,                      -- nur bei assistant
  status      TEXT NOT NULL CHECK (status IN ('active','proposed','rejected')),
  created     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  CHECK ((start_char IS NULL AND end_char IS NULL)
         OR (start_char IS NOT NULL AND end_char IS NOT NULL AND 0 <= start_char AND start_char <= end_char)),
  CHECK ((author_kind = 'assistant') = (model IS NOT NULL)),
  -- Personas nur als Vorschlag.
  CHECK (author_kind = 'human' OR status IN ('proposed','rejected'))
);
CREATE INDEX library_highlight_title ON library_highlight(title_id);
CREATE INDEX library_highlight_quote ON library_highlight(quote_id);

CREATE TRIGGER library_highlight_fts_insert AFTER INSERT ON library_highlight
BEGIN
  INSERT INTO search_fts (kind, row_id, work_id, title, body)
  VALUES ('highlight', new.id, NULL, (SELECT title FROM library_title WHERE id = new.title_id),
          trim(new.text || ' ' || new.note));
END;
CREATE TRIGGER library_highlight_fts_update AFTER UPDATE ON library_highlight
BEGIN
  UPDATE search_fts
     SET title = (SELECT title FROM library_title WHERE id = new.title_id),
         body = trim(new.text || ' ' || new.note),
         row_id = new.id
   WHERE kind = 'highlight' AND row_id = old.id;
END;
CREATE TRIGGER library_highlight_fts_delete AFTER DELETE ON library_highlight
BEGIN
  DELETE FROM search_fts WHERE kind = 'highlight' AND row_id = old.id;
END;
-- Hervorhebungen tragen den Titel der Quelle mit.
CREATE TRIGGER library_title_fts_highlight_title AFTER UPDATE OF title ON library_title
BEGIN
  UPDATE search_fts SET title = new.title
   WHERE kind = 'highlight' AND row_id IN (SELECT id FROM library_highlight WHERE title_id = new.id);
END;
-- Hervorhebungen, die per ON DELETE CASCADE mit dem Titel verschwunden sind.
CREATE TRIGGER library_title_fts_highlight_delete AFTER DELETE ON library_title
BEGIN
  DELETE FROM search_fts WHERE kind = 'highlight' AND row_id NOT IN (SELECT id FROM library_highlight);
END;

-- Index für vorhandene Zeilen nachfüllen (ohne Dubletten).
DELETE FROM search_fts WHERE kind = 'highlight';
INSERT INTO search_fts (kind, row_id, work_id, title, body)
  SELECT 'highlight', library_highlight.id, NULL, library_title.title,
         trim(library_highlight.text || ' ' || library_highlight.note)
    FROM library_highlight JOIN library_title ON library_title.id = library_highlight.title_id;
