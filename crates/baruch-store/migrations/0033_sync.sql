-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Runde 13 (abgleich): Abgleich Baruch ↔ MultiGPT. Nur neue Tabellen, keine Datenänderung.
-- sync_work: je Werk der Schalter „nach MultiGPT hochladen“ (Vorgabe aus) und die beim Einschalten
--   festgehaltene Sammlung „Baruch: <Werktitel>“; geht mit dem Werk.
-- sync_collection: die zum Holen gewählten Sammlungen von MultiGPT (lückenlose Reihenfolge).
-- sync_state: je Seite ('baruch' = Kapitel eines Werks, item 'work:<W>/key:<K>'; 'multigpt' = Dokument
--   einer gewählten Sammlung, item = Dokument-ID) der zuletzt abgeglichene Inhalts-Hash. work_id hat
--   absichtlich keinen Fremdschlüssel: Dokumente gelöschter Werke räumt der nächste Abgleich auf.
-- sync_document: lokale Kopien geholter Dokumente (offline lesbar und suchbar).
-- sync_run: Protokoll der Läufe („zuletzt abgeglichen …“).
CREATE TABLE sync_work (
  work_id INTEGER PRIMARY KEY REFERENCES work(id) ON DELETE CASCADE,
  upload INTEGER NOT NULL DEFAULT 0 CHECK (upload IN (0,1)),
  collection TEXT NOT NULL DEFAULT '',
  confirmed TEXT,
  updated TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE TABLE sync_collection (
  collection TEXT PRIMARY KEY CHECK (length(collection) BETWEEN 1 AND 200 AND instr(collection, char(10)) = 0
    AND instr(collection, char(13)) = 0),
  position INTEGER NOT NULL CHECK (position >= 1),
  added TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE TABLE sync_state (
  side TEXT NOT NULL CHECK (side IN ('baruch','multigpt')),
  collection TEXT NOT NULL,
  item TEXT NOT NULL,
  work_id INTEGER,
  node_key INTEGER,
  remote_id TEXT NOT NULL DEFAULT '',
  title TEXT NOT NULL DEFAULT '',
  hash TEXT NOT NULL,
  remote_stamp TEXT NOT NULL DEFAULT '',
  synced TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  PRIMARY KEY (side, collection, item)
);

CREATE TABLE sync_document (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  collection TEXT NOT NULL,
  document_id TEXT NOT NULL,
  title TEXT NOT NULL DEFAULT '',
  url TEXT NOT NULL DEFAULT '',
  text TEXT NOT NULL,
  hash TEXT NOT NULL,
  fetched TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  UNIQUE (collection, document_id)
);

CREATE TABLE sync_run (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  started TEXT NOT NULL,
  finished TEXT,
  ok INTEGER CHECK (ok IN (0,1)),
  message TEXT NOT NULL DEFAULT '',
  uploaded INTEGER NOT NULL DEFAULT 0,
  replaced INTEGER NOT NULL DEFAULT 0,
  removed INTEGER NOT NULL DEFAULT 0,
  downloaded INTEGER NOT NULL DEFAULT 0,
  unchanged INTEGER NOT NULL DEFAULT 0,
  failed INTEGER NOT NULL DEFAULT 0
);
