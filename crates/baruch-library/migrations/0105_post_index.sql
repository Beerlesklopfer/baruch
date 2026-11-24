-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Volltextindex für Beiträge (Vertrag Runde 4, Abschnitt 1): Beiträge waren bisher nur angehängt
-- (0102 kennt nur AFTER INSERT). Seit Werke und Gliederungspunkte gelöscht und zusammengeführt werden
-- können, verschwinden Beiträge oder wandern in einen anderen Abschnitt. Der Index trägt beides mit:
-- ein gelöschter Beitrag verschwindet, ein verschobener trägt Titel und Werk des neuen Abschnitts.

CREATE TRIGGER post_fts_delete AFTER DELETE ON post
BEGIN
  DELETE FROM search_fts WHERE kind = 'post' AND row_id = old.id;
END;

CREATE TRIGGER post_fts_move AFTER UPDATE OF node_id ON post
BEGIN
  UPDATE search_fts
     SET title = (SELECT title FROM node WHERE id = new.node_id),
         work_id = (SELECT work_id FROM node WHERE id = new.node_id)
   WHERE kind = 'post' AND row_id = new.id;
END;

-- Aufräumen: Indexzeilen von Beiträgen, die es nicht mehr gibt.
DELETE FROM search_fts WHERE kind = 'post' AND row_id NOT IN (SELECT id FROM post);
