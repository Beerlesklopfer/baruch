# SPDX-License-Identifier: AGPL-3.0-or-later
# Area "outline" (Round 39a, templates_1): texts of the templates named in the sections below. Base
# catalog in English; locales/de/outline.ftl has the same keys in the same order.

# templates/outline.html
outline.page.findings_one = { $count } open finding
outline.page.findings_other = { $count } open findings
outline.page.head_title = Outline – { $title } – Baruch
outline.page.page_title = Outline · { $title }
outline.page.back = Back
outline.page.back_to_work = Back to the work
outline.page.heading = Outline of “{ $title }”
outline.page.intro = This page is the fallback without JavaScript; with JavaScript everything happens in the tree on the left (drag and drop, context menu, double click or F2 to rename). Every change is a commit in the Git mirror. If level or title change, the section gets a new revision that differs only in the heading line. Assistants at most create new items; renaming, moving, changing the level and deleting they only request – you decide that below in the requests.
outline.page.total = Total time
outline.page.total_words =
    ({ $words } { $words ->
        [one] word
       *[other] words
    }, { $words_per_minute } words per minute)
outline.page.target = · Target time
outline.page.over = Warning: target time exceeded by { $over_clock }
outline.page.within = · within the limit
outline.page.col_number = Number
outline.page.col_title = Title
outline.page.col_level = Level
outline.page.col_duration = Duration
outline.page.col_arrange = Rearrange
outline.page.col_count = Count, edit
outline.page.findings_children = including sub-items
outline.page.findings_own = ({ $findings_own } on the item itself)
outline.page.findings_open = { $count } open
outline.page.words =
    { $words } { $words ->
        [one] word
       *[other] words
    }
outline.page.start_clock = from { $start_clock }
outline.page.before = Before “{ $up_title }”
outline.page.up = Up
outline.page.after = After “{ $down_title }”
outline.page.down = Down
outline.page.promote = Promote
outline.page.demote = Demote
outline.page.unnumber_title = Count without a number (e.g. preface, introduction)
outline.page.number_title = Count with a number again
outline.page.unnumbered = without number
outline.page.numbered = with number
outline.page.edit = Edit
outline.page.requests = Requests of the assistants
outline.page.requests_open = ({ $count } open)
outline.page.requests_help = Confirming carries out the request (one commit in the Git mirror, you as the confirmer, the assistant as the origin). If the item has changed in the meantime, the request is not carried out but closed as outdated. Rejecting changes nothing in the outline. A new part under which existing chapters (in a sermon: sermon parts) would move needs your consent (“Agree”); rejecting is then also recorded in the Git mirror.
outline.page.col_no = No.
outline.page.col_request = Request
outline.page.col_from = From
outline.page.col_reason = Reason
outline.page.show_preview = Show preview
outline.page.preview_lazy = (only computed on request)
outline.page.preview = Preview
outline.page.preview_error = No preview: { $preview_error }
outline.page.preview_changed = (before { $before_label } → after { $label })
outline.page.mark_renamed = renamed
outline.page.mark_moved = moved
outline.page.mark_level = new level
outline.page.mark_deleted = dropped
outline.page.mark_new = new
outline.page.mark_under = moves under the new part
outline.page.agree = Agree
outline.page.confirm = Confirm
outline.page.reject = Reject
outline.page.no_requests = No open requests.
outline.page.decided = Decided recently
outline.page.decided_item = Request { $id }: { $summary } ({ $origin_label }) –
outline.page.numbering = Numbering
outline.page.now = Now:
outline.page.pattern_effective = pattern “{ $effective }”
outline.page.no_numbers = no numbers
outline.page.template_none = (the work template sets none – default “1.1”)
outline.page.numbering_rule = . Parts count on their own, chapters continuously across parts.
outline.page.choose_scheme = Choose scheme…
outline.page.from_template = From the template
outline.page.pattern = Pattern
outline.page.set = Set
outline.page.no_numbers_button = No numbers
outline.page.pattern_help = Patterns as in Typst: counters 1, a, A, i, I with text before and between, e.g. “1.1”, “I.1”, “1.a)”; “I|1” numbers only the chapters, “I.;A.;1.;a)” gives each level its own pattern.

# templates/outline_confirm.html
outline.confirm.head_title = Confirm saving – Baruch
outline.confirm.page_title = Confirm saving · { $title }
outline.confirm.split = The text contains new headings. Saving splits “{ $title }”; new are:
outline.confirm.merge = The text has no heading any more. Saving appends it to “{ $previous }”; “{ $title }” is dropped (posts move along, the old text is in the Git mirror).
outline.confirm.split_save = Split and save
outline.confirm.merge_save = Merge and save
outline.confirm.back = Back
outline.confirm.back_note = “Back” discards the text; it is then not saved.

# templates/outline_edit.html
outline.edit.head_title = Edit { $title } – Outline – Baruch
outline.edit.page_title = Edit outline item · { $title }
outline.edit.back = Back
outline.edit.outline = Outline
outline.edit.back_to_work = Back to the work
outline.edit.rename = Rename
outline.edit.new_title = New title
outline.edit.title_rule = At most 200 characters, not empty
outline.edit.rename_part = The part mark in the text gets the new title (new revision, comment stays); an empty part gets it anew. A part text without a mark (older stock) stays unchanged, only the title changes.
outline.edit.rename_heading = The heading line in the text gets the new title (new revision, label and comment stay).
outline.edit.level = Level
outline.edit.set_level = Change level
outline.edit.level_note = Sub-items move along by the same number of levels.
outline.edit.move = Move
outline.edit.before = before
outline.edit.after = after
outline.edit.under = under
outline.edit.target = Target
outline.edit.move_note_html = With all sub-items. The level stays: before/after only an item of the same level, under only an item one level higher; otherwise change the level first (above). Invalid targets are refused.
outline.edit.new = New outline item
outline.edit.title = Title
outline.edit.add_before = before it
outline.edit.add_after = after it
outline.edit.add_under = below it
outline.edit.like_place = like the place
outline.edit.create = Create
outline.edit.add_note_html = Before/after on the same level, below it one level deeper as the last sub-item.
outline.edit.add_part_note = With level “{ $level_names }” a new part arises before or after it (also with a work template); the following chapters up to the next part then stand under it, their texts and numbers stay.
outline.edit.has_children = “{ $title }” has sub-items; move or delete them first.
outline.edit.delete_confirm = Really delete “{ $title }” with all revisions and posts
outline.edit.delete_note = The text of the current revision is then completely in the commit of the Git mirror.

# templates/outline_retry.html
outline.retry.head_title = Not saved – Baruch
outline.retry.page_title = Not saved · { $title }
outline.retry.failed = Not saved: { $message }
outline.retry.your_text = Your text (not saved yet)
outline.retry.again = Save again
outline.retry.back = Back without saving
outline.retry.back_note = “Back without saving” discards the text; it is then not saved.

# Round 39b (rest_literals): names and examples of the outline schemes of baruch-typst (scheme.rs),
# translated by the server (numbering.rs); the German text is verbatim the label of the crate.
outline.scheme.template = From the work template
outline.scheme.template_example = like #set heading(numbering: …) of the work template, otherwise 1 · 1.1
outline.scheme.decimal = Decimal (DIN 1421)
outline.scheme.decimal_example = 1 · 1.1 · 1.1.1
outline.scheme.decimal_dot = Decimal with a final dot
outline.scheme.decimal_dot_example = 1. · 1.1. · 1.1.1.
outline.scheme.alphanumeric = Alphanumeric
outline.scheme.alphanumeric_example = I. · A. · 1. · a) · aa)
outline.scheme.chapter_roman = Chapters only, Roman
outline.scheme.chapter_roman_example = I · II · III, sections without a number
outline.scheme.chapter_arabic = Chapters only, Arabic
outline.scheme.chapter_arabic_example = 1 · 2 · 3, sections without a number
outline.scheme.parts_chapters = Parts and chapters
outline.scheme.parts_chapters_example = Part I · Chapter 1 · 1.1 · 1.1.1
outline.scheme.chapter_words = Chapters in words
outline.scheme.chapter_words_example = First Part · Chapter One · Chapter Two (words in the work language)
outline.scheme.legal = Legal
outline.scheme.legal_example = § 1 · para. 1 · no. 1
outline.scheme.sermon = Sermon
outline.scheme.sermon_example = Introduction · I. · II. · Conclusion
outline.scheme.book = Book
outline.scheme.book_example = Introduction · I. · II. · Conclusion, parts “First Part”
outline.scheme.none = No numbers
outline.scheme.none_example = no numbers
outline.scheme.custom = Own pattern
outline.scheme.custom_example = pattern language, e.g. I.;A.;1.;a) or I|1
outline.scheme.no_numbers = none
outline.scheme.custom_missing = For “Own pattern” the pattern is missing, e.g. “I.;A.;1.” or “I|1”.
