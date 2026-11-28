# SPDX-License-Identifier: AGPL-3.0-or-later
# Sources, literature and web evidence (Round 39a, templates_3), German translation of locales/en/sources.ftl (same keys in the same order).

# templates/source.html
sources.source.window_title = { $title } – Quelle – Baruch
sources.source.page_title = Quelle · { $title }
sources.source.back_group = Zurück
sources.source.literature = Literatur
sources.source.back_to_work = Zurück zum Werk
sources.source.quotes = Zitate
sources.source.quote_n = Zitat { $index }
sources.source.no_quotes = Noch keine Zitate.
sources.source.fulltext = Volltext
sources.source.load_fulltext = Volltext laden
sources.source.part = Teil { $part }
sources.source.part_of = von { $parts }
sources.source.end_unknown = , Ende noch unbekannt
sources.source.marks = · { $marks } Fundstellen markiert
sources.source.parts_nav = Teile des Volltexts
sources.source.prev_part = ← vorheriger Teil
sources.source.next_part = nächster Teil →
sources.source.highlights = Hervorhebungen
sources.source.use_selection = Markierten Text verwenden
sources.source.start = Anfang im Volltext
sources.source.end = Ende im Volltext
sources.source.quote = Zitat
sources.source.no_quote = – ohne Zitat –
sources.source.locator = Fundstelle
sources.source.locator_placeholder = S. 12 (leer: Stelle des Zitats)
sources.source.text = Text
sources.source.text_placeholder = Markierten Text hier einfügen …
sources.source.kind = Art
sources.source.note = Notiz
sources.source.question = Frage an die Persona
sources.source.question_placeholder = Was sagt dieser Text zu meinem Abschnitt?
sources.source.highlight = Hervorheben
sources.source.cite = Zitieren (Vorschlag im Abschnitt)
sources.source.persona_title = Die Persona antwortet im Abschnitts-Chat
sources.source.to_persona = An Persona geben
sources.source.similar = Ähnliches suchen

# templates/source_import.html
sources.import.window_title = PDF oder EPUB importieren – Baruch
sources.import.title = PDF oder EPUB importieren
sources.import.back_group = Zurück
sources.import.works = Werke
sources.import.back_to_work = Zurück zum Werk
sources.import.multigpt_failed = MultiGPT hat „{ $file_name }“ nicht gelesen; Baruch wechselt nicht von selbst den Weg.
sources.import.read_local = Lokal lesen
sources.import.read_local_help = pdftotext auf diesem Rechner; die Datei liegt noch bereit, nichts ist angelegt.
sources.import.read_file = Datei lesen
sources.import.intro = Baruch liest die Datei und schlägt eine Gliederung vor. Angelegt wird erst, wenn du die Gliederung in der Vorschau bestätigst – als Werk, als Quelle der Bibliothek oder beides.
sources.import.file = Datei (.pdf oder .epub, höchstens { $max_mib } MiB)
sources.import.way = Weg (nur PDF; EPUB liest Baruch immer selbst)
sources.import.way_auto = automatisch
sources.import.way_auto_help = (MultiGPT, wenn eingerichtet, eine Sammlung gewählt und Hochladen angekreuzt ist; sonst lokal)
sources.import.way_local = lokal
sources.import.way_local_help = (pdftotext auf diesem Rechner, nichts verlässt ihn)
sources.import.way_multigpt = über MultiGPT
sources.import.way_multigpt_help = (Inhaltsverzeichnis von MultiGPT)
sources.import.way_multigpt_missing = (keine Verbindung eingerichtet: Einstellungen → Verbindungen)
sources.import.collection = Sammlung in MultiGPT
sources.import.upload_named = in MultiGPT hochladen (Sammlung „{ $collection }“)
sources.import.upload_unnamed = in MultiGPT hochladen (Sammlung oben eintragen)
sources.import.title_field = Titel
sources.import.title_empty = (leer: aus der Datei)
sources.import.read_submit = Lesen und Gliederung zeigen
sources.import.typst_instead = Typst-Datei statt PDF?
sources.import.new_work = Neues Werk / Importieren …
sources.import.preview = Vorschau: { $file_name }
sources.import.way_line_html = <strong>Weg: { $way_label }</strong> ({ $kind_label }
sources.import.pages =
    , { $count } { $count ->
        [one] Seite
       *[other] Seiten
    }
sources.import.document_html = In MultiGPT: Sammlung „{ $collection }“, Dokument <code>{ $id }</code>.
sources.import.hints = Hinweise
sources.import.skipped = Übersprungene Bilder
sources.import.skipped_image_html = Bild übersprungen: <code>{ $path }</code> – { $reason }
sources.import.assets = { $assets } Bild(er) kommen mit ins Werk.
sources.import.check_outline = Gliederung prüfen
sources.import.outline_help_html = Je Zeile ein Punkt: <code>= Titel</code> Kapitel, <code>== Titel</code> Abschnitt, <code>=== Titel</code> Unterabschnitt; <code>= Teil: Titel</code> macht einen Teil. Zeile leeren = Punkt streichen (sein Text geht an den vorigen Punkt). Die Reihenfolge bleibt wie in der Datei.
sources.import.item = Punkt { $index }
sources.import.chars = { $chars } Zeichen
sources.import.part_hint = · möglicher Teil – als Teil nur mit „= Teil: …“
sources.import.adopt_as = Übernehmen als
sources.import.as_work = Werk
sources.import.as_library = Quelle der Bibliothek
sources.import.as_both = beides
sources.import.author = Autor
sources.import.adopt = Übernehmen
sources.import.discard = Verwerfen
sources.import.discard_help = löscht den Entwurf; nichts wird angelegt.

# templates/source_multigpt.html
sources.multigpt.window_title_end = – Volltext – Baruch
sources.multigpt.page_title = Volltext · { $title }
sources.multigpt.back_group = Zurück
sources.multigpt.literature = Literatur
sources.multigpt.back_to_work = Zurück zum Werk
sources.multigpt.collection = · Sammlung { $collection }
sources.multigpt.document_html = · Dokument <code>{ $document }</code>
sources.multigpt.section = · Fundstelle laut MultiGPT: { $section }
sources.multigpt.in_library = In der Bibliothek:
sources.multigpt.adopt = In die Bibliothek übernehmen
sources.multigpt.adopt_help = legt einen Titel an, verknüpft mit diesem Dokument (Volltext und Prüfung von Hervorhebungen)
sources.multigpt.fulltext = Volltext
sources.multigpt.load_fulltext = Volltext laden
sources.multigpt.part = Teil { $part }
sources.multigpt.part_of = von { $parts }
sources.multigpt.end_unknown = , Ende noch unbekannt
sources.multigpt.marks = · { $marks } Fundstellen markiert
sources.multigpt.parts_nav = Teile des Volltexts
sources.multigpt.prev_part = ← vorheriger Teil
sources.multigpt.next_part = nächster Teil →
sources.multigpt.to_persona = An Persona geben
sources.multigpt.needs_section = „An Persona geben“ braucht einen Abschnitt: die Literatur aus einem Abschnitt heraus öffnen.
sources.multigpt.text = Text (höchstens { $max_persona_text } Zeichen; auf die wichtige Stelle kürzen)
sources.multigpt.question = Frage an die Persona
sources.multigpt.question_placeholder = Was sagt dieser Text zu meinem Abschnitt?
sources.multigpt.give_source = Quelle am Beitrag: { $give_source } – kein Titel der Bibliothek nötig.

# templates/literature.html
sources.literature.window_title = Literatur – Baruch
sources.literature.page_title = Literatur · { $title }
sources.literature.back_group = Zurück
sources.literature.back_to_work = Zurück zum Werk
sources.literature.title = Literatur

# templates/literature_bible.html
sources.bible.bible = Bibel
sources.bible.window_title_end = – Literatur – Baruch
sources.bible.page_title = Bibel · { $title }
sources.bible.back_group = Zurück
sources.bible.literature = Literatur
sources.bible.back_to_work = Zurück zum Werk
sources.bible.settings_sources = Einstellungen → Quellen
sources.bible.heading = Bibel { $module }
sources.bible.work_bible = Bibel des Werks
sources.bible.license = Lizenz (laut conf):
sources.bible.no_license = keine Angabe in der conf
sources.bible.open_label = Bibel aufschlagen
sources.bible.of_work = (Werk)
sources.bible.reference = Stelle
sources.bible.reference_placeholder = z. B. Ps 139,23-24
sources.bible.open = Aufschlagen
sources.bible.search_label = Im Wortlaut suchen
sources.bible.wording = Wortlaut
sources.bible.wording_placeholder = Wörter im Bibeltext
sources.bible.search = Suchen
sources.bible.opened = Aufgeschlagene Stelle
sources.bible.chapter = Kapitel
sources.bible.prev_chapter = ← voriges Kapitel
sources.bible.next_chapter = nächstes Kapitel →
sources.bible.module_ref = gelesen in der Zählung des Moduls: { $module_ref }
sources.bible.ref_to_field = „{ $ref }“ in das Feld „Stelle“
sources.bible.no_text = (kein Text im Modul)
sources.bible.unmapped = (keine deutsche Stelle: { $unmapped })
sources.bible.use_verses = Verse verwenden
sources.bible.needs_section = Zitieren und „an Persona geben“ brauchen einen Abschnitt: die Literatur aus einem Abschnitt heraus öffnen.
sources.bible.question = Frage an die Persona
sources.bible.question_placeholder = Was sagt diese Stelle zu meinem Abschnitt?
sources.bible.cite = Zitieren (Vorschlag im Abschnitt)
sources.bible.to_persona = An Persona geben
sources.bible.commentaries = Kommentare
sources.bible.hits = Treffer
sources.bible.hits_for = Treffer für „{ $q }“
sources.bible.no_hits = Keine Treffer.
sources.bible.truncated = Nur die ersten Treffer (Grenze erreicht).
sources.bible.books = Bücher
sources.bible.book_chapter = Buch und Kapitel
sources.bible.book = Buch
sources.bible.open_chapter = Öffnen
sources.bible.chapters = { $chapters } Kapitel

# templates/fragments/literature_bibles.html
sources.bibles.bible = Bibel
sources.bibles.more_hits = Weitere Treffer auf der Bibelseite.
sources.bibles.bibles = Bibeln
sources.bibles.settings_sources = Einstellungen → Quellen
sources.bibles.work_bible = Bibel des Werks

# templates/fragments/library.html
sources.library.kind_quote = Zitat
sources.library.kind_core = Kernaussage
sources.library.kind_contradiction = Widerspruch
sources.library.kind_check = zu prüfen
sources.library.search_placeholder = Werk und Literatur durchsuchen …
sources.library.search_label = Werk und Literatur durchsuchen
sources.library.search_by = Suche nach
sources.library.mode_fts = Wortlaut (FTS)
sources.library.mode_rag = Bedeutung (RAG)
sources.library.search = Suchen
sources.library.rag_hits_for = Bedeutungs-Treffer für „{ $library_query }“
sources.library.same_fts = Dieselbe Suche im Wortlaut (FTS)
sources.library.fulltext = Volltext
sources.library.similarity = Ähnlichkeit { $percent } %
sources.library.found_by = · gefunden von { $found_by }
sources.library.weak = schwach
sources.library.no_hits = Keine Treffer.
sources.library.only_weak = Nur schwache Treffer.
sources.library.weak_hits =
    { $count } { $count ->
        [one] schwacher
       *[other] schwache
    } Treffer
sources.library.hits_for = Treffer für „{ $library_query }“
sources.library.revision = Revision { $rev }
sources.library.post = Beitrag
sources.library.quote_key = Zitat · @{ $key }
sources.library.highlight_key = Hervorhebung · @{ $key }
sources.library.title_key = Titel · @{ $key }
sources.library.in_source = in der Quelle
sources.library.no_quotes = Noch keine Zitate.
sources.library.proposal = Vorschlag
sources.library.document_view = Dokumentansicht
sources.library.no_titles = Noch keine Titel.
sources.library.no_matching = Keine passenden Titel.

# templates/websearch_evidence.html
sources.evidence.window_title = Belege der Websuche – Baruch
sources.evidence.title = Belege der Websuche
sources.evidence.back_group = Zurück
sources.evidence.to_section = Zum Abschnitt
sources.evidence.heading = Belege zu Beitrag { $post }
sources.evidence.intro = Die Websuche von @belege ruft jede Seite ab, bevor sie genannt wird. Baruch prüft jedes Zitat gegen den abgerufenen Text: „belegt“ heißt, der Wortlaut steht so auf der Seite. Nur belegte Zitate lassen sich in die Autorenbibliothek übernehmen (Titel, Adresse, Zitat, Hervorhebung).
sources.evidence.preferred = bevorzugte Quelle
sources.evidence.accessed = { $url } · abgerufen am { $accessed }
sources.evidence.quote = „{ $quote }“
sources.evidence.check = Prüfung:
sources.evidence.in_library = In der Bibliothek (Titel { $library_title })
sources.evidence.adopt = In die Bibliothek
sources.evidence.none = Zu diesem Beitrag gibt es keine Belege der Websuche.
