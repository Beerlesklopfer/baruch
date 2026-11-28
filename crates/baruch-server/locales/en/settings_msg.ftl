# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of the settings pages (src/settings/**): page names, storage notes, notices after saving, form
# errors and refusals of settings.* (Round 39a, task messages_2).

# Pages of the settings (navigation, title, storage note).
settings_msg.page.display = Display
settings_msg.page.work = Work
settings_msg.page.assistants = Assistants
settings_msg.page.connections = Connections
settings_msg.page.sources = Sources
settings_msg.page.access = MCP access
settings_msg.page.data = Data
settings_msg.page.accounts = Accounts
settings_msg.storage.display = Cookie in this browser
settings_msg.storage.work = SQLite, Git mirror of the work
settings_msg.storage.all_works = SQLite (all works)
settings_msg.storage.connections = SQLite, secrets in the keyring
settings_msg.storage.access = SQLite (only the hash of the tokens); clients at run time
settings_msg.storage.data = Data folder
settings_msg.storage.accounts = SQLite of this instance (password only as a check value; not in the exchange, not in the mirror)

# Frame, errors of the pages, settings.* over /rpc.
settings_msg.frame.unknown_work = unknown work { $work }
settings_msg.frame.unknown_page = unknown settings page “{ $slug }”
settings_msg.frame.error = Error: { $detail }
settings_msg.rpc.page_missing = page missing (e.g. “darstellung”)
settings_msg.rpc.work_id = work: work id (number) expected
settings_msg.rpc.query_text = query.{ $key }: text expected
settings_msg.rpc.query_object = query: object expected
settings_msg.rpc.read_connections = Connections and sources are read through connection.list or sources.list.
settings_msg.rpc.values_object = values: object with the fields of the page expected
settings_msg.rpc.display_cookies = Display lives in cookies of this browser: the form POST /einstellungen/darstellung/act sets the cookies.
settings_msg.rpc.write_connections = Connections and sources are written through connection.* or sources.*.
settings_msg.rpc.nothing_to_save = The page “{ $page }” has nothing to save.
settings_msg.rpc.backup_confirm = Please confirm the backup (confirm: true).

# Display.
settings_msg.display.saved = Saved – applies in this browser.
settings_msg.display.theme = Theme: choose “System”, “Light” or “Dark”.
settings_msg.display.editor = Editor: choose “Typeset view” or “Source”.
settings_msg.display.proposals = Proposals: choose “expanded” or “collapsed”.
settings_msg.display.accordion = Right area: choose “Assistants”, “Revisions” or “Section chat”.

# Assistants.
settings_msg.assistants.color = The colour is saved.
settings_msg.assistants.persona = The persona is saved.
settings_msg.assistants.lector = The choice of the lector is saved.
settings_msg.assistants.prompt = The role prompt is saved.
settings_msg.assistants.prompt_default = The role prompt follows the default again.
settings_msg.assistants.rag = The setting on the hits of the meaning search is saved.
settings_msg.assistants.websearch = The setting on the web search of @belege is saved.
settings_msg.assistants.connection = Connection and release of the persona are saved.
settings_msg.assistants.bible = The setting on the Bible evidence from SWORD is saved.
settings_msg.assistants.chat = The scope of the chat is saved.
settings_msg.assistants.cached_list = List from the cache (as of { $stamp }).
settings_msg.assistants.color_format = Give the colour as #rrggbb.

# MCP access.
settings_msg.access.revoke_confirm = Please confirm revoking (tick the box).
settings_msg.access.which_token = Which token? (id missing)

# Data and backup.
settings_msg.data.open_permissions = The data folder is also accessible to other users of this computer (permissions { $mode }). Baruch does not change the permissions of an existing folder; “chmod 700” on the folder lets only the owner in.
settings_msg.data.unreadable = { $path } cannot be read: { $detail }
settings_msg.data.not_created = { $path } cannot be created: { $detail }
settings_msg.data.not_copied = { $path } cannot be copied: { $detail }
settings_msg.data.no_room = Too little space for the backup: it needs about { $needed }, { $free } are free, and { $reserve } should stay free afterwards (for the running database and the mirrors); nothing created.
settings_msg.data.folder_missing = Data folder { $path } not found: { $detail }
settings_msg.data.no_parent = The data folder { $path } has no parent folder for the backup.
settings_msg.data.time = Time of the backup: { $detail }
settings_msg.data.space_aborted = Checking the space aborted: { $detail }
settings_msg.data.too_many = There are already { $count } backups next to the data folder ({ $name }.sicherung-…, at most { $max }); please remove or move old ones. Nothing created.
settings_msg.data.exists = The backup { $path } already exists; nothing overwritten. Please try again in a second.
settings_msg.data.not_utf8 = { $path } is not a UTF-8 path
settings_msg.data.vacuum = Backing up the database (VACUUM INTO): { $detail }
settings_msg.data.copy_aborted = Copying aborted: { $detail }
settings_msg.data.backup_confirm = Please confirm the backup (tick the box).

# Work.
settings_msg.work.saved = The work is saved.
settings_msg.work.scheme = The outline scheme is applied.
settings_msg.work.book = The book details are saved.
settings_msg.work.cover = The cover is applied.
settings_msg.work.imprint = The imprint template is applied to the work template (new revision of the template).
settings_msg.work.layout = The cover layout is saved.
settings_msg.work.upload_on = Uploading to MultiGPT is switched on; it happens at the next synchronisation (Tools → Synchronise).
settings_msg.work.upload_off = Uploading to MultiGPT is switched off.
settings_msg.work.typeset = The typesetting settings are saved.
settings_msg.work.typeset_reset = The typesetting settings are reset; only the work template applies again.
settings_msg.work.typeset_template = The chapter opening is taken from the work template into the typesetting settings (new revision of the template).
settings_msg.work.commentaries = Commentaries and lexicons of the work are saved.
settings_msg.work.timing = The sermon time is saved.
settings_msg.work.language = The work language is saved.
settings_msg.work.sheet = The typeset view settings are saved.
settings_msg.work.revisions = The setting “Revisions” is saved.
settings_msg.work.folded = Older revisions are hidden (nothing deleted).
settings_msg.work.unfolded = All revisions are shown again.
settings_msg.work.module_missing = found in no module folder
settings_msg.work.commentaries_error = The selection could not be saved: a module is missing or not readable; nothing changed.
settings_msg.work.timing_error = The sermon time could not be saved: words per minute 60 to 250, target time 1 to 240 minutes (or empty); nothing changed.
settings_msg.work.language_no_work = The work language could not be saved: this work does not exist; nothing changed.
settings_msg.work.language_error = The work language could not be saved: German and English are allowed; nothing changed.
settings_msg.work.revisions_error = The setting “Revisions” could not be saved: largest change { $lo } to { $hi } percent, new revision after { $min_lo } to { $min_hi } minutes (0 = no time limit); nothing changed.
settings_msg.work.fold_error = Older revisions could not be hidden: give the day as a date; nothing changed.
settings_msg.work.sheet_error = The typeset view settings could not be saved: font at most 200 characters (names separated by commas, without { $chars }), font size 6 to 48 pt, text width 6 to 60 cm (unit pt, mm, cm or em), colours as #rrggbb; nothing changed.
settings_msg.work.no_work = There is no work.

# Typesetting settings of the work.
settings_msg.typeset.unknown_choice = { $field }: unknown choice “{ $raw }”
settings_msg.typeset.margins = Margins: give all four margins or leave all empty
settings_msg.typeset.settings_missing = settings missing
settings_msg.typeset.register_gone = Index “{ $slug }” (no longer exists)
settings_msg.typeset.unknown_work = Unknown work.
settings_msg.typeset.reset_confirm = Please confirm the reset with the tick; nothing changed.
settings_msg.typeset.unknown_action = Unknown action (save, preview or reset).
settings_msg.typeset.apply_confirm = Please confirm applying with the tick; the work template stays unchanged.
settings_msg.typeset.preview_gone = The preview does not exist (any more); please press “Preview” again.
