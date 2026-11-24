-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Bindung des Geheimnisses einer Verbindung an ihr Ziel (Nachbesserung Runde 9): Ein Schlüssel, Token
-- oder Passwort im Schlüsselbund gilt nur für das Ziel, für das es hinterlegt wurde. `secret_target`
-- nennt dieses Ziel (bei Eliezer/MultiGPT der Ursprung der Adresse, bei pgvector Rechner, Port und TLS),
-- sobald sich die Einstellungen davon entfernen; NULL heißt „gilt für das jetzige Ziel“. Nie ein
-- Geheimnis, nur Adressangaben. Nur eine neue Spalte, alle Zeilen bleiben (NULL = wie bisher).

ALTER TABLE connection ADD COLUMN secret_target TEXT;
