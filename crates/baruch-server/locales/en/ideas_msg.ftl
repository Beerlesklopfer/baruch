# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of ideas and the idea map (Round 39a, task messages_1). Base catalog in English; locales/de/ideas_msg.ftl has the same keys in
# the same order with the German wording.

# Checks of the RPC methods idea.* (ideas.rs).
ideas_msg.param.number = { $name }: number expected
ideas_msg.param.id_missing = id missing (idea)
ideas_msg.param.text = { $name }: text expected
ideas_msg.param.text_list = { $name }: list of texts expected
ideas_msg.param.list_or_text = { $name }: list or text “a, b; c” expected
ideas_msg.param.status_all = status: open, done or all expected
ideas_msg.param.status = status: open or done expected
ideas_msg.param.not_negative = { $name }: not negative
ideas_msg.param.flag = { $name }: true or false expected
ideas_msg.param.place = place: before, after or into expected
ideas_msg.param.confirm_delete = Please confirm the deletion (confirm).
ideas_msg.process.no_work = An idea without a work cannot be processed like this; assign it to a work first.
ideas_msg.process.no_section = Which section? Give key or assign the idea to a section.
ideas_msg.process.as_invalid = as: outline, post, note or persona expected
ideas_msg.process.as_missing = as missing (outline, post, note or persona)
ideas_msg.process.deleted = Idea { $id } is deleted; restore it first.
ideas_msg.process.no_lector = No persona is active (no lector).
ideas_msg.create.words_missing = words missing (at least one keyword or a title)
ideas_msg.create.cloud = cloud: into the cloud only without parent and key
ideas_msg.search.query_missing = Search term missing.
ideas_msg.search.rag_one_work = Semantic search only within one work (work: work id).
ideas_msg.search.lancedb_not_asked = LanceDB was not asked
ideas_msg.search.rag_unavailable = Semantic search not available: { $reason }.
ideas_msg.search.mode = mode: fts or rag expected
ideas_msg.search.rag_note = Semantic search only within one work; shown is the wording search (FTS).

# Pages „Einfälle“.
ideas_msg.page.template_error = Error: template { $name }: { $error }
ideas_msg.page.error = Error: { $error }
ideas_msg.page.unknown_work = unknown work
ideas_msg.page.unknown_idea = Idea { $id } does not exist.
ideas_msg.page.unknown_action = unknown action
ideas_msg.notice.deleted = The idea is deleted; it can be restored under “Deleted”.
ideas_msg.notice.saved = Saved.
ideas_msg.notice.created = The idea is recorded.
ideas_msg.notice.restored = The idea is restored.
ideas_msg.notice.processed = The idea is processed.
ideas_msg.form.confirm_delete = Please confirm the deletion.

# Idea map (idea_map/mod.rs).
ideas_msg.map.work_missing = work missing (work)
ideas_msg.map.from_missing = from missing (idea from which the link starts)
ideas_msg.map.to_missing = to missing (idea to which it leads)
ideas_msg.map.idea_missing = idea missing (idea)
ideas_msg.map.missing = { $name } missing
ideas_msg.map.status = status: expected “open”, “done” or “all”, not “{ $word }”
ideas_msg.map.origin = origin: expected “human”, “persona” or “mcp”, not “{ $other }”
ideas_msg.map.near_limit = near_limit: 1 to 5, not { $n }
ideas_msg.map.near_threshold = near_threshold: 0 to 1, not { $t }
ideas_msg.map.kind_missing = kind missing (kind of the link)
ideas_msg.map.kind = kind: expected “leads_to”, “belongs_to”, “example_of” or “contradicts”, not “{ $word }”
ideas_msg.map.object = Parameters as an object expected
ideas_msg.map.links_scope = idea.links: give exactly one of work and idea
ideas_msg.near.off = Nearness switched off.
ideas_msg.near.capped = The map shows { $count } ideas; nearness is computed only for the first { $max } (by number).
ideas_msg.near.ready =
    Nearness from stored embeddings ({ $have } of { $count } { $count ->
        [one] idea
       *[other] ideas
    } embedded) and equal keywords; from { $threshold }, at most { $limit } per idea.
ideas_msg.near.failed = The nearness could not be computed ({ $error }).
ideas_msg.import.no_node = The file contains no node; nothing created.
ideas_msg.import.too_many = The mind map has { $count } nodes; at most { $max } can be imported. Nothing created.
ideas_msg.import.too_deep = The mind map is { $deepest } levels deep; at most { $max } can be imported. Nothing created.
ideas_msg.import.nothing_created = Nothing created.

# Notices of the idea map page (idea_map/bild/mod.rs).
ideas_msg.map_notice.reference = Reference set.
ideas_msg.map_notice.linked = Linked.
ideas_msg.map_notice.link_exists = This link already exists; nothing changed.
ideas_msg.map_notice.unlinked = Link removed.
ideas_msg.map_notice.link_restored = Link restored.
ideas_msg.map_notice.positions_reset = Stored positions discarded (no effect in the mind map: the tree arranges itself).
ideas_msg.map_notice.created = Idea created.
ideas_msg.map_notice.renamed = Renamed.
ideas_msg.map_notice.moved = Idea moved.
ideas_msg.map_notice.to_cloud = Put into the cloud (with its sub-nodes).
ideas_msg.map_notice.place_unchanged = The idea already stands there; nothing changed.
ideas_msg.map_notice.deleted = Idea deleted with its sub-nodes (can be restored in the outline view).
ideas_msg.map_notice.restored = Idea restored.
ideas_msg.map_notice.imported = File imported: the nodes are created as ideas.
ideas_msg.map_notice.named = The assistant proposed names (below under “Groups”). A name becomes a keyword only when you adopt it.
ideas_msg.map_notice.not_named = The assistant delivered no usable name; there is no new proposal.
ideas_msg.map_notice.no_groups = No group: there are no close ideas (at least two per group).
ideas_msg.map_notice.name_adopted = Name adopted: it is now a keyword on the ideas of the group.
ideas_msg.map_notice.rejected = Proposal rejected.

# Mind map files and outline view (idea_map/files/*).
ideas_msg.map.idea_elsewhere = Idea { $idea } does not belong to this work.
ideas_msg.map.target = ziel: expected “wolke”, “w” (main topic on the work), “i:{ $lt }idea{ $gt }” or “n:{ $lt }chapter{ $gt }”
ideas_msg.map.unknown_action = Unknown action “{ $action }” on the mind map.
ideas_msg.upload.not_an_idea = The chosen node is not an idea.
ideas_msg.upload.not_multipart = The form did not arrive as a file upload ({ $detail }).
ideas_msg.upload.form_unreadable = The form could not be read ({ $detail }).
ideas_msg.upload.file_unreadable = The file could not be read ({ $detail }).
ideas_msg.upload.choose = Please choose a file (.mm or .xmind).
ideas_msg.upload.unreadable = The file could not be read.
ideas_msg.upload.again = Please import the file again.
ideas_msg.upload.preview_broken = The preview is damaged or missing. { $again }
ideas_msg.upload.preview_too_large = The preview has become too large. { $again }
ideas_msg.upload.form_broken = The form could not be read. { $again }
ideas_msg.import.file_too_many = The file has more than { $max } nodes; Baruch does not import mind maps that large.
ideas_msg.import.file_too_deep = The mind map is deeper than { $max } levels; Baruch does not import mind maps that deep.
ideas_msg.import.empty = The file is empty.
ideas_msg.import.wrong_format = The file is neither a FreeMind/Freeplane file (.mm) nor an XMind file (.xmind).
ideas_msg.import.too_large = The file is larger than { $mib } MiB; Baruch does not import mind maps that large.
ideas_msg.import.links_lost = { $count } relation(s) not adopted: one end is missing in the file, is the main topic, or both ends are the same node.
ideas_msg.import.too_much_text = The mind map carries more than { $mib } MiB of text; Baruch does not import that much.
ideas_msg.import.only_root = Apart from the main topic the file contains no nodes; there is nothing to import.
ideas_msg.import.cut_titles =
    { $count ->
        [one] { $count } title was longer than { $max } characters and was shortened.
       *[other] { $count } titles were longer than { $max } characters and were shortened.
    }
ideas_msg.import.cut_notes =
    { $count ->
        [one] { $count } note was longer than { $max } characters and was shortened.
       *[other] { $count } notes were longer than { $max } characters and were shortened.
    }
ideas_msg.import.untitled =
    { $count ->
        [one] { $count } node without a title is now called “{ $untitled }”.
       *[other] { $count } nodes without a title are now called “{ $untitled }”.
    }
ideas_msg.import.cut_words =
    For { $count } { $count ->
        [one] node
       *[other] nodes
    } keywords were shortened (at most { $max } per node, { $chars } characters each).
ideas_msg.import.links_dropped = { $count } relation(s) not adopted (invalid or too many).
ideas_msg.outline_notice.moved = Moved.
ideas_msg.outline_notice.deleted = Idea deleted (with its sub-nodes).
ideas_msg.outline_notice.imported = Mind map imported.
ideas_msg.outline_notice.cloud = Cloud
ideas_msg.outline_notice.main_topic = { $title } (main topic on the work)
ideas_msg.map.link_id_missing = id missing (link)

# Picture of the mind map (src/idea_map/bild/mindmap.rs: page fragment and the .svg/.pdf export, both in the language
# of the request; Round 39c, task reste_1).
ideas_msg.mindmap.label = Mind map: { $title }
ideas_msg.mindmap.cloud_empty = Cloud – empty. Ideas without a place in the mind map lie here.
ideas_msg.mindmap.cloud_count = Cloud – { $count } in the cloud
ideas_msg.mindmap.fits = fits branch “{ $title }”
