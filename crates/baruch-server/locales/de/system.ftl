# SPDX-License-Identifier: AGPL-3.0-or-later
# System pages (about, shut down, sync, consent) (Round 39a, templates_3), German translation of locales/en/system.ftl (same keys in the same order).

# templates/consent.html
system.consent.needed_html = <strong>Zustimmung nötig:</strong>
system.consent.one = { $origin_label } möchte die Gliederung umstellen: { $summary }.
system.consent.many = { $requests } Anträge der Assistenten warten auf Ihre Zustimmung.
system.consent.nothing_without = Ohne Ihre Zustimmung ändert sich nichts.
system.consent.open = Ansehen und entscheiden
system.consent.title = Zustimmung zur Gliederung
system.consent.request_html = <strong>{ $origin_label }</strong> möchte im Werk „{ $work_title }“: { $summary }.
system.consent.note = Begründung: { $note }
system.consent.preview = Gliederung nach Ihrer Zustimmung:
system.consent.no_preview = Keine Vorschau: { $preview_error }
system.consent.before_after = (vorher { $before_label } → nachher { $label })
system.consent.added = neu
system.consent.adopted = wandert unter den neuen Teil
system.consent.confirm = Zustimmen
system.consent.reject = Ablehnen
system.consent.later = Später entscheiden

# templates/about.html
system.about.window_title = Über Baruch – Baruch
system.about.title = Über Baruch
system.about.back_group = Zurück
system.about.back_to_work = Zurück zum Werk
system.about.intro = Baruch ist eine freie Schreibwerkstatt für Bücher und Predigten. Es läuft auf dem eigenen Rechner, die Daten bleiben dort.
system.about.license = Lizenz
system.about.license_text = – GNU Affero General Public License, Version 3 oder (nach Wahl) jede spätere Version. Baruch kommt ohne jede Gewährleistung.
system.about.source = Quellcode
system.about.source_text_html = – der Quelltext genau dieser Version steht dort beim Tag <code>v{ $version }</code>.
system.about.website = Webseite
system.about.help = Hilfe mit Bildern:
system.about.help_online = (auf der Webseite, braucht das Internet)
system.about.data_dir = Datenordner
system.about.data_default_html = (Vorgabe: <code>{ $default_data_dir }</code>)
system.about.data_help = Alle Daten liegen in diesem Ordner des Benutzers, nie im Installationsverzeichnis. Kein Paket legt dort etwas an, und das Entfernen von Baruch löscht dort nichts. Größe, Spiegel der Werke und Sicherung:
system.about.settings_data = Einstellungen → Daten
system.about.updates = Neue Versionen
system.about.updates_text = Baruch fragt nie von sich aus im Netz nach neuen Versionen. Wer nachsehen will, öffnet selbst die
system.about.release_page = Release-Seite dieser Version
system.about.updates_more_html = (<code>{ $release_url }</code>) – erst der Klick ruft sie im Browser auf. Aktualisiert wird von Hand mit einem neuen Paket von dort; nur wer Baruch unter Debian oder Ubuntu über das APT-Repository bezieht, bekommt neue Versionen mit <code>sudo apt upgrade</code>.
system.about.third_party = Bibliotheken Dritter
system.about.third_party_text_html = Das Programm enthält Rust-Bibliotheken Dritter. Namen, Versionen, Lizenzen und Lizenztexte stehen in der Datei <code>{ $third_party_file }</code>, die jedes Paket mitbringt:
system.about.third_party_here_html = Auf diesem Rechner: <code>{ $third_party }</code>
system.about.appimage_html = – im eingehängten AppImage <code>{ $appimage }</code>; der Pfad gilt nur, solange Baruch läuft.
system.about.third_party_missing_html = Neben diesem Programm liegt die Datei nicht (aus dem Quelltext gebaut oder an einen anderen Ort kopiert). Im Quelltext erzeugt sie <code>scripts/third-party-licenses.py --out { $third_party_file }</code>.

# templates/shutdown.html
system.shutdown.window_title = Baruch beenden – Baruch
system.shutdown.title = Baruch beenden
system.shutdown.back_group = Zurück
system.shutdown.back_to_work = Zurück zum Werk
system.shutdown.question = Soll Baruch auf diesem Rechner beendet werden?
system.shutdown.help_html = Gespeicherte Texte stehen bereits in der Datenbank und im Git-Spiegel; es geht nichts verloren. Ungespeicherte Eingaben in anderen Fenstern vorher speichern. Laufende Antworten gehen noch zu Ende (höchstens zehn Sekunden), ein offener Live-Chat wird danach getrennt. Danach ist Baruch nicht mehr erreichbar, bis es neu gestartet wird (Startmenü, Programme, <code>baruch</code> im Terminal oder der eingerichtete Dienst).
system.shutdown.submit = Baruch beenden
system.shutdown.local_only = Baruch lässt sich nur am eigenen Rechner beenden.
system.shutdown.remote = Diese Anfrage kam von einem anderen Rechner oder über den Namen eines vorgeschalteten Servers (Reverse-Proxy). Von dort aus beendet Baruch sich nicht – sonst könnte jeder, der die Seite erreicht, die Schreibwerkstatt abschalten.
system.shutdown.how_html = Beenden geht am Rechner, auf dem Baruch läuft: dort im Browser <code>http://127.0.0.1:8765/beenden</code> öffnen (bzw. die Adresse, unter der Baruch dort lauscht), oder den Dienst anhalten (<code>systemctl --user stop baruch</code>).

# templates/shutdown_done.html
system.shutdown_done.window_title = Baruch ist beendet
system.shutdown_done.text = Gespeicherte Texte liegen in der Datenbank und im Git-Spiegel. Zum Weiterarbeiten Baruch neu starten.

# templates/sync.html
system.sync.window_title = Abgleich mit MultiGPT – Baruch
system.sync.title = Abgleich mit MultiGPT
system.sync.back_group = Zurück
system.sync.back_to_work = Zurück zum Werk
system.sync.intro = Baruch lädt die aktuellen Fassungen der Werke mit Schalter „nach MultiGPT hochladen“ hoch (ein Dokument je Kapitel) und holt die gewählten Sammlungen als lokale Kopie, die auch offline durchsuchbar ist. Nur dieser Knopf startet den Abgleich; Fehler erscheinen hier als Hinweis, lokal geht nichts verloren.
system.sync.unconfigured = MultiGPT ist noch nicht eingerichtet:
system.sync.settings_connections = Einstellungen → Verbindungen
system.sync.run_title = Abgleichen
system.sync.run = Abgleichen
system.sync.running = Der Abgleich läuft gerade …
system.sync.last = zuletzt abgeglichen
system.sync.unfinished = (begonnen { $started }, nicht beendet)
system.sync.ok_html = – <strong>ohne Fehler</strong>
system.sync.hints_html = – <strong>mit Hinweisen</strong>
system.sync.uploaded = hochgeladen:
system.sync.replaced = ersetzt:
system.sync.removed = entfernt:
system.sync.downloaded = geholt:
system.sync.unchanged = unverändert:
system.sync.failed = Fehler:
system.sync.never = Noch nie abgeglichen.
system.sync.works_title = Werke nach MultiGPT hochladen
system.sync.works_help = Vorgabe: aus. Eingeschaltet geht beim nächsten Abgleich je Kapitel ein Dokument mit den aktuellen Fassungen in die Sammlung „{ $collection_pattern }“; geänderte Kapitel werden ersetzt.
system.sync.work_title_placeholder = Werktitel
system.sync.upload_on = – hochladen an, Sammlung „{ $collection }“, { $documents } Kapitel
system.sync.last_synced = , zuletzt { $synced }
system.sync.switch_off = Hochladen ausschalten
system.sync.upload_off = – hochladen aus
system.sync.still_there =
    ({ $documents } { $documents ->
        [one] Kapitel liegt
       *[other] Kapitel liegen
    } noch in „{ $collection }“)
system.sync.confirm = Ja, die aktuellen Fassungen dieses Werks dürfen nach MultiGPT
system.sync.switch_on = Hochladen einschalten
system.sync.no_works = Es gibt noch kein Werk.
system.sync.collections_title = Sammlungen von MultiGPT holen
system.sync.chosen = Gewählte Sammlungen (lokale Kopie, offline suchbar)
system.sync.documents =
    { $count } { $count ->
        [one] Dokument
       *[other] Dokumente
    }
system.sync.documents_at_multigpt =
    { $count } { $count ->
        [one] Dokument
       *[other] Dokumente
    } bei MultiGPT
system.sync.no_collection = Keine Sammlung gewählt.
system.sync.more = Weitere Sammlung
system.sync.collections_apply = Sammlungen übernehmen
system.sync.collections_load = Sammlungen von MultiGPT laden
