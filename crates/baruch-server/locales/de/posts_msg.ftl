# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of posts, threads and chats (Round 39a, task messages_1). Base catalog in English; locales/de/posts_msg.ftl has the same keys in
# the same order with the German wording.

# Posts of the section chat (posts.rs).
posts_msg.param.number = { $name }: Zahl erwartet
posts_msg.param.id_missing = id fehlt (Beitrag)
posts_msg.param.rev = rev: Fassung ab 1
posts_msg.param.confirm_delete = Löschen bitte bestätigen (confirm).
posts_msg.param.target_seq = target: Stelle ab 0 erwartet
posts_msg.param.target_missing = Ziel fehlt: target_key (fester Schlüssel des Abschnitts) angeben.
posts_msg.param.words_missing = words fehlt (mindestens ein Schlagwort)
posts_msg.param.body_missing = body fehlt (neuer Text)
posts_msg.param.key_missing = key fehlt (fester Schlüssel des Abschnitts)
posts_msg.param.limit = limit: 1 bis 200 erwartet
posts_msg.param.work_number = work: Werk-id (Zahl) erwartet
posts_msg.param.no_section = Abschnitt { $key } gibt es nicht
posts_msg.param.before_elsewhere = before: Beitrag { $id } steht nicht in diesem Abschnitt
posts_msg.refused.not_own = Nur eigene Beiträge lassen sich über MCP verschieben, löschen oder umwandeln: Beitrag { $id } stammt von { $author }, nicht von { $actor }. Fremde Beiträge verwaltet der Mensch.
posts_msg.refused.not_editable = Nur eigene Beiträge lassen sich bearbeiten: Beitrag { $id } stammt von { $author }, nicht von { $actor }. Beiträge von Personas bleiben unverändert (kopieren, zitieren, in Einfall oder Befund umwandeln geht).
posts_msg.repeat.no_order = Beitrag { $id } hat keinen gespeicherten Auftrag und keine vorangehende Nachricht des Menschen; wiederholen lässt er sich nicht. Bitte die Frage neu stellen.
posts_msg.repeat.only_personas = Nur Antworten von Personas lassen sich wiederholen; Beitrag { $id } stammt von { $author }.
posts_msg.repeat.mode = Der Auftrag „{ $mode }“ lässt sich nicht wiederholen.
posts_msg.repeat.no_post = Die Wiederholung hat keinen Beitrag ergeben.
posts_msg.page.error = Fehler: { $error }
posts_msg.page.template_error = Fehler: Vorlage { $name }: { $error }
posts_msg.page.unknown_action = unbekannte Aktion
posts_msg.form.confirm_delete = Löschen bitte bestätigen (Kästchen ankreuzen).
posts_msg.page.key_missing = Fehler: key fehlt (fester Schlüssel des Abschnitts)
posts_msg.page.no_section = Abschnitt { $key } gibt es in diesem Werk nicht.
posts_msg.page.id_missing = Fehler: id fehlt (Beitrag)

# Threads without a section (threads.rs).
posts_msg.thread.empty_answer = Das Modell hat keine verwertbare Antwort geliefert; nichts gespeichert.
posts_msg.thread.message_empty = Die Nachricht ist leer.
posts_msg.thread.text = { $name }: Text erwartet
posts_msg.thread.thread_missing = thread fehlt (id des Fadens)
posts_msg.thread.id_missing = id fehlt (Fadenbeitrag)
posts_msg.thread.not_own = Nur eigene Beiträge lassen sich über MCP löschen: Fadenbeitrag { $id } stammt von { $author }, nicht von { $actor }. Fremde Beiträge verwaltet der Mensch.
posts_msg.thread.no_order = Beitrag { $id } hat keinen gespeicherten Auftrag und keine vorangehende Nachricht des Menschen; wiederholen lässt er sich nicht.
posts_msg.thread.choose = Faden fehlt: aus der Liste wählen.
posts_msg.thread.form_ids = Fehler: thread und id (Zahlen) fehlen
posts_msg.thread.confirm_delete = Löschen bitte bestätigen (Kästchen „wirklich löschen“ ankreuzen).
posts_msg.thread.partly_answered = Dein Beitrag ist gespeichert, aber nicht jede Persona hat geantwortet: { $errors }

# Links in chat posts (chat_render.rs).
posts_msg.chat.jump_no_section = Sprungmarke ungeprüft: Abschnitt { $key } gibt es in diesem Werk nicht
posts_msg.chat.jump_no_quote = Sprungmarke ungeprüft: das Zitat steht nicht in Abschnitt { $key }
posts_msg.chat.open_idea = Einfall { $id } öffnen
posts_msg.chat.open_finding = Befund { $id } öffnen
posts_msg.chat.open_bible = { $passage } in der Bibel des Werks aufschlagen
posts_msg.chat.quote = Zitat
posts_msg.chat.quote_in = Zitat in { $number } { $title }
