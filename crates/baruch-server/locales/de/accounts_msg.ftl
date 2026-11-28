# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of sign-in, accounts and roles (src/auth/**, src/konten_ui/**, src/roles/**): refusals of the gate,
# message codes ?ok=/?fehler= of the account pages, account menu, RPC refusals (Round 39a, task messages_2).

# Gate and sign-in (src/auth).
accounts_msg.gate.login_required = Anmeldung erforderlich.
accounts_msg.gate.refused = Abgelehnt: { $detail }
accounts_msg.login.failed = Name oder Passwort stimmt nicht.
accounts_msg.login.blocked = Zu viele Fehlversuche. Bitte warten und dann erneut versuchen.
accounts_msg.login.error = Fehler: { $detail }
accounts_msg.login.author_only = Abgelehnt: Das darf nur der Autor.
accounts_msg.login.random = Zufallszahlen des Systems: { $detail }
accounts_msg.login.hash_aborted = Passwort-Hash abgebrochen

# Sentences of account.* over /rpc (src/auth/rpc.rs).
accounts_msg.rpc.password_wrong = Das bisherige Passwort stimmt nicht.
accounts_msg.rpc.password_short = Das Passwort braucht mindestens 10 Zeichen.
accounts_msg.rpc.password_long = Das Passwort darf höchstens 200 Zeichen haben.
accounts_msg.rpc.password_differ = Die beiden Passwörter sind verschieden.
accounts_msg.rpc.name_taken = Diesen Namen gibt es schon.
accounts_msg.rpc.name_invalid = Ungültiger Name: 1 bis 40 Zeichen mit mindestens einem Buchstaben a–z oder einer Ziffer, nicht „unbekannt“; Anzeigename höchstens 80 Zeichen.
accounts_msg.rpc.last_author = Das letzte aktive Autor-Konto lässt sich nicht sperren.
accounts_msg.rpc.unknown_account = Dieses Konto gibt es nicht.
accounts_msg.rpc.needs_author = Es gibt noch kein Konto. Das erste Konto (der Autor) wird am eigenen Rechner unter /einrichten angelegt.
accounts_msg.rpc.failed = Das ging nicht.
accounts_msg.rpc.current_missing = account.set_password: current fehlt
accounts_msg.rpc.no_own_account = Ohne Anmeldung gibt es kein eigenes Konto; mit id das Passwort eines Kontos setzen.
accounts_msg.rpc.no_sessions = Ohne Anmeldung gibt es keine Sitzungen zu beenden.

# Message codes of ?ok=… and ?fehler=… (src/konten_ui/mod.rs, contract R35 §3.4).
accounts_msg.code.konto_angelegt = Das Konto ist angelegt.
accounts_msg.code.passwort_geaendert = Das Passwort ist geändert. Andere Sitzungen dieses Kontos sind abgemeldet.
accounts_msg.code.konto_gesperrt = Das Konto ist gesperrt; seine Sitzungen sind beendet.
accounts_msg.code.konto_entsperrt = Das Konto ist wieder aktiv.
accounts_msg.code.lokal_offen = Am eigenen Rechner gilt jetzt keine Anmeldung mehr (Autorrechte ohne Passwort).
accounts_msg.code.lokal_anmelden = Auch am eigenen Rechner ist jetzt eine Anmeldung nötig.
accounts_msg.code.freigegeben = Das Werk ist für das Konto freigegeben.
accounts_msg.code.entzogen = Die Freigabe ist entzogen.
accounts_msg.code.abgemeldet = Abgemeldet.
accounts_msg.code.passwort_falsch = Das bisherige Passwort stimmt nicht.
accounts_msg.code.passwort_kurz = Das Passwort ist zu kurz (mindestens 10 Zeichen).
accounts_msg.code.passwort_lang = Das Passwort ist zu lang (höchstens 200 Zeichen).
accounts_msg.code.passwort_ungleich = Die beiden Passwörter stimmen nicht überein.
accounts_msg.code.name_vergeben = Diesen Namen gibt es schon (oder einen, der dasselbe Kürzel ergäbe).
accounts_msg.code.name_ungueltig = Der Name ist ungültig (1 bis 40 Zeichen, nicht „unbekannt“).
accounts_msg.code.letzter_autor = Das letzte aktive Autor-Konto lässt sich nicht sperren.
accounts_msg.code.unbekanntes_konto = Dieses Konto gibt es nicht.
accounts_msg.code.kein_lektor = Freigeben lässt sich ein Werk nur für ein Lektor-Konto.
accounts_msg.code.nicht_eigener_rechner = Das geht nur am eigenen Rechner (Baruch direkt im Browser dieses Rechners, nicht über einen Proxy).

# Account pages and the entry in the menu “Baruch ▾” (src/konten_ui).
accounts_msg.page.error = Fehler: { $detail }
accounts_msg.page.template = Vorlage { $name }: { $detail }
accounts_msg.page.works_unreadable = Die Werke lassen sich nicht lesen: { $detail }
accounts_msg.page.unexpected_members = unerwartete Antwort von member.list
accounts_msg.page.members_author_only = Abgelehnt: Freigaben verwaltet nur der Autor.
accounts_msg.page.unknown_work = unbekanntes Werk
accounts_msg.menu.setup_title = Ein Konto für den Autor anlegen; danach verlangt Baruch eine Anmeldung, und Lektoren können mitarbeiten
accounts_msg.menu.setup = Konten einrichten…
accounts_msg.menu.author = Autor
accounts_msg.menu.lektor = Lektor
accounts_msg.menu.signed_in = Angemeldet: { $name } ({ $role })
accounts_msg.menu.own_title = Name, Rolle, freigegebene Werke, Passwort ändern
accounts_msg.menu.own = Mein Konto
accounts_msg.menu.sign_out = Abmelden
accounts_msg.menu.local_author = Autor (eigener Rechner, ohne Anmeldung)
accounts_msg.menu.sign_in = Anmelden

# Roles (src/roles).
accounts_msg.roles.author_only = Das darf nur der Autor.
accounts_msg.roles.no_work = Noch kein Werk freigegeben.
accounts_msg.roles.grants_unreadable = Freigaben nicht lesbar.
accounts_msg.roles.address_unreadable = Adresse nicht lesbar.
accounts_msg.roles.work_id = Werk-id (Zahl) erwartet
accounts_msg.roles.work_param = work: Werk-id (Zahl) erwartet
accounts_msg.roles.params_object = Parameter als Objekt erwartet
accounts_msg.roles.form_error = Fehler: { $detail }
accounts_msg.roles.number_expected = { $name }: Zahl erwartet
accounts_msg.roles.missing = { $name } fehlt ({ $what })
accounts_msg.roles.what_account = Konto
accounts_msg.roles.what_work = Werk
accounts_msg.roles.role_lektor = role: nur „lektor“ lässt sich freigeben
accounts_msg.roles.unknown_account = Unbekanntes Konto { $account }.
accounts_msg.roles.lektor_only = Nur Lektor-Konten bekommen eine Freigabe; der Autor ist Autor in jedem Werk.
accounts_msg.lektor.propose_way = revision.propose ist der Weg der Lektoren: Der Autor speichert seine Fassung mit revision.save bzw. outline.save.
accounts_msg.lektor.empty = Leerer Text: ein Vorschlag braucht den vollständigen Text des Abschnitts.
accounts_msg.lektor.too_big = Der Vorschlag darf höchstens { $max } Bytes haben.
accounts_msg.lektor.note_long = note darf höchstens { $max } Zeichen haben.
accounts_msg.lektor.same_text = Der Text gleicht der aktuellen Fassung; kein Vorschlag angelegt.
accounts_msg.lektor.text_expected = { $name }: Text erwartet
accounts_msg.lektor.key = key: fester Schlüssel (Zahl) erwartet
accounts_msg.lektor.finding_way = finding.add ist der Weg der Lektoren: Der Autor hält eine Anmerkung als Einfall oder Beitrag fest und entscheidet über Befunde mit finding.set_status.
accounts_msg.lektor.kind_missing = kind fehlt (hint, critical oder praise)
accounts_msg.lektor.kind_wrong = kind: hint, critical oder praise erwartet, nicht „{ $kind }“
accounts_msg.lektor.body_missing = body fehlt (Text des Befunds)
