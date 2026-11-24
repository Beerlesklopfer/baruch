-- Werkvorlage und Herkunft eines importierten Werks (Vertrag B1.5, Abschnitt 1).
ALTER TABLE work ADD COLUMN template TEXT NOT NULL DEFAULT '';   -- Präambel bis zur ersten Überschrift
ALTER TABLE work ADD COLUMN source_name TEXT;                     -- Dateiname beim Import (NULL = nicht importiert)
ALTER TABLE work ADD COLUMN source_sha256 TEXT;                   -- Hex, SHA-256 des Quelltexts nach CRLF→LF
CREATE UNIQUE INDEX work_source_sha256 ON work(source_sha256) WHERE source_sha256 IS NOT NULL;
