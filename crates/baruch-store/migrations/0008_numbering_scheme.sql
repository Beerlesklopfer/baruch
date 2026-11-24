-- SPDX-License-Identifier: AGPL-3.0-or-later
-- Gliederungsschemata je Werk (Vertrag Runde 6, Abschnitt 1): welches Schema die Nummern bestimmt.
-- work.numbering bleibt unverändert: beim Schema 'custom' das eigene Muster in der Mustersprache
-- (baruch_typst::scheme), bei den anderen Schemata ungelesen und beim Wechsel nicht gelöscht, damit
-- das eigene Muster beim Zurückschalten erhalten bleibt.
ALTER TABLE work ADD COLUMN numbering_scheme TEXT NOT NULL DEFAULT 'template'
  CHECK (numbering_scheme IN ('template','decimal','decimal_dot','alphanumeric','chapter_roman',
                              'chapter_arabic','parts_chapters','chapter_words','legal','sermon','none','custom'));

-- Bisherige Einstellung übernehmen: NULL = aus der Werkvorlage, '' = keine Nummern, sonst ein Muster.
UPDATE work SET numbering_scheme = CASE
  WHEN numbering IS NULL THEN 'template'
  WHEN numbering = '' THEN 'none'
  ELSE 'custom'
END;
