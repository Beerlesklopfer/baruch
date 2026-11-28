# SPDX-License-Identifier: AGPL-3.0-or-later
# Accounts (sign in, set up, my account, contributors) (Round 39a, templates_3). Base catalog in English; locales/de/accounts.ftl has the same keys in the same order.

# templates/anmelden.html
accounts.sign_in.window_title = Sign in – Baruch
accounts.sign_in.heading = Baruch – Sign in
accounts.sign_in.too_many =
    Too many failed attempts. Please wait { $count ->
        [one] { $count } second
       *[other] { $count } seconds
    } and then try again.
accounts.sign_in.signed_in = Signed in as { $signed_in }.
accounts.sign_in.continue = Continue
accounts.sign_in.name = Name
accounts.sign_in.password = Password
accounts.sign_in.submit = Sign in
accounts.sign_in.already = Already signed in?
accounts.sign_in.already_help = – after a link from outside (e-mail, another page) Baruch asks once; the click here is then enough.
accounts.sign_in.forgotten_html = Forgot the password? The author sets a new one under Settings → Accounts. If the author has forgotten their own, the command <code>{ $password_cmd }</code> helps on the computer where Baruch runs.
accounts.cmd_name = name

# templates/einrichten.html
accounts.setup.window_title = Set up accounts – Baruch
accounts.setup.heading = Baruch – Set up accounts
accounts.setup.first_html = The first account belongs to the <strong>author</strong>. As soon as it exists, Baruch requires a sign-in – on this computer too. Afterwards lectors can be added under Settings → Accounts and given access per work.
accounts.setup.without = Without accounts Baruch stays open as before. Whoever writes only alone on the own computer needs no accounts.
accounts.setup.name = Name (for signing in)
accounts.setup.display = Display name (optional)
accounts.setup.password = Password ({ $min } to { $max } characters)
accounts.setup.password_repeat = Repeat password
accounts.setup.create = Create account
accounts.setup.hash_html = Baruch stores the password only as a check value (Argon2id), never in plain text. If it gets lost, the command <code>{ $password_cmd }</code> sets a new one on the computer where Baruch runs.
accounts.setup.local_only = The first account can only be set up on the own computer.
accounts.setup.remote = This request came from another computer or through an upstream server (reverse proxy). From there Baruch creates no author account – otherwise whoever reaches the page first could take over the writing workshop.
accounts.setup.how_html = Setting up works on the computer where Baruch runs: open Baruch directly in the browser there (for example <code>http://127.0.0.1:8765/einrichten</code>, not through the name of the proxy).

# templates/konto.html
accounts.mine.window_title = My account – Baruch
accounts.mine.heading = My account
accounts.mine.back_group = Back
accounts.mine.back_to_work = Back to the work
accounts.mine.open = Baruch runs without accounts: everything is open as before, there is no sign-in.
accounts.mine.why = Only someone who wants lectors to read along or wants to make Baruch reachable through a proxy needs accounts. The first account belongs to the author.
accounts.mine.set_up = Set up accounts…
accounts.mine.local_html = You work as the <strong>author</strong> on the own computer, without sign-in (Settings → Accounts: “On the own computer without sign-in”).
accounts.mine.sign_in = Sign in
accounts.mine.settings_accounts = Settings → Accounts
accounts.mine.display = Display name
accounts.mine.role = Role
accounts.mine.origin = Origin
accounts.mine.origin_help_html = <code>{ $handle }</code> – your proposals, posts and findings appear under this name in the work and in the Git mirror.
accounts.mine.granted_works = Works shared with you
accounts.mine.works = Works
accounts.mine.none_granted = No work shared yet. The author shares works one by one (page “Contributors” of the work).
accounts.mine.no_work = No work yet.
accounts.mine.lector_help = As a lector you read the shared works and write proposals for versions, posts in the section chat and findings. Only the author can accept, change and delete.
accounts.mine.change_password = Change password
accounts.mine.old_password = Current password
accounts.mine.new_password = New password ({ $min } to { $max } characters)
accounts.mine.repeat_password = Repeat new password
accounts.mine.other_sessions = Afterwards all other sessions of this account are signed out.
accounts.mine.sign_out = Sign out

# templates/mitwirkende.html
accounts.contributors.window_title = Contributors · { $title } – Baruch
accounts.contributors.page_title = Contributors · { $title }
accounts.contributors.back_group = Back
accounts.contributors.back_to_work = Back to the work
accounts.contributors.accounts_title = Create and lock accounts, reset passwords
accounts.contributors.settings_accounts = Settings → Accounts
accounts.contributors.heading = Contributors to “{ $title }”
accounts.contributors.no_accounts = Baruch runs without accounts – there is nobody the work could be shared with.
accounts.contributors.set_up = Set up accounts…
accounts.contributors.load_error = The shares cannot be read: { $load_error }
accounts.contributors.help = The author sees and edits every work. A lector sees only shared works and writes proposals, posts and findings there under their origin. A withdrawn share takes effect with the next request; what the lector has written stays in the work.
accounts.contributors.granted_to = Shared with
accounts.contributors.col_display = Display name
accounts.contributors.col_origin = Origin
accounts.contributors.col_role = Role
accounts.contributors.col_granted = Shared on
accounts.contributors.col_actions = Actions
accounts.contributors.locked_title = The account is locked and cannot sign in
accounts.contributors.locked = locked
accounts.contributors.lector = Lector
accounts.contributors.revoke_title = { $name } no longer sees the work afterwards
accounts.contributors.revoke = Withdraw
accounts.contributors.nobody = Not shared with anybody yet.
accounts.contributors.grant_title = Share
accounts.contributors.grant = Share
accounts.contributors.no_candidates = No further lector account without a share. The author adds lectors under
accounts.contributors.no_candidates_end = .
