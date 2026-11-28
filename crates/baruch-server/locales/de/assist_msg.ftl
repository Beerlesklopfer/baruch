# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of the assistants (personas, chat, solutions, variants) (Round 39a, task messages_1). Base catalog in English; locales/de/assist_msg.ftl has the same keys in
# the same order with the German wording.

# Checks and refusals of model orders (assist/mod.rs).
assist_msg.check.question_too_long = Die Frage darf höchstens { $max } Zeichen haben.
assist_msg.check.message_too_long = Die Nachricht darf höchstens { $max } Zeichen haben.
assist_msg.check.post_too_long = Der Beitrag darf höchstens { $max } Zeichen haben.
assist_msg.check.question_missing = Die Frage fehlt.
assist_msg.retry.forbidden_opening = verbotene Einleitung
assist_msg.answer.empty = Das Modell hat keine verwertbare Antwort geliefert; nichts gespeichert.
assist_msg.answer.cut_off = Die Antwort des Modells brach an der Längengrenze (max_tokens) ab; ein unvollständiger Vorschlag wird nicht angelegt. Nichts gespeichert.
assist_msg.answer.equals_current = Der Vorschlag gleicht der aktuellen Fassung; nichts angelegt.
assist_msg.answer.equals_basis = Der Vorschlag gleicht Fassung { $rev }, auf der er beruht; nichts angelegt.
assist_msg.answer.nothing_to_check = Nichts zu prüfen: Der Text ist leer; nichts gespeichert.
assist_msg.answer.chapter_empty = Das Kapitel hat keinen Text; nichts zu überarbeiten, nichts angelegt.
assist_msg.answer.no_proposal = Kein Vorschlag angelegt: { $skipped }. Nichts gespeichert.
assist_msg.compress.summary_failed = Der Kontext war zu lang und musste komprimiert werden, aber das Zusammenfassen von „{ $label }“ scheiterte: { $error } Nichts gespeichert.
assist_msg.compress.too_many_parts = Die Kontextlänge von { $handle } ({ $tokens } Tokens) reicht auch komprimiert nicht: { $piece } bräuchte mit Umfeld mindestens { $size } Zeichen, Platz ist für { $budget } – das Kapitel hat zu viele Teile für diese Kontextlänge (auch ausgelassene Teile stehen mit ihrem Titel im Aufruf). Nichts gespeichert. Kontextlänge unter Einstellungen → Assistenten erhöhen oder einen einzelnen Abschnitt prüfen lassen.
assist_msg.compress.piece = Stück { $number } von { $count } ({ $what })

# Solutions and variants (assist/solutions.rs).
assist_msg.solution.no_wording = { $label } hat keine neue Formulierung (nur einen Schritt); sie lässt sich nicht einsetzen.
assist_msg.solution.quote_gone = { $label }: Das Zitat „{ $quote }“ steht nicht (mehr) im Text.
assist_msg.solution.no_revision_at_all = Der Abschnitt hat keine Fassung.
assist_msg.solution.no_such_revision = Fassung { $number } gibt es in diesem Abschnitt nicht.
assist_msg.solution.section_gone = Der Abschnitt ist nicht mehr da.
assist_msg.solution.label = Lösung
assist_msg.solution.numbered = Lösung { $pick }
assist_msg.solution.key_missing = key fehlt (fester Schlüssel des Abschnitts)
assist_msg.solution.rev_number = { $name }: Fassungsnummer ab 1 erwartet
assist_msg.solution.number_list = { $name }: Liste von Zahlen erwartet
assist_msg.solution.post_elsewhere = Beitrag { $id } gehört nicht zu diesem Abschnitt.
assist_msg.solution.no_section = Abschnitt { $key } gibt es in diesem Werk nicht.
assist_msg.solution.picks_missing = picks fehlt: mindestens eine Lösung wählen
assist_msg.solution.finding_elsewhere = Befund { $id } gehört nicht zu diesem Abschnitt.
assist_msg.solution.finding_without_post = Der Befund stammt aus keinem Beitrag dieses Abschnitts.
assist_msg.solution.post_missing = post fehlt (Beitrag mit den Lösungen) – oder finding (Befund)
assist_msg.solution.no_such_solution =
    Lösung { $pick } gibt es nicht ({ $count } { $count ->
        [one] Lösung
       *[other] Lösungen
    }).
assist_msg.solution.only_step = Sie nennt nur einen Schritt, keine neue Formulierung.
assist_msg.solution.quote_not_in = Das Zitat „{ $quote }“ steht nicht in Fassung { $rev }.
assist_msg.solution.not_adoptable = Lösung { $pick } ist nicht übernehmbar: { $why }
assist_msg.solution.no_change = Die Lösungen ändern den Text nicht; kein Vorschlag angelegt.
assist_msg.solution.count = count: 2 oder 3 Varianten (höchstens { $max }), nicht { $n }
assist_msg.solution.rev_missing = rev fehlt (die Variante, die bleibt)
assist_msg.solution.confirm_missing = confirm: true fehlt – die übrigen Varianten werden verworfen
assist_msg.solution.no_rev = Fassung { $rev } gibt es nicht.
assist_msg.solution.not_a_variant = Fassung { $rev } ist keine Variante.
assist_msg.solution.no_group_in_section = Variantengruppe { $group } gibt es in diesem Abschnitt nicht.
assist_msg.solution.no_group_in_work = Variantengruppe { $group } gibt es in diesem Werk nicht.
assist_msg.solution.group_missing = group fehlt (Variantengruppe)
assist_msg.page.template_error = Fehler: Vorlage { $name }: { $error }
assist_msg.page.error = Fehler: { $error }
assist_msg.page.unknown_action = unbekannte Aktion
assist_msg.page.work_missing = work fehlt (Werk-id)

# Variants of a section (assist/variants.rs).
assist_msg.variant.title = Variante { $label }: { $summary }
assist_msg.variant.finding_elsewhere = [B{ $id }] gehört nicht zu diesem Abschnitt und bleibt außen vor.
assist_msg.variant.count = 2 oder 3 Varianten (höchstens { $max }), nicht { $count }
assist_msg.variant.no_lector = Kein Lektor aktiv (Einstellungen → Assistenten).
assist_msg.variant.no_findings = Keine Befunde zum Umsetzen: weder genannt noch offen in diesem Abschnitt. Nichts angelegt.
assist_msg.variant.too_long = „{ $title }“ ist zu lang für die Kontextlänge von { $handle } ({ $tokens } Tokens), und kein Zitat der Befunde steht im Text – ein Teilvorschlag lässt sich nicht abgrenzen. Nichts angelegt.
assist_msg.variant.paragraphs_too_long = Auch die betroffenen Absätze von „{ $title }“ passen nicht in die Kontextlänge von { $handle } ({ $tokens } Tokens). Nichts angelegt.
assist_msg.variant.partial = Langer Abschnitt: Teilvorschlag für die Absätze { $paragraphs } (der Rest bleibt wörtlich).
assist_msg.variant.cut_off = Variante { $label }: an der Längengrenze abgebrochen, nicht angelegt.
assist_msg.variant.unchanged = Variante { $label }: unverändert, nicht angelegt.
assist_msg.variant.none = Keine Variante angelegt: { $notes }

# Personas and their settings (personas.rs).
assist_msg.persona.bible_refused = Bibel-Belege aus SWORD gibt es nur für @belege und den Lektor.
assist_msg.persona.rag_refused = Bedeutungs-Treffer gibt es nur für @belege und den Lektor.
assist_msg.persona.scope = scope: „chapter“ (ganzes Kapitel) oder „section“ (nur Abschnitt)
assist_msg.persona.flag = { $name }: true oder false erwartet
assist_msg.persona.persona_missing = persona fehlt (z. B. „@belege“)
assist_msg.persona.fixed_handle = handle fehlt: welche Persona ist fest Lektor?
assist_msg.persona.lector_mode = mode: „fixed“ oder „auto“ erwartet
assist_msg.persona.handle_missing = handle fehlt (z. B. „@lektor“)
assist_msg.persona.text = { $name }: Text erwartet
assist_msg.persona.context_range = Die Kontextlänge muss zwischen 1 und 10000000 Tokens liegen.
assist_msg.persona.active_flag = active: true oder false erwartet
assist_msg.persona.color_param = color: "#rrggbb" oder null erwartet
assist_msg.persona.prompt_param = prompt: Text oder null (Vorgabe) erwartet
assist_msg.persona.on_flag = on: true oder false erwartet
assist_msg.persona.connection_param = connection: id der Modell-Verbindung erwartet
assist_msg.persona.color_form = Farbe als #rrggbb angeben.
assist_msg.persona.choose_connection = Bitte eine Modell-Verbindung wählen.

# Persona orders over RPC (assist/rpc.rs, assist/chat.rs).
assist_msg.rpc.number = { $name }: Zahl erwartet
assist_msg.rpc.scope = scope: „section“ oder „chapter“ erwartet, nicht „{ $other }“
assist_msg.rpc.node_refused = node (Stelle) wird nicht angenommen; key mitgeben (fester Schlüssel des Abschnitts).
assist_msg.rpc.rev_number = rev: Fassungsnummer ab 1 erwartet
assist_msg.rpc.no_lector = Keine Persona ist aktiv (kein Lektor).
assist_msg.rpc.persona_missing = persona fehlt (z. B. „@lektor“)
assist_msg.chat.no_rev = Der Vorschlag hat keine Fassungsnummer.
assist_msg.chat.finding_elsewhere = [B{ $id }]: Der Befund gehört nicht zu diesem Abschnitt; kein Statusvorschlag zu Fassung { $rev }.
assist_msg.chat.basis = basis: shown, current oder eine Fassungsnummer

# Notice page after a persona order without JavaScript (assist/routes.rs).
assist_msg.notice.title_saved = Antwort gespeichert
assist_msg.notice.title = Hinweis
assist_msg.notice.onward = Weiter zum Abschnitt
assist_msg.notice.back = Zurück zum Abschnitt
assist_msg.notice.partly_answered = Dein Beitrag und die fertigen Antworten sind gespeichert, aber nicht jede Persona hat geantwortet:
assist_msg.notice.proposal_created = Der Vorschlag ist angelegt.
assist_msg.notice.answer_saved = Die Antwort ist gespeichert.
assist_msg.stream.json_object = JSON-Objekt {"{"}method, …{"}"} erwartet

# Model calls (assist/port.rs, assist/eliezer.rs).
assist_msg.model.not_configured = Kein Modell-Backend nutzbar: { $why }
assist_msg.model.unreachable = Das Modell-Backend ist nicht erreichbar: { $why }
assist_msg.model.denied = Das Modell-Backend verweigert den Zugang: { $why }
assist_msg.model.timeout = Zeitüberschreitung beim Modell: { $why }
assist_msg.model.unknown = Unbekanntes Modell: { $why }
assist_msg.model.busy = Das Modell ist ausgelastet: { $why }
assist_msg.model.cancelled = Abgebrochen; es wurde nichts gespeichert.
assist_msg.model.failed = Der Modellaufruf ist gescheitert: { $why }
assist_msg.model.cached_list = Liste aus dem Zwischenspeicher (Stand { $fetched }).

# Review basis and glossary proposals (assist/basis.rs, assist/glossary.rs).
assist_msg.basis.invalid = basis: „shown“, „current“ oder eine Fassungsnummer erwartet, nicht „{ $shown }“
assist_msg.basis.no_revision = Der Abschnitt hat noch keine Fassung, die sich prüfen ließe.
assist_msg.glossary.no_text = Kein Text, aus dem sich Begriffe vorschlagen ließen; nichts angelegt.
assist_msg.glossary.no_terms = { $handle } hat keine Begriffe in der Form „Begriff | Erklärung“ genannt; nichts angelegt.
