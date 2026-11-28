# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of the connections and sources (src/connections/**): checks, field checks, keyring, page
# „Verbindungen“ with its message codes, check details and form fields (Round 39a, task messages_2).

# Checks of a connection (src/connections/check.rs, mod.rs).
connections_msg.check.timeout = No answer from { $url } within { $seconds } s.
connections_msg.check.no_connection = No connection to { $url }: { $detail }
connections_msg.check.request_failed = Request to { $url } failed: { $detail }
connections_msg.check.too_big = Answer from { $url } is larger than 1 MiB; aborted.
connections_msg.check.redirect = { $target } redirects (HTTP { $status }); Baruch does not follow redirects – please enter the final address.
connections_msg.check.key_hint = check the key baruch/{ $account }
connections_msg.check.no_key = no key stored (baruch/{ $account })
connections_msg.check.denied = Access denied (HTTP { $status }): { $hint }.
connections_msg.check.http_status = { $target } answers with HTTP { $status }.
connections_msg.check.no_json = { $target } delivers no JSON.
connections_msg.check.no_models = { $target } delivers no model list (field data missing).
connections_msg.check.total_timeout = Time limit: no complete answer from { $url } within { $seconds } s; check aborted.
connections_msg.check.tool_pages = tools/list has more than { $max } pages; the tools of the first { $max } are shown.
connections_msg.check.collections_unreadable = { $tool }: answer not readable, no collections.
connections_msg.check.collections_failed = Collections not readable: { $detail }
connections_msg.check.collections_tool_missing = { $tool } missing; collections can be entered by hand.
connections_msg.check.pg_ok = Connection stands, extension vector { $vector }.
connections_msg.check.pg_no_vector = Connection stands, but the extension vector is missing (CREATE EXTENSION vector).
connections_msg.check.pg_password_hint = check the password baruch/pgvector
connections_msg.check.pg_no_password = no password stored (baruch/pgvector)
connections_msg.check.pg_login = Sign-in refused: { $detail } ({ $hint }).
connections_msg.check.pg_database = Database { $address }: { $detail }
connections_msg.check.pg_io = No connection to { $address }: { $detail }
connections_msg.check.pg_tls = TLS with { $address }: { $detail }
connections_msg.check.pg_other = Connection to { $address }: { $detail }
connections_msg.check.withheld = Not checked: the secret is stored for { $bound }, the address now points elsewhere. Baruch does not send it to another target. Enter the secret anew for the new address (or set the old address again).
connections_msg.check.incomplete = No complete answer within { $seconds } s; check aborted.

# Field checks and connection.* over /rpc (src/connections/config.rs, collections.rs, mod.rs, sources.rs).
connections_msg.field.url = Address missing or invalid (one line, at most 2000 characters).
connections_msg.field.url_invalid = Address is invalid.
connections_msg.field.url_scheme = Address must begin with http:// or https://.
connections_msg.field.url_user = Address without user and password (secrets belong in the keyring).
connections_msg.field.url_host = Address without a host name.
connections_msg.field.url_secret = Address without a key or token in the query (that is what the keyring is for).
connections_msg.field.text = { $name }: text expected.
connections_msg.field.host = Host: name or IP address without spaces, / or @.
connections_msg.field.port = Port: whole number from 1 to 65535.
connections_msg.field.short = { $name }: at most 63 characters on one line.
connections_msg.field.tls = TLS: disable, prefer, require or verify-full.
connections_msg.field.dir = Folder: not empty, one line.
connections_msg.field.table = Table: name of letters, digits and _ (not beginning with a digit, at most 63 characters), optionally schema.table.
connections_msg.field.column = { $name }: column name of letters, digits and _ (not beginning with a digit, at most 63 characters).
connections_msg.field.column_optional = { $name }: column name of letters, digits and _ (not beginning with a digit, at most 63 characters); empty = the column does not exist.
connections_msg.field.config_object = config: JSON object expected.
connections_msg.field.secret_in_config = Secrets do not belong in the settings but in the keyring (connection.set_secret).
connections_msg.rpc.way = way: pgvector or multigpt (only these ways have collections).
connections_msg.rpc.way_missing = way missing (pgvector or multigpt).
connections_msg.rpc.collections = collections: list of texts expected.
connections_msg.rpc.keyring = Keyring: { $detail }
connections_msg.rpc.kind = kind: eliezer, multigpt, pgvector or lancedb.
connections_msg.rpc.no_secret = LanceDB needs no secret.
connections_msg.rpc.secret_empty = secret: empty; nothing changed.
connections_msg.rpc.secret_long = secret: at most 4096 characters on one line.
connections_msg.rpc.work_id = work: work id (number) expected.
connections_msg.rpc.kind_missing = kind missing (eliezer, multigpt, pgvector or lancedb).
connections_msg.rpc.confirm = confirm: true missing; nothing changed.
connections_msg.rpc.ways = ways: list of lancedb, pgvector, multigpt.
connections_msg.rpc.params_object = Parameters expected as an object
connections_msg.sources.title = title: 1 to 200 characters on one line.
connections_msg.sources.url = url: http:// or https:// with a host, without a user, at most 2000 characters.
connections_msg.sources.note = note: at most 4000 characters.
connections_msg.sources.confirm = confirm: true missing; nothing deleted.
connections_msg.sources.usage = The web search for @belege (Settings → Assistants, default off) first queries the first three sources of this list via “{ $site }”, then the general web. Hits whose host matches a source are marked as “preferred source”.
connections_msg.keyring.no_access = no access to the keyring (locked or not reachable)
connections_msg.keyring.failure = Error of the keyring: { $detail }
connections_msg.keyring.encoding = the stored value is not UTF-8 text
connections_msg.keyring.ambiguous = { $count } entries match; please tidy up in the keyring

# What a connection is used for (page „Verbindungen“, the texts of connections::usage_stub).
connections_msg.usage.eliezer = Personas use Eliezer for their answers, checks and proposals – only one request at a time; the model per persona is under Settings → Assistants. Eliezer also embeds the texts for the meaning search (LanceDB and pgvector, own queue, at most 32 texts per request); without Eliezer the wording search remains.
connections_msg.usage.multigpt = Search through MultiGPT (search_documents) in the chosen collections of the work, hits with source; read_document as an excerpt of a hit and as full text in the document view of the literature (partly, without JavaScript); highlights without a quotation are checked against the full text.
connections_msg.usage.pgvector = Meaning search directly in pgvector (cosine distance, read only) in the chosen collections of the work; the query is embedded with the same embedding model as the data.
connections_msg.usage.lancedb = Main RAG in the data folder: sections (current revision), ideas, highlights and the author library, embedded through Eliezer; without an embedding model the wording search (FTS) remains.

# Details of a check (page „Verbindungen“, connections::pages::detail_lines).
connections_msg.detail.more = { $list } … and { $more } more
connections_msg.detail.count = { $name } ({ $count } { $unit })
connections_msg.detail.unit_documents = documents
connections_msg.detail.unit_sections = sections
connections_msg.detail.collections = Collections: { $list }
connections_msg.detail.models = Models: { $list }
connections_msg.detail.server = Server: { $name }
connections_msg.detail.tools = Tools: { $list }
connections_msg.detail.tool_pages = Tool list in { $pages } pages
connections_msg.detail.vector = Extension vector: { $vector }
connections_msg.detail.vector_missing = missing
connections_msg.detail.table = Table: { $table }
connections_msg.detail.table_columns = Table: { $table } ({ $columns })
connections_msg.detail.chunks = Sections: { $count }
connections_msg.detail.dimension = Dimension: { $count }
connections_msg.detail.data_models = Embedding models in the data: { $list }
connections_msg.detail.dir = Folder: { $dir }
connections_msg.detail.dir_missing = Folder: { $dir } (missing yet)
connections_msg.detail.dir_readonly = Folder: { $dir } (not writable)
connections_msg.detail.dir_writable = Folder: { $dir } (writable)
connections_msg.detail.tables = Tables: { $list }
connections_msg.detail.entries = Entries: { $count }
connections_msg.detail.index_dimension = Index: { $model }, dimension { $dimension }
connections_msg.detail.index = Index: { $model }
connections_msg.detail.configured_model = Configured model: { $model }
connections_msg.detail.embedder_ok = Embedding: works
connections_msg.detail.embedder = Embedding: { $detail }
connections_msg.detail.contexts = Reported context lengths: { $list }
connections_msg.detail.tokens = { $id }: { $count } tokens

# Input fields of a connection (label and hint, connections::pages::form_fields).
connections_msg.form.url = Address
connections_msg.form.url_hint = http:// or https://, without user and password
connections_msg.form.host = Host
connections_msg.form.host_hint = empty = not set up
connections_msg.form.port = Port
connections_msg.form.database = Database
connections_msg.form.database_hint = empty = like the user
connections_msg.form.user = User
connections_msg.form.tls = TLS
connections_msg.form.dir = Folder
connections_msg.form.dir_hint = relative to the data folder or absolute
connections_msg.form.embedding_model = Embedding model
connections_msg.form.embedding_model_hint = the same model the data are embedded with
connections_msg.form.table = Table
connections_msg.form.table_hint = table or schema.table, exactly as in the database
connections_msg.form.text_column = Column text
connections_msg.form.vector_column = Column vector
connections_msg.form.vector_column_hint = type vector (pgvector)
connections_msg.form.collection_column = Column collection
connections_msg.form.document_column = Column document
connections_msg.form.section_column = Column position
connections_msg.form.title_column = Column title
connections_msg.form.url_column = Column address
connections_msg.form.model_column = Column model
connections_msg.form.optional_hint = empty = does not exist
connections_msg.form.model_column_hint = empty = does not exist; otherwise Baruch checks the model per collection
connections_msg.form.query_prefix = Prefix of the query
connections_msg.form.query_prefix_pg_hint = as the data are embedded; for nomic-embed-text “search_query: ” (with a space at the end); empty = no prefix
connections_msg.form.query_prefix_hint = for nomic-embed-text “search_query: ” (with a space at the end); empty = no prefix
connections_msg.form.document_prefix = Prefix of the documents
connections_msg.form.document_prefix_hint = for nomic-embed-text “search_document: ”; empty = no prefix. A change requires “Rebuild index”
connections_msg.form.other = Field
connections_msg.page.template = Template { $name }: { $detail }

# Message codes of the page „Verbindungen“ (?ok=, ?fehler=, ?modellfehler=).

connections_msg.ok.gespeichert = The settings are saved.
connections_msg.ok.geheimnis = The secret is stored in the keyring.
connections_msg.ok.gespeichert_geheimnis = The settings are saved, the secret is stored in the keyring.
connections_msg.ok.geheimnis_entfernt = The secret is removed from the keyring.
connections_msg.ok.geprueft = The connection is checked; the result is shown with the connection.
connections_msg.ok.kurztest = The quick test has run; the result is shown with Eliezer.
connections_msg.ok.rag = The RAG ways of the work are saved.
connections_msg.ok.sammlungen = The collections of the work are saved.
connections_msg.ok.reindex = The index of the work is being rebuilt; “Status of the ways” shows the progress.
connections_msg.ok.angelegt = The source is created.
connections_msg.ok.geaendert = The source is changed.
connections_msg.ok.verschoben = The source is moved.
connections_msg.ok.geloescht = The source is deleted.
connections_msg.ok.bibel_ordner = The module folders of the Bibles are saved.
connections_msg.ok.bibel_vorgabe = The module folders of the Bibles are back at the default.
connections_msg.ok.bibel_gewaehlt = The Bible of the work is chosen; the reference check now checks the wording.
connections_msg.ok.bibel_keine = The work no longer has a Bible; the wording stays unchecked.
connections_msg.ok.bibel_schluessel = The key fits and is stored in the keyring; the module is readable.
connections_msg.ok.bibel_schluessel_entfernt = The key is removed from the keyring.
connections_msg.error.kind = Unknown connection; nothing changed.
connections_msg.error.url = The address is invalid; nothing changed.
connections_msg.error.url_schema = The address must begin with http:// or https://; nothing changed.
connections_msg.error.url_benutzer = The address must not contain a user or password – secrets belong in the keyring; nothing changed.
connections_msg.error.url_geheimnis = The address must not carry a key or token in the query – that is what the keyring is for; nothing changed.
connections_msg.error.geheimnis_feld = Secrets do not belong in the settings but in the keyring; nothing changed.
connections_msg.error.host = The host is invalid: name or IP address without spaces; nothing changed.
connections_msg.error.port = The port must be a whole number from 1 to 65535; nothing changed.
connections_msg.error.tls = TLS: disable, prefer, require or verify-full; nothing changed.
connections_msg.error.feld = A field is invalid (too long or multi-line); nothing changed.
connections_msg.error.ordner = The folder must not be empty; nothing changed.
connections_msg.error.modell = The embedding model must not be empty (at most 200 characters); nothing changed.
connections_msg.error.praefix = A task prefix may be at most 64 characters long and contain no control characters (not even a line break); nothing changed.
connections_msg.error.geheimnis_leer = The secret is empty; nothing changed.
connections_msg.error.geheimnis_ungueltig = The secret is too long or multi-line; nothing changed.
connections_msg.error.kein_geheimnis = LanceDB needs no secret.
connections_msg.error.bestaetigen = Please confirm (tick the box); nothing changed.
connections_msg.error.schluesselbund = The keyring is not reachable or refuses; nothing changed.
connections_msg.error.gespeichert_ohne_geheimnis = The settings are saved, but not the secret: the keyring is not reachable or refuses. The previous secret stays and is not sent to a changed address.
connections_msg.error.werk = Unknown work; nothing changed.
connections_msg.error.weg = Unknown RAG way; nothing changed.
connections_msg.error.sammlung = Invalid collection (1 to 200 characters, one line, at most 100); nothing changed.
connections_msg.error.reindex = The index could not be rebuilt; “Status of the ways” names the reason.
connections_msg.error.tabelle = The table must be a name of letters, digits and _ (optionally schema.table); nothing changed.
connections_msg.error.spalte = A column name is invalid (letters, digits and _, not beginning with a digit; only position, title, address and model may be empty); nothing changed.
connections_msg.error.titel = The title must have 1 to 200 characters on one line; nothing changed.
connections_msg.error.adresse = The address must begin with http:// or https://, name a host and must not contain spaces; nothing changed.
connections_msg.error.notiz = The note is too long (at most 4000 characters); nothing changed.
connections_msg.error.quelle = This source does not exist (any more); please reload.
connections_msg.error.position = Invalid position; nothing moved.
connections_msg.error.methode = Unknown action; nothing changed.
connections_msg.error.kurztest_modell = Please choose a model for the quick test (one line, at most 200 characters); nothing sent.
connections_msg.error.intern = Internal error; nothing changed. Please try again.
connections_msg.error.bibel_ordner = Module folders: one absolute path per line (or ~/…), at most 20 folders of 1000 characters each; nothing changed.
connections_msg.error.bibel_modul = This Bible module exists in no module folder, or Baruch cannot read it; nothing changed.
connections_msg.error.bibel_schluessel_falsch = The key does not fit the module; nothing saved.
connections_msg.error.bibel_schluessel = Key: 1 to 255 characters on one line, only for an encrypted module from the module folders; nothing saved.
connections_msg.error.other = That did not work (unknown error code).
connections_msg.model_ok.modell_gespeichert = The model connection is saved.
connections_msg.model_ok.modell_angelegt = The model connection is created.
connections_msg.model_ok.modell_geloescht = The model connection is deleted; personas that used it use Eliezer again.
connections_msg.model_ok.modell_schluessel = The key is stored in the keyring and applies to the present address.
connections_msg.model_ok.modell_schluessel_entfernt = The key of the model connection is removed from the keyring.
connections_msg.model_ok.modell_geprueft = The model connection is checked; the result is shown with it.
connections_msg.model_ok.modell_kurztest = The quick test has run; the result is shown with the connection.
connections_msg.model_error.name = The name is missing, too long or already exists; nothing changed.
connections_msg.model_error.art = Unknown kind (Eliezer, OpenAI-compatible or Ollama); nothing changed.
connections_msg.model_error.adresse = The address is invalid (http:// or https://, without user and key); nothing changed.
connections_msg.model_error.verbindung = This model connection does not exist (any more); please reload.
connections_msg.model_error.eliezer = The connection Eliezer cannot be deleted and stays of the kind Eliezer.
connections_msg.model_error.schluessel = The key is empty, too long or multi-line; nothing changed.
connections_msg.model_error.schluesselbund = The keyring is not reachable or refuses; nothing changed.
connections_msg.model_error.bestaetigen = Please confirm (tick the box); nothing changed.
connections_msg.model_error.modell = Please give a model for the quick test (one line, at most 200 characters); nothing sent.
connections_msg.model_error.aktion = Unknown action; nothing changed.
connections_msg.model_error.other = That did not work; nothing changed.
