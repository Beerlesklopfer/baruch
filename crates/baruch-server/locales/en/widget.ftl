# SPDX-License-Identifier: AGPL-3.0-or-later
# Texts of the TypeScript widgets (Round 39a, task widgets): keys widget.<module>.<name>, read in the browser
# with t() from web/src/i18n.ts (catalog element #baruch-i18n in the page head). Only simple { $name }
# placeables (no selectors: a plural is two keys). Same keys in the same order as locales/de/widget.ftl.

# assist.ts
widget.assist.cancelled_chat = Cancelled. Your post and any answers already finished are saved; the running answer is not.
widget.assist.cancelled_nothing = Cancelled; nothing was saved.
widget.assist.waiting = waiting for the model …
widget.assist.cancel = Cancel
widget.assist.writing = { $persona } is writing …
widget.assist.retry = { $persona }: requesting the answer again ({ $reason }) …
widget.assist.compressed = Compressed.
widget.assist.error = Error
widget.assist.status_cancelled = cancelled
widget.assist.status_not_saved = not saved

# band.ts
widget.band.no_persona = No persona is active – set one up under Settings → Assistants.
widget.band.nothing_to_save = Nothing to save – the text is unchanged.

# chat.ts
widget.chat.new_badge = new
widget.chat.new_title = New since your last visit to this section
widget.chat.new_count = { $count } new
widget.chat.new_since = { $count } new since your last visit
widget.chat.copied = Copied
widget.chat.copy_failed = Copying failed – select the text
widget.chat.copy = Copy
widget.chat.quote = Quote
widget.chat.open_post = Open post
widget.chat.older = Older posts ({ $count }) …
widget.chat.delete_question = Delete post { $id } by { $author }? The mirror keeps it in the deletion commit.
widget.chat.edited = edited
widget.chat.typing_one = { $name } is writing …
widget.chat.and = and
widget.chat.typing_many = { $names } are writing …

# commentaries.ts
widget.commentaries.expand_all = expand all
widget.commentaries.collapse_all = collapse all

# consent.ts
widget.consent.not_done = Not carried out ({ $label }): { $message }
widget.consent.error = Error: { $text }

# context_menu.ts
widget.context_menu.glossary_add_item = Add to glossary …
widget.context_menu.index_add_item = Add to index …
widget.context_menu.glossary_lookup_item = Look up in the glossary
widget.context_menu.index_edit_item = Edit index entry …
widget.context_menu.index_remove_item = Remove index entry
widget.context_menu.term_empty = Term: must not be empty.
widget.context_menu.menu_label = Index and glossary
widget.context_menu.cancel = Cancel
widget.context_menu.field_term = Entry
widget.context_menu.field_sub = Subentry
widget.context_menu.field_register = Index
widget.context_menu.field_see = see
widget.context_menu.field_also = see also
widget.context_menu.glossary_add_title = Add to glossary
widget.context_menu.field_glossary_term = Term
widget.context_menu.field_short = Short explanation
widget.context_menu.field_long = Detailed explanation
widget.context_menu.field_variants = Variants (one per line)
widget.context_menu.field_bible_refs = Bible references (one per line)
widget.context_menu.field_source = Source
widget.context_menu.glossary_add_ok = Add
widget.context_menu.glossary_added = “{ $term }” is now in the glossary.
widget.context_menu.no_word = Please select a word or put the cursor into a word.
widget.context_menu.lookup_title = In the glossary: { $word }
widget.context_menu.searching = Searching …
widget.context_menu.close = Close
widget.context_menu.not_in_glossary = “{ $word }” is not in the glossary.
widget.context_menu.variants = also: { $list }
widget.context_menu.index_add_title = Add to index
widget.context_menu.index_add_ok = Set mark
widget.context_menu.sheet_closed = The typeset view is not open.
widget.context_menu.index_edit_title = Edit index entry
widget.context_menu.apply = Apply
widget.context_menu.no_entries = No index entries in this section.
widget.context_menu.entry_see = see: { $see }
widget.context_menu.entry_also = see also: { $also }

# cover_editor.ts
widget.cover_editor.hidden = hidden
widget.cover_editor.position = { $label }: x { $x } mm, y { $y } mm, width { $width } mm
widget.cover_editor.nothing_to_undo = Nothing to undo.
widget.cover_editor.undone = Undone: { $label }.
widget.cover_editor.nothing_to_save = Nothing to save.
widget.cover_editor.saving = Saving …
widget.cover_editor.saved_one = Saved (1 block).
widget.cover_editor.saved_other = Saved ({ $count } blocks).
widget.cover_editor.print_check_one = Print check: 1 note – reloading the page shows it.
widget.cover_editor.print_check_other = Print check: { $count } notes – reloading the page shows them.
widget.cover_editor.mirror = Mirror: { $commit }
widget.cover_editor.discard = Discard unsaved changes to the layout?
widget.cover_editor.choose_block = Choose a block first.
widget.cover_editor.color_dark = { $label }: colour dark.
widget.cover_editor.color_light = { $label }: colour light.
widget.cover_editor.quick = Quick choice { $name }
widget.cover_editor.reset_all = the whole layout
widget.cover_editor.reset_faces = the chosen faces
widget.cover_editor.reset_question = Reset the layout ({ $what })? The defaults apply again.
widget.cover_editor.reset = Reset

# findings.ts
widget.findings.server_status = Server answers { $status }
widget.findings.no_findings = Answer without findings

# glossary.ts
widget.glossary.shown_of = { $shown } of { $count }

# ideas.ts
widget.ideas.source_titled = Literature: { $title }
widget.ideas.source = Literature
widget.ideas.words_missing = Please give at least one keyword.
widget.ideas.created = Idea { $id } noted.
widget.ideas.mirror = Git mirror: { $commit }

# jump.ts
widget.jump.bar_label = Jump history
widget.jump.back_chat = Back to the chat
widget.jump.back_chat_title = Back to the place in the chat (Alt+←)
widget.jump.back = ← Back
widget.jump.back_title = To where the jump started (Alt+←)
widget.jump.forward = Forward →
widget.jump.forward_title = Repeat the last jump

# literature.ts
widget.literature.kind_label = Kind of highlight
widget.literature.kind_core = Key statement
widget.literature.highlight = Highlight
widget.literature.cite = Quote
widget.literature.persona = Give to a persona
widget.literature.similar = Find similar
widget.literature.no_section_cite = Section missing: open the document view from a section.
widget.literature.no_section_persona = Choose a section: open the literature from a section, then the persona gets the text.
widget.literature.source = Literature
widget.literature.answering = The persona is answering …

# mention.ts
widget.mention.list_label = Personas
widget.mention.like = like { $handle }
widget.mention.also = also: “{ $name }, …” at the start of a sentence

# menu.ts
widget.menu.open_count = { $count } open

# mindmap.ts
widget.mindmap.new_title = New idea
widget.mindmap.kind_leads_to = leads on to
widget.mindmap.kind_belongs_to = belongs to
widget.mindmap.kind_example_of = example of
widget.mindmap.kind_contradicts = contradicts
widget.mindmap.undo = Undo
widget.mindmap.refresh_status = The picture could not be fetched again (server answers { $status }).
widget.mindmap.refresh_empty = The picture could not be fetched again (no mind map in the answer).
widget.mindmap.cloud_count = { $count } in the cloud
widget.mindmap.nothing_to_undo = Nothing to undo.
widget.mindmap.undone = Undone: { $label }.
widget.mindmap.rename_only_ideas = Only ideas can be renamed; the work, parts and chapters are named as in the outline.
widget.mindmap.title_field = Title of the idea (Enter applies, Esc discards)
widget.mindmap.undo_rename = Rename
widget.mindmap.renamed = Renamed to “{ $title }”.
widget.mindmap.no_new_idea = The server reported no new idea.
widget.mindmap.undo_create = Create
widget.mindmap.created_cloud = Idea created in the cloud. Type a title, Enter applies.
widget.mindmap.created = Idea created. Type a title, Enter applies.
widget.mindmap.undo_unlink = Remove connection
widget.mindmap.unlinked = Connection removed.
widget.mindmap.delete_only_ideas = Only ideas can be deleted; parts and chapters are changed by the outline (request).
widget.mindmap.delete_question_children_one = Delete idea “{ $title }” with { $count } sub-node? (can be restored)
widget.mindmap.delete_question_children_other = Delete idea “{ $title }” with { $count } sub-nodes? (can be restored)
widget.mindmap.delete_question = Delete idea “{ $title }”? (can be restored)
widget.mindmap.undo_delete = Delete
widget.mindmap.deleted_children_one = Idea “{ $title }” with { $count } sub-node deleted.
widget.mindmap.deleted_children_other = Idea “{ $title }” with { $count } sub-nodes deleted.
widget.mindmap.deleted = Idea “{ $title }” deleted.
widget.mindmap.undo_move = Move
widget.mindmap.cloud_only_ideas = Only ideas can be put into the cloud.
widget.mindmap.already_cloud = This idea is already in the cloud.
widget.mindmap.to_cloud_children_one = “{ $title }” with { $count } sub-node is now in the cloud.
widget.mindmap.to_cloud_children_other = “{ $title }” with { $count } sub-nodes is now in the cloud.
widget.mindmap.to_cloud = “{ $title }” is now in the cloud.
widget.mindmap.main_topic = “{ $title }” is now a main topic of the work.
widget.mindmap.not_below_itself = An idea cannot be hung below its own sub-nodes.
widget.mindmap.hangs_at = “{ $title }” now hangs at “{ $target }”.
widget.mindmap.child_of = “{ $title }” is now a sub-node of “{ $target }”.
widget.mindmap.now_before = “{ $title }” now stands before “{ $target }”.
widget.mindmap.now_after = “{ $title }” now stands after “{ $target }”.
widget.mindmap.link_choose = Connect: choose an idea first.
widget.mindmap.link_from = Connecting from “{ $title }”: now choose the second idea (click, or arrows and Enter; Esc cancels).
widget.mindmap.link_two = Connecting works only between two different ideas.
widget.mindmap.link_title = Connect “{ $from }” with “{ $to }”
widget.mindmap.link_kind = Kind
widget.mindmap.link_note = Note (label of the line)
widget.mindmap.link_ok = Connect
widget.mindmap.cancel = Cancel
widget.mindmap.link_exists = This connection exists already; nothing changed.
widget.mindmap.linked = Connected.
widget.mindmap.link_selected = Connection selected: Del removes it (can be restored with Ctrl+Z).
widget.mindmap.link_cancelled = Connecting cancelled.
widget.mindmap.rename_choose = Rename: choose an idea first.
widget.mindmap.delete_choose = Delete: choose an idea first.
widget.mindmap.zoom_out = Zoom out (-)
widget.mindmap.zoom_in = Zoom in (+)
widget.mindmap.fit = Fit
widget.mindmap.fit_title = Show the whole mind map (0)
widget.mindmap.help = Click selects, double click or F2 renames. Tab: sub-node, Enter: sibling, Del: delete, Space: fold, arrows: move around, Ctrl+Z: undo, Alt+L: connect, Alt+E: new idea in the cloud. Dragging re-hooks: onto a node, between siblings or into the cloud. Ctrl+mouse wheel, +, − and 0 zoom.

# outline.ts
widget.outline.level_part = Part
widget.outline.level_chapter = Chapter
widget.outline.level_section = Section
widget.outline.level_subsection = Subsection
widget.outline.judge_before = before “{ $title }” · level { $level } ({ $name })
widget.outline.judge_after = after “{ $title }” · level { $level } ({ $name })
widget.outline.judge_into = below “{ $title }” · level { $level } ({ $name })
widget.outline.add_part_before = Title of the new part before “{ $title }”
widget.outline.add_item_before = Title of the new item before “{ $title }”
widget.outline.add_part_after = Title of the new part after “{ $title }”
widget.outline.add_item_after = Title of the new item after “{ $title }”
widget.outline.add_part_into = Title of the new part below “{ $title }”
widget.outline.add_item_into = Title of the new item below “{ $title }”
widget.outline.keeps_level = Moving keeps the level (a chapter stays a chapter); change the level first (← / →).
widget.outline.title_empty = An outline item needs a title.
widget.outline.title_newline = The title must not contain a line break.
widget.outline.title_long = The title is too long (at most { $max } characters, here { $count }).
widget.outline.own_subtree = Not into its own subtree.
widget.outline.new_title = New title
widget.outline.has_children = The outline item has sub-items; move or delete them first.
widget.outline.delete_question =
    Delete “{ $title }” with all its versions and posts?
    The text stays readable in the Git mirror.
widget.outline.invalid_headings = These headings do not make a valid outline.
widget.outline.split_question =
    The text contains new headings: { $headings }.
    Split the section and save?
widget.outline.no_heading = The text has no heading any more.
widget.outline.merge_question = Merge and save?

# panes.ts
widget.panes.theme_auto = System
widget.panes.theme_light = Light
widget.panes.theme_dark = Dark
widget.panes.theme = Theme: { $name }

# proposals.ts
widget.proposals.collapse_all = Collapse all
widget.proposals.expand_all = Expand all

# ribbon.ts
widget.ribbon.not_in_sheet = “{ $label }” does not work in the typeset view yet – please insert it in the source (button “Source”).
widget.ribbon.no_field = “{ $label }”: no text field found to insert into.
widget.ribbon.no_image = “Image”: inserting images is not available here.

# rpc.ts
widget.rpc.server_status = Server answers { $status }
widget.rpc.saved_amended = Saved: continued (state { $seq }).
widget.rpc.saved_new_reason = Saved: new version { $rev } ({ $reason }).
widget.rpc.saved_new = Saved: new version { $rev }.
widget.rpc.unchanged = Nothing to save: the text is unchanged.

# scripture_check.ts
widget.scripture_check.state_matches = matches
widget.scripture_check.state_differs = differs
widget.scripture_check.state_no_quote = no quotation
widget.scripture_check.state_unchecked = unchecked
widget.scripture_check.state_error = cannot be checked
widget.scripture_check.panel_label = Result of the quotation check
widget.scripture_check.counts = { $matches } matching · { $differs } differing · { $others } other
widget.scripture_check.checked_with = Checked by Baruch with { $module } (versification { $versification })
widget.scripture_check.bible_unreadable = Bible not readable: { $problem }
widget.scripture_check.no_bible = The work has no Bible: the wording stays unchecked.
widget.scripture_check.license = Licence (according to conf): { $notice }
widget.scripture_check.read_as = read: { $ref }
widget.scripture_check.none_found = No Bible reference found.
widget.scripture_check.open_page = Open as a page
widget.scripture_check.old_browser = Marking in the typeset view needs a newer browser.
widget.scripture_check.painted = { $count } quotation(s) marked in the typeset view
widget.scripture_check.missed = { $count } quotation(s) not found unambiguously in the typeset view (only in the list)
widget.scripture_check.failed = Check not possible: { $error }
widget.scripture_check.quoted = “{ $quote }”

# search.ts
widget.search.no_hits = No hits
widget.search.hit_of = { $index } of { $count }
widget.search.timed_out = The search took longer than 2 s and was cancelled.
widget.search.nothing_to_replace = Nothing to replace.
widget.search.replaced_one = Replaced (not saved yet).
widget.search.replaced_many = { $count } places replaced (not saved yet).
widget.search.select_first = First select text in the source, then open the search and switch on “In selection”.
widget.search.work_title = Search in the work
widget.search.close = Close
widget.search.find = Search
widget.search.replace = Replace
widget.search.word = Word
widget.search.preview_replace = Preview and replace …
widget.search.total_one = { $count } hit · { $range }
widget.search.total_other = { $count } hits · { $range }
widget.search.cut_off = (cancelled after 2 s)

# settings.ts
widget.settings.unexpected = Unexpected answer from the server.
widget.settings.load_failed = The settings could not be loaded: { $error }
widget.settings.save_failed = Saving failed: { $error }

# sheet.ts
widget.sheet.at_cursor = at the cursor position
widget.sheet.image_note = As a block of its own at the chosen place; unsaved changes go along.

# threads.ts
widget.threads.answering = Personas are answering …

# timing.ts
widget.timing.over_by = Target time exceeded by { $time }
widget.timing.over = Target time exceeded

# toast.ts
widget.toast.summarized = Seen only as a summary ({ $count })
widget.toast.close_label = Close the notice
widget.toast.close = Close

# typeset_settings.ts
widget.typeset_settings.label_chapter_start = Chapter start
widget.typeset_settings.label_part_page = Part page
widget.typeset_settings.label_page_format = Page format
widget.typeset_settings.label_margins = Margins
widget.typeset_settings.label_paragraph = Paragraph
widget.typeset_settings.label_justify = Justification
widget.typeset_settings.label_hyphenate = Hyphenation
widget.typeset_settings.label_page_numbers = Page numbers
widget.typeset_settings.label_running_head = Running head
widget.typeset_settings.label_footnotes = Footnotes
widget.typeset_settings.label_widows_orphans = Widows/orphans
widget.typeset_settings.label_xref_with_page = Reference with page number
widget.typeset_settings.label_glossary_page = Glossary page
widget.typeset_settings.label_glossary_first_mention = Mark the first mention in the glossary
widget.typeset_settings.unknown_choice = { $label }: unknown choice “{ $value }”
widget.typeset_settings.not_a_number = { $field }: not a number
widget.typeset_settings.page_format_both = Page format: give width and height
widget.typeset_settings.margins_all = Margins: give all four margins or leave all empty
widget.typeset_settings.draft_unknown = Draft: unknown choice “{ $value }”
widget.typeset_settings.server_status = Server answers { $status }
widget.typeset_settings.preview_pages_one = Preview with sample text: { $count } page
widget.typeset_settings.preview_pages_other = Preview with sample text: { $count } pages
widget.typeset_settings.preview_errors_one = { $count } error
widget.typeset_settings.preview_errors_other = { $count } errors
widget.typeset_settings.preview_warnings_one = { $count } warning
widget.typeset_settings.preview_warnings_other = { $count } warnings
widget.typeset_settings.preview_page = Preview page { $page }

# variants.ts
widget.variants.reject_rest = Discard the remaining variants

# widgets.ts
widget.widgets.discard = Discard the unsaved change?

# works.ts
widget.works.from_file_name = from the file name

# workspace.ts
widget.workspace.editor = Editor
widget.workspace.preview = Preview

# ws_command.ts
widget.ws_command.register_stichwort = Subject index
widget.ws_command.register_personen = Index of persons
widget.ws_command.register_bibel = Index of Bible references
widget.ws_command.bad_slug = Index “{ $slug }”: not a valid name (a lowercase letter first, then lowercase letters, digits or hyphens, at most 32 characters).
widget.ws_command.term_empty = Entry: must not be empty.
widget.ws_command.field_term = Entry
widget.ws_command.field_sub = Subentry
widget.ws_command.field_see = see
widget.ws_command.field_also = see also
widget.ws_command.field_sort = Sort key
widget.ws_command.too_long = { $label }: at most { $max } characters.
widget.ws_command.control_chars = { $label }: no line breaks or control characters.
widget.ws_command.register_named = Index “{ $slug }”
widget.ws_command.mark_label = Index entry in { $register }: { $term }
widget.ws_command.mark_see = see: { $see }
widget.ws_command.mark_also = see also: { $also }
widget.ws_command.refuse_heading = No index mark in a heading line; please choose a word in the running text.
widget.ws_command.refuse_comment = No index mark in a comment.
widget.ws_command.refuse_inside = There is already an index mark here.
widget.ws_command.no_word = Please select a word or put the cursor into a word.
widget.ws_command.duplicate = This word already carries this index mark; nothing changed.

# wysiwyg.ts
widget.wysiwyg.title_empty = An outline item needs a title.
widget.wysiwyg.title_newline = The title must not contain a line break.
widget.wysiwyg.title_long = The title is too long (at most { $max } characters, here { $count }).
widget.wysiwyg.format_part = Parts and unnumbered items are changed by the outline or the source, not by the paragraph format.
widget.wysiwyg.format_only = The paragraph format can only be chosen for paragraphs and headings.
widget.wysiwyg.mark_unreadable = Index mark: unreadable command.
widget.wysiwyg.mark_gone = The index mark is no longer in the typeset view.
widget.wysiwyg.select_word = Please select a word in the typeset view first.
widget.wysiwyg.mark_heading = No index mark in a heading line or Typst island; please choose a word in the running text.
widget.wysiwyg.mark_here = No index mark can be set here; please choose a word in the running text.
widget.wysiwyg.mark_duplicate = This word already carries this index mark; nothing changed.
widget.wysiwyg.cursor_first = Please put the cursor into a paragraph first.
widget.wysiwyg.prompt_scripture = Bible reference (e.g. John 3:16):
widget.wysiwyg.scripture_refused = This Bible reference cannot be inserted like this (at most 200 characters, a function of the template is needed).
widget.wysiwyg.island = Typst island
widget.wysiwyg.prompt_cite = Key of the literature reference (e.g. mueller2020):
widget.wysiwyg.cite_invalid = Invalid key (letters, digits, _ - : .).
widget.wysiwyg.ref_title = Reference to { $target } (changeable only in the source)
widget.wysiwyg.linebreak = forced line break (\)
widget.wysiwyg.xref_title = Cross-reference to { $key } (Ctrl+click: jump)
widget.wysiwyg.prompt_link = Address of the link (e.g. https://…):
widget.wysiwyg.link_title = Link: { $url }
widget.wysiwyg.placeholder_quote = Quotation
widget.wysiwyg.placeholder_footnote = Footnote
widget.wysiwyg.list_from_paragraph = Lists, numbered lists and term lists are made from a paragraph (put the cursor into it).
widget.wysiwyg.list_not_terms = A list does not become a term list; please choose a paragraph.
widget.wysiwyg.placeholder_term = Term
widget.wysiwyg.scene_exists = There is already a scene or a work assignment here.
widget.wysiwyg.island_source = Source of the Typst island
widget.wysiwyg.apply = Apply
widget.wysiwyg.cancel = Cancel
widget.wysiwyg.ref_elsewhere = Reference to { $target } – the target is not in this section; change it only in the source.

# xref.ts
widget.xref.dialog_label = Insert cross-reference
widget.xref.heading = Cross-reference
widget.xref.find_target = Find target
widget.xref.find_placeholder = Title or number
widget.xref.targets_label = Outline items
widget.xref.link_text = Link text
widget.xref.with_page = with page number (“S. n”)
widget.xref.insert = Insert
widget.xref.cancel = Cancel
widget.xref.choose_target = Please choose a target.
widget.xref.open = Open
widget.xref.open_title = Jump to the target (Ctrl+click)
widget.xref.edit = Edit
widget.xref.xref_title = Cross-reference to { $key } (Ctrl+click: jump)
