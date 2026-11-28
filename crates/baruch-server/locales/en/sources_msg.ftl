# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of literature, bibles and sources (Round 39a, task messages_1). Base catalog in English; locales/de/sources_msg.ftl has the same keys in
# the same order with the German wording.

# Literature, sources and highlights (literature.rs).
sources_msg.literature.pick_section = Choose a section: the persona answers in the section chat. Open the literature from a section (work page → Literature) and then give the text to the persona.
sources_msg.literature.not_linked = This source is not linked with MultiGPT; a full text exists only for documents from MultiGPT (semantic search → “Full text” → “Adopt into the library”).
sources_msg.literature.foreign_fulltext = Request from a foreign page; Baruch loads the full text through MultiGPT only when the page is opened here in Baruch.
sources_msg.images.title = Images of the work for “{ $query }”
sources_msg.images.alt = Alternative text: { $alt }
sources_msg.images.only_first = Only the first { $count } images; more on the page
sources_msg.images.page_link = Images
sources_msg.images.none = No image matches (searched in name, caption and alternative text).
sources_msg.images.template_error = Images: faulty template ({ $error })
sources_msg.images.read_error = Images of the work: { $error }
sources_msg.page.template_error = Template { $name }: { $error }
sources_msg.page.error = Error: { $error }
sources_msg.page.unknown_work = unknown work
sources_msg.page.title_missing = Source missing: give title={ $lt }id{ $gt }
sources_msg.page.unknown_title = unknown source
sources_msg.page.document_missing = Document missing: give document={ $lt }id{ $gt }
sources_msg.rpc.key_number = key: expected the fixed key of the section (a number)
sources_msg.rpc.title_or_document = give title (linked title) or document (document in MultiGPT)
sources_msg.rpc.mark_query_too_long = q: at most { $max } characters (the request only serves to mark the places found).
sources_msg.rpc.node_refused = node (position) is not accepted; give key
sources_msg.rpc.key_missing = key missing: give the section by its fixed key
sources_msg.rag.no_way = no way answered
sources_msg.rag.foreign_page = Request from a foreign page; Baruch searches by meaning only when the search is sent here in Baruch
sources_msg.rag.unavailable = Semantic search not available: { $reason }. The wording search (FTS) continues.
sources_msg.notice.owned = Adopted as your own highlight; the proposal of the persona stays as it was.
sources_msg.notice.deleted = Highlight deleted.
sources_msg.notice.saved = Highlight saved.
sources_msg.notice.adopted = Adopted into the library; the title is linked with the document in MultiGPT.
sources_msg.notice.checked = Highlight checked against the full text.
sources_msg.persona.answer_saved = The answer of the persona is saved.
sources_msg.persona.text_missing = text: give the marked text
sources_msg.persona.text_too_long = text: at most { $max } characters
sources_msg.persona.source_too_long = source: at most { $max } characters on one line
sources_msg.persona.no_lector = No persona is active (no lector).
sources_msg.highlight.already_own = This highlight is already your own.
sources_msg.highlight.already_adopted = You have already adopted this proposal as your own highlight.
sources_msg.highlight.persona_stays_proposal = Highlights of personas stay proposals; only your own are active.
sources_msg.highlight.not_checkable = Only highlights without a quotation and with a character range (start, end) can be checked against the full text.
sources_msg.form.title_missing = title: source missing
sources_msg.form.document_missing = document: document missing
sources_msg.form.highlight_missing = highlight: highlight missing
sources_msg.form.section_missing = Section missing.
sources_msg.form.confirm_delete = To delete, tick the box “really delete”.
sources_msg.form.unknown_action = unknown action { $action }

# Bible page of the literature (literature/bibles.rs).
sources_msg.bible_page.no_bible = No SWORD Bible available: you set the module folders and the Bible of the work under Settings → Sources → Bibles (SWORD).
sources_msg.bible_page.no_highlight = Highlighting works only for titles of the library (quotation with a character range); a Bible passage is no such title. Quoting and “give to persona” work.
sources_msg.bible_page.unreadable = Bibles not readable: { $error }
sources_msg.bible_page.search_timeout = Bible search aborted: time limit of 3 s reached.
sources_msg.bible_page.cited = Quotation created as a proposal in the section; only you can accept it.
sources_msg.bible_page.passage_invalid = Passage invalid
sources_msg.bible_page.no_text = The module { $module } has no text for “{ $reference }”.
sources_msg.bible_page.reference_and_bible = Give a passage and a Bible (e.g. “Ps 139:23-24”).
sources_msg.bible_page.unknown_action = Unknown action “{ $action }”.

# Full text of a source (literature/fulltext.rs).
sources_msg.fulltext.document_invalid = document: document ID of MultiGPT (1–500 characters, one line).
sources_msg.fulltext.collection_invalid = collection: at most 200 characters on one line.
sources_msg.fulltext.unavailable_now = The full text cannot be fetched right now: { $error }
sources_msg.fulltext.part_from_one = part: parts count from 1.
sources_msg.fulltext.no_fulltext = Title { $id } has no full text: neither linked with MultiGPT nor imported as PDF/EPUB.
sources_msg.fulltext.local_copy = MultiGPT cannot be reached ({ $error }); shown is the local copy of { $time }.
sources_msg.fulltext.imported = Full text from the imported file “{ $name }” (read locally, without MultiGPT).
sources_msg.fulltext.imported_at = Full text from the imported file “{ $name }” (read locally on { $time }, without MultiGPT).
sources_msg.fulltext.library_error = Library: { $error }
sources_msg.fulltext.checked_imported = Checked against the local full text from “{ $name }”.
sources_msg.fulltext.not_checkable = The source is not linked with MultiGPT and has no imported full text; without a full text the range cannot be checked.
sources_msg.fulltext.checked_copy = Checked against the local copy of { $time } (MultiGPT cannot be reached).
sources_msg.fulltext.unavailable = Full text cannot be fetched: { $error }

# SWORD Bibles, commentaries and lexicons (bibles.rs).
sources_msg.sword.commentary_missing = Commentary { $name } not found in any module folder
sources_msg.sword.lexicon_missing = Lexicon { $name } not found in any module folder
sources_msg.sword.commentary_problem = Commentary { $name }: { $problem }
sources_msg.sword.lexicon_problem = Lexicon { $name }: { $problem }
sources_msg.sword.ref_problem = “{ $input }”: { $problem }
sources_msg.sword.unreadable_ref = “{ $reference }” is not a readable reference
sources_msg.sword.no_entry = { $name } has no entry for this passage
sources_msg.sword.query_length = query: 1 to 200 characters
sources_msg.sword.no_lexicon = No lexicon chosen: Settings → Work → Commentaries and lexicons, or give module.
sources_msg.sword.lexicon_unreadable = Lexicon not readable: { $problems }
sources_msg.sword.key_length = key: 1 to 200 characters
sources_msg.sword.name_list = { $name }: expected a list of module names
sources_msg.sword.param_missing = { $name } missing
sources_msg.sword.key_number = key: expected a fixed key (a number)
sources_msg.sword.dir_invalid = Folder too long or with control characters (at most { $max } characters)
sources_msg.sword.no_home = ~ cannot be resolved (HOME missing)
sources_msg.sword.dir_relative = Folder “{ $dir }”: please give an absolute path
sources_msg.sword.too_many_dirs = at most { $max } module folders
sources_msg.sword.no_bible_chosen = No Bible chosen: Settings → Sources → Bibles (SWORD), or give module.
sources_msg.sword.bible_missing = Bible module { $name } not found in any module folder.
sources_msg.sword.bible_problem = Bible module { $name }: { $problem }
sources_msg.sword.unknown_section = Section unknown
sources_msg.sword.no_revision = Revision { $number } does not exist
sources_msg.sword.section_without_revision = Section without a revision
sources_msg.sword.text_too_long = text: at most { $max } characters
sources_msg.sword.cipher_key_length = key: 1 to { $max } characters on one line; nothing saved.
sources_msg.sword.module_missing = Module { $name } not found in any module folder
sources_msg.sword.not_enciphered = Module { $name } is not enciphered; no key needed
sources_msg.sword.keyring_refused_set = The keyring refuses ({ $error }); nothing saved.
sources_msg.sword.confirm_delete = Please confirm the deletion (confirm: true); nothing changed.
sources_msg.sword.keyring_refused_delete = The keyring refuses ({ $error }); nothing changed.
sources_msg.sword.dirs_list = dirs: expected a list of texts
sources_msg.sword.dirs_list_or_null = dirs: a list of texts or null (defaults)
sources_msg.sword.module_or_null = module: a module name or null
sources_msg.sword.ref_too_long = ref: at most 500 characters
sources_msg.sword.rev_number = rev: expected the number of a revision
sources_msg.sword.ref_length = ref: 1 to 500 characters
sources_msg.sword.prefix_too_long = prefix: at most 200 characters
sources_msg.commentary.summary = Commentaries on { $reference }
sources_msg.commentary.one_entry = { $count } entry
sources_msg.commentary.entries = { $count } entries
sources_msg.commentary.group = Commentary · { $module }
sources_msg.commentary.shortened = (shortened)
sources_msg.commentary.more_hits = further hits not shown (limit reached)
sources_msg.commentary.license = License: { $notice }
sources_msg.commentary.no_notice = not stated
