# SPDX-License-Identifier: AGPL-3.0-or-later
# Area "reference" (Round 39a, templates_1): texts of the templates named in the sections below. Base
# catalog in English; locales/de/reference.ftl has the same keys in the same order.

# templates/xrefs.html
reference.xrefs.head_title = Cross references – { $title } – Baruch
reference.xrefs.page_title = { $title }: Cross references
reference.xrefs.back = Back
reference.xrefs.back_to_work = Back to the work
reference.xrefs.heading = Cross references
reference.xrefs.in_section = in { $section }
reference.xrefs.done_one = reference
reference.xrefs.done_other = references
reference.xrefs.done_note = inserted (new revision by you).
reference.xrefs.whole_work = Show the whole work
reference.xrefs.broken = Broken references
reference.xrefs.broken_item = { $section }: “{ $text }” points to no outline item with a current revision.
reference.xrefs.to_place = go to the place
reference.xrefs.no_broken = No broken references.
reference.xrefs.candidates = Possible cross references
reference.xrefs.candidates_help = Mentions of outline items in the text (“the introduction”, “in the third chapter”, “Chapter III”, titles in quotation marks), only when the target is unambiguous. Only a proposal: nothing is converted until you confirm – as a new revision by you.
reference.xrefs.rev = (revision { $rev })
reference.xrefs.line = Line { $line }: “{ $text }” →
reference.xrefs.place = Place
reference.xrefs.confirm = Yes, convert
reference.xrefs.convert = Convert into cross references
reference.xrefs.no_candidates = No possible cross references found.

# templates/scripture_check.html
reference.scripture.quoted = “{ $quote }”
reference.scripture.head_title = Check quotations – { $title } – Baruch
reference.scripture.page_title = { $title }: Check quotations
reference.scripture.back = Back
reference.scripture.back_to_section = Back to the section
reference.scripture.heading = Check quotations: { $title }
reference.scripture.rev = (revision { $rev })
reference.scripture.checked_by = Checked by Baruch (not by a model) with the SWORD module
reference.scripture.versification = , versification
reference.scripture.no_bible = The work has no Bible: the wording stays unchecked. Choose a Bible under
reference.scripture.settings_sources = Settings → Sources
reference.scripture.bad_bible = The Bible of the work cannot be read: { $problem } ·
reference.scripture.licence = Licence (according to conf): { $notice }
reference.scripture.matches =
    { $count } { $count ->
        [one] match
       *[other] matches
    }
reference.scripture.differs =
    { $count } { $count ->
        [one] differs
       *[other] differ
    }
reference.scripture.other = { $count } other
reference.scripture.col_place = Reference
reference.scripture.col_quote = Quotation in the text
reference.scripture.col_result = Result
reference.scripture.col_diff = Differences
reference.scripture.col_module = Read in the module
reference.scripture.bible_text = Bible: “{ $bible_text }”
reference.scripture.as_given = as given ({ $versification })
reference.scripture.none = No Bible reference found in this revision.
reference.scripture.quote_rule_html = A quotation belongs to a reference when it stands directly before or after it (quotation marks, <code>#quote[…]</code> or block quotation). A model never confirms a wording itself. With JavaScript, “Check quotations” in the editor marks the same results in the typeset view (green: matches, red: differs).

# templates/glossary.html
reference.glossary.head_title = Glossary – { $title } – Baruch
reference.glossary.page_title = { $title }: Glossary
reference.glossary.see_also = See also
reference.glossary.index = Index
reference.glossary.back = Back
reference.glossary.back_to_work = Back to the work
reference.glossary.term = Term
reference.glossary.short = Short explanation
reference.glossary.long = Detailed explanation
reference.glossary.variants = Variants
reference.glossary.variants_hint = (one per line, e.g. plural)
reference.glossary.bible_refs = Bible references
reference.glossary.bible_refs_hint = (one per line, e.g. John 3:16)
reference.glossary.source = Source
reference.glossary.heading = Glossary
reference.glossary.intro = Terms explained in the book: short and detailed explanation, variants (other word forms) and Bible references. Which page the glossary gets in the book is set under Settings → Work → Typesetting. Assistants only propose terms; only you accept, change and delete them.
reference.glossary.load_error = The glossary cannot be read: { $load_error }
reference.glossary.lookup_label = Look up in the glossary
reference.glossary.lookup = Look up
reference.glossary.lookup_placeholder = Word or variant
reference.glossary.lookup_found = In the glossary: “{ $word }”
reference.glossary.lookup_missing = “{ $word }” is not in the glossary.
reference.glossary.add = Add to glossary
reference.glossary.filter = Filter the list
reference.glossary.filter_placeholder = Term, variant, explanation
reference.glossary.terms = Terms ({ $count })
reference.glossary.empty = No terms yet. In the editor: mark a word, right-click → “Add to glossary”, or create one below.
reference.glossary.also = also: { $variants }
reference.glossary.bible_list = Bible references: { $bible_refs }
reference.glossary.source_line = Source: { $source }
reference.glossary.proposed_by = Proposed by { $origin } ·
reference.glossary.changed = changed { $updated }
reference.glossary.changed_by = by { $updated_by }
reference.glossary.created = created { $created }
reference.glossary.edit = Edit or delete
reference.glossary.delete_confirm = Yes, delete “{ $term }” from the glossary
reference.glossary.proposals = Proposals of the assistants ({ $count })
reference.glossary.ask = Have terms proposed
reference.glossary.ask_help = The Lektor (Settings → Assistants) reads the whole work and proposes terms that should be explained. They appear here as proposals; only you accept them. This can take a while.
reference.glossary.no_proposals = No open proposals.
reference.glossary.proposal_title = Proposal, not yet in the glossary
reference.glossary.proposal_by = Proposal by { $origin }
reference.glossary.accept = Accept
reference.glossary.reject = Reject
reference.glossary.new_term = New term

# templates/index_page.html
reference.index.decided_item = “{ $word }” → { $term } ({ $register }), { $origin }:
reference.index.head_title = Index – { $title } – Baruch
reference.index.page_title = { $title }: Index
reference.index.see_also = See also
reference.index.glossary = Glossary
reference.index.back = Back
reference.index.back_to_work = Back to the work
reference.index.word = Word in the text
reference.index.occurrence = Occurrence
reference.index.first = first
reference.index.all = all
reference.index.register = Index
reference.index.term = Entry
reference.index.term_hint = (empty = the word)
reference.index.sub = Subentry
reference.index.see = see
reference.index.see_also_field = see also
reference.index.heading = Index
reference.index.intro = Index entries are marks in the text, directly after the word; the index of Bible references builds itself from the recognised Bible references. Which indexes the book finally gets is set under Settings → Work → Typesetting. Setting or removing a mark makes a new revision of the section; assistants only propose entries.
reference.index.section = Section
reference.index.section_title = { $title } (current revision { $rev })
reference.index.no_entries_section = No index entries in this section.
reference.index.see_inline = (see: { $see })
reference.index.see_also_inline = (see also: { $also })
reference.index.auto = automatic
reference.index.remove_title = Remove mark (new revision)
reference.index.remove = Remove
reference.index.set_mark = Set mark (new revision)
reference.index.overview_error = The overview cannot be built: { $overview_error }
reference.index.filter = Filter entries
reference.index.term_placeholder = Entry
reference.index.auto_note = (automatically from the recognised Bible references)
reference.index.empty = No entries yet.
reference.index.see_list = see { $see }
reference.index.see_also_list = see also { $also }
reference.index.proposals = Proposals of the assistants ({ $count })
reference.index.proposals_error = The proposals cannot be read: { $proposals_error }
reference.index.no_proposals = No open index proposals.
reference.index.proposal_title = Proposal, not yet in the text
reference.index.proposal_by = Proposal by { $origin }
reference.index.proposal_word = “{ $word }” ({ $occurrence_label }) in
reference.index.proposal_mark = , revision { $based_on }: { $register } – { $term }
reference.index.reason = Reason: { $reason }
reference.index.accept_title = Set the mark into the section as a new revision
reference.index.accept = Accept
reference.index.reject = Reject
reference.index.decided = Decided proposals ({ $count })
reference.index.accepted = accepted
reference.index.rejected = rejected
reference.index.outdated = outdated (word no longer in the text)
reference.index.decided_by = by { $decided_by }
reference.index.manage = Manage indexes
reference.index.builtin = built in ({ $slug })
reference.index.rename = Rename
reference.index.delete_confirm = Yes, delete
reference.index.slug = Identifier
reference.index.slug_placeholder = e.g. places
reference.index.title = Title
reference.index.title_placeholder = e.g. Index of places
reference.index.create = Create index
reference.index.custom_left = { $custom_left } more own indexes possible (identifier: lower-case letters, digits, hyphen).
