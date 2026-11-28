# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of the view, ribbon, menu, consent bar and laptop mode (Round 39a, task messages_1). Base catalog in English; locales/de/view_msg.ftl has the same keys in
# the same order with the German wording.

# Format buttons of the ribbon (ribbon.rs).
view_msg.ribbon.scene = Scene
view_msg.ribbon.scene_title = Scene as defined by the work template (#szene)
view_msg.ribbon.work = Task
view_msg.ribbon.work_title = Work assignment as defined by the work template (#arbeit)
view_msg.ribbon.scripture = Bible passage
view_msg.ribbon.scripture_title = Bible passage as defined by the work template (#{ $name }[…], e.g. John 3:16)
view_msg.ribbon.cite = Literature reference
view_msg.ribbon.cite_title = Reference to an entry of the bibliography of the work template (@key)
view_msg.ribbon.footnote = Footnote
view_msg.ribbon.footnote_title = Footnote at the cursor (#footnote)
view_msg.ribbon.quote = Quotation
view_msg.ribbon.quote_title = Quotation as a block of its own (#quote); the reference comes as a line of its own directly below (– passage)
view_msg.ribbon.list = List
view_msg.ribbon.list_title = List with dashes (- …)
view_msg.ribbon.enum = Enumeration
view_msg.ribbon.enum_title = Numbered enumeration (1. …)
view_msg.ribbon.terms = Term
view_msg.ribbon.terms_title = Term list (/ Term: explanation)
view_msg.ribbon.break = Break
view_msg.ribbon.break_title = Line break within the paragraph (\)
view_msg.ribbon.link = Link
view_msg.ribbon.link_title = Link to an address (#link)
view_msg.ribbon.xref = Reference
view_msg.ribbon.xref_title = Cross-reference to an outline item (#link({ $lt }kN{ $gt })[…], optionally with page number)

# Levels, revision states and proposal heads of the workspace (view.rs).
view_msg.level.part = Part
view_msg.level.sermon_part = Sermon part
view_msg.level.chapter = Chapter
view_msg.level.section = Section
view_msg.level.subsection = Subsection
view_msg.status.current = current
view_msg.status.proposed = proposal
view_msg.status.rejected = rejected
view_msg.status.superseded = superseded
view_msg.proposal.diff_current = Difference to revision { $rev } (current) · shown
view_msg.proposal.diff = Difference to revision { $rev } · shown
view_msg.proposal.shown = shown
view_msg.proposal.head_lektor = { $handle } · revision { $rev }
view_msg.proposal.head = { $name } ({ $handle }) · revision { $rev }
view_msg.proposal.based_on_amend = based on revision { $rev }, state { $amend }
view_msg.proposal.on_rev = on revision { $rev }
view_msg.proposal.based_on_state = based on state { $amend }
view_msg.proposal.stale = outdated: revision { $rev } is now at state { $now }
view_msg.proposal.part = part { $k }/{ $parts }
view_msg.proposal.equals_shown = equals the shown revision
view_msg.saved.amended = Saved: amended (state { $amend }).
view_msg.saved.new = Saved: new revision { $rev }.
view_msg.saved.new_reason = Saved: new revision { $rev } ({ $reason }).
view_msg.xref.untitled = (untitled)
view_msg.basis.proposal_by = Proposal { $rev } by { $persona }
view_msg.basis.proposal = Proposal { $rev }

# Menu (menu.rs).
view_msg.menu.badges_params = menu.badges: parameters as an object expected
view_msg.menu.one_section = { $count } section
view_msg.menu.sections = { $count } sections

# Consent bar (consent.rs).
view_msg.consent.template_error = Template consent.html: { $error }
view_msg.consent.page_too_large = Error: page too large

# Editor (wysiwyg.rs).
view_msg.wysiwyg.chapter_unknown = unknown (numbering of the template not readable)
view_msg.wysiwyg.image_list = List of { $count } images, index chapter { $offset }; chapter counter { $at } lies outside

# Speaking time (timing.rs).
view_msg.timing.whole_number = { $name }: whole number expected
view_msg.timing.wpm_param = words_per_minute: whole number from 60 to 250 expected
view_msg.timing.wpm_range = Words per minute: 60 to 250 are allowed.
view_msg.timing.target_range = Target time: 1 to 240 minutes are allowed (empty or null = no target time).
view_msg.timing.work_missing = work missing: times are set per work (work: work id).
view_msg.timing.text = text: text expected
view_msg.timing.text_missing = text missing
view_msg.timing.text_too_long = text: at most 1 000 000 characters
