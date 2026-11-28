# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of ideas and the idea map (Round 39a, task messages_1). Base catalog in English; locales/de/ideas_msg.ftl has the same keys in
# the same order with the German wording.

# Checks of the RPC methods idea.* (ideas.rs).
ideas_msg.param.number = { $name }: Zahl erwartet
ideas_msg.param.id_missing = id fehlt (Einfall)
ideas_msg.param.text = { $name }: Text erwartet
ideas_msg.param.text_list = { $name }: Liste von Texten erwartet
ideas_msg.param.list_or_text = { $name }: Liste oder Text „a, b; c“ erwartet
ideas_msg.param.status_all = status: open, done oder all erwartet
ideas_msg.param.status = status: open oder done erwartet
ideas_msg.param.not_negative = { $name }: nicht negativ
ideas_msg.param.flag = { $name }: true oder false erwartet
ideas_msg.param.place = place: before, after oder into erwartet
ideas_msg.param.confirm_delete = Löschen bitte bestätigen (confirm).
ideas_msg.process.no_work = Ein Einfall ohne Werk lässt sich so nicht verarbeiten; erst einem Werk zuordnen.
ideas_msg.process.no_section = Welcher Abschnitt? key angeben oder den Einfall einem Abschnitt zuordnen.
ideas_msg.process.as_invalid = as: outline, post, note oder persona erwartet
ideas_msg.process.as_missing = as fehlt (outline, post, note oder persona)
ideas_msg.process.deleted = Einfall { $id } ist gelöscht; erst wiederherstellen.
ideas_msg.process.no_lector = Keine Persona ist aktiv (kein Lektor).
ideas_msg.create.words_missing = words fehlt (mindestens ein Schlagwort oder ein Titel)
ideas_msg.create.cloud = cloud: in die Wolke geht nur ohne parent und key
ideas_msg.search.query_missing = Suchbegriff fehlt.
ideas_msg.search.rag_one_work = Bedeutungssuche nur innerhalb eines Werks (work: Werk-id).
ideas_msg.search.lancedb_not_asked = LanceDB wurde nicht gefragt
ideas_msg.search.rag_unavailable = Bedeutungssuche nicht verfügbar: { $reason }.
ideas_msg.search.mode = mode: fts oder rag erwartet
ideas_msg.search.rag_note = Bedeutungssuche nur innerhalb eines Werks; gezeigt wird die Wortlaut-Suche (FTS).

# Pages „Einfälle“.
ideas_msg.page.template_error = Fehler: Vorlage { $name }: { $error }
ideas_msg.page.error = Fehler: { $error }
ideas_msg.page.unknown_work = unbekanntes Werk
ideas_msg.page.unknown_idea = Einfall { $id } gibt es nicht.
ideas_msg.page.unknown_action = unbekannte Aktion
ideas_msg.notice.deleted = Der Einfall ist gelöscht; unter „Gelöschte“ lässt er sich wiederherstellen.
ideas_msg.notice.saved = Gespeichert.
ideas_msg.notice.created = Der Einfall ist festgehalten.
ideas_msg.notice.restored = Der Einfall ist wiederhergestellt.
ideas_msg.notice.processed = Der Einfall ist verarbeitet.
ideas_msg.form.confirm_delete = Löschen bitte bestätigen.

# Idea map (idea_map/mod.rs).
ideas_msg.map.work_missing = work fehlt (Werk)
ideas_msg.map.from_missing = from fehlt (Einfall, von dem die Verbindung ausgeht)
ideas_msg.map.to_missing = to fehlt (Einfall, zu dem sie führt)
ideas_msg.map.idea_missing = idea fehlt (Einfall)
ideas_msg.map.missing = { $name } fehlt
ideas_msg.map.status = status: „open“, „done“ oder „all“ erwartet, nicht „{ $word }“
ideas_msg.map.origin = origin: „human“, „persona“ oder „mcp“ erwartet, nicht „{ $other }“
ideas_msg.map.near_limit = near_limit: 1 bis 5, nicht { $n }
ideas_msg.map.near_threshold = near_threshold: 0 bis 1, nicht { $t }
ideas_msg.map.kind_missing = kind fehlt (Art der Verbindung)
ideas_msg.map.kind = kind: „leads_to“, „belongs_to“, „example_of“ oder „contradicts“ erwartet, nicht „{ $word }“
ideas_msg.map.object = Parameter als Objekt erwartet
ideas_msg.map.links_scope = idea.links: genau eins von work und idea angeben
ideas_msg.near.off = Nähe ausgeschaltet.
ideas_msg.near.capped = Die Karte zeigt { $count } Einfälle; die Nähe ist nur für die ersten { $max } (nach Nummer) berechnet.
ideas_msg.near.ready =
    Nähe aus gespeicherten Einbettungen ({ $have } von { $count } { $count ->
        [one] Einfall
       *[other] Einfällen
    } eingebettet) und gleichen Schlagwörtern; ab { $threshold }, höchstens { $limit } je Einfall.
ideas_msg.near.failed = Die Nähe ließ sich nicht berechnen ({ $error }).
ideas_msg.import.no_node = Die Datei enthält keinen Knoten; nichts angelegt.
ideas_msg.import.too_many = Die Mindmap hat { $count } Knoten; einlesen lassen sich höchstens { $max }. Nichts angelegt.
ideas_msg.import.too_deep = Die Mindmap ist { $deepest } Ebenen tief; einlesen lassen sich höchstens { $max }. Nichts angelegt.
ideas_msg.import.nothing_created = Nichts angelegt.

# Notices of the idea map page (idea_map/bild/mod.rs).
ideas_msg.map_notice.reference = Bezug gesetzt.
ideas_msg.map_notice.linked = Verbunden.
ideas_msg.map_notice.link_exists = Diese Verbindung gibt es schon; nichts geändert.
ideas_msg.map_notice.unlinked = Verbindung gelöst.
ideas_msg.map_notice.link_restored = Verbindung wiederhergestellt.
ideas_msg.map_notice.positions_reset = Gespeicherte Lagen verworfen (in der Mindmap ohne Wirkung: Der Baum ordnet sich selbst).
ideas_msg.map_notice.created = Einfall angelegt.
ideas_msg.map_notice.renamed = Umbenannt.
ideas_msg.map_notice.moved = Einfall umgehängt.
ideas_msg.map_notice.to_cloud = In die Wolke gelegt (mit seinen Unterknoten).
ideas_msg.map_notice.place_unchanged = Der Einfall steht schon dort; nichts geändert.
ideas_msg.map_notice.deleted = Einfall gelöscht samt Unterknoten (wiederherstellbar in der Gliederungsansicht).
ideas_msg.map_notice.restored = Einfall wiederhergestellt.
ideas_msg.map_notice.imported = Datei eingelesen: Die Knoten sind als Einfälle angelegt.
ideas_msg.map_notice.named = Der Assistent hat Namen vorgeschlagen (unten bei „Gruppen“). Schlagwort wird ein Name erst, wenn du ihn übernimmst.
ideas_msg.map_notice.not_named = Der Assistent hat keinen brauchbaren Namen geliefert; es gibt keinen neuen Vorschlag.
ideas_msg.map_notice.no_groups = Keine Gruppe: Es gibt keine nahen Einfälle (mindestens zwei je Gruppe).
ideas_msg.map_notice.name_adopted = Name übernommen: Er ist jetzt Schlagwort an den Einfällen der Gruppe.
ideas_msg.map_notice.rejected = Vorschlag verworfen.

# Mind map files and outline view (idea_map/files/*).
ideas_msg.map.idea_elsewhere = Einfall { $idea } gehört nicht zu diesem Werk.
ideas_msg.map.target = ziel: „wolke“, „w“ (Hauptthema am Werk), „i:{ $lt }Einfall{ $gt }“ oder „n:{ $lt }Kapitel{ $gt }“ erwartet
ideas_msg.map.unknown_action = Unbekannte Aktion „{ $action }“ auf der Mindmap.
ideas_msg.upload.not_an_idea = Der gewählte Knoten ist kein Einfall.
ideas_msg.upload.not_multipart = Das Formular kam nicht als Datei-Upload an ({ $detail }).
ideas_msg.upload.form_unreadable = Das Formular ließ sich nicht lesen ({ $detail }).
ideas_msg.upload.file_unreadable = Die Datei ließ sich nicht lesen ({ $detail }).
ideas_msg.upload.choose = Bitte eine Datei wählen (.mm oder .xmind).
ideas_msg.upload.unreadable = Die Datei ließ sich nicht lesen.
ideas_msg.upload.again = Bitte die Datei noch einmal einlesen.
ideas_msg.upload.preview_broken = Die Vorschau ist beschädigt oder fehlt. { $again }
ideas_msg.upload.preview_too_large = Die Vorschau ist zu groß geworden. { $again }
ideas_msg.upload.form_broken = Das Formular ließ sich nicht lesen. { $again }
ideas_msg.import.file_too_many = Die Datei hat mehr als { $max } Knoten; so große Mindmaps liest Baruch nicht ein.
ideas_msg.import.file_too_deep = Die Mindmap ist tiefer als { $max } Ebenen; so tiefe Mindmaps liest Baruch nicht ein.
ideas_msg.import.empty = Die Datei ist leer.
ideas_msg.import.wrong_format = Die Datei ist weder eine FreeMind-/Freeplane-Datei (.mm) noch eine XMind-Datei (.xmind).
ideas_msg.import.too_large = Die Datei ist größer als { $mib } MiB; so große Mindmaps liest Baruch nicht ein.
ideas_msg.import.links_lost = { $count } Beziehung(en) nicht übernommen: Ein Ende fehlt in der Datei, ist das Hauptthema oder beide Enden sind derselbe Knoten.
ideas_msg.import.too_much_text = Die Mindmap trägt mehr als { $mib } MiB Text; so viel liest Baruch nicht ein.
ideas_msg.import.only_root = Die Datei enthält außer dem Hauptthema keine Knoten; es gibt nichts einzulesen.
ideas_msg.import.cut_titles =
    { $count ->
        [one] { $count } Titel war länger als { $max } Zeichen und wurde gekürzt.
       *[other] { $count } Titel waren länger als { $max } Zeichen und wurden gekürzt.
    }
ideas_msg.import.cut_notes =
    { $count ->
        [one] { $count } Notiz war länger als { $max } Zeichen und wurde gekürzt.
       *[other] { $count } Notizen waren länger als { $max } Zeichen und wurden gekürzt.
    }
ideas_msg.import.untitled =
    { $count ->
        [one] { $count } Knoten ohne Titel heißt jetzt „{ $untitled }“.
       *[other] { $count } Knoten ohne Titel heißen jetzt „{ $untitled }“.
    }
ideas_msg.import.cut_words = Bei { $count } Knoten wurden Schlagwörter gekürzt (höchstens { $max } je Knoten, je { $chars } Zeichen).
ideas_msg.import.links_dropped = { $count } Beziehung(en) nicht übernommen (ungültig oder zu viele).
ideas_msg.outline_notice.moved = Umgehängt.
ideas_msg.outline_notice.deleted = Einfall gelöscht (samt Unterknoten).
ideas_msg.outline_notice.imported = Mindmap eingelesen.
ideas_msg.outline_notice.cloud = Wolke
ideas_msg.outline_notice.main_topic = { $title } (Hauptthema am Werk)
ideas_msg.map.link_id_missing = id fehlt (Verbindung)

# Picture of the mind map (src/idea_map/bild/mindmap.rs: page fragment and the .svg/.pdf export, both in the language
# of the request; Round 39c, task reste_1).
ideas_msg.mindmap.label = Mindmap: { $title }
ideas_msg.mindmap.cloud_empty = Wolke – leer. Einfälle ohne Platz in der Mindmap liegen hier.
ideas_msg.mindmap.cloud_count = Wolke – { $count } in der Wolke
ideas_msg.mindmap.fits = passt zu Ast „{ $title }“
