-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Suchen/Ersetzen, Lösungsvorschläge, Varianten und Chat-Bedienung (Runde 30, Vertrag §1.1; S1 schreibt sie,
-- danach eingefroren).
--
-- - `post` und `chat_thread_post`: Metadaten eines Beitrags (Runde 30c/30b):
--   - `basis_rev`/`basis_label`: die geprüfte Fassung („geprüft: aktuelle Fassung n“), NULL bzw. '' = unbekannt;
--   - `reply_to`: Beitrag, auf den dieser antwortet (Relay, Antwort auf die Nachricht des Menschen);
--   - `repeat_of`: Beitrag, den dieser wiederholt (↻);
--   - `relay_from`: Handle der Persona, die angeredet hat (Relay im Abschnitts-Chat);
--   - `request`: JSON `{"persona","mode","scope","text","rev"}` des Auftrags (für „wiederholen“).
--   Verweise auf Beiträge mit ON DELETE SET NULL: Löschen eines Beitrags lässt die Antworten stehen.
-- - `finding.solutions`: Lösungsvorschläge zu einem Befund, JSON-Liste `[{quote, replacement, step}]` (Runde 30b).
-- - `revision.variant_*`: Varianten als Geschwister im Fassungsbaum (Runde 30b): Gruppe (uuid, klein), Etikett
--   A/B/C, eine Zeile Zusammenfassung, umgesetzte Befunde (JSON-Liste von ids). `variant_label` und `variant_group`
--   sind entweder beide NULL oder beide gesetzt; das prüft der Code (`Store::add_proposal_on`), denn ein CHECK über
--   zwei Spalten lässt sich per ALTER TABLE nicht hinzufügen.
-- - `replace_batch`: Protokoll eines werkweiten Ersetzens (Runde 30a), je Abschnitt `{key, prev_rev, new_rev}`, für
--   „Rückgängig“ (`undone`). Ein lokales Protokoll wie die Sync-Zustände: keine uuid, kein Grabstein, nicht im
--   Austauschformat (so auch in docs/Austauschformat.md). Ebenso bleibt `request` lokal.
--
-- Kein Tabellenumbau, kein FOREIGN_KEYS_OFF, keine neue Grabstein-Art: nur ADD COLUMN (mit Vorgaben bzw. NULL),
-- ein Teilindex und eine neue Tabelle (Beleg in tests/migration_0047.rs).
--
-- Reserviert (Runde 30): 0048 suchen-verweise, 0049 chat-loesungen, 0050 chat-bedienung, 0051 Integrator (alle
-- erwartet unbenutzt); Bibliothek 0108 suchen-verweise, 0109 chat-bedienung.

ALTER TABLE post ADD COLUMN basis_rev INTEGER CHECK (basis_rev IS NULL OR basis_rev >= 1);
ALTER TABLE post ADD COLUMN basis_label TEXT NOT NULL DEFAULT '' CHECK (length(basis_label) <= 300);
ALTER TABLE post ADD COLUMN reply_to INTEGER REFERENCES post(id) ON DELETE SET NULL;
ALTER TABLE post ADD COLUMN repeat_of INTEGER REFERENCES post(id) ON DELETE SET NULL;
ALTER TABLE post ADD COLUMN relay_from TEXT CHECK (relay_from IS NULL OR length(relay_from) BETWEEN 1 AND 200);
ALTER TABLE post ADD COLUMN request TEXT CHECK (request IS NULL OR json_valid(request));
ALTER TABLE chat_thread_post ADD COLUMN basis_rev INTEGER CHECK (basis_rev IS NULL OR basis_rev >= 1);
ALTER TABLE chat_thread_post ADD COLUMN basis_label TEXT NOT NULL DEFAULT '' CHECK (length(basis_label) <= 300);
ALTER TABLE chat_thread_post ADD COLUMN reply_to INTEGER REFERENCES chat_thread_post(id) ON DELETE SET NULL;
ALTER TABLE chat_thread_post ADD COLUMN repeat_of INTEGER REFERENCES chat_thread_post(id) ON DELETE SET NULL;
ALTER TABLE chat_thread_post ADD COLUMN relay_from TEXT CHECK (relay_from IS NULL OR length(relay_from) BETWEEN 1 AND 200);
ALTER TABLE chat_thread_post ADD COLUMN request TEXT CHECK (request IS NULL OR json_valid(request));
ALTER TABLE finding ADD COLUMN solutions TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(solutions) AND json_type(solutions) = 'array');
ALTER TABLE revision ADD COLUMN variant_group TEXT CHECK (variant_group IS NULL OR (length(variant_group) = 36 AND variant_group = lower(variant_group)));
ALTER TABLE revision ADD COLUMN variant_label TEXT CHECK (variant_label IS NULL OR variant_label IN ('A','B','C'));
ALTER TABLE revision ADD COLUMN variant_summary TEXT NOT NULL DEFAULT '' CHECK (length(variant_summary) <= 300);
ALTER TABLE revision ADD COLUMN variant_findings TEXT NOT NULL DEFAULT '[]' CHECK (json_valid(variant_findings) AND json_type(variant_findings) = 'array');
CREATE INDEX revision_variant_group ON revision(variant_group) WHERE variant_group IS NOT NULL;
CREATE TABLE replace_batch (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  work_id INTEGER NOT NULL REFERENCES work(id) ON DELETE CASCADE,
  query TEXT NOT NULL CHECK (length(query) BETWEEN 1 AND 500),
  replacement TEXT NOT NULL CHECK (length(replacement) <= 2000),
  options TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(options)),
  items TEXT NOT NULL CHECK (json_valid(items) AND json_type(items) = 'array'),   -- [{key, prev_rev, new_rev}]
  created TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  undone TEXT
);
CREATE INDEX replace_batch_work ON replace_batch(work_id, id);
