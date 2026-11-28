# SPDX-License-Identifier: AGPL-3.0-or-later
# Round 39a (templates_2): Findings page, findings panel and a finding as a block.
# Base catalog in English; locales/de/findings.ftl has the same keys in the same order.

# Shared by the findings page, the panel and the block.
findings.back_to_work = Back to the work
findings.status = Status
findings.kind = Kind
findings.kind_hint = Hint
findings.kind_critical = Critical
findings.kind_praise = Praise
findings.open_in_work = open findings in the work (badge)
findings.open_count = { $count } open
findings.by = by { $name }
findings.dash_quote = – “{ $text }”
findings.quoted = “{ $text }”
findings.without_section = without a section
findings.post = Post
findings.place_hint = Open the place in the section
findings.place_hint_quote = Open the place in the section and mark the quotation
findings.to_place = to the place
findings.jump_hint = Jump to the place in the section

# Page /werke/{id}/befunde (befunde.html).
findings.page.window_title = Findings · { $title } – Baruch
findings.page.page_title = Findings · { $title }
findings.page.heading = Findings in “{ $title }”
findings.page.intro = Findings are the single points from the answers of the personas. Only the human sets the status; assistants propose at most. Open counts what is neither decided nor attached to an equal finding nor praise – the same number appears as the badge “k open” in the outline and in the editor head.
findings.page.book_note_heading = Write a finding: note on the book
findings.page.book_note_hint = A note on the whole book, without a section. You write findings on a section on the work page (on the right under “Findings”). The author decides on the status.
findings.page.book_note_label = Note
findings.page.book_note_text = Text of the note
findings.page.book_note_submit = Write a note on the book
findings.page.filter_label = Filter findings
findings.page.all = all
findings.page.since = since
findings.page.until = before
findings.page.until_hint = up to this day, exclusive
findings.page.older_than = older than
findings.page.days = days
findings.page.show_praise = Show praise
findings.page.apply = Apply
findings.page.clear_filter = Clear filter
findings.page.praise_hide = Hide praise
findings.page.praise_show = Show praise
findings.page.matrix_label = Matrix chapter by persona
findings.page.matrix_heading = Matrix: chapter × persona
findings.page.signs = Signs
findings.page.legend = Signs: ○ open · ✓ adopted · ✗ rejected · → moved · = done without a change
findings.page.legend_praise = · + praise
findings.page.legend_many = . Chapters with { $count } or more open findings are highlighted.
findings.page.truncated = Capped: the matrix counts only the newest { $limit } of { $total } findings in the filter; the badges count all of them. A narrower filter (period, persona, status) shows the rest.
findings.page.solutions = Solutions: { $words } (without praise; per row in the sum as “with/without”).
findings.page.chapter = Chapter
findings.page.sum = Sum
findings.page.many_hint = many open findings
findings.page.many = many open
findings.page.all_personas = all personas: { $words }
findings.page.sum_of = Sum { $title }
findings.page.total = Total
findings.page.none = No findings
findings.page.none_for_filter = for this filter
findings.page.none_after = . Findings arise from the answers of the personas (review, chat, threads).
findings.page.single = Single finding
findings.page.missing = Finding { $ref } does not exist in this work.
findings.page.finding = Finding { $ref }
findings.page.attached_to = Attached to
findings.page.not_open = – does not count as open.
findings.page.history = History
findings.page.proposal = proposal
findings.page.still_open = , open
findings.page.note = – note: { $note }
findings.page.no_change = No change yet (created { $time }).
findings.page.cell_label = Findings of the cell
findings.page.cell_missing = Section { $key } does not exist in this work (any more).
findings.page.cell_in = Findings in
findings.page.back_to_matrix = Back to the whole matrix
findings.page.cell_empty = No findings in this cell.
findings.page.old_label =
    Open, older than { $days } { $days ->
        [one] day
       *[other] days
    }
findings.page.old_heading =
    Open, older than { $days } { $days ->
        [one] day
       *[other] days
    } ({ $count })
findings.page.show = Show
findings.page.old_truncated = There are { $total } open findings that are older; shown are the { $limit } newest of them. The older ones are in the matrix (click a cell) or appear with a larger number at “older than … days”.
findings.page.old_none = No open findings that are older.
findings.page.summaries = Collective proposals
findings.page.summaries_heading = Collective proposals ({ $count })
findings.page.summaries_hint = In an RFC the lector sums up the findings of the others – only as a proposal with a reference to the origins.
findings.page.summaries_none = No collective proposals.
findings.page.history_per_chapter = History per chapter
findings.page.col_adopted = ✓ incorporated
findings.page.col_rejected = ✗ rejected
findings.page.col_deferred = → moved
findings.page.col_done = = done
findings.page.col_revisions = In revisions
findings.page.revision = revision { $rev }
findings.page.stand_hint = State of the continuation when adopting
findings.page.stand = · state { $stand }
findings.page.history_none = No decided findings yet.

# Cards (macros of fragments/findings_panel.html, also fragments/finding_block.html).
findings.card.question_label = Query about finding { $ref }
findings.card.question_hint = Query of a persona
findings.card.question = Query
findings.card.question_default = Finding { $id } seems done – mark it as done?
findings.card.yes_done = Yes, done
findings.card.no_open = No, still open
findings.card.proposal_hint = open proposal of an assistant
findings.card.proposal = Proposal
findings.card.proposes = { $actor } proposes:
findings.card.confirm = Confirm
findings.card.reject = Reject
findings.card.set_status = Set status …
findings.card.change_status = Change status …
findings.card.adopted_label = Finding { $ref } adopted
findings.card.adopted_in = ✓ adopted in
findings.card.adopted = Adopted
findings.card.reject_label = Reject finding { $ref }
findings.card.rejected_because = ✗ rejected because (required)
findings.card.defer_label = Move finding { $ref }
findings.card.deferred_to_section = → moved to section
findings.card.or_idea = or into idea no.
findings.card.note = Note
findings.card.one_target = Exactly one target: a section or an idea.
findings.card.defer = Move
findings.card.done_label = Finding { $ref } done
findings.card.done_note = = done without a change, note
findings.card.done = Done
findings.card.reopen_label = Reopen finding { $ref }
findings.card.reopen = ○ Reopen
findings.card.solutions_label = Solutions for finding { $ref }
findings.card.solutions = { $count ->
    [one] Solution
    *[other] Solutions
    }
findings.card.instead = instead of “{ $quote }”
findings.card.better = better “{ $replacement }”
findings.card.step = Step: { $step }
findings.card.at_quote = (at “{ $quote }”)
findings.card.adopt_as_proposal = Adopt as a proposal
findings.card.no_solution = without a proposed solution
findings.card.single = Finding { $id } on its own
findings.card.kind_is = Kind: { $label }
findings.card.status_is = Status: { $label }
findings.card.reason = Reason: { $reason }
findings.card.summary_by = Collective proposal by { $persona }
findings.card.summary_on = on
findings.card.origin_missing = (origin missing)
findings.card.attached = { $count } attached (equal to this finding, do not count as open)

# Block of a finding (fragments/finding_block.html).
findings.block.adopted_in_rev = ✓ adopted in revision { $number }
findings.block.done = = done
findings.block.rejected_because = ✗ rejected because
findings.block.more = more options (move …)

# Accordion "Befunde" of the work page (fragments/findings_panel.html).
findings.panel.title = Findings
findings.panel.open_hint =
    { $count } open { $count ->
        [one] finding
       *[other] findings
    } in this section
findings.panel.pending_hint = open proposals of the assistants
findings.panel.pending = { $count ->
    [one] { $count } proposal
    *[other] { $count } proposals
    }
findings.panel.total = { $count } in total
findings.panel.all_of_section = All findings of this section
findings.panel.overview = Overview of the work
findings.panel.variants_label = Turn the solutions of the open findings into variants
findings.panel.lector_writes = Lector writes
findings.panel.variants_from_open = variants from the open findings
findings.panel.apply_solutions = Apply solutions
findings.panel.none_open = No open findings in this section.
findings.panel.decided = { $count } decided (adopted, rejected, moved, done)
findings.panel.praise = { $count } praise (hidden)

# Round 39b (rest_literals): status and kind of a finding (baruch-store FindingStatus/FindingKind::label),
# translated by the server (findings/mod.rs); German verbatim as the store and the mirror write them.
findings.status_label.open = open
findings.status_label.adopted = adopted
findings.status_label.rejected = rejected
findings.status_label.deferred = deferred
findings.status_label.done = done without a change
findings.kind_label.praise = praise
findings.kind_label.hint = hint
findings.kind_label.critical = critical
