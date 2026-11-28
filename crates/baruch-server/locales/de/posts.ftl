# SPDX-License-Identifier: AGPL-3.0-or-later
# Area "posts" (Round 39a, templates_1), German translation of locales/en/posts.ftl (same keys in the same
# order; the wording of the templates before Round 39a).

# templates/fragments/post_item.html
posts.item.open_own_title = Beitrag zum Markieren und Kopieren, Zitieren, Bearbeiten und Verlauf (auch ohne JavaScript)
posts.item.open_title = Beitrag zum Markieren und Kopieren, Zitieren und Verlauf (auch ohne JavaScript)
posts.item.manage_own_title = Beitrag bearbeiten, löschen, verschieben oder umwandeln
posts.item.manage_title = Beitrag löschen, verschieben oder umwandeln
posts.item.edited_title = zuletzt bearbeitet { $edited }; die alte Fassung steht im Verlauf
posts.item.edited = bearbeitet
posts.item.basis_title = Grundlage dieser Antwort (von Baruch, nicht vom Modell)
posts.item.replies_to = antwortet auf
posts.item.post_ref = Beitrag { $reply_to }
posts.item.relay_from = von { $relay_from }
posts.item.repeat_of = Wiederholung von
posts.item.post_repeat = Beitrag { $repeat_of }
posts.item.after_rev = nach Fassung { $basis_rev }
posts.item.ref = ↪ { $number } { $title } · Revision { $rev }
posts.item.source = Quelle: { $source }
posts.item.web_title = abgerufene Seiten, Zitate und ihre Prüfung; belegte Zitate in die Bibliothek
posts.item.web = ↪ Belege der Websuche
posts.item.repeat_label = Antwort von { $author } wiederholen
posts.item.repeat_title = Dieselbe Anfrage an { $author } gegen die gezeigte Fassung { $rev }; die alte Antwort bleibt
posts.item.delete_label = Beitrag löschen
posts.item.delete_title = Beitrag löschen (mit Rückfrage)
posts.item.copy_title = Text des Beitrags in die Zwischenablage
posts.item.copy = Kopieren
posts.item.quote_title = Beitrag als „> …“ ins Eingabefeld
posts.item.quote = Zitieren
posts.item.open = Beitrag öffnen
posts.item.manage = Verwalten
posts.item.edit = Bearbeiten
posts.item.new_text_label = Neuer Text des Beitrags
posts.item.old_stays = die alte Fassung bleibt im Verlauf
posts.item.delete_confirm = wirklich löschen
posts.item.move_menu = Verschieben…
posts.item.move_to = nach
posts.item.move = Verschieben
posts.item.tags = Schlagwörter
posts.item.tags_placeholder = z. B. Rose, Dorn
posts.item.tags_label = Schlagwörter des Einfalls
posts.item.to_idea = In Einfall umwandeln
posts.item.to_findings_title = Befunde aus diesem Beitrag anlegen; der Beitrag bleibt unverändert
posts.item.to_findings = In Befunde umwandeln
posts.item.repeat_again_title = Dieselbe Anfrage noch einmal an { $author }; die alte Antwort bleibt
posts.item.quote_reply_title = Beitrag als „> …“ in die Antwort
posts.item.new_text = Neuer Text
posts.item.old_stays_thread = Die alte Fassung bleibt im Verlauf (Fäden stehen nicht im Spiegel)
posts.item.history = Verlauf ({ $count })
posts.item.version = gültig ab { $valid_from }, ersetzt { $replaced } von { $edited_by }

# templates/fragments/post_solutions.html
posts.solutions.label = Lösungen aus Beitrag { $id }
posts.solutions.heading = Lösungen
posts.solutions.by = von { $author }
posts.solutions.instead = statt „{ $quote }“
posts.solutions.better = besser „{ $replacement }“
posts.solutions.step = Schritt: { $step }
posts.solutions.at = (bei „{ $quote }“)
posts.solutions.missing = (Zitat steht nicht in der gezeigten Fassung)
posts.solutions.accept = Als Vorschlag übernehmen
posts.solutions.variants_label = Lösungen aus Beitrag { $id } in Varianten umsetzen
posts.solutions.lektor_writes = Lektor schreibt
posts.solutions.variants = Varianten
posts.solutions.apply = Lösungen umsetzen

# templates/posts.html
posts.chat.count_one = { $count } Beitrag
posts.chat.count_other = { $count } Beiträge
posts.chat.open_own_title = Kopieren, Zitieren, Bearbeiten und Verlauf
posts.chat.open_title = Kopieren, Zitieren und Verlauf
posts.chat.head_title = Chat-Historie – { $title } – Baruch
posts.chat.page_title = Chat-Historie · { $title }
posts.chat.back = Zurück
posts.chat.back_to_section = Zurück zum Abschnitt
posts.chat.heading = Abschnitts-Chat: { $title }
posts.chat.oldest_first = , älteste zuerst.
posts.chat.to_newest = Zum neuesten Beitrag ↓
posts.chat.source = Quelle: { $source }
posts.chat.repeat_label = Antwort von { $author } wiederholen
posts.chat.repeat_title = Dieselbe Anfrage gegen die aktuelle Fassung; die alte Antwort bleibt
posts.chat.delete_label = Beitrag löschen
posts.chat.delete_title = Beitrag löschen (mit Rückfrage)
posts.chat.copy_title = Text des Beitrags in die Zwischenablage
posts.chat.copy = Kopieren
posts.chat.open = Beitrag öffnen
posts.chat.empty = Noch keine Beiträge zu diesem Abschnitt.
posts.chat.post = Beitrag
posts.chat.placeholder = @persona fragen … · RFC … · @alle
posts.chat.as_task = als Auftrag (Schreibauftrag wird ein Vorschlag)
posts.chat.send = Senden

# templates/post.html
posts.post.head_title = Beitrag { $id } – Baruch
posts.post.page_title = Beitrag · { $title }
posts.post.back = Zurück
posts.post.history = Chat-Historie
posts.post.back_to_section = Zurück zum Abschnitt
posts.post.heading = Beitrag { $id } von { $author }
posts.post.edited = · bearbeitet { $edited }
posts.post.post_label = Beitrag
posts.post.copy_label = Text zum Kopieren
posts.post.copy_text = Text (markieren und kopieren)
posts.post.source = Quelle: { $source }
posts.post.copy_title = Text in die Zwischenablage
posts.post.copy = Kopieren
posts.post.quote = Zitieren
posts.post.reply = Antwort im Abschnitts-Chat
posts.post.as_task = als Auftrag (Schreibauftrag wird ein Vorschlag)
posts.post.send = Senden
posts.post.edit = Bearbeiten
posts.post.new_text = Neuer Text
posts.post.old_stays = Die alte Fassung bleibt im Verlauf; die Änderung steht im Spiegel des Werks
posts.post.to_findings = In Befunde umwandeln
posts.post.persona_posts = Beiträge von Personas bleiben unverändert: kopieren, zitieren oder umwandeln.
posts.post.own_only = Nur eigene Beiträge lassen sich bearbeiten.
posts.post.to_findings_title = Befunde aus diesem Beitrag anlegen (ohne Marken ein Hinweis mit dem ganzen Text); der Beitrag bleibt
posts.post.delete_confirm = Beitrag { $id } wirklich löschen
posts.post.delete_title = Der Beitrag verschwindet aus dem Chat; der Spiegel hält ihn im Lösch-Commit fest
posts.post.versions = Verlauf
posts.post.version = gültig ab { $valid_from }, ersetzt { $replaced } von { $edited_by }
posts.post.never_edited = Nie bearbeitet.

# templates/thread.html
posts.thread.head_title = { $title } – Faden – Baruch
posts.thread.page_title = Faden · { $title }
posts.thread.back = Zurück
posts.thread.all_threads = Alle Fäden
posts.thread.back_to_work = Zurück zum Werk
posts.thread.started_by = begonnen von { $created_by } · ohne Abschnitt ·
posts.thread.to_newest = zum neuesten Beitrag ↓
posts.thread.posts_label = Beiträge des Fadens
posts.thread.reply = Antwort
posts.thread.placeholder = @lektor … bzw. „Lektor, …“ ruft eine Persona · RFC … bzw. @alle ruft alle
posts.thread.persona = Persona antworten lassen
posts.thread.mentioned = – nur Erwähnte (@…) –
posts.thread.send = Antworten

# templates/threads.html
posts.threads.count_one = { $count } Beitrag
posts.threads.count_other = { $count } Beiträge
posts.threads.head_title = Fäden – Baruch
posts.threads.page_title = Fäden · { $title }
posts.threads.back = Zurück
posts.threads.back_to_work = Zurück zum Werk
posts.threads.heading = Fäden ohne Abschnitt
posts.threads.intro = Gespräche mit Personas und MCP-Clients, die an keinem Abschnitt hängen. Sie ändern keinen Buchtext.
posts.threads.started_by = begonnen von { $created_by } ·
posts.threads.last = · zuletzt { $last_author }, { $updated }
posts.threads.empty = Noch keine Fäden.
posts.threads.new = Neuer Faden
posts.threads.message = Nachricht
posts.threads.placeholder = @lektor … ruft eine Persona
posts.threads.persona = Persona antworten lassen
posts.threads.mentioned = – nur Erwähnte (@…) –
posts.threads.start = Faden beginnen
