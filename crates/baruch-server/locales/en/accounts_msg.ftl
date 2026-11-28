# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of sign-in, accounts and roles (src/auth/**, src/konten_ui/**, src/roles/**): refusals of the gate,
# message codes ?ok=/?fehler= of the account pages, account menu, RPC refusals (Round 39a, task messages_2).

# Gate and sign-in (src/auth).
accounts_msg.gate.login_required = Sign-in required.
accounts_msg.gate.refused = Refused: { $detail }
accounts_msg.login.failed = Name or password is wrong.
accounts_msg.login.blocked = Too many failed attempts. Please wait and then try again.
accounts_msg.login.error = Error: { $detail }
accounts_msg.login.author_only = Refused: only the author may do this.
accounts_msg.login.random = Random numbers of the system: { $detail }
accounts_msg.login.hash_aborted = Password hash aborted

# Sentences of account.* over /rpc (src/auth/rpc.rs).
accounts_msg.rpc.password_wrong = The previous password is wrong.
accounts_msg.rpc.password_short = The password needs at least 10 characters.
accounts_msg.rpc.password_long = The password may have at most 200 characters.
accounts_msg.rpc.password_differ = The two passwords differ.
accounts_msg.rpc.name_taken = This name already exists.
accounts_msg.rpc.name_invalid = Invalid name: 1 to 40 characters with at least one letter a–z or one digit, not “unbekannt”; display name at most 80 characters.
accounts_msg.rpc.last_author = The last active author account cannot be locked.
accounts_msg.rpc.unknown_account = This account does not exist.
accounts_msg.rpc.needs_author = There is no account yet. The first account (the author) is created on the own computer under /einrichten.
accounts_msg.rpc.failed = That did not work.
accounts_msg.rpc.current_missing = account.set_password: current missing
accounts_msg.rpc.no_own_account = Without sign-in there is no own account; set the password of an account with id.
accounts_msg.rpc.no_sessions = Without sign-in there are no sessions to end.

# Message codes of ?ok=… and ?fehler=… (src/konten_ui/mod.rs, contract R35 §3.4).
accounts_msg.code.konto_angelegt = The account is created.
accounts_msg.code.passwort_geaendert = The password is changed. Other sessions of this account are signed out.
accounts_msg.code.konto_gesperrt = The account is locked; its sessions are ended.
accounts_msg.code.konto_entsperrt = The account is active again.
accounts_msg.code.lokal_offen = On the own computer no sign-in applies any more (author rights without a password).
accounts_msg.code.lokal_anmelden = Sign-in is now needed on the own computer too.
accounts_msg.code.freigegeben = The work is released for the account.
accounts_msg.code.entzogen = The release is withdrawn.
accounts_msg.code.abgemeldet = Signed out.
accounts_msg.code.passwort_falsch = The previous password is wrong.
accounts_msg.code.passwort_kurz = The password is too short (at least 10 characters).
accounts_msg.code.passwort_lang = The password is too long (at most 200 characters).
accounts_msg.code.passwort_ungleich = The two passwords do not match.
accounts_msg.code.name_vergeben = This name already exists (or one that would give the same short name).
accounts_msg.code.name_ungueltig = The name is invalid (1 to 40 characters, not “unbekannt”).
accounts_msg.code.letzter_autor = The last active author account cannot be locked.
accounts_msg.code.unbekanntes_konto = This account does not exist.
accounts_msg.code.kein_lektor = A work can only be released for a lektor account.
accounts_msg.code.nicht_eigener_rechner = That only works on the own computer (Baruch directly in the browser of this computer, not through a proxy).

# Account pages and the entry in the menu “Baruch ▾” (src/konten_ui).
accounts_msg.page.error = Error: { $detail }
accounts_msg.page.template = Template { $name }: { $detail }
accounts_msg.page.works_unreadable = The works cannot be read: { $detail }
accounts_msg.page.unexpected_members = unexpected answer from member.list
accounts_msg.page.members_author_only = Refused: only the author manages releases.
accounts_msg.page.unknown_work = unknown work
accounts_msg.menu.setup_title = Create an account for the author; afterwards Baruch asks for a sign-in, and lektors can collaborate
accounts_msg.menu.setup = Set up accounts…
accounts_msg.menu.author = Author
accounts_msg.menu.lektor = Lektor
accounts_msg.menu.signed_in = Signed in: { $name } ({ $role })
accounts_msg.menu.own_title = Name, role, released works, change password
accounts_msg.menu.own = My account
accounts_msg.menu.sign_out = Sign out
accounts_msg.menu.local_author = Author (own computer, without sign-in)
accounts_msg.menu.sign_in = Sign in

# Roles (src/roles).
accounts_msg.roles.author_only = Only the author may do this.
accounts_msg.roles.no_work = No work released yet.
accounts_msg.roles.grants_unreadable = Releases not readable.
accounts_msg.roles.address_unreadable = Address not readable.
accounts_msg.roles.work_id = Work id (number) expected
accounts_msg.roles.work_param = work: work id (number) expected
accounts_msg.roles.params_object = Parameters expected as an object
accounts_msg.roles.form_error = Error: { $detail }
accounts_msg.roles.number_expected = { $name }: number expected
accounts_msg.roles.missing = { $name } missing ({ $what })
accounts_msg.roles.what_account = account
accounts_msg.roles.what_work = work
accounts_msg.roles.role_lektor = role: only “lektor” can be granted
accounts_msg.roles.unknown_account = Unknown account { $account }.
accounts_msg.roles.lektor_only = Only lektor accounts get a release; the author is author in every work.
accounts_msg.lektor.propose_way = revision.propose is the way of the lektors: the author saves his revision with revision.save or outline.save.
accounts_msg.lektor.empty = Empty text: a proposal needs the complete text of the section.
accounts_msg.lektor.too_big = The proposal may have at most { $max } bytes.
accounts_msg.lektor.note_long = note may have at most { $max } characters.
accounts_msg.lektor.same_text = The text equals the current revision; no proposal created.
accounts_msg.lektor.text_expected = { $name }: text expected
accounts_msg.lektor.key = key: fixed key (number) expected
accounts_msg.lektor.finding_way = finding.add is the way of the lektors: the author records a remark as an idea or post and decides on findings with finding.set_status.
accounts_msg.lektor.kind_missing = kind missing (hint, critical or praise)
accounts_msg.lektor.kind_wrong = kind: hint, critical or praise expected, not “{ $kind }”
accounts_msg.lektor.body_missing = body missing (text of the finding)
