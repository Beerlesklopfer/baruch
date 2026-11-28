# SPDX-License-Identifier: AGPL-3.0-or-later
# Round 39a (templates_2): Export and import pages: Word, EPUB check and preview, exchange file, typesetting.
# German translation of locales/en/export.ftl (same keys in the same order).

# Shared by the export and import pages. These pages translate only the surrounding interface: book texts, style
# names in the Word file and the contents of exchange files stay as they are (Round 39b).
export.back_to_work = Zurück zum Werk
export.to_work = Zum Werk
export.works = Werke
export.next = Weiter
export.title = Titel
export.notes = Hinweise
export.discard = Verwerfen
export.show_preview = Vorschau zeigen

# Page /werke/{id}/word (docx_export.html).
export.word.window_title = Word-Export – { $title } – Baruch
export.word.page_title = { $title }: Word-Export
export.word.heading = Word-Export
export.word.intro_html = Das Werk <strong>{ $title }</strong> als Word-Datei (<code>{ $slug }.docx</code>) für den Verlag: Titelei, dann alle Abschnitte in Lesereihenfolge mit ihrer <strong>aktuellen Fassung</strong>.
export.word.styles = Überschriften tragen die Formatvorlagen „Überschrift 1–3“, Teile „Baruch Teil“, Überschriften ohne Nummer „Baruch ohne Nummer 1–3“.
export.word.footnotes = Fußnoten sind echte Fußnoten, Bilder (PNG, JPEG, GIF) sind eingebettet und tragen ihren Alternativtext.
export.word.unmapped = Was Word nicht abbilden kann (Typst-Funktionen, Formeln, Bilder in anderen Formaten), steht als sichtbarer Quelltext in einem grau hinterlegten Feld. Nichts geht still verloren; solche Stellen nennt der Export über RPC und Kommandozeile als Warnung.
export.word.unchanged = Der Export ändert nichts am Werk.
export.word.with_proposals = Offene Vorschläge als Änderungsverfolgung mitgeben
export.word.open_proposals = Zurzeit { $proposals ->
    [one] ist 1 Vorschlag
    *[other] sind { $proposals } Vorschläge
    } offen, in { $sections ->
    [one] 1 Abschnitt
    *[other] { $sections } Abschnitten
    }.
export.word.no_open_proposals = Zurzeit ist kein Vorschlag offen; die Datei ist mit und ohne Häkchen dieselbe.
export.word.proposals_hint = Je Abschnitt steht der jüngste offene Vorschlag als Einfügung und Löschung in der Datei, mit der Persona als Bearbeiter; die aktuelle Fassung bleibt der Ausgangstext. Weitere offene Vorschläge desselben Abschnitts stehen nicht in der Datei.
export.word.download = Word-Datei herunterladen
export.word.check_hint = Ob die Datei in Word oder LibreOffice so aussieht wie gedacht, zeigt erst das Öffnen dort.

# Importing a Word file (docx_import.html).
export.word_import.window_title = Word-Datei importieren – Baruch
export.word_import.page_title = Word-Datei importieren
export.word_import.heading = Word-Datei lesen
export.word_import.intro = Baruch liest die Datei und zeigt Gliederung, Bearbeiter und Warnungen. Angelegt wird erst, wenn du die Vorschau bestätigst. Die Datei bleibt auf diesem Rechner.
export.word_import.file = Datei (.docx, höchstens { $mib } MiB)
export.word_import.title_hint = (leer: aus der Datei)
export.word_import.read = Lesen und Vorschau zeigen
export.word_import.doc_hint = Das alte Format .doc liest Baruch nicht – in Word als .docx speichern. Typst oder Markdown?
export.word_import.new_work = Neues Werk / Importieren …
export.word_import.preview = Vorschau: { $name }
export.word_import.rule = Änderungen und Kommentare werden nie zur aktuellen Fassung.
export.word_import.rule_more = Fassung 1 jedes Abschnitts ist der Stand vor allen verfolgten Änderungen; die Änderungen jedes Bearbeiters werden offene Vorschläge, Kommentare werden Beiträge am Abschnitt der kommentierten Stelle.
export.word_import.sections = { $count } { $count ->
    [one] Abschnitt
    *[other] Abschnitte
    }
export.word_import.comments = { $count } { $count ->
    [one] Kommentar
    *[other] Kommentare
    }
export.word_import.footnotes = { $count } { $count ->
    [one] Fußnote
    *[other] Fußnoten
    }
export.word_import.images = { $count } { $count ->
    [one] Bild
    *[other] Bilder
    }
export.word_import.editors = Bearbeiter
export.word_import.col_name = Name in Word
export.word_import.col_baruch = in Baruch
export.word_import.col_insertions = Einfügungen
export.word_import.col_deletions = Löschungen
export.word_import.col_proposals = Vorschläge (Abschnitte)
export.word_import.no_name = ohne Namen
export.word_import.no_changes = Die Datei enthält keine verfolgten Änderungen.
export.word_import.outline = Gliederung
export.word_import.part_prefix = Teil:
export.word_import.part = Teil
export.word_import.level = Ebene { $level }
export.word_import.unnumbered = ohne Nummer
export.word_import.chars = { $count } Zeichen
export.word_import.apply = Übernehmen
export.word_import.changes_as_proposals = verfolgte Änderungen als offene Vorschläge anlegen
export.word_import.comments_as_posts = Kommentare als Beiträge anlegen
export.word_import.adopt = Als Werk übernehmen
export.word_import.discard_hint = löscht den Entwurf; nichts wird angelegt.
export.word_import.adopted = Übernommen: { $title }
export.word_import.report =
    Das Werk ist angelegt: { $sections } { $sections ->
        [one] Abschnitt
       *[other] Abschnitte
    }, { $proposals } { $proposals ->
        [one] offener Vorschlag
       *[other] offene Vorschläge
    }, { $posts } { $posts ->
        [one] Beitrag
       *[other] Beiträge
    }. Beim Übernehmen ist etwas aufgefallen:

# EPUB pages (epub_check.html, fragments/epub_check_panel.html, epub_preview.html).
export.epub.download = EPUB herunterladen
export.epub.preview = EPUB-Vorschau
export.epub.required = (Pflicht)
export.epub.footnotes = Fußnoten
export.epub.as_popup = als Popup (Vorgabe)
export.epub_check.window_title = EPUB prüfen – { $title } – Baruch
export.epub_check.page_title = { $title }: EPUB prüfen
export.epub_check.heading = EPUB prüfen
export.epub_check.as_endnotes = als Endnoten
export.epub_check.recheck = Neu prüfen
export.epub_check.epubcheck_before = EPUBCheck ist das Prüfprogramm der W3C-Gemeinschaft. Baruch nutzt „epubcheck“ aus dem PATH oder ein epubcheck.jar unter einem eingestellten Pfad (Einstellungen →
export.epub_check.epubcheck_link = Daten
export.epub_check.epubcheck_after = , braucht Java) und lädt nie etwas herunter. Ein Lauf dauert einige Sekunden.
export.epub_check.run = EPUBCheck ausführen
export.epub_panel.heading = EPUB-Prüfung
export.epub_panel.no_errors = Keine Fehler.
export.epub_panel.errors = { $count } Fehler
export.epub_panel.warnings = { $count } { $count ->
    [one] Warnung
    *[other] Warnungen
    }
export.epub_panel.infos = { $count } { $count ->
    [one] Hinweis
    *[other] Hinweise
    }
export.epub_panel.tools = (Wohlgeformtheit: { $xml }; Reader: { $readers })
export.epub_panel.col_kind = Art
export.epub_panel.col_message = Meldung
export.epub_panel.col_place = Ort
export.epub_panel.no_findings = Keine Befunde.
export.epub_panel.full_report = Ganzer Prüfbericht, EPUBCheck ausführen …
export.epub_preview.window_title = EPUB-Vorschau – { $title } – Baruch
export.epub_preview.page_title = { $title }: EPUB-Vorschau
export.epub_preview.heading = EPUB-Vorschau je Reader
export.epub_preview.kindle_html = Kindle: Baruch erzeugt kein eigenes Kindle-Format. <strong>Send to Kindle nimmt EPUB an</strong> und wandelt es für Kindle-Geräte und die Kindle-App um; die Profile „Kindle (über EPUB)“ und „Kindle-App (über EPUB)“ zeigen das ungefähre Ergebnis.
export.epub_preview.profiles = Die Profile bilden Schrift, Größe, Ränder, Zeilenabstand, Silbentrennung und Nachtmodus der Reader nach – ohne echtes Gerät, als Annahme. Verbindlich ist das Gerät selbst.
export.epub_preview.file = Datei
export.epub_preview.as_endnotes = als Anmerkungen am Ende
export.epub_preview.night = Nachtmodus
export.epub_preview.show = Anzeigen
export.epub_preview.files = Dateien und Kapitelgrenzen
export.epub_preview.files_hint = Jeder Teil und jedes Kapitel beginnt eine eigene Datei (neue Seite im Reader).
export.epub_preview.warnings = Hinweise beim Erzeugen
export.epub_preview.side_by_side = Reader nebeneinander
export.epub_preview.via_kindle = Über Send to Kindle (EPUB wird umgewandelt).
export.epub_preview.frame_title = { $name }: Vorschau
export.epub_preview.lacks = Eigenheiten: { $list }.
export.epub_preview.open_alone = Datei allein öffnen
export.epub_preview.check = Prüfung
export.epub_preview.no_report = Kein Prüfbericht hier. Ausführlich:

# Page /werke/{id}/austausch (exchange.html).
export.exchange.window_title = Austausch – { $title } – Baruch
export.exchange.page_title = { $title }: Austausch
export.exchange.heading = Austausch: Ausgeben
export.exchange.intro_html = Die Austauschdatei trägt das <strong>ganze Werk</strong> zu einer anderen Baruch-Instanz: Angaben, Buchangaben samt Umschlag, Werkvorlage, die Gliederung mit <strong>allen Fassungen</strong> und der Markierung „aktuell“, Beiträge, Einfälle (auch gelöschte), die Gedankenkarte (Verbindungen zwischen Einfällen und gemerkte Lagen), Fäden, Bilder mit allen Fassungen, Satz-Einstellungen, das Glossar (auch Vorschläge) mit den eigenen Registern und die Grabsteine gelöschter Teile. Jedes Stück hat eine weltweit eindeutige Kennung (UUID); so erkennt die andere Instanz beim Einlesen, was sie schon kennt, und fügt nur hinzu, was fehlt.
export.exchange.not_in_file = Nicht in der Datei: Schlüssel und Token, Verbindungen zu Diensten (Modelle, MultiGPT, pgvector), Personas samt Modellen (nur ihre Namen als Herkunft), offene Anträge und Registervorschläge, Belege der Websuche, Bibel-, Kommentar- und Literaturtexte (nur Modulnamen und Stellen), Suchindex und die Autorenbibliothek.
export.exchange.downloads = Herunterladen
export.exchange.download_baruch = Austauschdatei herunterladen (.baruch)
export.exchange.baruch_hint_html = { $name } – ZIP mit <code>werk.xml</code>, den Bildern als eigene Dateien und einem Manifest mit Prüfsummen. Für alle Werke.
export.exchange.download_xml = Nur XML herunterladen (.baruch.xml)
export.exchange.xml_hint = { $name } – eine einzige XML-Datei, Bilder als Base64 darin. Gut lesbar, für kleine Werke.
export.exchange.scope = Umfang
export.exchange.count_nodes = { $nodes } { $nodes ->
    [one] Gliederungspunkt
    *[other] Gliederungspunkte
    } mit { $revisions } { $revisions ->
    [one] Fassung
    *[other] Fassungen
    } und { $posts } { $posts ->
    [one] Beitrag
    *[other] Beiträgen
    }
export.exchange.count_template = { $count } { $count ->
    [one] Fassung
    *[other] Fassungen
    } der Werkvorlage
export.exchange.count_ideas = { $count } { $count ->
    [one] Einfall
    *[other] Einfälle
    }
export.exchange.of_them_deleted = (davon { $count } gelöscht)
export.exchange.count_threads = { $threads } { $threads ->
    [one] Faden
    *[other] Fäden
    } mit { $posts } { $posts ->
    [one] Beitrag
    *[other] Beiträgen
    }
export.exchange.count_findings = { $findings } { $findings ->
    [one] Befund
    *[other] Befunde
    } (davon { $open } offen) und { $versions } { $versions ->
    [one] alte Beitragsfassung
    *[other] alte Beitragsfassungen
    }
export.exchange.count_assets = { $assets } { $assets ->
    [one] Bild
    *[other] Bilder
    } mit { $revisions } { $revisions ->
    [one] Fassung
    *[other] Fassungen
    }
export.exchange.count_links = Gedankenkarte: { $count } { $count ->
    [one] Verbindung
    *[other] Verbindungen
    }
export.exchange.count_positions = und { $count } { $count ->
    [one] gemerkte Lage
    *[other] gemerkte Lagen
    }
export.exchange.typeset = Satz-Einstellungen: { $fields }
export.exchange.count_terms = Glossar: { $count } { $count ->
    [one] Begriff
    *[other] Begriffe
    }
export.exchange.of_them_proposed = (davon { $count } vorgeschlagen)
export.exchange.count_registers = , { $count } { $count ->
    [one] eigenes Register
    *[other] eigene Register
    }
export.exchange.count_tombstones = { $count } { $count ->
    [one] Grabstein
    *[other] Grabsteine
    } gelöschter Teile
export.exchange.work_id = Werk-Kennung
export.exchange.instance = Instanz { $label }
export.exchange.format = Format
export.exchange.version = Version { $version }
export.exchange.read_in_before = Eine Datei einer anderen Instanz übernehmen:
export.exchange.read_in_link = Austausch: Einlesen…
export.exchange.read_in_after = (mit Vorschau; Baruch löscht und überschreibt dabei nie etwas).
export.exchange.cli = Auch über die Kommandozeile:
export.exchange.deviations = Abweichungen ({ $count })
export.exchange.col_what = Was
export.exchange.col_here = Hier
export.exchange.col_there = Dort
export.exchange.col_action = Folge

# Answer after adopting an exchange file (exchange_done.html).
export.exchange_done.window_title = Austausch übernommen – { $title } – Baruch
export.exchange_done.page_title = Austausch übernommen
export.exchange_done.heading = „{ $title }“: { $mode } übernommen
export.exchange_done.from_html = Aus <strong>{ $instance }</strong>.
export.exchange_done.nothing_added = Nichts kam hinzu.
export.exchange_done.added = Hinzu:
export.exchange_done.commit_html = Im Spiegel als Commit <code>{ $commit }</code> eingetragen.
export.exchange_done.mirror = Spiegel: { $error } (das Werk ist trotzdem übernommen).
export.exchange_done.refused = Nicht übernommen: { $refusal }
export.exchange_done.requests = { $count } { $count ->
    [one] Antrag
    *[other] Anträge
    } zur Gliederung ansehen
export.exchange_done.deleted_here = Hier gelöscht und nicht wieder angelegt: { $count }.
export.exchange_done.read_another = Weitere Datei einlesen

# Page /austausch/einlesen (exchange_import.html).
export.exchange_import.window_title = Austausch einlesen – Baruch
export.exchange_import.heading = Austausch: Einlesen
export.exchange_import.choose = Austauschdatei wählen
export.exchange_import.file = Datei (.baruch oder .baruch.xml, höchstens { $mib } MiB)
export.exchange_import.title_hint = (nur für ein neues Werk; leer: aus der Datei)
export.exchange_import.what_happens = Was geschieht
export.exchange_import.preview_first_html = Zuerst kommt eine <strong>Vorschau</strong>: was hinzukäme, was abweicht, welche Anträge entstünden. Geschrieben wird erst mit „Übernehmen“.
export.exchange_import.unknown_work_html = Ist das Werk hier <strong>unbekannt</strong>, entsteht ein neues Werk mit allen Fassungen, Einfällen, Beiträgen, Fäden und Bildern.
export.exchange_import.known_work_html = Ist es <strong>bekannt</strong>, wird abgeglichen: Es kommt nur hinzu, was fehlt. Nichts wird gelöscht oder überschrieben, keine eingelesene Fassung wird aktuell; eine abweichende Fassung kommt als weitere Fassung, Änderungen der Gliederung werden
export.exchange_import.requests_link = Anträge
export.exchange_import.known_work_after = , über die du entscheidest.
export.exchange_import.deleted_stays = Was hier gelöscht wurde, kommt nicht wieder. Unbekannte Teile einer neueren Datei werden gemeldet, aber nicht übernommen.

# Preview of an exchange file (exchange_preview.html).
export.exchange_preview.window_title = Austausch: Vorschau – Baruch
export.exchange_preview.page_title = Austausch: Vorschau
export.exchange_preview.group = Austausch
export.exchange_preview.other_file = Andere Datei
export.exchange_preview.heading = Vorschau: { $name }
export.exchange_preview.from_html = Aus <strong>{ $instance }</strong>, ausgegeben { $exported }. Werk „{ $title }“ –
export.exchange_preview.mode_new_html = <strong>neu</strong>: es gibt es hier noch nicht.
export.exchange_preview.mode_merge_html = <strong>abgleichen</strong> mit dem vorhandenen Werk.
export.exchange_preview.newer = Die Datei hat Unterversion { $version }; dieses Baruch liest Unterversion 1. Unbekanntes wird unten gemeldet und nicht übernommen.
export.exchange_preview.would_add = Was hinzukäme
export.exchange_preview.nothing = Nichts: Alles aus der Datei gibt es hier schon.
export.exchange_preview.single = Einzeln ({ $count }
export.exchange_preview.truncated = , gekürzt
export.exchange_preview.requests = Anträge zur Gliederung ({ $count })
export.exchange_preview.requests_hint = Sie ändern nichts sofort; du bestätigst oder lehnst sie unter Gliederung → Anträge ab.
export.exchange_preview.deleted_here = Hier gelöscht ({ $count })
export.exchange_preview.does_not_return = Kommt nicht wieder.
export.exchange_preview.unknown = Unbekannt, wird nicht übernommen
export.exchange_preview.title_hint = (leer: „{ $title }“ aus der Datei)
export.exchange_preview.checksum_html = Prüfsumme der Datei: <code>{ $sha256 }</code>. Übernommen wird genau diese Datei; geplant wird dabei neu, in einem Schritt, mit einem Eintrag im Spiegel.
export.exchange_preview.adopt = Übernehmen ({ $mode })

# Page /werke/{id}/satz (typeset.html): only the interface around the typeset pages.
export.typeset.window_title = Satz – { $title } – Baruch
export.typeset.page_title = { $title }: Satz
export.typeset.download = Herunterladen
export.typeset.pdf_hint = Das ganze Werk als PDF (Werkvorlage, aktuelle Fassungen, Bilder)
export.typeset.pdf = PDF herunterladen
export.typeset.pages = Seiten
export.typeset.previous = Vorige Seite
export.typeset.previous_none = Vorige Seite (dies ist die erste)
export.typeset.page_of = Seite { $shown } von { $total }
export.typeset.next = Nächste Seite
export.typeset.next_none = Nächste Seite (dies ist die letzte)
export.typeset.run = Satzlauf
export.typeset.cancel_hint = Laufenden Typst-Satz dieses Werks abbrechen (auch einen, der noch wartet)
export.typeset.cancel = Satz abbrechen
export.typeset.heading = Satz mit Typst
export.typeset.draft = Entwurf
export.typeset.watermarked = Die Vorschau zeigt das Wasserzeichen.
export.typeset.draft_hint = Buch-PDF mit Wasserzeichen „Entwurf“; die Satz-Einstellung bleibt, wie sie ist
export.typeset.download_draft = Als Entwurf herunterladen
export.typeset.plain_hint = Buch-PDF ohne Wasserzeichen; die Satz-Einstellung bleibt, wie sie ist
export.typeset.download_plain = Ohne Wasserzeichen herunterladen
export.typeset.not_reset = Es wurde nicht neu gesetzt.
export.typeset.last_state = Gezeigt wird der zuletzt gesetzte Stand.
export.typeset.restart = Neu setzen
export.typeset.page_count = { $count } { $count ->
    [one] Seite
    *[other] Seiten
    }
export.typeset.no_pages = keine Seiten
export.typeset.errors = { $count } Fehler
export.typeset.warnings = { $count } { $count ->
    [one] Warnung
    *[other] Warnungen
    }
export.typeset.cached = aus dem Zwischenspeicher
export.typeset.millis = gesetzt in { $millis } ms
export.typeset.messages = Meldungen
export.typeset.line = Zeile { $line }
export.typeset.column = , Spalte { $column }
export.typeset.in_file = in { $file }
export.typeset.comma_line = , Zeile { $line }
export.typeset.hint = Hinweis: { $hint }
export.typeset.print_check = Druckprüfung
export.typeset.badge_template = Vorlage
export.typeset.template_rule = Die Werkvorlage bestimmt den Kapitelanfang selbst ({ $label }, Zeile { $line }):
export.typeset.adopt_rule = In den Satz-Einstellungen übernehmen und die Zeile entfernen
export.typeset.badge_typeset = Satz
export.typeset.settings = Satz-Einstellungen
export.typeset.badge_xref = Verweis
export.typeset.broken_xref = Kaputter Querverweis { $label } in { $section }: „{ $text }“ (im Satz nur als Text)
export.typeset.to_place = zur Stelle
export.typeset.check_xrefs = Querverweise prüfen
export.typeset.badge_missing = Fehlt
export.typeset.image_missing = Bild fehlt: { $path }
export.typeset.upload_or_assign = Bilder hochladen oder Pfad zuordnen
export.typeset.dpi_where = Seite { $page } · { $width } × { $height } Pixel
export.typeset.badge_imprint = Impressum
export.typeset.imprint_hint = üblich nach den Landespressegesetzen, keine Rechtsberatung
export.typeset.book_details = Buchangaben
export.typeset.all_fine = Alle Bilder sind da; kein Rasterbild liegt unter { $dpi } dpi.
export.typeset.chapter_pages = Seiten des Kapitels
export.typeset.chapter = Kapitel
