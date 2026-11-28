# SPDX-License-Identifier: AGPL-3.0-or-later
# Area "search" (Round 39a, templates_1): texts of the templates named in the sections below. Base
# catalog in English; locales/de/search.ftl has the same keys in the same order.

# templates/fragments/search_bar.html
search.bar.label = Search in the section
search.bar.expand_html = Show/hide replace (Ctrl+H)
search.bar.query = Search
search.bar.case_html = Match case (Alt+C)
search.bar.word = Whole words only (Alt+W)
search.bar.regex = Regular expression (Alt+R)
search.bar.selection = Search only in the selection (Alt+L; the selection in the source text when the search opens)
search.bar.no_hits = No matches
search.bar.prev = Previous match (Shift+Enter, Shift+F3)
search.bar.next = Next match (Enter, F3)
search.bar.close = Close (Escape)
search.bar.close_label = Close search bar
search.bar.replace = Replace
search.bar.replace_one = Replace current match (Ctrl+Shift+1)
search.bar.replace_all_title = Replace all in the section (Ctrl+Alt+Enter)
search.bar.replace_all = All
search.bar.work_title = Search and replace in the whole work (Ctrl+Shift+F)
search.bar.work = in the work …
search.bar.section_page = Search and replace in the section

# templates/search.html
search.page.head_title = Search and replace – { $title } – Baruch
search.page.page_title = { $title }: Search and replace
search.page.back = Back
search.page.back_to_work = Back to the work
search.page.heading = Search and replace in the work
search.page.undone_batch = Replacing “{ $query }” → “{ $replacement }” was undone.
search.page.replaced = Replaced:
search.page.replaced_in = “{ $query }” → “{ $replacement }” in { $count }
search.page.sections_one = section
search.page.sections_other = sections
search.page.replaced_note = (one new revision by you each, one commit in the mirror).
search.page.undo = Undo
search.page.check_quotes = Please check the wording of the Bible quotations (check view):
search.page.undone = Undone: the sections are back on their previous revision.
search.page.query = Search
search.page.replacement = Replace with
search.page.case_title_html = Match case (Alt+C)
search.page.case_html = Match case
search.page.word_title = Whole words only (Alt+W)
search.page.word = Whole word
search.page.regex_title = Regular expression (Alt+R)
search.page.regex = Regular expression
search.page.range = Range
search.page.outline = Outline (for “Section”, “Chapter”, “Part”, “from section” choose the item, for “Selection” tick the boxes)
search.page.pick_section = choose as section
search.page.picked_section = (chosen section)
search.page.rev = Revision (only section and selection)
search.page.rev_current = current revision
search.page.rev_shown = shown revision { $shown_rev }
search.page.rev_note = Preview and replace always apply to the current revision.
search.page.kinds = Kinds of text
search.page.kinds_note = Replacing leaves quotations (also Bible quotations) and Typst code untouched, unless you choose quotations explicitly.
search.page.preview_button = Preview the replacement
search.page.hits_one = match
search.page.hits_other = matches
search.page.truncated = Showing { $shown } of { $total } matches.
search.page.timed_out = The search took longer than 2 s and was stopped; the list is incomplete.
search.page.no_hits = No matches.
search.page.preview_heading = Preview: “{ $q }” → “{ $replacement }” · { $total } matches · { $range_label }
search.page.preview_timed_out = The search took longer than 2 s; replacing only works with a smaller range.
search.page.preview_truncated = The preview does not show all sections (at most 500 whole sections with at most 2000 places together); please narrow the search or the range.
search.page.preview_rev = Revision { $base_rev }, { $count }
search.page.places_one = place
search.page.places_other = places
search.page.open = open
search.page.places = Places:
search.page.confirm = Yes, replace (one new revision by me per section)
search.page.replace_all = Replace all
search.page.nothing = Nothing to replace.
search.page.latest = Last replacement: “{ $query }” → “{ $replacement }” in { $count }
search.page.keys_help_html = The search looks in the Typst source of the revisions. Keys as in VS Code: Ctrl+F search and Ctrl+H replace in the section, Ctrl+Shift+F or Ctrl+Shift+H in the work, Enter/F3 next match, Alt+C/W/R match case, whole word, regular expression.
