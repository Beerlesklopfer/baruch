-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Runde 21a („pdf-import“): lokaler Volltext einer als Quelle importierten PDF- oder EPUB-Datei (gelesen ohne
-- MultiGPT: pdftotext bzw. EPUB-Leser). Über ihn zeigt die Dokumentansicht /quelle den Volltext, und Hervorhebungen
-- ohne Zitat lassen sich gegen ihn prüfen – wie bei einer Verknüpfung mit MultiGPT (0106). Nur eine neue, anfangs
-- leere Tabelle; vorhandene Zeilen bleiben unberührt. Ein Titel hat höchstens einen Volltext; er verschwindet mit ihm.

CREATE TABLE library_title_text (
  title_id    INTEGER PRIMARY KEY REFERENCES library_title(id) ON DELETE CASCADE,
  source_name TEXT NOT NULL,
  mime        TEXT NOT NULL,
  sha256      TEXT NOT NULL,
  pages       INTEGER,
  text        TEXT NOT NULL CHECK (typeof(text)='text'),
  created     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
