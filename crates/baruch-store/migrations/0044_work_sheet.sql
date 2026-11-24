-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Satzbild-Einstellungen je Werk (Runde 18c): überschreiben die aus der Werkvorlage abgeleitete Darstellung des
-- HTML-Satzbilds (nie Typst, Export oder PDF). Höchstens eine Zeile je Werk; NULL = aus der Vorlage; ohne Zeile gilt
-- alles aus der Vorlage. Nicht im Spiegel, ohne uuid, nicht im Austauschformat. Die genaue Prüfung der Werte
-- (Schriftliste, Längen mit Einheit und Grenzen, Farben #rrggbb) macht baruch_store::sheet; die CHECK-Regeln sind die
-- äußere Grenze der Längen.
CREATE TABLE work_sheet (
    work_id INTEGER PRIMARY KEY REFERENCES work(id) ON DELETE CASCADE,
    font TEXT NULL CHECK (font IS NULL OR length(font) BETWEEN 1 AND 200),
    heading_font TEXT NULL CHECK (heading_font IS NULL OR length(heading_font) BETWEEN 1 AND 200),
    size TEXT NULL CHECK (size IS NULL OR length(size) BETWEEN 3 AND 16),
    fill TEXT NULL CHECK (fill IS NULL OR (length(fill) = 7 AND substr(fill, 1, 1) = '#')),
    heading_fill TEXT NULL CHECK (heading_fill IS NULL OR (length(heading_fill) = 7 AND substr(heading_fill, 1, 1) = '#')),
    background TEXT NULL CHECK (background IS NULL OR (length(background) = 7 AND substr(background, 1, 1) = '#')),
    width TEXT NULL CHECK (width IS NULL OR length(width) BETWEEN 3 AND 16),
    updated TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now'))
);
