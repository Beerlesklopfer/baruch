# SPDX-License-Identifier: AGPL-3.0-or-later
# Sources, literature and web evidence (Round 39a, templates_3). Base catalog in English; locales/de/sources.ftl has the same keys in the same order.

# templates/source.html
sources.source.window_title = { $title } – Source – Baruch
sources.source.page_title = Source · { $title }
sources.source.back_group = Back
sources.source.literature = Literature
sources.source.back_to_work = Back to the work
sources.source.quotes = Quotations
sources.source.quote_n = Quotation { $index }
sources.source.no_quotes = No quotations yet.
sources.source.fulltext = Full text
sources.source.load_fulltext = Load full text
sources.source.part = Part { $part }
sources.source.part_of = of { $parts }
sources.source.end_unknown = , end not known yet
sources.source.marks = · { $marks } places marked
sources.source.parts_nav = Parts of the full text
sources.source.prev_part = ← previous part
sources.source.next_part = next part →
sources.source.highlights = Highlights
sources.source.use_selection = Use the selected text
sources.source.start = Start in the full text
sources.source.end = End in the full text
sources.source.quote = Quotation
sources.source.no_quote = – without quotation –
sources.source.locator = Place
sources.source.locator_placeholder = p. 12 (empty: place of the quotation)
sources.source.text = Text
sources.source.text_placeholder = Paste the selected text here …
sources.source.kind = Kind
sources.source.note = Note
sources.source.question = Question to the persona
sources.source.question_placeholder = What does this text say about my section?
sources.source.highlight = Highlight
sources.source.cite = Cite (proposal in the section)
sources.source.persona_title = The persona answers in the section chat
sources.source.to_persona = Give to persona
sources.source.similar = Find similar

# templates/source_import.html
sources.import.window_title = Import PDF or EPUB – Baruch
sources.import.title = Import PDF or EPUB
sources.import.back_group = Back
sources.import.works = Works
sources.import.back_to_work = Back to the work
sources.import.multigpt_failed = MultiGPT did not read “{ $file_name }”; Baruch does not switch the path on its own.
sources.import.read_local = Read locally
sources.import.read_local_help = pdftotext on this computer; the file is still ready, nothing has been created.
sources.import.read_file = Read file
sources.import.intro = Baruch reads the file and proposes an outline. Nothing is created until you confirm the outline in the preview – as a work, as a source of the library or both.
sources.import.file = File (.pdf or .epub, at most { $max_mib } MiB)
sources.import.way = Path (PDF only; Baruch always reads EPUB itself)
sources.import.way_auto = automatic
sources.import.way_auto_help = (MultiGPT if it is set up, a collection is chosen and upload is ticked; otherwise local)
sources.import.way_local = local
sources.import.way_local_help = (pdftotext on this computer, nothing leaves it)
sources.import.way_multigpt = through MultiGPT
sources.import.way_multigpt_help = (table of contents from MultiGPT)
sources.import.way_multigpt_missing = (no connection set up: Settings → Connections)
sources.import.collection = Collection in MultiGPT
sources.import.upload_named = upload to MultiGPT (collection “{ $collection }”)
sources.import.upload_unnamed = upload to MultiGPT (collection: enter it above)
sources.import.title_field = Title
sources.import.title_empty = (empty: from the file)
sources.import.read_submit = Read and show outline
sources.import.typst_instead = Typst file instead of PDF?
sources.import.new_work = New work / import …
sources.import.preview = Preview: { $file_name }
sources.import.way_line_html = <strong>Path: { $way_label }</strong> ({ $kind_label }
sources.import.pages =
    , { $count ->
        [one] { $count } page
       *[other] { $count } pages
    }
sources.import.document_html = In MultiGPT: collection “{ $collection }”, document <code>{ $id }</code>.
sources.import.hints = Notes
sources.import.skipped = Skipped images
sources.import.skipped_image_html = Image skipped: <code>{ $path }</code> – { $reason }
sources.import.assets = { $assets } image(s) come along into the work.
sources.import.check_outline = Check outline
sources.import.outline_help_html = One item per line: <code>= Title</code> chapter, <code>== Title</code> section, <code>=== Title</code> subsection; <code>= Teil: Title</code> makes a part. Emptying a line removes the item (its text goes to the previous item). The order stays as in the file.
sources.import.item = Item { $index }
sources.import.chars = { $chars } characters
sources.import.part_hint = · possible part – as a part only with “= Teil: …”
sources.import.adopt_as = Take over as
sources.import.as_work = Work
sources.import.as_library = Source of the library
sources.import.as_both = both
sources.import.author = Author
sources.import.adopt = Take over
sources.import.discard = Discard
sources.import.discard_help = deletes the draft; nothing is created.

# templates/source_multigpt.html
sources.multigpt.window_title_end = – Full text – Baruch
sources.multigpt.page_title = Full text · { $title }
sources.multigpt.back_group = Back
sources.multigpt.literature = Literature
sources.multigpt.back_to_work = Back to the work
sources.multigpt.collection = · collection { $collection }
sources.multigpt.document_html = · document <code>{ $document }</code>
sources.multigpt.section = · place according to MultiGPT: { $section }
sources.multigpt.in_library = In the library:
sources.multigpt.adopt = Take into the library
sources.multigpt.adopt_help = creates a title linked to this document (full text and checking of highlights)
sources.multigpt.fulltext = Full text
sources.multigpt.load_fulltext = Load full text
sources.multigpt.part = Part { $part }
sources.multigpt.part_of = of { $parts }
sources.multigpt.end_unknown = , end not known yet
sources.multigpt.marks = · { $marks } places marked
sources.multigpt.parts_nav = Parts of the full text
sources.multigpt.prev_part = ← previous part
sources.multigpt.next_part = next part →
sources.multigpt.to_persona = Give to persona
sources.multigpt.needs_section = “Give to persona” needs a section: open the literature from within a section.
sources.multigpt.text = Text (at most { $max_persona_text } characters; shorten it to the important passage)
sources.multigpt.question = Question to the persona
sources.multigpt.question_placeholder = What does this text say about my section?
sources.multigpt.give_source = Source on the post: { $give_source } – no title of the library needed.

# templates/literature.html
sources.literature.window_title = Literature – Baruch
sources.literature.page_title = Literature · { $title }
sources.literature.back_group = Back
sources.literature.back_to_work = Back to the work
sources.literature.title = Literature

# templates/literature_bible.html
sources.bible.bible = Bible
sources.bible.window_title_end = – Literature – Baruch
sources.bible.page_title = Bible · { $title }
sources.bible.back_group = Back
sources.bible.literature = Literature
sources.bible.back_to_work = Back to the work
sources.bible.settings_sources = Settings → Sources
sources.bible.heading = Bible { $module }
sources.bible.work_bible = Bible of the work
sources.bible.license = Licence (according to conf):
sources.bible.no_license = no information in the conf
sources.bible.open_label = Open the Bible
sources.bible.of_work = (work)
sources.bible.reference = Reference
sources.bible.reference_placeholder = e.g. Ps 139:23-24
sources.bible.open = Open
sources.bible.search_label = Search in the wording
sources.bible.wording = Wording
sources.bible.wording_placeholder = Words in the Bible text
sources.bible.search = Search
sources.bible.opened = Opened passage
sources.bible.chapter = Chapter
sources.bible.prev_chapter = ← previous chapter
sources.bible.next_chapter = next chapter →
sources.bible.module_ref = read in the versification of the module: { $module_ref }
sources.bible.ref_to_field = “{ $ref }” into the field “Reference”
sources.bible.no_text = (no text in the module)
sources.bible.unmapped = (no German reference: { $unmapped })
sources.bible.use_verses = Use verses
sources.bible.needs_section = Citing and “give to persona” need a section: open the literature from within a section.
sources.bible.question = Question to the persona
sources.bible.question_placeholder = What does this passage say about my section?
sources.bible.cite = Cite (proposal in the section)
sources.bible.to_persona = Give to persona
sources.bible.commentaries = Commentaries
sources.bible.hits = Hits
sources.bible.hits_for = Hits for “{ $q }”
sources.bible.no_hits = No hits.
sources.bible.truncated = Only the first hits (limit reached).
sources.bible.books = Books
sources.bible.book_chapter = Book and chapter
sources.bible.book = Book
sources.bible.open_chapter = Open
sources.bible.chapters = { $chapters } chapters

# templates/fragments/literature_bibles.html
sources.bibles.bible = Bible
sources.bibles.more_hits = More hits on the Bible page.
sources.bibles.bibles = Bibles
sources.bibles.settings_sources = Settings → Sources
sources.bibles.work_bible = Bible of the work

# templates/fragments/library.html
sources.library.kind_quote = Quotation
sources.library.kind_core = Key statement
sources.library.kind_contradiction = Contradiction
sources.library.kind_check = to check
sources.library.search_placeholder = Search work and literature …
sources.library.search_label = Search work and literature
sources.library.search_by = Search by
sources.library.mode_fts = Wording (FTS)
sources.library.mode_rag = Meaning (RAG)
sources.library.search = Search
sources.library.rag_hits_for = Meaning hits for “{ $library_query }”
sources.library.same_fts = The same search in the wording (FTS)
sources.library.fulltext = Full text
sources.library.similarity = Similarity { $percent } %
sources.library.found_by = · found by { $found_by }
sources.library.weak = weak
sources.library.no_hits = No hits.
sources.library.only_weak = Only weak hits.
sources.library.weak_hits =
    { $count ->
        [one] { $count } weak hit
       *[other] { $count } weak hits
    }
sources.library.hits_for = Hits for “{ $library_query }”
sources.library.revision = Revision { $rev }
sources.library.post = Post
sources.library.quote_key = Quotation · @{ $key }
sources.library.highlight_key = Highlight · @{ $key }
sources.library.title_key = Title · @{ $key }
sources.library.in_source = in the source
sources.library.no_quotes = No quotations yet.
sources.library.proposal = Proposal
sources.library.document_view = Document view
sources.library.no_titles = No titles yet.
sources.library.no_matching = No matching titles.

# templates/websearch_evidence.html
sources.evidence.window_title = Web search evidence – Baruch
sources.evidence.title = Web search evidence
sources.evidence.back_group = Back
sources.evidence.to_section = To the section
sources.evidence.heading = Evidence for post { $post }
sources.evidence.intro = The web search of @belege fetches every page before it is named. Baruch checks every quotation against the fetched text: “verified” means the wording is on the page exactly like this. Only verified quotations can be taken into the author's library (title, address, quotation, highlight).
sources.evidence.preferred = preferred source
sources.evidence.accessed = { $url } · retrieved on { $accessed }
sources.evidence.quote = “{ $quote }”
sources.evidence.check = Check:
sources.evidence.in_library = In the library (title { $library_title })
sources.evidence.adopt = Into the library
sources.evidence.none = There is no web search evidence for this post.
