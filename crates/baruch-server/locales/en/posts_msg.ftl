# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of posts, threads and chats (Round 39a, task messages_1). Base catalog in English; locales/de/posts_msg.ftl has the same keys in
# the same order with the German wording.

# Posts of the section chat (posts.rs).
posts_msg.param.number = { $name }: number expected
posts_msg.param.id_missing = id missing (post)
posts_msg.param.rev = rev: revision from 1
posts_msg.param.confirm_delete = Please confirm the deletion (confirm).
posts_msg.param.target_seq = target: position from 0 expected
posts_msg.param.target_missing = Target missing: give target_key (fixed key of the section).
posts_msg.param.words_missing = words missing (at least one keyword)
posts_msg.param.body_missing = body missing (new text)
posts_msg.param.key_missing = key missing (fixed key of the section)
posts_msg.param.limit = limit: 1 to 200 expected
posts_msg.param.work_number = work: work id (a number) expected
posts_msg.param.no_section = Section { $key } does not exist
posts_msg.param.before_elsewhere = before: post { $id } is not in this section
posts_msg.refused.not_own = Only your own posts can be moved, deleted or converted through MCP: post { $id } comes from { $author }, not from { $actor }. The human manages other posts.
posts_msg.refused.not_editable = Only your own posts can be edited: post { $id } comes from { $author }, not from { $actor }. Posts of personas stay unchanged (copying, quoting, converting into an idea or a finding works).
posts_msg.repeat.no_order = Post { $id } has no stored order and no preceding message of the human; it cannot be repeated. Please ask the question again.
posts_msg.repeat.only_personas = Only answers of personas can be repeated; post { $id } comes from { $author }.
posts_msg.repeat.mode = The order “{ $mode }” cannot be repeated.
posts_msg.repeat.no_post = The repetition produced no post.
posts_msg.page.error = Error: { $error }
posts_msg.page.template_error = Error: template { $name }: { $error }
posts_msg.page.unknown_action = unknown action
posts_msg.form.confirm_delete = Please confirm the deletion (tick the box).
posts_msg.page.key_missing = Error: key missing (fixed key of the section)
posts_msg.page.no_section = Section { $key } does not exist in this work.
posts_msg.page.id_missing = Error: id missing (post)

# Threads without a section (threads.rs).
posts_msg.thread.empty_answer = The model delivered no usable answer; nothing saved.
posts_msg.thread.message_empty = The message is empty.
posts_msg.thread.text = { $name }: text expected
posts_msg.thread.thread_missing = thread missing (id of the thread)
posts_msg.thread.id_missing = id missing (thread post)
posts_msg.thread.not_own = Only your own posts can be deleted through MCP: thread post { $id } comes from { $author }, not from { $actor }. The human manages other posts.
posts_msg.thread.no_order = Post { $id } has no stored order and no preceding message of the human; it cannot be repeated.
posts_msg.thread.choose = Thread missing: choose one from the list.
posts_msg.thread.form_ids = Error: thread and id (numbers) missing
posts_msg.thread.confirm_delete = Please confirm the deletion (tick the box “really delete”).
posts_msg.thread.partly_answered = Your post is saved, but not every persona answered: { $errors }

# Links in chat posts (chat_render.rs).
posts_msg.chat.jump_no_section = Jump mark unchecked: section { $key } does not exist in this work
posts_msg.chat.jump_no_quote = Jump mark unchecked: the quotation is not in section { $key }
posts_msg.chat.open_idea = Open idea { $id }
posts_msg.chat.open_finding = Open finding { $id }
posts_msg.chat.open_bible = Open { $passage } in the Bible of the work
posts_msg.chat.quote = Quotation
posts_msg.chat.quote_in = Quotation in { $number } { $title }
