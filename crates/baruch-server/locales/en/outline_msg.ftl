# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of the outline, numbering and revisions (Round 39a, task messages_1). Base catalog in English; locales/de/outline_msg.ftl has the same keys in
# the same order with the German wording.

# Outline (outline.rs).
outline_msg.param.number = { $name }: number from 0 expected
outline_msg.param.missing = { $name } missing
outline_msg.param.too_large = { $name }: too large
outline_msg.param.text_missing = { $name } missing (text)
outline_msg.param.place = place: before, after or into expected
outline_msg.param.decision = decision: confirm or reject expected
outline_msg.param.no_lock = Write lock missing
outline_msg.param.numbering = numbering: text or null expected
outline_msg.param.level_1_3 = level: 1 to 3
outline_msg.param.level_0_3 = level: 0 to 3
outline_msg.param.numbered = numbered missing (true = with number, false = without)
outline_msg.param.confirm_delete = Please confirm the deletion (confirm).
outline_msg.param.work_number = work: work id (a number) expected
outline_msg.preview.unnumbered = { $word } (without number)
outline_msg.page.template_error = Template { $name }: { $error }
outline_msg.page.error = Error: { $error }
outline_msg.page.unknown_work = unknown work
outline_msg.page.no_outline = The work has no outline.
outline_msg.page.unknown_action = unknown action

# Numbering schemes (numbering.rs).
outline_msg.numbering.scheme_missing = scheme missing (id of an outline scheme, e.g. “decimal”)
outline_msg.numbering.custom = custom: text or null expected
outline_msg.numbering.template_error = Error: template { $name }: { $error }
outline_msg.numbering.adopted = The outline scheme is adopted.
outline_msg.numbering.work_missing = Error: work missing (work id)
outline_msg.numbering.choose = Please choose an outline scheme.

# Revision history and policy (revisions.rs, revision_fold.rs).
outline_msg.revision.node_missing = node or key missing
outline_msg.revision.switch = { $name }: on or off expected
outline_msg.revision.max_change = Largest change: 1 to 100 percent.
outline_msg.revision.new_after = New revision after: 0 to 1440 minutes.
outline_msg.revision.amend = Continuation
outline_msg.revision.undo = Undo
outline_msg.revision.key_rev_missing = key and rev missing (section and revision).
outline_msg.revision.undone = Undone: the text is back at the chosen state.
outline_msg.fold.date_format = date: day as YYYY-MM-DD expected
outline_msg.fold.date_invalid = date: invalid day
outline_msg.fold.date_missing = date missing (YYYY-MM-DD)
