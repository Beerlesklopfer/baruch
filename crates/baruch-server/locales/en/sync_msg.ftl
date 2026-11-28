# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of the synchronisation with MultiGPT (src/sync/**): notices of the page, errors of sync.*, notes of a
# run (Round 39a, task messages_2).

# Page /abgleich.
sync_msg.page.started = The synchronisation runs in the background; after reloading this page shows the result.
sync_msg.page.running = A synchronisation is already running; please wait until it has finished.
sync_msg.page.upload_on = Uploading switched on: the next synchronisation uploads the chapters of this work to MultiGPT.
sync_msg.page.upload_off = Uploading switched off: the synchronisation no longer uploads this work.
sync_msg.page.collections = The selection of the collections is saved; the next synchronisation fetches them.
sync_msg.page.loaded = The collections of MultiGPT are loaded; please select and apply.
sync_msg.page.confirm = Please confirm that the current revisions of this work may go to MultiGPT; nothing changed.
sync_msg.page.no_work = This work does not exist.
sync_msg.page.collection = Collection: 1 to 200 characters on one line, at most 100; nothing changed.
sync_msg.page.not_loaded = Collections not loaded: { $detail }
sync_msg.page.no_answer = MultiGPT does not answer.
sync_msg.page.failed = That did not work; please try again.
sync_msg.page.error = Error: { $detail }

# Connection and errors of sync.* (src/sync/mod.rs).
sync_msg.link.settings = Settings of the connection MultiGPT: { $detail }
sync_msg.link.no_address = No address entered for MultiGPT (Settings → Connections).
sync_msg.link.keyring = Keyring: { $detail }
sync_msg.link.token_other = The token baruch/multigpt is stored for another address; Baruch does not send it to the present one. Enter the token anew for the new address (Settings → Connections).
sync_msg.link.denied = MultiGPT refuses access ({ $detail }). For uploading the key needs the right docs.write, for fetching docs.read.
sync_msg.link.unreachable = MultiGPT is not reachable ({ $detail }); work continues locally.
sync_msg.run.nothing = Nothing to synchronise: no work with “upload to MultiGPT”, no collection chosen.
sync_msg.run.done = Synchronisation finished.
sync_msg.run.done_notes = Synchronisation finished with notes.
sync_msg.run.store = Storage: { $detail }
sync_msg.rpc.unknown_work = Unknown work { $work }.
sync_msg.rpc.confirm = confirm: true missing. Please confirm the upload: the current revisions of this work go to MultiGPT as documents at the next synchronisation; nothing changed.
sync_msg.rpc.no_collections_tool = MultiGPT does not offer the tool list_collections (tools/list).
sync_msg.rpc.collections_unreadable = list_collections: answer not readable (neither structuredContent nor JSON in content).
sync_msg.rpc.work_id = work: work id (number) expected.
sync_msg.rpc.params_object = Parameters expected as an object
sync_msg.rpc.work_missing = work: work id missing.
sync_msg.rpc.on = on: true or false expected.
sync_msg.rpc.collections_list = collections: list of texts expected.

# Tools of MultiGPT and their schemas (src/sync/args.rs, download.rs, upload.rs).
sync_msg.tool.missing = MultiGPT does not offer the tool { $tool } (tools/list).
sync_msg.tool.unknown_param = { $tool } requires the parameter “{ $name }”, which Baruch does not know.
sync_msg.tool.no_collection_param = { $tool } names no parameter for the collection in the inputSchema.
sync_msg.tool.no_collection_list = { $tool } names no parameter for the collection in the inputSchema; without it MultiGPT would list all documents, so Baruch does not ask.
sync_msg.tool.no_document_param = { $tool } names no parameter for the document in the inputSchema.
sync_msg.tool.no_title_param = { $tool } names no parameter for the title in the inputSchema.
sync_msg.tool.no_text_param = { $tool } names no parameter for the text in the inputSchema.
sync_msg.tool.list_unreadable = list_documents: answer not readable (neither structuredContent nor JSON in content).
sync_msg.tool.no_list = list_documents: no document list in the answer (documents, items …).
sync_msg.tool.no_ids =
    list_documents: { $count } { $count ->
        [one] entry
       *[other] entries
    } without a document id; format not readable.

# Notes of a run: fetching (download.rs).
sync_msg.download.not_read = Collection “{ $collection }” not read: { $detail }
sync_msg.download.incomplete = The list of the collection “{ $collection }” may be incomplete (full page without continuation); local copies missing from it are kept.
sync_msg.download.too_many = Collection “{ $collection }” has more than { $max } documents; only the first { $max } are fetched.
sync_msg.download.doc_not_read = “{ $title }” ({ $collection }) not read: { $detail }
sync_msg.download.doc_too_big = “{ $title }” ({ $collection }) is larger than { $max } and is not fetched.
sync_msg.download.budget = Limit of { $max } per synchronisation reached; the remaining documents follow at the next synchronisation.

# Notes of a run: uploading (upload.rs).
sync_msg.upload.resting = Further changes to MultiGPT rest until the next synchronisation; nothing is lost locally.
sync_msg.upload.problem = { $what }: { $detail }
sync_msg.upload.aborted = The last synchronisation broke off while uploading “{ $title }” (the answer of MultiGPT did not arrive); it may lie there twice without assignment – please check in the collection “{ $collection }”.
sync_msg.upload.uploading = Uploading “{ $title }”
sync_msg.upload.no_id = “{ $title }”: MultiGPT named no document id. The document probably lies there, but Baruch can neither replace nor delete it; it is uploaded again only after a change.
sync_msg.upload.old_no_id = The previous revision of “{ $title }” had no document id and stays at MultiGPT (please delete it there).
sync_msg.upload.delete_old = Deleting the old revision of “{ $title }” ({ $id })
sync_msg.upload.delete = Deleting “{ $title }” ({ $id })
