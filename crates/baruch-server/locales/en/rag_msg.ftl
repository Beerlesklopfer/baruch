# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of the semantic search (src/rag/**): states of the paths, the local index (LanceDB), the embedder,
# the check of LanceDB and the RPC methods rag.* (Round 39c, task reste_1).

# States of the paths (src/rag/mod.rs).
rag_msg.way.busy = Too many simultaneous searches; please try again in a moment.
rag_msg.way.not_chosen = Not chosen for this work (Settings → Connections).
rag_msg.way.not_asked = Not asked in this request.
rag_msg.way.not_asked_kinds = Not asked in this request (none of the kinds section, idea, highlight, library, remote).
rag_msg.way.no_collection = No collection chosen (Settings → Connections).
rag_msg.way.never_checked = not checked yet (Check connection)
rag_msg.way.not_set_up = Not set up (Settings → Connections).
rag_msg.way.timeout = No answer within { $seconds } s; aborted.
rag_msg.way.hits =
    { $count ->
        [one] { $count } hit.
       *[other] { $count } hits.
    }
rag_msg.way.last_search = Last search { $at }: { $message }
rag_msg.way.last_check = Last check { $at }: { $message }
rag_msg.way.index_pending =
    Index with { $entries ->
        [one] { $entries } entry
       *[other] { $entries } entries
    }; { $pending ->
        [one] { $pending } job
       *[other] { $pending } jobs
    } pending.
rag_msg.way.index_entries =
    Index with { $entries ->
        [one] { $entries } entry
       *[other] { $entries } entries
    }.
rag_msg.stale =
    { $count ->
        [one] { $count } job stuck
       *[other] { $count } jobs stuck
    } – another attempt after a change or “Rebuild index”; searches do not start a new attempt.

# Embedder (src/rag/embed.rs, src/rag/mod.rs).
rag_msg.no_embedder = No embedding model connected (Baruch runs without embeddings, e.g. in tests or helper commands); the wording search (FTS) continues.
rag_msg.no_embedder_own_data = No embedding model connected: own data folder without a stored connection Eliezer – Baruch does not embed at start (otherwise a request would go to the default address). Store Eliezer under Settings → Connections, then “Rebuild index”; the wording search (FTS) continues.
rag_msg.embed.batch = Embedding: 1 to { $max } texts per request, not { $count }.
rag_msg.embed.unreachable = No embedding model reachable: { $detail }. The wording search (FTS) continues.
rag_msg.embed.external = The connection Eliezer is marked as external; no book text goes out for embedding (embedding has no persona and therefore no clearance). Nothing sent. For the semantic search keep Eliezer as local (Settings → Connections) or switch RAG off
rag_msg.embed.model_connection = Model connection Eliezer not readable: { $detail }; nothing sent
rag_msg.embed.connection = Connection eliezer not readable: { $detail }
rag_msg.embed.no_address = No address entered for Eliezer (Settings → Connections)
rag_msg.embed.keyring = Keyring not readable ({ $detail }); nothing sent
rag_msg.embed.foreign_key = The key applies to another target; not sent (Settings → Connections)
rag_msg.embed.queue_closed = Queue closed.

# Local index (src/rag/index.rs).
rag_msg.index.job_aborted = Job aborted: { $detail }
rag_msg.index.data_folder = The folder of the index must not be the data folder itself (Settings → Connections → LanceDB).
rag_msg.index.in_mirror = The folder of the index { $dir } lies in the Git mirror ({ $mirror }); please choose another one (Settings → Connections → LanceDB).
rag_msg.index.connection = Connection lancedb not readable: { $detail }
rag_msg.index.prefix_changed = Task prefix changed; rebuild the index.
rag_msg.index.model_changed = The index is built with the embedding model { $built }, configured is { $configured }. Baruch does not mix vectors of different models; “Rebuild index” (Settings → Connections) builds it with the new model.
rag_msg.index.prefix_detail = { $changed } The index is embedded with the document prefix “{ $built }”, configured is “{ $configured }”; Baruch does not mix vectors (Settings → Connections → LanceDB, “Rebuild index”).
rag_msg.index.store = Store: { $detail }
rag_msg.index.library = Library: { $detail }
rag_msg.index.highlights = Highlights: { $detail }
rag_msg.index.vector_count = Embedding delivered { $got } instead of { $want } vectors.
rag_msg.index.dimension_mixed = The embedding model delivers { $got } dimensions, the index has { $index }; nothing mixed. “Rebuild index” helps.
rag_msg.index.dimension_query = The embedding model { $model } delivers { $got } dimensions, the index has { $index }; “Rebuild index” helps.

# Check of LanceDB (src/rag/check.rs).
rag_msg.check.dir_create = Folder cannot be created: { $detail }
rag_msg.check.not_writable = not writable: { $detail }
rag_msg.check.probe_delete = Probe file cannot be deleted: { $detail }
rag_msg.check.folder = Folder { $dir }: { $why }.
rag_msg.check.dimension = The model delivers { $got } dimensions, the index has { $index }; “Rebuild index” helps.
rag_msg.check.ready =
    LanceDB ready: folder { $dir }, { $entries ->
        [one] { $entries } entry
       *[other] { $entries } entries
    }, embedding with { $model } ({ $dimension } dimensions) works.

# Collections and nearness (src/rag/hooks.rs, src/rag/neighbors.rs).
rag_msg.collections = Collections of the work not readable: { $detail }
rag_msg.near.no_embedder = No embedding connected (Settings → Connections); nearness only through shared keywords.
rag_msg.near.not_indexed = The index holds no embeddings of the ideas of this work yet; nearness only through shared keywords.
rag_msg.near.unavailable = Nearness from the index not available: { $detail }

# Requests (src/rag/mod.rs: search and RPC rag.*).
rag_msg.unknown_work = Unknown work { $work }.
rag_msg.query_length = query: 1 to { $max } characters, not { $count }.
rag_msg.limit_range = limit: 1 to { $max }, not { $limit }.
rag_msg.rpc.query = query: text expected.
rag_msg.rpc.limit = limit: whole number from 1 to 50.
rag_msg.rpc.list = { $name }: list of texts expected.
rag_msg.rpc.ways = ways: only lancedb, pgvector, multigpt.
rag_msg.rpc.object = Parameters expected as an object
rag_msg.rpc.work = work: work id missing.
rag_msg.rpc.confirm = confirm: true missing; nothing changed.
