# SPDX-License-Identifier: AGPL-3.0-or-later
# Book catalog, German translation (Round 39b, interface S1). Same keys in the same order as locales/en/book.ftl.
# The German values are verbatim the literals the outputs wrote before Round 39b, so German output stays byte-equal.

# Contents and front matter
book.toc.title = Inhalt
book.cover.title = Umschlag
book.cover.alt = Umschlag: { $title }
book.cover.isbn_placeholder = ISBN und Strichcode
book.title_page.title = Titel
book.untitled = Ohne Titel

# Back matter
book.glossary.title = Glossar
book.index.empty = Keine Einträge.

# Figures, notes, cross references
book.figure.supplement = Abbildung
book.footnote.back = zurück zum Text
book.xref.page_abbr = S.

# Typesetting
book.draft.watermark = ENTWURF

# Numbering words
book.numbering.part = Teil { $number }
book.numbering.chapter = Kapitel { $number }
book.numbering.introduction = Einleitung
book.numbering.conclusion = Schluss
book.numbering.legal_paragraph = Abs.
book.numbering.legal_number = Nr.

# Imprint
book.imprint.dnb_title = Bibliografische Information der Deutschen Nationalbibliothek
book.imprint.dnb_text = Die Deutsche Nationalbibliothek verzeichnet diese Publikation in der Deutschen Nationalbibliografie; detaillierte bibliografische Daten sind im Internet über https://dnb.de abrufbar.
book.imprint.typeset_with = Gesetzt mit Baruch
book.imprint.cover_credit = Umschlaggestaltung und Bildnachweis: { $credit }

# Read-back words (scripture check, claims)
book.scripture.wording_unchecked = Wortlaut ungeprüft
book.claims.not_supported = nicht belegt
book.claims.check_reference = Angabe prüfen

# Added by task ausgaben (Round 39b): verbatim the literals the outputs wrote before, same order as locales/en/book.ftl.

# EPUB front matter, navigation and metadata
book.front.imprint = Impressum
book.front.dedication = Widmung
book.nav.landmarks = Orientierung
book.nav.text = Text
book.endnotes.title = Anmerkungen
book.image.missing = Bild fehlt: { $path }
book.meta.image_credit = Bildnachweis: { $credit }
book.a11y.summary_text = Reiner Text mit Inhaltsverzeichnis und Überschriften in Lesereihenfolge.
book.a11y.summary_all_alt = Text mit Inhaltsverzeichnis und Überschriften in Lesereihenfolge; jedes Bild hat einen Alternativtext.
book.a11y.summary_some_alt = Text mit Inhaltsverzeichnis und Überschriften in Lesereihenfolge; nicht jedes Bild hat einen Alternativtext.

# Glossary and index
book.glossary.empty = Keine Begriffe.
book.glossary.variants = Auch: { $variants }
book.glossary.bible_refs = Bibelstellen: { $refs }
book.glossary.bible_refs_label = Bibelstellen:
book.index.see = siehe
book.index.see_also = siehe auch
book.index.keyword_register = Stichwortregister
book.index.person_register = Personenregister
book.index.scripture_register = Bibelstellenregister

# Draft
book.draft.title = { $title } (Entwurf)
book.draft.notice = Entwurf: { $text }
book.draft.as_of = Stand { $date }

# Imprint lines
book.imprint.bible = Bibelzitate: { $bible }.
book.imprint.printer = Druck: { $printer }

# Notes sheet of a sermon
book.notes_sheet.title = Stichwortzettel – { $title }
book.notes_sheet.total = Gesamt { $clock }
book.notes_sheet.target = Ziel { $clock }
book.notes_sheet.empty = Keine Abschnitte.

# Read-back words of the scripture check
book.scripture.unchecked = ungeprüft
book.scripture.mark_scripture = Wortlaut: ungeprüft – keine lokale Bibel in Baruch (geprüft nur Buch/Kapitel/Vers).
book.scripture.mark_others = Hinweis: Bibelstellen und Wortlaut prüft @schrift; hier ungeprüft.
book.scripture.mark_bible = Wortlaut geprüft von Baruch (nicht vom Modell)
book.scripture.mark_others_bible = { $mark } mit dem Modul { $module }; die Persona hat ihn nicht selbst bestätigt (Ergebnis je Stelle: Zitate prüfen oder @belege).
book.scripture.bible_line = { $mark }, Modul { $module }, Versifikation { $versification }, Lizenz laut conf: { $notice }: { $list }.
book.scripture.no_passage = keine Stelle im Abschnitt
book.scripture.wording_matches = Wortlaut stimmt
book.scripture.wording_differs = Wortlaut weicht ab
book.scripture.partial = Teilzitat
book.scripture.module_spacing = Leerraum im Modul
book.scripture.no_quote = kein Zitat gefunden
book.scripture.invalid_ref = Stelle ungültig – Wortlaut nicht geprüft
book.scripture.missing_verse = Vers fehlt im Modul
book.scripture.versification_mismatch = Versifikation passt nicht
book.scripture.module = Modul

# Read-back words of claims and web evidence
book.claims.in_text = (Stelle steht im Text:
book.claims.invalid = (Stelle ungültig:
book.websearch.not_supported = (nicht belegt)
book.websearch.unknown_source = (unbekannte Quelle)
book.websearch.not_fetched = (nicht abgerufen)
book.websearch.check_note = Angabe prüfen: nicht belegt
