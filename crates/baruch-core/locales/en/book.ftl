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
