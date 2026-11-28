# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of the synchronisation with MultiGPT (src/sync/**): notices of the page, errors of sync.*, notes of a
# run (Round 39a, task messages_2).

# Page /abgleich.
sync_msg.page.started = Der Abgleich läuft im Hintergrund; diese Seite zeigt nach dem Neuladen das Ergebnis.
sync_msg.page.running = Es läuft schon ein Abgleich; bitte warten, bis er fertig ist.
sync_msg.page.upload_on = Hochladen eingeschaltet: Der nächste Abgleich lädt die Kapitel dieses Werks nach MultiGPT.
sync_msg.page.upload_off = Hochladen ausgeschaltet: Der Abgleich lädt dieses Werk nicht mehr hoch.
sync_msg.page.collections = Die Auswahl der Sammlungen ist gespeichert; der nächste Abgleich holt sie.
sync_msg.page.loaded = Die Sammlungen von MultiGPT sind geladen; bitte auswählen und übernehmen.
sync_msg.page.confirm = Bitte bestätigen, dass die aktuellen Fassungen dieses Werks nach MultiGPT gehen dürfen; nichts geändert.
sync_msg.page.no_work = Dieses Werk gibt es nicht.
sync_msg.page.collection = Sammlung: 1 bis 200 Zeichen in einer Zeile, höchstens 100; nichts geändert.
sync_msg.page.not_loaded = Sammlungen nicht geladen: { $detail }
sync_msg.page.no_answer = MultiGPT antwortet nicht.
sync_msg.page.failed = Das ging nicht; bitte noch einmal versuchen.
sync_msg.page.error = Fehler: { $detail }

# Connection and errors of sync.* (src/sync/mod.rs).
sync_msg.link.settings = Einstellungen der Verbindung MultiGPT: { $detail }
sync_msg.link.no_address = Keine Adresse für MultiGPT eingetragen (Einstellungen → Verbindungen).
sync_msg.link.keyring = Schlüsselbund: { $detail }
sync_msg.link.token_other = Das Token baruch/multigpt ist für eine andere Adresse hinterlegt; Baruch schickt es nicht an die jetzige. Für die neue Adresse das Token neu eintragen (Einstellungen → Verbindungen).
sync_msg.link.denied = MultiGPT verweigert den Zugriff ({ $detail }). Zum Hochladen braucht der Schlüssel das Recht docs.write, zum Holen docs.read.
sync_msg.link.unreachable = MultiGPT ist nicht erreichbar ({ $detail }); die Arbeit geht lokal weiter.
sync_msg.run.nothing = Nichts abzugleichen: kein Werk mit „nach MultiGPT hochladen“, keine Sammlung gewählt.
sync_msg.run.done = Abgleich fertig.
sync_msg.run.done_notes = Abgleich mit Hinweisen beendet.
sync_msg.run.store = Speicher: { $detail }
sync_msg.rpc.unknown_work = Unbekanntes Werk { $work }.
sync_msg.rpc.confirm = confirm: true fehlt. Hochladen bitte bestätigen: Die aktuellen Fassungen dieses Werks gehen beim nächsten Abgleich als Dokumente nach MultiGPT; nichts geändert.
sync_msg.rpc.no_collections_tool = MultiGPT bietet das Werkzeug list_collections nicht an (tools/list).
sync_msg.rpc.collections_unreadable = list_collections: Antwort nicht lesbar (weder structuredContent noch JSON in content).
sync_msg.rpc.work_id = work: Werk-id (Zahl) erwartet.
sync_msg.rpc.params_object = Parameter als Objekt erwartet
sync_msg.rpc.work_missing = work: Werk-id fehlt.
sync_msg.rpc.on = on: true oder false erwartet.
sync_msg.rpc.collections_list = collections: Liste von Texten erwartet.

# Tools of MultiGPT and their schemas (src/sync/args.rs, download.rs, upload.rs).
sync_msg.tool.missing = MultiGPT bietet das Werkzeug { $tool } nicht an (tools/list).
sync_msg.tool.unknown_param = { $tool } verlangt den Parameter „{ $name }“, den Baruch nicht kennt.
sync_msg.tool.no_collection_param = { $tool } nennt im inputSchema keinen Parameter für die Sammlung.
sync_msg.tool.no_collection_list = { $tool } nennt im inputSchema keinen Parameter für die Sammlung; ohne ihn listete MultiGPT alle Dokumente, darum fragt Baruch nicht.
sync_msg.tool.no_document_param = { $tool } nennt im inputSchema keinen Parameter für das Dokument.
sync_msg.tool.no_title_param = { $tool } nennt im inputSchema keinen Parameter für den Titel.
sync_msg.tool.no_text_param = { $tool } nennt im inputSchema keinen Parameter für den Text.
sync_msg.tool.list_unreadable = list_documents: Antwort nicht lesbar (weder structuredContent noch JSON in content).
sync_msg.tool.no_list = list_documents: keine Dokumentliste in der Antwort (documents, items …).
sync_msg.tool.no_ids =
    list_documents: { $count } { $count ->
        [one] Eintrag
       *[other] Einträge
    } ohne Dokument-ID; Format nicht lesbar.

# Notes of a run: fetching (download.rs).
sync_msg.download.not_read = Sammlung „{ $collection }“ nicht gelesen: { $detail }
sync_msg.download.incomplete = Die Liste der Sammlung „{ $collection }“ ist womöglich unvollständig (volle Seite ohne Fortsetzung); lokale Kopien, die darin fehlen, bleiben erhalten.
sync_msg.download.too_many = Sammlung „{ $collection }“ hat mehr als { $max } Dokumente; geholt werden nur die ersten { $max }.
sync_msg.download.doc_not_read = „{ $title }“ ({ $collection }) nicht gelesen: { $detail }
sync_msg.download.doc_too_big = „{ $title }“ ({ $collection }) ist größer als { $max } und wird nicht geholt.
sync_msg.download.budget = Grenze von { $max } je Abgleich erreicht; die übrigen Dokumente folgen beim nächsten Abgleich.

# Notes of a run: uploading (upload.rs).
sync_msg.upload.resting = Weitere Änderungen nach MultiGPT ruhen bis zum nächsten Abgleich; lokal ist nichts verloren.
sync_msg.upload.problem = { $what }: { $detail }
sync_msg.upload.aborted = Der letzte Abgleich brach beim Hochladen von „{ $title }“ ab (die Antwort von MultiGPT kam nicht an); dort liegt es womöglich ohne Zuordnung doppelt – bitte in der Sammlung „{ $collection }“ prüfen.
sync_msg.upload.uploading = Hochladen von „{ $title }“
sync_msg.upload.no_id = „{ $title }“: MultiGPT nannte keine Dokument-ID. Das Dokument liegt vermutlich dort, Baruch kann es aber weder ersetzen noch löschen; erneut hochgeladen wird es erst nach einer Änderung.
sync_msg.upload.old_no_id = Die vorige Fassung von „{ $title }“ hatte keine Dokument-ID und bleibt bei MultiGPT liegen (bitte dort löschen).
sync_msg.upload.delete_old = Löschen der alten Fassung von „{ $title }“ ({ $id })
sync_msg.upload.delete = Löschen von „{ $title }“ ({ $id })
