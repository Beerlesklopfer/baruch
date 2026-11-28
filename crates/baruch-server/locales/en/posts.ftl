# SPDX-License-Identifier: AGPL-3.0-or-later
# Area "posts" (Round 39a, templates_1): texts of the templates named in the sections below. Base
# catalog in English; locales/de/posts.ftl has the same keys in the same order.

# templates/fragments/post_item.html
posts.item.open_own_title = Post for marking and copying, quoting, editing and history (also without JavaScript)
posts.item.open_title = Post for marking and copying, quoting and history (also without JavaScript)
posts.item.manage_own_title = Edit, delete, move or convert the post
posts.item.manage_title = Delete, move or convert the post
posts.item.edited_title = last edited { $edited }; the old version is in the history
posts.item.edited = edited
posts.item.basis_title = Basis of this answer (from Baruch, not from the model)
posts.item.replies_to = replies to
posts.item.post_ref = post { $reply_to }
posts.item.relay_from = by { $relay_from }
posts.item.repeat_of = repeat of
posts.item.post_repeat = post { $repeat_of }
posts.item.after_rev = after revision { $basis_rev }
posts.item.ref = ↪ { $number } { $title } · Revision { $rev }
posts.item.source = Source: { $source }
posts.item.web_title = fetched pages, quotations and their check; confirmed quotations into the library
posts.item.web = ↪ Evidence of the web search
posts.item.repeat_label = Repeat the answer of { $author }
posts.item.repeat_title = The same request to { $author } against the shown revision { $rev }; the old answer stays
posts.item.delete_label = Delete post
posts.item.delete_title = Delete post (with confirmation)
posts.item.copy_title = Text of the post to the clipboard
posts.item.copy = Copy
posts.item.quote_title = Post as “> …” into the input field
posts.item.quote = Quote
posts.item.open = Open post
posts.item.manage = Manage
posts.item.edit = Edit
posts.item.new_text_label = New text of the post
posts.item.old_stays = the old version stays in the history
posts.item.delete_confirm = really delete
posts.item.move_menu = Move…
posts.item.move_to = to
posts.item.move = Move
posts.item.tags = Keywords
posts.item.tags_placeholder = e.g. rose, thorn
posts.item.tags_label = Keywords of the idea
posts.item.to_idea = Convert into an idea
posts.item.to_findings_title = Create findings from this post; the post stays unchanged
posts.item.to_findings = Convert into findings
posts.item.repeat_again_title = The same request to { $author } once more; the old answer stays
posts.item.quote_reply_title = Post as “> …” into the reply
posts.item.new_text = New text
posts.item.old_stays_thread = The old version stays in the history (threads are not in the mirror)
posts.item.history = History ({ $count })
posts.item.version = valid from { $valid_from }, replaced { $replaced } by { $edited_by }

# templates/fragments/post_solutions.html
posts.solutions.label = Solutions from post { $id }
posts.solutions.heading = Solutions
posts.solutions.by = by { $author }
posts.solutions.instead = instead of “{ $quote }”
posts.solutions.better = better “{ $replacement }”
posts.solutions.step = Step: { $step }
posts.solutions.at = (at “{ $quote }”)
posts.solutions.missing = (quotation is not in the shown revision)
posts.solutions.accept = Accept as proposal
posts.solutions.variants_label = Turn the solutions from post { $id } into variants
posts.solutions.lektor_writes = Lektor writes
posts.solutions.variants = Variants
posts.solutions.apply = Apply solutions

# templates/posts.html
posts.chat.count_one = { $count } post
posts.chat.count_other = { $count } posts
posts.chat.open_own_title = Copy, quote, edit and history
posts.chat.open_title = Copy, quote and history
posts.chat.head_title = Chat history – { $title } – Baruch
posts.chat.page_title = Chat history · { $title }
posts.chat.back = Back
posts.chat.back_to_section = Back to the section
posts.chat.heading = Section chat: { $title }
posts.chat.oldest_first = , oldest first.
posts.chat.to_newest = To the newest post ↓
posts.chat.source = Source: { $source }
posts.chat.repeat_label = Repeat the answer of { $author }
posts.chat.repeat_title = The same request against the current revision; the old answer stays
posts.chat.delete_label = Delete post
posts.chat.delete_title = Delete post (with confirmation)
posts.chat.copy_title = Text of the post to the clipboard
posts.chat.copy = Copy
posts.chat.open = Open post
posts.chat.empty = No posts on this section yet.
posts.chat.post = Post
posts.chat.placeholder = ask @persona … · RFC … · @alle
posts.chat.as_task = as a task (a writing task becomes a proposal)
posts.chat.send = Send

# templates/post.html
posts.post.head_title = Post { $id } – Baruch
posts.post.page_title = Post · { $title }
posts.post.back = Back
posts.post.history = Chat history
posts.post.back_to_section = Back to the section
posts.post.heading = Post { $id } by { $author }
posts.post.edited = · edited { $edited }
posts.post.post_label = Post
posts.post.copy_label = Text for copying
posts.post.copy_text = Text (mark and copy)
posts.post.source = Source: { $source }
posts.post.copy_title = Text to the clipboard
posts.post.copy = Copy
posts.post.quote = Quote
posts.post.reply = Reply in the section chat
posts.post.as_task = as a task (a writing task becomes a proposal)
posts.post.send = Send
posts.post.edit = Edit
posts.post.new_text = New text
posts.post.old_stays = The old version stays in the history; the change is in the mirror of the work
posts.post.to_findings = Convert into findings
posts.post.persona_posts = Posts of personas stay unchanged: copy, quote or convert them.
posts.post.own_only = Only your own posts can be edited.
posts.post.to_findings_title = Create findings from this post (without marks one note with the whole text); the post stays
posts.post.delete_confirm = Really delete post { $id }
posts.post.delete_title = The post disappears from the chat; the mirror keeps it in the deletion commit
posts.post.versions = History
posts.post.version = valid from { $valid_from }, replaced { $replaced } by { $edited_by }
posts.post.never_edited = Never edited.

# templates/thread.html
posts.thread.head_title = { $title } – Thread – Baruch
posts.thread.page_title = Thread · { $title }
posts.thread.back = Back
posts.thread.all_threads = All threads
posts.thread.back_to_work = Back to the work
posts.thread.started_by = started by { $created_by } · without a section ·
posts.thread.to_newest = to the newest post ↓
posts.thread.posts_label = Posts of the thread
posts.thread.reply = Reply
posts.thread.placeholder = @lektor … or “Lektor, …” calls a persona · RFC … or @alle calls everyone
posts.thread.persona = Let a persona answer
posts.thread.mentioned = – only those mentioned (@…) –
posts.thread.send = Reply

# templates/threads.html
posts.threads.count_one = { $count } post
posts.threads.count_other = { $count } posts
posts.threads.head_title = Threads – Baruch
posts.threads.page_title = Threads · { $title }
posts.threads.back = Back
posts.threads.back_to_work = Back to the work
posts.threads.heading = Threads without a section
posts.threads.intro = Conversations with personas and MCP clients that belong to no section. They do not change any book text.
posts.threads.started_by = started by { $created_by } ·
posts.threads.last = · last { $last_author }, { $updated }
posts.threads.empty = No threads yet.
posts.threads.new = New thread
posts.threads.message = Message
posts.threads.placeholder = @lektor … calls a persona
posts.threads.persona = Let a persona answer
posts.threads.mentioned = – only those mentioned (@…) –
posts.threads.start = Start thread
