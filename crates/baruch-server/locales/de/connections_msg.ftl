# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of the connections and sources (src/connections/**): checks, field checks, keyring, page
# „Verbindungen“ with its message codes, check details and form fields (Round 39a, task messages_2).

# Checks of a connection (src/connections/check.rs, mod.rs).
connections_msg.check.timeout = Keine Antwort von { $url } innerhalb von { $seconds } s.
connections_msg.check.no_connection = Keine Verbindung zu { $url }: { $detail }
connections_msg.check.request_failed = Anfrage an { $url } gescheitert: { $detail }
connections_msg.check.too_big = Antwort von { $url } ist größer als 1 MiB; abgebrochen.
connections_msg.check.redirect = { $target } leitet weiter (HTTP { $status }); Baruch folgt Weiterleitungen nicht – bitte die endgültige Adresse eintragen.
connections_msg.check.key_hint = Schlüssel baruch/{ $account } prüfen
connections_msg.check.no_key = kein Schlüssel hinterlegt (baruch/{ $account })
connections_msg.check.denied = Zugang verweigert (HTTP { $status }): { $hint }.
connections_msg.check.http_status = { $target } antwortet mit HTTP { $status }.
connections_msg.check.no_json = { $target } liefert kein JSON.
connections_msg.check.no_models = { $target } liefert keine Modellliste (Feld data fehlt).
connections_msg.check.total_timeout = Zeitlimit: Keine vollständige Antwort von { $url } innerhalb von { $seconds } s; Prüfung abgebrochen.
connections_msg.check.tool_pages = tools/list hat mehr als { $max } Seiten; gezeigt sind die Werkzeuge der ersten { $max }.
connections_msg.check.collections_unreadable = { $tool }: Antwort nicht lesbar, keine Sammlungen.
connections_msg.check.collections_failed = Sammlungen nicht lesbar: { $detail }
connections_msg.check.collections_tool_missing = { $tool } fehlt; Sammlungen lassen sich von Hand eintragen.
connections_msg.check.pg_ok = Verbindung steht, Erweiterung vector { $vector }.
connections_msg.check.pg_no_vector = Verbindung steht, aber die Erweiterung vector fehlt (CREATE EXTENSION vector).
connections_msg.check.pg_password_hint = Passwort baruch/pgvector prüfen
connections_msg.check.pg_no_password = kein Passwort hinterlegt (baruch/pgvector)
connections_msg.check.pg_login = Anmeldung abgelehnt: { $detail } ({ $hint }).
connections_msg.check.pg_database = Datenbank { $address }: { $detail }
connections_msg.check.pg_io = Keine Verbindung zu { $address }: { $detail }
connections_msg.check.pg_tls = TLS mit { $address }: { $detail }
connections_msg.check.pg_other = Verbindung zu { $address }: { $detail }
connections_msg.check.withheld = Nicht geprüft: Das Geheimnis ist für { $bound } hinterlegt, die Adresse zeigt jetzt anderswohin. Baruch schickt es nicht an ein anderes Ziel. Für die neue Adresse das Geheimnis neu eintragen (oder die alte Adresse wieder einstellen).
connections_msg.check.incomplete = Keine vollständige Antwort innerhalb von { $seconds } s; Prüfung abgebrochen.

# Field checks and connection.* over /rpc (src/connections/config.rs, collections.rs, mod.rs, sources.rs).
connections_msg.field.url = Adresse fehlt oder ist ungültig (eine Zeile, höchstens 2000 Zeichen).
connections_msg.field.url_invalid = Adresse ist ungültig.
connections_msg.field.url_scheme = Adresse muss mit http:// oder https:// beginnen.
connections_msg.field.url_user = Adresse ohne Benutzer und Passwort (Geheimnisse gehören in den Schlüsselbund).
connections_msg.field.url_host = Adresse ohne Rechnernamen.
connections_msg.field.url_secret = Adresse ohne Schlüssel oder Token in der Abfrage (dafür gibt es den Schlüsselbund).
connections_msg.field.text = { $name }: Text erwartet.
connections_msg.field.host = Rechner: Name oder IP-Adresse ohne Leerzeichen, / oder @.
connections_msg.field.port = Port: ganze Zahl von 1 bis 65535.
connections_msg.field.short = { $name }: höchstens 63 Zeichen in einer Zeile.
connections_msg.field.tls = TLS: disable, prefer, require oder verify-full.
connections_msg.field.dir = Ordner: nicht leer, eine Zeile.
connections_msg.field.table = Tabelle: Name aus Buchstaben, Ziffern und _ (nicht mit einer Ziffer beginnend, höchstens 63 Zeichen), wahlweise schema.tabelle.
connections_msg.field.column = { $name }: Spaltenname aus Buchstaben, Ziffern und _ (nicht mit einer Ziffer beginnend, höchstens 63 Zeichen).
connections_msg.field.column_optional = { $name }: Spaltenname aus Buchstaben, Ziffern und _ (nicht mit einer Ziffer beginnend, höchstens 63 Zeichen); leer = Spalte gibt es nicht.
connections_msg.field.config_object = config: JSON-Objekt erwartet.
connections_msg.field.secret_in_config = Geheimnisse gehören nicht in die Einstellungen, sondern in den Schlüsselbund (connection.set_secret).
connections_msg.rpc.way = way: pgvector oder multigpt (nur diese Wege haben Sammlungen).
connections_msg.rpc.way_missing = way fehlt (pgvector oder multigpt).
connections_msg.rpc.collections = collections: Liste von Texten erwartet.
connections_msg.rpc.keyring = Schlüsselbund: { $detail }
connections_msg.rpc.kind = kind: eliezer, multigpt, pgvector oder lancedb.
connections_msg.rpc.no_secret = LanceDB braucht kein Geheimnis.
connections_msg.rpc.secret_empty = secret: leer; nichts geändert.
connections_msg.rpc.secret_long = secret: höchstens 4096 Zeichen in einer Zeile.
connections_msg.rpc.work_id = work: Werk-id (Zahl) erwartet.
connections_msg.rpc.kind_missing = kind fehlt (eliezer, multigpt, pgvector oder lancedb).
connections_msg.rpc.confirm = confirm: true fehlt; nichts geändert.
connections_msg.rpc.ways = ways: Liste aus lancedb, pgvector, multigpt.
connections_msg.rpc.params_object = Parameter als Objekt erwartet
connections_msg.sources.title = title: 1 bis 200 Zeichen in einer Zeile.
connections_msg.sources.url = url: http:// oder https:// mit Rechner, ohne Benutzer, höchstens 2000 Zeichen.
connections_msg.sources.note = note: höchstens 4000 Zeichen.
connections_msg.sources.confirm = confirm: true fehlt; nichts gelöscht.
connections_msg.sources.usage = Die Websuche für @belege (Einstellungen → Assistenten, Vorgabe aus) fragt zuerst die ersten drei Quellen dieser Liste über „{ $site }“ ab, danach das allgemeine Web. Treffer, deren Rechner zu einer Quelle passt, sind als „bevorzugte Quelle“ gekennzeichnet.
connections_msg.keyring.no_access = kein Zugriff auf den Schlüsselbund (gesperrt oder nicht erreichbar)
connections_msg.keyring.failure = Fehler des Schlüsselbunds: { $detail }
connections_msg.keyring.encoding = der hinterlegte Wert ist kein UTF-8-Text
connections_msg.keyring.ambiguous = { $count } Einträge passen; bitte im Schlüsselbund aufräumen

# What a connection is used for (page „Verbindungen“, the texts of connections::usage_stub).
connections_msg.usage.eliezer = Personas nutzen Eliezer für ihre Antworten, Prüfungen und Vorschläge – immer nur eine Anfrage zur Zeit; das Modell je Persona steht unter Einstellungen → Assistenten. Dazu bettet Eliezer die Texte für die Bedeutungssuche ein (LanceDB und pgvector, eigene Warteschlange, höchstens 32 Texte je Anfrage); ohne Eliezer bleibt die Wortlaut-Suche.
connections_msg.usage.multigpt = Suche über MultiGPT (search_documents) in den gewählten Sammlungen des Werks, Treffer mit Quellenangabe; read_document als Ausschnitt einer Fundstelle und als Volltext in der Dokumentansicht der Literatur (teilweise, ohne JavaScript); Hervorhebungen ohne Zitat werden gegen den Volltext geprüft.
connections_msg.usage.pgvector = Bedeutungssuche direkt in pgvector (Kosinus-Abstand, nur lesend) in den gewählten Sammlungen des Werks; die Anfrage wird mit demselben Einbettungsmodell eingebettet wie die Daten.
connections_msg.usage.lancedb = Haupt-RAG im Datenordner: Abschnitte (aktuelle Fassung), Einfälle, Hervorhebungen und die Autorenbibliothek, eingebettet über Eliezer; ohne Einbettungsmodell bleibt die Wortlaut-Suche (FTS).

# Details of a check (page „Verbindungen“, connections::pages::detail_lines).
connections_msg.detail.more = { $list } … und { $more } weitere
connections_msg.detail.count = { $name } ({ $count } { $unit })
connections_msg.detail.unit_documents = Dokumente
connections_msg.detail.unit_sections = Abschnitte
connections_msg.detail.collections = Sammlungen: { $list }
connections_msg.detail.models = Modelle: { $list }
connections_msg.detail.server = Server: { $name }
connections_msg.detail.tools = Werkzeuge: { $list }
connections_msg.detail.tool_pages = Werkzeugliste in { $pages } Seiten
connections_msg.detail.vector = Erweiterung vector: { $vector }
connections_msg.detail.vector_missing = fehlt
connections_msg.detail.table = Tabelle: { $table }
connections_msg.detail.table_columns = Tabelle: { $table } ({ $columns })
connections_msg.detail.chunks = Abschnitte: { $count }
connections_msg.detail.dimension = Dimension: { $count }
connections_msg.detail.data_models = Einbettungsmodelle in den Daten: { $list }
connections_msg.detail.dir = Ordner: { $dir }
connections_msg.detail.dir_missing = Ordner: { $dir } (fehlt noch)
connections_msg.detail.dir_readonly = Ordner: { $dir } (nicht beschreibbar)
connections_msg.detail.dir_writable = Ordner: { $dir } (beschreibbar)
connections_msg.detail.tables = Tabellen: { $list }
connections_msg.detail.entries = Einträge: { $count }
connections_msg.detail.index_dimension = Index: { $model }, Dimension { $dimension }
connections_msg.detail.index = Index: { $model }
connections_msg.detail.configured_model = Eingestelltes Modell: { $model }
connections_msg.detail.embedder_ok = Einbettung: geht
connections_msg.detail.embedder = Einbettung: { $detail }
connections_msg.detail.contexts = Gemeldete Kontextlängen: { $list }
connections_msg.detail.tokens = { $id }: { $count } Tokens

# Input fields of a connection (label and hint, connections::pages::form_fields).
connections_msg.form.url = Adresse
connections_msg.form.url_hint = http:// oder https://, ohne Benutzer und Passwort
connections_msg.form.host = Rechner (Host)
connections_msg.form.host_hint = leer = nicht eingerichtet
connections_msg.form.port = Port
connections_msg.form.database = Datenbank
connections_msg.form.database_hint = leer = wie der Benutzer
connections_msg.form.user = Benutzer
connections_msg.form.tls = TLS
connections_msg.form.dir = Ordner
connections_msg.form.dir_hint = relativ zum Datenordner oder absolut
connections_msg.form.embedding_model = Einbettungsmodell
connections_msg.form.embedding_model_hint = dasselbe Modell, mit dem die Daten eingebettet sind
connections_msg.form.table = Tabelle
connections_msg.form.table_hint = tabelle oder schema.tabelle, genau wie in der Datenbank
connections_msg.form.text_column = Spalte Text
connections_msg.form.vector_column = Spalte Vektor
connections_msg.form.vector_column_hint = Typ vector (pgvector)
connections_msg.form.collection_column = Spalte Sammlung
connections_msg.form.document_column = Spalte Dokument
connections_msg.form.section_column = Spalte Stelle
connections_msg.form.title_column = Spalte Titel
connections_msg.form.url_column = Spalte Adresse
connections_msg.form.model_column = Spalte Modell
connections_msg.form.optional_hint = leer = gibt es nicht
connections_msg.form.model_column_hint = leer = gibt es nicht; sonst prüft Baruch das Modell je Sammlung
connections_msg.form.query_prefix = Präfix der Anfrage
connections_msg.form.query_prefix_pg_hint = wie die Daten eingebettet sind; für nomic-embed-text „search_query: “ (mit Leerzeichen am Ende); leer = kein Präfix
connections_msg.form.query_prefix_hint = für nomic-embed-text „search_query: “ (mit Leerzeichen am Ende); leer = kein Präfix
connections_msg.form.document_prefix = Präfix der Dokumente
connections_msg.form.document_prefix_hint = für nomic-embed-text „search_document: “; leer = kein Präfix. Eine Änderung verlangt „Index neu aufbauen“
connections_msg.form.other = Feld
connections_msg.page.template = Vorlage { $name }: { $detail }

# Message codes of the page „Verbindungen“ (?ok=, ?fehler=, ?modellfehler=).

connections_msg.ok.gespeichert = Die Einstellungen sind gespeichert.
connections_msg.ok.geheimnis = Das Geheimnis ist im Schlüsselbund hinterlegt.
connections_msg.ok.gespeichert_geheimnis = Die Einstellungen sind gespeichert, das Geheimnis ist im Schlüsselbund hinterlegt.
connections_msg.ok.geheimnis_entfernt = Das Geheimnis ist aus dem Schlüsselbund entfernt.
connections_msg.ok.geprueft = Die Verbindung ist geprüft; das Ergebnis steht bei der Verbindung.
connections_msg.ok.kurztest = Der Kurztest ist gelaufen; das Ergebnis steht bei Eliezer.
connections_msg.ok.rag = Die RAG-Wege des Werks sind gespeichert.
connections_msg.ok.sammlungen = Die Sammlungen des Werks sind gespeichert.
connections_msg.ok.reindex = Der Index des Werks wird neu aufgebaut; den Stand zeigt „Status der Wege“.
connections_msg.ok.angelegt = Die Quelle ist angelegt.
connections_msg.ok.geaendert = Die Quelle ist geändert.
connections_msg.ok.verschoben = Die Quelle ist verschoben.
connections_msg.ok.geloescht = Die Quelle ist gelöscht.
connections_msg.ok.bibel_ordner = Die Modulordner der Bibeln sind gespeichert.
connections_msg.ok.bibel_vorgabe = Die Modulordner der Bibeln stehen wieder auf der Vorgabe.
connections_msg.ok.bibel_gewaehlt = Die Bibel des Werks ist gewählt; die Stellenprüfung prüft jetzt den Wortlaut.
connections_msg.ok.bibel_keine = Das Werk hat keine Bibel mehr; der Wortlaut bleibt ungeprüft.
connections_msg.ok.bibel_schluessel = Der Schlüssel passt und ist im Schlüsselbund hinterlegt; das Modul ist lesbar.
connections_msg.ok.bibel_schluessel_entfernt = Der Schlüssel ist aus dem Schlüsselbund entfernt.
connections_msg.error.kind = Unbekannte Verbindung; nichts geändert.
connections_msg.error.url = Die Adresse ist ungültig; nichts geändert.
connections_msg.error.url_schema = Die Adresse muss mit http:// oder https:// beginnen; nichts geändert.
connections_msg.error.url_benutzer = Die Adresse darf keinen Benutzer und kein Passwort enthalten – Geheimnisse gehören in den Schlüsselbund; nichts geändert.
connections_msg.error.url_geheimnis = Die Adresse darf keinen Schlüssel und kein Token in der Abfrage tragen – dafür gibt es den Schlüsselbund; nichts geändert.
connections_msg.error.geheimnis_feld = Geheimnisse gehören nicht in die Einstellungen, sondern in den Schlüsselbund; nichts geändert.
connections_msg.error.host = Der Rechner (Host) ist ungültig: Name oder IP-Adresse ohne Leerzeichen; nichts geändert.
connections_msg.error.port = Der Port muss eine ganze Zahl von 1 bis 65535 sein; nichts geändert.
connections_msg.error.tls = TLS: disable, prefer, require oder verify-full; nichts geändert.
connections_msg.error.feld = Ein Feld ist ungültig (zu lang oder mehrzeilig); nichts geändert.
connections_msg.error.ordner = Der Ordner darf nicht leer sein; nichts geändert.
connections_msg.error.modell = Das Einbettungsmodell darf nicht leer sein (höchstens 200 Zeichen); nichts geändert.
connections_msg.error.praefix = Ein Aufgaben-Präfix darf höchstens 64 Zeichen lang sein und keine Steuerzeichen (auch keinen Zeilenumbruch) enthalten; nichts geändert.
connections_msg.error.geheimnis_leer = Das Geheimnis ist leer; nichts geändert.
connections_msg.error.geheimnis_ungueltig = Das Geheimnis ist zu lang oder mehrzeilig; nichts geändert.
connections_msg.error.kein_geheimnis = LanceDB braucht kein Geheimnis.
connections_msg.error.bestaetigen = Bitte bestätigen (Kästchen anhaken); nichts geändert.
connections_msg.error.schluesselbund = Der Schlüsselbund ist nicht erreichbar oder lehnt ab; nichts geändert.
connections_msg.error.gespeichert_ohne_geheimnis = Die Einstellungen sind gespeichert, das Geheimnis aber nicht: Der Schlüsselbund ist nicht erreichbar oder lehnt ab. Das bisherige Geheimnis bleibt und wird nicht an eine geänderte Adresse geschickt.
connections_msg.error.werk = Unbekanntes Werk; nichts geändert.
connections_msg.error.weg = Unbekannter RAG-Weg; nichts geändert.
connections_msg.error.sammlung = Sammlung ungültig (1 bis 200 Zeichen, eine Zeile, höchstens 100); nichts geändert.
connections_msg.error.reindex = Der Index ließ sich nicht neu aufbauen; den Grund nennt „Status der Wege“.
connections_msg.error.tabelle = Die Tabelle muss ein Name aus Buchstaben, Ziffern und _ sein (wahlweise schema.tabelle); nichts geändert.
connections_msg.error.spalte = Ein Spaltenname ist ungültig (Buchstaben, Ziffern und _, nicht mit einer Ziffer beginnend; nur Stelle, Titel, Adresse und Modell dürfen leer sein); nichts geändert.
connections_msg.error.titel = Der Titel muss 1 bis 200 Zeichen in einer Zeile haben; nichts geändert.
connections_msg.error.adresse = Die Adresse muss mit http:// oder https:// beginnen, einen Rechner nennen und darf keine Leerzeichen enthalten; nichts geändert.
connections_msg.error.notiz = Die Notiz ist zu lang (höchstens 4000 Zeichen); nichts geändert.
connections_msg.error.quelle = Diese Quelle gibt es nicht (mehr); bitte neu laden.
connections_msg.error.position = Ungültige Stelle; nichts verschoben.
connections_msg.error.methode = Unbekannte Aktion; nichts geändert.
connections_msg.error.kurztest_modell = Für den Kurztest bitte ein Modell wählen (eine Zeile, höchstens 200 Zeichen); nichts gesendet.
connections_msg.error.intern = Interner Fehler; nichts geändert. Bitte erneut versuchen.
connections_msg.error.bibel_ordner = Modulordner: je Zeile ein absoluter Pfad (oder ~/…), höchstens 20 Ordner zu je 1000 Zeichen; nichts geändert.
connections_msg.error.bibel_modul = Dieses Bibelmodul gibt es in keinem Modulordner, oder Baruch kann es nicht lesen; nichts geändert.
connections_msg.error.bibel_schluessel_falsch = Der Schlüssel passt nicht zum Modul; nichts gespeichert.
connections_msg.error.bibel_schluessel = Schlüssel: 1 bis 255 Zeichen in einer Zeile, nur für ein verschlüsseltes Modul aus den Modulordnern; nichts gespeichert.
connections_msg.error.other = Das hat nicht geklappt (unbekannter Fehlercode).
connections_msg.model_ok.modell_gespeichert = Die Modell-Verbindung ist gespeichert.
connections_msg.model_ok.modell_angelegt = Die Modell-Verbindung ist angelegt.
connections_msg.model_ok.modell_geloescht = Die Modell-Verbindung ist gelöscht; Personas, die sie nutzten, nutzen wieder Eliezer.
connections_msg.model_ok.modell_schluessel = Der Schlüssel ist im Schlüsselbund hinterlegt und gilt für die jetzige Adresse.
connections_msg.model_ok.modell_schluessel_entfernt = Der Schlüssel ist aus dem Schlüsselbund entfernt.
connections_msg.model_ok.modell_geprueft = Die Modell-Verbindung ist geprüft; das Ergebnis steht bei ihr.
connections_msg.model_ok.modell_kurztest = Der Kurztest ist gelaufen; das Ergebnis steht bei der Verbindung.
connections_msg.model_error.name = Der Name fehlt, ist zu lang oder gibt es schon; nichts geändert.
connections_msg.model_error.art = Unbekannte Art (Eliezer, OpenAI-kompatibel oder Ollama); nichts geändert.
connections_msg.model_error.adresse = Die Adresse ist ungültig (http:// oder https://, ohne Benutzer und Schlüssel); nichts geändert.
connections_msg.model_error.verbindung = Diese Modell-Verbindung gibt es nicht (mehr); bitte neu laden.
connections_msg.model_error.eliezer = Die Verbindung Eliezer lässt sich nicht löschen und bleibt von der Art Eliezer.
connections_msg.model_error.schluessel = Der Schlüssel ist leer, zu lang oder mehrzeilig; nichts geändert.
connections_msg.model_error.schluesselbund = Der Schlüsselbund ist nicht erreichbar oder lehnt ab; nichts geändert.
connections_msg.model_error.bestaetigen = Bitte bestätigen (Kästchen anhaken); nichts geändert.
connections_msg.model_error.modell = Für den Kurztest bitte ein Modell angeben (eine Zeile, höchstens 200 Zeichen); nichts gesendet.
connections_msg.model_error.aktion = Unbekannte Aktion; nichts geändert.
connections_msg.model_error.other = Das hat nicht geklappt; nichts geändert.

# MultiGPT as a RAG path and pgvector (src/connections/multigpt.rs, src/connections/pgvector.rs; Round 39c, task
# reste_1). The check messages above (check.timeout, check.no_connection, check.pg_*) are shared with them.
connections_msg.mcp.not_utf8 = Antwort ist kein UTF-8-Text.
connections_msg.mcp.not_json = Antwort ist weder JSON noch text/event-stream.
connections_msg.mcp.no_message = ohne Meldung
connections_msg.mcp.remote_error = Fehler der Gegenstelle: { $message }
connections_msg.mcp.no_result = Antwort ohne result
connections_msg.mcp.no_answer_id = keine JSON-RPC-Antwort mit id { $id } gefunden
connections_msg.mcp.incomplete = Keine vollständige Antwort von { $url } innerhalb von { $seconds } s.
connections_msg.mcp.too_big = Antwort von { $url } ist größer als { $mib } MiB; abgebrochen.
connections_msg.mcp.redirect = { $url } leitet weiter (HTTP { $status }); Baruch folgt Weiterleitungen nicht, damit das Token nicht an eine andere Adresse geht. Die Zieladresse unter Einstellungen → Verbindungen eintragen.
connections_msg.mcp.http_status = { $url } antwortet mit HTTP { $status }.
connections_msg.mcp.tool_error = { $tool } meldet einen Fehler: { $text }
connections_msg.mcp.no_hit_list = { $tool }: keine Trefferliste in der Antwort (results, hits, documents …).
connections_msg.mcp.hits_unreadable = { $tool }: { $count } Treffer ohne Dokument und Titel; Format nicht lesbar.
connections_msg.mcp.no_query_param = { $tool } nennt im inputSchema keinen Parameter für die Anfrage ({ $keys }).
connections_msg.mcp.no_collection_param = { $tool } nennt im inputSchema keinen Parameter für Sammlungen ({ $keys }); ohne ihn suchte MultiGPT in allen Sammlungen, darum fragt Baruch nicht.
connections_msg.mcp.unknown_param = { $tool } verlangt den Parameter „{ $name }“, den Baruch nicht kennt.
connections_msg.mcp.settings = Einstellungen der Verbindung MultiGPT: { $detail }
connections_msg.mcp.no_address = Keine Adresse für MultiGPT eingetragen (Einstellungen → Verbindungen).
connections_msg.mcp.keyring = Schlüsselbund: { $detail }
connections_msg.mcp.foreign_token = Das Token baruch/multigpt ist für eine andere Adresse hinterlegt; Baruch schickt es nicht an die jetzige. Für die neue Adresse das Token neu eintragen (Einstellungen → Verbindungen).
connections_msg.mcp.session_closed = Sitzung zu MultiGPT ist schon beendet.
connections_msg.mcp.list_not_document = { $tool }: Antwort ist eine Liste, kein Dokument.
connections_msg.mcp.no_text_keys = { $tool }: kein Text in der Antwort ({ $keys }).
connections_msg.mcp.no_text = { $tool }: kein Text in der Antwort.
connections_msg.mcp.no_part = Teil { $part } gibt es nicht; das Dokument hat { $count } Teile.
connections_msg.mcp.no_page = Teil { $part } gibt es nicht; das Dokument hat { $count } Seiten.
connections_msg.mcp.part_from_one = part: Teile zählen ab 1.
connections_msg.mcp.fulltext_too_big = { $tool }: Volltext ist größer als { $mib } MiB; abgebrochen.
connections_msg.mcp.too_many_pages = { $tool }: mehr als { $max } Seiten; Volltext abgebrochen.
connections_msg.mcp.no_collection = Keine Sammlung gewählt (Einstellungen → Verbindungen).
connections_msg.mcp.tool_missing_list = MultiGPT bietet das Werkzeug { $tool } nicht an (tools/list).
connections_msg.mcp.tool_missing = MultiGPT bietet das Werkzeug { $tool } nicht an.
connections_msg.mcp.answer_unreadable = { $tool }: Antwort nicht lesbar (weder structuredContent noch JSON in content).
connections_msg.mcp.no_document_param = { $tool } nennt im inputSchema keinen Parameter für das Dokument.
connections_msg.mcp.no_document_param_keys = { $tool } nennt im inputSchema keinen Parameter für das Dokument ({ $keys }).
connections_msg.pg.table_name = Tabelle „{ $table }“ ist kein gültiger Name (tabelle oder schema.tabelle).
connections_msg.pg.column_name = { $field }: „{ $column }“ ist kein gültiger Spaltenname.
connections_msg.pg.no_model = Kein Einbettungsmodell eingetragen (embedding_model).
connections_msg.pg.role_text = Text
connections_msg.pg.role_vector = Vektor
connections_msg.pg.role_collection = Sammlung
connections_msg.pg.role_document = Dokument
connections_msg.pg.role_section = Stelle
connections_msg.pg.role_title = Titel
connections_msg.pg.role_url = Adresse
connections_msg.pg.role_model = Modell
connections_msg.pg.tls_require = Hinweis: TLS steht auf „require“ – die Verbindung ist verschlüsselt, das Zertifikat des Servers prüft Baruch dabei aber nicht. Wer sich im Netz als Server ausgibt, könnte Anmeldung, Anfragen und Treffer, je nach Anmeldeverfahren auch das Passwort, mitlesen. Sicher ist nur „verify-full“.
connections_msg.pg.tls_disable = Achtung: TLS steht auf „disable“ – Anmeldung, Anfragen und Treffer gehen unverschlüsselt über das Netz, je nach Anmeldeverfahren des Servers auch das Passwort. Besser „verify-full“ einstellen.
connections_msg.pg.tls_prefer = Achtung: TLS steht auf „prefer“ – bietet der Server kein TLS an, verbindet Baruch unverschlüsselt und ohne Prüfung des Zertifikats; Anmeldung, Anfragen und Treffer, je nach Anmeldeverfahren auch das Passwort, gingen dann offen über das Netz. Besser „verify-full“ einstellen.
connections_msg.pg.statement_timeout = pgvector { $address }: Abfrage länger als { $seconds } s, abgebrochen (statement_timeout).
connections_msg.pg.no_table_search_path = Tabelle „{ $table }“ gibt es im Suchpfad des Benutzers nicht (oder der Benutzer darf sie nicht lesen).
connections_msg.pg.no_table_schema = Tabelle „{ $table }“ gibt es im Schema { $schema } nicht (oder der Benutzer darf sie nicht lesen).
connections_msg.pg.no_column = Spalte „{ $column }“ ({ $role }) fehlt in Tabelle { $schema }.{ $table }.
connections_msg.pg.column_type = Spalte „{ $column }“ hat den Typ { $type }, nicht vector (pgvector).
connections_msg.pg.no_host_check = Kein Rechner (Host) eingetragen.
connections_msg.pg.mapping_invalid = Zuordnung ungültig: { $detail }
connections_msg.pg.but = Verbindung steht (vector { $vector }), aber: { $detail }
connections_msg.pg.no_dimension_check = Verbindung steht (vector { $vector }), aber die Vektorspalte „{ $column }“ hat keine feste Dimension und noch keine Daten.
connections_msg.pg.ready =
    Verbindung steht, Erweiterung vector { $vector }: { $count ->
        [one] { $count } Sammlung
       *[other] { $count } Sammlungen
    }, { $chunks } Abschnitte, Dimension { $dimension }
connections_msg.pg.model_set = ; eingestellt ist das Modell { $model }.
connections_msg.pg.models_found = ; Modelle in den Daten: { $models }.
connections_msg.pg.strangers = Achtung: Eingestellt ist { $model }; Sammlungen mit { $strangers } lehnt die Suche ab (anderes Modell).
connections_msg.pg.prefixed = pgvector: { $detail }
connections_msg.pg.foreign_model = pgvector: Die gewählten Sammlungen sind mit { $models } eingebettet, eingestellt ist { $model }; Treffer wären wertlos.
connections_msg.pg.no_dimension = pgvector: Die Vektorspalte „{ $column }“ hat keine feste Dimension und noch keine Daten.
connections_msg.pg.query_model = pgvector: Die Anfrage wurde mit { $query } eingebettet, die Daten mit { $model }; Treffer wären wertlos.
connections_msg.pg.not_one_vector = pgvector: Der Einbetter lieferte nicht genau einen Vektor.
connections_msg.pg.query_dimension = pgvector: Die Anfrage hat { $got } Dimensionen, die Spalte „{ $column }“ { $dimension }; anderes Einbettungsmodell?
connections_msg.pg.not_finite = pgvector: Der Einbetter lieferte keine endlichen Zahlen.
connections_msg.pg.no_host = pgvector: Kein Rechner eingetragen (Einstellungen → Verbindungen).
connections_msg.pg.settings = Einstellungen von pgvector: { $detail }
connections_msg.pg.foreign_password = pgvector: Das Passwort baruch/pgvector ist für ein anderes Ziel hinterlegt; Baruch schickt es nicht an die jetzige Adresse. Für sie das Passwort neu eintragen (Einstellungen → Verbindungen).
