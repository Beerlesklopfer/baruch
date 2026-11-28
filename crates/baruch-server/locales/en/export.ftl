# SPDX-License-Identifier: AGPL-3.0-or-later
# Round 39a (templates_2): Export and import pages: Word, EPUB check and preview, exchange file, typesetting.
# Base catalog in English; locales/de/export.ftl has the same keys in the same order.

# Shared by the export and import pages. These pages translate only the surrounding interface: book texts, style
# names in the Word file and the contents of exchange files stay as they are (Round 39b).
export.back_to_work = Back to the work
export.to_work = To the work
export.works = Works
export.next = Next
export.title = Title
export.notes = Notes
export.discard = Discard
export.show_preview = Show preview

# Page /werke/{id}/word (docx_export.html).
export.word.window_title = Word export – { $title } – Baruch
export.word.page_title = { $title }: Word export
export.word.heading = Word export
export.word.intro_html = The work <strong>{ $title }</strong> as a Word file (<code>{ $slug }.docx</code>) for the publisher: front matter, then all sections in reading order with their <strong>current revision</strong>.
export.word.styles = Headings carry the styles “Überschrift 1–3”, parts “Baruch Teil”, headings without a number “Baruch ohne Nummer 1–3”.
export.word.footnotes = Footnotes are real footnotes, images (PNG, JPEG, GIF) are embedded and carry their alternative text.
export.word.unmapped = What Word cannot represent (Typst functions, formulas, images in other formats) appears as visible source text in a grey field. Nothing gets lost silently; the export names such places as a warning through RPC and the command line.
export.word.unchanged = The export changes nothing in the work.
export.word.with_proposals = Include open proposals as tracked changes
export.word.open_proposals = At present { $proposals ->
    [one] 1 proposal is
    *[other] { $proposals } proposals are
    } open, in { $sections ->
    [one] 1 section
    *[other] { $sections } sections
    }.
export.word.no_open_proposals = At present no proposal is open; the file is the same with and without the tick.
export.word.proposals_hint = Per section the newest open proposal appears as insertion and deletion in the file, with the persona as the editor; the current revision stays the base text. Further open proposals of the same section are not in the file.
export.word.download = Download Word file
export.word.check_hint = Whether the file looks as intended in Word or LibreOffice only opening it there shows.

# Importing a Word file (docx_import.html).
export.word_import.window_title = Import a Word file – Baruch
export.word_import.page_title = Import a Word file
export.word_import.heading = Read a Word file
export.word_import.intro = Baruch reads the file and shows the outline, editors and warnings. Nothing is created until you confirm the preview. The file stays on this computer.
export.word_import.file = File (.docx, at most { $mib } MiB)
export.word_import.title_hint = (empty: from the file)
export.word_import.read = Read and show preview
export.word_import.doc_hint = Baruch does not read the old format .doc – save it as .docx in Word. Typst or Markdown?
export.word_import.new_work = New work / import …
export.word_import.preview = Preview: { $name }
export.word_import.rule = Changes and comments never become the current revision.
export.word_import.rule_more = Revision 1 of every section is the state before all tracked changes; the changes of every editor become open proposals, comments become posts on the section of the commented place.
export.word_import.sections = { $count ->
    [one] { $count } section
    *[other] { $count } sections
    }
export.word_import.comments = { $count ->
    [one] { $count } comment
    *[other] { $count } comments
    }
export.word_import.footnotes = { $count ->
    [one] { $count } footnote
    *[other] { $count } footnotes
    }
export.word_import.images = { $count ->
    [one] { $count } image
    *[other] { $count } images
    }
export.word_import.editors = Editors
export.word_import.col_name = Name in Word
export.word_import.col_baruch = in Baruch
export.word_import.col_insertions = Insertions
export.word_import.col_deletions = Deletions
export.word_import.col_proposals = Proposals (sections)
export.word_import.no_name = without a name
export.word_import.no_changes = The file contains no tracked changes.
export.word_import.outline = Outline
export.word_import.part_prefix = Part:
export.word_import.part = Part
export.word_import.level = Level { $level }
export.word_import.unnumbered = without a number
export.word_import.chars =
    { $count } { $count ->
        [one] character
       *[other] characters
    }
export.word_import.apply = Adopt
export.word_import.changes_as_proposals = create tracked changes as open proposals
export.word_import.comments_as_posts = create comments as posts
export.word_import.adopt = Adopt as a work
export.word_import.discard_hint = deletes the draft; nothing is created.
export.word_import.adopted = Adopted: { $title }
export.word_import.report =
    The work has been created: { $sections } { $sections ->
        [one] section
       *[other] sections
    }, { $proposals } open { $proposals ->
        [one] proposal
       *[other] proposals
    }, { $posts } { $posts ->
        [one] post
       *[other] posts
    }. Something stood out while adopting:

# EPUB pages (epub_check.html, fragments/epub_check_panel.html, epub_preview.html).
export.epub.download = Download EPUB
export.epub.preview = EPUB preview
export.epub.required = (required)
export.epub.footnotes = Footnotes
export.epub.as_popup = as a popup (default)
export.epub_check.window_title = Check EPUB – { $title } – Baruch
export.epub_check.page_title = { $title }: check EPUB
export.epub_check.heading = Check EPUB
export.epub_check.as_endnotes = as endnotes
export.epub_check.recheck = Check again
export.epub_check.epubcheck_before = EPUBCheck is the checking program of the W3C community. Baruch uses “epubcheck” from the PATH or an epubcheck.jar under a set path (Settings →
export.epub_check.epubcheck_link = Data
export.epub_check.epubcheck_after = , needs Java) and never downloads anything. A run takes a few seconds.
export.epub_check.run = Run EPUBCheck
export.epub_panel.heading = EPUB check
export.epub_panel.no_errors = No errors.
export.epub_panel.errors = { $count ->
    [one] { $count } error
    *[other] { $count } errors
    }
export.epub_panel.warnings = { $count ->
    [one] { $count } warning
    *[other] { $count } warnings
    }
export.epub_panel.infos = { $count ->
    [one] { $count } note
    *[other] { $count } notes
    }
export.epub_panel.tools = (well-formedness: { $xml }; readers: { $readers })
export.epub_panel.col_kind = Kind
export.epub_panel.col_message = Message
export.epub_panel.col_place = Place
export.epub_panel.no_findings = No findings.
export.epub_panel.full_report = Full check report, run EPUBCheck …
export.epub_preview.window_title = EPUB preview – { $title } – Baruch
export.epub_preview.page_title = { $title }: EPUB preview
export.epub_preview.heading = EPUB preview per reader
export.epub_preview.kindle_html = Kindle: Baruch creates no Kindle format of its own. <strong>Send to Kindle accepts EPUB</strong> and converts it for Kindle devices and the Kindle app; the profiles “Kindle (via EPUB)” and “Kindle app (via EPUB)” show the approximate result.
export.epub_preview.profiles = The profiles imitate font, size, margins, line spacing, hyphenation and night mode of the readers – without a real device, as an assumption. The device itself is what counts.
export.epub_preview.file = File
export.epub_preview.as_endnotes = as notes at the end
export.epub_preview.night = Night mode
export.epub_preview.show = Show
export.epub_preview.files = Files and chapter boundaries
export.epub_preview.files_hint = Every part and every chapter starts a file of its own (new page in the reader).
export.epub_preview.warnings = Notes while creating
export.epub_preview.side_by_side = Readers side by side
export.epub_preview.via_kindle = Through Send to Kindle (the EPUB is converted).
export.epub_preview.frame_title = { $name }: preview
export.epub_preview.lacks = Peculiarities: { $list }.
export.epub_preview.open_alone = Open the file on its own
export.epub_preview.check = Check
export.epub_preview.no_report = No check report here. In detail:

# Page /werke/{id}/austausch (exchange.html).
export.exchange.window_title = Exchange – { $title } – Baruch
export.exchange.page_title = { $title }: exchange
export.exchange.heading = Exchange: output
export.exchange.intro_html = The exchange file carries the <strong>whole work</strong> to another Baruch instance: details, book details including the cover, work template, the outline with <strong>all revisions</strong> and the mark “current”, posts, ideas (also deleted ones), the idea map (links between ideas and remembered positions), threads, images with all revisions, typesetting settings, the glossary (also proposals) with the own registers and the tombstones of deleted parts. Every piece has a worldwide unique identifier (UUID); so the other instance recognises when reading in what it already knows and adds only what is missing.
export.exchange.not_in_file = Not in the file: keys and tokens, connections to services (models, MultiGPT, pgvector), personas including models (only their names as the origin), open requests and register proposals, evidence of the web search, Bible, commentary and literature texts (only module names and references), search index and the author library.
export.exchange.downloads = Download
export.exchange.download_baruch = Download exchange file (.baruch)
export.exchange.baruch_hint_html = { $name } – ZIP with <code>werk.xml</code>, the images as files of their own and a manifest with checksums. For all works.
export.exchange.download_xml = Download only XML (.baruch.xml)
export.exchange.xml_hint = { $name } – one single XML file, images as Base64 in it. Easy to read, for small works.
export.exchange.scope = Scope
export.exchange.count_nodes = { $nodes ->
    [one] { $nodes } outline item
    *[other] { $nodes } outline items
    } with { $revisions ->
    [one] { $revisions } revision
    *[other] { $revisions } revisions
    } and { $posts ->
    [one] { $posts } post
    *[other] { $posts } posts
    }
export.exchange.count_template = { $count ->
    [one] { $count } revision
    *[other] { $count } revisions
    } of the work template
export.exchange.count_ideas = { $count ->
    [one] { $count } idea
    *[other] { $count } ideas
    }
export.exchange.of_them_deleted = ({ $count } of them deleted)
export.exchange.count_threads = { $threads ->
    [one] { $threads } thread
    *[other] { $threads } threads
    } with { $posts ->
    [one] { $posts } post
    *[other] { $posts } posts
    }
export.exchange.count_findings = { $findings ->
    [one] { $findings } finding
    *[other] { $findings } findings
    } ({ $open } of them open) and { $versions ->
    [one] { $versions } old post version
    *[other] { $versions } old post versions
    }
export.exchange.count_assets = { $assets ->
    [one] { $assets } image
    *[other] { $assets } images
    } with { $revisions ->
    [one] { $revisions } revision
    *[other] { $revisions } revisions
    }
export.exchange.count_links = Idea map: { $count ->
    [one] { $count } link
    *[other] { $count } links
    }
export.exchange.count_positions = and { $count ->
    [one] { $count } remembered position
    *[other] { $count } remembered positions
    }
export.exchange.typeset = Typesetting settings: { $fields }
export.exchange.count_terms = Glossary: { $count ->
    [one] { $count } term
    *[other] { $count } terms
    }
export.exchange.of_them_proposed = ({ $count } of them proposed)
export.exchange.count_registers = , { $count ->
    [one] { $count } own register
    *[other] { $count } own registers
    }
export.exchange.count_tombstones = { $count ->
    [one] { $count } tombstone
    *[other] { $count } tombstones
    } of deleted parts
export.exchange.work_id = Work identifier
export.exchange.instance = instance { $label }
export.exchange.format = format
export.exchange.version = version { $version }
export.exchange.read_in_before = Adopt a file of another instance:
export.exchange.read_in_link = Exchange: read in…
export.exchange.read_in_after = (with a preview; Baruch never deletes or overwrites anything in the process).
export.exchange.cli = Also through the command line:
export.exchange.deviations = Deviations ({ $count })
export.exchange.col_what = What
export.exchange.col_here = Here
export.exchange.col_there = There
export.exchange.col_action = Consequence

# Answer after adopting an exchange file (exchange_done.html).
export.exchange_done.window_title = Exchange adopted – { $title } – Baruch
export.exchange_done.page_title = Exchange adopted
export.exchange_done.heading = “{ $title }”: { $mode } adopted
export.exchange_done.from_html = From <strong>{ $instance }</strong>.
export.exchange_done.nothing_added = Nothing was added.
export.exchange_done.added = Added:
export.exchange_done.commit_html = Recorded in the mirror as commit <code>{ $commit }</code>.
export.exchange_done.mirror = Mirror: { $error } (the work has been adopted anyway).
export.exchange_done.refused = Not adopted: { $refusal }
export.exchange_done.requests = View { $count ->
    [one] { $count } request
    *[other] { $count } requests
    } about the outline
export.exchange_done.deleted_here = Deleted here and not created again: { $count }.
export.exchange_done.read_another = Read in another file

# Page /austausch/einlesen (exchange_import.html).
export.exchange_import.window_title = Read in an exchange file – Baruch
export.exchange_import.heading = Exchange: read in
export.exchange_import.choose = Choose an exchange file
export.exchange_import.file = File (.baruch or .baruch.xml, at most { $mib } MiB)
export.exchange_import.title_hint = (only for a new work; empty: from the file)
export.exchange_import.what_happens = What happens
export.exchange_import.preview_first_html = First comes a <strong>preview</strong>: what would be added, what deviates, which requests would arise. Nothing is written until “Adopt”.
export.exchange_import.unknown_work_html = If the work is <strong>unknown</strong> here, a new work arises with all revisions, ideas, posts, threads and images.
export.exchange_import.known_work_html = If it is <strong>known</strong>, it is reconciled: only what is missing is added. Nothing is deleted or overwritten, no revision read in becomes current; a deviating revision comes as a further revision, changes of the outline become
export.exchange_import.requests_link = requests
export.exchange_import.known_work_after = , which you decide on.
export.exchange_import.deleted_stays = What was deleted here does not come back. Unknown parts of a newer file are reported but not adopted.

# Preview of an exchange file (exchange_preview.html).
export.exchange_preview.window_title = Exchange: preview – Baruch
export.exchange_preview.page_title = Exchange: preview
export.exchange_preview.group = Exchange
export.exchange_preview.other_file = Another file
export.exchange_preview.heading = Preview: { $name }
export.exchange_preview.from_html = From <strong>{ $instance }</strong>, output { $exported }. Work “{ $title }” –
export.exchange_preview.mode_new_html = <strong>new</strong>: it does not exist here yet.
export.exchange_preview.mode_merge_html = <strong>reconcile</strong> with the existing work.
export.exchange_preview.newer = The file has minor version { $version }; this Baruch reads minor version 1. Anything unknown is reported below and not adopted.
export.exchange_preview.would_add = What would be added
export.exchange_preview.nothing = Nothing: everything from the file exists here already.
export.exchange_preview.single = One by one ({ $count }
export.exchange_preview.truncated = , shortened
export.exchange_preview.requests = Requests about the outline ({ $count })
export.exchange_preview.requests_hint = They change nothing at once; you confirm or reject them under Outline → Requests.
export.exchange_preview.deleted_here = Deleted here ({ $count })
export.exchange_preview.does_not_return = Does not come back.
export.exchange_preview.unknown = Unknown, will not be adopted
export.exchange_preview.title_hint = (empty: “{ $title }” from the file)
export.exchange_preview.checksum_html = Checksum of the file: <code>{ $sha256 }</code>. Exactly this file is adopted; it is planned anew, in one step, with one entry in the mirror.
export.exchange_preview.adopt = Adopt ({ $mode })

# Page /werke/{id}/satz (typeset.html): only the interface around the typeset pages.
export.typeset.window_title = Typesetting – { $title } – Baruch
export.typeset.page_title = { $title }: typesetting
export.typeset.download = Download
export.typeset.pdf_hint = The whole work as PDF (work template, current revisions, images)
export.typeset.pdf = Download PDF
export.typeset.pages = Pages
export.typeset.previous = Previous page
export.typeset.previous_none = Previous page (this is the first)
export.typeset.page_of = Page { $shown } of { $total }
export.typeset.next = Next page
export.typeset.next_none = Next page (this is the last)
export.typeset.run = Typesetting run
export.typeset.cancel_hint = Cancel the running Typst typesetting of this work (also one that is still waiting)
export.typeset.cancel = Cancel typesetting
export.typeset.heading = Typesetting with Typst
export.typeset.draft = Draft
export.typeset.watermarked = The preview shows the watermark.
export.typeset.draft_hint = Book PDF with the watermark “Entwurf”; the typesetting setting stays as it is
export.typeset.download_draft = Download as a draft
export.typeset.plain_hint = Book PDF without a watermark; the typesetting setting stays as it is
export.typeset.download_plain = Download without a watermark
export.typeset.not_reset = It was not typeset again.
export.typeset.last_state = Shown is the last typeset state.
export.typeset.restart = Typeset again
export.typeset.page_count = { $count ->
    [one] { $count } page
    *[other] { $count } pages
    }
export.typeset.no_pages = no pages
export.typeset.errors = { $count ->
    [one] { $count } error
    *[other] { $count } errors
    }
export.typeset.warnings = { $count ->
    [one] { $count } warning
    *[other] { $count } warnings
    }
export.typeset.cached = from the cache
export.typeset.millis = typeset in { $millis } ms
export.typeset.messages = Messages
export.typeset.line = Line { $line }
export.typeset.column = , column { $column }
export.typeset.in_file = in { $file }
export.typeset.comma_line = , line { $line }
export.typeset.hint = Hint: { $hint }
export.typeset.print_check = Print check
export.typeset.badge_template = Template
export.typeset.template_rule = The work template sets the chapter start itself ({ $label }, line { $line }):
export.typeset.adopt_rule = Adopt it into the typesetting settings and remove the line
export.typeset.badge_typeset = Typesetting
export.typeset.settings = Typesetting settings
export.typeset.badge_xref = Reference
export.typeset.broken_xref = Broken cross-reference { $label } in { $section }: “{ $text }” (only as text in the typesetting)
export.typeset.to_place = to the place
export.typeset.check_xrefs = Check cross-references
export.typeset.badge_missing = Missing
export.typeset.image_missing = Image missing: { $path }
export.typeset.upload_or_assign = Upload images or assign the path
export.typeset.dpi_where = Page { $page } · { $width } × { $height } pixels
export.typeset.badge_imprint = Imprint
export.typeset.imprint_hint = usual under the German state press laws, no legal advice
export.typeset.book_details = Book details
export.typeset.all_fine = All images are there; no raster image lies below { $dpi } dpi.
export.typeset.chapter_pages = Pages of the chapter
export.typeset.chapter = Chapter
