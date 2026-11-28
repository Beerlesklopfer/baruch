# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of the Typst import (src/import.rs) and of the Markdown, ZIP and folder import (src/markdown_import/**):
# refusals, skipped images, notes of the report, the report of the command line (Round 39c, task reste_1).
# The command line imports run in the German interface language (as the other CLI imports since Round 39a).

# Typst import and export into a folder (src/import.rs).
markdown_msg.typst.only_typ = Nur Typst-Dateien (.typ) lassen sich importieren.
markdown_msg.typst.title_hint = Mit --title "…" einen anderen Titel wählen.
markdown_msg.typst.back_matter_cut = Das erzeugte Buchende eines Exports (Register- und Glossarseiten ab „// Baruch: Register und Glossar“) ist abgeschnitten; es entsteht beim Export aus den Satz-Einstellungen neu.
markdown_msg.typst.back_matter_kept = Die Zeile „// Baruch: Register und Glossar“ steht im Text, aber ohne das erzeugte Buchende dahinter; nichts abgeschnitten.
markdown_msg.typst.projection_removed = Zusätze der Projektion eines Exports (automatische Bibelstellen-Marken, Auszeichnung der ersten Erwähnung) sind entfernt; sie entstehen beim Export aus den Einstellungen neu.
markdown_msg.typst.typeset_adopted = Der Regelblock „// Baruch: Satz“ ist in die Satz-Einstellungen des Werks übernommen (Einstellungen → Werk → Satz), nicht in die Werkvorlage.
markdown_msg.typst.typeset_kept = Der Regelblock „// Baruch: Satz“ ließ sich nicht genau in Satz-Einstellungen zurücklesen (von Hand geändert?) und steht weiter in der Werkvorlage.
markdown_msg.typst.read_aborted = Lesen von { $path } abgebrochen: { $detail }
markdown_msg.file.too_large = { $file } ist größer als 16 MiB
markdown_msg.file.too_large_mib = { $file } ist größer als { $mib } MiB
markdown_msg.file.unreadable = { $file } lässt sich nicht lesen: { $detail }
markdown_msg.file.not_file = { $file } ist keine Datei
markdown_msg.file.not_utf8 = { $file } ist kein UTF-8-Text
markdown_msg.file.no_name = { $file }: Dateiname fehlt oder ist kein UTF-8
markdown_msg.asset.folder = Ordner der Quelldatei nicht lesbar ({ $detail })
markdown_msg.asset.not_path = kein Dateipfad
markdown_msg.asset.not_found = nicht gefunden ({ $detail })
markdown_msg.asset.outside = liegt außerhalb des Ordners der Quelldatei
markdown_msg.asset.unreadable = nicht lesbar ({ $detail })
markdown_msg.asset.not_file = keine Datei
markdown_msg.asset.too_large = größer als 8 MiB
markdown_msg.asset.total = die Bilder dieser Datei sind zusammen größer als 64 MiB
markdown_msg.asset.not_image = kein unterstütztes Bild (PNG, JPEG, GIF, WebP, SVG)
markdown_msg.export.asset_path = Bildpfad { $path } ist kein relativer Pfad ohne ..; nichts geschrieben
markdown_msg.export.twice = { $path } käme zweimal vor; nichts geschrieben
markdown_msg.export.aborted = Schreiben abgebrochen: { $detail }
markdown_msg.export.not_folder = { $path } ist kein Ordner; nichts geschrieben
markdown_msg.export.file_and_folder = { $path } wäre zugleich Datei und Ordner (für { $other }); nichts geschrieben
markdown_msg.export.exists = { $path } gibt es schon; nichts überschrieben
markdown_msg.export.link = { $path } ist eine Verknüpfung; Bilder könnten außerhalb von { $out } landen; nichts geschrieben
markdown_msg.export.is_file = { $path } ist eine Datei, kein Ordner; nichts geschrieben
markdown_msg.export.create = { $path } lässt sich nicht anlegen: { $detail }
markdown_msg.export.read = { $path } lässt sich nicht lesen: { $detail }
markdown_msg.export.outside = { $path } liegt außerhalb von { $out }; abgebrochen
markdown_msg.export.write = { $path } lässt sich nicht schreiben: { $detail }

# Markdown, ZIP and folder (src/markdown_import/mod.rs).
markdown_msg.import.only_markdown = Nur Markdown-Dateien (.md, .markdown) lassen sich so importieren.
markdown_msg.import.not_sent = nicht mitgeschickt (nur die Markdown-Datei kam an; Ordner oder ZIP hochladen, um Bilder mitzunehmen)
markdown_msg.import.too_many_files = mehr als { $max } Dateien
markdown_msg.import.duplicate = doppelter Name (auch Groß-/Kleinschreibung zählt gleich)
markdown_msg.import.file_too_large = „{ $path }“ ist größer als 16 MiB
markdown_msg.import.total_too_large = alle Dateien zusammen größer als 80 MiB
markdown_msg.import.no_main = Keine Haupt-Datei (.typ, .md oder .markdown) gefunden.
markdown_msg.import.several_main = Mehrere mögliche Haupt-Dateien ({ $files }); genau eine .typ/.md-Datei oder eine index.md bzw. main.typ auf oberster Ebene erwartet.
markdown_msg.import.bad_part = „..“, „.“ oder leerer Teil im Pfad
markdown_msg.import.unused = { $count } Datei(en) nicht übernommen, weil die Haupt-Datei „{ $main }“ sie nicht nennt: { $files }
markdown_msg.import.address = kein Dateipfad (Adresse)
markdown_msg.import.no_file_name = kein Dateiname
markdown_msg.import.not_in_bundle = nicht gefunden (nicht im Archiv bzw. Ordner)
markdown_msg.import.failed = Import von { $file }: { $detail }

# Report of the command line (markdown_import::report_text).
markdown_msg.report.work = Werk: { $title } (id { $id })
markdown_msg.report.folder = Ordnername: { $slug }
markdown_msg.report.sections = Abschnitte: { $count }
markdown_msg.report.images = Bilder: { $count }
markdown_msg.report.skipped = Bild übersprungen: { $path } ({ $reason })
markdown_msg.report.note_line = Hinweis ({ $code }, Zeile { $line }): { $message }
markdown_msg.report.note = Hinweis ({ $code }): { $message }
markdown_msg.report.commit = Commit: { $commit }
markdown_msg.report.mirror_failed = Git-Spiegel nicht geschrieben (das Werk ist angelegt): { $error }
markdown_msg.report.no_commit = Commit: keiner

# Notes of the Markdown conversion (src/markdown_import/convert.rs).
markdown_msg.md.preamble = Text vor der ersten Überschrift steht jetzt im Kapitel „{ $chapter }“ (in Baruch wäre er sonst die Werkvorlage).
markdown_msg.md.footnote_unused = Fußnote „[^{ $label }]“ wird nirgends verwendet und fehlt im Text.
markdown_msg.md.truncated = … und { $count } weitere Hinweise derselben Art (nicht einzeln aufgeführt).
markdown_msg.md.front_matter = Angaben im YAML-Kopf nicht übernommen: { $keys } (nur title und author werden #set document).
markdown_msg.md.unknown = Unbekannte Markdown-Form nicht übernommen.
markdown_msg.md.heading_attributes = Kennung/Klassen der Überschrift ({ $syntax }) nicht übernommen.
markdown_msg.md.heading_format = Auszeichnung in der Überschrift „{ $title }“ entfällt (Titel als Text).
markdown_msg.md.heading_nested = Überschrift „{ $title }“ in Zitat oder Liste ist fett gesetzt, keine Gliederung.
markdown_msg.md.heading_level = Überschrift der Ebene { $depth } („{ $title }“) auf Ebene 3 begrenzt (Baruch kennt Kapitel, Abschnitt, Unterabschnitt).
markdown_msg.md.heading_empty = Leere Überschrift heißt „{ $title }“.
markdown_msg.md.quote_kind = Art des Hinweisblocks (> [!…]) nicht übernommen.
markdown_msg.md.unknown_block = Unbekannte Markdown-Form; ihr Text bleibt als Absatz.
markdown_msg.md.heading_footnote = Fußnote „[^{ $label }]“ in einer Überschrift entfällt (Titel bleiben einzeilig).
markdown_msg.md.unknown_inline = Unbekannte Auszeichnung entfällt, ihr Text bleibt.
markdown_msg.md.footnote_nested = Fußnote „[^{ $label }]“ in einer Fußnote bleibt als Text stehen.
markdown_msg.md.footnote_missing = Fußnote „[^{ $label }]“ hat keine Definition; bleibt als Text.
markdown_msg.md.footnote_repeated = Fußnote „[^{ $label }]“ wird mehrfach verwendet; jede Stelle bekommt eine eigene Fußnote.
markdown_msg.md.html_comment = HTML-Kommentar im Text entfällt.
markdown_msg.md.html = HTML-Auszeichnung „{ $shown }“ entfällt, der Text bleibt.
markdown_msg.md.html_block = HTML-Block „{ $shown }“ steht nur als Kommentar im Typst-Text (wird nicht gesetzt).
markdown_msg.md.code_lang = Sprachangabe „{ $word }“ des Codeblocks nicht übernommen (nur Buchstaben, Ziffern, _+-).
markdown_msg.md.definition_multiple = Begriff mit mehreren Erklärungen: jede Erklärung wird ein eigener Eintrag „{ $term }“.
markdown_msg.md.link_anchor = Sprung innerhalb des Dokuments
markdown_msg.md.link_scheme = Adresse mit unbekanntem Schema
markdown_msg.md.link_file = Verweis auf eine andere Datei
markdown_msg.md.link_local = Link „{ $shown }“ ({ $why }) entfällt, der Text bleibt.
markdown_msg.md.image_data = Eingebettetes Bild (data:) nicht übernommen; nur der Alternativtext bleibt.
markdown_msg.md.image_remote = Bild aus dem Netz „{ $shown }“ wird nicht geladen; an seiner Stelle steht ein Link.
markdown_msg.md.image_path = Bildpfad „{ $path }“ mit ungültiger %-Folge bleibt, wie er ist.
markdown_msg.md.image_title = Titel „{ $caption }“ eines Bilds im Fließtext entfällt.
