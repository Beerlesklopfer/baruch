-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Gliederungsmanager (Vertrag Runde 4, Abschnitte 1 und 2): Nummerierung je Knoten an- und
-- abschaltbar, Nummerierungsmuster je Werk, Titel mit aufgelösten Typst-Maskierungen.
-- Revisionstexte bleiben unverändert (die Überschriftszeile im Text bleibt wörtlich).

-- (a) Zählt der Knoten mit? Knoten ohne Nummer (z. B. das Vorwort des Beispielwerks) zählen nicht.
ALTER TABLE node ADD COLUMN numbered INTEGER NOT NULL DEFAULT 1 CHECK (numbered IN (0,1));
UPDATE node SET numbered = 0 WHERE trim(number) = '';

-- Die Saat (baruch-core sample.rs) legt das Vorwort ohne Nummer an; es bleibt ohne Nummer.
CREATE TRIGGER node_numbered_default AFTER INSERT ON node WHEN trim(NEW.number) = ''
BEGIN UPDATE node SET numbered = 0 WHERE id = NEW.id; END;

-- (b) Nummerierungsmuster des Werks (Typst, z. B. „I.1“); '' = keine Nummern, NULL = aus der
-- Werkvorlage (#set heading(numbering: …)), sonst „1.1“.
ALTER TABLE work ADD COLUMN numbering TEXT;

-- (c) Titel mit aufgelösten Maskierungen wie baruch_typst::unescape_title: erst '\\' → char(1),
-- dann '\X' → 'X' für X aus ' " # * _ $ @ < > [ ] ` ~ = - / + . und zuletzt char(1) → '\'.
UPDATE node SET title =
  replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(
  replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(
    title,
    '\\', char(1)),
    '\''', ''''),
    '\"', '"'),
    '\#', '#'),
    '\*', '*'),
    '\_', '_'),
    '\$', '$'),
    '\@', '@'),
    '\<', '<'),
    '\>', '>'),
    '\[', '['),
    '\]', ']'),
    '\`', '`'),
    '\~', '~'),
    '\=', '='),
    '\-', '-'),
    '\/', '/'),
    '\+', '+'),
    '\.', '.'),
    char(1), '\')
WHERE instr(title, char(92)) > 0;
