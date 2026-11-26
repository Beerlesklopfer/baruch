-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Runde 35 „Konten und Rollen“ (Schnittstelle R35-S1, fachlich: rollen): Freigabe eines Werks für ein Lektor-Konto.
-- Nur CREATE (nicht in FOREIGN_KEYS_OFF). Instanzdaten: nicht im Austauschformat, kein Commit im Spiegel (Zeitpunkt
-- in `granted`). Inhalt nach S1 unveränderlich.
CREATE TABLE work_member (
  work_id INTEGER NOT NULL REFERENCES work(id) ON DELETE CASCADE,
  account_id INTEGER NOT NULL REFERENCES account(id) ON DELETE CASCADE,
  role TEXT NOT NULL CHECK (role IN ('lektor')),
  granted TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  PRIMARY KEY (work_id, account_id)
);
CREATE INDEX work_member_account ON work_member(account_id);
