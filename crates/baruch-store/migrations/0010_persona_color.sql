-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Runde 7 (proposals): Farbe je Persona. NULL = Vorgabe aus der Palette nach Reihenfolge (position, id),
-- sonst #rrggbb in Kleinbuchstaben. Keine Datenänderung.
ALTER TABLE persona ADD COLUMN color TEXT CHECK (color IS NULL OR (length(color) = 7 AND color GLOB '#[0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f]'));
