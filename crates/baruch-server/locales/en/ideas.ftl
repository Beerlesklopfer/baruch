# SPDX-License-Identifier: AGPL-3.0-or-later
# Round 39a (templates_2): Ideas pages and the mind map (picture, reading in, outline from a mind map file).
# Base catalog in English; locales/de/ideas.ftl has the same keys in the same order.

# Shared by the ideas pages and the mind map.
ideas.see_also = See also
ideas.back_to_work = Back to the work
ideas.to_work = To the work
ideas.mindmap = Mind map
ideas.outline_view = Outline view
ideas.work = Work
ideas.without_work = without a work
ideas.open = open
ideas.done = processed
ideas.done_as = as { $label }
ideas.deleted_at = deleted { $date }
ideas.idea_number = Idea { $id }
ideas.cloud = Cloud
ideas.section_quoted = Section “{ $title }”
ideas.source = Source
ideas.source_is = Source: { $source }
ideas.bible = Bible reference
ideas.bible_example = e.g. Hld 2,12
ideas.reference = Reference
ideas.thought = Thought
ideas.quote = Marked text
ideas.optional = optional
ideas.words_example = e.g. arbour, roses
ideas.restore = Restore

# Page /einfaelle (ideas.html).
ideas.list.window_title = Ideas – Baruch
ideas.list.heading = Ideas
ideas.list.heading_deleted = – deleted
ideas.list.map_hint = The ideas of this work as a mind map around chapters and parts, with the cloud beside it
ideas.list.filter_label = Filter ideas
ideas.list.all = all
ideas.list.status = Status
ideas.list.keyword = Keyword
ideas.list.keywords = Keywords
ideas.list.search = Search
ideas.list.search_placeholder = Search words
ideas.list.search_by = Search by
ideas.list.mode_fts = Wording (FTS)
ideas.list.mode_rag = Meaning (RAG)
ideas.list.filter = Filter
ideas.list.show_present = Show present ones
ideas.list.show_deleted = Show deleted ones
ideas.list.no_keywords = No keywords yet.
ideas.list.all_keywords = all keywords
ideas.list.rag_hits = Hits of the semantic search for “{ $query }”:
ideas.list.same_search_fts = The same search by wording (FTS)
ideas.list.fts_hits = Hits of the full-text search for “{ $query }”:
ideas.list.cloud_hint = Without a place in the mind map
ideas.list.child_hint = Sub-node in the mind map
ideas.list.under_idea = under idea { $id }
ideas.list.similarity = Similarity { $percent } %
ideas.list.no_hits = No hits.
ideas.list.none_for_filter = No ideas for this filter.
ideas.list.only_weak = Only weak hits.
ideas.list.weak_hits = { $count ->
    [one] { $count } weak hit
    *[other] { $count } weak hits
    }
ideas.list.new = New idea
ideas.list.new_title = Title (node of the mind map)
ideas.list.new_words = Keywords (separated by commas; a title or a keyword is required)
ideas.list.capture = Capture

# Page /einfaelle/{id} (idea.html).
ideas.idea.window_title = Idea { $id } – Baruch
ideas.idea.all_ideas = All ideas
ideas.idea.child_of = Sub-node of
ideas.idea.at_chapter = at the chapter of the reference
ideas.idea.main_topic = main topic on the work
ideas.idea.in_cloud = in the cloud (without a place)
ideas.idea.origin = Origin
ideas.idea.changed = Changed
ideas.idea.links = Links
ideas.idea.link_note = – “{ $note }”
ideas.idea.unlink_hint = Remove the link (restorable)
ideas.idea.unlink = Remove
ideas.idea.no_links = No links.
ideas.idea.link_in_map = Link in the mind map …
ideas.idea.deleted_hint = The idea is deleted. Restoring brings it back into the list, the search and the mirror.
ideas.idea.edit = Edit
ideas.idea.without_section = without a section
ideas.idea.place = Place in the mind map
ideas.idea.under_number = Into the mind map below idea no.
ideas.idea.hang_in = Hang in
ideas.idea.to_cloud_hint = Removes the parent node and the reference; the sub-nodes stay with the idea
ideas.idea.to_cloud = Put into the cloud
ideas.idea.to_main_topic = As a main topic onto the work
ideas.idea.place_hint = Without a place an idea lies in the cloud beside the mind map. Hanging in, removing and ordering also work in the mind map itself.
ideas.idea.process = Process
ideas.idea.becomes = This becomes
ideas.idea.as_outline = a new outline item
ideas.idea.as_post = a post in the section chat
ideas.idea.as_note = a note on the section
ideas.idea.as_persona = a job for a persona
ideas.idea.as_persona_hint = (its answer becomes a post in the section chat)
ideas.idea.persona = Persona for the job
ideas.idea.target_section = Section (target or place of the post)
ideas.idea.new_item = New item
ideas.idea.after = after it
ideas.idea.before = before it
ideas.idea.into = below it
ideas.idea.new_item_title = Title of the new item
ideas.idea.needs_work = An idea without a work can only be processed once it is assigned to a work.
ideas.idea.delete_confirm = Yes, delete this idea (restorable under “Deleted ones” and through the Git mirror)

# Page /werke/{id}/karte (karte.html).
ideas.map.window_title = Mind map · { $title } – Baruch
ideas.map.page_title = Mind map · { $title }
ideas.map.group_node = Node
ideas.map.child_hint = Create a sub-node (Tab) – without JavaScript in the outline view
ideas.map.child = Sub-node
ideas.map.sibling_hint = Create a sibling node (Enter) – without JavaScript in the outline view
ideas.map.sibling = Sibling
ideas.map.rename_hint = Rename the chosen idea (F2, double click) – without JavaScript in the outline view
ideas.map.rename = Rename
ideas.map.delete_hint = Delete the chosen idea with its sub-nodes (Del), restorable – without JavaScript in the outline view
ideas.map.link_hint = Draw a relation from the chosen idea to the next one chosen (Alt+L) – without JavaScript in the outline view
ideas.map.link = Link
ideas.map.cloud_hint = Put the chosen idea with its sub-nodes into the cloud – without JavaScript in the outline view
ideas.map.to_cloud = Into the cloud
ideas.map.undo_hint = Undo the last action on the mind map (Ctrl+Z) – without JavaScript in the outline view
ideas.map.undo = Undo
ideas.map.group_view = View
ideas.map.open_all_hint = Expand all branches
ideas.map.open_all = Expand all
ideas.map.close_all_hint = Collapse all main branches
ideas.map.close_all = Collapse all
ideas.map.near_hint = Nearness: dotted lines between similar ideas and the proposal “fits branch …” in the cloud
ideas.map.near_hide = Hide nearness
ideas.map.near_show = Show nearness
ideas.map.findings_hint = Findings layer: open findings as a number on the chapter branch
ideas.map.findings_hide = Hide findings layer
ideas.map.findings_show = Show findings layer
ideas.map.group_output = Output
ideas.map.out_xmind = Mind map for XMind
ideas.map.out_mm = Mind map for FreeMind and Freeplane
ideas.map.out_svg = Mind map as an SVG graphic
ideas.map.out_pdf = Mind map as PDF
ideas.map.out_md = Tree, relations and contradictions as Markdown
ideas.map.group_read_in = Read in
ideas.map.read_in_hint = Read in a mind map from XMind (.xmind) or FreeMind/Freeplane (.mm) – with a preview
ideas.map.read_in = Read in a file …
ideas.map.outline_view_hint = The mind map as a nested list with forms (usable without JavaScript)
ideas.map.filter_label = Filter the mind map
ideas.map.origin_human = Human
ideas.map.search_placeholder = Highlight words
ideas.map.findings_layer = Findings layer
ideas.map.apply_filter = Apply
ideas.map.clear_filter = Clear filter
ideas.map.findings_note = Findings layer: number “k open” per part and chapter (open findings including sections); branches with { $count } or more open ones are highlighted.
ideas.map.all_findings = All findings
ideas.map.filter_words = Filter: { $words }
ideas.map.stats = { $ideas } ideas · { $links } links ·
ideas.map.in_cloud = { $count } in the cloud
ideas.map.name_groups_hint = The lector (or the chosen persona) proposes a name for each group of near ideas; it becomes a keyword only when you accept it
ideas.map.name_groups = Name groups (assistant)
ideas.map.nojs_hint = Without JavaScript: “−” and “+ n” on a node collapse the branch, “§” leads to the section, the title to the idea. Creating, renaming, moving (“into the mind map below …”, “into the cloud”) and deleting are available as forms in the
ideas.map.nojs_hint_after = . In the cloud below the tree lie the ideas without a place.
ideas.map.legend = Legend of the relations
ideas.map.key_leads_to = leads on (solid, arrow)
ideas.map.key_belongs_to = belongs to (dashed, arrow)
ideas.map.key_example_of = example of (dotted, arrow)
ideas.map.key_contradicts = contradicts (red, dash-dot, no arrow)
ideas.map.key_near = nearness “near” (finely dotted, only shown)
ideas.map.groups = Groups
ideas.map.groups_hint = A group are ideas connected through nearness (at least two). “Name groups (assistant)” fetches names as a proposal; they last only until the next restart of Baruch. A name becomes a keyword on the ideas only when you accept it (one commit per idea).
ideas.map.group_words = – keywords: { $words }
ideas.map.proposal_by = Proposal by
ideas.map.proposal_name = “{ $name }”
ideas.map.name = Name
ideas.map.accept_hint = Set the name as a keyword on every idea of the group
ideas.map.accept = Accept
ideas.map.dismiss = Discard
ideas.map.name_self = Name it yourself …
ideas.map.set_keyword = Set as keyword
ideas.map.no_groups = No groups: no two ideas shown are near each other.
ideas.map.near_off = Nearness is switched off; groups exist only with “Show nearness”.
ideas.map.contradictions = Contradictions
ideas.map.contradicts = contradicts
ideas.map.no_contradictions = No contradictions recorded. “Link” with the kind “contradicts” connects two ideas.

# Page /werke/{id}/karte/einlesen (karte_einlesen.html).
ideas.read_in.window_title = Read in a mind map · { $title } – Baruch
ideas.read_in.page_title = Read in a mind map · { $title }
ideas.read_in.other_file = Another file …
ideas.read_in.heading = Read in a mind map
ideas.read_in.intro = Baruch reads a mind map from XMind (.xmind), FreeMind or Freeplane (.mm) and first shows a preview. Nothing is created until you confirm the preview. The file stays on this computer. Titles, notes, keywords (labels), order and relations are taken over; images, styles and markers are not. The main topic of the file does not itself become an idea.
ideas.read_in.file = File (.xmind or .mm, at most { $mib } MiB, { $nodes } nodes, { $depth } levels)
ideas.read_in.where = Where to
ideas.read_in.as_main_topics = as main topics onto the work (branches of their own)
ideas.read_in.under = below { $label }
ideas.read_in.show_preview = Show preview
ideas.read_in.preview = Preview: { $title }
ideas.read_in.untitled = (untitled)
ideas.read_in.nodes = { $count ->
    [one] { $count } node
    *[other] { $count } nodes
    }
ideas.read_in.links = , { $count } relation(s)
ideas.read_in.become_children = become sub-nodes of “{ $parent }”
ideas.read_in.become_main = become main topics on the work (branches of their own in the mind map)
ideas.read_in.nothing_yet = Nothing has been created yet.
ideas.read_in.warnings = Notes
ideas.read_in.create = Create
ideas.read_in.cloud_hint = Lay in the cloud of the file and goes back into the cloud

# Page /werke/{id}/karte/gliederung (karte_gliederung.html).
ideas.outline.window_title = Mind map (outline view) · { $title } – Baruch
ideas.outline.page_title = Mind map · Outline view · { $title }
ideas.outline.map_hint = The mind map as a picture
ideas.outline.xmind_file = XMind file
ideas.outline.read_in_hint = Read in a mind map from XMind, FreeMind or Freeplane
ideas.outline.read_in = Read in …
ideas.outline.chosen_node = Chosen node
ideas.outline.chosen = Chosen: { $title }
ideas.outline.structure_hint = Parts and chapters belong to the outline of the work: renaming, moving and deleting exist only there. Here an idea can be attached.
ideas.outline.child_title = Sub-node (title)
ideas.outline.comma_separated = separated by commas
ideas.outline.add_child = Create sub-node
ideas.outline.hangs_under = Hangs below: { $parent }
ideas.outline.children_count = { $count ->
    [one] { $count } sub-node
    *[other] { $count } sub-nodes
    }
ideas.outline.title = Title
ideas.outline.sibling_title = Sibling (title)
ideas.outline.add_sibling = Create sibling
ideas.outline.into_map_under = Into the mind map below
ideas.outline.move_under = Move below
ideas.outline.please_choose = – please choose –
ideas.outline.here = (here)
ideas.outline.hang_into_map = Hang into the mind map
ideas.outline.move = Move
ideas.outline.main_topic_hint = Hangs the idea as a main branch of its own directly onto the work; its sub-nodes stay with it
ideas.outline.cloud_hint = Removes the idea from its parent node and reference; its sub-nodes stay with it
ideas.outline.with_children = { $count ->
    [one] with { $count } sub-node
    *[other] with { $count } sub-nodes
    }
ideas.outline.order = Order
ideas.outline.up = up
ideas.outline.down = down
ideas.outline.reference = Reference (link to the section)
ideas.outline.no_reference = – no reference –
ideas.outline.set_reference = Set reference
ideas.outline.link_with = Link with
ideas.outline.choose_idea = – choose an idea –
ideas.outline.kind = Kind
ideas.outline.caption = Caption
ideas.outline.delete_with_children = { $count ->
    [one] with { $count } sub-node;
    *[other] with { $count } sub-nodes;
    }
ideas.outline.delete_restorable = restorable (below under “Deleted ideas”).
ideas.outline.branches = Branches of the mind map
ideas.outline.no_branches = The mind map has no branches yet: the work has no chapters and no main topic.
ideas.outline.new_main_title = New main topic (title)
ideas.outline.new_main_hint = A main branch of its own directly on the work
ideas.outline.add_main = Create main topic
ideas.outline.done_mark = (processed)
ideas.outline.to_section = To the section
ideas.outline.to_section_in_work = To the section in the work
ideas.outline.section = Section
ideas.outline.open_findings = { $count ->
    [one] { $count } open finding
    *[other] { $count } open findings
    }
ideas.outline.choose = Choose
ideas.outline.cloud_intro = Ideas without a place in the mind map. “Edit” chooses an idea; with “Into the mind map below …” or “As a main topic onto the work” it gets its place, with “Into the cloud” a node comes back here.
ideas.outline.new_idea_title = New idea (title)
ideas.outline.cloud_empty = The cloud is empty.
ideas.outline.fits_branch = fits branch “{ $branch }” (near “{ $partner }”)
ideas.outline.hang_there = Hang in there
ideas.outline.relations = Relations
ideas.outline.none = None.
ideas.outline.deleted = Deleted ideas
ideas.outline.deleted_at = , deleted { $date }
