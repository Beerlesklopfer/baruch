# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of the imports in the browser (src/pdf_import/pages.rs, src/docx_import): upload and form errors,
# outline check of the PDF/EPUB preview, warnings and errors of the Word import (Round 39a, task messages_2).

# Upload pages (PDF/EPUB and Word).
import_msg.page.template = Fehler: Vorlage { $name }: { $detail }
import_msg.page.discarded = Entwurf verworfen; es wurde nichts angelegt.
import_msg.page.too_large = Die Datei ist größer als { $max } MiB.
import_msg.page.not_upload = Das Formular kam nicht als Datei-Upload an ({ $detail }).
import_msg.page.form_unreadable = Das Formular ließ sich nicht lesen ({ $detail }).
import_msg.page.file_unreadable = Die Datei ließ sich nicht lesen ({ $detail }).
import_msg.page.stored_gone = Die abgelegte Datei gibt es nicht mehr (älter als 24 Stunden oder schon gelesen); bitte die Datei neu wählen.
import_msg.page.choose_source = Bitte eine PDF- oder EPUB-Datei wählen.
import_msg.page.choose_docx = Bitte eine Word-Datei (.docx) wählen.
import_msg.page.other_title = Bitte oben einen anderen Titel eintragen.

# Outline confirmed in the PDF/EPUB preview (one line per item).
import_msg.outline.line_start = Zeile „{ $line }“: Jede Zeile beginnt mit =, == oder === und dem Titel (leer = streichen).
import_msg.outline.levels = Zeile „{ $line }“: Baruch kennt drei Ebenen (=, ==, ===).
import_msg.outline.part_level = Zeile „{ $line }“: Ein Teil steht auf der obersten Ebene („= Teil: Titel“).
import_msg.outline.no_title = Zeile „{ $line }“: Titel fehlt (zum Streichen die ganze Zeile leeren).
import_msg.outline.empty = Die Gliederung ist leer: mindestens ein Punkt (Kapitel) ist nötig.

# Word import (src/docx_import/mod.rs).
import_msg.docx.no_text = Die Datei enthält keinen Text.
import_msg.docx.lead_text = { $who } hat Text vor der ersten Überschrift eingefügt; er gehört zu keinem Abschnitt und ist nicht übernommen (Anfang: „{ $start }“).
import_msg.docx.unaligned_editor = Die Änderungen von { $who } ließen sich den Abschnitten nicht zuordnen ({ $found } statt { $expected }); kein Vorschlag angelegt.
import_msg.docx.heading_inserted = { $who } hat die Überschrift „{ $heading }“ eingefügt; sie legt keinen Abschnitt an, ihr Text steht fett im Vorschlag zum vorangehenden Abschnitt.
import_msg.docx.heading_deleted = { $who } hat die Überschrift „{ $heading }“ gelöscht; sie bleibt bestehen.
import_msg.docx.lost_comments = { $count } Kommentar(e) ohne auffindbare Stelle im Text stehen am ersten Abschnitt.
import_msg.docx.image_skipped = Bild „{ $path }“ übersprungen: { $reason }.
import_msg.docx.mirror = Der Git-Spiegel ließ sich nicht schreiben: { $detail }
import_msg.docx.unaligned_work =
    Das Werk hat { $work } { $work ->
        [one] Abschnitt
       *[other] Abschnitte
    }, die Datei { $file }; Vorschläge und Beiträge ließen sich nicht zuordnen und sind nicht angelegt.
import_msg.docx.not_docx = „{ $file }“ ist keine Word-Datei (.docx); das alte Format .doc bitte in Word als .docx speichern.
import_msg.docx.read_aborted = Lesen abgebrochen: { $detail }
import_msg.docx.draft_unknown = Entwurf „{ $id }“ gibt es nicht (verworfen, schon übernommen oder älter als 24 Stunden); die Datei bitte neu lesen.
import_msg.docx.draft_busy = Entwurf „{ $id }“ wird gerade übernommen.
import_msg.docx.draft_id = draft: 32 Kleinbuchstaben-Hex (aus docx_import.preview).
import_msg.docx.delete_aborted = Löschen abgebrochen: { $detail }

# Round 39b (rest_literals): the stored drafts of the Word import (src/docx_import/draft.rs).
import_msg.docx_draft.invalid = ungültiger Entwurf
import_msg.docx_draft.create = Entwurf lässt sich nicht anlegen: { $error }
import_msg.docx_draft.write = Entwurf lässt sich nicht schreiben: { $error }
import_msg.docx_draft.encode = Entwurf: { $error }
import_msg.docx_draft.unreadable = Entwurf unlesbar: { $error }
import_msg.docx_draft.source_missing = Datei des Entwurfs fehlt: { $error }
