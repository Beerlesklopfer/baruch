# SPDX-License-Identifier: AGPL-3.0-or-later
# Round 39a (templates_2): Images of a work, one image, open image requests.
# German translation of locales/en/assets.ftl (same keys in the same order).

# Shared by the image pages.
assets.back_to_work = Zurück zum Werk
assets.name = Name
assets.kind = Typ
assets.size = Größe
assets.paths = Pfade
assets.no_path = keiner
assets.alt = Alternativtext
assets.caption = Bildunterschrift
assets.note = Notiz
assets.cover = Umschlag
assets.as_cover = Als Umschlag
assets.upload = Hochladen

# Page /werke/{id}/bilder (assets.html).
assets.list.window_title = Bilder – { $title } – Baruch
assets.list.page_title = { $title }: Bilder
assets.list.heading = Bilder
assets.list.conflicts = Pfade mit zwei Bildern
assets.list.conflicts_hint_html = Diese Pfade sind gleich, wenn man führendes <code>/</code> und <code>./</code> weglässt, gehören aber zu verschiedenen Bildern (Altbestand). Satz, Satzbild und Export nehmen dann je nach Schreibweise ein anderes Bild. Bitte wählen, welches Bild den Pfad behält; der Text bleibt unverändert.
assets.list.path_belongs_to = Pfad gehört künftig zu
assets.list.assign = Zuordnen
assets.list.search = Suchen
assets.list.search_placeholder = Name, Bildunterschrift oder Alternativtext
assets.list.search_label = Bilder suchen
assets.list.found =
    { $count } von { $total } { $total ->
        [one] Bild
       *[other] Bildern
    } für „{ $query }“
assets.list.show_all = alle zeigen
assets.list.col_preview = Vorschau
assets.list.col_revision = Fassung
assets.list.of_revisions = von { $count }
assets.list.no_match = Kein Bild passt zu „{ $query }“.
assets.list.show_all_images = Alle Bilder zeigen
assets.list.empty_html = Noch keine Bilder. Bilder kommen mit dem Import von der Kommandozeile (<code>baruch-server import buch.typ</code> liest die Dateien, die der Text mit <code>image("…")</code> nennt) oder über das Formular unten.
assets.list.add = Bild hinzufügen
assets.list.file = Datei (PNG, JPEG, GIF, WebP oder SVG, höchstens { $mib } MiB)
assets.list.name_field = Name (leer = aus dem Dateinamen; ein vorhandener Name lädt eine neue Fassung hoch)
assets.list.path_field = Pfad in Vorlage und Text (einer je Zeile)
assets.list.as_base64 = SVG als Base64 speichern (sonst als Text; andere Typen immer als Base64)
assets.list.path_hint_html = Der Pfad ist die Schreibweise in <code>#image("…")</code> bzw. <code>#figure(image("…"))</code>; Export und Satzbild finden das Bild darüber. Im Git-Spiegel liegt jedes aktuelle Bild unter <code>bilder/{ $name }</code> (SVG) bzw. <code>bilder/{ $name }.b64</code>.

# Page /werke/{id}/bilder/{name} (asset.html).
assets.one.window_title = { $name } – Bilder – Baruch
assets.one.page_title = { $title }: Bild { $name }
assets.one.all_images = Alle Bilder
assets.one.heading = { $name } · Fassung { $number }
assets.one.current_mark = (aktuell)
assets.one.stored_as = gespeichert als
assets.one.svg_text = SVG-Text
assets.one.by = Von
assets.one.changed = geändert { $date }
assets.one.file = Datei
assets.one.open = öffnen
assets.one.revisions = Fassungen
assets.one.revision = Fassung { $number }
assets.one.current = aktuell
assets.one.based_on = auf Fassung { $number }
assets.one.make_current = Aktuell machen
assets.one.describe = Fassung { $number } beschreiben
assets.one.paths_field = Pfade des Bilds in Vorlage und Text (einer je Zeile)
assets.one.replace = Ersetzen (neue Fassung)
assets.one.new_file = Neue Datei ({ $kind }, höchstens { $mib } MiB; ein anderes Format nur mit „Endung anpassen“)
assets.one.as_base64 = SVG als Base64 speichern
assets.one.rename = Endung anpassen, wenn die Datei ein anderes Format hat (z. B. PNG statt SVG: das Bild heißt dann mit neuer Endung, alle Pfade bleiben, der Text bleibt unverändert)
assets.one.replace_hint = Die neue Fassung wird sofort aktuell; die bisherigen bleiben und lassen sich zurückschalten.
assets.one.delete_revision_heading = Fassung { $number } löschen
assets.one.delete_revision_confirm = Fassung { $number } wirklich löschen
assets.one.delete_revision = Fassung löschen
assets.one.delete_image = Bild löschen
assets.one.delete_image_hint =
    Löscht { $name } mit { $count ->
        [one] seiner { $count } Fassung
       *[other] allen { $count } Fassungen
    } und Pfaden. Der Inhalt bleibt im Git-Spiegel lesbar.
assets.one.delete_image_confirm = { $name } wirklich löschen

# Requests of the assistants about images (fragments/asset_requests.html).
assets.requests.heading = Anträge der Assistenten
assets.requests.open_count = ({ $count } offen)
assets.requests.intro = Assistenten legen Bilder sofort an; Ersetzen, Umschlag setzen, Löschen und eine neue Umschlag-Anordnung beantragen sie nur. Bestätigen führt den Antrag aus (ein Commit im Git-Spiegel, Sie als Bestätiger, der Assistent als Herkunft). Hat sich das Bild, der Umschlag oder die Anordnung inzwischen geändert, wird der Antrag nicht ausgeführt, sondern als veraltet abgeschlossen. Ablehnen ändert nichts.
assets.requests.col_number = Nr.
assets.requests.col_request = Antrag
assets.requests.col_from = Von
assets.requests.col_reason = Begründung
assets.requests.view_cover = Umschlag ansehen
assets.requests.view_image = Bild ansehen
assets.requests.view_content = vorgeschlagenen Inhalt ansehen
assets.requests.content_alt = Vorgeschlagener Inhalt für { $name }
assets.requests.caption = Bildunterschrift: { $caption }
assets.requests.alt = Alternativtext: { $alt }
assets.requests.confirm = Bestätigen
assets.requests.reject = Ablehnen
assets.requests.none = Keine offenen Anträge zu Bildern.
assets.requests.decided = Zuletzt entschieden
assets.requests.request = Antrag { $id }:
