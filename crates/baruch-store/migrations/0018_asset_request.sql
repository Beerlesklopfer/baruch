-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Anträge zu Bildern (Vertrag Runde 11, Muster 0012 outline_request): Assistenten und MCP-Clients dürfen
-- Bilder sofort anlegen (MCP asset_add), aber ein Bild ersetzen, als Umschlag setzen bzw. den Umschlag
-- entfernen oder löschen nur beantragen. Der Mensch bestätigt (dann führt Baruch den Antrag aus) oder
-- lehnt ab. Nur eine neue Tabelle; bestehende Zeilen bleiben unberührt. Ein Werk zu löschen löscht seine
-- Anträge mit.
--
-- `asset_name` ist das Bild (bei set_cover '' = Umschlag entfernen). `params` ist JSON:
--   replace:   {mime, encoding, content, caption?, alt?, expected_sha256}
--   delete:    {rev?, expected_sha256}
--   set_cover: {expected_cover}   (Umschlag beim Antrag, null = keiner)
-- `expected_*` ist der Stand beim Antrag; passt er beim Bestätigen nicht mehr, wird der Antrag als
-- „stale“ mit Grund abgeschlossen und nichts geändert.

CREATE TABLE asset_request (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  work_id INTEGER NOT NULL REFERENCES work(id) ON DELETE CASCADE,
  op TEXT NOT NULL CHECK (op IN ('replace','set_cover','delete')),
  asset_name TEXT NOT NULL DEFAULT '',             -- '' nur bei set_cover = Umschlag entfernen
  params TEXT NOT NULL CHECK (json_valid(params)),
  origin_kind TEXT NOT NULL CHECK (origin_kind IN ('persona','mcp')),
  origin_name TEXT NOT NULL,                       -- @handle bzw. @mcp:<client>
  note TEXT NOT NULL DEFAULT '',                   -- Begründung des Antragstellers
  created TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  decided TEXT,                                    -- NULL = offen
  decision TEXT CHECK (decision IN ('confirmed','rejected','stale')),
  decided_by TEXT,                                 -- der Mensch, der entschieden hat
  message TEXT,                                    -- bei „stale“: warum veraltet bzw. nicht ausführbar
  CHECK ((decided IS NULL) = (decision IS NULL))
);

CREATE INDEX asset_request_open ON asset_request(work_id, decided);
