-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Werk im Volltextindex: Revisionen und Beiträge (Kernschema 0001 von baruch-store).
-- Revisionen sind unveränderlich (Trigger im Kernschema), darum genügt AFTER INSERT; der Status
-- wird bei der Abfrage per JOIN gelesen. Beiträge werden ebenfalls nur angehängt.

CREATE TRIGGER revision_fts_insert AFTER INSERT ON revision
BEGIN
  INSERT INTO search_fts (kind, row_id, work_id, title, body)
  SELECT 'revision', new.id, node.work_id, node.title, new.body FROM node WHERE node.id = new.node_id;
END;
CREATE TRIGGER post_fts_insert AFTER INSERT ON post
BEGIN
  INSERT INTO search_fts (kind, row_id, work_id, title, body)
  SELECT 'post', new.id, node.work_id, node.title, new.body FROM node WHERE node.id = new.node_id;
END;
-- Abschnittstitel dürfen sich ändern; der Index trägt sie mit.
CREATE TRIGGER node_fts_title AFTER UPDATE OF title ON node
BEGIN
  UPDATE search_fts SET title = new.title
   WHERE (kind = 'revision' AND row_id IN (SELECT id FROM revision WHERE node_id = new.id))
      OR (kind = 'post' AND row_id IN (SELECT id FROM post WHERE node_id = new.id));
END;

-- Index vollständig aus den vorhandenen Daten neu aufbauen (Werk und Bibliothek).
DELETE FROM search_fts;
INSERT INTO search_fts (kind, row_id, work_id, title, body)
  SELECT 'revision', revision.id, node.work_id, node.title, revision.body
    FROM revision JOIN node ON node.id = revision.node_id;
INSERT INTO search_fts (kind, row_id, work_id, title, body)
  SELECT 'post', post.id, node.work_id, node.title, post.body
    FROM post JOIN node ON node.id = post.node_id;
INSERT INTO search_fts (kind, row_id, work_id, title, body)
  SELECT 'title', id, NULL, title,
         trim(authors || ' ' || coalesce(year, '') || ' ' || publisher || ' ' || place || ' ' || key)
    FROM library_title;
INSERT INTO search_fts (kind, row_id, work_id, title, body)
  SELECT 'quote', library_quote.id, NULL, library_title.title, library_quote.text
    FROM library_quote JOIN library_title ON library_title.id = library_quote.title_id;
