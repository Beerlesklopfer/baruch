# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of the outputs (PDF, EPUB, typesetting, Word, exchange) (Round 39a, task messages_1). Base catalog in English; locales/de/export_msg.ftl has the same keys in
# the same order with the German wording.

# Reading an exchange file (exchange/import.rs).
export_msg.exchange.title_hint = Import with a title of your own (form: field “Title”, CLI: --title "…").
export_msg.page.template_error = Template { $name }: { $error }
export_msg.exchange.read_aborted = Reading aborted: { $error }
export_msg.exchange.checksum_mismatch = The checksum does not match the file shown; please fetch the preview again.
export_msg.exchange.base64_invalid = data is not valid Base64.
export_msg.exchange.mode = mode is “new” or “merge”.
export_msg.exchange.inbox_symlink = { $path } is a symbolic link; nothing is put there
export_msg.exchange.inbox_create = { $path } cannot be created: { $error }
export_msg.exchange.inbox_write = { $path } cannot be written: { $error }
export_msg.exchange.inbox_store = { $path } cannot be stored: { $error }
export_msg.exchange.too_large = The file is larger than { $mib } MiB; import larger works through the command line.
export_msg.exchange.not_multipart = The form did not arrive as a file upload ({ $detail }).
export_msg.exchange.form_unreadable = The form could not be read ({ $detail }).
export_msg.exchange.file_unreadable = The file could not be read ({ $detail }).
export_msg.exchange.instance_unnamed = without a name ({ $uuid })
export_msg.exchange.choose_file = Please choose an exchange file (.baruch or .baruch.xml).
export_msg.exchange.checksum_invalid = Invalid checksum; please choose the file again.
export_msg.exchange.mode_unknown = Unknown mode (new or merge).
export_msg.exchange.preview_gone = The file of the preview is no longer there (cleared away after 24 hours or already adopted); please choose it again.
export_msg.exchange.choose_again = Please choose the file again.

# Counts of an exchange preview („2 Fassungen“; the number stands before the word).
export_msg.exchange.add.nodes = { $count ->
    [one] outline item
    *[other] outline items
    }
export_msg.exchange.add.revisions = { $count ->
    [one] revision
    *[other] revisions
    }
export_msg.exchange.add.template_revisions = { $count ->
    [one] template revision
    *[other] template revisions
    }
export_msg.exchange.add.ideas = { $count ->
    [one] idea
    *[other] ideas
    }
export_msg.exchange.add.posts = { $count ->
    [one] post
    *[other] posts
    }
export_msg.exchange.add.threads = { $count ->
    [one] thread
    *[other] threads
    }
export_msg.exchange.add.thread_posts = { $count ->
    [one] thread post
    *[other] thread posts
    }
export_msg.exchange.add.assets = { $count ->
    [one] image
    *[other] images
    }
export_msg.exchange.add.asset_revisions = { $count ->
    [one] image revision
    *[other] image revisions
    }
export_msg.exchange.add.links = { $count ->
    [one] link of the idea map
    *[other] links of the idea map
    }
export_msg.exchange.add.positions = { $count ->
    [one] position on the idea map
    *[other] positions on the idea map
    }
export_msg.exchange.add.cover_blocks = { $count ->
    [one] block of the cover layout
    *[other] blocks of the cover layout
    }
export_msg.exchange.add.findings = { $count ->
    [one] finding
    *[other] findings
    }
export_msg.exchange.add.finding_entries = { $count ->
    [one] entry in the history of findings
    *[other] entries in the history of findings
    }
export_msg.exchange.add.post_versions = { $count ->
    [one] old version of a post
    *[other] old versions of posts
    }
export_msg.exchange.add.typeset = { $count ->
    [one] typesetting settings
    *[other] typesetting settings
    }
export_msg.exchange.add.registers = { $count ->
    [one] own index
    *[other] own indexes
    }
export_msg.exchange.add.glossary_terms = { $count ->
    [one] glossary term
    *[other] glossary terms
    }
export_msg.exchange.add.amends = { $count ->
    [one] continuation
    *[other] continuations
    }

# EPUB check (epub_check.rs).
export_msg.epub_check.unknown_reader = Unknown reader “{ $slug }” (known: { $known }).
export_msg.epub_check.section = Section { $key }
export_msg.epub_check.queue_closed = EPUBCheck: queue closed
export_msg.epub_check.temp_dir = Temporary folder for EPUBCheck: { $error }
export_msg.epub_check.write_epub = Writing the EPUB for EPUBCheck: { $error }
export_msg.epub_check.aborted = Check aborted: { $error }
export_msg.epub_check.readers_list = readers: list of slugs expected
export_msg.epub_check.footnotes = footnotes: “popup” or “endnotes” expected
export_msg.epub_check.epubcheck_flag = epubcheck: true or false expected
export_msg.epub_check.jar_too_long = EPUBCheck: the path is too long (at most { $max } bytes).
export_msg.epub_check.jar_control = EPUBCheck: the path contains control characters.
export_msg.epub_check.jar_relative = EPUBCheck: “{ $path }” is not an absolute path (e.g. /opt/epubcheck/epubcheck.jar).
export_msg.epub_check.jar_extension = EPUBCheck: “{ $path }” does not end in .jar.
export_msg.epub_check.jar_not_file = EPUBCheck: “{ $path }” is not a file.
export_msg.epub_check.jar_missing = EPUBCheck: “{ $path }” does not exist.
export_msg.epub_check.jar_param_missing = jar missing: path to epubcheck.jar, or null to remove it
export_msg.epub_check.jar_param = jar: path (text) or null expected
export_msg.epub_check.quick_xml = quick-xml (built in; xmllint not found)
export_msg.epub_check.program = Program { $path }
export_msg.epub_check.found_title = EPUBCheck found
export_msg.epub_check.found_text = { $tool } – starts only at the push of a button.
export_msg.epub_check.missing_title = EPUBCheck not found
export_msg.epub_check.missing_text = Neither “epubcheck” in the PATH nor a configured epubcheck.jar (Settings → Data). Above is Baruch's own check; EPUBCheck is never downloaded.
export_msg.epub_check.passed = EPUBCheck: passed
export_msg.epub_check.failed = EPUBCheck: not passed
export_msg.epub_check.ran = { $tool } – the messages stand in the list with “EPUBCheck:”.
export_msg.epub_check.ran_version = { $tool }, version { $version } – the messages stand in the list with “EPUBCheck:”.
export_msg.epub_check.not_through = EPUBCheck did not run through
export_msg.epub_check.report_error = Check report: { $error }
export_msg.epub_check.footnotes_form = fussnoten: “popup” or “endnoten”, not “{ $other }”.
export_msg.epub_check.foreign = Refused: the check is not made at the request of a foreign page.
export_msg.page.error = Error: { $error }
export_msg.page.unknown_work = unknown work
export_msg.page.work_missing = Error: work missing.

# Typesetting (typeset.rs).
export_msg.typeset.cancelled = Typesetting cancelled.
export_msg.typeset.timeout = Typst took longer than { $limit }
export_msg.typeset.draft_param = draft: true (with watermark), false (without) or leave it out (as set)
export_msg.typeset.draft_form = entwurf “{ $other }”: 1 (with watermark) or 0 (without) expected
export_msg.typeset.lock = Typst lock: { $error }
export_msg.typeset.crashed = Typst crashed: { $error }
export_msg.typeset.tail = End of the book (index, glossary)
export_msg.typeset.number = { $name }: number expected
export_msg.typeset.no_section = A section with the key { $key } does not exist in work { $work }.
export_msg.typeset.no_page = Page { $page } does not exist (1–{ $total }).
export_msg.typeset.foreign = Refused: typesetting does not start at the request of a foreign page.
export_msg.typeset.template_error = Error: template { $name }: { $error }
export_msg.typeset.nothing_to_cancel = No typesetting was running that could be cancelled.
export_msg.typeset.was_cancelled = The typesetting was cancelled.
export_msg.typeset.preview_plain = Preview without watermark
export_msg.typeset.preview_draft = Preview as draft
export_msg.typeset.page_gone = Page does not exist (other or outdated state)

# PDF output (pdf.rs).
export_msg.pdf.kind_book = book
export_msg.pdf.kind_cover = cover
export_msg.pdf.kind_notes = speaker notes
export_msg.pdf.busy = A PDF is being generated for this work right now; please wait and then try again.
export_msg.pdf.no_pdf_from_typst = Typst delivered no PDF
export_msg.pdf.error_in_file = { $message } ({ $file }, line { $line })
export_msg.pdf.error_in_export = { $message } (line { $line } in the export)
export_msg.pdf.failed_one = No PDF ({ $kind }): typesetting fails with { $count } error. First: { $first }
export_msg.pdf.failed = No PDF ({ $kind }): typesetting fails with { $count } errors. First: { $first }
export_msg.pdf.flag = { $name }: true or false expected
export_msg.pdf.kind_map = mind map

# EPUB output (epub.rs).
export_msg.epub.raster = Raster image: { $error }
export_msg.epub.cover_image_instead = EPUB cover: the front of the cover could not be typeset as an image ({ $reason }); instead the image file of the cover “{ $path }” stands in the EPUB without a title.
export_msg.epub.cover_none = EPUB cover: the front of the cover could not be typeset as an image ({ $reason }); the EPUB has no cover.
export_msg.epub.foreign = Refused: the EPUB is not made at the request of a foreign page.
export_msg.epub.footnotes = fussnoten: popup or endnoten

# EPUB preview (epub_preview.rs).
export_msg.epub_kind.cover = Cover
export_msg.epub_kind.front = Front matter
export_msg.epub_kind.part = Part
export_msg.epub_kind.chapter = Chapter
export_msg.epub_kind.notes = Notes
export_msg.epub_kind.nav = Contents
export_msg.epub_kind.ncx = Contents (EPUB 2)
export_msg.epub_kind.style = Style
export_msg.epub_kind.image = Image
export_msg.epub_kind.cover_image = Cover image
export_msg.epub_kind.glossary = Glossary
export_msg.epub_kind.index = Index
export_msg.epub_preview.unknown_reader = readers: unknown reader profile “{ $slug }”
export_msg.epub_preview.no_text_file = file: no file with text in the EPUB: { $file }
export_msg.epub_preview.no_text = The EPUB has no file with text.
export_msg.epub_preview.foreign = Refused: the EPUB preview is not made at the request of a foreign page.
export_msg.epub_preview.unknown_profile = Unknown reader profile.
export_msg.epub_preview.unknown_file = Unknown file.
export_msg.epub_preview.unknown_work = Unknown work.
export_msg.epub_preview.lacks_popup = footnotes as a jump instead of a popup
export_msg.epub_preview.lacks_backgrounds = no background colours
export_msg.epub_preview.lacks_svg = SVG images unsafe
export_msg.epub_preview.lacks_fonts = no embedded fonts
export_msg.epub_preview.overrides_font = own default font instead of the font of the book
export_msg.epub_preview.lacks_hyphenation = no hyphenation
export_msg.epub_preview.lacks_night = no night mode
export_msg.epub_preview.profile_not_shown = Unknown reader profile “{ $slug }” – not shown.
export_msg.epub_preview.footnotes_default = Footnotes: “popup” or “endnoten” – shown is the default (popup).
export_msg.epub_preview.file_missing = The file “{ $file }” does not exist in the EPUB – shown is the first chapter file.

# Exchange page (exchange/export.rs).
export_msg.exchange.title = Exchange
export_msg.exchange.failed = Exchange: { $error }
export_msg.exchange.format_not = format: expected “baruch” or “xml”, not “{ $word }”
export_msg.exchange.format = format: “baruch” or “xml” expected

# Speaker notes (pdf/notes.rs).
export_msg.notes.rule_marker = own marking
export_msg.notes.rule_strong = emphasis
export_msg.notes.rule_first_sentence = first sentence per paragraph
export_msg.notes.format_unknown = Format “{ $text }” does not exist; a5 or karten expected
export_msg.notes.font_size = Font size “{ $text }”: whole number from { $min } to { $max } (points) expected
export_msg.notes.format_param = format: "a5" or "karten" expected
export_msg.notes.font_param = font_pt: number from { $min } to { $max } expected

# Word export (docx_export/*).
export_msg.docx.write = Writing the Word file: { $error }
export_msg.docx.unknown_work = work: unknown work
export_msg.docx.foreign = Refused: the Word file is not made at the request of a foreign page.
export_msg.docx.proposals_flag = vorschlaege: 1 or 0
export_msg.docx.more_islands = { $count } further Typst islands also stand only as visible source text in the file.
export_msg.docx.untitled = without title
export_msg.docx.quoted_title = “{ $title }”
export_msg.docx.section_warning = Section { $place }: { $message }
export_msg.docx.more_proposals_one = Section { $place }: { $more } further open proposal is not in the file – only the latest stands as tracked changes (revision { $rev } by { $author }).
export_msg.docx.more_proposals = Section { $place }: { $more } further open proposals are not in the file – only the latest stands as tracked changes (revision { $rev } by { $author }).

# Typesetting settings and sheet (typeset_settings.rs, sheet.rs).
export_msg.typeset_settings.confirm = confirm: true expected
export_msg.typeset_settings.missing = settings missing
export_msg.typeset_settings.invalid = settings: { $error }
export_msg.typeset_settings.no_rule = The work template has no recognisable rule for the chapter start; nothing changed.
export_msg.typeset_settings.no_document_title = Running head “work title”: the work template sets no title (#set document(title: …)); the running head stays empty
export_msg.typeset_settings.font_missing = Font “{ $font }” is missing; Typst typesets with the embedded font
export_msg.sheet.reset = reset: true or false expected
export_msg.sheet.text = { $field }: text expected
