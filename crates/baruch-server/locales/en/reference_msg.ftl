# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of glossary, index, cross references, search and scripture check (Round 39a, task messages_1). Base catalog in English; locales/de/reference_msg.ftl has the same keys in
# the same order with the German wording.

# Glossary (glossary.rs, glossary_page.rs).
reference_msg.glossary.unknown_work = Unknown work.
reference_msg.glossary.ok_add = Term added to the glossary.
reference_msg.glossary.ok_update = Term changed.
reference_msg.glossary.ok_delete = Term deleted (can be restored from the mirror).
reference_msg.glossary.ok_accept = Proposal adopted – the term is now in the glossary.
reference_msg.glossary.ok_reject = Proposal rejected.
reference_msg.glossary.suggest_none = The lector proposed no new terms.
reference_msg.glossary.suggest_one = The lector proposed { $count } term – it is listed under “Proposals of the assistants”.
reference_msg.glossary.suggest_many = The lector proposed { $count } terms – they are listed under “Proposals of the assistants”.
reference_msg.glossary.suggest_skipped = { $count } skipped (already in the glossary or proposed, or unusable).
reference_msg.page.template_error = Template { $name }: { $error }
reference_msg.page.error = Error: { $error }
reference_msg.glossary.id_missing = Term missing (field id).
reference_msg.glossary.confirm_delete = Please confirm the deletion with the tick; nothing changed.
reference_msg.glossary.key_number = Section “{ $key }”: number expected.
reference_msg.glossary.unknown_action = Unknown action (add, update, delete, accept, reject or suggest).
reference_msg.marking.key_missing = Which section? The link needs key (and optionally rev).
reference_msg.marking.no_revision = Revision { $rev } does not exist in this section.
reference_msg.marking.no_section = A section with the key { $key } does not exist (any more).
reference_msg.param.number = { $name }: number expected
reference_msg.param.text = { $name }: text expected
reference_msg.param.missing = { $name } missing
reference_msg.param.flag = { $name }: true or false expected
reference_msg.param.text_list = { $name }: list of texts expected
reference_msg.param.confirm_delete = Please confirm the deletion (confirm).
reference_msg.glossary.ref_unreadable = Bible passage “{ $reference }”: not readable
reference_msg.glossary.ref_problem = Bible passage “{ $reference }”: { $problem }
reference_msg.glossary.status = status: accepted, proposed or all, not “{ $other }”
reference_msg.index.status = status: open or all, not “{ $other }”

# Index (index_page.rs, index_marks.rs).
reference_msg.index.ok_mark = Index mark set – as a new revision of the section.
reference_msg.index.ok_unmark = Index mark removed – as a new revision of the section.
reference_msg.index.ok_accept = Index proposal adopted – the mark stands in a new revision.
reference_msg.index.ok_reject = Index proposal rejected.
reference_msg.index.ok_register_add = Index created.
reference_msg.index.ok_register_rename = Index renamed.
reference_msg.index.ok_register_delete = Index deleted.
reference_msg.index.section = Section { $key }
reference_msg.index.first = first occurrence
reference_msg.index.all = all occurrences
reference_msg.index.nth = occurrence { $n }
reference_msg.index.field_key = Section
reference_msg.index.field_rev = Revision
reference_msg.index.field_start = Start
reference_msg.index.field_end = End
reference_msg.index.not_a_number = { $label }: not a valid number (field { $name }).
reference_msg.index.word_missing = Word: please give the word after which the mark goes.
reference_msg.index.occurrence_form = Occurrence: “{ $raw }” is neither “erste”, “alle” nor a number from 1.
reference_msg.index.proposal_missing = Index proposal missing (field id).
reference_msg.marks.whole_number = { $name }: whole number expected
reference_msg.marks.key_missing = key: section missing
reference_msg.marks.rev_missing = rev: revision missing
reference_msg.marks.start_missing = start: start of the mark missing
reference_msg.marks.end_missing = end: end of the mark missing
reference_msg.marks.rev_number = rev: number of a revision expected
reference_msg.marks.occurrence = occurrence: expected “first”, “all” or a number from 1
reference_msg.marks.word_missing = word: word missing
reference_msg.marks.start_byte = start: byte position from 0 expected
reference_msg.marks.end_byte = end: byte position from 0 expected
reference_msg.marks.empty_range = start/end: empty range
reference_msg.marks.no_section = Section { $key } does not exist in this work.
reference_msg.marks.no_register = Index “{ $register }” does not exist in this work (present: { $present }).

# Cross references (xref.rs).
reference_msg.xref.picks = Please choose at least one possible reference (picks).
reference_msg.xref.no_candidate = Possible reference { $i } does not exist (any more).
reference_msg.xref.not_a_label = “{ $label }” is not a jump mark (kN).
reference_msg.xref.label_unknown = The jump mark { $label } belongs to no outline item.
reference_msg.xref.confirm = Please confirm the conversion (confirm: true).
reference_msg.xref.rev_number = rev: number expected
reference_msg.xref.title = Cross references
reference_msg.xref.unknown_action = Unknown action (method).
reference_msg.xref.tick = Please tick “Yes, convert”.
reference_msg.xref.key_rev_missing = key and rev missing.

# Scripture check page (scripture_check.rs).
reference_msg.check.title = Check quotations
reference_msg.check.key_missing = Please give a section (key = fixed key).
reference_msg.check.rev_number = rev: number of the revision expected.

# Versification mapping (scripture_map.rs).
reference_msg.map.name_text = { $field }: name of a versification as text
reference_msg.map.unknown = { $field }: versification “{ $name }” unknown; known are { $known }
reference_msg.map.ref_missing = ref missing (reference, e.g. “Joel 2:28–32”)
reference_msg.map.ref_too_long = ref: at most { $max } characters
reference_msg.map.same = same in { $name }: { $target }
reference_msg.map.mapped = in { $name }: { $target }
reference_msg.map.rule = (rule: { $rules })
reference_msg.map.truncated = – shortened to { $max } verses
reference_msg.map.no_book = the book { $abbr } does not exist in { $name }
reference_msg.map.chapters =
    { $abbr } has only { $count } { $count ->
        [one] chapter
       *[other] chapters
    } in { $name }
reference_msg.map.verses =
    { $abbr } { $chapter } has only { $count } { $count ->
        [one] verse
       *[other] verses
    } in { $name }
reference_msg.map.end_before_start = { $abbr }: end before the start

# Text search and replace (search/*).
reference_msg.search.kind_text = Text
reference_msg.search.kind_heading = Headings
reference_msg.search.kind_footnote = Footnotes
reference_msg.search.kind_quote = Quotations (also Bible quotations)
reference_msg.search.kind_code = Typst code
reference_msg.search.query_missing = Please enter a search text.
reference_msg.search.query_too_long = The search text may have at most { $max } characters (here { $chars }).
reference_msg.search.shown_rev = The shown revision can only be searched in the section or in the selection.
reference_msg.search.key_missing = The section (key) is missing for the range “{ $range }”.
reference_msg.search.sel_missing = Please choose at least one outline item (sel).
reference_msg.search.range_selection = Selection in { $name }
reference_msg.search.range_section = Section { $name }
reference_msg.search.range_chapter = Chapter { $name }
reference_msg.search.range_part = Part { $name }
reference_msg.search.range_work_no_parts = whole work (no parts)
reference_msg.search.range_from = from { $name } to the end
reference_msg.search.range_picked_one = { $count } chosen section
reference_msg.search.range_picked = { $count } chosen sections
reference_msg.search.range_work = whole work
reference_msg.search.range_all = all works
reference_msg.search.params = Search: { $error }
reference_msg.search.title = Search
reference_msg.search.foreign = Refused: the replace preview does not start at the request of a foreign page.
reference_msg.search.selection = Selection in the editor (characters { $start }–{ $end })
reference_msg.search.choice_section = Section
reference_msg.search.choice_chapter = Chapter
reference_msg.search.choice_part = Part
reference_msg.search.choice_from = from the section to the end
reference_msg.search.choice_picked = Selection from the outline
reference_msg.search.choice_work = whole work
reference_msg.search.choice_all = all works (search only)
reference_msg.search.tick = Please tick “Yes, replace”.
reference_msg.search.none_picked = No section chosen.
reference_msg.search.regex_too_big = The regular expression is too big.
reference_msg.search.regex_invalid = Invalid regular expression: { $error }
reference_msg.replace.too_long = The replacement may have at most { $max } characters (here { $chars }).
reference_msg.replace.one_work = Replacing works only within one work (not “all works”).
reference_msg.replace.too_many = Too many replacements at once (the inserted texts would together be larger than { $mib } MiB); nothing was replaced. Please narrow the search or the range.
reference_msg.replace.timeout = The search took longer than 2 s; nothing was replaced. Please make the range smaller.
reference_msg.replace.max_sections = At most { $max } sections per replacement (here { $count }).
reference_msg.replace.no_more_hits = No more hits in this section.
reference_msg.replace.confirm = Please confirm the replacement (confirm: true).
reference_msg.replace.replacement_missing = replacement: replacement text missing (empty is allowed).
reference_msg.replace.undo_confirm = Please confirm the undo (confirm: true).
reference_msg.replace.batch_missing = batch: number of the replacement missing.

# Round 39c: notes and refusals of the Bible checks (src/bibles.rs, src/scripture.rs)
reference_msg.bibles.fragment_example = “{ $example }”
reference_msg.bibles.fragment_note = Module { $module } has strikingly many word fragments ({ $suspects } of { $pairs } word pairs, e.g. { $examples }) – probably hyphenation remnants; Baruch therefore also compares the wording without whitespace.
reference_msg.scripture.text_missing = text missing (reference or running text)
reference_msg.scripture.text_too_long = text: at most { $max } characters
reference_msg.xref.tooltip = Cross reference to { $target } (Ctrl+click: jump)
