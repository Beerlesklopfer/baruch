# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of literature, bibles and sources (Round 39a, task messages_1). Base catalog in English; locales/de/sources_msg.ftl has the same keys in
# the same order with the German wording.

# Literature, sources and highlights (literature.rs).
sources_msg.literature.pick_section = Abschnitt wählen: Die Persona antwortet im Abschnitts-Chat. Die Literatur aus einem Abschnitt heraus öffnen (Arbeitsseite → Literatur) und den Text dann an die Persona geben.
sources_msg.literature.not_linked = Diese Quelle ist nicht mit MultiGPT verknüpft; einen Volltext gibt es nur für Dokumente aus MultiGPT (Bedeutungssuche → „Volltext“ → „In die Bibliothek übernehmen“).
sources_msg.literature.foreign_fulltext = Aufruf von einer fremden Seite; Baruch lädt den Volltext über MultiGPT nur, wenn die Seite hier in Baruch geöffnet wird.
sources_msg.images.title = Bilder des Werks für „{ $query }“
sources_msg.images.alt = Alternativtext: { $alt }
sources_msg.images.only_first = Nur die ersten { $count } Bilder; mehr auf der Seite
sources_msg.images.page_link = Bilder
sources_msg.images.none = Kein Bild passt (gesucht in Name, Bildunterschrift und Alternativtext).
sources_msg.images.template_error = Bilder: Vorlage fehlerhaft ({ $error })
sources_msg.images.read_error = Bilder des Werks: { $error }
sources_msg.page.template_error = Vorlage { $name }: { $error }
sources_msg.page.error = Fehler: { $error }
sources_msg.page.unknown_work = unbekanntes Werk
sources_msg.page.title_missing = Quelle fehlt: title={ $lt }id{ $gt } angeben
sources_msg.page.unknown_title = unbekannte Quelle
sources_msg.page.document_missing = Dokument fehlt: document={ $lt }id{ $gt } angeben
sources_msg.rpc.key_number = key: fester Schlüssel des Abschnitts (Zahl) erwartet
sources_msg.rpc.title_or_document = title (verknüpfter Titel) oder document (Dokument in MultiGPT) angeben
sources_msg.rpc.mark_query_too_long = q: höchstens { $max } Zeichen (die Anfrage dient nur dem Markieren der Fundstellen).
sources_msg.rpc.node_refused = node (Stelle) wird nicht angenommen; key mitgeben
sources_msg.rpc.key_missing = key fehlt: Abschnitt über seinen festen Schlüssel angeben
sources_msg.rag.no_way = kein Weg hat geantwortet
sources_msg.rag.foreign_page = Aufruf von einer fremden Seite; Baruch sucht nach Bedeutung nur, wenn die Suche hier in Baruch abgeschickt wird
sources_msg.rag.unavailable = Bedeutungssuche nicht verfügbar: { $reason }. Die Wortlaut-Suche (FTS) geht weiter.
sources_msg.notice.owned = Als eigene Hervorhebung übernommen; der Vorschlag der Persona bleibt, wie er war.
sources_msg.notice.deleted = Hervorhebung gelöscht.
sources_msg.notice.saved = Hervorhebung gespeichert.
sources_msg.notice.adopted = In die Bibliothek übernommen; der Titel ist mit dem Dokument in MultiGPT verknüpft.
sources_msg.notice.checked = Hervorhebung gegen den Volltext geprüft.
sources_msg.persona.answer_saved = Die Antwort der Persona ist gespeichert.
sources_msg.persona.text_missing = text: markierten Text angeben
sources_msg.persona.text_too_long = text: höchstens { $max } Zeichen
sources_msg.persona.source_too_long = source: höchstens { $max } Zeichen in einer Zeile
sources_msg.persona.no_lector = Keine Persona ist aktiv (kein Lektor).
sources_msg.highlight.already_own = Diese Hervorhebung ist schon deine eigene.
sources_msg.highlight.already_adopted = Diesen Vorschlag hast du schon als eigene Hervorhebung übernommen.
sources_msg.highlight.persona_stays_proposal = Hervorhebungen von Personas bleiben Vorschläge; aktiv sind nur eigene.
sources_msg.highlight.not_checkable = Gegen den Volltext prüfen lassen sich nur Hervorhebungen ohne Zitat mit Zeichenbereich (start, end).
sources_msg.form.title_missing = title: Quelle fehlt
sources_msg.form.document_missing = document: Dokument fehlt
sources_msg.form.highlight_missing = highlight: Hervorhebung fehlt
sources_msg.form.section_missing = Abschnitt fehlt.
sources_msg.form.confirm_delete = Zum Löschen das Kästchen „wirklich löschen“ ankreuzen.
sources_msg.form.unknown_action = unbekannte Aktion { $action }

# Bible page of the literature (literature/bibles.rs).
sources_msg.bible_page.no_bible = Keine SWORD-Bibel verfügbar: Modulordner und Bibel des Werks stellst du unter Einstellungen → Quellen → Bibeln (SWORD) ein.
sources_msg.bible_page.no_highlight = Hervorheben geht nur bei Titeln der Bibliothek (Zitat mit Zeichenbereich); eine Bibelstelle ist kein solcher Titel. Zitieren und „an Persona geben“ gehen.
sources_msg.bible_page.unreadable = Bibeln nicht lesbar: { $error }
sources_msg.bible_page.search_timeout = Bibelsuche abgebrochen: Zeitgrenze von 3 s erreicht.
sources_msg.bible_page.cited = Zitat als Vorschlag im Abschnitt angelegt; übernehmen kannst nur du.
sources_msg.bible_page.passage_invalid = Stelle ungültig
sources_msg.bible_page.no_text = Zu „{ $reference }“ hat das Modul { $module } keinen Text.
sources_msg.bible_page.reference_and_bible = Stelle und Bibel angeben (z. B. „Ps 139,23-24“).
sources_msg.bible_page.unknown_action = Unbekannte Aktion „{ $action }“.

# Full text of a source (literature/fulltext.rs).
sources_msg.fulltext.document_invalid = document: Dokument-ID von MultiGPT (1–500 Zeichen, eine Zeile).
sources_msg.fulltext.collection_invalid = collection: höchstens 200 Zeichen in einer Zeile.
sources_msg.fulltext.unavailable_now = Der Volltext ist gerade nicht abrufbar: { $error }
sources_msg.fulltext.part_from_one = part: Teile zählen ab 1.
sources_msg.fulltext.no_fulltext = Titel { $id } hat keinen Volltext: weder mit MultiGPT verknüpft noch als PDF/EPUB importiert.
sources_msg.fulltext.local_copy = MultiGPT ist nicht erreichbar ({ $error }); gezeigt wird die lokale Kopie vom { $time }.
sources_msg.fulltext.imported = Volltext aus der importierten Datei „{ $name }“ (lokal gelesen, ohne MultiGPT).
sources_msg.fulltext.imported_at = Volltext aus der importierten Datei „{ $name }“ (lokal gelesen am { $time }, ohne MultiGPT).
sources_msg.fulltext.library_error = Bibliothek: { $error }
sources_msg.fulltext.checked_imported = Geprüft gegen den lokalen Volltext aus „{ $name }“.
sources_msg.fulltext.not_checkable = Die Quelle ist nicht mit MultiGPT verknüpft und hat keinen importierten Volltext; ohne Volltext lässt sich der Bereich nicht prüfen.
sources_msg.fulltext.checked_copy = Geprüft gegen die lokale Kopie vom { $time } (MultiGPT nicht erreichbar).
sources_msg.fulltext.unavailable = Volltext nicht abrufbar: { $error }

# SWORD Bibles, commentaries and lexicons (bibles.rs).
sources_msg.sword.commentary_missing = Kommentar { $name } in keinem Modulordner gefunden
sources_msg.sword.lexicon_missing = Lexikon { $name } in keinem Modulordner gefunden
sources_msg.sword.commentary_problem = Kommentar { $name }: { $problem }
sources_msg.sword.lexicon_problem = Lexikon { $name }: { $problem }
sources_msg.sword.ref_problem = „{ $input }“: { $problem }
sources_msg.sword.unreadable_ref = „{ $reference }“ ist keine lesbare Stellenangabe
sources_msg.sword.no_entry = { $name } hat zu dieser Stelle keinen Eintrag
sources_msg.sword.query_length = query: 1 bis 200 Zeichen
sources_msg.sword.no_lexicon = Kein Lexikon gewählt: Einstellungen → Werk → Kommentare und Lexika oder module angeben.
sources_msg.sword.lexicon_unreadable = Lexikon nicht lesbar: { $problems }
sources_msg.sword.key_length = key: 1 bis 200 Zeichen
sources_msg.sword.name_list = { $name }: Liste von Modulnamen erwartet
sources_msg.sword.param_missing = { $name } fehlt
sources_msg.sword.key_number = key: fester Schlüssel (Zahl) erwartet
sources_msg.sword.dir_invalid = Ordner zu lang oder mit Steuerzeichen (höchstens { $max } Zeichen)
sources_msg.sword.no_home = ~ lässt sich nicht auflösen (HOME fehlt)
sources_msg.sword.dir_relative = Ordner „{ $dir }“: bitte einen absoluten Pfad angeben
sources_msg.sword.too_many_dirs = höchstens { $max } Modulordner
sources_msg.sword.no_bible_chosen = Keine Bibel gewählt: Einstellungen → Quellen → Bibeln (SWORD) oder module angeben.
sources_msg.sword.bible_missing = Bibelmodul { $name } in keinem Modulordner gefunden.
sources_msg.sword.bible_problem = Bibelmodul { $name }: { $problem }
sources_msg.sword.unknown_section = Abschnitt unbekannt
sources_msg.sword.no_revision = Fassung { $number } gibt es nicht
sources_msg.sword.section_without_revision = Abschnitt ohne Fassung
sources_msg.sword.text_too_long = text: höchstens { $max } Zeichen
sources_msg.sword.cipher_key_length = key: 1 bis { $max } Zeichen in einer Zeile; nichts gespeichert.
sources_msg.sword.module_missing = Modul { $name } in keinem Modulordner gefunden
sources_msg.sword.not_enciphered = Modul { $name } ist nicht verschlüsselt; kein Schlüssel nötig
sources_msg.sword.keyring_refused_set = Schlüsselbund lehnt ab ({ $error }); nichts gespeichert.
sources_msg.sword.confirm_delete = Löschen bitte bestätigen (confirm: true); nichts geändert.
sources_msg.sword.keyring_refused_delete = Schlüsselbund lehnt ab ({ $error }); nichts geändert.
sources_msg.sword.dirs_list = dirs: Liste von Texten erwartet
sources_msg.sword.dirs_list_or_null = dirs: Liste von Texten oder null (Vorgaben)
sources_msg.sword.module_or_null = module: Modulname oder null
sources_msg.sword.ref_too_long = ref: höchstens 500 Zeichen
sources_msg.sword.rev_number = rev: Nummer der Fassung erwartet
sources_msg.sword.ref_length = ref: 1 bis 500 Zeichen
sources_msg.sword.prefix_too_long = prefix: höchstens 200 Zeichen
sources_msg.commentary.summary = Kommentare zu { $reference }
sources_msg.commentary.one_entry = { $count } Eintrag
sources_msg.commentary.entries = { $count } Einträge
sources_msg.commentary.group = Kommentar · { $module }
sources_msg.commentary.shortened = (gekürzt)
sources_msg.commentary.more_hits = weitere Treffer nicht gezeigt (Grenze erreicht)
sources_msg.commentary.license = Lizenz: { $notice }
sources_msg.commentary.no_notice = keine Angabe
