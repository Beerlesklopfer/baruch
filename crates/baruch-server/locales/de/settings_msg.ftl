# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of the settings pages (src/settings/**): page names, storage notes, notices after saving, form
# errors and refusals of settings.* (Round 39a, task messages_2).

# Pages of the settings (navigation, title, storage note).
settings_msg.page.display = Darstellung
settings_msg.page.work = Werk
settings_msg.page.assistants = Assistenten
settings_msg.page.connections = Verbindungen
settings_msg.page.sources = Quellen
settings_msg.page.access = MCP-Zugang
settings_msg.page.data = Daten
settings_msg.page.accounts = Konten
settings_msg.storage.display = Cookie in diesem Browser
settings_msg.storage.work = SQLite, Git-Spiegel des Werks
settings_msg.storage.all_works = SQLite (alle Werke)
settings_msg.storage.connections = SQLite, Geheimnisse im Schlüsselbund
settings_msg.storage.access = SQLite (nur Hash der Tokens); Clients zur Laufzeit
settings_msg.storage.data = Datenordner
settings_msg.storage.accounts = SQLite dieser Instanz (Passwort nur als Prüfwert; nicht im Austausch, nicht im Spiegel)

# Frame, errors of the pages, settings.* over /rpc.
settings_msg.frame.unknown_work = unbekanntes Werk { $work }
settings_msg.frame.unknown_page = unbekannte Einstellungsseite „{ $slug }“
settings_msg.frame.error = Fehler: { $detail }
settings_msg.rpc.page_missing = page fehlt (z. B. „darstellung“)
settings_msg.rpc.work_id = work: Werk-id (Zahl) erwartet
settings_msg.rpc.query_text = query.{ $key }: Text erwartet
settings_msg.rpc.query_object = query: Objekt erwartet
settings_msg.rpc.read_connections = Verbindungen und Quellen liest man über connection.list bzw. sources.list.
settings_msg.rpc.values_object = values: Objekt mit den Feldern der Seite erwartet
settings_msg.rpc.display_cookies = Darstellung liegt in Cookies dieses Browsers: Cookies setzt das Formular POST /einstellungen/darstellung/act.
settings_msg.rpc.write_connections = Verbindungen und Quellen schreibt man über connection.* bzw. sources.*.
settings_msg.rpc.nothing_to_save = Die Seite „{ $page }“ hat nichts zu speichern.
settings_msg.rpc.backup_confirm = Sicherung bitte bestätigen (confirm: true).

# Display.
settings_msg.display.saved = Gespeichert – gilt in diesem Browser.
settings_msg.display.theme = Thema: „System“, „Hell“ oder „Dunkel“ wählen.
settings_msg.display.editor = Editor: „Satzbild“ oder „Quelltext“ wählen.
settings_msg.display.proposals = Vorschläge: „aufgeklappt“ oder „eingeklappt“ wählen.
settings_msg.display.accordion = Bereich rechts: „Assistenten“, „Fassungen“ oder „Abschnitts-Chat“ wählen.

# Assistants.
settings_msg.assistants.color = Die Farbe ist gespeichert.
settings_msg.assistants.persona = Die Persona ist gespeichert.
settings_msg.assistants.lector = Die Wahl des Lektors ist gespeichert.
settings_msg.assistants.prompt = Der Rollen-Prompt ist gespeichert.
settings_msg.assistants.prompt_default = Der Rollen-Prompt gilt wieder in der Vorgabe.
settings_msg.assistants.rag = Die Einstellung zu den Treffern der Bedeutungssuche ist gespeichert.
settings_msg.assistants.websearch = Die Einstellung zur Websuche von @belege ist gespeichert.
settings_msg.assistants.connection = Verbindung und Freigabe der Persona sind gespeichert.
settings_msg.assistants.bible = Die Einstellung zu den Bibel-Belegen aus SWORD ist gespeichert.
settings_msg.assistants.chat = Der Umfang des Chats ist gespeichert.
settings_msg.assistants.cached_list = Liste aus dem Zwischenspeicher (Stand { $stamp }).
settings_msg.assistants.color_format = Farbe als #rrggbb angeben.

# MCP access.
settings_msg.access.revoke_confirm = Widerrufen bitte bestätigen (Kästchen ankreuzen).
settings_msg.access.which_token = Welcher Token? (id fehlt)

# Data and backup.
settings_msg.data.open_permissions = Der Datenordner ist auch für andere Benutzer dieses Rechners zugänglich (Rechte { $mode }). Baruch ändert die Rechte eines vorhandenen Ordners nicht; „chmod 700“ auf den Ordner lässt nur noch den Besitzer hinein.
settings_msg.data.unreadable = { $path } lässt sich nicht lesen: { $detail }
settings_msg.data.not_created = { $path } lässt sich nicht anlegen: { $detail }
settings_msg.data.not_copied = { $path } lässt sich nicht kopieren: { $detail }
settings_msg.data.no_room = Zu wenig Platz für die Sicherung: Sie braucht etwa { $needed }, frei sind { $free }, und { $reserve } sollen danach frei bleiben (für die laufende Datenbank und die Spiegel); nichts angelegt.
settings_msg.data.folder_missing = Datenordner { $path } nicht gefunden: { $detail }
settings_msg.data.no_parent = Der Datenordner { $path } hat keinen Elternordner für die Sicherung.
settings_msg.data.time = Zeit der Sicherung: { $detail }
settings_msg.data.space_aborted = Platz prüfen abgebrochen: { $detail }
settings_msg.data.too_many = Neben dem Datenordner liegen schon { $count } Sicherungen ({ $name }.sicherung-…, höchstens { $max }); bitte alte entfernen oder verschieben. Nichts angelegt.
settings_msg.data.exists = Die Sicherung { $path } gibt es schon; nichts überschrieben. Bitte in einer Sekunde noch einmal.
settings_msg.data.not_utf8 = { $path } ist kein UTF-8-Pfad
settings_msg.data.vacuum = Datenbank sichern (VACUUM INTO): { $detail }
settings_msg.data.copy_aborted = Kopieren abgebrochen: { $detail }
settings_msg.data.backup_confirm = Sicherung bitte bestätigen (Kästchen ankreuzen).

# Work.
settings_msg.work.saved = Das Werk ist gespeichert.
settings_msg.work.scheme = Das Gliederungsschema ist übernommen.
settings_msg.work.book = Die Buchangaben sind gespeichert.
settings_msg.work.cover = Der Umschlag ist übernommen.
settings_msg.work.imprint = Die Impressum-Vorlage ist in die Werkvorlage übernommen (neue Fassung der Vorlage).
settings_msg.work.layout = Die Umschlag-Anordnung ist gespeichert.
settings_msg.work.upload_on = Hochladen nach MultiGPT ist eingeschaltet; es geschieht beim nächsten Abgleich (Werkzeuge → Abgleichen).
settings_msg.work.upload_off = Hochladen nach MultiGPT ist ausgeschaltet.
settings_msg.work.typeset = Die Satz-Einstellungen sind gespeichert.
settings_msg.work.typeset_reset = Die Satz-Einstellungen sind zurückgesetzt; es gilt wieder nur die Werkvorlage.
settings_msg.work.typeset_template = Der Kapitelanfang ist aus der Werkvorlage in die Satz-Einstellungen übernommen (neue Fassung der Vorlage).
settings_msg.work.commentaries = Kommentare und Lexika des Werks sind gespeichert.
settings_msg.work.timing = Die Predigtzeit ist gespeichert.
settings_msg.work.language = Die Werksprache ist gespeichert.
settings_msg.work.sheet = Die Satzbild-Einstellungen sind gespeichert.
settings_msg.work.revisions = Die Einstellung „Fassungen“ ist gespeichert.
settings_msg.work.folded = Ältere Fassungen sind ausgeblendet (nichts gelöscht).
settings_msg.work.unfolded = Alle Fassungen werden wieder gezeigt.
settings_msg.work.module_missing = in keinem Modulordner gefunden
settings_msg.work.commentaries_error = Die Auswahl ließ sich nicht speichern: Ein Modul fehlt oder ist nicht lesbar; nichts geändert.
settings_msg.work.timing_error = Die Predigtzeit ließ sich nicht speichern: Wörter je Minute 60 bis 250, Zielzeit 1 bis 240 Minuten (oder leer); nichts geändert.
settings_msg.work.language_no_work = Die Werksprache ließ sich nicht speichern: dieses Werk gibt es nicht; nichts geändert.
settings_msg.work.language_error = Die Werksprache ließ sich nicht speichern: erlaubt sind Deutsch und Englisch; nichts geändert.
settings_msg.work.revisions_error = Die Einstellung „Fassungen“ ließ sich nicht speichern: größte Änderung { $lo } bis { $hi } Prozent, neue Fassung nach { $min_lo } bis { $min_hi } Minuten (0 = keine Zeitgrenze); nichts geändert.
settings_msg.work.fold_error = Ältere Fassungen ließen sich nicht ausblenden: Tag als Datum angeben; nichts geändert.
settings_msg.work.sheet_error = Die Satzbild-Einstellungen ließen sich nicht speichern: Schrift höchstens 200 Zeichen (Namen durch Kommas getrennt, ohne { $chars }), Schriftgröße 6 bis 48 pt, Satzbreite 6 bis 60 cm (Einheit pt, mm, cm oder em), Farben als #rrggbb; nichts geändert.
settings_msg.work.no_work = Es gibt kein Werk.

# Typesetting settings of the work.
settings_msg.typeset.unknown_choice = { $field }: unbekannte Wahl „{ $raw }“
settings_msg.typeset.margins = Ränder: alle vier Ränder angeben oder alle leer
settings_msg.typeset.settings_missing = settings fehlt
settings_msg.typeset.register_gone = Register „{ $slug }“ (gibt es nicht mehr)
settings_msg.typeset.unknown_work = Unbekanntes Werk.
settings_msg.typeset.reset_confirm = Zurücksetzen bitte mit dem Häkchen bestätigen; nichts geändert.
settings_msg.typeset.unknown_action = Unbekannte Aktion (save, preview oder reset).
settings_msg.typeset.apply_confirm = Übernehmen bitte mit dem Häkchen bestätigen; die Werkvorlage bleibt unverändert.
settings_msg.typeset.preview_gone = Vorschau gibt es nicht (mehr); bitte „Vorschau“ erneut drücken.
