# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of the view, ribbon, menu, consent bar and laptop mode (Round 39a, task messages_1). Base catalog in English; locales/de/view_msg.ftl has the same keys in
# the same order with the German wording.

# Format buttons of the ribbon (ribbon.rs).
view_msg.ribbon.scene = Szene
view_msg.ribbon.scene_title = Szene nach der Werkvorlage (#szene)
view_msg.ribbon.work = Arbeit
view_msg.ribbon.work_title = Arbeitsauftrag nach der Werkvorlage (#arbeit)
view_msg.ribbon.scripture = Bibelstelle
view_msg.ribbon.scripture_title = Bibelstelle nach der Werkvorlage (#{ $name }[…], z. B. Joh 3,16)
view_msg.ribbon.cite = Literaturverweis
view_msg.ribbon.cite_title = Verweis auf einen Eintrag des Literaturverzeichnisses der Werkvorlage (@schlüssel)
view_msg.ribbon.footnote = Fußnote
view_msg.ribbon.footnote_title = Fußnote an der Cursorstelle (#footnote)
view_msg.ribbon.quote = Zitat
view_msg.ribbon.quote_title = Zitat als eigener Block (#quote); die Stellenangabe kommt als eigene Zeile direkt darunter (– Stelle)
view_msg.ribbon.list = Liste
view_msg.ribbon.list_title = Liste mit Spiegelstrichen (- …)
view_msg.ribbon.enum = Aufzählung
view_msg.ribbon.enum_title = Nummerierte Aufzählung (1. …)
view_msg.ribbon.terms = Begriff
view_msg.ribbon.terms_title = Begriffsliste (/ Begriff: Erklärung)
view_msg.ribbon.break = Umbruch
view_msg.ribbon.break_title = Zeilenumbruch im Absatz (\)
view_msg.ribbon.link = Link
view_msg.ribbon.link_title = Verweis auf eine Adresse (#link)
view_msg.ribbon.xref = Verweis
view_msg.ribbon.xref_title = Querverweis auf einen Gliederungspunkt (#link({ $lt }kN{ $gt })[…], wahlweise mit Seitenzahl)

# Levels, revision states and proposal heads of the workspace (view.rs).
view_msg.level.part = Teil
view_msg.level.sermon_part = Predigtteil
view_msg.level.chapter = Kapitel
view_msg.level.section = Abschnitt
view_msg.level.subsection = Unterabschnitt
view_msg.status.current = aktuell
view_msg.status.proposed = Vorschlag
view_msg.status.rejected = verworfen
view_msg.status.superseded = ersetzt
view_msg.proposal.diff_current = Unterschied zu Fassung { $rev } (aktuell) · gezeigt
view_msg.proposal.diff = Unterschied zu Fassung { $rev } · gezeigt
view_msg.proposal.shown = gezeigt
view_msg.proposal.head_lektor = { $handle } · Fassung { $rev }
view_msg.proposal.head = { $name } ({ $handle }) · Fassung { $rev }
view_msg.proposal.based_on_amend = beruht auf Fassung { $rev }, Stand { $amend }
view_msg.proposal.on_rev = auf Fassung { $rev }
view_msg.proposal.based_on_state = beruht auf Stand { $amend }
view_msg.proposal.stale = veraltet: Fassung { $rev } ist inzwischen bei Stand { $now }
view_msg.proposal.part = Teil { $k }/{ $parts }
view_msg.proposal.equals_shown = gleicht der gezeigten Fassung
view_msg.saved.amended = Gespeichert: fortgeschrieben (Stand { $amend }).
view_msg.saved.new = Gespeichert: neue Fassung { $rev }.
view_msg.saved.new_reason = Gespeichert: neue Fassung { $rev } ({ $reason }).
view_msg.xref.untitled = (ohne Titel)
view_msg.basis.proposal_by = Vorschlag { $rev } von { $persona }
view_msg.basis.proposal = Vorschlag { $rev }

# Menu (menu.rs).
view_msg.menu.badges_params = menu.badges: Parameter als Objekt erwartet
view_msg.menu.one_section = { $count } Abschnitt
view_msg.menu.sections = { $count } Abschnitte

# Consent bar (consent.rs).
view_msg.consent.template_error = Vorlage consent.html: { $error }
view_msg.consent.page_too_large = Fehler: Seite zu groß

# Editor (wysiwyg.rs).
view_msg.wysiwyg.chapter_unknown = unbekannt (Nummerierung der Vorlage nicht lesbar)
view_msg.wysiwyg.image_list = Liste mit { $count } Bildern, Index Kapitel { $offset }; Kapitelzähler { $at } liegt außerhalb

# Speaking time (timing.rs).
view_msg.timing.whole_number = { $name }: ganze Zahl erwartet
view_msg.timing.wpm_param = words_per_minute: ganze Zahl von 60 bis 250 erwartet
view_msg.timing.wpm_range = Wörter je Minute: erlaubt sind 60 bis 250.
view_msg.timing.target_range = Zielzeit: erlaubt sind 1 bis 240 Minuten (leer bzw. null = keine Zielzeit).
view_msg.timing.work_missing = work fehlt: Zeiten setzt man je Werk (work: Werk-id).
view_msg.timing.text = text: Text erwartet
view_msg.timing.text_missing = text fehlt
view_msg.timing.text_too_long = text: höchstens 1 000 000 Zeichen
