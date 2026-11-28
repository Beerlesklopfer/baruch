# SPDX-License-Identifier: AGPL-3.0-or-later
# Area "search" (Round 39a, templates_1), German translation of locales/en/search.ftl (same keys in the same
# order; the wording of the templates before Round 39a).

# templates/fragments/search_bar.html
search.bar.label = Im Abschnitt suchen
search.bar.expand_html = Ersetzen ein-/ausblenden (Strg+H)
search.bar.query = Suchen
search.bar.case_html = Groß/klein beachten (Alt+C)
search.bar.word = Nur ganze Wörter (Alt+W)
search.bar.regex = Regulärer Ausdruck (Alt+R)
search.bar.selection = Nur in der Markierung suchen (Alt+L; die Markierung im Quelltext beim Öffnen der Suche)
search.bar.no_hits = Keine Treffer
search.bar.prev = Voriger Treffer (Umschalt+Enter, Umschalt+F3)
search.bar.next = Nächster Treffer (Enter, F3)
search.bar.close = Schließen (Escape)
search.bar.close_label = Suchleiste schließen
search.bar.replace = Ersetzen
search.bar.replace_one = Aktuellen Treffer ersetzen (Strg+Umschalt+1)
search.bar.replace_all_title = Alle im Abschnitt ersetzen (Strg+Alt+Enter)
search.bar.replace_all = Alle
search.bar.work_title = Im ganzen Werk suchen und ersetzen (Strg+Umschalt+F)
search.bar.work = im Werk …
search.bar.section_page = Im Abschnitt suchen und ersetzen

# templates/search.html
search.page.head_title = Suchen und ersetzen – { $title } – Baruch
search.page.page_title = { $title }: Suchen und ersetzen
search.page.back = Zurück
search.page.back_to_work = Zurück zum Werk
search.page.heading = Im Werk suchen und ersetzen
search.page.undone_batch = Ersetzen „{ $query }“ → „{ $replacement }“ wurde rückgängig gemacht.
search.page.replaced = Ersetzt:
search.page.replaced_in = „{ $query }“ → „{ $replacement }“ in { $count }
search.page.sections_one = Abschnitt
search.page.sections_other = Abschnitten
search.page.replaced_note = (je eine neue Fassung von dir, ein Commit im Spiegel).
search.page.undo = Rückgängig
search.page.check_quotes = Bitte den Wortlaut der Bibelzitate prüfen (Prüfanzeige):
search.page.undone = Rückgängig gemacht: Die Abschnitte stehen wieder auf ihrer vorigen Fassung.
search.page.query = Suchen
search.page.replacement = Ersetzen durch
search.page.case_title_html = Groß/klein beachten (Alt+C)
search.page.case_html = Groß/klein
search.page.word_title = Nur ganze Wörter (Alt+W)
search.page.word = Ganzes Wort
search.page.regex_title = Regulärer Ausdruck (Alt+R)
search.page.regex = Regulärer Ausdruck
search.page.range = Bereich
search.page.outline = Gliederung (für „Abschnitt“, „Kapitel“, „Teil“, „ab Abschnitt“ den Punkt wählen, für „Auswahl“ Häkchen setzen)
search.page.pick_section = als Abschnitt wählen
search.page.picked_section = (gewählter Abschnitt)
search.page.rev = Fassung (nur Abschnitt und Markierung)
search.page.rev_current = aktuelle Fassung
search.page.rev_shown = gezeigte Fassung { $shown_rev }
search.page.rev_note = Vorschau und Ersetzen gelten immer der aktuellen Fassung.
search.page.kinds = Textarten
search.page.kinds_note = Beim Ersetzen bleiben Zitate (auch Bibelzitate) und Typst-Code unberührt, außer du wählst Zitate ausdrücklich.
search.page.preview_button = Vorschau des Ersetzens
search.page.hits_one = Treffer
search.page.hits_other = Treffer
search.page.truncated = Es werden { $shown } von { $total } Treffern gezeigt.
search.page.timed_out = Die Suche brauchte länger als 2 s und wurde abgebrochen; die Liste ist unvollständig.
search.page.no_hits = Keine Treffer.
search.page.preview_heading = Vorschau: „{ $q }“ → „{ $replacement }“ · { $total } Treffer · { $range_label }
search.page.preview_timed_out = Die Suche brauchte länger als 2 s; Ersetzen geht erst mit kleinerem Bereich.
search.page.preview_truncated = Die Vorschau zeigt nicht alle Abschnitte (höchstens 500 ganze Abschnitte mit zusammen höchstens 2000 Stellen); bitte Suche oder Bereich eingrenzen.
search.page.preview_rev = Fassung { $base_rev }, { $count }
search.page.places_one = Stelle
search.page.places_other = Stellen
search.page.open = öffnen
search.page.places = Stellen:
search.page.confirm = Ja, ersetzen (je Abschnitt eine neue Fassung von mir)
search.page.replace_all = Alle ersetzen
search.page.nothing = Nichts zu ersetzen.
search.page.latest = Letztes Ersetzen: „{ $query }“ → „{ $replacement }“ in { $count }
search.page.keys_help_html = Gesucht wird im Typst-Quelltext der Fassungen. Tasten wie in VS Code: Strg+F suchen und Strg+H ersetzen im Abschnitt, Strg+Umschalt+F bzw. Strg+Umschalt+H im Werk, Enter/F3 nächster Treffer, Alt+C/W/R Groß/klein, ganzes Wort, regulärer Ausdruck.
