# SPDX-License-Identifier: AGPL-3.0-or-later
# Round 39b (rest_literals): names, devices and notes of the reader profiles of baruch-epub (readers.rs) on the
# EPUB preview, translated by the server (epub_preview.rs). Base catalog in English; locales/de/readers.ftl has the
# same keys in the same order, the German text verbatim as the crate writes it.

readers.apple_books_iphone.name = Apple Books (iPhone)
readers.apple_books_iphone.device = iPhone mit 6,1-Zoll-Bildschirm
readers.apple_books_iphone.note = Vorgabeschrift „Original“ (Schrift des Buchs, sonst Georgia), etwa 17 px, schmale Ränder, Fußnoten als Popup. Eigene Schriften sicher nur mit com.apple.ibooks.display-options.xml (Annahme).
readers.apple_books_ipad.name = Apple Books (iPad)
readers.apple_books_ipad.device = iPad mit 10,9-Zoll-Bildschirm
readers.apple_books_ipad.note = Wie auf dem iPhone, größere Schrift und breite Ränder; im Querformat zwei Spalten (Annahme).
readers.google_play_books.name = Google Play Books
readers.google_play_books.device = Android-Telefon mit 6,3-Zoll-Bildschirm
readers.google_play_books.note = Vorgabe „Original“ (Schrift des Buchs, sonst Literata), Fußnoten als Popup, Nachtmodus. Hersteller-Reader auf Android sind ohne Gerät nicht geprüft (Annahme, Nutzerfrage).
readers.kindle.name = Kindle (über EPUB)
readers.kindle.device = Kindle Paperwhite, 6,8 Zoll, E-Ink
readers.kindle.note = Kein eigenes Kindle-Format: Send to Kindle nimmt EPUB an und wandelt es um. Vorgabeschrift Bookerly (die Schrift des Buchs nur mit „Verlagsschrift“), Graustufen, keine Hintergrundfarben.
readers.kindle_app.name = Kindle-App (über EPUB)
readers.kindle_app.device = Telefon mit Kindle-App, 6,1 Zoll
readers.kindle_app.note = Send to Kindle nimmt EPUB an und wandelt es um; in der App Bookerly, farbig, ohne Hintergrundfarben (Annahme).
readers.tolino.name = tolino
readers.tolino.device = tolino shine, 6 Zoll, E-Ink
readers.tolino.note = Schrift des Buchs (sonst eine Serifenschrift des Geräts), Graustufen, Fußnoten als Sprung zum Notentext statt Popup (Annahme).
readers.kobo.name = Kobo
readers.kobo.device = Kobo Clara, 6 Zoll, E-Ink
readers.kobo.note = Gewöhnliches EPUB (nicht KEPUB): Fußnoten als Sprung, Schrift des Buchs wählbar, Graustufen (Annahme).
readers.thorium.name = Thorium Reader
readers.thorium.device = Rechner (Fenster)
readers.thorium.note = Readium-Leser am Rechner: EPUB 3 vollständig, Fußnoten als Popup, Nachtmodus.
readers.calibre.name = calibre (E-Book-Betrachter)
readers.calibre.device = Rechner (Fenster)
readers.calibre.note = Betrachter von calibre: Schrift des Buchs, Fußnoten als Popup, dunkles Farbschema wählbar.
