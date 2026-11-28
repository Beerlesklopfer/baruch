# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of the store (baruch-store) in the language of the user interface (Round 39a, task messages_2).
# The German texts are the store's own wording verbatim: src/store_text.rs reads them as patterns (a `{ $name }`
# matches any text) and formats the matching key in the language of the request. Arguments named `detail` are
# translated again (nested messages). Owned by messages_2.

# Variants of StoreError with a fixed wording.
store_msg.error.db = Database: { $detail }
store_msg.error.migrate = Migration: { $detail }
store_msg.error.io = Data folder { $path }: { $detail }
store_msg.error.unknown_work = unknown work { $work }
store_msg.error.unknown_node = unknown section { $node }
store_msg.error.unknown_revision = Revision { $rev } does not exist; available: { $list }
store_msg.error.unknown_revision_none = Revision { $rev } does not exist; available: none
store_msg.error.empty_post = Empty post.
store_msg.error.already_imported = This file has already been imported: work “{ $title }” ({ $slug }).
store_msg.error.import = Import: { $detail }
store_msg.error.duplicate_title = There already is a work “{ $title }” ({ $slug }).
store_msg.error.empty_title = A work needs a title.
store_msg.error.gone_node = This outline item no longer exists (deleted or merged); please reload.
store_msg.error.numbering = Numbering: { $detail }
store_msg.error.unknown_asset = There is no image “{ $name }” in this work.
store_msg.error.unknown_persona = There is no persona “{ $handle }”.
store_msg.error.unknown_idea = Idea { $id } does not exist.
store_msg.error.unknown_post = Post { $post } does not exist in work { $work }.
store_msg.error.unknown_request = Request { $id } does not exist in work { $work }.
store_msg.error.exchange = Exchange: { $detail }
store_msg.error.unknown_finding = Finding { $id } does not exist in work { $work }.
store_msg.error.typeset = Typesetting settings: { $detail }
store_msg.error.glossary = Glossary/index: { $detail }
store_msg.error.unknown_glossary_term = Glossary term { $id } does not exist in work { $work }.
store_msg.error.unknown_index_proposal = Index proposal { $id } does not exist in work { $work }.

# Accounts (pub const of Store).
store_msg.accounts.taken = This name already exists.
store_msg.accounts.not_first = There already is an account.
store_msg.accounts.needs_author = The first account belongs to the author (set-up).
store_msg.accounts.last_author = The last active author account cannot be locked.
store_msg.accounts.share_role = Only the role “lektor” can be granted.
store_msg.accounts.share_unknown = Unknown account: nothing granted.
store_msg.accounts.share_lektor_only = Only lektor accounts get a grant; the author is author in every work.
store_msg.accounts.ui_language = Interface language: two lower-case letters (e.g. de or en).

# Revisions and history.
store_msg.revision.no_current = The section has no current revision.
store_msg.revision.undo_current_only = Undoing through the history only works on the current revision; switch to this revision first.
store_msg.revision.undo_human_only = Undoing through the history only works on a revision of the human (not on proposals of personas).
store_msg.revision.no_such_step = This step does not exist in the history of this revision.
store_msg.revision.already_at_step = The text is already at this state; nothing to take back.
store_msg.revision.step_other_heading = The state before this step has a different heading; please save it as a new revision.
store_msg.revision.policy_percent = Largest change: 1 to 100 percent.
store_msg.revision.policy_minutes = New revision after: 0 to 1440 minutes (0 = no time limit).
store_msg.revision.already_current = This revision is already current.
store_msg.revision.unchanged = The revision is unchanged.
store_msg.revision.current_not_deletable = The current revision cannot be deleted; switch to another one first.
store_msg.revision.needs_text = A revision needs text.
store_msg.revision.only_proposals_accept = Only proposals can be accepted.
store_msg.revision.only_proposals_reject = Only proposals can be rejected.
store_msg.revision.proposals_need_current = Proposals need a current revision.
store_msg.revision.fold_above_current = Nothing above the current revision can be hidden; set the mark at most at the current revision.
store_msg.revision.stale_amend = Outdated: revision { $rev } is now at state { $seq } (state { $expected } was loaded); please reload and enter the change there.
store_msg.revision.stale = Outdated: { $detail }

# Images and image requests.
store_msg.asset.unchanged = The image is unchanged.
store_msg.asset.request_current_rev = The current revision cannot be deleted; without rev the whole image is requested.
store_msg.asset.layout_unchanged = The cover layout is already saved like this; nothing to change.
store_msg.asset.human_direct = The human changes images directly; only assistants make requests.
store_msg.asset.request_open = This request already exists (open); the human decides on it.
store_msg.asset.request_decided = This request has already been decided.
store_msg.asset.name_taken = An image with this name already exists in this work.
store_msg.asset.no_collision = There is no collision (any more) under this path.
store_msg.asset.paths_unchanged = The paths are unchanged.
store_msg.asset.changed_meanwhile = The image “{ $name }” has different content by now (expected_sha256 does not match); please read it again.
store_msg.asset.new_too_big = The new content is larger than 8 MiB.
store_msg.asset.new_unsupported = The new content is not a supported image (PNG, JPEG, GIF, WebP, SVG).
store_msg.asset.svg_needs_text = encoding “svg” requires SVG text.
store_msg.asset.mime_mismatch = mime “{ $mime }” does not match the content ({ $found }).
store_msg.asset.format_change = The new content is { $found }, “{ $name }” is { $current }; another format only with rename: true (the image gets the new extension on confirmation and keeps its paths) or as a new image.
store_msg.asset.unknown_rev_named = Revision { $rev } of “{ $name }” does not exist; available: { $list }
store_msg.asset.unknown_rev = Image { $name }: revision { $rev } does not exist; available: { $list }
store_msg.asset.blocks_expected = blocks: at least one block of the layout expected.
store_msg.asset.layout_changed = The cover layout has changed in the meantime (expected_layout does not match); please read it again (cover_layout).
store_msg.asset.bad_base64 = Not valid Base64 ({ $detail }).
store_msg.asset.path_empty = Invalid path “{ $path }”: empty.
store_msg.asset.path_too_long = Invalid path “{ $path }”: too long.
store_msg.asset.path_separator = Invalid path “{ $path }”: only folder and file name separated by '/'.
store_msg.asset.path_dotdot = Invalid path “{ $path }”: '..' is not allowed.
store_msg.asset.path_no_file = Invalid path “{ $path }”: no file name.
store_msg.asset.too_big = “{ $name }” is larger than 8 MiB.
store_msg.asset.unsupported = “{ $name }” is not a supported image (PNG, JPEG, GIF, WebP, SVG).
store_msg.asset.path_taken = The path “{ $path }” already belongs to image { $other }.
store_msg.asset.bad_name = Invalid image name “{ $name }”: only a–z, 0–9, '.', '_' and '-', at most 80 characters, with extension (.png, .jpg, .jpeg, .gif, .webp, .svg).
store_msg.asset.extension_mismatch = The extension of “{ $name }” does not match the type { $mime }.
store_msg.asset.replace_mismatch = “{ $file }” is { $mime }, which does not match the extension of { $name }; with “Adjust extension” (rename) the image gets the new extension and keeps its paths, otherwise add it as a new image.
store_msg.asset.collision_paths = The image “{ $asset }” has none of the paths of “{ $wanted }”; available: { $paths }.
store_msg.asset.one_path = An image in the text needs exactly one path.

# Book details and cover print.
store_msg.book.year = Year: leave empty or four digits (e.g. 2026).
store_msg.book.paper = Paper thickness: more than 0 and at most 1 mm per page.
store_msg.book.bleed = Bleed: 0 to 10 mm.
store_msg.book.field_too_long = { $field }: at most { $max } characters.
store_msg.book.field_one_line = { $field }: please on one line.

# Connections, sources, model connections, collections.
store_msg.connections.unknown = Unknown connection (eliezer, multigpt, pgvector or lancedb).
store_msg.connections.unknown_way = Unknown RAG way (lancedb, pgvector or multigpt).
store_msg.connections.source_title = Title of a source: 1 to 200 characters on one line.
store_msg.connections.source_address = Address of a source: http:// or https://, at most 2000 characters.
store_msg.connections.source_gone = This source does not exist (any more).
store_msg.connections.config_object = Settings of a connection are a JSON object.
store_msg.connections.model_gone = This model connection does not exist (any more).
store_msg.connections.model_name = Name of a model connection: 1 to 80 characters on one line.
store_msg.connections.model_kind = Kind of a model connection: eliezer, openai or ollama.
store_msg.connections.model_address = Address of a model connection: http:// or https://, without whitespace.
store_msg.connections.model_taken = A model connection with this name already exists.
store_msg.connections.eliezer_kind = The connection Eliezer stays of the kind eliezer.
store_msg.connections.eliezer_delete = The connection Eliezer cannot be deleted.
store_msg.connections.lancedb_collections = LanceDB has no collections (only pgvector and MultiGPT).
store_msg.connections.collection_name = Collection: 1 to 200 characters on one line.
store_msg.connections.collections_max = At most 100 collections per work and way.
store_msg.connections.fetch_max = At most 100 collections to fetch.
store_msg.connections.sword_name = Module name: at most 200 characters on one line.
store_msg.connections.sword_max = At most 20 commentaries or lexicons per work.
store_msg.connections.persona_unknown = This persona does not exist.

# Exchange files and instance.
store_msg.exchange.work_exists = Work already present: this file belongs to a work that exists here; please synchronise instead of reading it in anew.
store_msg.exchange.work_missing = Synchronising is not possible: the work of this file does not exist here; please read it in as a new work.
store_msg.exchange.request_part = Request with identifier: no part under the new items.
store_msg.exchange.request_current = Request with identifier: exactly one current revision expected.
store_msg.exchange.glossary_max = The glossary would have { $count } terms; at most { $max } per work (proposals included). Nothing read in.
store_msg.exchange.register_title = Index { $slug }: title has { $chars } characters (at most { $max }). Nothing read in.
store_msg.exchange.uuid_elsewhere = { $what }: the identifier { $uuid } already exists here in another work; nothing taken over
store_msg.exchange.unknown_scheme = unknown outline scheme “{ $scheme }”
store_msg.exchange.db_mismatch = Database does not fit the exchange format: { $detail }
store_msg.exchange.instance_name_long = The name of the instance may have at most { $max } characters (now { $chars }); nothing saved.
store_msg.exchange.instance_name_control = The name of the instance must not contain control characters (line break, tab …); nothing saved.
store_msg.exchange.instance_uuid = Instance identifier missing (migration 0041)

# Glossary.
store_msg.glossary.no_text = This section has no text yet.

# MCP tokens.
store_msg.tokens.name_needed = A token needs a name (e.g. that of the computer).
store_msg.tokens.name_long = The name of a token has at most 64 characters.
store_msg.tokens.name_control = The name of a token has no line breaks or control characters.
store_msg.tokens.no_random = No randomness from the operating system; token not created.
store_msg.tokens.name_valid = There already is a valid token with this name; choose another name or revoke the old one.
store_msg.tokens.unknown = This token does not exist.
store_msg.tokens.handle_reserved = Names whose author begins with “lokal-” are reserved for clients without a token; choose another name.
store_msg.tokens.handle_taken = This name gives the same author (@mcp:…) as an existing token – case, umlauts, special characters and everything after 32 characters do not count. Choose another name.
store_msg.tokens.handle_revoked = This name gives the same author (@mcp:…) as a revoked token; its posts keep carrying it. Choose another name.

# Outline.
store_msg.outline.into_own_subtree = An outline item cannot be moved into its own subtree.
store_msg.outline.already_there = The outline item is already at this position.
store_msg.outline.parts_level = Parts cannot change level, and nothing becomes a part.
store_msg.outline.too_deep = Deeper than level 3 (subsection) is not possible.
store_msg.outline.same_level = The outline item already has this level.
store_msg.outline.children_too_deep = With its sub-items this would go deeper than level 3 (subsection); raise the sub-items first.
store_msg.outline.same_title = The outline item already has this name.
store_msg.outline.no_items = No outline items given (title or items).
store_msg.outline.max_items = At most 100 outline items in one call.
store_msg.outline.key_missing = Reference item (key) missing: without it only the first item of an empty work can be created.
store_msg.outline.first_level = The first new item stands on the level of the place (before/after: level of the reference item, under: one deeper).
store_msg.outline.none_higher = No new item stands higher than the first; otherwise existing items would become its sub-items.
store_msg.outline.parts_counted = Parts are always counted; “without number” exists only for chapters and sections.
store_msg.outline.has_number = The outline item already has a number.
store_msg.outline.has_no_number = The outline item is already without a number.
store_msg.outline.has_children = The outline item has sub-items; move or delete them first.
store_msg.outline.last_item = The last outline item cannot be deleted; a work needs at least one chapter.
store_msg.outline.confirm_merge = The text has no heading any more; confirm to merge it with the previous outline item.
store_msg.outline.confirm_split = The text contains new headings; confirm to split it.
store_msg.outline.numbering_same = The numbering is already set like this.
store_msg.outline.part_not_into = A part always stands on the top level; “under” (into) does not work with level 0.
store_msg.outline.part_place = A part is only created before or after a part or chapter, not next to a section.
store_msg.outline.lead_text = There is text before the first heading; the heading belongs at the beginning of the section.
store_msg.outline.merge_first = The first outline item cannot be merged.
store_msg.outline.merge_part = A part stands before it; merging only works with a chapter or section.
store_msg.outline.to_part = The first heading is a part marker; a chapter or section does not become a part (create a new part in the outline).
store_msg.outline.part_hint = A part text with a marker begins with exactly this one marker (only whitespace before it) and contains no further heading.
store_msg.outline.switch_hint = Splitting, merging and text before the heading go through “Save” in the editor.
store_msg.outline.with_switch_hint = { $detail } Splitting, merging and text before the heading go through “Save” in the editor.
store_msg.outline.switch_no_heading = The revision has no heading; the outline would not know where it belongs. Splitting, merging and text before the heading go through “Save” in the editor.
store_msg.outline.switch_more_headings = The revision contains further headings ({ $titles }); switching does not split a section. Splitting, merging and text before the heading go through “Save” in the editor.
store_msg.outline.proposal_refused = Proposal refused: { $detail }
store_msg.outline.not_executable = No longer executable: { $detail }
store_msg.outline.title_empty = An outline item needs a title.
store_msg.outline.title_newline = The title must not contain a line break.
store_msg.outline.title_long = The title is too long (at most { $max } characters, here { $count }).
store_msg.outline.item_n = Item { $n }: { $detail }
store_msg.outline.unknown_scheme = unknown outline scheme “{ $scheme }”; known: { $known }
store_msg.outline.idea_other_work = Idea { $idea } does not belong to this work.

# Personas.
store_msg.personas.prompt_empty = The role prompt must not be empty (default: reset).
store_msg.personas.prompt_long = The role prompt may have at most 8000 characters.
store_msg.personas.color = Give the colour as #rrggbb.

# Posts, threads and their editing.
store_msg.posts.already_here = The post is already in this section.
store_msg.posts.unknown_thread = This thread does not exist in this work.
store_msg.posts.edit_empty = The post must not be empty; nothing changed.
store_msg.posts.edit_long = A post may have at most { $max } characters; nothing changed.
store_msg.posts.edit_unchanged = The text is unchanged; nothing changed.
store_msg.posts.basis_start = The checked revision starts at 1.
store_msg.posts.basis_long = The note on the checked revision is too long (at most { $max } characters).
store_msg.posts.relay_from = “Relay from” needs 1 to { $max } characters.

# Findings.
store_msg.findings.too_long_body = The text of the finding is too long (at most { $max } characters).
store_msg.findings.too_long_quote = The quotation is too long (at most { $max } characters).
store_msg.findings.too_long_check = The check note is too long (at most { $max } characters).
store_msg.findings.too_long_solution_quote = The quotation of a solution is too long (at most { $max } characters).
store_msg.findings.too_long_solution = A solution is too long (at most { $max } characters).
store_msg.findings.too_long_note = The note on the proposal is too long (at most { $max } characters).
store_msg.findings.too_long_reason = The reason is too long (at most { $max } characters).
store_msg.findings.needs_text = A finding needs a text.
store_msg.findings.persona_length = The persona of a finding needs 1 to { $max } characters.
store_msg.findings.one_origin = A finding comes from a post or a thread post, not from both.
store_msg.findings.unknown_section = Section { $key } does not exist in work { $work }.
store_msg.findings.unknown_thread_post = Thread post { $post } does not exist in work { $work }.
store_msg.findings.solution_empty = A solution needs a quotation with a new wording or a step.
store_msg.findings.setter_name = Whoever sets the status needs a name.
store_msg.findings.status_same = Finding { $id } already has this status ({ $status }); nothing changed.
store_msg.findings.human_direct = The human sets the status directly; only assistants make proposals.
store_msg.findings.proposal_origin = A proposal needs an origin (persona or MCP client).
store_msg.findings.human_only = Only the human can discard a finding or open it again; assistants only propose “adopted”, “deferred” or “done without change”.
store_msg.findings.status_same_proposal = Finding { $id } already has this status ({ $status }); no proposal needed.
store_msg.findings.proposal_open = This proposal is already open for finding { $id } (entry { $twin }); not created twice.
store_msg.findings.decider_name = Whoever decides needs a name.
store_msg.findings.not_proposal = Entry { $entry } is not a proposal but a status change of the human.
store_msg.findings.decided = Proposal { $entry } has already been decided ({ $decision }).
store_msg.findings.reject_reason = Discarding needs a reason.
store_msg.findings.no_section = Finding { $id } belongs to no section; “adopted” needs a revision of the section.
store_msg.findings.unknown_rev = Revision { $rev } does not exist in the section of finding { $id }.
store_msg.findings.defer_own = A finding cannot be deferred to its own section.
store_msg.findings.defer_section = Section { $key } does not exist in work { $work } (target of the deferral).
store_msg.findings.defer_idea = Idea { $id } does not exist in work { $work } (target of the deferral).
store_msg.findings.unknown_proposal = Proposal { $entry } does not exist in work { $work }.

# Work-wide replacing.
store_msg.replace.query_length = The search text needs 1 to { $max } characters (here { $chars }).
store_msg.replace.replacement_length = The replacement may have at most { $max } characters (here { $chars }).
store_msg.replace.sections_max = At most { $max } sections per replacement (here { $count }).
store_msg.replace.twice = Section { $key } appears twice in the replacement.
store_msg.replace.undone = This replacement has already been undone ({ $when }).
store_msg.replace.unknown = Replacement { $batch } does not exist in this work.

# Speaking time.
store_msg.timing.words = Words per minute: 60 to 250 are allowed.
store_msg.timing.target = Target time: 1 to 240 minutes are allowed (or no target time).

# Work template and works.
store_msg.works.template_heading = The work template must not contain a heading (= at the start of a line); headings belong to the outline.
store_msg.works.template_unchanged = The work template would be unchanged; nothing changed.
store_msg.works.bible_module = Bible module: name with 1 to 100 characters on one line expected; the work was not created.
store_msg.works.title_mismatch = The title entered does not match; nothing deleted.
store_msg.works.not_archived = This work is not in the archive (unknown folder name); nothing restored.
store_msg.works.restore_title = A living work already carries the same title; rename that one first, then restore. Nothing restored.
store_msg.works.restore_source = The same source has already been imported as a living work; nothing restored.
store_msg.works.no_sections = The work has no sections.
store_msg.exchange.part_exists = The part exists here by now with this identifier (or one of its revisions); nothing created.
store_msg.outline.move_keeps_level = Moving keeps the level (a chapter stays a chapter); change the level first (← / →).

# Round 39c: more store texts (persona checks, requests, ideas, connections, typeset settings)
store_msg.personas.name_empty = The name must not be empty.
store_msg.personas.name_long = The name may have at most 80 characters.
store_msg.personas.name_line = The name must fit on one line.
store_msg.personas.role_empty = The role must not be empty.
store_msg.personas.role_long = The role may have at most 200 characters.
store_msg.personas.model_empty = The model must not be empty.
store_msg.personas.model_long = The model may have at most 200 characters.
store_msg.personas.context_range = The context length must be between 1 and 10000000 tokens.
store_msg.requests.create_via_add = Creating is not requested; use outline_add.
store_msg.requests.preview_decided = This request has already been decided; a preview exists only for open requests.
store_msg.ideas.keyword_newline = A keyword must not contain a line break.
store_msg.ideas.keyword_long = The keyword “{ $word }…” is too long (at most { $max } characters, here { $chars }).
store_msg.ideas.thought_long = The thought is too long (at most { $max } characters, here { $chars }).
store_msg.ideas.quote_long = The marked text is too long (at most { $max } characters, here { $chars }).
store_msg.ideas.deleted_restore = Idea { $id } is deleted; restore it first.
store_msg.ideas.not_before_self = An idea cannot be placed before itself.
store_msg.ideas.not_sibling = Idea { $id } is not a sibling: placing works only before an idea with the same parent node.
store_msg.ideas.not_under_self = An idea cannot be hung under itself or one of its own child nodes.
store_msg.ideas.parent_deleted = Idea { $id } is deleted; nothing can be hung under it.
store_msg.ideas.child_needs_work = A child node works only for an idea with a work.
store_msg.ideas.too_deep = The mind map would be too deep here (depth { $depth }); at most { $max } levels are allowed.
store_msg.ideas.needs_title = An idea needs a title or at least one keyword.
store_msg.ideas.topic_needs_work = A main topic of the mind map works only for an idea with a work.
store_msg.ideas.already_deleted = Idea { $id } is already deleted.
store_msg.ideas.not_deleted = Idea { $id } is not deleted.
store_msg.ideas.cloud_only_free = Only an idea without a parent node and without a reference can go into the cloud.
store_msg.ideas.map_needs_work = The mind map exists only for ideas with a work.
store_msg.ideas.tree_empty = The tree contains no node.
store_msg.ideas.tree_nodes = The tree has too many nodes ({ $count }); at most { $max } are allowed.
store_msg.ideas.tree_depth = The tree is too deep (depth { $depth }); at most { $max } levels are allowed.
store_msg.ideas.tree_path = A connection points to a node that is not in the tree (path { $path }).
store_msg.ideas.section_needs_work = A section as reference works only for an idea with a work.
store_msg.ideas.section_other_work = The section does not belong to this work.
store_msg.links.note_control = The note of a connection must not contain control characters (except a line break).
store_msg.links.note_long = The note of a connection is too long (at most { $max } characters, here { $chars }).
store_msg.links.unknown = Connection { $id } does not exist.
store_msg.links.self = An idea cannot be connected to itself.
store_msg.links.other_work = Idea { $from } and idea { $to } do not belong to the same work; only ideas of one work can be connected.
store_msg.links.origin_name = The origin of a connection needs a name.
store_msg.links.already_deleted = Connection { $id } is already deleted.
store_msg.links.not_deleted = Connection { $id } is not deleted.
store_msg.links.coordinate = Position: { $name } must be a finite number between -{ $limit } and { $limit }.
store_msg.typeset.page_width = Page width: { $min } to { $max } mm
store_msg.typeset.page_height = Page height: { $min } to { $max } mm
store_msg.typeset.margin_inside = Inside margin: { $min } to { $max } mm
store_msg.typeset.margin_outside = Outside margin: { $min } to { $max } mm
store_msg.typeset.margin_top = Top margin: { $min } to { $max } mm
store_msg.typeset.margin_bottom = Bottom margin: { $min } to { $max } mm
store_msg.typeset.binding = Gutter: { $min } to { $max } mm
store_msg.typeset.font_family = Body font: give a family name
store_msg.typeset.font_family_bad = Body font: a family name of at most { $max } characters, without a line break, " and \
store_msg.typeset.font_size = Font size: { $min } to { $max } pt
store_msg.typeset.leading = Line spacing: { $min } to { $max } em
store_msg.typeset.paragraph_indent = Paragraph indent: { $min } to { $max } em
store_msg.typeset.paragraph_spacing = Paragraph spacing: { $min } to { $max } em
store_msg.typeset.lang = Language: a code like “de”, “en” or “de-CH”
store_msg.typeset.index_none = Index pages: choose at least one index
store_msg.typeset.index_max = Index pages: at most { $max } indexes
store_msg.typeset.index_bad = Index pages: “{ $bad }” is not a valid index id (lower-case letters, digits, hyphen, at most 32 characters)
store_msg.typeset.index_double = Index pages: index “{ $double }” is chosen twice
store_msg.typeset.index_columns = Index pages: { $min } to { $max } columns
store_msg.typeset.draft_text = Draft: a text of 1 to { $max } characters, without a line break and control characters
store_msg.typeset.draft_note = Draft: an addition of at most { $max } characters, without a line break and control characters
store_msg.typeset.draft_opacity = Draft: opacity { $min } to { $max } %
store_msg.typeset.text_width = Margins: the text width must be at least { $min } mm (now { $now } mm)
store_msg.typeset.text_height = Margins: the text height must be at least { $min } mm (now { $now } mm)
