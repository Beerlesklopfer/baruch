-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Nachbesserung zu 0041 (Runde 26, ids): Grabsteine der Fäden mit Werk-uuid.
--
-- `chat_thread` hängt mit ON DELETE CASCADE an `work`, `chat_thread_post` mit ON DELETE CASCADE an
-- `chat_thread`. Bei einer Kaskade ist die Elternzeile schon fort, wenn der Grabstein-Trigger der Kindzeile
-- läuft; `(SELECT uuid FROM work WHERE id = OLD.work_id)` ergäbe NULL, der Grabstein wüsste sein Werk nicht
-- mehr (in einer nachgebauten Datenbank belegt, Test `crates/baruch-store/tests/uuids.rs`).
--
-- Darum löschen zwei BEFORE-DELETE-Trigger die Kinder selbst, solange die Elternzeile noch steht:
-- - vor einem Werk seine Fäden (der Trigger des Fadens räumt dabei dessen Beiträge),
-- - vor einem Faden seine Beiträge.
-- Danach hat die Kaskade nichts mehr zu tun; das Ergebnis (alles fort) ist dasselbe wie vorher, nur die
-- Grabsteine tragen jetzt die Werk-uuid. Kein Umbau einer Tabelle, darum nicht in FOREIGN_KEYS_OFF.
-- Die Namen enden auf `_uuid_tombstone` wie die Grabstein-Trigger aus 0041.

CREATE TRIGGER chat_thread_cascade_uuid_tombstone BEFORE DELETE ON chat_thread
BEGIN DELETE FROM chat_thread_post WHERE thread_id = OLD.id; END;

CREATE TRIGGER work_cascade_uuid_tombstone BEFORE DELETE ON work
BEGIN DELETE FROM chat_thread WHERE work_id = OLD.id; END;
