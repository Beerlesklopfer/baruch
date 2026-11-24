-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Bilder und Grafiken eines Werks (Vertrag Runde 6, Abschnitte 1 und 4): Inhalt als TEXT (Base64 bzw.
-- SVG-Text, nie BLOB), Fassungen wie bei Abschnitten: genau eine aktuelle je Bild (umschaltbar),
-- Ändern (nur Bildunterschrift, Alternativtext, Notiz) und Löschen nur durch den Menschen, Nummern
-- nie wiederverwendet (`asset.last_rev`). Ersetzen ist immer eine neue Fassung. Nur neue Tabellen;
-- bestehende Zeilen bleiben unberührt.

CREATE TABLE asset (
  id       INTEGER PRIMARY KEY AUTOINCREMENT,
  work_id  INTEGER NOT NULL REFERENCES work(id),
  name     TEXT NOT NULL,                -- eindeutig je Werk; Regel in Rust (assets::valid_name)
  last_rev INTEGER NOT NULL DEFAULT 0,   -- höchste je vergebene Fassungsnummer
  created  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  UNIQUE (work_id, name)
);

-- Pfade, wie sie in Vorlage und Text stehen ("/bilder/haus.svg"), → Bild.
CREATE TABLE asset_path (
  work_id  INTEGER NOT NULL REFERENCES work(id),
  path     TEXT NOT NULL,
  asset_id INTEGER NOT NULL REFERENCES asset(id),
  PRIMARY KEY (work_id, path)
);

CREATE TABLE asset_revision (
  id          INTEGER PRIMARY KEY,
  asset_id    INTEGER NOT NULL REFERENCES asset(id),
  number      INTEGER NOT NULL,
  based_on    INTEGER,
  mime        TEXT NOT NULL CHECK (mime IN ('image/png','image/jpeg','image/gif','image/webp','image/svg+xml')),
  encoding    TEXT NOT NULL CHECK (encoding IN ('base64','svg')),
  content     TEXT NOT NULL CHECK (typeof(content) = 'text'),  -- Base64 (Standardalphabet, mit '=', ohne Zeilenumbrüche) bzw. SVG-Text
  bytes       INTEGER NOT NULL,          -- Größe entschlüsselt
  sha256      TEXT NOT NULL,             -- über die entschlüsselten Bytes, Kleinbuchstaben-Hex
  caption     TEXT NOT NULL DEFAULT '',  -- Bildunterschrift
  alt         TEXT NOT NULL DEFAULT '',  -- Alternativtext
  author_name TEXT NOT NULL,
  note        TEXT NOT NULL DEFAULT '',
  status      TEXT NOT NULL CHECK (status IN ('current','superseded')),
  created     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  edited      TEXT,
  UNIQUE (asset_id, number),
  CHECK (encoding = 'base64' OR mime = 'image/svg+xml')
);

CREATE UNIQUE INDEX asset_one_current ON asset_revision(asset_id) WHERE status = 'current';

-- Herkunft (Bild, Nummer, Autor, Zeit) ist unveränderlich.
CREATE TRIGGER asset_revision_origin_immutable
BEFORE UPDATE OF asset_id, number, author_name, created ON asset_revision
BEGIN SELECT RAISE(ABORT, 'Herkunft einer Bildfassung ist unveränderlich'); END;

-- Der Inhalt einer Fassung ist unveränderlich: Ersetzen ist immer eine neue Fassung.
CREATE TRIGGER asset_revision_content_immutable
BEFORE UPDATE OF mime, encoding, content, bytes, sha256 ON asset_revision
BEGIN SELECT RAISE(ABORT, 'Inhalt einer Bildfassung ist unveränderlich; Ersetzen ergibt eine neue Fassung'); END;

-- Umschalten: superseded → current und current → superseded, sonst nichts.
CREATE TRIGGER asset_revision_status_transition BEFORE UPDATE OF status ON asset_revision
WHEN NOT ((NEW.status = 'current' AND OLD.status = 'superseded')
       OR (OLD.status = 'current' AND NEW.status = 'superseded'))
BEGIN SELECT RAISE(ABORT, 'Unzulässiger Statuswechsel der Bildfassung'); END;

CREATE TRIGGER asset_revision_no_delete_current BEFORE DELETE ON asset_revision
WHEN OLD.status = 'current'
BEGIN SELECT RAISE(ABORT, 'Die aktuelle Bildfassung lässt sich nicht löschen'); END;

-- Kinder einer gelöschten Fassung hängen danach an deren Elternteil.
CREATE TRIGGER asset_revision_reparent BEFORE DELETE ON asset_revision
BEGIN
  UPDATE asset_revision SET based_on = OLD.based_on WHERE asset_id = OLD.asset_id AND based_on = OLD.number;
END;

CREATE TRIGGER asset_revision_parent_insert BEFORE INSERT ON asset_revision
WHEN NEW.based_on IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM asset_revision p WHERE p.asset_id = NEW.asset_id AND p.number = NEW.based_on AND p.number < NEW.number)
BEGIN SELECT RAISE(ABORT, 'Elternteil der Bildfassung fehlt'); END;

CREATE TRIGGER asset_revision_parent_update BEFORE UPDATE OF based_on ON asset_revision
WHEN NEW.based_on IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM asset_revision p WHERE p.asset_id = NEW.asset_id AND p.number = NEW.based_on AND p.number < NEW.number)
BEGIN SELECT RAISE(ABORT, 'Elternteil der Bildfassung fehlt'); END;

CREATE TRIGGER asset_revision_number_fresh BEFORE INSERT ON asset_revision
WHEN NEW.number <= (SELECT last_rev FROM asset WHERE id = NEW.asset_id)
BEGIN SELECT RAISE(ABORT, 'Nummern von Bildfassungen werden nicht wiederverwendet'); END;

CREATE TRIGGER asset_revision_number_bump AFTER INSERT ON asset_revision
BEGIN UPDATE asset SET last_rev = NEW.number WHERE id = NEW.asset_id; END;
