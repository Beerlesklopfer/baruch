-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Round 38: work language. Personas answer in this language; book output follows it from Round 39 on.
-- Existing works become 'de'. ADD COLUMN only, no table rebuild (not in FOREIGN_KEYS_OFF).
ALTER TABLE work ADD COLUMN language TEXT NOT NULL DEFAULT 'de' CHECK (language IN ('de', 'en'));
