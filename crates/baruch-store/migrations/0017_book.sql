-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Buchangaben und Umschlag je Werk (Vertrag Runde 11, Abschnitt 1, Aufgabe cover): Umschlagbild,
-- Titelbereich, Titelei an/aus, Umschlagseiten im Satz, Angaben für Impressum und Rückseite,
-- Autorenfoto, Papierdicke und Beschnitt. Nur eine neue Tabelle; bestehende Zeilen bleiben unberührt.
-- Keine Zeile = Vorgaben. Untertitel, Autor und Bibelübersetzung bleiben in `work`.
--
-- Umschlag und Autorenfoto müssen Bilder desselben Werks sein (Trigger). Wird das Bild gelöscht,
-- wird der Verweis NULL; ein Werk zu löschen löscht seine Zeile mit.

CREATE TABLE work_book (
  work_id INTEGER PRIMARY KEY REFERENCES work(id) ON DELETE CASCADE,
  cover_asset_id INTEGER REFERENCES asset(id) ON DELETE SET NULL,
  title_area TEXT NOT NULL DEFAULT 'top' CHECK (title_area IN ('top','middle','bottom')),
  front_matter INTEGER NOT NULL DEFAULT 0 CHECK (front_matter IN (0,1)),   -- Titelei
  cover_pages INTEGER NOT NULL DEFAULT 1 CHECK (cover_pages IN (0,1)),     -- Vorder-/Rückseite im Satz (nur wirksam mit Umschlag)
  publisher TEXT NOT NULL DEFAULT '',
  place TEXT NOT NULL DEFAULT '',
  year TEXT NOT NULL DEFAULT '',
  edition TEXT NOT NULL DEFAULT '',
  isbn TEXT NOT NULL DEFAULT '',
  price TEXT NOT NULL DEFAULT '',
  series TEXT NOT NULL DEFAULT '',
  printer TEXT NOT NULL DEFAULT '',
  bible_notice TEXT NOT NULL DEFAULT '',
  image_credit TEXT NOT NULL DEFAULT '',
  dedication TEXT NOT NULL DEFAULT '',
  blurb TEXT NOT NULL DEFAULT '',
  bio TEXT NOT NULL DEFAULT '',
  author_photo_asset_id INTEGER REFERENCES asset(id) ON DELETE SET NULL,
  paper_mm REAL NOT NULL DEFAULT 0.1 CHECK (paper_mm > 0 AND paper_mm <= 1),   -- mm je Seite
  bleed_mm REAL NOT NULL DEFAULT 3 CHECK (bleed_mm >= 0 AND bleed_mm <= 10),
  updated TEXT
);

CREATE TRIGGER work_book_same_work_insert BEFORE INSERT ON work_book
WHEN (NEW.cover_asset_id IS NOT NULL
      AND NOT EXISTS (SELECT 1 FROM asset WHERE id = NEW.cover_asset_id AND work_id = NEW.work_id))
  OR (NEW.author_photo_asset_id IS NOT NULL
      AND NOT EXISTS (SELECT 1 FROM asset WHERE id = NEW.author_photo_asset_id AND work_id = NEW.work_id))
BEGIN SELECT RAISE(ABORT, 'Umschlag und Autorenfoto müssen Bilder desselben Werks sein'); END;

CREATE TRIGGER work_book_same_work_update BEFORE UPDATE OF work_id, cover_asset_id, author_photo_asset_id ON work_book
WHEN (NEW.cover_asset_id IS NOT NULL
      AND NOT EXISTS (SELECT 1 FROM asset WHERE id = NEW.cover_asset_id AND work_id = NEW.work_id))
  OR (NEW.author_photo_asset_id IS NOT NULL
      AND NOT EXISTS (SELECT 1 FROM asset WHERE id = NEW.author_photo_asset_id AND work_id = NEW.work_id))
BEGIN SELECT RAISE(ABORT, 'Umschlag und Autorenfoto müssen Bilder desselben Werks sein'); END;
