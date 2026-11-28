# SPDX-License-Identifier: AGPL-3.0-or-later
# Settings pages (Round 39a, templates_3). Base catalog in English; locales/de/settings.ftl has the same keys in the same order.

# templates/settings/daten.html
settings.data.backup_saved_named_html = Backup created: <code>{ $saved }</code> (next to the data folder).
settings.data.backup_saved = Backup created (next to the data folder).
settings.data.title = Data
settings.data.dir = Data folder
settings.data.size = Size
settings.data.size_bytes = ({ $size_bytes } bytes, symbolic links not counted)
settings.data.about_link = About Baruch
settings.data.about_hint = (licence, source code, third-party libraries)
settings.data.instance_title = This instance
settings.data.instance_saved = Instance name saved: { $instance_label }.
settings.data.instance_uuid = Identifier (UUID)
settings.data.instance_help = The name travels along as the origin when instances exchange works (e.g. “Study” or “Laptop”). At most { $max_instance_name } characters, no line breaks; leaving it empty resets it.
settings.data.instance_name = Instance name
settings.data.instance_save = Save name
settings.data.epubcheck_saved = EPUBCheck setting saved.
settings.data.epubcheck_found = Found
settings.data.epubcheck_not_found = EPUBCheck not found
settings.data.java_found = java is on the PATH
settings.data.java_missing = java not on the PATH (needed for an epubcheck.jar)
settings.data.jar_missing_html = The configured epubcheck.jar no longer exists: <code>{ $jar }</code>
settings.data.epubcheck_help_html = EPUBCheck checks an EPUB against the rules of the W3C community. Baruch takes “epubcheck” from the PATH, otherwise an epubcheck.jar at the path given here (with <code>java -jar</code>), and never downloads anything. Absolute path ending in .jar; leaving it empty removes the setting.
settings.data.jar_path = Path to epubcheck.jar
settings.data.jar_save = Save path
settings.data.mirrors_title = Git mirror per work
settings.data.col_work = Work
settings.data.col_dir = Folder
settings.data.col_commit = last commit
settings.data.no_commit = none yet
settings.data.no_works = There is no work yet.
settings.data.backup_title = Backup
settings.data.backup_help_html = Copies the whole data folder (database as a consistent state, mirrors, ideas) into a new folder <code>{ $backup_folder }</code> next to the data folder. Nothing is overwritten.
settings.data.backup_name = name
settings.data.backup_stamp = date-time
settings.data.backup_confirm = Create backup now
settings.data.backup_now = Back up now

# templates/settings.html
settings.page.window_title = Settings · { $title } – Baruch
settings.page.heading = Settings
settings.page.back_group = Back
settings.page.back_to_work = Back to the work
settings.page.works = Works

# templates/settings/frame.html
settings.frame.nav_label = Settings
settings.frame.storage = Saved: { $storage }

# templates/settings/darstellung.html
settings.display.title = Display
settings.display.theme = Theme
settings.display.theme_system = Like the system
settings.display.theme_light = Light
settings.display.theme_dark = Dark
settings.display.editor = Editor when opening
settings.display.editor_satz = Typeset view
settings.display.editor_source = Source (Typst)
settings.display.pv = Proposals in the reading flow at first
settings.display.pv_open = expanded
settings.display.pv_closed = collapsed (except the one shown)
settings.display.accordion = Open area on the right
settings.display.accordion_personas = Assistants
settings.display.accordion_revisions = Revisions
settings.display.accordion_chat = Section chat
settings.display.widths = Column widths: left { $left }, right { $right }.
settings.display.width_default = default
settings.display.reset_widths = Reset column widths
settings.display.cookies_html = These values are stored only in this browser (cookies <code>baruch_theme</code>, <code>baruch_editor</code>, <code>baruch_pv</code>, <code>baruch_accordion</code>, <code>baruch_left</code>/<code>baruch_right</code>); another browser has its own.

# templates/settings/assistenten.html
settings.assistants.title = Assistants
settings.assistants.lector_label = Lector:
settings.assistants.lector_tokens = { $handle } · effective { $tokens } tokens
settings.assistants.lector_setting = (setting { $context_length })
settings.assistants.lector_reported = , remote side reports { $reported }
settings.assistants.lector_none = none (no persona is active)
settings.assistants.lector_fixed = chosen explicitly
settings.assistants.lector_fallback = ({ $lector_fixed } is not active, so the largest context applies)
settings.assistants.lector_auto = automatically the active persona with the largest context
settings.assistants.lector_legend = Lector
settings.assistants.trial_title = Trial since 2026-11-06: all personas on one model
settings.assistants.trial_badge = Trial: fixed @lektor
settings.assistants.mode_fixed = fixed
settings.assistants.fixed_select = fixed persona as lector
settings.assistants.inactive_suffix = – inactive
settings.assistants.mode_auto = automatically by context (active persona with the largest context)
settings.assistants.chat_scope = Section chat: what the personas get to see
settings.assistants.scope_chapter = the whole chapter (for a sermon the whole sermon; default)
settings.assistants.scope_section = Chat only the section (plus title and outline of the work)
settings.assistants.models_pending = Model list not fetched yet: Baruch asks no remote side when the page loads.
settings.assistants.models_fetch = Fetch model list
settings.assistants.models_free = . Model names can be entered freely.
settings.assistants.models_unavailable = Model list not available: { $models_note } Model names can be entered freely.
settings.assistants.mixed_models = The active personas use different models ({ $mixed_models }). Every model needs its own graphics memory (VRAM); switching reloads the model.
settings.assistants.unknown_persona = Persona “{ $unknown_persona }” does not exist; showing the first one.
settings.assistants.lector_badge = Lector
settings.assistants.largest_context = largest context
settings.assistants.inactive = (inactive)
settings.assistants.own_color = own colour
settings.assistants.default = default
settings.assistants.color_prompt = { $color } · role prompt
settings.assistants.prompt_own = own
settings.assistants.model_unknown = Model “{ $model }” is unknown to Eliezer.
settings.assistants.model_alias = Old name: alias → { $model_target }
settings.assistants.reset_prompt_question = Discard the own role prompt of { $name } and use the default again?
settings.assistants.reset_prompt_yes = Yes, restore the default
settings.assistants.handle = Handle
settings.assistants.handle_title = The handle (@handle) is fixed; chat and MCP address the persona through it.
settings.assistants.color = Colour
settings.assistants.color_text = or as text
settings.assistants.model = Model
settings.assistants.context = Context (tokens)
settings.assistants.active = active
settings.assistants.role = Role
settings.assistants.prompt = Role prompt
settings.assistants.prompt_custom = (own)
settings.assistants.prompt_default = (default from the program; saving without a change keeps the default)
settings.assistants.prompt_label = Role prompt of { $handle }
settings.assistants.palette_already = The palette colour already applies.
settings.assistants.palette_title = Colour back to the palette default
settings.assistants.palette = Palette colour
settings.assistants.default_already = The default already applies.
settings.assistants.asks_first = asks first
settings.assistants.restore_default = Restore default
settings.assistants.backend = Model connection
settings.assistants.connection = Connection
settings.assistants.external_allowed = Send book text to an external connection (clearance for { $handle })
settings.assistants.apply = Apply
settings.assistants.backend_blocked = “{ $connection_name }” is external and not cleared for { $handle }: requests to { $name } are refused, nothing leaves.
settings.assistants.context_tokens = Effective context: { $context_tokens } tokens
settings.assistants.context_reported = (remote side reports { $context_reported })
settings.assistants.context_unreported = (no report from the remote side; the setting applies)
settings.assistants.backend_help = Connections are set up under Settings → Connections → Model connections. “external” leaves this computer or the own network; without clearance Baruch sends no book text there (default: no clearance).
settings.assistants.bible = Bible evidence (SWORD)
settings.assistants.bible_on = Pass along Bible evidence from SWORD (wording of the work's Bible, chosen commentaries and lexicons)
settings.assistants.bible_help = The wording, commentaries and lexicon articles for the Bible references in the text go along as data (local, from the SWORD modules; selection per work under Settings → Work), numbered [S1] …; the persona does not follow instructions in them. Only for @belege (default on) and the lector (default off)
settings.assistants.bible_default_on = ; here: default on
settings.assistants.rag = Semantic search
settings.assistants.rag_on = Pass along hits of the semantic search as evidence
settings.assistants.rag_help = Up to five hits from the paths of the work (Settings → Connections) go along as data, numbered [R1] … [R5]; { $name } may name only these as a source. The persona does not follow instructions in the hits. Only for @belege and the lector.
settings.assistants.websearch = Web search
settings.assistants.websearch_on_html = research on the web (through MultiGPT: <code>web_search</code> and <code>fetch_url</code>)
settings.assistants.websearch_unconfigured = MultiGPT is not set up (Settings → Connections); without an address { $name } does not search the web.
settings.assistants.websearch_help = First the preferred sources (Settings → Sources, the first three through “site:”), then the general web; at most eight hits, four of them fetched pages, at most 30 s altogether. { $name } names only fetched pages as [W1] … [W4]. Baruch checks every quotation against the fetched text, marks “(not verified)” and writes the source line with the retrieval date itself. Verified quotations can be taken into the library on the evidence page. Default: off.
settings.assistants.websearch_leaves = What leaves the house: the search query is the question or, when checking, the beginning of the section (at most 200 characters of unpublished book text); it goes through MultiGPT to the search engine.
settings.assistants.colors_help = Every persona appears in its colour: in the list of assistants, in the section chat, in the versions and with its proposals in the book text. Without an own choice the palette default applies (in the order of the personas). Backgrounds are lightened or darkened so that the text stays readable in both themes.
settings.assistants.no_personas = There are no personas yet.

# templates/settings/konten.html
settings.accounts.title = Accounts
settings.accounts.open_html = Baruch runs <strong>without accounts</strong>: everything is open as before, there is no sign-in. That is enough as long as only you write on your own computer.
settings.accounts.why_html = Accounts are needed by anyone who wants lectors to collaborate or wants to make Baruch reachable through a proxy. The first account belongs to the author and can only be created on the own computer. As soon as it exists, pages, forms and <code>/rpc</code> require a sign-in – on this computer too (this can be switched off here again afterwards). <code>/mcp</code> stays with its tokens.
settings.accounts.set_up = Set up
settings.accounts.author_only = Only the author manages accounts.
settings.accounts.load_error = The accounts cannot be read: { $load_error }
settings.accounts.col_display = Display name
settings.accounts.col_kind = Kind
settings.accounts.col_origin = Origin
settings.accounts.col_active = Active
settings.accounts.col_last_login = Last sign-in
settings.accounts.col_actions = Actions
settings.accounts.you_title = You are signed in with this account
settings.accounts.you = you
settings.accounts.author_title = The author writes as “Author”
settings.accounts.author = Author
settings.accounts.yes = yes
settings.accounts.locked = locked
settings.accounts.never = never
settings.accounts.lock_title = { $name } can no longer sign in afterwards; open sessions end at once. What the account has written stays.
settings.accounts.lock = Lock
settings.accounts.unlock = Unlock
settings.accounts.reset_password = Reset password …
settings.accounts.new_password = New password for { $name } ({ $min } to { $max } characters)
settings.accounts.repeat = Repeat
settings.accounts.sessions_end = All sessions of this account end
settings.accounts.set_password = Set password
settings.accounts.never_deleted_html = Accounts are never deleted, only locked: the origin (<code>@lektorat:…</code>) stays taken so that old proposals, posts and findings stay unambiguous. The last active author account cannot be locked.
settings.accounts.add_lector = Add lector
settings.accounts.name_hint = Name (for signing in, at most 40 characters)
settings.accounts.display_hint = Display name (optional)
settings.accounts.password_hint = Password ({ $min } to { $max } characters)
settings.accounts.password_repeat = Repeat password
settings.accounts.new_is_lector = A new account is always a lector and sees no work at first. Access is granted per work on the page “Contributors”
settings.accounts.contributors_link = Contributors of this work
settings.accounts.contributors_tab = (tab Book → Contributors)
settings.accounts.pass_on = . You pass the password on to the lector yourself; they can change it under “My account”.
settings.accounts.local_title = On the own computer without sign-in
settings.accounts.state = State:
settings.accounts.local_on = switched on – on the own computer author rights apply without a password
settings.accounts.local_off = switched off – a sign-in is needed on the own computer too (default)
settings.accounts.proxy_warning_html = <strong>Careful with a proxy on the same computer:</strong> a reverse proxy also connects through 127.0.0.1. Baruch recognises it by its host name (<code>--allow-host</code>) and by the headers <code>Forwarded</code>, <code>X-Forwarded-*</code>, <code>X-Real-IP</code> and <code>Via</code>. A proxy that rewrites the <code>Host</code> and sets none of these headers would look like the own computer – then every visitor would have author rights. Only switch on if no such proxy stands in front of Baruch.
settings.accounts.local_sign_in = Sign in on the own computer too
settings.accounts.forgotten_html = Forgot the password? On the computer where Baruch runs: <code>{ $cmd }</code> reads a new password from the input and ends all sessions of the account.
settings.accounts.cmd_name = name

# templates/settings/mcp.html
settings.mcp.revoked = Token revoked. It is no longer valid from now on.
settings.mcp.title = MCP access
settings.mcp.address = Address
settings.mcp.address_label = Address of the MCP access
settings.mcp.address_hint_html = (address of this server + <code>/mcp</code>)
settings.mcp.proxied_html = You reach Baruch through a reverse proxy (allowed host name, <code>--allow-host</code>). Under this name <code>/mcp</code> accepts requests <strong>only with a token</strong>, from the own computer too. Create a token below, then in Claude Code (the token instead of <code>{ $ph_token }</code>):
settings.mcp.proxy_auth_html = If the proxy protects access with its own sign-in (such as Basic-Auth), it must exempt <code>/mcp</code> from it or pass the header <code>Authorization</code> through unchanged – otherwise the token does not arrive. Directly on this computer (<code>http://127.0.0.1:…/mcp</code>) access stays without a token.
settings.mcp.loopback = Reachable without a token only through loopback (127.0.0.1 or ::1). In Claude Code:
settings.mcp.fresh_html = <strong>New token “{ $name }”</strong> – visible only now. Baruch stores only its hash; whoever loses it creates a new one.
settings.mcp.fresh_label = New token (visible only now)
settings.mcp.fresh_proxied_html = In Claude Code (the token instead of <code>{ $ph_token }</code>):
settings.mcp.fresh_remote_html = On the other computer (the network address of this computer instead of <code>{ $ph_ip }</code>, the token instead of <code>{ $ph_token }</code>):
settings.mcp.fresh_handle_html = Posts and proposals of this access appear under <code>{ $handle }</code>.
settings.mcp.tokens_title = Tokens for other computers
settings.mcp.tokens_help_html = With a token <code>/mcp</code> accepts requests from any computer (<code>{ $bearer }</code>); all other pages stay reserved for the own computer. For this Baruch must listen on a network address (<code>--listen 192.168.…:8765</code>), and the other computer addresses it by its IP address, not by a name. <strong>Caution:</strong> Baruch speaks only HTTP without encryption – token and texts travel through the network in plain text. Use only in your own trusted network. Encrypted and under a host name it works only through a reverse proxy with TLS (<code>--allow-host</code>, <code>docs/Betrieb.md</code>).
settings.mcp.col_prefix = Start
settings.mcp.col_author = Author
settings.mcp.col_created = created
settings.mcp.col_last_used = last used
settings.mcp.col_state = State
settings.mcp.legacy_html = (formerly <code>{ $legacy }</code>)
settings.mcp.taken_html = (keeps the author from before Round 23: <code>{ $taken }</code> belongs to another token)
settings.mcp.never = never
settings.mcp.revoked_at = revoked { $revoked }
settings.mcp.really = really
settings.mcp.revoke = Revoke
settings.mcp.no_tokens = No token created yet.
settings.mcp.name_hint = Name (e.g. of the computer)
settings.mcp.create = Create token
settings.mcp.name_rules_html = The name becomes the author <code>{ $ph_mcp_name }</code>: lower case, umlauts transcribed (ü → ue), special characters as “-”, at most 32 characters. A name that would give the same author as an existing or revoked token, or that would begin with <code>lokal-</code>, is refused. A client without a token whose name belongs to a token writes as <code>{ $ph_mcp_local }</code>.
settings.mcp.clients_title = Clients connected since the start
settings.mcp.first_seen = first seen
settings.mcp.last_seen = last seen
settings.mcp.no_clients_html = No client has signed in since the start (MCP <code>initialize</code>).

# templates/settings/quellen.html
settings.sources.intro_html = Preferred sources for <code>@belege</code>: title, address, note, in this order. The list applies to all works; assistants read it through MCP (<code>sources_list</code>), only the human can change it.
settings.sources.up = “{ $title }” up
settings.sources.down = “{ $title }” down
settings.sources.edit = Edit
settings.sources.title_field = Title
settings.sources.address = Address
settings.sources.note = Note
settings.sources.really_delete = really delete
settings.sources.none = No sources yet.
settings.sources.new = New source
settings.sources.create = Create
settings.sources.bibles = Bibles (SWORD)
settings.sources.bibles_help_html = With a chosen Bible the reference check (@schrift, <code>scripture_check</code>) also checks the wording of a quotation. Baruch reads SWORD modules (zText, RawText; compression ZIP, LZSS, BZIP2, XZ) in their own versification and maps German references onto it. Encrypted modules become readable as soon as the publisher's key is stored below. A module folder contains <code>mods.d/</code> and <code>modules/</code>.
settings.sources.dirs = Module folders (one per line, in this order)
settings.sources.dirs_default_html = Default: <code>$SWORD_PATH</code>, <code>~/.sword</code>, <code>/usr/share/sword</code>.
settings.sources.dirs_own = Own list.
settings.sources.dirs_save = Save folders
settings.sources.dirs_reset = Default
settings.sources.col_module = Module
settings.sources.col_kind = Kind
settings.sources.col_description = Description
settings.sources.col_versification = Versification
settings.sources.col_license = Licence (according to conf)
settings.sources.col_cipher = Encryption
settings.sources.col_readable = Readable
settings.sources.kind_commentary = Commentary
settings.sources.kind_lexicon = Lexicon
settings.sources.kind_bible = Bible
settings.sources.cipher_locked = locked – key missing
settings.sources.cipher_unlocked = unlocked
settings.sources.cipher_key_stored = – key stored
settings.sources.cipher_key_conf = (key in the conf)
settings.sources.readable_with_key = only with a key
settings.sources.readable_yes = yes
settings.sources.readable_no = no – { $problem }
settings.sources.keys_title = Keys of encrypted modules
settings.sources.keys_help_html = The publisher of the module gives the key (e.g. after purchase). Baruch checks it against the first entry and stores it only in the keyring (account <code>{ $cipher_account }</code>); it never appears in a page, answer or message.
settings.sources.cipher_module = module
settings.sources.key_of_html = <code>{ $module }</code> – key
settings.sources.key_stored = stored
settings.sources.key_in_conf = is in the conf
settings.sources.key_missing = missing
settings.sources.key = Key
settings.sources.key_replace = Replace key
settings.sources.key_check_store = Check and store key
settings.sources.really_remove = really remove
settings.sources.key_remove = Remove key
settings.sources.no_modules = No modules (Bibles, commentaries, lexicons) found in the module folders.
settings.sources.work_bible = Bible of the work “{ $work_title }”
settings.sources.no_bible = none (wording unchecked)
settings.sources.chosen_missing = “{ $work_bible }” is chosen, but this module is in no module folder; the wording stays unchecked.
settings.sources.choose_bible = Choose Bible

# templates/settings/verbindungen.html
settings.connections.intro_html = Addresses and settings are stored in SQLite (for all works), keys, tokens and passwords only in the system keyring (service <code>baruch</code>). This page only says whether a secret is stored; an entry replaces it. “Check connection” asks the remote side once (at most 10 s) and remembers the result.
settings.connections.default_badge = Default
settings.connections.password = Password
settings.connections.key = Key
settings.connections.secret_placeholder = empty = unchanged
settings.connections.keyring_html = Keyring <code>baruch/{ $account }</code>:
settings.connections.unknown = unknown
settings.connections.secret_for_html = valid only for <code>{ $secret_for }</code>, not for the current address; enter it again to send to it
settings.connections.check = Check connection
settings.connections.really_remove = really remove
settings.connections.remove_secret = Remove secret
settings.connections.checked = ✓ checked
settings.connections.failed = ✗ failed
settings.connections.inactive = (inactive)
settings.connections.model_known = ✓ at Eliezer
settings.connections.model_alias = Alias → { $target }
settings.connections.model_unknown = unknown at Eliezer
settings.connections.model_also_alias = (also alias { $target })
settings.connections.not_checked = Not checked yet.
settings.connections.probe_model = Quick test with model
settings.connections.probe_placeholder = Model name at Eliezer
settings.connections.probe_help = Sends “Reply only with: OK” (at most 16 tokens) through the queue; if Eliezer has to load the model first, this can take a while.
settings.connections.probe_list_hint = List from the models of the personas; “Check connection” fetches the list from Eliezer.
settings.connections.probe = Quick test
settings.connections.probe_ok = ✓ Quick test
settings.connections.probe_failed = ✗ Quick test failed
settings.connections.reply = Reply: “{ $reply }”
settings.connections.models_title = Model connections
settings.connections.models_help = For each persona, Settings → Assistants chooses one of these connections (default: Eliezer). “external” means: the connection leaves this computer or the own network. Book text goes through an external connection only for personas that are explicitly cleared for it (default: not cleared). The address of Eliezer is above under “Eliezer (model backend)”.
settings.connections.used_by = used by { $personas }
settings.connections.eliezer_url_html = Address: <code>{ $url }</code> (setting above at Eliezer)
settings.connections.kind = Kind
settings.connections.address = Address
settings.connections.external = external (leaves this computer or the own network)
settings.connections.new_key = new key
settings.connections.store_key = Store key
settings.connections.remove_key = Remove key
settings.connections.really_delete = really delete (personas fall back to Eliezer)
settings.connections.delete = Delete connection
settings.connections.model_name = Model name
settings.connections.probe_help_external = Sends only “Reply only with: OK” (no book text); so it also goes through an external connection.
settings.connections.new_title = New model connection
settings.connections.name_placeholder = e.g. Ollama in the basement
settings.connections.locality = Location
settings.connections.locality_auto = by the address (not recognisably local → external)
settings.connections.local = local (this computer or the own network)
settings.connections.locality_help = “By the address”: local are only loopback, private addresses (10.…, 172.16–31.…, 192.168.…), host names without a dot and names under .local, .lan, .internal, .home.arpa; everything else becomes external.
settings.connections.key_later_html = Store the key (if needed) at the connection afterwards; it is kept in the keyring under <code>{ $model_account }</code> and applies only to this address.
settings.connections.create = Create
settings.connections.rag_title = RAG paths of the work
settings.connections.rag_for = These paths apply to “{ $title }”
settings.connections.rag_default = (default)
settings.connections.rag_fulltext = ; without a tick only the full-text search.
settings.connections.rag_save = Save paths
settings.connections.collections = Collections: { $label }
settings.connections.collections_help = The semantic search searches only the ticked collections for this work; without a collection { $label } stays off.
settings.connections.collections_off = { $label } is currently not chosen as a path for this work.
settings.connections.collections_hint = First “Check connection”, then the collections appear.
settings.connections.collections_checked = Reported at the check { $checked_at }.
settings.connections.gone = no longer reported
settings.connections.collection_add = Add collection
settings.connections.collection_placeholder = Name of the collection
settings.connections.collection_limits = one line, at most 200 characters; at most 100 collections
settings.connections.collections_save = Save collections
settings.connections.status_title = Status of the paths
settings.connections.index_dir = Folder
settings.connections.index_entries = Entries
settings.connections.index_model = Model/dimension
settings.connections.index_not_built = not built yet
settings.connections.index_configured = Configured
settings.connections.index_built = Last built
settings.connections.index_never = never
settings.connections.index_pending = Pending
settings.connections.index_stale = Stuck
settings.connections.stale =
    { $count ->
        [one] { $count } job stuck
       *[other] { $count } jobs stuck
    } – another attempt after a change or “Rebuild index”
settings.connections.index_problem = Problem
settings.connections.no_status = No status: { $message } The wording search (FTS) continues.
settings.connections.reindex = Rebuild index
settings.connections.reindex_help = Discards the local index (LanceDB) of this work and rebuilds it in the background; the book text stays untouched.
settings.connections.really_reindex = really rebuild
settings.connections.no_work = No work chosen; the RAG paths apply per work.

# templates/settings/werk.html
settings.work.commentaries_lexicons = Commentaries and lexicons
settings.work.commentaries = Commentaries
settings.work.not_readable = (not readable: { $problem })
settings.work.no_commentaries = No commentaries in the module folders (Settings → Sources → Bibles).
settings.work.lexicons = Lexicons
settings.work.no_lexicons = No lexicons in the module folders (Settings → Sources → Bibles).
settings.work.save_selection = Save selection
settings.work.modules_help = Chosen commentaries appear for a reference in the literature (open the Bible) and in the search as the group “Commentary”; @belege and – when switched on – the lector get them as evidence [S1] … (local, nothing leaves the computer). Licence notes of the modules are always shown too.
settings.work.language_title = Work language
settings.work.language = Work language
settings.work.language_de = German
settings.work.language_en = English
settings.work.language_help = The assistants answer in this language. The language of the interface does not change through it.
settings.work.timing_title = Sermon time
settings.work.timing_summary_html =
    Speaking time now ≈ <strong>{ $total_clock }</strong> ({ $words } spoken { $words ->
        [one] word
       *[other] words
    })
settings.work.timing_target = · target { $target_minutes } min
settings.work.timing_over = Target time exceeded by { $over_clock }
settings.work.wpm = Words per minute
settings.work.target_minutes = Target time in minutes
settings.work.target_none = none
settings.work.timing_save = Save sermon time
settings.work.timing_default = Default { $default_wpm } words per minute; empty target time = none.
settings.work.timing_sermon = Sermons always show their times
settings.work.timing_book = Books show times only with a target time
settings.work.timing_where = – in the outline and in the editor header. Typst markup, headings, comments and footnotes do not count. Stored only in the database, not in the Git mirror.
settings.work.sheet_title = Typeset view
settings.work.font = Font
settings.work.font_default = Default: Libertinus Serif, Georgia, serif
settings.work.heading_font = Font of the headings
settings.work.heading_font_default = like the font
settings.work.size = Font size
settings.work.width = Text width
settings.work.fill = Text colour
settings.work.heading_fill = Colour of the headings
settings.work.background = Page colour
settings.work.background_placeholder = #f4f1ea (light)
settings.work.sheet_save = Save typeset view
settings.work.sheet_reset = Everything from the template
settings.work.sheet_help_html = Empty = from the work template (grey: its value). Font as a list as in CSS (<code>"Erfundene Antiqua", Georgia, serif</code>), size 6 to 48 pt and text width 6 to 60 cm with the unit pt, mm, cm or em, colours as <code>#rrggbb</code>. Applies only to the typeset view in the browser – typesetting with Typst, PDF and export keep following the work template. Stored only in the database, not in the Git mirror and not in the exchange format.
settings.work.revisions_title = Revisions
settings.work.amend = Continue the own current version when saving (instead of a new version every time)
settings.work.percent_before = New version after a change of more than
settings.work.percent_after = percent
settings.work.minutes_before = New version when since the last save
settings.work.minutes_after = minutes have passed (0 = no time limit)
settings.work.revisions_save = Save versions
settings.work.revisions_help = Default: on, { $default_percent } %, { $default_minutes } min – so no limit, saving always continues as in a word processor. A new version is also always created when “Save as new version” is chosen, the current version comes from a persona (also an accepted proposal), a version other than the current one was edited, or the heading or outline was changed. Every continuation is a commit in the Git mirror and appears in the history of the version (there also “Back to here”). Personas always create new versions (proposals). Stored only in the database, not in the exchange format.
settings.work.fold_title = Hide older versions
settings.work.fold_until = Hide older revisions up to
settings.work.fold = Hide
settings.work.fold_help = Sets the mark in every section to the last version before this day (at most to the current one); replaced and discarded versions below it are then behind the switch “show n older versions”. The current version and open proposals always stay visible; nothing is deleted. Existing marks are replaced.
settings.work.folds_now =
    { $count ->
        [one] Currently 1 section has a mark.
       *[other] Currently { $count } sections have a mark.
    }
settings.work.fold_clear = Remove all marks (show all revisions again)
settings.work.sync_title = Sync with MultiGPT
settings.work.sync_on_html =
    Upload to MultiGPT: <strong>on</strong> – one document per chapter in the collection “{ $collection }” ({ $documents } { $documents ->
        [one] chapter
       *[other] chapters
    } uploaded
settings.work.sync_last = , last { $synced }
settings.work.sync_switch_off = Switch upload off
settings.work.sync_off_html = Upload to MultiGPT: <strong>off</strong>. When switched on, the next sync uploads the current versions of this work as one document per chapter into the collection “{ $collection }”.
settings.work.sync_confirm = Yes, the current versions of this work may go to MultiGPT
settings.work.sync_switch_on = Switch upload on
settings.work.sync_link = To the sync (Tools → Sync)
settings.work.no_works = There is no work yet.
settings.work.new_work = Create or import a new work …

# templates/settings/werk_satz.html
settings.typeset.from_template = from the work template
settings.typeset.on = on
settings.typeset.off = off
settings.typeset.title = Typesetting
settings.typeset.intro = Rules for typesetting the book with Typst – typeset preview, book PDF, cover and export. “from the work template” or an empty field means: Baruch sets no rule for it, whatever the work template (or Typst) prescribes applies. The typeset view in the editor stays untouched by it (block “Typeset view”).
settings.typeset.saved = Saved
settings.typeset.saved_by = by { $updated_by }
settings.typeset.never_saved = No typesetting settings saved yet.
settings.typeset.unsaved = Unsaved values – “Save” takes them over.
settings.typeset.template_rule_html = The work template sets the chapter start itself (line { $line }): <code>{ $text }</code> – this corresponds to “{ $chapter_start }”. As a setting it is visible and switchable here.
settings.typeset.adopt_confirm = Yes, adopt “Chapter start: { $chapter_start }” and remove the line from the work template (new version of the template)
settings.typeset.adopt = Adopt as a setting
settings.typeset.chapters = Chapters and parts
settings.typeset.part_blank_back = Back of the part page blank
settings.typeset.page = Page and margins
settings.typeset.width = Width
settings.typeset.custom_only = custom format only
settings.typeset.height = Height
settings.typeset.all_or_none = (all four or none)
settings.typeset.inside = inside
settings.typeset.template = template
settings.typeset.outside = outside
settings.typeset.top = top
settings.typeset.bottom = bottom
settings.typeset.binding = Binding offset
settings.typeset.binding_help = Binding offset: extra space at the inner margin for the binding – what disappears in the fold and glue, so that the text does not slip into the middle of the book. It is added to the inner margin.
settings.typeset.font_paragraph = Font and paragraph
settings.typeset.font_placeholder = Template (Typst: Libertinus Serif)
settings.typeset.size_placeholder = Template (Typst: 11)
settings.typeset.leading_placeholder = Template (Typst: 0.65)
settings.typeset.measure = Measure
settings.typeset.measure_placeholder = Indent 1, spacing 0.5
settings.typeset.lang_placeholder = Template (e.g. de)
settings.typeset.widows_help = Orphan: the first line of a paragraph alone at the bottom of a page; widow: the last line of a paragraph alone at the top of the next page. “on” lets Typst avoid such single lines.
settings.typeset.numbers_heads_notes = Page numbers, running heads, footnotes
settings.typeset.counting = Numbering
settings.typeset.head_help = Running head: the header line above the text, here the title of the work or the title of the current chapter (never on chapter-opening, part and blank pages).
settings.typeset.xrefs = Cross-references
settings.typeset.xref_help = Default for the tick “with page number” in the dialog “Reference” of the editor; not a typesetting rule.
settings.typeset.index_glossary = Index and glossary
settings.typeset.none = none
settings.typeset.no_index_pages = (no ticks = no index pages)
settings.typeset.columns = Columns
settings.typeset.no = no
settings.typeset.yes = yes
settings.typeset.no_explicit = no (explicitly saved)
settings.typeset.first_mention_help = Highlights the first mention of a glossary term (or a variant) per chapter in the typesetting, with a reference to the glossary if there is a glossary page.
settings.typeset.draft_watermark = Watermark “Draft” in the outputs
settings.typeset.draft_help = Typeset preview, book PDF and cue sheet get a light watermark, the EPUB a note on the title page and “(Draft)” in the title. Export and mirror stay unchanged. When downloading it also works once without this setting (“Download as draft”).
settings.typeset.draft_text = Text
settings.typeset.draft_note = Addition
settings.typeset.draft_note_placeholder = e.g. versions of today
settings.typeset.draft_date = Date (“As of …”, day of the output)
settings.typeset.draft_opacity = Opacity
settings.typeset.draft_placement = Position
settings.typeset.draft_cover = on the cover PDF too (the print check then warns)
settings.typeset.preview = Preview
settings.typeset.whole_work = View the whole work typeset
settings.typeset.rules = Generated Typst rules
settings.typeset.rules_unsaved = (unsaved values)
settings.typeset.no_rules = None – everything comes from the work template.
settings.typeset.preview_pages =
    Preview with sample text: { $pages } { $pages ->
        [one] page
       *[other] pages
    }
settings.typeset.preview_errors =
    , { $errors } { $errors ->
        [one] error
       *[other] errors
    }
settings.typeset.preview_warnings =
    , { $warnings } { $warnings ->
        [one] warning
       *[other] warnings
    }
settings.typeset.preview_alt = Preview page { $index }
settings.typeset.preview_help = “Preview” typesets an invented sample text (one part, three chapters with footnotes) with the work template and the values above, without saving them.
settings.typeset.reset_confirm = Yes, reset all typesetting settings (then only the work template applies again)
settings.typeset.reset = Reset

# Round 39b (rest_literals): labels and choices of the typesetting settings of baruch-typst (typeset/model.rs),
# translated by the server (typeset.rs, ui_labels); the German text is verbatim the label of the crate.
settings.typeset.field.chapter_start = Chapter start
settings.typeset.field.part_page = Part page
settings.typeset.field.page_format = Page format
settings.typeset.field.margins = Margins
settings.typeset.field.font = Body font
settings.typeset.field.font_size_pt = Font size
settings.typeset.field.leading_em = Line spacing
settings.typeset.field.paragraph = Paragraph
settings.typeset.field.justify = Justified text
settings.typeset.field.hyphenate = Hyphenation
settings.typeset.field.lang = Language
settings.typeset.field.page_numbers = Page numbers
settings.typeset.field.running_head = Running head
settings.typeset.field.footnotes = Footnotes
settings.typeset.field.widows_orphans = Widows and orphans
settings.typeset.field.xref_with_page = Cross-reference with page number
settings.typeset.field.glossary_page = Glossary page
settings.typeset.field.index_pages = Index pages
settings.typeset.field.glossary_first_mention = Mark the first mention in the glossary
settings.typeset.field.draft = Draft
settings.typeset.choice.chapter_start.continuous = continuous
settings.typeset.choice.chapter_start.new_page = new page
settings.typeset.choice.chapter_start.right_page = new right-hand page
settings.typeset.choice.part_side.right = right-hand page
settings.typeset.choice.part_side.any = any page
settings.typeset.choice.page_number_position.bottom_center = bottom centre
settings.typeset.choice.page_number_position.bottom_outside = bottom outside
settings.typeset.choice.page_number_position.top_outside = top outside
settings.typeset.choice.page_number_position.hidden = none
settings.typeset.choice.page_number_start.first_page = from the first page
settings.typeset.choice.page_number_start.first_chapter = from the first chapter
settings.typeset.choice.running_head.off = off
settings.typeset.choice.running_head.work_title = work title
settings.typeset.choice.running_head.chapter = chapter
settings.typeset.choice.footnote_numbering.arabic = 1, 2, 3
settings.typeset.choice.footnote_numbering.symbols = *, †, ‡
settings.typeset.choice.footnote_numbering.letters = a, b, c
settings.typeset.choice.footnote_restart.continuous = continuous
settings.typeset.choice.footnote_restart.per_chapter = anew per chapter
settings.typeset.choice.glossary_page.before_indexes = before the indexes
settings.typeset.choice.glossary_page.after_indexes = after the indexes
settings.typeset.choice.draft_placement.diagonal = diagonally across the page
settings.typeset.choice.draft_placement.header = header
settings.typeset.choice.draft_placement.footer = footer
settings.typeset.choice.paragraph.indent = Indent
settings.typeset.choice.paragraph.spacing = Spacing
settings.typeset.choice.page_format.a5 = A5 (148 × 210 mm)
settings.typeset.choice.page_format.120x190 = 120 × 190 mm
settings.typeset.choice.page_format.135x215 = 135 × 215 mm
settings.typeset.choice.page_format.a4 = A4 (210 × 297 mm)
settings.typeset.choice.page_format.custom = own format

# Round 39b (rest_literals): display words of the model connections (models.rs); `locality` stays the code
# lokal|extern of RPC and data-locality, the page shows these words.
settings.models.locality_local = local
settings.models.locality_external = external
settings.models.kind_openai = OpenAI-compatible
settings.models.kind_unknown = unknown
