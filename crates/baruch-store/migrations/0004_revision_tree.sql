-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Revisionsbaum (Vertrag Runde 3, Abschnitt 2): Jede Revision hat ein Elternteil (`based_on`), je
-- Abschnitt trägt genau eine die Markierung `current` (Teilindex `revision_one_current` bleibt), und
-- die Markierung lässt sich auf jede andere Fassung umschalten. Ändern (Text, Notiz) und Löschen gibt
-- es nur für den Menschen; die Herkunft einer Revision bleibt unveränderlich, die aktuelle Fassung
-- lässt sich nicht löschen, Nummern werden nie wiederverwendet.

-- (a) Die alten Regeln „nur anhängen“ und „Statuswechsel nur vorwärts“ fallen weg.
DROP TRIGGER revision_no_delete;
DROP TRIGGER revision_immutable;
DROP TRIGGER revision_status_transition;

-- (b) Zeitpunkt der letzten Änderung durch den Menschen, NULL = nie geändert.
ALTER TABLE revision ADD COLUMN edited TEXT;

-- (c) Höchste je vergebene Revisionsnummer des Abschnitts; die nächste ist last_rev + 1.
ALTER TABLE node ADD COLUMN last_rev INTEGER NOT NULL DEFAULT 0;
UPDATE node SET last_rev = coalesce((SELECT max(number) FROM revision WHERE node_id = node.id), 0);

-- (d) Elternteil nachtragen (Regel R): die höchste frühere Fassung desselben Abschnitts, die aktuell
-- war oder ist (current, superseded). Gibt es keine, bleibt die Revision eine Wurzel.
UPDATE revision
   SET based_on = (SELECT max(p.number) FROM revision p
                    WHERE p.node_id = revision.node_id AND p.number < revision.number
                      AND p.status IN ('current','superseded'))
 WHERE number > 1 AND based_on IS NULL;

-- (e) Neue Regeln.
CREATE TRIGGER revision_origin_immutable
BEFORE UPDATE OF node_id, number, author_kind, author_name, model, created ON revision
BEGIN SELECT RAISE(ABORT, 'Herkunft einer Revision ist unveränderlich'); END;

-- Umschalten: jede andere Fassung darf aktuell werden, die bisherige wird superseded; Vorschläge
-- lassen sich verwerfen. Derselbe Status gilt nicht als Wechsel.
CREATE TRIGGER revision_status_transition BEFORE UPDATE OF status ON revision
WHEN NOT ((NEW.status = 'current' AND OLD.status IN ('proposed','rejected','superseded'))
       OR (OLD.status = 'current' AND NEW.status = 'superseded')
       OR (OLD.status = 'proposed' AND NEW.status = 'rejected'))
BEGIN SELECT RAISE(ABORT, 'Unzulässiger Statuswechsel der Revision'); END;

CREATE TRIGGER revision_no_delete_current BEFORE DELETE ON revision
WHEN OLD.status = 'current'
BEGIN SELECT RAISE(ABORT, 'Die aktuelle Fassung lässt sich nicht löschen'); END;

-- Kinder einer gelöschten Fassung hängen danach an deren Elternteil.
CREATE TRIGGER revision_reparent BEFORE DELETE ON revision
BEGIN
  UPDATE revision SET based_on = OLD.based_on WHERE node_id = OLD.node_id AND based_on = OLD.number;
END;

CREATE TRIGGER revision_parent_insert BEFORE INSERT ON revision
WHEN NEW.based_on IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM revision p WHERE p.node_id = NEW.node_id AND p.number = NEW.based_on AND p.number < NEW.number)
BEGIN SELECT RAISE(ABORT, 'Elternteil der Revision fehlt'); END;

CREATE TRIGGER revision_parent_update BEFORE UPDATE OF based_on ON revision
WHEN NEW.based_on IS NOT NULL AND NOT EXISTS (
  SELECT 1 FROM revision p WHERE p.node_id = NEW.node_id AND p.number = NEW.based_on AND p.number < NEW.number)
BEGIN SELECT RAISE(ABORT, 'Elternteil der Revision fehlt'); END;

CREATE TRIGGER revision_number_fresh BEFORE INSERT ON revision
WHEN NEW.number <= (SELECT last_rev FROM node WHERE id = NEW.node_id)
BEGIN SELECT RAISE(ABORT, 'Revisionsnummern werden nicht wiederverwendet'); END;

CREATE TRIGGER revision_number_bump AFTER INSERT ON revision
BEGIN UPDATE node SET last_rev = NEW.number WHERE id = NEW.node_id; END;
