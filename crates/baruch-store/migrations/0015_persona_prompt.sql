-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Runde 10 (personas): eigener Rollen-Prompt je Persona. NULL = Vorgabe aus dem Code. Keine Datenänderung.
ALTER TABLE persona ADD COLUMN prompt TEXT CHECK (prompt IS NULL OR length(prompt) BETWEEN 1 AND 8000);
