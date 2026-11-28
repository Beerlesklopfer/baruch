# SPDX-License-Identifier: AGPL-3.0-or-later
# Round 39a (templates_2): Pages of the works: list, archive, edit, delete, outline scheme, import report.
# Base catalog in English; locales/de/works.ftl has the same keys in the same order.

# Shared by the works pages.
works.back_to_work = Back to the work

# Page /werke (works.html).
works.list.window_title = Works – Baruch
works.list.heading = Works
works.list.new = New work or import…
works.list.archive_hint = View and restore deleted works
works.list.archive = Archive
works.list.col_cover = Cover
works.list.col_title = Title
works.list.col_author = Author
works.list.col_kind = Kind
works.list.col_sections = Sections
works.list.col_proposals = Open proposals
works.list.col_updated = Last changed
works.list.col_actions = Actions
works.list.cover_alt = Cover of { $title }
works.list.open = Open
works.list.edit = Edit
works.list.export = Export
works.list.pdf_hint = Download the book as PDF
works.list.empty = No work yet. Create one or import a Typst or Markdown file (also as a ZIP or a folder with images):
works.list.empty_lektor = No work shared yet. The author shares works one by one (page “Contributors” of the work).

# Page /werke/archiv (works_archive.html).
works.archive.window_title = Archive – Baruch
works.archive.heading = Archive of deleted works
works.archive.intro_html = A deleted work lies with its Git mirror under <code>works/{ $trash }/</code>. Restoring creates it anew with its old folder name and title: the current texts as revision 1 (“restored from the archive”), the work template and the typesetting settings from the mirror. Older revisions, posts and findings stay readable only in the Git history of the folder; images do not come along, and the kind of work is lost (a sermon comes back as a “book”). Ideas of the work stay ideas without a work – afterwards they can be assigned to a work on the page “Ideas”.
works.archive.col_folder = Folder
works.archive.col_deleted = Deleted
works.archive.col_action = Action
works.archive.not_found = not found
works.archive.restore_label = Restore { $title }
works.archive.confirm = Restore “{ $title }”
works.archive.restore = Restore
works.archive.not_restorable = Cannot be restored: { $reason }
works.archive.empty = The archive is empty: there are no deleted works.

# Page /werke/{id}/bearbeiten (work_edit.html, also on the settings page "Werk").
works.edit.window_title = Edit { $title } – Baruch
works.edit.page_title = Edit { $title }
works.edit.heading = Edit work
works.edit.folder_html = Folder name in the mirror: <code>works/{ $slug }/</code> (stays when renaming).
works.edit.imported_from_html = Imported from <code>{ $source }</code>.
works.edit.uuid = Identifier of the work (UUID, for the exchange between instances; cannot be changed):
works.edit.title = Title
works.edit.subtitle = Subtitle
works.edit.kind_book = Book
works.edit.kind_sermon = Sermon
works.edit.audience = Audience
works.edit.bible = Bible translation
works.edit.template = Work template
works.edit.template_hint = (everything before the first heading: #set, #show, #let …; currently revision { $rev })
works.edit.note = Note on the new revision of the work template
works.edit.note_hint = A changed work template becomes a new revision; the old ones stay readable below.
works.edit.revisions = Revisions of the work template
works.edit.revision = Revision { $number }
works.edit.current = current
works.edit.revision_read_only = Revision { $number } (read only)
works.edit.export_typst = Export as a Typst file
works.edit.delete = Delete work…
works.edit.numbering = Outline scheme…
works.edit.images = Images…

# Page /werke/{id}/loeschen (work_delete.html).
works.delete.window_title = Delete { $title } – Baruch
works.delete.page_title = Delete { $title }
works.delete.heading = Delete work
works.delete.intro_html = This deletes the work <strong>{ $title }</strong>
works.delete.by = by { $author }
works.delete.sections = with { $count ->
    [one] one section
    *[other] { $count } sections
    }
works.delete.revisions = , all revisions, proposals
works.delete.open = ({ $count } of them open)
works.delete.posts = , posts and the revisions of the work template.
works.delete.mirror_html = The Git mirror <code>works/{ $slug }/</code> gets a last commit “Werk gelöscht” and moves to <code>works/{ $trash }/{ $slug }-JJJJ-MM-TT/</code>; there every text stays readable in the Git history.
works.delete.restore_before = Through
works.delete.restore_link = Works → Archive
works.delete.restore_after = the work can be restored later (current texts as revision 1); its ideas become ideas without a work.
works.delete.export_before = Worth doing first:
works.delete.export_link = an export as a Typst file
works.delete.reserved_html = The folder name <code>{ $slug }</code> stays reserved afterwards so that the archive stays unambiguous: a new work with the same title gets a folder of its own (e.g. <code>{ $slug }-2</code>) and a new number. If no work is left after deleting, Baruch does not create a sample work again.
works.delete.confirm_label = Type the title to confirm
works.delete.submit = Delete for good

# Page /werke/{id}/nummerierung (work_numbering.html, also on the settings page "Werk").
works.numbering.window_title = Outline scheme – { $title } – Baruch
works.numbering.page_title = Outline scheme · { $title }
works.numbering.see_also = See also
works.numbering.heading = Outline scheme of “{ $title }”
works.numbering.now_html = Now: <strong>{ $label }</strong>
works.numbering.pattern = pattern “{ $pattern }”
works.numbering.no_numbers = no numbers
works.numbering.template_sets_none = , the template sets none – default “1.1”
works.numbering.custom = (“{ $pattern }”)
works.numbering.explain_html = The scheme applies to the outline, the head of the editor, the export, the typesetting and the Git mirror (<code>main.typ</code>). With “From the work template” the template stays unchanged in the export; otherwise a line <code>#set heading(numbering: …)</code> follows it, and the stored template stays as it is. Parts count on their own, chapters run on across parts; items without a number do not count and appear in the export as <code>#heading(…, numbering: none)</code>.
works.numbering.choose = Choose a scheme
works.numbering.in_force = in force now
works.numbering.custom_label = Own pattern
works.numbering.custom_hint = Pattern language: one Typst pattern for all levels (“1.1”, “I.1”, “1.a)”), with “|k” only levels 1 to k (“I|1” = only chapters in Roman numerals) or one pattern per level, separated by “;” (“I.;A.;1.;a);aa)”; a pattern with one counting sign shows only its own number, an empty one none). Counting signs: 1, a, A, i, I, *; “aa” = doubled letter (aa, bb, cc).
works.numbering.preview_of = Preview { $label }
works.numbering.more = … and { $count } more
works.numbering.rule = In the export the work template is then followed by:
works.numbering.preview = Preview
works.numbering.apply = Apply
works.numbering.buttons_hint = “Preview” only shows the chosen scheme and the own pattern; only “Apply” changes the numbers (one commit in the Git mirror).

# Report of an import (import_report.html).
works.import.window_title = Import: { $title } – Baruch
works.import.page_title = Import report
works.import.next = Next
works.import.to_work = To the work
works.import.heading = “{ $title }” has been created
works.import.taken_over = { $nodes ->
    [one] { $nodes } section
    *[other] { $nodes } sections
    }, { $images ->
    [one] { $images } image
    *[other] { $images } images
    } taken over.
works.import.mirror_failed = The Git mirror could not be written: { $reason }
works.import.skipped = Skipped images ({ $count })
works.import.skipped_hint = The text still names them; on the page “Images” they can be created later with their path.
works.import.warnings = Notes ({ $count })
works.import.line = Line { $line }:
works.import.open_work = Open work
