# SPDX-License-Identifier: AGPL-3.0-or-later
# Round 39a (templates_2): Findings page, findings panel and a finding as a block.
# German translation of locales/en/findings.ftl (same keys in the same order).

# Shared by the findings page, the panel and the block.
findings.back_to_work = Zurück zum Werk
findings.status = Status
findings.kind = Art
findings.kind_hint = Hinweis
findings.kind_critical = Kritisch
findings.kind_praise = Lob
findings.open_in_work = offene Befunde im Werk (Abzeichen)
findings.open_count = { $count } offen
findings.by = von { $name }
findings.dash_quote = – „{ $text }“
findings.quoted = „{ $text }“
findings.without_section = ohne Abschnitt
findings.post = Beitrag
findings.place_hint = Die Stelle im Abschnitt öffnen
findings.place_hint_quote = Die Stelle im Abschnitt öffnen und das Zitat markieren
findings.to_place = zur Stelle
findings.jump_hint = Stelle im Abschnitt anspringen

# Page /werke/{id}/befunde (befunde.html).
findings.page.window_title = Befunde · { $title } – Baruch
findings.page.page_title = Befunde · { $title }
findings.page.heading = Befunde in „{ $title }“
findings.page.intro = Befunde sind die einzelnen Punkte aus den Antworten der Personas. Den Status setzt nur der Mensch; Assistenten schlagen höchstens vor. Offen zählt, was weder entschieden noch an einen gleichen Befund angehängt noch Lob ist – dieselbe Zahl steht als Abzeichen „k offen“ in der Gliederung und im Editorkopf.
findings.page.book_note_heading = Befund schreiben: Anmerkung zum Buch
findings.page.book_note_hint = Eine Anmerkung zum ganzen Buch, ohne Abschnitt. Befunde zu einem Abschnitt schreibst du auf der Arbeitsseite (rechts unter „Befunde“). Über den Status entscheidet der Autor.
findings.page.book_note_label = Anmerkung
findings.page.book_note_text = Text der Anmerkung
findings.page.book_note_submit = Anmerkung zum Buch schreiben
findings.page.filter_label = Befunde filtern
findings.page.all = alle
findings.page.since = seit
findings.page.until = vor dem
findings.page.until_hint = bis ausschließlich diesem Tag
findings.page.older_than = älter als
findings.page.days = Tage
findings.page.show_praise = Lob zeigen
findings.page.apply = Anwenden
findings.page.clear_filter = Filter aufheben
findings.page.praise_hide = Lob ausblenden
findings.page.praise_show = Lob einblenden
findings.page.matrix_label = Matrix Kapitel mal Persona
findings.page.matrix_heading = Matrix: Kapitel × Persona
findings.page.signs = Zeichen
findings.page.legend = Zeichen: ○ offen · ✓ übernommen · ✗ verworfen · → verschoben · = erledigt ohne Änderung
findings.page.legend_praise = · + Lob
findings.page.legend_many = . Kapitel mit { $count } oder mehr offenen Befunden sind hervorgehoben.
findings.page.truncated = Gekappt: Die Matrix zählt nur die neuesten { $limit } von { $total } Befunden im Filter; die Abzeichen zählen alle. Ein engerer Filter (Zeitraum, Persona, Status) zeigt den Rest.
findings.page.solutions = Lösungen: { $words } (ohne Lob; je Zeile in der Summe als „mit/ohne“).
findings.page.chapter = Kapitel
findings.page.sum = Summe
findings.page.many_hint = viele offene Befunde
findings.page.many = viele offen
findings.page.all_personas = alle Personas: { $words }
findings.page.sum_of = Summe { $title }
findings.page.total = Gesamt
findings.page.none = Keine Befunde
findings.page.none_for_filter = für diesen Filter
findings.page.none_after = . Befunde entstehen aus den Antworten der Personas (Prüfen, Chat, Fäden).
findings.page.single = Einzelner Befund
findings.page.missing = Befund { $ref } gibt es in diesem Werk nicht.
findings.page.finding = Befund { $ref }
findings.page.attached_to = Angehängt an
findings.page.not_open = – zählt nicht als offen.
findings.page.history = Verlauf
findings.page.proposal = Vorschlag
findings.page.still_open = , offen
findings.page.note = – Notiz: { $note }
findings.page.no_change = Noch keine Änderung (angelegt { $time }).
findings.page.cell_label = Befunde der Zelle
findings.page.cell_missing = Abschnitt { $key } gibt es in diesem Werk nicht (mehr).
findings.page.cell_in = Befunde in
findings.page.back_to_matrix = Zurück zur ganzen Matrix
findings.page.cell_empty = Keine Befunde in dieser Zelle.
findings.page.old_label = Offen, älter als { $days } Tage
findings.page.old_heading = Offen, älter als { $days } Tage ({ $count })
findings.page.show = Zeigen
findings.page.old_truncated = Es gibt { $total } offene Befunde, die älter sind; gezeigt sind die { $limit } neuesten davon. Die älteren stehen in der Matrix (Zelle anklicken) oder erscheinen mit einer größeren Zahl bei „älter als … Tage“.
findings.page.old_none = Keine offenen Befunde, die älter sind.
findings.page.summaries = Sammelvorschläge
findings.page.summaries_heading = Sammelvorschläge ({ $count })
findings.page.summaries_hint = Beim RFC fasst der Lektor die Befunde der anderen zusammen – nur als Vorschlag mit Verweis auf die Ursprünge.
findings.page.summaries_none = Keine Sammelvorschläge.
findings.page.history_per_chapter = Verlauf je Kapitel
findings.page.col_adopted = ✓ eingeflossen
findings.page.col_rejected = ✗ verworfen
findings.page.col_deferred = → verschoben
findings.page.col_done = = erledigt
findings.page.col_revisions = In Fassungen
findings.page.revision = Fassung { $rev }
findings.page.stand_hint = Stand der Fortschreibung beim Übernehmen
findings.page.stand = · Stand { $stand }
findings.page.history_none = Noch keine entschiedenen Befunde.

# Cards (macros of fragments/findings_panel.html, also fragments/finding_block.html).
findings.card.question_label = Rückfrage zu Befund { $ref }
findings.card.question_hint = Rückfrage einer Persona
findings.card.question = Rückfrage
findings.card.question_default = Befund { $id } scheint erledigt – als erledigt markieren?
findings.card.yes_done = Ja, erledigt
findings.card.no_open = Nein, noch offen
findings.card.proposal_hint = offener Vorschlag eines Assistenten
findings.card.proposal = Vorschlag
findings.card.proposes = { $actor } schlägt vor:
findings.card.confirm = Bestätigen
findings.card.reject = Verwerfen
findings.card.set_status = Status setzen …
findings.card.change_status = Status ändern …
findings.card.adopted_label = Befund { $ref } übernommen
findings.card.adopted_in = ✓ übernommen in
findings.card.adopted = Übernommen
findings.card.reject_label = Befund { $ref } verwerfen
findings.card.rejected_because = ✗ verworfen, weil (Pflicht)
findings.card.defer_label = Befund { $ref } verschieben
findings.card.deferred_to_section = → verschoben nach Abschnitt
findings.card.or_idea = oder in Einfall Nr.
findings.card.note = Notiz
findings.card.one_target = Genau ein Ziel: ein Abschnitt oder ein Einfall.
findings.card.defer = Verschieben
findings.card.done_label = Befund { $ref } erledigt
findings.card.done_note = = erledigt ohne Änderung, Notiz
findings.card.done = Erledigt
findings.card.reopen_label = Befund { $ref } wieder öffnen
findings.card.reopen = ○ Wieder öffnen
findings.card.solutions_label = Lösungen zu Befund { $ref }
findings.card.solutions = { $count ->
    [one] Lösung
    *[other] Lösungen
    }
findings.card.instead = statt „{ $quote }“
findings.card.better = besser „{ $replacement }“
findings.card.step = Schritt: { $step }
findings.card.at_quote = (bei „{ $quote }“)
findings.card.adopt_as_proposal = Als Vorschlag übernehmen
findings.card.no_solution = ohne Lösungsvorschlag
findings.card.single = Befund { $id } einzeln
findings.card.kind_is = Art: { $label }
findings.card.status_is = Status: { $label }
findings.card.reason = Begründung: { $reason }
findings.card.summary_by = Sammelvorschlag von { $persona }
findings.card.summary_on = zu
findings.card.origin_missing = (Ursprung fehlt)
findings.card.attached = { $count } angehängt (gleichen diesem Befund, zählen nicht als offen)

# Block of a finding (fragments/finding_block.html).
findings.block.adopted_in_rev = ✓ übernommen in Fassung { $number }
findings.block.done = = erledigt
findings.block.rejected_because = ✗ verworfen, weil
findings.block.more = weitere Möglichkeiten (verschieben …)

# Accordion "Befunde" of the work page (fragments/findings_panel.html).
findings.panel.title = Befunde
findings.panel.open_hint = { $count } offene Befunde in diesem Abschnitt
findings.panel.pending_hint = offene Vorschläge der Assistenten
findings.panel.pending = { $count } { $count ->
    [one] Vorschlag
    *[other] Vorschläge
    }
findings.panel.total = { $count } insgesamt
findings.panel.all_of_section = Alle Befunde dieses Abschnitts
findings.panel.overview = Übersicht übers Werk
findings.panel.variants_label = Lösungen der offenen Befunde in Varianten umsetzen
findings.panel.lector_writes = Lektor schreibt
findings.panel.variants_from_open = Varianten aus den offenen Befunden
findings.panel.apply_solutions = Lösungen umsetzen
findings.panel.none_open = Keine offenen Befunde in diesem Abschnitt.
findings.panel.decided = { $count } entschieden (übernommen, verworfen, verschoben, erledigt)
findings.panel.praise = { $count } Lob (ausgeblendet)
