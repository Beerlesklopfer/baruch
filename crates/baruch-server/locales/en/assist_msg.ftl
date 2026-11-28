# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of the assistants (personas, chat, solutions, variants) (Round 39a, task messages_1). Base catalog in English; locales/de/assist_msg.ftl has the same keys in
# the same order with the German wording.

# Checks and refusals of model orders (assist/mod.rs).
assist_msg.check.question_too_long = The question may have at most { $max } characters.
assist_msg.check.message_too_long = The message may have at most { $max } characters.
assist_msg.check.post_too_long = The post may have at most { $max } characters.
assist_msg.check.question_missing = The question is missing.
assist_msg.retry.forbidden_opening = forbidden opening
assist_msg.answer.empty = The model delivered no usable answer; nothing saved.
assist_msg.answer.cut_off = The model's answer broke off at the length limit (max_tokens); an incomplete proposal is not created. Nothing saved.
assist_msg.answer.equals_current = The proposal equals the current revision; nothing created.
assist_msg.answer.equals_basis = The proposal equals revision { $rev } on which it is based; nothing created.
assist_msg.answer.nothing_to_check = Nothing to check: the text is empty; nothing saved.
assist_msg.answer.chapter_empty = The chapter has no text; nothing to revise, nothing created.
assist_msg.answer.no_proposal = No proposal created: { $skipped }. Nothing saved.
assist_msg.compress.summary_failed = The context was too long and had to be compressed, but summarising “{ $label }” failed: { $error } Nothing saved.
assist_msg.compress.too_many_parts = The context length of { $handle } ({ $tokens } tokens) is not enough even compressed: { $piece } would need at least { $size } characters with its surroundings, there is room for { $budget } – the chapter has too many parts for this context length (omitted parts also stand with their title in the call). Nothing saved. Increase the context length under Settings → Assistants or have a single section checked.
assist_msg.compress.piece = piece { $number } of { $count } ({ $what })

# Solutions and variants (assist/solutions.rs).
assist_msg.solution.no_wording = { $label } has no new wording (only a step); it cannot be inserted.
assist_msg.solution.quote_gone = { $label }: the quotation “{ $quote }” is not (any longer) in the text.
assist_msg.solution.no_revision_at_all = The section has no revision.
assist_msg.solution.no_such_revision = Revision { $number } does not exist in this section.
assist_msg.solution.section_gone = The section is no longer there.
assist_msg.solution.label = Solution
assist_msg.solution.numbered = Solution { $pick }
assist_msg.solution.key_missing = key missing (fixed key of the section)
assist_msg.solution.rev_number = { $name }: revision number from 1 expected
assist_msg.solution.number_list = { $name }: list of numbers expected
assist_msg.solution.post_elsewhere = Post { $id } does not belong to this section.
assist_msg.solution.no_section = Section { $key } does not exist in this work.
assist_msg.solution.picks_missing = picks missing: choose at least one solution
assist_msg.solution.finding_elsewhere = Finding { $id } does not belong to this section.
assist_msg.solution.finding_without_post = The finding comes from no post of this section.
assist_msg.solution.post_missing = post missing (post with the solutions) – or finding
assist_msg.solution.no_such_solution =
    Solution { $pick } does not exist ({ $count } { $count ->
        [one] solution
       *[other] solutions
    }).
assist_msg.solution.only_step = It names only a step, no new wording.
assist_msg.solution.quote_not_in = The quotation “{ $quote }” is not in revision { $rev }.
assist_msg.solution.not_adoptable = Solution { $pick } cannot be adopted: { $why }
assist_msg.solution.no_change = The solutions do not change the text; no proposal created.
assist_msg.solution.count = count: 2 or 3 variants (at most { $max }), not { $n }
assist_msg.solution.rev_missing = rev missing (the variant that stays)
assist_msg.solution.confirm_missing = confirm: true missing – the other variants are discarded
assist_msg.solution.no_rev = Revision { $rev } does not exist.
assist_msg.solution.not_a_variant = Revision { $rev } is not a variant.
assist_msg.solution.no_group_in_section = Variant group { $group } does not exist in this section.
assist_msg.solution.no_group_in_work = Variant group { $group } does not exist in this work.
assist_msg.solution.group_missing = group missing (variant group)
assist_msg.page.template_error = Error: template { $name }: { $error }
assist_msg.page.error = Error: { $error }
assist_msg.page.unknown_action = unknown action
assist_msg.page.work_missing = work missing (work id)

# Variants of a section (assist/variants.rs).
assist_msg.variant.title = Variant { $label }: { $summary }
assist_msg.variant.finding_elsewhere = [B{ $id }] does not belong to this section and stays out.
assist_msg.variant.count = 2 or 3 variants (at most { $max }), not { $count }
assist_msg.variant.no_lector = No lector active (Settings → Assistants).
assist_msg.variant.no_findings = No findings to carry out: neither named nor open in this section. Nothing created.
assist_msg.variant.too_long = “{ $title }” is too long for the context length of { $handle } ({ $tokens } tokens), and no quotation of the findings is in the text – a partial proposal cannot be delimited. Nothing created.
assist_msg.variant.paragraphs_too_long = Even the affected paragraphs of “{ $title }” do not fit the context length of { $handle } ({ $tokens } tokens). Nothing created.
assist_msg.variant.partial = Long section: partial proposal for the paragraphs { $paragraphs } (the rest stays verbatim).
assist_msg.variant.cut_off = Variant { $label }: broken off at the length limit, not created.
assist_msg.variant.unchanged = Variant { $label }: unchanged, not created.
assist_msg.variant.none = No variant created: { $notes }

# Personas and their settings (personas.rs).
assist_msg.persona.bible_refused = Bible evidence from SWORD exists only for @belege and the lector.
assist_msg.persona.rag_refused = Semantic hits exist only for @belege and the lector.
assist_msg.persona.scope = scope: “chapter” (whole chapter) or “section” (section only)
assist_msg.persona.flag = { $name }: true or false expected
assist_msg.persona.persona_missing = persona missing (e.g. “@belege”)
assist_msg.persona.fixed_handle = handle missing: which persona is the fixed lector?
assist_msg.persona.lector_mode = mode: “fixed” or “auto” expected
assist_msg.persona.handle_missing = handle missing (e.g. “@lektor”)
assist_msg.persona.text = { $name }: text expected
assist_msg.persona.context_range = The context length must be between 1 and 10000000 tokens.
assist_msg.persona.active_flag = active: true or false expected
assist_msg.persona.color_param = color: "#rrggbb" or null expected
assist_msg.persona.prompt_param = prompt: text or null (default) expected
assist_msg.persona.on_flag = on: true or false expected
assist_msg.persona.connection_param = connection: id of the model connection expected
assist_msg.persona.color_form = Give the colour as #rrggbb.
assist_msg.persona.choose_connection = Please choose a model connection.

# Persona orders over RPC (assist/rpc.rs, assist/chat.rs).
assist_msg.rpc.number = { $name }: number expected
assist_msg.rpc.scope = scope: expected “section” or “chapter”, not “{ $other }”
assist_msg.rpc.node_refused = node (position) is not accepted; give key (fixed key of the section).
assist_msg.rpc.rev_number = rev: revision number from 1 expected
assist_msg.rpc.no_lector = No persona is active (no lector).
assist_msg.rpc.persona_missing = persona missing (e.g. “@lektor”)
assist_msg.chat.no_rev = The proposal has no revision number.
assist_msg.chat.finding_elsewhere = [B{ $id }]: the finding does not belong to this section; no status proposal for revision { $rev }.
assist_msg.chat.basis = basis: shown, current or a revision number

# Notice page after a persona order without JavaScript (assist/routes.rs).
assist_msg.notice.title_saved = Answer saved
assist_msg.notice.title = Notice
assist_msg.notice.onward = On to the section
assist_msg.notice.back = Back to the section
assist_msg.notice.partly_answered = Your post and the finished answers are saved, but not every persona answered:
assist_msg.notice.proposal_created = The proposal is created.
assist_msg.notice.answer_saved = The answer is saved.
assist_msg.stream.json_object = JSON object {"{"}method, …{"}"} expected

# Model calls (assist/port.rs, assist/eliezer.rs).
assist_msg.model.not_configured = No model backend usable: { $why }
assist_msg.model.unreachable = The model backend cannot be reached: { $why }
assist_msg.model.denied = The model backend refuses access: { $why }
assist_msg.model.timeout = Time-out at the model: { $why }
assist_msg.model.unknown = Unknown model: { $why }
assist_msg.model.busy = The model is busy: { $why }
assist_msg.model.cancelled = Cancelled; nothing was saved.
assist_msg.model.failed = The model call failed: { $why }
assist_msg.model.cached_list = List from the cache (as of { $fetched }).

# Review basis and glossary proposals (assist/basis.rs, assist/glossary.rs).
assist_msg.basis.invalid = basis: expected “shown”, “current” or a revision number, not “{ $shown }”
assist_msg.basis.no_revision = The section has no revision yet that could be checked.
assist_msg.glossary.no_text = No text from which terms could be proposed; nothing created.
assist_msg.glossary.no_terms = { $handle } named no terms in the form “term | explanation”; nothing created.
