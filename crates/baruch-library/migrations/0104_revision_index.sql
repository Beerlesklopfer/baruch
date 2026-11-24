-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Volltextindex für den Revisionsbaum (Vertrag Runde 3, Abschnitt 2): Seit Migration 0004 von
-- baruch-store darf der Mensch den Text einer Revision ändern und Revisionen löschen. Der Index trägt
-- beides mit: ein geänderter Text ersetzt den alten, eine gelöschte Revision verschwindet.

CREATE TRIGGER revision_fts_update AFTER UPDATE OF body ON revision
BEGIN
  UPDATE search_fts SET body = new.body WHERE kind = 'revision' AND row_id = new.id;
END;

CREATE TRIGGER revision_fts_delete AFTER DELETE ON revision
BEGIN
  DELETE FROM search_fts WHERE kind = 'revision' AND row_id = old.id;
END;
