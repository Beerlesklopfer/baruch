-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Runde 35 „Konten und Rollen“ (Schnittstelle R35-S1, fachlich: konten): Konten, Sitzungen und die Sperre gegen
-- Durchprobieren. Nur CREATE, kein Tabellenumbau (nicht in FOREIGN_KEYS_OFF). Instanzdaten: nicht im Austauschformat,
-- nicht im Spiegel. `password` trägt nur den Argon2id-PHC-Text, `account_session.token_hash` nur SHA-256 (hex).
-- Inhalt nach S1 unveränderlich.
CREATE TABLE account (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 40),
  name_key TEXT NOT NULL UNIQUE,
  slug TEXT NOT NULL UNIQUE CHECK (length(slug) BETWEEN 1 AND 40),
  display TEXT NOT NULL DEFAULT '' CHECK (length(display) <= 80),
  kind TEXT NOT NULL CHECK (kind IN ('author','lektor')),
  password TEXT NOT NULL CHECK (password LIKE '$argon2id$%'),
  active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0,1)),
  created TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  password_changed TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  last_login TEXT
);
CREATE TABLE account_session (
  token_hash TEXT PRIMARY KEY CHECK (length(token_hash) = 64),
  account_id INTEGER NOT NULL REFERENCES account(id) ON DELETE CASCADE,
  created TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  last_seen TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE INDEX account_session_account ON account_session(account_id);
CREATE TABLE login_attempt (
  scope TEXT PRIMARY KEY CHECK (length(scope) BETWEEN 1 AND 200),
  failures INTEGER NOT NULL DEFAULT 0 CHECK (failures >= 0),
  last TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  blocked_until TEXT
);
