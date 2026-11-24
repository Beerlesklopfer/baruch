-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Zeitwächter (Runde 16b): Sprechgeschwindigkeit und Zielzeit je Werk. Ohne Zeile gilt die Vorgabe
-- (110 Wörter je Minute, keine Zielzeit). Nicht im Spiegel.
CREATE TABLE work_timing (work_id INTEGER PRIMARY KEY REFERENCES work(id) ON DELETE CASCADE, words_per_minute INTEGER NOT NULL CHECK (words_per_minute BETWEEN 60 AND 250), target_seconds INTEGER CHECK (target_seconds IS NULL OR target_seconds BETWEEN 60 AND 14400), updated TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ','now')));
