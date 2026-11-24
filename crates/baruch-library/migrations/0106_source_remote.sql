-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Runde 13 („quellen“): Titel der Bibliothek mit einem Dokument in MultiGPT verknüpfen (Volltext über
-- read_document) und das Ergebnis der Prüfung einer Hervorhebung gegen diesen Volltext merken.
-- Beide Tabellen sind neu und anfangs leer; vorhandene Zeilen bleiben unberührt.

CREATE TABLE library_title_remote (
  title_id    INTEGER PRIMARY KEY REFERENCES library_title(id) ON DELETE CASCADE,
  way         TEXT NOT NULL CHECK (way IN ('multigpt')),
  collection  TEXT NOT NULL DEFAULT '',
  document    TEXT NOT NULL CHECK (length(document) BETWEEN 1 AND 500)
);

CREATE TABLE library_highlight_check (
  highlight_id INTEGER PRIMARY KEY REFERENCES library_highlight(id) ON DELETE CASCADE,
  result       TEXT NOT NULL CHECK (result IN ('found','not_found','unverifiable')),
  message      TEXT NOT NULL DEFAULT '',
  checked      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
