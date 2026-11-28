# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of the work management (Round 39a, task messages_1). Base catalog in English; locales/de/works_msg.ftl has the same keys in
# the same order with the German wording.

# Import (POST /werke/import, work.import, work.import_archive).
works_msg.import.use_word_import = Word-Dateien liest der eigene Import mit Vorschau: Baruch ▾ → Word (.docx) importieren… (/werke/import/word) bzw. RPC docx_import.preview.
works_msg.import.request_too_large = Die Anfrage ist größer als 80 MiB (Grenze für ZIP und Ordner).
works_msg.import.not_multipart = Das Formular kam nicht als Datei-Upload an ({ $detail }).
works_msg.import.form_unreadable = Das Formular ließ sich nicht lesen ({ $detail }).
works_msg.import.file_unreadable = Die Datei ließ sich nicht lesen ({ $detail }).
works_msg.import.not_both = Bitte entweder eine Datei oder einen Ordner wählen, nicht beides.
works_msg.import.choose = Bitte eine Datei (.typ, .md, .markdown, .zip) oder einen Ordner wählen.
works_msg.import.folder_failed = Der Ordner lässt sich nicht importieren: { $error }.
works_msg.import.file_failed = „{ $base }“ lässt sich nicht importieren: { $error }.
works_msg.import.wrong_type = „{ $base }“ lässt sich nicht importieren; nur .typ lässt sich importieren, dazu Markdown (.md, .markdown) und ZIP-Archive (.zip).
works_msg.import.file_too_large = Die Datei ist größer als 16 MiB.
works_msg.import.not_utf8 = „{ $base }“ ist kein UTF-8-Text.
works_msg.import.choose_other_title = Bitte unten einen anderen Titel eintragen und die Datei noch einmal wählen.

# Checks of the RPC methods work.*.
works_msg.rpc.kind_invalid = kind: „book“ oder „sermon“ erwartet, nicht „{ $other }“
works_msg.rpc.preset_invalid = preset: „book“ oder „sermon“ erwartet, nicht „{ $other }“
works_msg.rpc.language_invalid = language: „de“ oder „en“ erwartet, nicht „{ $other }“
works_msg.rpc.work_missing = work fehlt: Welches Werk? (Werk-id, Zahl)
works_msg.rpc.zip_via_archive = Ein ZIP-Archiv geht über work.import_archive (Inhalt als Base64).
works_msg.rpc.not_zip = „{ $base }“ ist kein ZIP-Archiv; work.import_archive nimmt nur .zip.
works_msg.rpc.confirm_title_missing = confirm_title fehlt: Titel des Werks zur Bestätigung
works_msg.rpc.slug_missing = slug fehlt: Ordnername des archivierten Werks (aus work.archives)

# Sermon text of a new sermon (work.create with text_ref).
works_msg.sermon.ref_too_long = text_ref: höchstens { $max } Zeichen
works_msg.sermon.module_invalid = module: Modulname mit höchstens { $max } Zeichen in einer Zeile erwartet
works_msg.sermon.not_a_passage = Predigttext: „{ $text_ref }“ ist keine Bibelstelle (z. B. „Joh 3,16“).
works_msg.sermon.one_passage = Predigttext: bitte genau eine Bibelstelle angeben, nicht „{ $text_ref }“.
works_msg.sermon.passage_problem = Predigttext „{ $input }“: { $problem }
works_msg.sermon.no_bible = Keine SWORD-Bibel eingerichtet (Einstellungen → Quellen → Bibeln); nur die Stellenangabe steht da.
works_msg.sermon.module_missing = Bibelmodul { $name } in keinem Modulordner gefunden; nur die Stellenangabe steht da.
works_msg.sermon.module_error = Bibelmodul { $name }: { $error }; nur die Stellenangabe steht da.
works_msg.sermon.passage_missing =
    Die Stelle fehlt im Bibelmodul { $name } ({ $missing } { $missing ->
        [one] Vers
       *[other] Verse
    }); nur die Stellenangabe steht da.
works_msg.sermon.verses_missing =
    { $missing } { $missing ->
        [one] Vers fehlt
       *[other] Verse fehlen
    } im Bibelmodul { $name }; der Wortlaut ist unvollständig.
works_msg.sermon.truncated = Nur die ersten { $max } Verse eingefügt.
works_msg.sermon.only_sermon_preset = Einen Predigttext gibt es nur beim Grundgerüst „Predigt“.

# Deleting, archive and restoring.
works_msg.delete.title_mismatch = Der eingegebene Titel passt nicht; nichts gelöscht.
works_msg.trash.move_aborted = Verschieben abgebrochen: { $error }
works_msg.trash.symlink = { $path } ist eine symbolische Verknüpfung; der Spiegel bleibt, wo er ist
works_msg.trash.create_failed = { $path } lässt sich nicht anlegen: { $error }
works_msg.trash.move_failed = { $from } lässt sich nicht nach { $to } verschieben: { $error }
works_msg.archive.trash_symlink = { $path } ist eine symbolische Verknüpfung; das Archiv wird nicht gelesen
works_msg.archive.unreadable = { $path } lässt sich nicht lesen: { $error }
works_msg.archive.parent_symlink = { $path }: Ordner ist eine symbolische Verknüpfung; nichts gelesen
works_msg.archive.missing_file = { $path } fehlt im Archivordner
works_msg.archive.symlink = { $path } ist eine symbolische Verknüpfung; nichts gelesen
works_msg.archive.not_a_file = { $path } ist keine Datei
works_msg.archive.too_large = Der Archivordner ist größer als 16 MiB (Grenze wie beim Import); nichts wiederhergestellt.
works_msg.archive.not_utf8 = { $path } ist kein UTF-8-Text
works_msg.archive.invalid_folder = ungültiger Archivordner { $folder }
works_msg.archive.missing = { $path } fehlt
works_msg.archive.not_a_folder = { $path } ist kein Ordner
works_msg.archive.template_symlink = vorlage.typ ist eine symbolische Verknüpfung
works_msg.archive.bad_include = main.typ bindet { $path } ein; erlaubt sind nur abschnitte/{ $lt }name{ $gt }.typ
works_msg.archive.too_many_sections = mehr als { $max } Abschnittsdateien; nichts wiederhergestellt
works_msg.archive.read_aborted = Archiv lesen abgebrochen: { $error }
works_msg.archive.no_folder = Kein Archivordner works/{ $trash }/{ $slug }-… gefunden.
works_msg.archive.no_main = Im Archivordner fehlt main.typ.
works_msg.archive.live_title = Ein lebendes Werk trägt denselben Titel; erst jenes umbenennen.
works_msg.restore.not_archived = „{ $slug }“ steht nicht im Archiv der gelöschten Werke; nichts wiederhergestellt.
works_msg.restore.no_folder = Kein Archivordner works/{ $trash }/{ $slug }-… gefunden; nichts wiederhergestellt.
works_msg.restore.exists = works/{ $slug } gibt es schon; nichts wiederhergestellt.
works_msg.restore.move_failed = { $from } lässt sich nicht nach works/{ $slug } verschieben: { $error }
works_msg.restore.slug_missing = Welches Werk? Der Ordnername fehlt.
works_msg.restore.confirm = Bitte das Wiederherstellen mit dem Kästchen bestätigen; nichts wiederhergestellt.

# Pages of the work management.
works_msg.page.template_error = Fehler: Vorlage { $name }: { $error }
works_msg.page.error = Fehler: { $error }
works_msg.page.unknown_work = unbekanntes Werk
works_msg.kind.book = Buch
works_msg.kind.sermon = Predigt
works_msg.list.deleted = Das Werk „{ $title }“ ist gelöscht; sein Git-Spiegel liegt jetzt unter works/{ $trash }/.
works_msg.act.work_missing = Fehler: Werk fehlt.
works_msg.act.unknown_action = unbekannte Aktion
