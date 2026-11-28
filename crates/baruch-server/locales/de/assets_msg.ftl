# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of images and image requests (Round 39a, task messages_1). Base catalog in English; locales/de/assets_msg.ftl has the same keys in
# the same order with the German wording.

# RPC methods asset.* (assets.rs).
assets_msg.rpc.encoding = encoding: „base64“ oder „svg“ erwartet, nicht „{ $encoding }“
assets_msg.rpc.confirm_delete = Löschen bitte bestätigen (confirm).
assets_msg.rpc.no_revision = Bild { $name }: Fassung { $rev } gibt es nicht; vorhanden: { $numbers }
assets_msg.rpc.no_current = Bild { $name }: keine aktuelle Fassung

# Pages „Bilder“.
assets_msg.page.template_error = Fehler: Vorlage { $name }: { $error }
assets_msg.page.error = Fehler: { $error }
assets_msg.page.unknown_work = unbekanntes Werk
assets_msg.page.image = Bild
assets_msg.page.cover_panel = Umschlag
assets_msg.page.requests_panel = Anträge
assets_msg.page.no_path = Fehler: kein Pfad
assets_msg.page.no_image_for_path = Kein Bild zum Pfad „{ $path }“.
assets_msg.notice.added = Das Bild ist hinzugefügt.
assets_msg.notice.replaced = Neue Fassung hochgeladen; sie ist jetzt aktuell.
assets_msg.notice.switched = Die gewählte Fassung ist jetzt aktuell.
assets_msg.notice.saved = Gespeichert.
assets_msg.notice.unchanged = Nichts geändert.
assets_msg.notice.revision_deleted = Die Fassung ist gelöscht; ihr Inhalt steht im Git-Spiegel.
assets_msg.notice.deleted = Das Bild ist gelöscht; sein Inhalt steht im Git-Spiegel.
assets_msg.notice.cover_set = Das Bild ist jetzt der Umschlag des Werks.
assets_msg.notice.cover_removed = Das Werk hat keinen Umschlag mehr.
assets_msg.notice.request_confirmed = Der Antrag ist bestätigt und ausgeführt.
assets_msg.notice.request_rejected = Der Antrag ist abgelehnt.
assets_msg.notice.request_stale = Der Antrag ist veraltet (das Bild hat sich inzwischen geändert) und wurde nicht ausgeführt.
assets_msg.notice.extension_adapted = Neue Fassung hochgeladen; das Bild heißt jetzt mit der neuen Endung, die Pfade blieben.
assets_msg.notice.path_assigned = Der Pfad gehört jetzt in allen Schreibweisen zum gewählten Bild.

# Thumbnails.
assets_msg.thumb.unreadable = Bild nicht lesbar: { $error }
assets_msg.thumb.write_failed = Vorschau ließ sich nicht schreiben: { $error }
assets_msg.thumb.bound = b: längste Seite in Pixeln zwischen { $min } und { $max } angeben.
assets_msg.thumb.unavailable = Fehler: Vorschaubilder nicht verfügbar

# Upload and forms.
assets_msg.upload.too_large = Das Bild ist größer als { $mib } MiB.
assets_msg.upload.not_multipart = Das Formular kam nicht als Datei-Upload an ({ $detail }).
assets_msg.upload.form_unreadable = Das Formular ließ sich nicht lesen ({ $detail }).
assets_msg.upload.file_unreadable = Die Datei ließ sich nicht lesen ({ $detail }).
assets_msg.upload.work_missing = Werk fehlt.
assets_msg.upload.choose_file = Bitte eine Bilddatei wählen.
assets_msg.insert.at_end = am Ende
assets_msg.insert.at_start = am Anfang (unter der Überschrift)
assets_msg.insert.before = vor „{ $excerpt }“
assets_msg.insert.place_fields = Fehler: Werk, Abschnitt (key) und Fassung (rev) angeben.
assets_msg.insert.rev_invalid = Fehler: Fassung ungültig.
assets_msg.insert.place_invalid = Fehler: Stelle ungültig (end, start, block:{ $lt }Nummer{ $gt } oder offset:{ $lt }Stelle{ $gt }).
assets_msg.insert.choose_image = Fehler: Bild wählen.
assets_msg.insert.heading_changed = Die Überschriftszeile ist geändert – erst speichern, dann das Bild einfügen.
assets_msg.form.work_missing = Fehler: Werk fehlt.
assets_msg.form.path_and_image = Pfad und Bild wählen.
assets_msg.form.image_missing = Fehler: Bild fehlt.
assets_msg.form.rev_missing = Fassung fehlt.
assets_msg.form.confirm_delete = Löschen bitte mit dem Kästchen bestätigen.
assets_msg.form.unknown_action = unbekannte Aktion

# Requests about images (asset_requests.rs).
assets_msg.request.decided = Über diesen Antrag ist schon entschieden.
assets_msg.request.no_cover = keiner
assets_msg.request.quoted = „{ $name }“
assets_msg.request.cover_changed = Der Umschlag ist inzwischen { $now } (beim Antrag { $then }).
assets_msg.request.image_gone = Das Bild „{ $name }“ gibt es nicht mehr.
assets_msg.request.not_executable = Nicht mehr ausführbar: { $error }
assets_msg.request.other_current = „{ $name }“ hat inzwischen eine andere aktuelle Fassung ({ $number }); der Antrag bezog sich auf den Stand davor.
assets_msg.request.revision_gone = Fassung { $rev } von „{ $name }“ gibt es nicht mehr.
assets_msg.request.revision_changed = Fassung { $rev } von „{ $name }“ ist inzwischen eine andere.
assets_msg.request.revision_current = Fassung { $rev } von „{ $name }“ ist inzwischen die aktuelle und lässt sich nicht löschen.
assets_msg.request.id_number = id: Zahl erwartet
assets_msg.request.id_missing = id fehlt (Nummer des Antrags)
assets_msg.request.decision = decision: confirm oder reject erwartet
assets_msg.request.open_flag = open: true oder false erwartet
assets_msg.request.work_missing = work fehlt (Werk-id)
assets_msg.request.no_content = Antrag mit Inhalt gibt es nicht
assets_msg.request.purged = Der vorgeschlagene Inhalt von Antrag { $nr } ist entfernt: über den Antrag wurde am { $decided } entschieden, und Baruch hebt den Inhalt entschiedener Anträge { $days } Tage lang auf.
