-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Sammlungen je Werk und Remote-Weg (Runde 12 „Verbindungen nutzen, RAG“): welche Sammlungen von pgvector
-- bzw. MultiGPT die Bedeutungssuche eines Werks durchsucht, in der Reihenfolge der Wahl. Keine Zeile = keine
-- Sammlung gewählt (der Weg bleibt „aus“). Eine gemeinsame Tabelle für beide Remote-Wege; LanceDB hat keine
-- Sammlungen. Keine Datenänderung; ein gelöschtes Werk nimmt seine Zeilen mit.
CREATE TABLE work_rag_collection (
  work_id INTEGER NOT NULL REFERENCES work(id) ON DELETE CASCADE,
  way TEXT NOT NULL CHECK (way IN ('pgvector','multigpt')),
  collection TEXT NOT NULL CHECK (length(collection) BETWEEN 1 AND 200 AND instr(collection, char(10)) = 0 AND instr(collection, char(13)) = 0),
  position INTEGER NOT NULL CHECK (position >= 1),        -- 1..n lückenlos je (work_id, way), vom Store gepflegt
  PRIMARY KEY (work_id, way, collection)
);
