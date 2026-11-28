# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of the cover (Round 39a, task messages_1). Base catalog in English; locales/de/cover_msg.ftl has the same keys in
# the same order with the German wording.

# Measuring the cover from the typeset book (cover.rs).
cover_msg.measure.typeset_errors = Der Buchsatz hat { $errors } Fehler; ohne gesetzte Seiten gibt es kein Seitenmaß für den Umschlag (Meldungen unter /werke/{ $work }/satz).
cover_msg.measure.page_size = Seitengröße der ersten Buchseite nicht lesbar
cover_msg.measure.block_heights = Blockhöhen: { $error }
cover_msg.measure.timeout = Die Umschlag-Blöcke lassen sich nicht setzen: Typst wurde nach { $seconds } Sekunden nicht fertig (Zeitgrenze).
cover_msg.measure.blocks_failed = Die Umschlag-Blöcke lassen sich nicht setzen: { $message }
cover_msg.measure.page_format = das Seitenformat der Werkvorlage ließ sich nicht setzen (Satzfehler in der Vorlage)

# Checks of work.meta.set and cover.* (RPC and forms).
cover_msg.param.text = { $name }: Text erwartet
cover_msg.param.switch = { $name }: an/aus (true/false) erwartet
cover_msg.param.mm = { $label }: Zahl in mm erwartet (z. B. 0,1).
cover_msg.param.paper = Papierdicke
cover_msg.param.paper_sheet = Papierdicke je Blatt
cover_msg.param.paper_both = paper_mm und paper_sheet_mm nicht zusammen angeben (paper_sheet_mm = Dicke je Blatt = 2 × paper_mm).
cover_msg.param.paper_sheet_range = Papierdicke je Blatt: mehr als 0 und höchstens 2 mm (1 Blatt = 2 Seiten).
cover_msg.param.author_photo = author_photo: Bildname oder null erwartet
cover_msg.param.title_area = title_area: „top“, „middle“ oder „bottom“ erwartet, nicht „{ $area }“
cover_msg.param.bleed = Beschnitt
cover_msg.param.name = name: Bildname oder null erwartet
cover_msg.param.name_missing = name fehlt: Bildname des Umschlags oder null zum Entfernen
cover_msg.imprint.not_adopted = Impressum-Vorlage nicht übernommen: { $why }
cover_msg.imprint.complete = Impressum-Vorlage nicht übernommen: Die Werkvorlage hat schon alles.

# Cover page.
cover_msg.page.template_error = Vorlage { $name }: { $error }
cover_msg.page.error = Fehler: { $error }
cover_msg.page.unknown_work = unbekanntes Werk
cover_msg.page.not_typeset = Der Umschlag ließ sich nicht setzen.
cover_msg.page.no_pdf = Kein Umschlag-PDF: { $error }
cover_msg.notice.cover_set = Der Umschlag ist gesetzt.
cover_msg.notice.cover_removed = Der Umschlag ist entfernt.
cover_msg.notice.book_details = Die Buchangaben sind gespeichert.
cover_msg.notice.layout = Die Umschlag-Anordnung ist gespeichert.
cover_msg.notice.imprint = Die Impressum-Vorlage ist in die Werkvorlage übernommen (neue Fassung der Vorlage).

# Fields of a cover block and their checks.
cover_msg.block_field.width = Breite
cover_msg.block_field.align = Ausrichtung
cover_msg.block_field.size = Schriftgröße
cover_msg.block_field.leading = Zeilenabstand
cover_msg.block_field.color = Farbe
cover_msg.block_field.font = Schrift
cover_msg.block_field.style = Schnitt
cover_msg.block_field.backdrop = Hintergrund
cover_msg.block_field.visible = Sichtbar
cover_msg.block_field.direction = Laufrichtung
cover_msg.block_field.text = Text
cover_msg.block_field.other = Feld
cover_msg.block.bounds = { $low } und { $high }
cover_msg.block.number_range = Zahl zwischen { $bounds } { $unit } erwartet, nicht „{ $raw }“.
cover_msg.block.align = „left“, „center“, „right“ oder „justify“ erwartet, nicht „{ $raw }“.
cover_msg.block.style = „normal“, „bold“ oder „italic“ erwartet, nicht „{ $raw }“.
cover_msg.block.backdrop = „none“, „shadow“ oder „gradient“ erwartet, nicht „{ $raw }“.
cover_msg.block.color = „light“, „dark“ oder #rrggbb erwartet, nicht „{ $raw }“.
cover_msg.block.font = Schriftname aus Buchstaben, Ziffern, Leerzeichen und - _ . (höchstens { $max } Zeichen) erwartet.
cover_msg.block.switch = an/aus (1/0) erwartet, nicht „{ $raw }“.
cover_msg.block.direction = „down“ (von oben nach unten) oder „up“ erwartet, nicht „{ $raw }“.
cover_msg.block.spine_only = gibt es nur auf dem Rücken.
cover_msg.block.text_too_long = höchstens { $max } Zeichen.
cover_msg.block.text_from_work = Dieser Block nimmt seinen Text aus Werk und Buchangaben.
cover_msg.block.unknown_field = { $block }: unbekanntes Feld „{ $field }“.
cover_msg.block.unknown_kind = Unbekannte Blockart „{ $kind }“ im Feld „{ $name }“.
cover_msg.block.no_base = { $block }: kein Ausgangsblock.

# Guides and labels of the cover editor.
cover_msg.guide.bleed = Beschnitt (Außenkante)
cover_msg.guide.trim = Schnittkante
cover_msg.guide.safety = Sicherheitsabstand { $mm } mm ({ $face })
cover_msg.guide.spine_left = Rückenkante links
cover_msg.guide.spine_right = Rückenkante rechts
cover_msg.guide.middle_back = Mitte der Rückseite
cover_msg.guide.middle_front = Mitte der Vorderseite
cover_msg.guide.middle_height = Mitte der Höhe
cover_msg.editor.block_aria = { $label }: x { $x } mm, y { $y } mm, Breite { $width } mm
cover_msg.editor.block_aria_hidden = { $label }: x { $x } mm, y { $y } mm, Breite { $width } mm, verborgen

# Forms of the cover.
cover_msg.form.work_missing = Fehler: Werk fehlt.
cover_msg.form.choose_image = Bitte ein Bild für den Umschlag wählen.
cover_msg.form.unknown_action = unbekannte Aktion
cover_msg.form.no_blocks = Keine Blöcke im Formular (Felder b.{ $lt }Art{ $gt }.{ $lt }Feld{ $gt }).
cover_msg.form.quick_area = Schnellwahl: „top“, „middle“ oder „bottom“ erwartet, nicht „{ $area }“.
cover_msg.form.faces = Flächen: „front“, „spine“ oder „back“ erwartet, nicht „{ $word }“.

# Cover layout (cover_layout.rs).
cover_msg.layout.not_object = Umschlag-Block Nr. { $number }: ein Objekt mit kind, x_mm, … erwartet
cover_msg.layout.unknown_kind = Umschlag-Block Nr. { $number }, Feld kind: unbekannte Art „{ $word }“ (erlaubt: { $allowed })
cover_msg.layout.kind_text = Umschlag-Block Nr. { $number }, Feld kind: Art als Text erwartet ({ $allowed })
cover_msg.layout.block_label = Umschlag-Block „{ $label }“ ({ $kind })
cover_msg.layout.field = { $label }, Feld { $field }: { $what }
cover_msg.layout.unknown_field = unbekanntes Feld
cover_msg.layout.missing = fehlt
cover_msg.layout.number = Zahl erwartet
cover_msg.layout.allowed = „{ $value }“ – erlaubt: { $allowed }
cover_msg.layout.text_allowed = Text erwartet ({ $allowed })
cover_msg.layout.text_or_null = Text oder null erwartet
cover_msg.layout.color = Text erwartet (light, dark oder #rrggbb)
cover_msg.layout.flag = true oder false erwartet
cover_msg.layout.direction = down, up oder null erwartet
cover_msg.layout.blocks_list = blocks: Liste von Blöcken erwartet (je {"{"}kind, x_mm, y_mm, width_mm, …{"}"})
cover_msg.layout.stale = Die Umschlag-Anordnung wurde inzwischen geändert; der Antrag bezog sich auf den Stand davor.
cover_msg.layout.not_executable = Nicht mehr ausführbar: { $error }
cover_msg.layout.fonts = Schriften: { $error }
cover_msg.layout.not_layout_request = Dieser Antrag betrifft nicht die Umschlag-Anordnung.
cover_msg.layout.faces = faces: Liste aus „front“, „spine“, „back“ erwartet
cover_msg.layout.area_not = area: „top“, „middle“ oder „bottom“ erwartet, nicht „{ $word }“
cover_msg.layout.area = area: „top“, „middle“ oder „bottom“ erwartet

# Print check of the cover (cover_print.rs).
cover_msg.print.draft = Das Umschlag-PDF trägt das Wasserzeichen „Entwurf“ – so nicht an die Druckerei geben (ohne Wasserzeichen: Umschlag-PDF mit ?entwurf=0 oder „Entwurf auch auf dem Umschlag“ ausschalten).
cover_msg.print.font_missing = { $block }: Schrift „{ $font }“ fehlt; gesetzt mit der Vorgabe „{ $default }“.
cover_msg.print.spine = { $who } fällt aus dem Rücken: { $w } dick ab x { $x }, der Rücken ist { $fw } breit.
cover_msg.print.bleed_spine = { $who } ragt oben oder unten in den Beschnitt (y { $y } bis { $y2 }).
cover_msg.print.safety_spine = { $who } liegt näher als { $safety } an der Schnittkante oben oder unten.
cover_msg.print.bleed = { $who } ragt über die Schnittkante bzw. den Falz in den Beschnitt (x { $x } bis { $x2 }, y { $y } bis { $y2 }; Fläche { $fw } × { $fh }).
cover_msg.print.safety = { $who } liegt näher als { $safety } an der Schnittkante bzw. am Falz.
cover_msg.print.barcode = { $who } überdeckt den Strichcode.
cover_msg.print.contrast = { $who }: Kontrast { $ratio } : 1 zum Hintergrund ist zu gering (mindestens { $min } : 1); hellere bzw. dunklere Schrift, Schatten oder Verlauf wählen.
cover_msg.print.imprint_publisher = Impressum: Verlag bzw. Herausgeber oder dessen Anschrift fehlt (bei Selbstverlag Name und Anschrift des Verfassers bzw. Herausgebers).
cover_msg.print.imprint_printer = Impressum: Druckerei oder deren Anschrift fehlt.
cover_msg.print.imprint_year = Impressum: Erscheinungsjahr fehlt.
cover_msg.print.imprint_bible_notice = Impressum: Eine Bibelübersetzung ist gesetzt, aber der Rechtehinweis der Übersetzung fehlt.
