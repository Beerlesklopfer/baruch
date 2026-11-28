# SPDX-License-Identifier: AGPL-3.0-or-later
# Area "outline" (Round 39a, templates_1), German translation of locales/en/outline.ftl (same keys in the same
# order; the wording of the templates before Round 39a).

# templates/outline.html
outline.page.findings_one = { $count } offener Befund
outline.page.findings_other = { $count } offene Befunde
outline.page.head_title = Gliederung – { $title } – Baruch
outline.page.page_title = Gliederung · { $title }
outline.page.back = Zurück
outline.page.back_to_work = Zurück zum Werk
outline.page.heading = Gliederung von „{ $title }“
outline.page.intro = Diese Seite ist der Rückfall ohne JavaScript; mit JavaScript geschieht alles im Baum links (Ziehen und Ablegen, Kontextmenü, Doppelklick bzw. F2 zum Umbenennen). Jede Änderung ist ein Commit im Git-Spiegel. Ändern sich Ebene oder Titel, bekommt der Abschnitt eine neue Fassung, die sich nur in der Überschriftszeile unterscheidet. Assistenten legen höchstens neue Punkte an; Umbenennen, Verschieben, Umstufen und Löschen beantragen sie nur – das entscheiden Sie unten bei den Anträgen.
outline.page.total = Gesamtzeit
outline.page.total_words =
    ({ $words } { $words ->
        [one] Wort
       *[other] Wörter
    }, { $words_per_minute } Wörter je Minute)
outline.page.target = · Zielzeit
outline.page.over = Warnung: Zielzeit um { $over_clock } überschritten
outline.page.within = · im Rahmen
outline.page.col_number = Nummer
outline.page.col_title = Titel
outline.page.col_level = Ebene
outline.page.col_duration = Dauer
outline.page.col_arrange = Umstellen
outline.page.col_count = Zählen, Bearbeiten
outline.page.findings_children = samt Unterpunkten
outline.page.findings_own = ({ $findings_own } am Punkt selbst)
outline.page.findings_open = { $count } offen
outline.page.words =
    { $words } { $words ->
        [one] Wort
       *[other] Wörter
    }
outline.page.start_clock = ab { $start_clock }
outline.page.before = Vor „{ $up_title }“
outline.page.up = Nach oben
outline.page.after = Hinter „{ $down_title }“
outline.page.down = Nach unten
outline.page.promote = Höher stufen
outline.page.demote = Tiefer stufen
outline.page.unnumber_title = Ohne Nummer zählen (z. B. Vorwort, Einleitung)
outline.page.number_title = Wieder mit Nummer zählen
outline.page.unnumbered = ohne Nummer
outline.page.numbered = mit Nummer
outline.page.edit = Bearbeiten
outline.page.requests = Anträge der Assistenten
outline.page.requests_open = ({ $count } offen)
outline.page.requests_help = Bestätigen führt den Antrag aus (ein Commit im Git-Spiegel, Sie als Bestätiger, der Assistent als Herkunft). Hat sich der Punkt inzwischen geändert, wird der Antrag nicht ausgeführt, sondern als veraltet abgeschlossen. Ablehnen ändert nichts an der Gliederung. Ein neuer Teil, unter den vorhandene Kapitel (in einer Predigt: Predigtteile) wandern würden, braucht Ihre Zustimmung („Zustimmen“); auch Ablehnen steht dann im Git-Spiegel.
outline.page.col_no = Nr.
outline.page.col_request = Antrag
outline.page.col_from = Von
outline.page.col_reason = Begründung
outline.page.show_preview = Vorschau anzeigen
outline.page.preview_lazy = (wird erst auf Anforderung berechnet)
outline.page.preview = Vorschau
outline.page.preview_error = Keine Vorschau: { $preview_error }
outline.page.preview_changed = (vorher { $before_label } → nachher { $label })
outline.page.mark_renamed = umbenannt
outline.page.mark_moved = verschoben
outline.page.mark_level = neue Ebene
outline.page.mark_deleted = entfällt
outline.page.mark_new = neu
outline.page.mark_under = wandert unter den neuen Teil
outline.page.agree = Zustimmen
outline.page.confirm = Bestätigen
outline.page.reject = Ablehnen
outline.page.no_requests = Keine offenen Anträge.
outline.page.decided = Zuletzt entschieden
outline.page.decided_item = Antrag { $id }: { $summary } ({ $origin_label }) –
outline.page.numbering = Nummerierung
outline.page.now = Jetzt:
outline.page.pattern_effective = Muster „{ $effective }“
outline.page.no_numbers = keine Nummern
outline.page.template_none = (die Werkvorlage setzt keins – Vorgabe „1.1“)
outline.page.numbering_rule = . Teile zählen eigen, Kapitel über Teile hinweg fortlaufend.
outline.page.choose_scheme = Schema wählen…
outline.page.from_template = Aus der Vorlage
outline.page.pattern = Muster
outline.page.set = Setzen
outline.page.no_numbers_button = Keine Nummern
outline.page.pattern_help = Muster wie in Typst: Zählzeichen 1, a, A, i, I mit Text davor und dazwischen, z. B. „1.1“, „I.1“, „1.a)“; „I|1“ nummeriert nur die Kapitel, „I.;A.;1.;a)“ gibt jeder Ebene ihr eigenes Muster.

# templates/outline_confirm.html
outline.confirm.head_title = Speichern bestätigen – Baruch
outline.confirm.page_title = Speichern bestätigen · { $title }
outline.confirm.split = Der Text enthält neue Überschriften. Beim Speichern wird „{ $title }“ geteilt; neu entstehen:
outline.confirm.merge = Der Text hat keine Überschrift mehr. Beim Speichern wird er an „{ $previous }“ angehängt; „{ $title }“ entfällt (Beiträge wandern mit, der alte Text steht im Git-Spiegel).
outline.confirm.split_save = Teilen und speichern
outline.confirm.merge_save = Zusammenführen und speichern
outline.confirm.back = Zurück
outline.confirm.back_note = „Zurück“ verwirft den Text; er ist dann nicht gespeichert.

# templates/outline_edit.html
outline.edit.head_title = { $title } bearbeiten – Gliederung – Baruch
outline.edit.page_title = Gliederungspunkt bearbeiten · { $title }
outline.edit.back = Zurück
outline.edit.outline = Gliederung
outline.edit.back_to_work = Zurück zum Werk
outline.edit.rename = Umbenennen
outline.edit.new_title = Neuer Titel
outline.edit.title_rule = Höchstens 200 Zeichen, nicht leer
outline.edit.rename_part = Die Teil-Markierung im Text bekommt den neuen Titel (neue Fassung, Kommentar bleibt); ein leerer Teil bekommt sie neu. Ein Teiltext ohne Markierung (älterer Bestand) bleibt unverändert, nur der Titel ändert sich.
outline.edit.rename_heading = Die Überschriftszeile im Text bekommt den neuen Titel (neue Fassung, Label und Kommentar bleiben).
outline.edit.level = Ebene
outline.edit.set_level = Ebene ändern
outline.edit.level_note = Unterpunkte wandern um dieselbe Zahl Ebenen mit.
outline.edit.move = Verschieben
outline.edit.before = vor
outline.edit.after = hinter
outline.edit.under = unter
outline.edit.target = Ziel
outline.edit.move_note_html = Mit allen Unterpunkten. Die Ebene bleibt: vor/hinter nur einen Punkt derselben Ebene, unter nur einen Punkt eine Ebene höher; sonst erst die Ebene ändern (oben). Ungültige Ziele werden abgewiesen.
outline.edit.new = Neuer Gliederungspunkt
outline.edit.title = Titel
outline.edit.add_before = davor
outline.edit.add_after = danach
outline.edit.add_under = darunter
outline.edit.like_place = wie der Ort
outline.edit.create = Anlegen
outline.edit.add_note_html = Davor/danach auf derselben Ebene, darunter eine Ebene tiefer als letzter Unterpunkt.
outline.edit.add_part_note = Mit Ebene „{ $level_names }“ entsteht davor bzw. danach ein neuer Teil (auch mit Werkvorlage); die folgenden Kapitel bis zum nächsten Teil stehen dann unter ihm, ihre Texte und Nummern bleiben.
outline.edit.has_children = „{ $title }“ hat Unterpunkte; erst verschieben oder löschen.
outline.edit.delete_confirm = „{ $title }“ mit allen Fassungen und Beiträgen wirklich löschen
outline.edit.delete_note = Der Text der aktuellen Fassung steht danach vollständig im Commit des Git-Spiegels.

# templates/outline_retry.html
outline.retry.head_title = Nicht gespeichert – Baruch
outline.retry.page_title = Nicht gespeichert · { $title }
outline.retry.failed = Nicht gespeichert: { $message }
outline.retry.your_text = Ihr Text (noch nicht gespeichert)
outline.retry.again = Erneut speichern
outline.retry.back = Zurück ohne Speichern
outline.retry.back_note = „Zurück ohne Speichern“ verwirft den Text; er ist dann nicht gespeichert.

# Round 39b (rest_literals): names and examples of the outline schemes of baruch-typst (scheme.rs),
# translated by the server (numbering.rs); the German text is verbatim the label of the crate.
outline.scheme.template = Aus der Werkvorlage
outline.scheme.template_example = wie #set heading(numbering: …) der Werkvorlage, sonst 1 · 1.1
outline.scheme.decimal = Dezimal (DIN 1421)
outline.scheme.decimal_example = 1 · 1.1 · 1.1.1
outline.scheme.decimal_dot = Dezimal mit Schlusspunkt
outline.scheme.decimal_dot_example = 1. · 1.1. · 1.1.1.
outline.scheme.alphanumeric = Alphanumerisch
outline.scheme.alphanumeric_example = I. · A. · 1. · a) · aa)
outline.scheme.chapter_roman = Nur Kapitel römisch
outline.scheme.chapter_roman_example = I · II · III, Abschnitte ohne Nummer
outline.scheme.chapter_arabic = Nur Kapitel arabisch
outline.scheme.chapter_arabic_example = 1 · 2 · 3, Abschnitte ohne Nummer
outline.scheme.parts_chapters = Teile und Kapitel
outline.scheme.parts_chapters_example = Teil I · Kapitel 1 · 1.1 · 1.1.1
outline.scheme.chapter_words = Kapitel ausgeschrieben
outline.scheme.chapter_words_example = Erster Teil · Erstes Kapitel · Zweites Kapitel
outline.scheme.legal = Juristisch
outline.scheme.legal_example = § 1 · Abs. 1 · Nr. 1
outline.scheme.sermon = Predigt
outline.scheme.sermon_example = Einleitung · I. · II. · Schluss
outline.scheme.book = Buch
outline.scheme.book_example = Einleitung · I. · II. · Schluss, Teile „Erster Teil“
outline.scheme.none = Ohne Nummern
outline.scheme.none_example = keine Nummern
outline.scheme.custom = Eigenes Muster
outline.scheme.custom_example = Mustersprache, z. B. I.;A.;1.;a) oder I|1
outline.scheme.no_numbers = keine
outline.scheme.custom_missing = Für „Eigenes Muster“ fehlt das Muster, z. B. „I.;A.;1.“ oder „I|1“.
