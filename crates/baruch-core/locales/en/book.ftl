# SPDX-License-Identifier: AGPL-3.0-or-later
# Book catalog (Round 39b, interface S1): labels and generated words that go INTO a book output (EPUB, PDF/Typst,
# DOCX, notes sheet) and words Baruch writes for read-back. Chosen by the WORK language, never by the interface
# language. Base catalog in English; locales/de/book.ftl has the same keys in the same order.
# Format: a strict subset of Fluent, one `key = value` per line, placeholders exactly `{ $name }`. Values are plain
# text; callers escape for XML/XHTML/Typst/OOXML themselves. Read by baruch_core::book_text.
# Keys: book.<area>.<name>[.<more>]; the contract seed `book.untitled` is the one exception (see KEY_PATTERN_EXCEPTIONS).

# Contents and front matter
book.toc.title = Contents
book.cover.title = Cover
book.cover.alt = Cover: { $title }
book.cover.isbn_placeholder = ISBN and barcode
book.title_page.title = Title page
book.untitled = Untitled

# Back matter
book.glossary.title = Glossary
book.index.empty = No entries.

# Figures, notes, cross references
book.figure.supplement = Figure
book.footnote.back = back to the text
book.xref.page_abbr = p.

# Typesetting
book.draft.watermark = DRAFT

# Numbering words
book.numbering.part = Part { $number }
book.numbering.chapter = Chapter { $number }
book.numbering.introduction = Introduction
book.numbering.conclusion = Conclusion
book.numbering.legal_paragraph = para.
book.numbering.legal_number = no.

# Imprint
book.imprint.dnb_title = Bibliographic information published by the Deutsche Nationalbibliothek
book.imprint.dnb_text = The Deutsche Nationalbibliothek lists this publication in the Deutsche Nationalbibliografie; detailed bibliographic data are available on the Internet at https://dnb.de.
book.imprint.typeset_with = Typeset with Baruch
book.imprint.cover_credit = Cover design and image credit: { $credit }

# Read-back words (scripture check, claims)
book.scripture.wording_unchecked = wording unchecked
book.claims.not_supported = not supported
book.claims.check_reference = check reference

# Added by task ausgaben (Round 39b): further words of the book outputs, same order in locales/de/book.ftl.

# EPUB front matter, navigation and metadata
book.front.imprint = Imprint
book.front.dedication = Dedication
book.nav.landmarks = Guide
book.nav.text = Text
book.endnotes.title = Notes
book.image.missing = Image missing: { $path }
book.meta.image_credit = Image credit: { $credit }
book.a11y.summary_text = Plain text with a table of contents and headings in reading order.
book.a11y.summary_all_alt = Text with a table of contents and headings in reading order; every image has an alternative text.
book.a11y.summary_some_alt = Text with a table of contents and headings in reading order; not every image has an alternative text.

# Glossary and index
book.glossary.empty = No terms.
book.glossary.variants = Also: { $variants }
book.glossary.bible_refs = Bible references: { $refs }
book.glossary.bible_refs_label = Bible references:
book.index.see = see
book.index.see_also = see also
book.index.keyword_register = Index
book.index.person_register = Index of persons
book.index.scripture_register = Index of Bible references

# Draft
book.draft.title = { $title } (Draft)
book.draft.notice = Draft: { $text }
book.draft.as_of = As of { $date }

# Imprint lines
book.imprint.bible = Bible quotations: { $bible }.
book.imprint.printer = Printed by: { $printer }

# Notes sheet of a sermon
book.notes_sheet.title = Cue sheet – { $title }
book.notes_sheet.total = Total { $clock }
book.notes_sheet.target = Target { $clock }
book.notes_sheet.empty = No sections.

# Read-back words of the scripture check
book.scripture.unchecked = unchecked
book.scripture.mark_scripture = Wording: unchecked – no local Bible in Baruch (only book/chapter/verse checked).
book.scripture.mark_others = Note: Bible references and wording are checked by @schrift; unchecked here.
book.scripture.mark_bible = Wording checked by Baruch (not by the model)
book.scripture.mark_others_bible = { $mark } with the module { $module }; the persona did not confirm it itself (result per reference: check quotations or @belege).
book.scripture.bible_line = { $mark }, module { $module }, versification { $versification }, licence according to conf: { $notice }: { $list }.
book.scripture.no_passage = no reference in the section
book.scripture.wording_matches = wording matches
book.scripture.wording_differs = wording differs
book.scripture.partial = partial quotation
book.scripture.module_spacing = whitespace in the module
book.scripture.no_quote = no quotation found
book.scripture.invalid_ref = reference invalid – wording not checked
book.scripture.missing_verse = Verse missing in the module
book.scripture.versification_mismatch = Versification does not match
book.scripture.module = module

# Read-back words of claims and web evidence
book.claims.in_text = (reference is in the text:
book.claims.invalid = (reference invalid:
book.websearch.not_supported = (not supported)
book.websearch.unknown_source = (unknown source)
book.websearch.not_fetched = (not fetched)
book.websearch.check_note = check reference: not supported

# Titles of the preset chapters of a new work (Round 39c; the work language decides, see Store::create_work)
book.preset.first_chapter = First chapter
book.preset.sermon_text = Sermon text
book.preset.sermon_introduction = Introduction
book.preset.sermon_observation = Observation on the text
book.preset.sermon_interpretation = Interpretation
book.preset.sermon_assurance = Assurance
book.preset.sermon_application = Application
book.preset.sermon_outlook = Outlook

# Note on a Bible module with many word fragments (facts block and closing note of a persona answer, Round 39c)
book.scripture.fragment_example = “{ $example }”
book.scripture.fragment_note = Module { $module } has strikingly many word fragments ({ $suspects } of { $pairs } word pairs, e.g. { $examples }) – probably hyphenation remnants; Baruch therefore also compares the wording without whitespace.
