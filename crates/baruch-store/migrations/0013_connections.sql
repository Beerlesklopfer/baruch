-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Verbindungen und Quellen (Runde 9, Aufgabe „connections“): Einstellungen der Modell- und
-- Such-Backends, je Werk die RAG-Wege und die Lesezeichenliste bevorzugter Quellen. Nur drei neue
-- Tabellen und ein Index; bestehende Tabellen bleiben unberührt.
--
-- Geheimnisse (Schlüssel, Token, Passwörter) stehen NIE hier, sondern im Schlüsselbund des Systems
-- (Dienst „baruch“, Konten „eliezer“, „multigpt“, „pgvector“). Eine fehlende Zeile in `connection`
-- bedeutet die Vorgabe der Verbindung, eine fehlende Zeile in `work_rag` die Vorgabe ["lancedb"].

CREATE TABLE connection (
  kind TEXT PRIMARY KEY CHECK (kind IN ('eliezer','multigpt','pgvector','lancedb')),
  config TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(config) AND json_type(config) = 'object'),  -- nie Geheimnisse
  updated TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  checked TEXT,                       -- Zeit der letzten Prüfung, NULL = nie geprüft
  check_ok INTEGER CHECK (check_ok IN (0,1)),
  check_message TEXT,
  check_details TEXT CHECK (check_details IS NULL OR json_valid(check_details)),
  CHECK ((checked IS NULL) = (check_ok IS NULL))
);

CREATE TABLE work_rag (
  work_id INTEGER PRIMARY KEY REFERENCES work(id) ON DELETE CASCADE,
  ways TEXT NOT NULL CHECK (json_valid(ways) AND json_type(ways) = 'array'),   -- Teilmenge von ["lancedb","pgvector","multigpt"], leer erlaubt (= nur Volltext)
  updated TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE TABLE preferred_source (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  title TEXT NOT NULL CHECK (length(trim(title)) BETWEEN 1 AND 200),
  url TEXT NOT NULL CHECK (length(url) <= 2000 AND (url LIKE 'http://%' OR url LIKE 'https://%')),
  note TEXT NOT NULL DEFAULT '',
  position INTEGER NOT NULL,          -- 1..n lückenlos, vom Store gepflegt
  created TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE INDEX preferred_source_order ON preferred_source(position, id);
