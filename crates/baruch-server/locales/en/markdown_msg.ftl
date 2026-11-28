# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of the Typst import (src/import.rs) and of the Markdown, ZIP and folder import (src/markdown_import/**):
# refusals, skipped images, notes of the report, the report of the command line (Round 39c, task reste_1).
# The command line imports run in the German interface language (as the other CLI imports since Round 39a).

# Typst import and export into a folder (src/import.rs).
markdown_msg.typst.only_typ = Only Typst files (.typ) can be imported.
markdown_msg.typst.title_hint = Choose another title with --title "…".
markdown_msg.typst.back_matter_cut = The generated book end of an export (index and glossary pages from “// Baruch: Register und Glossar” on) is cut off; it is created anew on export from the typeset settings.
markdown_msg.typst.back_matter_kept = The line “// Baruch: Register und Glossar” is in the text, but without the generated book end after it; nothing cut off.
markdown_msg.typst.projection_removed = Additions of the projection of an export (automatic scripture reference marks, marking of the first mention) are removed; they are created anew on export from the settings.
markdown_msg.typst.typeset_adopted = The rule block “// Baruch: Satz” is taken over into the typeset settings of the work (Settings → Work → Typesetting), not into the work template.
markdown_msg.typst.typeset_kept = The rule block “// Baruch: Satz” could not be read back exactly into typeset settings (changed by hand?) and stays in the work template.
markdown_msg.typst.read_aborted = Reading { $path } aborted: { $detail }
markdown_msg.file.too_large = { $file } is larger than 16 MiB
markdown_msg.file.too_large_mib = { $file } is larger than { $mib } MiB
markdown_msg.file.unreadable = { $file } cannot be read: { $detail }
markdown_msg.file.not_file = { $file } is not a file
markdown_msg.file.not_utf8 = { $file } is not UTF-8 text
markdown_msg.file.no_name = { $file }: file name missing or not UTF-8
markdown_msg.asset.folder = folder of the source file not readable ({ $detail })
markdown_msg.asset.not_path = not a file path
markdown_msg.asset.not_found = not found ({ $detail })
markdown_msg.asset.outside = lies outside the folder of the source file
markdown_msg.asset.unreadable = not readable ({ $detail })
markdown_msg.asset.not_file = not a file
markdown_msg.asset.too_large = larger than 8 MiB
markdown_msg.asset.total = the images of this file are larger than 64 MiB together
markdown_msg.asset.not_image = not a supported image (PNG, JPEG, GIF, WebP, SVG)
markdown_msg.export.asset_path = Image path { $path } is not a relative path without ..; nothing written
markdown_msg.export.twice = { $path } would occur twice; nothing written
markdown_msg.export.aborted = Writing aborted: { $detail }
markdown_msg.export.not_folder = { $path } is not a folder; nothing written
markdown_msg.export.file_and_folder = { $path } would be file and folder at once (for { $other }); nothing written
markdown_msg.export.exists = { $path } exists already; nothing overwritten
markdown_msg.export.link = { $path } is a link; images could land outside { $out }; nothing written
markdown_msg.export.is_file = { $path } is a file, not a folder; nothing written
markdown_msg.export.create = { $path } cannot be created: { $detail }
markdown_msg.export.read = { $path } cannot be read: { $detail }
markdown_msg.export.outside = { $path } lies outside { $out }; aborted
markdown_msg.export.write = { $path } cannot be written: { $detail }

# Markdown, ZIP and folder (src/markdown_import/mod.rs).
markdown_msg.import.only_markdown = Only Markdown files (.md, .markdown) can be imported this way.
markdown_msg.import.not_sent = not sent along (only the Markdown file arrived; upload a folder or ZIP to take images along)
markdown_msg.import.too_many_files = more than { $max } files
markdown_msg.import.duplicate = duplicate name (upper/lower case counts as the same)
markdown_msg.import.file_too_large = “{ $path }” is larger than 16 MiB
markdown_msg.import.total_too_large = all files together larger than 80 MiB
markdown_msg.import.no_main = No main file (.typ, .md or .markdown) found.
markdown_msg.import.several_main = Several possible main files ({ $files }); exactly one .typ/.md file or one index.md or main.typ on the top level expected.
markdown_msg.import.bad_part = “..”, “.” or an empty part in the path
markdown_msg.import.unused = { $count } file(s) not taken over because the main file “{ $main }” does not name them: { $files }
markdown_msg.import.address = not a file path (address)
markdown_msg.import.no_file_name = no file name
markdown_msg.import.not_in_bundle = not found (not in the archive or folder)
markdown_msg.import.failed = Import of { $file }: { $detail }

# Report of the command line (markdown_import::report_text).
markdown_msg.report.work = Work: { $title } (id { $id })
markdown_msg.report.folder = Folder name: { $slug }
markdown_msg.report.sections = Sections: { $count }
markdown_msg.report.images = Images: { $count }
markdown_msg.report.skipped = Image skipped: { $path } ({ $reason })
markdown_msg.report.note_line = Note ({ $code }, line { $line }): { $message }
markdown_msg.report.note = Note ({ $code }): { $message }
markdown_msg.report.commit = Commit: { $commit }
markdown_msg.report.mirror_failed = Git mirror not written (the work is created): { $error }
markdown_msg.report.no_commit = Commit: none

# Notes of the Markdown conversion (src/markdown_import/convert.rs).
markdown_msg.md.preamble = Text before the first heading is now in the chapter “{ $chapter }” (in Baruch it would otherwise be the work template).
markdown_msg.md.footnote_unused = Footnote “[^{ $label }]” is used nowhere and is missing in the text.
markdown_msg.md.truncated =
    … and { $count ->
        [one] { $count } more note
       *[other] { $count } more notes
    } of the same kind (not listed one by one).
markdown_msg.md.front_matter = Entries in the YAML header not taken over: { $keys } (only title and author become #set document).
markdown_msg.md.unknown = Unknown Markdown form not taken over.
markdown_msg.md.heading_attributes = Id/classes of the heading ({ $syntax }) not taken over.
markdown_msg.md.heading_format = Markup in the heading “{ $title }” is dropped (title as text).
markdown_msg.md.heading_nested = Heading “{ $title }” in a quotation or list is set in bold, not part of the outline.
markdown_msg.md.heading_level = Heading of level { $depth } (“{ $title }”) limited to level 3 (Baruch knows chapter, section, subsection).
markdown_msg.md.heading_empty = An empty heading is called “{ $title }”.
markdown_msg.md.quote_kind = Kind of the note block (> [!…]) not taken over.
markdown_msg.md.unknown_block = Unknown Markdown form; its text stays as a paragraph.
markdown_msg.md.heading_footnote = Footnote “[^{ $label }]” in a heading is dropped (titles stay on one line).
markdown_msg.md.unknown_inline = Unknown markup is dropped, its text stays.
markdown_msg.md.footnote_nested = Footnote “[^{ $label }]” inside a footnote stays as text.
markdown_msg.md.footnote_missing = Footnote “[^{ $label }]” has no definition; stays as text.
markdown_msg.md.footnote_repeated = Footnote “[^{ $label }]” is used more than once; every place gets its own footnote.
markdown_msg.md.html_comment = HTML comment in the text is dropped.
markdown_msg.md.html = HTML markup “{ $shown }” is dropped, the text stays.
markdown_msg.md.html_block = HTML block “{ $shown }” stays only as a comment in the Typst text (is not typeset).
markdown_msg.md.code_lang = Language of the code block “{ $word }” not taken over (only letters, digits, _+-).
markdown_msg.md.definition_multiple = Term with several explanations: every explanation becomes an entry of its own “{ $term }”.
markdown_msg.md.link_anchor = jump within the document
markdown_msg.md.link_scheme = address with an unknown scheme
markdown_msg.md.link_file = reference to another file
markdown_msg.md.link_local = Link “{ $shown }” ({ $why }) is dropped, the text stays.
markdown_msg.md.image_data = Embedded image (data:) not taken over; only the alternative text stays.
markdown_msg.md.image_remote = Image from the web “{ $shown }” is not loaded; a link stands in its place.
markdown_msg.md.image_path = Image path “{ $path }” with an invalid %-sequence stays as it is.
markdown_msg.md.image_title = Title “{ $caption }” of an image in running text is dropped.
