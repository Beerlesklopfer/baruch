# SPDX-License-Identifier: AGPL-3.0-or-later
# Round 39a (templates_2): Pages of the works: list, archive, edit, delete, outline scheme, import report.
# German translation of locales/en/works.ftl (same keys in the same order).

# Shared by the works pages.
works.back_to_work = Zurück zum Werk

# Page /werke (works.html).
works.list.window_title = Werke – Baruch
works.list.heading = Werke
works.list.new = Neues Werk oder Import…
works.list.archive_hint = Gelöschte Werke ansehen und wiederherstellen
works.list.archive = Archiv
works.list.col_cover = Umschlag
works.list.col_title = Titel
works.list.col_author = Autor
works.list.col_kind = Art
works.list.col_sections = Abschnitte
works.list.col_proposals = Offene Vorschläge
works.list.col_updated = Zuletzt geändert
works.list.col_actions = Aktionen
works.list.cover_alt = Umschlag von { $title }
works.list.open = Öffnen
works.list.edit = Ändern
works.list.export = Export
works.list.pdf_hint = Das Buch als PDF herunterladen
works.list.empty = Noch kein Werk. Lege eins an oder importiere eine Typst- oder Markdown-Datei (auch als ZIP oder Ordner mit Bildern):
works.list.empty_lektor = Noch kein Werk freigegeben. Der Autor gibt Werke einzeln frei (Seite „Mitwirkende“ des Werks).

# Page /werke/archiv (works_archive.html).
works.archive.window_title = Archiv – Baruch
works.archive.heading = Archiv der gelöschten Werke
works.archive.intro_html = Ein gelöschtes Werk liegt mit seinem Git-Spiegel unter <code>works/{ $trash }/</code>. Wiederherstellen legt es mit seinem alten Ordnernamen und Titel neu an: die aktuellen Texte als Fassung 1 („aus dem Archiv wiederhergestellt“), die Werkvorlage und die Satz-Einstellungen aus dem Spiegel. Ältere Fassungen, Beiträge und Befunde bleiben nur in der Git-Historie des Ordners lesbar; Bilder kommen nicht mit, und die Werkart geht verloren (eine Predigt kommt als „Buch“ zurück). Einfälle des Werks bleiben Einfälle ohne Werk – sie lassen sich danach auf der Seite „Einfälle“ einem Werk zuordnen.
works.archive.col_folder = Ordner
works.archive.col_deleted = Gelöscht
works.archive.col_action = Aktion
works.archive.not_found = nicht gefunden
works.archive.restore_label = { $title } wiederherstellen
works.archive.confirm = „{ $title }“ wiederherstellen
works.archive.restore = Wiederherstellen
works.archive.not_restorable = Nicht wiederherstellbar: { $reason }
works.archive.empty = Das Archiv ist leer: Es gibt keine gelöschten Werke.

# Page /werke/{id}/bearbeiten (work_edit.html, also on the settings page "Werk").
works.edit.window_title = { $title } ändern – Baruch
works.edit.page_title = { $title } ändern
works.edit.heading = Werk ändern
works.edit.folder_html = Ordnername im Spiegel: <code>works/{ $slug }/</code> (bleibt beim Umbenennen).
works.edit.imported_from_html = Importiert aus <code>{ $source }</code>.
works.edit.uuid = Kennung des Werks (UUID, für den Austausch zwischen Instanzen; nicht änderbar):
works.edit.title = Titel
works.edit.subtitle = Untertitel
works.edit.kind_book = Buch
works.edit.kind_sermon = Predigt
works.edit.audience = Zielgruppe
works.edit.bible = Bibelübersetzung
works.edit.template = Werkvorlage
works.edit.template_hint = (alles vor der ersten Überschrift: #set, #show, #let …; aktuell Fassung { $rev })
works.edit.note = Notiz zur neuen Fassung der Werkvorlage
works.edit.note_hint = Eine geänderte Werkvorlage wird eine neue Fassung; die alten bleiben unten lesbar.
works.edit.revisions = Fassungen der Werkvorlage
works.edit.revision = Fassung { $number }
works.edit.current = aktuell
works.edit.revision_read_only = Fassung { $number } (nur lesen)
works.edit.export_typst = Als Typst-Datei exportieren
works.edit.delete = Werk löschen…
works.edit.numbering = Gliederungsschema…
works.edit.images = Bilder…

# Page /werke/{id}/loeschen (work_delete.html).
works.delete.window_title = { $title } löschen – Baruch
works.delete.page_title = { $title } löschen
works.delete.heading = Werk löschen
works.delete.intro_html = Gelöscht werden das Werk <strong>{ $title }</strong>
works.delete.by = von { $author }
works.delete.sections = mit { $count } Abschnitten
works.delete.revisions = , alle Fassungen, Vorschläge
works.delete.open = (davon { $count } offen)
works.delete.posts = , Beiträge und die Fassungen der Werkvorlage.
works.delete.mirror_html = Der Git-Spiegel <code>works/{ $slug }/</code> bekommt einen letzten Commit „Werk gelöscht“ und wandert nach <code>works/{ $trash }/{ $slug }-JJJJ-MM-TT/</code>; dort bleibt jeder Text in der Git-Historie lesbar.
works.delete.restore_before = Über
works.delete.restore_link = Werke → Archiv
works.delete.restore_after = lässt sich das Werk später wiederherstellen (aktuelle Texte als Fassung 1); seine Einfälle werden zu Einfällen ohne Werk.
works.delete.export_before = Vorher lohnt ein
works.delete.export_link = Export als Typst-Datei
works.delete.reserved_html = Der Ordnername <code>{ $slug }</code> bleibt danach reserviert, damit das Archiv eindeutig bleibt: Ein neues Werk mit demselben Titel bekommt einen eigenen Ordner (z. B. <code>{ $slug }-2</code>) und eine neue Nummer. Ist nach dem Löschen kein Werk mehr da, legt Baruch kein Beispielwerk neu an.
works.delete.confirm_label = Zur Bestätigung den Titel eintippen
works.delete.submit = Endgültig löschen

# Page /werke/{id}/nummerierung (work_numbering.html, also on the settings page "Werk").
works.numbering.window_title = Gliederungsschema – { $title } – Baruch
works.numbering.page_title = Gliederungsschema · { $title }
works.numbering.see_also = Siehe auch
works.numbering.heading = Gliederungsschema von „{ $title }“
works.numbering.now_html = Jetzt: <strong>{ $label }</strong>
works.numbering.pattern = Muster „{ $pattern }“
works.numbering.no_numbers = keine Nummern
works.numbering.template_sets_none = , die Vorlage setzt keins – Vorgabe „1.1“
works.numbering.custom = („{ $pattern }“)
works.numbering.explain_html = Das Schema gilt für die Gliederung, den Kopf des Editors, den Export, den Satz und den Git-Spiegel (<code>main.typ</code>). Bei „Aus der Werkvorlage“ bleibt die Vorlage im Export unverändert; sonst folgt ihr eine Zeile <code>#set heading(numbering: …)</code>, die gespeicherte Vorlage bleibt, wie sie ist. Teile zählen eigen, Kapitel über Teile hinweg fortlaufend; Punkte ohne Nummer zählen nicht mit und stehen im Export als <code>#heading(…, numbering: none)</code>.
works.numbering.choose = Schema wählen
works.numbering.in_force = gilt jetzt
works.numbering.custom_label = Eigenes Muster
works.numbering.custom_hint = Mustersprache: ein Typst-Muster für alle Ebenen („1.1“, „I.1“, „1.a)“), mit „|k“ nur die Ebenen 1 bis k („I|1“ = nur Kapitel römisch) oder je Ebene ein Muster, getrennt durch „;“ („I.;A.;1.;a);aa)“; ein Muster mit einem Zählzeichen zeigt nur die eigene Zahl, ein leeres keine). Zählzeichen: 1, a, A, i, I, *; „aa“ = verdoppelter Buchstabe (aa, bb, cc).
works.numbering.preview_of = Vorschau { $label }
works.numbering.more = … und { $count } weitere
works.numbering.rule = Im Export steht dann hinter der Werkvorlage:
works.numbering.preview = Vorschau
works.numbering.apply = Übernehmen
works.numbering.buttons_hint = „Vorschau“ zeigt das gewählte Schema und das eigene Muster nur an; erst „Übernehmen“ ändert die Nummern (ein Commit im Git-Spiegel).

# Report of an import (import_report.html).
works.import.window_title = Import: { $title } – Baruch
works.import.page_title = Import-Bericht
works.import.next = Weiter
works.import.to_work = Zum Werk
works.import.heading = „{ $title }“ ist angelegt
works.import.taken_over = { $nodes } { $nodes ->
    [one] Abschnitt
    *[other] Abschnitte
    }, { $images } { $images ->
    [one] Bild
    *[other] Bilder
    } übernommen.
works.import.mirror_failed = Der Git-Spiegel ließ sich nicht schreiben: { $reason }
works.import.skipped = Übersprungene Bilder ({ $count })
works.import.skipped_hint = Der Text nennt sie weiter; auf der Seite „Bilder“ lassen sie sich später mit ihrem Pfad anlegen.
works.import.warnings = Hinweise ({ $count })
works.import.line = Zeile { $line }:
works.import.open_work = Werk öffnen
