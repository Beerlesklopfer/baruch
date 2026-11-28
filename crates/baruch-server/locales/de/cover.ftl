# SPDX-License-Identifier: AGPL-3.0-or-later
# Round 39a (templates_2): Cover editor page and its panel.
# German translation of locales/en/cover.ftl (same keys in the same order).

# Shared by the cover page and its panel.
cover.back_to_work = Zurück zum Werk
cover.image_alt = Umschlag: Rückseite, Rücken und Vorderseite
cover.area.top = oben
cover.area.middle = Mitte
cover.area.bottom = unten

# Page /werke/{id}/umschlag (cover.html).
cover.page.window_title = Umschlag – { $title } – Baruch
cover.page.page_title = { $title }: Umschlag
cover.page.download = Herunterladen
cover.page.pdf = Umschlag-PDF
cover.page.heading = Umschlag
cover.page.facts =
    Gesamt { $width } × { $height } mm · Seite { $page_width } × { $page_height } mm · Rücken { $spine } mm = { $book_pages } { $book_pages ->
        [one] Buchseite
       *[other] Buchseiten
    } ({ $sheets } Blatt) × { $sheet_mm } mm je Blatt
cover.page.facts_cover_pages = (die { $count } Umschlagseiten im Satz zählen nicht)
cover.page.facts_bleed = · Beschnitt { $bleed } mm
cover.page.caption = Links die Rückseite, in der Mitte der Rücken, rechts die Vorderseite; außen der Beschnitt.
cover.page.print_check = Druckprüfung
cover.page.line = , Zeile { $line }
cover.page.no_messages = Keine Meldungen beim Satz des Umschlags.

# Cover editor (cover.html, section "anordnung").
cover.editor.heading = Anordnung der Blöcke
cover.editor.preview_notice = Vorschau – noch nicht gespeichert. „Anordnung übernehmen“ speichert diese Werte.
cover.editor.help = Koordinaten in mm ab der Schnittkante (oben links) der eigenen Fläche; negative Werte ragen in den Beschnitt.
cover.editor.help_js = Rahmen ziehen verschiebt, die Griffe an den Ecken ändern die Breite. Tastatur: Rahmen mit Tab wählen, Pfeiltasten 0,5 mm (mit Umschalt 5 mm), Strg+←/→ Breite, Strg+Z rückgängig, Strg+S speichern. Beim Ziehen rastet der Rahmen an den Hilfslinien ein.
cover.editor.toolbar = Umschlag-Editor
cover.editor.undo = Rückgängig
cover.editor.tone_hint = Farbe des gewählten Blocks zwischen hell und dunkel umschalten
cover.editor.tone = Hell/Dunkel
cover.editor.preview_mark = (Vorschau)
cover.editor.block_role = verschiebbarer Block
cover.editor.own_layout = Das Werk hat eine eigene Anordnung.
cover.editor.defaults_before = Noch keine eigene Anordnung: Es gelten die Vorgaben (Titelbereich
cover.editor.defaults_after = ).
cover.editor.default_font = Vorgabeschrift: { $font }.
cover.editor.default_mark = (Vorgabe)
cover.editor.hidden_mark = – verborgen
cover.editor.width = Breite (mm)
cover.editor.size = Schriftgröße (pt)
cover.editor.leading = Zeilenabstand (em)
cover.editor.align = Ausrichtung
cover.editor.align_left = links
cover.editor.align_right = rechts
cover.editor.align_justify = Blocksatz
cover.editor.color = Farbe
cover.editor.color_placeholder = light, dark oder #rrggbb
cover.editor.font = Schrift
cover.editor.style = Schnitt
cover.editor.style_normal = normal
cover.editor.style_bold = fett
cover.editor.style_italic = kursiv
cover.editor.backdrop = Hintergrund
cover.editor.backdrop_none = keiner
cover.editor.backdrop_shadow = Schatten
cover.editor.backdrop_gradient = Verlauf
cover.editor.visible = Sichtbar
cover.editor.yes = ja
cover.editor.no = nein
cover.editor.direction = Laufrichtung
cover.editor.direction_down = von oben nach unten
cover.editor.direction_up = von unten nach oben
cover.editor.text = Text
cover.editor.apply = Anordnung übernehmen
cover.editor.preview_without_saving = Vorschau ohne Speichern
cover.editor.quick = Schnellwahl Titelbereich (setzt die Vorderseite zurück)
cover.editor.reset_legend = Anordnung zurücksetzen (keine Fläche gewählt = alle)
cover.editor.reset = Anordnung zurücksetzen
cover.editor.unavailable = Ohne gesetzten Umschlag kein Editor: { $reason }

# Group "Umschlag" (fragments/cover_panel.html).
cover.panel.image_html = Umschlagbild: <strong>{ $name }</strong>
cover.panel.no_image = Noch kein Umschlagbild. Wähle eins aus den Bildern des Werks.
cover.panel.image_for_cover = Bild für den Umschlag
cover.panel.set_cover = Als Umschlag setzen
cover.panel.no_images = Das Werk hat noch keine Bilder:
cover.panel.upload_images = Bilder hochladen …
cover.panel.remove = Umschlag entfernen
cover.panel.area_legend = Titel und Untertitel auf der Vorderseite
cover.panel.apply_area = Titelbereich übernehmen
cover.panel.view = Umschlag ansehen (Rückseite, Rücken, Vorderseite) …
cover.panel.imprint_adopt = Impressum-Vorlage in die Werkvorlage übernehmen
cover.panel.imprint_hint = Hängt die Funktionen schmutztitel, haupttitel, impressum und widmung, die der Werkvorlage noch fehlen, als neue Fassung der Vorlage an; danach lassen sich Titelei und Impressum in der Werkvorlage frei gestalten.
cover.panel.imprint_complete = Die Werkvorlage hat schon alle vier.
cover.panel.imprint_present = Schon vorhanden: { $present }.
cover.panel.imprint_none = Die Werkvorlage hat noch keine davon.

# Form "Buchangaben" (fragments/cover_panel.html).
cover.book.heading = Buchangaben
cover.book.intro = Für Titelei, Impressum und Umschlag. Titel, Untertitel, Autor und Bibelübersetzung stehen bei den Angaben des Werks.
cover.book.publisher = Verlag
cover.book.place = Verlagsort
cover.book.year = Jahr
cover.book.edition = Auflage
cover.book.edition_placeholder = 1. Auflage
cover.book.price = Preis
cover.book.series = Reihe
cover.book.printer = Druck
cover.book.author_photo = Autorenfoto
cover.book.none = – keins –
cover.book.paper = Papierdicke je Blatt in mm (1 Blatt = 2 Seiten)
cover.book.paper_hint = Bestimmt die Rückenbreite. Druckereien nennen die Dicke je Blatt. Vorgabe 0,2 mm je Blatt (eher dick); übliches Werkdruckpapier um 80–90 g/m² hat etwa 0,1 mm je Blatt.
cover.book.bleed = Beschnitt (mm)
cover.book.title_area = Titelbereich
cover.book.front_matter = Titelei setzen (Schmutztitel, Seite 2, Haupttitel, Impressum, Widmung)
cover.book.cover_pages = Umschlag-Vorder- und Rückseite im Satz zeigen (nur mit Umschlagbild)
cover.book.dedication = Widmung
cover.book.blurb = Klappentext (Rückseite)
cover.book.bio = Kurzbiografie
cover.book.publisher_address = Anschrift des Verlags bzw. Herausgebers
cover.book.publisher_address_hint = (bei Selbstverlag die des Verfassers oder Herausgebers; mehrzeilig)
cover.book.printer_address = Anschrift der Druckerei
cover.book.multiline = (mehrzeilig)
cover.book.bible_notice = Rechtehinweis der Bibelübersetzung
cover.book.image_credit = Bildnachweis / Umschlaggestaltung
cover.book.image_credit_hint = (leer: Bildunterschrift des Umschlags)
cover.book.save = Buchangaben speichern
