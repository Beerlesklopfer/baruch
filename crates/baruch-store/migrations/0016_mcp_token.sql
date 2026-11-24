-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Runde 10 (scripture): Tokens für den MCP-Zugang von anderen Rechnern. Nur der SHA-256-Hash (hex) wird gespeichert, nie der Klartext.
CREATE TABLE mcp_token (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL CHECK (length(trim(name)) BETWEEN 1 AND 64),
  hash TEXT NOT NULL UNIQUE CHECK (length(hash) = 64),
  prefix TEXT NOT NULL,
  created TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  last_used TEXT,
  revoked TEXT
);
CREATE UNIQUE INDEX mcp_token_active_name ON mcp_token(name) WHERE revoked IS NULL;
