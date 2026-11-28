# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of the store (baruch-store) in the language of the user interface (Round 39a, task messages_2).
# The German texts are the store's own wording verbatim: src/store_text.rs reads them as patterns (a `{ $name }`
# matches any text) and formats the matching key in the language of the request. Arguments named `detail` are
# translated again (nested messages). Owned by messages_2.

# Variants of StoreError with a fixed wording.
store_msg.error.db = Datenbank: { $detail }
store_msg.error.migrate = Migration: { $detail }
store_msg.error.io = Datenordner { $path }: { $detail }
store_msg.error.unknown_work = unbekanntes Werk { $work }
store_msg.error.unknown_node = unbekannter Abschnitt { $node }
store_msg.error.unknown_revision = Fassung { $rev } gibt es nicht; vorhanden: { $list }
store_msg.error.unknown_revision_none = Fassung { $rev } gibt es nicht; vorhanden: keine
store_msg.error.empty_post = Leerer Beitrag.
store_msg.error.already_imported = Diese Datei ist schon importiert: Werk „{ $title }“ ({ $slug }).
store_msg.error.import = Import: { $detail }
store_msg.error.duplicate_title = Es gibt schon ein Werk „{ $title }“ ({ $slug }).
store_msg.error.empty_title = Ein Werk braucht einen Titel.
store_msg.error.gone_node = Diesen Gliederungspunkt gibt es nicht mehr (gelöscht oder zusammengeführt); bitte neu laden.
store_msg.error.numbering = Nummerierung: { $detail }
store_msg.error.unknown_asset = Bild „{ $name }“ gibt es in diesem Werk nicht.
store_msg.error.unknown_persona = Persona „{ $handle }“ gibt es nicht.
store_msg.error.unknown_idea = Einfall { $id } gibt es nicht.
store_msg.error.unknown_post = Beitrag { $post } gibt es in Werk { $work } nicht.
store_msg.error.unknown_request = Antrag { $id } gibt es in Werk { $work } nicht.
store_msg.error.exchange = Austausch: { $detail }
store_msg.error.unknown_finding = Befund { $id } gibt es in Werk { $work } nicht.
store_msg.error.typeset = Satz-Einstellungen: { $detail }
store_msg.error.glossary = Glossar/Register: { $detail }
store_msg.error.unknown_glossary_term = Glossarbegriff { $id } gibt es in Werk { $work } nicht.
store_msg.error.unknown_index_proposal = Registervorschlag { $id } gibt es in Werk { $work } nicht.

# Accounts (pub const of Store).
store_msg.accounts.taken = Diesen Namen gibt es schon.
store_msg.accounts.not_first = Es gibt schon ein Konto.
store_msg.accounts.needs_author = Das erste Konto gehört dem Autor (Einrichten).
store_msg.accounts.last_author = Das letzte aktive Autor-Konto lässt sich nicht sperren.
store_msg.accounts.share_role = Freigeben lässt sich nur die Rolle „lektor“.
store_msg.accounts.share_unknown = Unbekanntes Konto: nichts freigegeben.
store_msg.accounts.share_lektor_only = Nur Lektor-Konten bekommen eine Freigabe; der Autor ist Autor in jedem Werk.
store_msg.accounts.ui_language = Sprache der Oberfläche: zwei Kleinbuchstaben (z. B. de oder en).

# Revisions and history.
store_msg.revision.no_current = Der Abschnitt hat keine aktuelle Fassung.
store_msg.revision.undo_current_only = Rückgängig über den Verlauf geht nur auf der aktuellen Fassung; erst auf diese Fassung umschalten.
store_msg.revision.undo_human_only = Rückgängig über den Verlauf geht nur auf einer Fassung des Menschen (nicht auf Vorschlägen von Personas).
store_msg.revision.no_such_step = Diesen Schritt gibt es im Verlauf dieser Fassung nicht.
store_msg.revision.already_at_step = Der Text steht schon auf diesem Stand; nichts zurückzunehmen.
store_msg.revision.step_other_heading = Der Stand vor diesem Schritt hat eine andere Überschrift; bitte als neue Fassung speichern.
store_msg.revision.policy_percent = Größte Änderung: 1 bis 100 Prozent.
store_msg.revision.policy_minutes = Neue Fassung nach: 0 bis 1440 Minuten (0 = keine Zeitgrenze).
store_msg.revision.already_current = Diese Fassung ist schon aktuell.
store_msg.revision.unchanged = Die Fassung ist unverändert.
store_msg.revision.current_not_deletable = Die aktuelle Fassung lässt sich nicht löschen; erst auf eine andere umschalten.
store_msg.revision.needs_text = Eine Fassung braucht Text.
store_msg.revision.only_proposals_accept = Nur Vorschläge lassen sich übernehmen.
store_msg.revision.only_proposals_reject = Nur Vorschläge lassen sich verwerfen.
store_msg.revision.proposals_need_current = Vorschläge brauchen eine aktuelle Revision.
store_msg.revision.fold_above_current = Über der aktuellen Fassung lässt sich nicht ausblenden; die Marke höchstens an die aktuelle Fassung setzen.
store_msg.revision.stale_amend = Veraltet: Fassung { $rev } steht inzwischen auf Stand { $seq } (geladen war Stand { $expected }); bitte neu laden und die Änderung dort eintragen.
store_msg.revision.stale = Veraltet: { $detail }

# Images and image requests.
store_msg.asset.unchanged = Das Bild ist unverändert.
store_msg.asset.request_current_rev = Die aktuelle Fassung lässt sich nicht löschen; ohne rev wird das ganze Bild beantragt.
store_msg.asset.layout_unchanged = Die Umschlag-Anordnung ist so schon gespeichert; nichts zu ändern.
store_msg.asset.human_direct = Der Mensch ändert Bilder direkt; Anträge stellen nur Assistenten.
store_msg.asset.request_open = Diesen Antrag gibt es schon (offen); der Mensch entscheidet darüber.
store_msg.asset.request_decided = Über diesen Antrag ist schon entschieden.
store_msg.asset.name_taken = Ein Bild mit diesem Namen gibt es in diesem Werk schon.
store_msg.asset.no_collision = Unter diesem Pfad gibt es keine Kollision (mehr).
store_msg.asset.paths_unchanged = Die Pfade sind unverändert.
store_msg.asset.changed_meanwhile = Das Bild „{ $name }“ hat inzwischen einen anderen Inhalt (expected_sha256 passt nicht); bitte neu lesen.
store_msg.asset.new_too_big = Der neue Inhalt ist größer als 8 MiB.
store_msg.asset.new_unsupported = Der neue Inhalt ist kein unterstütztes Bild (PNG, JPEG, GIF, WebP, SVG).
store_msg.asset.svg_needs_text = encoding „svg“ verlangt SVG-Text.
store_msg.asset.mime_mismatch = mime „{ $mime }“ passt nicht zum Inhalt ({ $found }).
store_msg.asset.format_change = Der neue Inhalt ist { $found }, „{ $name }“ ist { $current }; ein anderes Format nur mit rename: true (das Bild bekommt beim Bestätigen die neue Endung und behält seine Pfade) oder als neues Bild.
store_msg.asset.unknown_rev_named = Fassung { $rev } von „{ $name }“ gibt es nicht; vorhanden: { $list }
store_msg.asset.unknown_rev = Bild { $name }: Fassung { $rev } gibt es nicht; vorhanden: { $list }
store_msg.asset.blocks_expected = blocks: mindestens ein Block der Anordnung erwartet.
store_msg.asset.layout_changed = Die Umschlag-Anordnung hat sich inzwischen geändert (expected_layout passt nicht); bitte neu lesen (cover_layout).
store_msg.asset.bad_base64 = Kein gültiges Base64 ({ $detail }).
store_msg.asset.path_empty = Ungültiger Pfad „{ $path }“: leer.
store_msg.asset.path_too_long = Ungültiger Pfad „{ $path }“: zu lang.
store_msg.asset.path_separator = Ungültiger Pfad „{ $path }“: nur Ordner und Dateiname mit '/' getrennt.
store_msg.asset.path_dotdot = Ungültiger Pfad „{ $path }“: '..' ist nicht erlaubt.
store_msg.asset.path_no_file = Ungültiger Pfad „{ $path }“: kein Dateiname.
store_msg.asset.too_big = „{ $name }“ ist größer als 8 MiB.
store_msg.asset.unsupported = „{ $name }“ ist kein unterstütztes Bild (PNG, JPEG, GIF, WebP, SVG).
store_msg.asset.path_taken = Der Pfad „{ $path }“ gehört schon zum Bild { $other }.
store_msg.asset.bad_name = Ungültiger Bildname „{ $name }“: nur a–z, 0–9, '.', '_' und '-', höchstens 80 Zeichen, mit Endung (.png, .jpg, .jpeg, .gif, .webp, .svg).
store_msg.asset.extension_mismatch = Die Endung von „{ $name }“ passt nicht zum Typ { $mime }.
store_msg.asset.replace_mismatch = „{ $file }“ ist { $mime }, das passt nicht zur Endung von { $name }; mit „Endung anpassen“ (rename) bekommt das Bild die neue Endung und behält seine Pfade, sonst als neues Bild hinzufügen.
store_msg.asset.collision_paths = Das Bild „{ $asset }“ hat keinen der Pfade von „{ $wanted }“; zur Wahl stehen: { $paths }.
store_msg.asset.one_path = Ein Bild im Text braucht genau einen Pfad.

# Book details and cover print.
store_msg.book.year = Jahr: leer lassen oder vier Ziffern (z. B. 2026).
store_msg.book.paper = Papierdicke: mehr als 0 und höchstens 1 mm je Seite.
store_msg.book.bleed = Beschnitt: 0 bis 10 mm.
store_msg.book.field_too_long = { $field }: höchstens { $max } Zeichen.
store_msg.book.field_one_line = { $field }: bitte in einer Zeile.

# Connections, sources, model connections, collections.
store_msg.connections.unknown = Unbekannte Verbindung (eliezer, multigpt, pgvector oder lancedb).
store_msg.connections.unknown_way = Unbekannter RAG-Weg (lancedb, pgvector oder multigpt).
store_msg.connections.source_title = Titel einer Quelle: 1 bis 200 Zeichen in einer Zeile.
store_msg.connections.source_address = Adresse einer Quelle: http:// oder https://, höchstens 2000 Zeichen.
store_msg.connections.source_gone = Diese Quelle gibt es nicht (mehr).
store_msg.connections.config_object = Einstellungen einer Verbindung sind ein JSON-Objekt.
store_msg.connections.model_gone = Diese Modell-Verbindung gibt es nicht (mehr).
store_msg.connections.model_name = Name einer Modell-Verbindung: 1 bis 80 Zeichen in einer Zeile.
store_msg.connections.model_kind = Art einer Modell-Verbindung: eliezer, openai oder ollama.
store_msg.connections.model_address = Adresse einer Modell-Verbindung: http:// oder https://, ohne Leerraum.
store_msg.connections.model_taken = Eine Modell-Verbindung mit diesem Namen gibt es schon.
store_msg.connections.eliezer_kind = Die Verbindung Eliezer bleibt von der Art eliezer.
store_msg.connections.eliezer_delete = Die Verbindung Eliezer lässt sich nicht löschen.
store_msg.connections.lancedb_collections = LanceDB hat keine Sammlungen (nur pgvector und MultiGPT).
store_msg.connections.collection_name = Sammlung: 1 bis 200 Zeichen in einer Zeile.
store_msg.connections.collections_max = Höchstens 100 Sammlungen je Werk und Weg.
store_msg.connections.fetch_max = Höchstens 100 Sammlungen zum Holen.
store_msg.connections.sword_name = Modulname: höchstens 200 Zeichen in einer Zeile.
store_msg.connections.sword_max = Höchstens 20 Kommentare bzw. Lexika je Werk.
store_msg.connections.persona_unknown = Diese Persona gibt es nicht.

# Exchange files and instance.
store_msg.exchange.work_exists = Werk schon vorhanden: Diese Datei gehört zu einem Werk, das es hier gibt; bitte abgleichen statt neu einlesen.
store_msg.exchange.work_missing = Abgleichen geht nicht: Das Werk dieser Datei gibt es hier nicht; bitte als neues Werk einlesen.
store_msg.exchange.request_part = Antrag mit Kennung: kein Teil unter den neuen Punkten.
store_msg.exchange.request_current = Antrag mit Kennung: genau eine aktuelle Fassung erwartet.
store_msg.exchange.glossary_max = Das Glossar hätte { $count } Begriffe; höchstens { $max } je Werk (Vorschläge mitgezählt). Nichts eingelesen.
store_msg.exchange.register_title = Register { $slug }: Titel hat { $chars } Zeichen (höchstens { $max }). Nichts eingelesen.
store_msg.exchange.uuid_elsewhere = { $what }: die Kennung { $uuid } gibt es hier schon in einem anderen Werk; nichts übernommen
store_msg.exchange.unknown_scheme = unbekanntes Gliederungsschema „{ $scheme }“
store_msg.exchange.db_mismatch = Datenbank passt nicht zum Austauschformat: { $detail }
store_msg.exchange.instance_name_long = Der Name der Instanz darf höchstens { $max } Zeichen haben (jetzt { $chars }); nichts gespeichert.
store_msg.exchange.instance_name_control = Der Name der Instanz darf keine Steuerzeichen (Zeilenumbruch, Tab …) enthalten; nichts gespeichert.
store_msg.exchange.instance_uuid = Instanzkennung fehlt (Migration 0041)

# Glossary.
store_msg.glossary.no_text = Dieser Abschnitt hat noch keinen Text.

# MCP tokens.
store_msg.tokens.name_needed = Ein Token braucht einen Namen (z. B. den des Rechners).
store_msg.tokens.name_long = Der Name eines Tokens hat höchstens 64 Zeichen.
store_msg.tokens.name_control = Der Name eines Tokens hat keine Zeilenumbrüche oder Steuerzeichen.
store_msg.tokens.no_random = Kein Zufall vom Betriebssystem; Token nicht angelegt.
store_msg.tokens.name_valid = Es gibt schon einen gültigen Token mit diesem Namen; anderen Namen wählen oder den alten widerrufen.
store_msg.tokens.unknown = Diesen Token gibt es nicht.
store_msg.tokens.handle_reserved = Namen, deren Autor mit „lokal-“ beginnt, sind Clients ohne Token vorbehalten; anderen Namen wählen.
store_msg.tokens.handle_taken = Dieser Name ergibt denselben Autor (@mcp:…) wie ein vorhandener Token – Groß/klein, Umlaute, Sonderzeichen und alles nach 32 Zeichen zählen nicht. Anderen Namen wählen.
store_msg.tokens.handle_revoked = Dieser Name ergibt denselben Autor (@mcp:…) wie ein widerrufener Token; dessen Beiträge tragen ihn weiter. Anderen Namen wählen.

# Outline.
store_msg.outline.into_own_subtree = Ein Gliederungspunkt lässt sich nicht in seinen eigenen Teilbaum verschieben.
store_msg.outline.already_there = Der Gliederungspunkt steht schon an dieser Stelle.
store_msg.outline.parts_level = Teile lassen sich nicht umstufen, und nichts wird zum Teil.
store_msg.outline.too_deep = Tiefer als Ebene 3 (Unterabschnitt) geht es nicht.
store_msg.outline.same_level = Der Gliederungspunkt hat schon diese Ebene.
store_msg.outline.children_too_deep = Mit den Unterpunkten ginge das tiefer als Ebene 3 (Unterabschnitt); erst Unterpunkte höher stufen.
store_msg.outline.same_title = Der Gliederungspunkt heißt schon so.
store_msg.outline.no_items = Keine Gliederungspunkte angegeben (title oder items).
store_msg.outline.max_items = Höchstens 100 Gliederungspunkte in einem Aufruf.
store_msg.outline.key_missing = Bezugspunkt (key) fehlt: Ohne ihn lässt sich nur der erste Punkt eines leeren Werks anlegen.
store_msg.outline.first_level = Der erste neue Punkt steht auf der Ebene des Orts (vor/nach: Ebene des Bezugspunkts, unter: eine tiefer).
store_msg.outline.none_higher = Kein neuer Punkt steht höher als der erste; sonst würden vorhandene Punkte zu seinen Unterpunkten.
store_msg.outline.parts_counted = Teile werden immer gezählt; „ohne Nummer“ gibt es nur für Kapitel und Abschnitte.
store_msg.outline.has_number = Der Gliederungspunkt hat schon eine Nummer.
store_msg.outline.has_no_number = Der Gliederungspunkt ist schon ohne Nummer.
store_msg.outline.has_children = Der Gliederungspunkt hat Unterpunkte; erst verschieben oder löschen.
store_msg.outline.last_item = Der letzte Gliederungspunkt lässt sich nicht löschen; ein Werk braucht mindestens ein Kapitel.
store_msg.outline.confirm_merge = Der Text hat keine Überschrift mehr; zum Zusammenführen mit dem vorigen Gliederungspunkt bestätigen.
store_msg.outline.confirm_split = Der Text enthält neue Überschriften; zum Teilen bestätigen.
store_msg.outline.numbering_same = Die Nummerierung ist schon so eingestellt.
store_msg.outline.part_not_into = Ein Teil steht immer auf oberster Ebene; „darunter“ (into) geht mit Ebene 0 nicht.
store_msg.outline.part_place = Ein Teil entsteht nur vor oder hinter einem Teil oder Kapitel, nicht neben einem Abschnitt.
store_msg.outline.lead_text = Vor der ersten Überschrift steht Text; die Überschrift gehört an den Anfang des Abschnitts.
store_msg.outline.merge_first = Der erste Gliederungspunkt lässt sich nicht zusammenführen.
store_msg.outline.merge_part = Davor steht ein Teil; zusammenführen geht nur mit einem Kapitel oder Abschnitt.
store_msg.outline.to_part = Die erste Überschrift ist eine Teil-Markierung; ein Kapitel oder Abschnitt wird nicht zum Teil (neuen Teil in der Gliederung anlegen).
store_msg.outline.part_hint = Ein Teiltext mit Markierung beginnt mit genau dieser einen Markierung (davor nur Leerraum) und enthält keine weitere Überschrift.
store_msg.outline.switch_hint = Teilen, Zusammenführen und Text vor der Überschrift gehen über „Speichern“ im Editor.
store_msg.outline.with_switch_hint = { $detail } Teilen, Zusammenführen und Text vor der Überschrift gehen über „Speichern“ im Editor.
store_msg.outline.switch_no_heading = Die Fassung hat keine Überschrift; die Gliederung wüsste nicht, wohin sie gehört. Teilen, Zusammenführen und Text vor der Überschrift gehen über „Speichern“ im Editor.
store_msg.outline.switch_more_headings = Die Fassung enthält weitere Überschriften ({ $titles }); Umschalten teilt keinen Abschnitt. Teilen, Zusammenführen und Text vor der Überschrift gehen über „Speichern“ im Editor.
store_msg.outline.proposal_refused = Vorschlag abgelehnt: { $detail }
store_msg.outline.not_executable = Nicht mehr ausführbar: { $detail }
store_msg.outline.title_empty = Ein Gliederungspunkt braucht einen Titel.
store_msg.outline.title_newline = Der Titel darf keinen Zeilenumbruch enthalten.
store_msg.outline.title_long = Der Titel ist zu lang (höchstens { $max } Zeichen, hier { $count }).
store_msg.outline.item_n = Punkt { $n }: { $detail }
store_msg.outline.unknown_scheme = unbekanntes Gliederungsschema „{ $scheme }“; bekannt: { $known }
store_msg.outline.idea_other_work = Einfall { $idea } gehört nicht zu diesem Werk.

# Personas.
store_msg.personas.prompt_empty = Der Rollen-Prompt darf nicht leer sein (Vorgabe: zurücksetzen).
store_msg.personas.prompt_long = Der Rollen-Prompt darf höchstens 8000 Zeichen haben.
store_msg.personas.color = Farbe als #rrggbb angeben.

# Posts, threads and their editing.
store_msg.posts.already_here = Der Beitrag steht schon in diesem Abschnitt.
store_msg.posts.unknown_thread = Diesen Faden gibt es in diesem Werk nicht.
store_msg.posts.edit_empty = Der Beitrag darf nicht leer sein; nichts geändert.
store_msg.posts.edit_long = Ein Beitrag darf höchstens { $max } Zeichen haben; nichts geändert.
store_msg.posts.edit_unchanged = Der Text ist unverändert; nichts geändert.
store_msg.posts.basis_start = Die geprüfte Fassung beginnt bei 1.
store_msg.posts.basis_long = Die Angabe der geprüften Fassung ist zu lang (höchstens { $max } Zeichen).
store_msg.posts.relay_from = „Relay von“ braucht 1 bis { $max } Zeichen.

# Findings.
store_msg.findings.too_long_body = Der Text des Befunds ist zu lang (höchstens { $max } Zeichen).
store_msg.findings.too_long_quote = Das Zitat ist zu lang (höchstens { $max } Zeichen).
store_msg.findings.too_long_check = Der Prüfvermerk ist zu lang (höchstens { $max } Zeichen).
store_msg.findings.too_long_solution_quote = Das Zitat einer Lösung ist zu lang (höchstens { $max } Zeichen).
store_msg.findings.too_long_solution = Eine Lösung ist zu lang (höchstens { $max } Zeichen).
store_msg.findings.too_long_note = Die Notiz zum Vorschlag ist zu lang (höchstens { $max } Zeichen).
store_msg.findings.too_long_reason = Die Begründung ist zu lang (höchstens { $max } Zeichen).
store_msg.findings.needs_text = Ein Befund braucht einen Text.
store_msg.findings.persona_length = Die Persona eines Befunds braucht 1 bis { $max } Zeichen.
store_msg.findings.one_origin = Ein Befund stammt aus einem Beitrag oder einem Fadenbeitrag, nicht aus beiden.
store_msg.findings.unknown_section = Abschnitt { $key } gibt es in Werk { $work } nicht.
store_msg.findings.unknown_thread_post = Fadenbeitrag { $post } gibt es in Werk { $work } nicht.
store_msg.findings.solution_empty = Eine Lösung braucht ein Zitat mit neuer Formulierung oder einen Schritt.
store_msg.findings.setter_name = Wer den Status setzt, braucht einen Namen.
store_msg.findings.status_same = Befund { $id } hat diesen Status schon ({ $status }); nichts geändert.
store_msg.findings.human_direct = Der Mensch setzt den Status direkt; Vorschläge stellen nur Assistenten.
store_msg.findings.proposal_origin = Ein Vorschlag braucht eine Herkunft (Persona bzw. MCP-Client).
store_msg.findings.human_only = Nur der Mensch kann einen Befund verwerfen oder wieder öffnen; Assistenten schlagen nur „übernommen“, „verschoben“ oder „erledigt ohne Änderung“ vor.
store_msg.findings.status_same_proposal = Befund { $id } hat diesen Status schon ({ $status }); kein Vorschlag nötig.
store_msg.findings.proposal_open = Diesen Vorschlag gibt es zu Befund { $id } schon offen (Eintrag { $twin }); nicht doppelt angelegt.
store_msg.findings.decider_name = Wer entscheidet, braucht einen Namen.
store_msg.findings.not_proposal = Eintrag { $entry } ist kein Vorschlag, sondern eine Statusänderung des Menschen.
store_msg.findings.decided = Über Vorschlag { $entry } ist schon entschieden ({ $decision }).
store_msg.findings.reject_reason = Verwerfen braucht eine Begründung.
store_msg.findings.no_section = Befund { $id } gehört zu keinem Abschnitt; „übernommen“ braucht eine Fassung des Abschnitts.
store_msg.findings.unknown_rev = Fassung { $rev } gibt es im Abschnitt des Befunds { $id } nicht.
store_msg.findings.defer_own = Ein Befund lässt sich nicht in seinen eigenen Abschnitt verschieben.
store_msg.findings.defer_section = Abschnitt { $key } gibt es in Werk { $work } nicht (Ziel des Verschiebens).
store_msg.findings.defer_idea = Einfall { $id } gibt es in Werk { $work } nicht (Ziel des Verschiebens).
store_msg.findings.unknown_proposal = Vorschlag { $entry } gibt es in Werk { $work } nicht.

# Work-wide replacing.
store_msg.replace.query_length = Der Suchtext braucht 1 bis { $max } Zeichen (hier { $chars }).
store_msg.replace.replacement_length = Der Ersatz darf höchstens { $max } Zeichen haben (hier { $chars }).
store_msg.replace.sections_max = Höchstens { $max } Abschnitte je Ersetzen (hier { $count }).
store_msg.replace.twice = Abschnitt { $key } steht zweimal im Ersetzen.
store_msg.replace.undone = Dieses Ersetzen ist schon rückgängig gemacht ({ $when }).
store_msg.replace.unknown = Ersetzen { $batch } gibt es in diesem Werk nicht.

# Speaking time.
store_msg.timing.words = Wörter je Minute: erlaubt sind 60 bis 250.
store_msg.timing.target = Zielzeit: erlaubt sind 1 bis 240 Minuten (oder keine Zielzeit).

# Work template and works.
store_msg.works.template_heading = Die Werkvorlage darf keine Überschrift (= am Zeilenanfang) enthalten; Überschriften gehören zur Gliederung.
store_msg.works.template_unchanged = Die Werkvorlage wäre unverändert; nichts geändert.
store_msg.works.bible_module = Bibelmodul: Name mit 1 bis 100 Zeichen in einer Zeile erwartet; das Werk wurde nicht angelegt.
store_msg.works.title_mismatch = Der eingegebene Titel passt nicht; nichts gelöscht.
store_msg.works.not_archived = Dieses Werk steht nicht im Archiv (unbekannter Ordnername); nichts wiederhergestellt.
store_msg.works.restore_title = Ein lebendes Werk trägt schon denselben Titel; erst jenes umbenennen, dann wiederherstellen. Nichts wiederhergestellt.
store_msg.works.restore_source = Dieselbe Quelle ist schon als lebendes Werk importiert; nichts wiederhergestellt.
store_msg.works.no_sections = Das Werk hat keine Abschnitte.
store_msg.exchange.part_exists = Den Teil gibt es hier inzwischen mit dieser Kennung (oder eine seiner Fassungen); nichts angelegt.
store_msg.outline.move_keeps_level = Verschieben behält die Ebene (Kapitel bleibt Kapitel); erst umstufen (← / →).
