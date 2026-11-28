# SPDX-License-Identifier: AGPL-3.0-or-later
# Messages of the semantic search (src/rag/**): states of the paths, the local index (LanceDB), the embedder,
# the check of LanceDB and the RPC methods rag.* (Round 39c, task reste_1).

# States of the paths (src/rag/mod.rs).
rag_msg.way.busy = Zu viele gleichzeitige Suchen; bitte gleich noch einmal versuchen.
rag_msg.way.not_chosen = Für dieses Werk nicht gewählt (Einstellungen → Verbindungen).
rag_msg.way.not_asked = In dieser Anfrage nicht gefragt.
rag_msg.way.not_asked_kinds = In dieser Anfrage nicht gefragt (keine der Arten section, idea, highlight, library, remote).
rag_msg.way.no_collection = Keine Sammlung gewählt (Einstellungen → Verbindungen).
rag_msg.way.never_checked = noch nicht geprüft (Verbindung prüfen)
rag_msg.way.not_set_up = Nicht eingerichtet (Einstellungen → Verbindungen).
rag_msg.way.timeout = Keine Antwort innerhalb von { $seconds } s; abgebrochen.
rag_msg.way.hits =
    { $count ->
        [one] { $count } Treffer.
       *[other] { $count } Treffer.
    }
rag_msg.way.last_search = Letzte Suche { $at }: { $message }
rag_msg.way.last_check = Letzte Prüfung { $at }: { $message }
rag_msg.way.index_pending = Index mit { $entries } Einträgen; { $pending } Aufträge ausstehend.
rag_msg.way.index_entries = Index mit { $entries } Einträgen.
rag_msg.stale =
    { $count ->
        [one] { $count } Auftrag liegengeblieben
       *[other] { $count } Aufträge liegengeblieben
    } – erneuter Versuch nach einer Änderung oder „Index neu aufbauen“; Suchen stoßen keinen neuen Versuch an.

# Embedder (src/rag/embed.rs, src/rag/mod.rs).
rag_msg.no_embedder = Kein Einbettungsmodell angebunden (Baruch läuft ohne Einbettungen, z. B. in Tests oder Hilfsbefehlen); die Wortlaut-Suche (FTS) geht weiter.
rag_msg.no_embedder_own_data = Kein Einbettungsmodell angebunden: eigener Datenordner ohne gespeicherte Verbindung Eliezer – Baruch bettet beim Start nicht ein (sonst ginge eine Anfrage an die Vorgabe-Adresse). Eliezer unter Einstellungen → Verbindungen speichern, dann „Index neu aufbauen“; die Wortlaut-Suche (FTS) geht weiter.
rag_msg.embed.batch = Einbettung: 1 bis { $max } Texte je Anfrage, nicht { $count }.
rag_msg.embed.unreachable = Kein Einbettungsmodell erreichbar: { $detail }. Die Wortlaut-Suche (FTS) geht weiter.
rag_msg.embed.external = Die Verbindung Eliezer ist als extern markiert; zum Einbetten geht kein Buchtext hinaus (Einbetten hat keine Persona und damit keine Freigabe). Nichts gesendet. Für die Bedeutungssuche Eliezer als lokal führen (Einstellungen → Verbindungen) oder RAG ausschalten
rag_msg.embed.model_connection = Modell-Verbindung Eliezer nicht lesbar: { $detail }; nichts gesendet
rag_msg.embed.connection = Verbindung eliezer nicht lesbar: { $detail }
rag_msg.embed.no_address = Keine Adresse für Eliezer eingetragen (Einstellungen → Verbindungen)
rag_msg.embed.keyring = Schlüsselbund nicht lesbar ({ $detail }); nichts gesendet
rag_msg.embed.foreign_key = Schlüssel gilt für ein anderes Ziel; nicht gesendet (Einstellungen → Verbindungen)
rag_msg.embed.queue_closed = Warteschlange geschlossen.

# Local index (src/rag/index.rs).
rag_msg.index.job_aborted = Auftrag abgebrochen: { $detail }
rag_msg.index.data_folder = Der Ordner des Index darf nicht der Datenordner selbst sein (Einstellungen → Verbindungen → LanceDB).
rag_msg.index.in_mirror = Der Ordner des Index { $dir } liegt im Git-Spiegel ({ $mirror }); bitte einen anderen wählen (Einstellungen → Verbindungen → LanceDB).
rag_msg.index.connection = Verbindung lancedb nicht lesbar: { $detail }
rag_msg.index.prefix_changed = Aufgaben-Präfix geändert; Index neu aufbauen.
rag_msg.index.model_changed = Der Index ist mit dem Einbettungsmodell { $built } aufgebaut, eingestellt ist { $configured }. Baruch mischt keine Vektoren verschiedener Modelle; „Index neu aufbauen“ (Einstellungen → Verbindungen) baut ihn mit dem neuen Modell auf.
rag_msg.index.prefix_detail = { $changed } Der Index ist mit dem Dokument-Präfix „{ $built }“ eingebettet, eingestellt ist „{ $configured }“; Baruch mischt keine Vektoren (Einstellungen → Verbindungen → LanceDB, „Index neu aufbauen“).
rag_msg.index.store = Speicher: { $detail }
rag_msg.index.library = Bibliothek: { $detail }
rag_msg.index.highlights = Hervorhebungen: { $detail }
rag_msg.index.vector_count = Einbettung lieferte { $got } statt { $want } Vektoren.
rag_msg.index.dimension_mixed = Das Einbettungsmodell liefert { $got } Dimensionen, der Index hat { $index }; nichts gemischt. „Index neu aufbauen“ hilft.
rag_msg.index.dimension_query = Das Einbettungsmodell { $model } liefert { $got } Dimensionen, der Index hat { $index }; „Index neu aufbauen“ hilft.

# Check of LanceDB (src/rag/check.rs).
rag_msg.check.dir_create = Ordner nicht anlegbar: { $detail }
rag_msg.check.not_writable = nicht beschreibbar: { $detail }
rag_msg.check.probe_delete = Probedatei nicht löschbar: { $detail }
rag_msg.check.folder = Ordner { $dir }: { $why }.
rag_msg.check.dimension = Das Modell liefert { $got } Dimensionen, der Index hat { $index }; „Index neu aufbauen“ hilft.
rag_msg.check.ready = LanceDB bereit: Ordner { $dir }, { $entries } Einträge, Einbettung mit { $model } ({ $dimension } Dimensionen) geht.

# Collections and nearness (src/rag/hooks.rs, src/rag/neighbors.rs).
rag_msg.collections = Sammlungen des Werks nicht lesbar: { $detail }
rag_msg.near.no_embedder = Keine Einbettung angebunden (Einstellungen → Verbindungen); Nähe nur über gleiche Schlagwörter.
rag_msg.near.not_indexed = Im Index liegen noch keine Einbettungen der Einfälle dieses Werks; Nähe nur über gleiche Schlagwörter.
rag_msg.near.unavailable = Nähe aus dem Index nicht verfügbar: { $detail }

# Requests (src/rag/mod.rs: search and RPC rag.*).
rag_msg.unknown_work = Unbekanntes Werk { $work }.
rag_msg.query_length = query: 1 bis { $max } Zeichen, nicht { $count }.
rag_msg.limit_range = limit: 1 bis { $max }, nicht { $limit }.
rag_msg.rpc.query = query: Text erwartet.
rag_msg.rpc.limit = limit: ganze Zahl von 1 bis 50.
rag_msg.rpc.list = { $name }: Liste von Texten erwartet.
rag_msg.rpc.ways = ways: nur lancedb, pgvector, multigpt.
rag_msg.rpc.object = Parameter als Objekt erwartet
rag_msg.rpc.work = work: Werk-id fehlt.
rag_msg.rpc.confirm = confirm: true fehlt; nichts geändert.
