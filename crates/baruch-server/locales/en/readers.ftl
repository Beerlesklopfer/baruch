# SPDX-License-Identifier: AGPL-3.0-or-later
# Round 39b (rest_literals): names, devices and notes of the reader profiles of baruch-epub (readers.rs) on the
# EPUB preview, translated by the server (epub_preview.rs). Base catalog in English; locales/de/readers.ftl has the
# same keys in the same order, the German text verbatim as the crate writes it.

readers.apple_books_iphone.name = Apple Books (iPhone)
readers.apple_books_iphone.device = iPhone with a 6.1-inch screen
readers.apple_books_iphone.note = Default font “Original” (the font of the book, otherwise Georgia), about 17 px, narrow margins, footnotes as a popup. Own fonts are safe only with com.apple.ibooks.display-options.xml (assumption).
readers.apple_books_ipad.name = Apple Books (iPad)
readers.apple_books_ipad.device = iPad with a 10.9-inch screen
readers.apple_books_ipad.note = As on the iPhone, larger font and wide margins; two columns in landscape (assumption).
readers.google_play_books.name = Google Play Books
readers.google_play_books.device = Android phone with a 6.3-inch screen
readers.google_play_books.note = Default “Original” (the font of the book, otherwise Literata), footnotes as a popup, night mode. Readers of the Android makers are not checked without a device (assumption, question to the user).
readers.kindle.name = Kindle (via EPUB)
readers.kindle.device = Kindle Paperwhite, 6.8 inches, E-Ink
readers.kindle.note = No Kindle format of its own: Send to Kindle accepts EPUB and converts it. Default font Bookerly (the font of the book only with “Publisher font”), greyscale, no background colours.
readers.kindle_app.name = Kindle app (via EPUB)
readers.kindle_app.device = Phone with the Kindle app, 6.1 inches
readers.kindle_app.note = Send to Kindle accepts EPUB and converts it; Bookerly in the app, in colour, without background colours (assumption).
readers.tolino.name = tolino
readers.tolino.device = tolino shine, 6 inches, E-Ink
readers.tolino.note = Font of the book (otherwise a serif font of the device), greyscale, footnotes as a jump to the note text instead of a popup (assumption).
readers.kobo.name = Kobo
readers.kobo.device = Kobo Clara, 6 inches, E-Ink
readers.kobo.note = Ordinary EPUB (not KEPUB): footnotes as a jump, the font of the book selectable, greyscale (assumption).
readers.thorium.name = Thorium Reader
readers.thorium.device = Computer (window)
readers.thorium.note = Readium reader on the computer: full EPUB 3, footnotes as a popup, night mode.
readers.calibre.name = calibre (e-book viewer)
readers.calibre.device = Computer (window)
readers.calibre.note = The viewer of calibre: font of the book, footnotes as a popup, dark colour scheme selectable.
