# SPDX-License-Identifier: AGPL-3.0-or-later
# Accounts (sign in, set up, my account, contributors) (Round 39a, templates_3), German translation of locales/en/accounts.ftl (same keys in the same order).

# templates/anmelden.html
accounts.sign_in.window_title = Anmelden – Baruch
accounts.sign_in.heading = Baruch – Anmelden
accounts.sign_in.too_many =
    Zu viele Fehlversuche. Bitte { $count } { $count ->
        [one] Sekunde
       *[other] Sekunden
    } warten und dann erneut versuchen.
accounts.sign_in.signed_in = Angemeldet als { $signed_in }.
accounts.sign_in.continue = Weiter
accounts.sign_in.name = Name
accounts.sign_in.password = Passwort
accounts.sign_in.submit = Anmelden
accounts.sign_in.already = Schon angemeldet?
accounts.sign_in.already_help = – nach einem Link von außen (E-Mail, andere Seite) fragt Baruch einmal nach; der Klick hier genügt dann.
accounts.sign_in.forgotten_html = Passwort vergessen? Der Autor setzt es unter Einstellungen → Konten neu. Hat der Autor selbst es vergessen, hilft am Rechner, auf dem Baruch läuft, der Befehl <code>{ $password_cmd }</code>.
accounts.cmd_name = Name

# templates/einrichten.html
accounts.setup.window_title = Konten einrichten – Baruch
accounts.setup.heading = Baruch – Konten einrichten
accounts.setup.first_html = Das erste Konto gehört dem <strong>Autor</strong>. Sobald es besteht, verlangt Baruch eine Anmeldung – auch an diesem Rechner. Danach lassen sich unter Einstellungen → Konten Lektoren anlegen und je Werk freigeben.
accounts.setup.without = Ohne Konten bleibt Baruch offen wie bisher. Wer nur allein am eigenen Rechner schreibt, braucht keine Konten.
accounts.setup.name = Name (zum Anmelden)
accounts.setup.display = Anzeigename (freiwillig)
accounts.setup.password = Passwort ({ $min } bis { $max } Zeichen)
accounts.setup.password_repeat = Passwort wiederholen
accounts.setup.create = Konto anlegen
accounts.setup.hash_html = Das Passwort speichert Baruch nur als Prüfwert (Argon2id), nie im Klartext. Geht es verloren, setzt am Rechner, auf dem Baruch läuft, der Befehl <code>{ $password_cmd }</code> ein neues.
accounts.setup.local_only = Das erste Konto lässt sich nur am eigenen Rechner einrichten.
accounts.setup.remote = Diese Anfrage kam von einem anderen Rechner oder über einen vorgeschalteten Server (Reverse-Proxy). Von dort aus legt Baruch kein Autor-Konto an – sonst könnte der Erste, der die Seite erreicht, die Schreibwerkstatt übernehmen.
accounts.setup.how_html = Einrichten geht am Rechner, auf dem Baruch läuft: dort im Browser Baruch direkt öffnen (zum Beispiel <code>http://127.0.0.1:8765/einrichten</code>, nicht über den Namen des Proxys).

# templates/konto.html
accounts.mine.window_title = Mein Konto – Baruch
accounts.mine.heading = Mein Konto
accounts.mine.back_group = Zurück
accounts.mine.back_to_work = Zurück zum Werk
accounts.mine.open = Baruch läuft ohne Konten: Alles ist offen wie bisher, eine Anmeldung gibt es nicht.
accounts.mine.why = Konten braucht nur, wer Lektoren mitlesen lassen oder Baruch über einen Proxy erreichbar machen will. Das erste Konto gehört dem Autor.
accounts.mine.set_up = Konten einrichten…
accounts.mine.local_html = Du arbeitest als <strong>Autor</strong> am eigenen Rechner, ohne Anmeldung (Einstellungen → Konten: „Am eigenen Rechner ohne Anmeldung“).
accounts.mine.sign_in = Anmelden
accounts.mine.settings_accounts = Einstellungen → Konten
accounts.mine.display = Anzeigename
accounts.mine.role = Rolle
accounts.mine.origin = Herkunft
accounts.mine.origin_help_html = <code>{ $handle }</code> – unter diesem Namen stehen deine Vorschläge, Beiträge und Befunde im Werk und im Git-Spiegel.
accounts.mine.granted_works = Freigegebene Werke
accounts.mine.works = Werke
accounts.mine.none_granted = Noch kein Werk freigegeben. Der Autor gibt Werke einzeln frei (Seite „Mitwirkende“ des Werks).
accounts.mine.no_work = Noch kein Werk.
accounts.mine.lector_help = Als Lektor liest du die freigegebenen Werke und schreibst Vorschläge für Fassungen, Beiträge im Abschnitts-Chat und Befunde. Übernehmen, ändern und löschen kann nur der Autor.
accounts.mine.change_password = Passwort ändern
accounts.mine.old_password = Bisheriges Passwort
accounts.mine.new_password = Neues Passwort ({ $min } bis { $max } Zeichen)
accounts.mine.repeat_password = Neues Passwort wiederholen
accounts.mine.other_sessions = Danach sind alle anderen Sitzungen dieses Kontos abgemeldet.
accounts.mine.sign_out = Abmelden

# templates/mitwirkende.html
accounts.contributors.window_title = Mitwirkende · { $title } – Baruch
accounts.contributors.page_title = Mitwirkende · { $title }
accounts.contributors.back_group = Zurück
accounts.contributors.back_to_work = Zurück zum Werk
accounts.contributors.accounts_title = Konten anlegen, sperren, Passwort zurücksetzen
accounts.contributors.settings_accounts = Einstellungen → Konten
accounts.contributors.heading = Mitwirkende an „{ $title }“
accounts.contributors.no_accounts = Baruch läuft ohne Konten – es gibt niemanden, dem das Werk freigegeben werden könnte.
accounts.contributors.set_up = Konten einrichten…
accounts.contributors.load_error = Die Freigaben lassen sich nicht lesen: { $load_error }
accounts.contributors.help = Der Autor sieht und bearbeitet jedes Werk. Ein Lektor sieht nur freigegebene Werke und schreibt dort Vorschläge, Beiträge und Befunde unter seiner Herkunft. Eine entzogene Freigabe wirkt bei der nächsten Anfrage; was der Lektor geschrieben hat, bleibt im Werk.
accounts.contributors.granted_to = Freigegeben für
accounts.contributors.col_display = Anzeigename
accounts.contributors.col_origin = Herkunft
accounts.contributors.col_role = Rolle
accounts.contributors.col_granted = Freigegeben am
accounts.contributors.col_actions = Aktionen
accounts.contributors.locked_title = Das Konto ist gesperrt und kann sich nicht anmelden
accounts.contributors.locked = gesperrt
accounts.contributors.lector = Lektor
accounts.contributors.revoke_title = { $name } sieht das Werk danach nicht mehr
accounts.contributors.revoke = Entziehen
accounts.contributors.nobody = Noch für niemanden freigegeben.
accounts.contributors.grant_title = Freigeben
accounts.contributors.grant = Freigeben
accounts.contributors.no_candidates = Kein weiteres Lektor-Konto ohne Freigabe. Lektoren legt der Autor unter
accounts.contributors.no_candidates_end = an.
