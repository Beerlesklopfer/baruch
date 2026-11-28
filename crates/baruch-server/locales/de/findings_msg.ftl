# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of findings (Round 39a, task messages_1). Base catalog in English; locales/de/findings_msg.ftl has the same keys in
# the same order with the German wording.

# Findings page and matrix (findings/page.rs).
findings_msg.filter.status = status: „open“, „adopted“, „rejected“, „deferred“ oder „done“ erwartet, nicht „{ $word }“
findings_msg.filter.kind = kind: „praise“, „hint“ oder „critical“ erwartet, nicht „{ $word }“
findings_msg.param.texts = { $name }: Texte erwartet
findings_msg.param.text_or_list = { $name }: Text oder Liste erwartet
findings_msg.param.text = { $name }: Text erwartet
findings_msg.param.time = { $name }: Zeit wie 2026-11-01 erwartet
findings_msg.param.include_praise = include_praise: true oder false erwartet
findings_msg.param.older_range = older_than_days: 0 bis { $max } erwartet
findings_msg.param.older_number = older_than_days: Zahl erwartet
findings_msg.param.persona = persona: ungültig
findings_msg.param.object = Parameter als Objekt erwartet
findings_msg.param.id_missing = id fehlt (Befund)
findings_msg.param.entry_missing = entry fehlt (Vorschlag)
findings_msg.matrix.solutions = { $solved } mit Lösung, { $unsolved } ohne Lösung
findings_msg.matrix.praise = { $count } Lob
findings_msg.matrix.none = keine Befunde
findings_msg.matrix.no_part = Ohne Teil
findings_msg.matrix.part_itself = { $title } (Teil selbst)
findings_msg.matrix.no_section_long = Ohne Abschnitt (Fäden, gelöschte Abschnitte)
findings_msg.matrix.no_section = Ohne Abschnitt
findings_msg.notice.status = Status gespeichert (ein Commit im Spiegel).
findings_msg.notice.decision = Entscheidung gespeichert (ein Commit im Spiegel).
findings_msg.notice.saved = Gespeichert.
findings_msg.page.unknown_work = unbekanntes Werk
findings_msg.page.error = Fehler: { $error }
findings_msg.page.template_error = Fehler: Vorlage { $name }: { $error }
findings_msg.page.unknown_action = unbekannte Aktion
findings_msg.page.work_missing = work fehlt (Werk-id)

# RPC methods finding.* (findings/rpc.rs).
findings_msg.param.number = { $name }: Zahl erwartet
findings_msg.param.entry_history_missing = entry fehlt (Vorschlag im Verlauf)
findings_msg.param.flag = { $name }: true oder false erwartet
findings_msg.param.text_list = { $name }: Liste von Texten erwartet
findings_msg.param.list_or_text = { $name }: Liste oder Text „a,b“ erwartet
findings_msg.param.confirm_missing = confirm fehlt (true = bestätigen, false = ablehnen)
findings_msg.param.keys = keys: fester Schlüssel (Zahl) erwartet, nicht „{ $key }“
findings_msg.param.status_unknown = status: unbekannter Status „{ $status }“
findings_msg.param.kind_unknown = kind: unbekannte Art „{ $kind }“
findings_msg.param.limit = limit: 1 bis 2000
findings_msg.status.adopted_rev = „übernommen“ braucht die Fassung (rev).
findings_msg.status.rev_number = rev: Fassungsnummer ab 1
findings_msg.status.reason_missing = Verwerfen braucht eine Begründung (reason).
findings_msg.status.target_missing = „verschoben“ braucht ein Ziel: target_key (Abschnitt) oder target_idea (Einfall).
findings_msg.status.target_both = „verschoben“ braucht genau ein Ziel: target_key oder target_idea, nicht beide.
findings_msg.status.unknown = status: unbekannter Status „{ $status }“ (open, adopted, rejected, deferred, done)
findings_msg.status.history_missing = Verlauf des Befunds fehlt nach dem Setzen

# Findings in words (findings/view.rs).
findings_msg.view.untitled = (ohne Titel)
findings_msg.view.adopted_in = { $label } in { $revision }
findings_msg.view.revision_deleted = { $label } (Fassung gelöscht)
findings_msg.view.section = Abschnitt { $key }
findings_msg.view.deferred_to = { $label } nach { $target }
findings_msg.view.deferred_idea = { $label } in Einfall { $id }
findings_msg.view.target_deleted = { $label } (Ziel gelöscht)
findings_msg.view.confirmed = bestätigt
findings_msg.view.rejected = verworfen
findings_msg.view.stale = veraltet
findings_msg.view.rev_current = aktuell
findings_msg.view.rev_proposed = Vorschlag
findings_msg.view.rev_rejected = verworfen
findings_msg.view.rev_superseded = ersetzt
findings_msg.view.revision = Fassung { $number } ({ $status })
findings_msg.view.stale_summary = veraltet (alle Ursprünge entschieden)
