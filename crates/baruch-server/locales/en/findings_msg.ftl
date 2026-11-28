# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of findings (Round 39a, task messages_1). Base catalog in English; locales/de/findings_msg.ftl has the same keys in
# the same order with the German wording.

# Findings page and matrix (findings/page.rs).
findings_msg.filter.status = status: expected “open”, “adopted”, “rejected”, “deferred” or “done”, not “{ $word }”
findings_msg.filter.kind = kind: expected “praise”, “hint” or “critical”, not “{ $word }”
findings_msg.param.texts = { $name }: texts expected
findings_msg.param.text_or_list = { $name }: text or list expected
findings_msg.param.text = { $name }: text expected
findings_msg.param.time = { $name }: time like 2026-11-01 expected
findings_msg.param.include_praise = include_praise: true or false expected
findings_msg.param.older_range = older_than_days: 0 to { $max } expected
findings_msg.param.older_number = older_than_days: number expected
findings_msg.param.persona = persona: invalid
findings_msg.param.object = Parameters as an object expected
findings_msg.param.id_missing = id missing (finding)
findings_msg.param.entry_missing = entry missing (proposal)
findings_msg.matrix.solutions = { $solved } with solution, { $unsolved } without solution
findings_msg.matrix.praise = { $count } praise
findings_msg.matrix.none = no findings
findings_msg.matrix.no_part = Without part
findings_msg.matrix.part_itself = { $title } (the part itself)
findings_msg.matrix.no_section_long = Without section (threads, deleted sections)
findings_msg.matrix.no_section = Without section
findings_msg.notice.status = Status saved (one commit in the mirror).
findings_msg.notice.decision = Decision saved (one commit in the mirror).
findings_msg.notice.saved = Saved.
findings_msg.page.unknown_work = unknown work
findings_msg.page.error = Error: { $error }
findings_msg.page.template_error = Error: template { $name }: { $error }
findings_msg.page.unknown_action = unknown action
findings_msg.page.work_missing = work missing (work id)

# RPC methods finding.* (findings/rpc.rs).
findings_msg.param.number = { $name }: number expected
findings_msg.param.entry_history_missing = entry missing (proposal in the history)
findings_msg.param.flag = { $name }: true or false expected
findings_msg.param.text_list = { $name }: list of texts expected
findings_msg.param.list_or_text = { $name }: list or text “a,b” expected
findings_msg.param.confirm_missing = confirm missing (true = confirm, false = reject)
findings_msg.param.keys = keys: fixed key (a number) expected, not “{ $key }”
findings_msg.param.status_unknown = status: unknown status “{ $status }”
findings_msg.param.kind_unknown = kind: unknown kind “{ $kind }”
findings_msg.param.limit = limit: 1 to 2000
findings_msg.status.adopted_rev = “adopted” needs the revision (rev).
findings_msg.status.rev_number = rev: revision number from 1
findings_msg.status.reason_missing = Rejecting needs a reason (reason).
findings_msg.status.target_missing = “deferred” needs a target: target_key (section) or target_idea (idea).
findings_msg.status.target_both = “deferred” needs exactly one target: target_key or target_idea, not both.
findings_msg.status.unknown = status: unknown status “{ $status }” (open, adopted, rejected, deferred, done)
findings_msg.status.history_missing = History of the finding missing after setting

# Findings in words (findings/view.rs).
findings_msg.view.untitled = (untitled)
findings_msg.view.adopted_in = { $label } in { $revision }
findings_msg.view.revision_deleted = { $label } (revision deleted)
findings_msg.view.section = Section { $key }
findings_msg.view.deferred_to = { $label } to { $target }
findings_msg.view.deferred_idea = { $label } in idea { $id }
findings_msg.view.target_deleted = { $label } (target deleted)
findings_msg.view.confirmed = confirmed
findings_msg.view.rejected = rejected
findings_msg.view.stale = stale
findings_msg.view.rev_current = current
findings_msg.view.rev_proposed = proposal
findings_msg.view.rev_rejected = rejected
findings_msg.view.rev_superseded = superseded
findings_msg.view.revision = Revision { $number } ({ $status })
findings_msg.view.stale_summary = stale (all origins decided)
