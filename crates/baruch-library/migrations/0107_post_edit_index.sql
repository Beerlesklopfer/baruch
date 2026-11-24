-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Volltextindex für bearbeitete Beiträge (Runde 28, chat-auftrag; Vorbild 0105): Seit Beiträge des Menschen bzw. eines
-- MCP-Clients bearbeitet werden können (Store `post_edit.rs`, alte Fassung in `post_version`), ändert sich `post.body`.
-- Der Index trägt den neuen Text mit; die alte Fassung ist nicht mehr auffindbar (sie steht im Verlauf des Beitrags).
-- Fadenbeiträge (`chat_thread_post`) stehen nicht im Index, darum hier nichts für sie.

CREATE TRIGGER post_fts_edit AFTER UPDATE OF body ON post
BEGIN
  UPDATE search_fts SET body = new.body WHERE kind = 'post' AND row_id = new.id;
END;
