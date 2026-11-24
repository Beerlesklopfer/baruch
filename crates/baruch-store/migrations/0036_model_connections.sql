-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Modell-Verbindungen und Verbindung je Persona (Runde 15b „anbieter“). Nur zwei neue Tabellen und ein
-- Trigger; bestehende Tabellen bleiben unberührt.
--
-- model_connection: Modell-Backends der Art eliezer, openai (OpenAI-kompatibel) oder ollama. Die Verbindung
-- „Eliezer“ (id 1) entsteht hier aus der bisherigen Verbindung `eliezer`: Ihre Adresse und die Bindung ihres
-- Schlüssels stehen weiter in `connection` (url hier NULL, nicht doppelt geführt), der Schlüssel bleibt im
-- Konto baruch/eliezer. Neue Verbindungen haben eine eigene Adresse und das Konto baruch/model-<id>, an ihr
-- Ziel gebunden über secret_target (wie Migration 0014). Geheimnisse stehen NIE hier.
--
-- persona_backend: je Persona die Verbindung und die Freigabe für externe Verbindungen (Vorgabe 0). Keine
-- Zeile = Verbindung Eliezer, nicht extern freigegeben.

CREATE TABLE model_connection (
  id INTEGER PRIMARY KEY,               -- ohne AUTOINCREMENT (sqlite_sequence bleibt unberührt); der Store vergibt ids nie doppelt (store_meta model_connection_last)
  name TEXT NOT NULL UNIQUE COLLATE NOCASE CHECK (length(trim(name)) BETWEEN 1 AND 80),
  kind TEXT NOT NULL CHECK (kind IN ('eliezer','openai','ollama')),
  url TEXT CHECK (url IS NULL OR (length(url) <= 2000 AND (url LIKE 'http://%' OR url LIKE 'https://%'))),
  external INTEGER NOT NULL DEFAULT 0 CHECK (external IN (0,1)),
  account TEXT NOT NULL UNIQUE,         -- Konto im Schlüsselbund (Dienst „baruch“), nie das Geheimnis
  secret_target TEXT,                   -- Ziel, für das das Geheimnis hinterlegt wurde (NULL = das jetzige)
  probe TEXT CHECK (probe IS NULL OR json_valid(probe)),        -- letzter Kurztest
  checked TEXT CHECK (checked IS NULL OR json_valid(checked)),  -- letzte Prüfung
  position INTEGER NOT NULL,
  created TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  CHECK (url IS NOT NULL OR id = 1)
);

INSERT INTO model_connection (id, name, kind, url, external, account, position)
VALUES (1, 'Eliezer', 'eliezer', NULL, 0, 'eliezer', 1);

CREATE TRIGGER model_connection_keep_eliezer BEFORE DELETE ON model_connection
WHEN OLD.id = 1
BEGIN
  SELECT RAISE(ABORT, 'Die Verbindung Eliezer lässt sich nicht löschen');
END;

CREATE TABLE persona_backend (
  persona_id INTEGER PRIMARY KEY REFERENCES persona(id) ON DELETE CASCADE,
  connection_id INTEGER NOT NULL REFERENCES model_connection(id) ON DELETE CASCADE,
  external_allowed INTEGER NOT NULL DEFAULT 0 CHECK (external_allowed IN (0,1)),
  updated TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
