# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of glossary, index, cross references, search and scripture check (Round 39a, task messages_1). Base catalog in English; locales/de/reference_msg.ftl has the same keys in
# the same order with the German wording.

# Glossary (glossary.rs, glossary_page.rs).
reference_msg.glossary.unknown_work = Unbekanntes Werk.
reference_msg.glossary.ok_add = Begriff ins Glossar aufgenommen.
reference_msg.glossary.ok_update = Begriff geändert.
reference_msg.glossary.ok_delete = Begriff gelöscht (wiederherstellbar über den Spiegel).
reference_msg.glossary.ok_accept = Vorschlag übernommen – der Begriff steht jetzt im Glossar.
reference_msg.glossary.ok_reject = Vorschlag verworfen.
reference_msg.glossary.suggest_none = Der Lektor hat keine neuen Begriffe vorgeschlagen.
reference_msg.glossary.suggest_one = Der Lektor hat { $count } Begriff vorgeschlagen – er steht unter „Vorschläge der Assistenten“.
reference_msg.glossary.suggest_many = Der Lektor hat { $count } Begriffe vorgeschlagen – sie stehen unter „Vorschläge der Assistenten“.
reference_msg.glossary.suggest_skipped = { $count } übersprungen (schon im Glossar oder vorgeschlagen bzw. unbrauchbar).
reference_msg.page.template_error = Vorlage { $name }: { $error }
reference_msg.page.error = Fehler: { $error }
reference_msg.glossary.id_missing = Begriff fehlt (Feld id).
reference_msg.glossary.confirm_delete = Löschen bitte mit dem Häkchen bestätigen; nichts geändert.
reference_msg.glossary.key_number = Abschnitt „{ $key }“: Zahl erwartet.
reference_msg.glossary.unknown_action = Unbekannte Aktion (add, update, delete, accept, reject oder suggest).
reference_msg.marking.key_missing = Welcher Abschnitt? Der Link braucht key (und wahlweise rev).
reference_msg.marking.no_revision = Fassung { $rev } gibt es in diesem Abschnitt nicht.
reference_msg.marking.no_section = Abschnitt mit Schlüssel { $key } gibt es nicht (mehr).
reference_msg.param.number = { $name }: Zahl erwartet
reference_msg.param.text = { $name }: Text erwartet
reference_msg.param.missing = { $name } fehlt
reference_msg.param.flag = { $name }: true oder false erwartet
reference_msg.param.text_list = { $name }: Liste von Texten erwartet
reference_msg.param.confirm_delete = Löschen bitte bestätigen (confirm).
reference_msg.glossary.ref_unreadable = Bibelstelle „{ $reference }“: nicht lesbar
reference_msg.glossary.ref_problem = Bibelstelle „{ $reference }“: { $problem }
reference_msg.glossary.status = status: accepted, proposed oder all, nicht „{ $other }“
reference_msg.index.status = status: open oder all, nicht „{ $other }“

# Index (index_page.rs, index_marks.rs).
reference_msg.index.ok_mark = Registermarke gesetzt – als neue Fassung des Abschnitts.
reference_msg.index.ok_unmark = Registermarke entfernt – als neue Fassung des Abschnitts.
reference_msg.index.ok_accept = Registervorschlag übernommen – die Marke steht in einer neuen Fassung.
reference_msg.index.ok_reject = Registervorschlag verworfen.
reference_msg.index.ok_register_add = Register angelegt.
reference_msg.index.ok_register_rename = Register umbenannt.
reference_msg.index.ok_register_delete = Register gelöscht.
reference_msg.index.section = Abschnitt { $key }
reference_msg.index.first = erste Fundstelle
reference_msg.index.all = alle Fundstellen
reference_msg.index.nth = { $n }. Fundstelle
reference_msg.index.field_key = Abschnitt
reference_msg.index.field_rev = Fassung
reference_msg.index.field_start = Anfang
reference_msg.index.field_end = Ende
reference_msg.index.not_a_number = { $label }: keine gültige Zahl (Feld { $name }).
reference_msg.index.word_missing = Wort: bitte das Wort angeben, hinter das die Marke kommt.
reference_msg.index.occurrence_form = Fundstelle: „{ $raw }“ ist weder „erste“, „alle“ noch eine Zahl ab 1.
reference_msg.index.proposal_missing = Registervorschlag fehlt (Feld id).
reference_msg.marks.whole_number = { $name }: ganze Zahl erwartet
reference_msg.marks.key_missing = key: Abschnitt fehlt
reference_msg.marks.rev_missing = rev: Fassung fehlt
reference_msg.marks.start_missing = start: Anfang der Marke fehlt
reference_msg.marks.end_missing = end: Ende der Marke fehlt
reference_msg.marks.rev_number = rev: Nummer einer Fassung erwartet
reference_msg.marks.occurrence = occurrence: erwartet „first“, „all“ oder eine Zahl ab 1
reference_msg.marks.word_missing = word: Wort fehlt
reference_msg.marks.start_byte = start: Byte-Stelle ab 0 erwartet
reference_msg.marks.end_byte = end: Byte-Stelle ab 0 erwartet
reference_msg.marks.empty_range = start/end: leerer Bereich
reference_msg.marks.no_section = Den Abschnitt { $key } gibt es in diesem Werk nicht.
reference_msg.marks.no_register = Register „{ $register }“ gibt es in diesem Werk nicht (vorhanden: { $present }).

# Cross references (xref.rs).
reference_msg.xref.picks = Bitte mindestens einen möglichen Verweis wählen (picks).
reference_msg.xref.no_candidate = Möglichen Verweis { $i } gibt es nicht (mehr).
reference_msg.xref.not_a_label = „{ $label }“ ist keine Sprungmarke (kN).
reference_msg.xref.label_unknown = Die Sprungmarke { $label } gehört zu keinem Gliederungspunkt.
reference_msg.xref.confirm = Umwandeln bitte bestätigen (confirm: true).
reference_msg.xref.rev_number = rev: Nummer erwartet
reference_msg.xref.title = Querverweise
reference_msg.xref.unknown_action = Unbekannte Aktion (method).
reference_msg.xref.tick = Bitte das Häkchen „Ja, umwandeln“ setzen.
reference_msg.xref.key_rev_missing = key und rev fehlen.

# Scripture check page (scripture_check.rs).
reference_msg.check.title = Zitate prüfen
reference_msg.check.key_missing = Bitte einen Abschnitt angeben (key = fester Schlüssel).
reference_msg.check.rev_number = rev: Nummer der Fassung erwartet.

# Versification mapping (scripture_map.rs).
reference_msg.map.name_text = { $field }: Name einer Verszählung als Text
reference_msg.map.unknown = { $field }: Verszählung „{ $name }“ unbekannt; bekannt sind { $known }
reference_msg.map.ref_missing = ref fehlt (Stellenangabe, z. B. „Joel 2,28–32“)
reference_msg.map.ref_too_long = ref: höchstens { $max } Zeichen
reference_msg.map.same = gleich in { $name }: { $target }
reference_msg.map.mapped = in { $name }: { $target }
reference_msg.map.rule = (Regel: { $rules })
reference_msg.map.truncated = – gekürzt auf { $max } Verse
reference_msg.map.no_book = das Buch { $abbr } gibt es in { $name } nicht
reference_msg.map.chapters = { $abbr } hat in { $name } nur { $count } Kapitel
reference_msg.map.verses =
    { $abbr } { $chapter } hat in { $name } nur { $count } { $count ->
        [one] Vers
       *[other] Verse
    }
reference_msg.map.end_before_start = { $abbr }: Ende vor dem Anfang

# Text search and replace (search/*).
reference_msg.search.kind_text = Text
reference_msg.search.kind_heading = Überschriften
reference_msg.search.kind_footnote = Fußnoten
reference_msg.search.kind_quote = Zitate (auch Bibelzitate)
reference_msg.search.kind_code = Typst-Code
reference_msg.search.query_missing = Bitte einen Suchtext eingeben.
reference_msg.search.query_too_long = Der Suchtext darf höchstens { $max } Zeichen haben (hier { $chars }).
reference_msg.search.shown_rev = Die gezeigte Fassung lässt sich nur im Abschnitt oder in der Auswahl durchsuchen.
reference_msg.search.key_missing = Für den Bereich „{ $range }“ fehlt der Abschnitt (key).
reference_msg.search.sel_missing = Bitte mindestens einen Gliederungspunkt auswählen (sel).
reference_msg.search.range_selection = Auswahl in { $name }
reference_msg.search.range_section = Abschnitt { $name }
reference_msg.search.range_chapter = Kapitel { $name }
reference_msg.search.range_part = Teil { $name }
reference_msg.search.range_work_no_parts = ganzes Werk (keine Teile)
reference_msg.search.range_from = ab { $name } bis zum Ende
reference_msg.search.range_picked_one = { $count } ausgewählter Abschnitt
reference_msg.search.range_picked = { $count } ausgewählte Abschnitte
reference_msg.search.range_work = ganzes Werk
reference_msg.search.range_all = alle Werke
reference_msg.search.params = Suche: { $error }
reference_msg.search.title = Suchen
reference_msg.search.foreign = Abgelehnt: Die Ersetzen-Vorschau startet nicht auf Anfrage einer fremden Seite.
reference_msg.search.selection = Markierung im Editor (Zeichen { $start }–{ $end })
reference_msg.search.choice_section = Abschnitt
reference_msg.search.choice_chapter = Kapitel
reference_msg.search.choice_part = Teil
reference_msg.search.choice_from = ab Abschnitt bis zum Ende
reference_msg.search.choice_picked = Auswahl aus der Gliederung
reference_msg.search.choice_work = ganzes Werk
reference_msg.search.choice_all = alle Werke (nur Suchen)
reference_msg.search.tick = Bitte das Häkchen „Ja, ersetzen“ setzen.
reference_msg.search.none_picked = Kein Abschnitt ausgewählt.
reference_msg.search.regex_too_big = Der reguläre Ausdruck ist zu groß.
reference_msg.search.regex_invalid = Ungültiger regulärer Ausdruck: { $error }
reference_msg.replace.too_long = Der Ersatz darf höchstens { $max } Zeichen haben (hier { $chars }).
reference_msg.replace.one_work = Ersetzen geht nur in einem Werk (nicht „alle Werke“).
reference_msg.replace.too_many = Zu viele Ersetzungen auf einmal (die eingesetzten Texte wären zusammen größer als { $mib } MiB); ersetzt wurde nichts. Bitte die Suche oder den Bereich eingrenzen.
reference_msg.replace.timeout = Die Suche brauchte länger als 2 s; ersetzt wurde nichts. Bitte den Bereich verkleinern.
reference_msg.replace.max_sections = Höchstens { $max } Abschnitte je Ersetzen (hier { $count }).
reference_msg.replace.no_more_hits = Keine Treffer mehr in diesem Abschnitt.
reference_msg.replace.confirm = Ersetzen bitte bestätigen (confirm: true).
reference_msg.replace.replacement_missing = replacement: Ersatztext fehlt (leer ist erlaubt).
reference_msg.replace.undo_confirm = Rückgängig bitte bestätigen (confirm: true).
reference_msg.replace.batch_missing = batch: Nummer des Ersetzens fehlt.

# Round 39c: notes and refusals of the Bible checks (src/bibles.rs, src/scripture.rs)
reference_msg.bibles.fragment_example = „{ $example }“
reference_msg.bibles.fragment_note = Modul { $module } hat auffällig viele Wortfragmente ({ $suspects } von { $pairs } Wortpaaren, z. B. { $examples }) – vermutlich Silbentrennreste; Baruch vergleicht den Wortlaut darum auch ohne Leerraum.
reference_msg.scripture.text_missing = text fehlt (Stellenangabe oder Fließtext)
reference_msg.scripture.text_too_long = text: höchstens { $max } Zeichen
reference_msg.xref.tooltip = Querverweis auf { $target } (Strg+Klick: springen)
