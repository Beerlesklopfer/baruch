# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of the outline, numbering and revisions (Round 39a, task messages_1). Base catalog in English; locales/de/outline_msg.ftl has the same keys in
# the same order with the German wording.

# Outline (outline.rs).
outline_msg.param.number = { $name }: Zahl ab 0 erwartet
outline_msg.param.missing = { $name } fehlt
outline_msg.param.too_large = { $name }: zu groß
outline_msg.param.text_missing = { $name } fehlt (Text)
outline_msg.param.place = place: before, after oder into erwartet
outline_msg.param.decision = decision: confirm oder reject erwartet
outline_msg.param.no_lock = Schreibsperre fehlt
outline_msg.param.numbering = numbering: Text oder null erwartet
outline_msg.param.level_1_3 = level: 1 bis 3
outline_msg.param.level_0_3 = level: 0 bis 3
outline_msg.param.numbered = numbered fehlt (true = mit Nummer, false = ohne)
outline_msg.param.confirm_delete = Löschen bitte bestätigen (confirm).
outline_msg.param.work_number = work: Werk-id (Zahl) erwartet
outline_msg.preview.unnumbered = { $word } (ohne Nummer)
outline_msg.page.template_error = Vorlage { $name }: { $error }
outline_msg.page.error = Fehler: { $error }
outline_msg.page.unknown_work = unbekanntes Werk
outline_msg.page.no_outline = Das Werk hat keine Gliederung.
outline_msg.page.unknown_action = unbekannte Aktion

# Numbering schemes (numbering.rs).
outline_msg.numbering.scheme_missing = scheme fehlt (id eines Gliederungsschemas, z. B. „decimal“)
outline_msg.numbering.custom = custom: Text oder null erwartet
outline_msg.numbering.template_error = Fehler: Vorlage { $name }: { $error }
outline_msg.numbering.adopted = Das Gliederungsschema ist übernommen.
outline_msg.numbering.work_missing = Fehler: work fehlt (Werk-id)
outline_msg.numbering.choose = Bitte ein Gliederungsschema wählen.

# Revision history and policy (revisions.rs, revision_fold.rs).
outline_msg.revision.node_missing = node oder key fehlt
outline_msg.revision.switch = { $name }: an oder aus erwartet
outline_msg.revision.max_change = Größte Änderung: 1 bis 100 Prozent.
outline_msg.revision.new_after = Neue Fassung nach: 0 bis 1440 Minuten.
outline_msg.revision.amend = Fortschreibung
outline_msg.revision.undo = Rücknahme
outline_msg.revision.key_rev_missing = key und rev fehlen (Abschnitt und Fassung).
outline_msg.revision.undone = Zurückgenommen: der Text steht wieder auf dem gewählten Stand.
outline_msg.fold.date_format = date: Tag als JJJJ-MM-TT erwartet
outline_msg.fold.date_invalid = date: ungültiger Tag
outline_msg.fold.date_missing = date fehlt (JJJJ-MM-TT)
