# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of the imports in the browser (src/pdf_import/pages.rs, src/docx_import): upload and form errors,
# outline check of the PDF/EPUB preview, warnings and errors of the Word import (Round 39a, task messages_2).

# Upload pages (PDF/EPUB and Word).
import_msg.page.template = Error: template { $name }: { $detail }
import_msg.page.discarded = Draft discarded; nothing was created.
import_msg.page.too_large = The file is larger than { $max } MiB.
import_msg.page.not_upload = The form did not arrive as a file upload ({ $detail }).
import_msg.page.form_unreadable = The form could not be read ({ $detail }).
import_msg.page.file_unreadable = The file could not be read ({ $detail }).
import_msg.page.stored_gone = The stored file no longer exists (older than 24 hours or already read); please choose the file again.
import_msg.page.choose_source = Please choose a PDF or EPUB file.
import_msg.page.choose_docx = Please choose a Word file (.docx).
import_msg.page.other_title = Please enter another title above.

# Outline confirmed in the PDF/EPUB preview (one line per item).
import_msg.outline.line_start = Line “{ $line }”: every line begins with =, == or === and the title (empty = delete).
import_msg.outline.levels = Line “{ $line }”: Baruch knows three levels (=, ==, ===).
import_msg.outline.part_level = Line “{ $line }”: a part stands on the top level (“= Teil: Title”).
import_msg.outline.no_title = Line “{ $line }”: title missing (empty the whole line to delete it).
import_msg.outline.empty = The outline is empty: at least one item (chapter) is needed.

# Word import (src/docx_import/mod.rs).
import_msg.docx.no_text = The file contains no text.
import_msg.docx.lead_text = { $who } inserted text before the first heading; it belongs to no section and is not taken over (beginning: “{ $start }”).
import_msg.docx.unaligned_editor = The changes of { $who } could not be assigned to the sections ({ $found } instead of { $expected }); no proposal created.
import_msg.docx.heading_inserted = { $who } inserted the heading “{ $heading }”; it creates no section, its text stands in bold in the proposal for the preceding section.
import_msg.docx.heading_deleted = { $who } deleted the heading “{ $heading }”; it stays.
import_msg.docx.lost_comments = { $count } comment(s) without a findable place in the text stand at the first section.
import_msg.docx.image_skipped = Image “{ $path }” skipped: { $reason }.
import_msg.docx.mirror = The Git mirror could not be written: { $detail }
import_msg.docx.unaligned_work =
    The work has { $work } { $work ->
        [one] section
       *[other] sections
    }, the file { $file }; proposals and posts could not be assigned and are not created.
import_msg.docx.not_docx = “{ $file }” is not a Word file (.docx); please save the old format .doc as .docx in Word.
import_msg.docx.read_aborted = Reading aborted: { $detail }
import_msg.docx.draft_unknown = Draft “{ $id }” does not exist (discarded, already taken over or older than 24 hours); please read the file again.
import_msg.docx.draft_busy = Draft “{ $id }” is being taken over right now.
import_msg.docx.draft_id = draft: 32 lower-case hex characters (from docx_import.preview).
import_msg.docx.delete_aborted = Deleting aborted: { $detail }
