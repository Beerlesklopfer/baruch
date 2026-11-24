-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Runde 13 (websuche): Belege der Websuche von @belege. Je Zitat bzw. genannter Seite eine Zeile mit dem
-- Urteil der Prüfung im Code (belegt = Wortlaut steht auf der abgerufenen Seite). Wird der Beitrag gelöscht,
-- bleibt der Beleg ohne Beitrag (post_id NULL). work_id/node_key ohne Fremdschlüssel: Belege überdauern ein
-- gelöschtes Werk, wie der Spiegel. library_title_id zeigt auf den übernommenen Titel der Autorenbibliothek
-- (nur der Mensch übernimmt). Keine Datenänderung; der Schalter der Websuche liegt ohne Migration in
-- store_meta ('websearch_belege').
CREATE TABLE web_evidence (
  id               INTEGER PRIMARY KEY AUTOINCREMENT,
  post_id          INTEGER REFERENCES post(id) ON DELETE SET NULL,
  work_id          INTEGER,
  node_key         INTEGER,
  persona          TEXT NOT NULL,
  label            TEXT NOT NULL,
  url              TEXT NOT NULL,
  title            TEXT NOT NULL DEFAULT '',
  preferred        INTEGER NOT NULL DEFAULT 0 CHECK (preferred IN (0,1)),
  accessed         TEXT NOT NULL,
  quote            TEXT NOT NULL DEFAULT '',
  verdict          TEXT NOT NULL CHECK (verdict IN ('belegt','nicht_belegt','nicht_zitiert')),
  page_hash        TEXT NOT NULL DEFAULT '',
  library_title_id INTEGER,
  created          TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX web_evidence_post ON web_evidence(post_id);
