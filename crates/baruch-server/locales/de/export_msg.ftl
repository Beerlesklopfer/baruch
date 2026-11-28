# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of the outputs (PDF, EPUB, typesetting, Word, exchange) (Round 39a, task messages_1). Base catalog in English; locales/de/export_msg.ftl has the same keys in
# the same order with the German wording.

# Reading an exchange file (exchange/import.rs).
export_msg.exchange.title_hint = Mit einem eigenen Titel einlesen (Formular: Feld „Titel“, CLI: --title "…").
export_msg.page.template_error = Vorlage { $name }: { $error }
export_msg.exchange.read_aborted = Lesen abgebrochen: { $error }
export_msg.exchange.checksum_mismatch = Die Prüfsumme passt nicht zur gezeigten Datei; bitte die Vorschau neu holen.
export_msg.exchange.base64_invalid = data ist kein gültiges Base64.
export_msg.exchange.mode = mode ist „new“ oder „merge“.
export_msg.exchange.inbox_symlink = { $path } ist eine symbolische Verknüpfung; dort wird nichts abgelegt
export_msg.exchange.inbox_create = { $path } lässt sich nicht anlegen: { $error }
export_msg.exchange.inbox_write = { $path } lässt sich nicht schreiben: { $error }
export_msg.exchange.inbox_store = { $path } lässt sich nicht ablegen: { $error }
export_msg.exchange.too_large = Die Datei ist größer als { $mib } MiB; größere Werke über die Kommandozeile einlesen.
export_msg.exchange.not_multipart = Das Formular kam nicht als Datei-Upload an ({ $detail }).
export_msg.exchange.form_unreadable = Das Formular ließ sich nicht lesen ({ $detail }).
export_msg.exchange.file_unreadable = Die Datei ließ sich nicht lesen ({ $detail }).
export_msg.exchange.instance_unnamed = ohne Namen ({ $uuid })
export_msg.exchange.choose_file = Bitte eine Austauschdatei (.baruch oder .baruch.xml) wählen.
export_msg.exchange.checksum_invalid = Ungültige Prüfsumme; bitte die Datei neu wählen.
export_msg.exchange.mode_unknown = Unbekannter Modus (neu oder abgleichen).
export_msg.exchange.preview_gone = Die Datei der Vorschau ist nicht mehr da (nach 24 Stunden weggeräumt oder schon übernommen); bitte neu wählen.
export_msg.exchange.choose_again = Die Datei bitte noch einmal wählen.

# Counts of an exchange preview („2 Fassungen“; the number stands before the word).
export_msg.exchange.add.nodes = { $count ->
    [one] Gliederungspunkt
    *[other] Gliederungspunkte
    }
export_msg.exchange.add.revisions = { $count ->
    [one] Fassung
    *[other] Fassungen
    }
export_msg.exchange.add.template_revisions = { $count ->
    [one] Vorlagenfassung
    *[other] Vorlagenfassungen
    }
export_msg.exchange.add.ideas = { $count ->
    [one] Einfall
    *[other] Einfälle
    }
export_msg.exchange.add.posts = { $count ->
    [one] Beitrag
    *[other] Beiträge
    }
export_msg.exchange.add.threads = { $count ->
    [one] Faden
    *[other] Fäden
    }
export_msg.exchange.add.thread_posts = { $count ->
    [one] Fadenbeitrag
    *[other] Fadenbeiträge
    }
export_msg.exchange.add.assets = { $count ->
    [one] Bild
    *[other] Bilder
    }
export_msg.exchange.add.asset_revisions = { $count ->
    [one] Bildfassung
    *[other] Bildfassungen
    }
export_msg.exchange.add.links = { $count ->
    [one] Verbindung der Gedankenkarte
    *[other] Verbindungen der Gedankenkarte
    }
export_msg.exchange.add.positions = { $count ->
    [one] Lage auf der Gedankenkarte
    *[other] Lagen auf der Gedankenkarte
    }
export_msg.exchange.add.cover_blocks = { $count ->
    [one] Block der Umschlag-Anordnung
    *[other] Blöcke der Umschlag-Anordnung
    }
export_msg.exchange.add.findings = { $count ->
    [one] Befund
    *[other] Befunde
    }
export_msg.exchange.add.finding_entries = { $count ->
    [one] Eintrag im Verlauf der Befunde
    *[other] Einträge im Verlauf der Befunde
    }
export_msg.exchange.add.post_versions = { $count ->
    [one] alte Fassung eines Beitrags
    *[other] alte Fassungen von Beiträgen
    }
export_msg.exchange.add.typeset = { $count ->
    [one] Satz-Einstellungen
    *[other] Satz-Einstellungen
    }
export_msg.exchange.add.registers = { $count ->
    [one] eigenes Register
    *[other] eigene Register
    }
export_msg.exchange.add.glossary_terms = { $count ->
    [one] Glossarbegriff
    *[other] Glossarbegriffe
    }
export_msg.exchange.add.amends = { $count ->
    [one] Fortschreibung
    *[other] Fortschreibungen
    }

# EPUB check (epub_check.rs).
export_msg.epub_check.unknown_reader = Unbekannter Reader „{ $slug }“ (bekannt: { $known }).
export_msg.epub_check.section = Abschnitt { $key }
export_msg.epub_check.queue_closed = EPUBCheck: Warteschlange geschlossen
export_msg.epub_check.temp_dir = Temp-Ordner für EPUBCheck: { $error }
export_msg.epub_check.write_epub = EPUB für EPUBCheck schreiben: { $error }
export_msg.epub_check.aborted = Prüfung abgebrochen: { $error }
export_msg.epub_check.readers_list = readers: Liste von Slugs erwartet
export_msg.epub_check.footnotes = footnotes: „popup“ oder „endnotes“ erwartet
export_msg.epub_check.epubcheck_flag = epubcheck: true oder false erwartet
export_msg.epub_check.jar_too_long = EPUBCheck: Der Pfad ist zu lang (höchstens { $max } Bytes).
export_msg.epub_check.jar_control = EPUBCheck: Der Pfad enthält Steuerzeichen.
export_msg.epub_check.jar_relative = EPUBCheck: „{ $path }“ ist kein absoluter Pfad (z. B. /opt/epubcheck/epubcheck.jar).
export_msg.epub_check.jar_extension = EPUBCheck: „{ $path }“ endet nicht auf .jar.
export_msg.epub_check.jar_not_file = EPUBCheck: „{ $path }“ ist keine Datei.
export_msg.epub_check.jar_missing = EPUBCheck: „{ $path }“ gibt es nicht.
export_msg.epub_check.jar_param_missing = jar fehlt: Pfad zum epubcheck.jar oder null zum Entfernen
export_msg.epub_check.jar_param = jar: Pfad (Text) oder null erwartet
export_msg.epub_check.quick_xml = quick-xml (eingebaut; xmllint nicht gefunden)
export_msg.epub_check.program = Programm { $path }
export_msg.epub_check.found_title = EPUBCheck gefunden
export_msg.epub_check.found_text = { $tool } – startet nur auf Knopfdruck.
export_msg.epub_check.missing_title = EPUBCheck nicht gefunden
export_msg.epub_check.missing_text = Weder „epubcheck“ im PATH noch ein eingestelltes epubcheck.jar (Einstellungen → Daten). Oben steht die eigene Prüfung von Baruch; EPUBCheck wird nie heruntergeladen.
export_msg.epub_check.passed = EPUBCheck: bestanden
export_msg.epub_check.failed = EPUBCheck: nicht bestanden
export_msg.epub_check.ran = { $tool } – die Meldungen stehen mit „EPUBCheck:“ in der Liste.
export_msg.epub_check.ran_version = { $tool }, Version { $version } – die Meldungen stehen mit „EPUBCheck:“ in der Liste.
export_msg.epub_check.not_through = EPUBCheck lief nicht durch
export_msg.epub_check.report_error = Prüfbericht: { $error }
export_msg.epub_check.footnotes_form = fussnoten: „popup“ oder „endnoten“, nicht „{ $other }“.
export_msg.epub_check.foreign = Abgelehnt: Die Prüfung entsteht nicht auf Anfrage einer fremden Seite.
export_msg.page.error = Fehler: { $error }
export_msg.page.unknown_work = unbekanntes Werk
export_msg.page.work_missing = Fehler: Werk fehlt.

# Typesetting (typeset.rs).
export_msg.typeset.cancelled = Satz abgebrochen.
export_msg.typeset.timeout = Typst brauchte länger als { $limit }
export_msg.typeset.draft_param = draft: true (mit Wasserzeichen), false (ohne) oder weglassen (nach Einstellung)
export_msg.typeset.draft_form = entwurf „{ $other }“: 1 (mit Wasserzeichen) oder 0 (ohne) erwartet
export_msg.typeset.lock = Typst-Sperre: { $error }
export_msg.typeset.crashed = Typst ist abgestürzt: { $error }
export_msg.typeset.tail = Buchende (Register, Glossar)
export_msg.typeset.number = { $name }: Zahl erwartet
export_msg.typeset.no_section = Abschnitt mit Schlüssel { $key } gibt es in Werk { $work } nicht.
export_msg.typeset.no_page = Seite { $page } gibt es nicht (1–{ $total }).
export_msg.typeset.foreign = Abgelehnt: Der Satz startet nicht auf Anfrage einer fremden Seite.
export_msg.typeset.template_error = Fehler: Vorlage { $name }: { $error }
export_msg.typeset.nothing_to_cancel = Es lief kein Satz, der abzubrechen war.
export_msg.typeset.was_cancelled = Der Satz wurde abgebrochen.
export_msg.typeset.preview_plain = Vorschau ohne Wasserzeichen
export_msg.typeset.preview_draft = Vorschau als Entwurf
export_msg.typeset.page_gone = Seite gibt es nicht (anderer oder veralteter Stand)

# PDF output (pdf.rs).
export_msg.pdf.kind_book = Buch
export_msg.pdf.kind_cover = Umschlag
export_msg.pdf.kind_notes = Stichwortzettel
export_msg.pdf.busy = Für dieses Werk entsteht gerade ein PDF; bitte warten und dann noch einmal versuchen.
export_msg.pdf.no_pdf_from_typst = Typst lieferte kein PDF
export_msg.pdf.error_in_file = { $message } ({ $file }, Zeile { $line })
export_msg.pdf.error_in_export = { $message } (Zeile { $line } im Export)
export_msg.pdf.failed_one = Kein PDF ({ $kind }): Der Satz scheitert mit { $count } Fehler. Erster: { $first }
export_msg.pdf.failed = Kein PDF ({ $kind }): Der Satz scheitert mit { $count } Fehlern. Erster: { $first }
export_msg.pdf.flag = { $name }: true oder false erwartet
export_msg.pdf.kind_map = Mindmap

# EPUB output (epub.rs).
export_msg.epub.raster = Rasterbild: { $error }
export_msg.epub.cover_image_instead = EPUB-Cover: Die Umschlag-Vorderseite ließ sich nicht als Bild setzen ({ $reason }); statt dessen steht die Bilddatei des Umschlags „{ $path }“ ohne Titel im EPUB.
export_msg.epub.cover_none = EPUB-Cover: Die Umschlag-Vorderseite ließ sich nicht als Bild setzen ({ $reason }); das EPUB hat kein Cover.
export_msg.epub.foreign = Abgelehnt: Das EPUB entsteht nicht auf Anfrage einer fremden Seite.
export_msg.epub.footnotes = fussnoten: popup oder endnoten

# EPUB preview (epub_preview.rs).
export_msg.epub_kind.cover = Umschlag
export_msg.epub_kind.front = Titelei
export_msg.epub_kind.part = Teil
export_msg.epub_kind.chapter = Kapitel
export_msg.epub_kind.notes = Anmerkungen
export_msg.epub_kind.nav = Inhalt
export_msg.epub_kind.ncx = Inhalt (EPUB 2)
export_msg.epub_kind.style = Stil
export_msg.epub_kind.image = Bild
export_msg.epub_kind.cover_image = Umschlagbild
export_msg.epub_kind.glossary = Glossar
export_msg.epub_kind.index = Register
export_msg.epub_preview.unknown_reader = readers: unbekanntes Reader-Profil „{ $slug }“
export_msg.epub_preview.no_text_file = file: keine Datei mit Text im EPUB: { $file }
export_msg.epub_preview.no_text = Das EPUB hat keine Datei mit Text.
export_msg.epub_preview.foreign = Abgelehnt: Die EPUB-Vorschau entsteht nicht auf Anfrage einer fremden Seite.
export_msg.epub_preview.unknown_profile = Unbekanntes Reader-Profil.
export_msg.epub_preview.unknown_file = Unbekannte Datei.
export_msg.epub_preview.unknown_work = Unbekanntes Werk.
export_msg.epub_preview.lacks_popup = Fußnoten als Sprung statt Popup
export_msg.epub_preview.lacks_backgrounds = keine Hintergrundfarben
export_msg.epub_preview.lacks_svg = SVG-Bilder unsicher
export_msg.epub_preview.lacks_fonts = keine eingebetteten Schriften
export_msg.epub_preview.overrides_font = eigene Vorgabeschrift statt der Schrift des Buchs
export_msg.epub_preview.lacks_hyphenation = keine Silbentrennung
export_msg.epub_preview.lacks_night = kein Nachtmodus
export_msg.epub_preview.profile_not_shown = Unbekanntes Reader-Profil „{ $slug }“ – nicht angezeigt.
export_msg.epub_preview.footnotes_default = Fußnoten: „popup“ oder „endnoten“ – gezeigt wird die Vorgabe (Popup).
export_msg.epub_preview.file_missing = Die Datei „{ $file }“ gibt es im EPUB nicht – gezeigt wird die erste Kapiteldatei.

# Exchange page (exchange/export.rs).
export_msg.exchange.title = Austausch
export_msg.exchange.failed = Austausch: { $error }
export_msg.exchange.format_not = format: „baruch“ oder „xml“ erwartet, nicht „{ $word }“
export_msg.exchange.format = format: „baruch“ oder „xml“ erwartet

# Speaker notes (pdf/notes.rs).
export_msg.notes.rule_marker = eigene Markierung
export_msg.notes.rule_strong = Hervorhebung
export_msg.notes.rule_first_sentence = erster Satz je Absatz
export_msg.notes.format_unknown = Format „{ $text }“ gibt es nicht; a5 oder karten erwartet
export_msg.notes.font_size = Schriftgröße „{ $text }“: ganze Zahl von { $min } bis { $max } (Punkt) erwartet
export_msg.notes.format_param = format: "a5" oder "karten" erwartet
export_msg.notes.font_param = font_pt: Zahl von { $min } bis { $max } erwartet

# Word export (docx_export/*).
export_msg.docx.write = Word-Datei schreiben: { $error }
export_msg.docx.unknown_work = work: unbekanntes Werk
export_msg.docx.foreign = Abgelehnt: Die Word-Datei entsteht nicht auf Anfrage einer fremden Seite.
export_msg.docx.proposals_flag = vorschlaege: 1 oder 0
export_msg.docx.more_islands = { $count } weitere Typst-Inseln stehen ebenfalls nur als sichtbarer Quelltext in der Datei.
export_msg.docx.untitled = ohne Titel
export_msg.docx.quoted_title = „{ $title }“
export_msg.docx.section_warning = Abschnitt { $place }: { $message }
export_msg.docx.more_proposals_one = Abschnitt { $place }: { $more } weiterer offener Vorschlag steht nicht in der Datei – als Änderungsverfolgung steht nur der jüngste (Fassung { $rev } von { $author }).
export_msg.docx.more_proposals = Abschnitt { $place }: { $more } weitere offene Vorschläge stehen nicht in der Datei – als Änderungsverfolgung steht nur der jüngste (Fassung { $rev } von { $author }).

# Typesetting settings and sheet (typeset_settings.rs, sheet.rs).
export_msg.typeset_settings.confirm = confirm: true erwartet
export_msg.typeset_settings.missing = settings fehlt
export_msg.typeset_settings.invalid = settings: { $error }
export_msg.typeset_settings.no_rule = Die Werkvorlage hat keine erkennbare Regel für den Kapitelanfang; nichts geändert.
export_msg.typeset_settings.no_document_title = Kolumnentitel „Werktitel“: Die Werkvorlage setzt keinen Titel (#set document(title: …)); die Kolumne bleibt leer
export_msg.typeset_settings.font_missing = Schrift „{ $font }“ fehlt; Typst setzt mit der eingebetteten Schrift
export_msg.sheet.reset = reset: true oder false erwartet
export_msg.sheet.text = { $field }: Text erwartet
