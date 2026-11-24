-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Statusübergänge der Revisionen: nur proposed → current/rejected und current → superseded.
-- Auch ein Wechsel auf denselben Status wird abgelehnt. INSERT bleibt frei, weil das
-- Beispielwerk (seed.rs) Revisionen gleich mit superseded/rejected anlegt.
CREATE TRIGGER revision_status_transition BEFORE UPDATE OF status ON revision
WHEN NOT ((OLD.status = 'proposed' AND NEW.status IN ('current','rejected')) OR (OLD.status = 'current' AND NEW.status = 'superseded'))
BEGIN SELECT RAISE(ABORT, 'Unzulässiger Statuswechsel der Revision'); END;
