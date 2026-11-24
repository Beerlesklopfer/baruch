-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Ältere Fassungen ausblenden (Nutzer 2026-11-24, Release 0.9.0): je Abschnitt eine Marke „bis hierhin“.
--
-- - `node_revision_fold`: höchstens eine Zeile je Abschnitt. `below` ist die Nummer der Fassung, an der die Marke
--   steht; ausgeblendet werden ersetzte und verworfene Fassungen mit kleinerer Nummer (nie die aktuelle, nie offene
--   Vorschläge; liegt die Marke über der aktuellen Fassung, gilt sie nur bis unter die aktuelle –
--   `baruch_core::revision_fold`). Fehlt die Zeile, ist nichts ausgeblendet.
-- - Nichts wird gelöscht: Fassungen, Verlauf und Spiegel bleiben unverändert; die Marke ist eine Ansichtsmarke wie die
--   Einstellung „Fassungen“ (`work_revision_policy`, 0050): ohne uuid, nicht im Austauschformat, kein Commit im
--   Spiegel; protokolliert in `updated`/`updated_by` (nur der Mensch setzt und hebt sie auf).
-- - Löschen des Abschnitts nimmt die Marke mit (ON DELETE CASCADE).
--
-- Kein FOREIGN_KEYS_OFF: nur eine neue Tabelle.

CREATE TABLE node_revision_fold (
  node_id    INTEGER PRIMARY KEY REFERENCES node(id) ON DELETE CASCADE,
  below      INTEGER NOT NULL CHECK (below >= 1),
  updated    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated_by TEXT NOT NULL DEFAULT '' CHECK (length(updated_by) <= 200)
);
