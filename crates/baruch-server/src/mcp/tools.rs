// SPDX-License-Identifier: AGPL-3.0-or-later
//! Werkzeuge des MCP-Servers (Reihenfolge = `tools/list`). Beschreibung in `docs/MCP.md`.
//!
//! Beiträge (Runde 8): `post_move`, `post_delete`, `post_to_idea` nur für eigene Beiträge (Autor = Persona
//! aus `persona` bzw. `@mcp:<name>`), sonst `isError` ohne Änderung ([`crate::posts::act`]). Ein Client mit
//! Token heißt wie der Token und darf `persona` nie nennen ([`acting_persona`]). Antworten mit
//! Abschnitt tragen `url` = `/?work=W&key=K` (relative Adresse in der Oberfläche).
//!
//! Eigentum (Runde 23): nur über den Handle des Clients ([`Client::handle`], Regel [`super::session::handle_name`]).
//! Mit Token ist das der Tokenname; kein zweiter Token kann denselben Handle bekommen (auch nicht den eines
//! widerrufenen), und für alte Beiträge gilt zusätzlich der Alt-Handle desselben Tokens
//! ([`Client::owned_handles`]). Ohne Token (nur Loopback) gilt der angegebene `clientInfo.name` – gehört sein Handle
//! einem Token, heißt der Client `lokal-<name>` und besitzt dessen Beiträge nicht. Untereinander bleiben Clients
//! ohne Token ununterscheidbar, und `persona` dürfen sie nennen (bewusste Entscheidung: Loopback ist der Mensch
//! selbst bzw. ein Programm, das er auf seinem Rechner gestartet hat; nur die Beiträge des Menschen „Autor“ trifft
//! keine Angabe).
//!
//! Grenzen (Runde 23, Codereview ki-2): `add_post` und `propose_revision` prüfen Länge und Zahl
//! ([`MAX_MCP_POST_CHARS`], [`MAX_MCP_PROPOSAL_BYTES`], [`MAX_MCP_POSTS_PER_DAY`], [`MAX_MCP_OPEN_PROPOSALS`]).
//!
//! Lesend: `list_works`, `outline`, `work_timing` (Zeitwächter, Runde 16b), `read_node`, `search`, `list_personas`, `list_assets`, `read_asset`
//! (Bilder, Runde 6), `sources_list` (bevorzugte Quellen, Runde 9;
//! Verbindungen und Geheimnisse gibt es über MCP nicht). Schreibend nur als
//! Vorschlag bzw. Beitrag: `propose_revision` (immer `proposed`, über [`Store::add_proposal`]) und
//! `add_post`. Schreibende Werkzeuge adressieren Abschnitte nur über den festen Schlüssel `key`
//! (auch jeder Verweis in `refs`); die Stelle `node` lehnen sie ab (Vertrag Runde 5): Ein Assistent
//! mit einer älteren `outline` träfe mit ihr nach einer Umstellung einen fremden Abschnitt. Kein
//! MCP-Aufrufer kann eine Revision aktuell machen, ändern, löschen oder umschalten, und es gibt keinen
//! Import über MCP (nur `baruch-server import`, Entscheidung 2026-11-05).
//!
//! Bilder ([`super::assets`], Runde 11): `asset_add` legt ein Bild sofort an (Autor die Persona bzw. der
//! Client, Commit mit Herkunft; `role: "cover"` beantragt zusätzlich den Umschlag); `asset_replace`,
//! `asset_set_cover`, `asset_delete` legen nur Anträge an, über die der Mensch entscheidet;
//! `asset_requests` liest sie. Runde 21c: `asset_replace` nimmt `rename` (Typwechsel); Kontingent je Client und
//! Werk in 24 Stunden (30 schreibende Aufrufe, 64 MiB, 10 offene Anträge zum Ersetzen, `super::assets`).
//!
//! Umschlag-Anordnung ([`super::cover`], Runde 27): `cover_layout` liest, `cover_layout_set` stellt nur einen
//! Antrag (Bildanträge, op `cover_layout`), den der Mensch bestätigt.
//!
//! Gliederung ([`super::outline`], Wunsch 2026-11-06): `outline_add` legt Punkte sofort an (Autor die
//! Persona bzw. der Client; seit Runde 14 auch Teile) – außer ein neuer Teil übernähme vorhandene
//! Kapitel: dann ein Antrag, dem der Mensch im Dialog zustimmt (Nutzerfrage 12, 2026-11-07);
//! `outline_rename`, `outline_move`, `outline_set_level`, `outline_delete` legen nur Anträge an, über die
//! der Mensch entscheidet; `outline_requests` liest sie.
//!
//! Einfälle (Runde 7, [`super::ideas`]): `idea_create`, `idea_read`, `idea_list`, `idea_search`,
//! `idea_update`, `idea_delete`, seit Runde 34 `idea_move` – vollständiges CRUD auch für Assistenten, weil Einfälle keinen
//! Buchtext ändern (Entscheidung IDEA_CRUD); jede Aktion ist ein Commit mit Herkunft.
//!
//! Mindmap (Gedankenkarte, Runde 25, [`super::idea_map`]; Runde 34: `idea_tree` liest sie als Baum samt Wolke):
//! `idea_map` liest die flache Karte eines Werks, `idea_links` die
//! Verbindungen; `idea_link` und `idea_unlink` verbinden bzw. lösen Einfälle (kein Buchtext, ein Commit mit
//! Herkunft).
//!
//! Personas (Runde 10, [`crate::assist`]): `ask_persona` fragt eine Persona (`mode` ask: Frage als Beitrag
//! des Clients, Antwort als Beitrag der Persona; review: Befunde als Beitrag; propose: nur ein Vorschlag),
//! `chat` schreibt in den Abschnitts-Chat (Autor = Client) und ruft erwähnte Personas (`@handle`, höchstens
//! drei) auf. Runde 15: Ohne `key` schreibt `chat` in einen Faden ohne Abschnitt ([`crate::threads`]; neu oder
//! `thread`); Antworten, die weitere aktive Personas erwähnen, rufen diese einmal auf (eine Relay-Stufe, je
//! Nachricht höchstens drei Antworten, keine Schleifen). Je Client laufen bzw. warten höchstens zwei
//! Modellaufträge ([`quota`]); der dritte bekommt sofort `isError`.
//!
//! Austausch (Runde 26, [`super::exchange`]): `exchange_export` gibt das ganze Werk als Austauschdatei aus, nur
//! lesend (XML als Text oder ZIP als Base64, höchstens 4 MiB); ein Einlesen gibt es über MCP nicht.
//!
//! Bedeutungssuche (Runde 12, [`super::rag`]): `rag_search` und `rag_status`, nur lesend, Herkunft der Client;
//! einen Neuaufbau des Index gibt es über MCP nicht.
//!
//! Quellen (Runde 13, [`super::sources`]): `source_read` liest einen Teil des Volltexts über MultiGPT, nur
//! lesend; Inhalte sind Daten. Übernehmen in die Bibliothek (`source.adopt`) gibt es über MCP nicht.
//!
//! Revisionen werden immer über ihre **Nummer** gesucht ([`revision_by_number`], [`current_number`]),
//! nie über die Stelle in `Node::revisions`: Nach dem Löschen einer Revision hat die Folge Lücken.

use baruch_core::{Author, Node, Persona, Post, Revision, RevisionStatus, TextRef, Work, WorkKind};
use baruch_library::{HitKind, SearchOptions};
use baruch_store::{IdeaOrigin, Store, StoreError, WorkId};
use serde::Deserialize;
use serde_json::{Value, json};

use super::session::Client;
use crate::AppState;
use crate::rpc::{self, RpcError};

/// Warteschlange je MCP-Client (Runde 15); liegt neben diesem Modul in `src/mcp/quota.rs`.
#[path = "quota.rs"]
pub mod quota;

/// Warum ein Werkzeugaufruf scheitert.
#[derive(Debug)]
pub enum Failure {
    /// Unlesbare `arguments` → JSON-RPC-Fehler -32602.
    Args(String),
    /// Fachlicher Fehler → Ergebnis mit `isError: true` und diesem Text.
    Tool(String),
    /// Interner Fehler (Datenbank) → JSON-RPC-Fehler -32603.
    Internal(String),
}

impl From<RpcError> for Failure {
    fn from(e: RpcError) -> Self {
        if e.code == rpc::INTERNAL { Failure::Internal(e.message) } else { Failure::Tool(e.message) }
    }
}

impl From<StoreError> for Failure {
    fn from(e: StoreError) -> Self {
        RpcError::from(e).into()
    }
}

impl From<baruch_library::LibraryError> for Failure {
    fn from(e: baruch_library::LibraryError) -> Self {
        RpcError::from(e).into()
    }
}

pub(crate) fn tool_error(message: impl Into<String>) -> Failure {
    Failure::Tool(message.into())
}

/// Ein Werkzeug: Name, Beschreibung, Eingabeschema.
pub struct Tool {
    pub name: &'static str,
    pub description: &'static str,
    pub schema: fn() -> Value,
}

pub(crate) fn work_schema() -> Value {
    json!({ "description": "Work: id (number) or slug (text), see list_works",
            "oneOf": [{ "type": "integer" }, { "type": "string" }] })
}

fn node_schema() -> Value {
    json!({ "type": "integer", "minimum": 0,
            "description": "Section (node from outline): position, reassigned by the human whenever the outline is rearranged" })
}

fn key_schema() -> Value {
    json!({ "type": "integer", "description": "stable key of the section (key from outline/read_node); \
                                              takes precedence over node and survives rearranging the outline" })
}

/// id eines Beitrags (`id` aus read_node bzw. add_post).
fn post_id_schema() -> Value {
    json!({ "type": "integer", "description": "id of the post (from add_post or posts in read_node)" })
}

/// Relative Adresse eines Abschnitts in der Oberfläche (Vertrag Runde 8, §1): `/?work=W&key=K`.
pub(crate) fn node_url(work: WorkId, key: i64) -> String {
    crate::view::node_url(work, key, None, false, "", "")
}

/// `key` der schreibenden Werkzeuge: Pflicht, die Stelle `node` gibt es dort nicht.
fn write_key_schema() -> Value {
    json!({ "type": "integer", "description": "stable key of the section (key from outline/read_node); \
                                              required, survives rearranging the outline" })
}

pub const TOOLS: &[Tool] = &[
    Tool {
        name: "list_works",
        description: "All works with id, slug, title and work language (language: de or en).",
        schema: || json!({ "type": "object", "properties": {} }),
    },
    Tool {
        name: "outline",
        description: "Outline of a work: per section node (position), key (stable key), parent, number, title, \
                      kind, current revision and number of open proposals; work.language is the work language. The \
                      human reassigns positions when rearranging; pass key for writing.",
        schema: || json!({ "type": "object", "required": ["work"], "properties": { "work": work_schema() } }),
    },
    // Zeitwächter (Runde 16b), nur lesend; Einstellen nur der Mensch.
    Tool {
        name: "work_timing",
        description: "Speaking time of a work (time keeper), read-only: words per minute, target time, total words \
                      and seconds (clock \"m:ss\"), status none|ok|over with over_seconds, shown (sermon or target \
                      time) and per outline item key, number, title, words, seconds, start_seconds (time mark), \
                      subtree_seconds with clock times. The current revision is counted without Typst markup. Only \
                      the human can change the settings (Einstellungen → Werk → Predigtzeit).",
        schema: || json!({ "type": "object", "required": ["work"], "properties": { "work": work_schema() } }),
    },
    Tool {
        name: "read_node",
        description: "Text of a section (current revision or `rev`) with all revisions including status, author \
                      and note.",
        schema: || {
            json!({ "type": "object", "required": ["work"], "properties": {
                "work": work_schema(), "node": node_schema(), "key": key_schema(),
                "rev": { "type": "integer", "minimum": 1,
                         "description": "revision number (number from revisions, from 1, gaps possible)" } } })
        },
    },
    Tool {
        name: "search",
        description: "Full-text search in works (revisions, posts) and the author library (titles, quotes, \
                      highlights), read-only. Without `work` across all works. Snippets as in the user interface.",
        schema: || {
            json!({ "type": "object", "required": ["query"], "properties": {
                "query": { "type": "string" },
                "kinds": { "type": "array",
                           "items": { "enum": ["revision", "post", "title", "quote", "highlight"] } },
                "work": work_schema(),
                "all_revisions": { "type": "boolean", "description": "also revisions that are not current" },
                "limit": { "type": "integer", "minimum": 1, "maximum": 100 } } })
        },
    },
    Tool { name: "typst_check", description: super::typst::DESCRIPTION, schema: super::typst::check_schema },
    Tool { name: super::exchange::NAME, description: super::exchange::DESCRIPTION, schema: super::exchange::schema },
    // Runde 29: Satz-Einstellungen, nur lesend (ändern nur der Mensch), bei den lesenden Satz-Werkzeugen.
    Tool { name: super::typeset::NAME, description: super::typeset::DESCRIPTION, schema: super::typeset::schema },
    // Runde 20 (Schnittstelle R20-S1): EPUB ausgeben und prüfen, beide nur lesend.
    Tool { name: super::epub::NAME, description: super::epub::DESCRIPTION, schema: super::epub::schema },
    Tool {
        name: super::epub_check::NAME,
        description: super::epub_check::DESCRIPTION,
        schema: super::epub_check::schema,
    },
    // Runde 31 (Schnittstelle R31-S1): Glossar lesen, Begriffe und Registereinträge nur vorschlagen.
    Tool {
        name: super::glossary::GLOSSARY_LIST,
        description: super::glossary::LIST_DESCRIPTION,
        schema: super::glossary::list_schema,
    },
    Tool {
        name: super::glossary::GLOSSARY_PROPOSE,
        description: super::glossary::PROPOSE_DESCRIPTION,
        schema: super::glossary::propose_schema,
    },
    Tool {
        name: super::glossary::INDEX_PROPOSE,
        description: super::glossary::INDEX_DESCRIPTION,
        schema: super::glossary::index_schema,
    },
    // Runde 22 (Schnittstelle R22-S1): Verlauf der Fortschreibungen, nur lesend.
    Tool { name: super::revisions::NAME, description: super::revisions::DESCRIPTION, schema: super::revisions::schema },
    Tool {
        name: "propose_revision",
        description: "Propose a new text for a section. It always becomes a proposal (proposed); only the human \
                      can accept it. The text starts with exactly one heading of the section's level (the title may \
                      change); level, splitting and merging stay with the human. Section only by key (required, \
                      from outline); node is refused. The author is `persona` (must exist) or the client as \
                      @mcp:<name>. Limits: text at most 1 MiB, note 2000 characters, at most 100 open proposals per \
                      author and work (beyond that isError, nothing created).",
        schema: || {
            json!({ "type": "object", "required": ["work", "key", "body"], "properties": {
                "work": work_schema(), "key": write_key_schema(),
                "body": { "type": "string", "description": "complete Typst text of the section" },
                "note": { "type": "string", "description": "reason for the proposal" },
                "persona": { "type": "string", "description": "persona handle, e.g. @lektor (only without a token from the own machine)" },
                "model": { "type": "string", "description": "model; otherwise name and version of the client" } } })
        },
    },
    Tool {
        name: "add_post",
        description: "Post in the section chat, optionally with references to places in the work (refs) and \
                      sources. Section (also per reference) only by key (required, from outline); node is refused. \
                      The author is `persona` or the client as @mcp:<name>. Limits: text at most 20000 characters, \
                      20 sources (500 characters each) and 20 references (quote 2000 characters), at most 300 posts \
                      per author and work in 24 hours (beyond that isError, nothing stored).",
        schema: || {
            json!({ "type": "object", "required": ["work", "key", "body"], "properties": {
                "work": work_schema(), "key": write_key_schema(),
                "body": { "type": "string" },
                "refs": { "type": "array", "items": { "type": "object", "required": ["key", "rev", "quote"],
                    "properties": { "key": write_key_schema(), "rev": { "type": "integer" },
                                    "quote": { "type": "string", "description": "verbatim from this revision" } } } },
                "sources": { "type": "array", "items": { "type": "string" } },
                "persona": { "type": "string" } } })
        },
    },
    Tool {
        name: "post_move",
        description: "Move an own post (author = `persona` or the client as @mcp:<name>) to the end of the section \
                      chat of another section. Target only by key (required, from outline); node is refused. Posts of \
                      others are managed by the human only (isError). One commit in the mirror. The findings from \
                      the post move along (except accepted ones). Own is what stands under the author of this \
                      client: with a token the token name, without a token the given name – as @mcp:lokal-<name> \
                      if it belongs to a token.",
        schema: || {
            json!({ "type": "object", "required": ["work", "id", "key"], "properties": {
                "work": work_schema(), "id": post_id_schema(), "key": write_key_schema(),
                "persona": { "type": "string", "description": "persona handle if the post is hers (only without a token from the own machine)" } } })
        },
    },
    Tool {
        name: "post_delete",
        description: "Delete an own post (author = `persona` or the client as @mcp:<name>), without confirmation; \
                      the delete commit in the mirror records it completely. With `thread` an own thread post is \
                      deleted. Posts of others are managed by the human only (isError). Changes no book text. \
                      Ownership as for post_move.",
        schema: || {
            json!({ "type": "object", "required": ["work", "id"], "properties": {
                "work": work_schema(), "id": post_id_schema(),
                "thread": { "type": "integer", "description": "thread id: deletes the thread post `id` of this thread instead of a section post" },
                "persona": { "type": "string", "description": "persona handle if the post is hers (only without a token from the own machine)" } } })
        },
    },
    Tool {
        name: "post_to_idea",
        description: "Turn an own post into an idea (thought = text, relation = section, source = sources, keywords \
                      required); afterwards the post is deleted. Exactly one commit with einfaelle.md. Posts of \
                      others are managed by the human only (isError). Ownership as for post_move.",
        schema: || {
            json!({ "type": "object", "required": ["work", "id", "words"], "properties": {
                "work": work_schema(), "id": post_id_schema(),
                "words": { "description": "keywords: list or text \"a, b; c\"",
                           "oneOf": [{ "type": "array", "items": { "type": "string" } }, { "type": "string" }] },
                "persona": { "type": "string", "description": "persona handle if the post is hers (only without a token from the own machine)" } } })
        },
    },
    Tool {
        name: "outline_add",
        description: "Add outline items (wish of 2026-11-06): one (title, level?) or a whole outline (items: \
                      [{title, level?}], absolute level 0–3) as a block before/after/below the reference item key \
                      (place, default after) – one transaction, one commit, nothing on an error. Title not empty, \
                      one line, at most 200 characters. Parts (level 0, Round 14) before or after a part or chapter, \
                      also in works with a work template (not into); their text is the part marker. Without key \
                      only in a work without any outline item. node is refused. The author of revision 1 and of the \
                      commit is `persona` or the client as @mcp:<name>. Answer (action added): key and url per new \
                      item, adopted = []. If a new part would adopt existing chapters (the following chapters up to \
                      the next part), nothing is created; instead a request is filed that the human must actively \
                      approve (user question 12): answer action requested, status open, request (id, summary), \
                      adopted = keys of the chapters that would move; state via outline_requests. note = reason \
                      for the human.",
        schema: super::outline::add_schema,
    },
    Tool {
        name: "outline_rename",
        description: "Request: rename an outline item (title by the title rules; also parts – their part marker \
                      gets the title). Changes nothing immediately: files a request that the human confirms or \
                      rejects on the outline page (outline_requests shows the state, with preview the outline \
                      afterwards). Item only by key (required, from outline); node is refused.",
        schema: super::outline::rename_schema,
    },
    Tool {
        name: "outline_move",
        description: "Request: move an outline item with its sub-items before/after/below target_key (the level \
                      stays: before/after only next to an item of the same level, into only below one a level \
                      higher). Changes nothing immediately: files a request that the human confirms or rejects. \
                      Item and target only by key and target_key; node and target are refused.",
        schema: super::outline::move_schema,
    },
    Tool {
        name: "outline_set_level",
        description: "Request: change the level of an outline item (1–3, sub-items move along; never parts, and \
                      nothing becomes a part – level 0 is refused). Changes nothing immediately: files a request \
                      that the human confirms or rejects. Item only by key (required); node is refused.",
        schema: super::outline::set_level_schema,
    },
    Tool {
        name: "outline_set_numbered",
        description: "Request: count an outline item with (numbered: true) or without a number (false, e.g. \
                      preface, introduction); the other numbers follow. Parts always count (part without number → \
                      error). Changes nothing immediately: files a request that the human confirms or rejects. \
                      Item only by key (required); node is refused.",
        schema: super::outline::set_numbered_schema,
    },
    Tool {
        name: "outline_delete",
        description: "Request: delete an outline item (only without sub-items, not the last chapter). Changes \
                      nothing immediately: files a request that the human confirms or rejects. Item only by key \
                      (required); node is refused.",
        schema: super::outline::delete_schema,
    },
    Tool {
        name: "outline_requests",
        description: "Read the outline requests: open ones (default) or with open: false also decided ones \
                      (confirmed, rejected, stale with reason); mine: true only own ones. preview: true (Round 14) \
                      gives per open request the outline after confirming (preview: [{key, level, number, title, \
                      mark}], mark same|renamed|moved|level|numbered|deleted|added|adopted, deleted ones at their \
                      old place; since Round 16c per item also kind_word, before_number, before_level, label, \
                      before_label, changed_label) or preview_error (stale or no longer executable); nothing is \
                      changed.",
        schema: super::outline::requests_schema,
    },
    Tool { name: "asset_add", description: super::assets::ADD_DESCRIPTION, schema: super::assets::add_schema },
    Tool {
        name: "asset_replace",
        description: super::assets::REPLACE_DESCRIPTION,
        schema: super::assets::replace_schema,
    },
    Tool {
        name: "asset_set_cover",
        description: super::assets::SET_COVER_DESCRIPTION,
        schema: super::assets::set_cover_schema,
    },
    Tool { name: "asset_delete", description: super::assets::DELETE_DESCRIPTION, schema: super::assets::delete_schema },
    Tool {
        name: "asset_requests",
        description: super::assets::REQUESTS_DESCRIPTION,
        schema: super::assets::requests_schema,
    },
    Tool { name: "cover_layout", description: super::cover::LAYOUT_DESCRIPTION, schema: super::cover::layout_schema },
    Tool {
        name: "cover_layout_set",
        description: super::cover::LAYOUT_SET_DESCRIPTION,
        schema: super::cover::layout_set_schema,
    },
    // Runde 28: Befunde (lesen, Statusvorschlag) und eigene Beiträge bearbeiten.
    Tool {
        name: "findings",
        description: super::findings::FINDINGS_DESCRIPTION,
        schema: super::findings::findings_schema,
    },
    Tool {
        name: "finding_propose_status",
        description: super::findings::PROPOSE_DESCRIPTION,
        schema: super::findings::propose_schema,
    },
    Tool { name: "post_edit", description: super::posts::EDIT_DESCRIPTION, schema: super::posts::edit_schema },
    // Runde 30 (Schnittstelle R30-S1): Suchen im Werk (nur lesend) und Varianten als Vorschläge.
    Tool { name: super::search::NAME, description: super::search::DESCRIPTION, schema: super::search::schema },
    Tool { name: super::solutions::NAME, description: super::solutions::DESCRIPTION, schema: super::solutions::schema },
    Tool {
        name: "list_personas",
        description: "Personas with handle, name, role, model, context length, active; the lector is marked.",
        schema: || json!({ "type": "object", "properties": {} }),
    },
    Tool {
        name: "list_assets",
        description: "Images of a work (read-only): name, type, size, current revision, number of revisions, \
                      caption, alternative text and the paths under which template and text name them. Without \
                      `work` the default work. Adding works immediately with asset_add; replacing, cover and \
                      deleting only as a request to the human (asset_replace, asset_set_cover, asset_delete).",
        schema: || json!({ "type": "object", "properties": { "work": work_schema() } }),
    },
    Tool {
        name: "read_asset",
        description: "Read an image (read-only): data of revision `rev` (missing: the current one) and of all \
                      revisions; with `content: true` also the content (Base64 or SVG text, see `encoding`).",
        schema: || {
            json!({ "type": "object", "required": ["name"], "properties": {
                "work": work_schema(),
                "name": { "type": "string", "description": "image name from list_assets, e.g. haus.svg" },
                "rev": { "type": "integer", "minimum": 1, "description": "revision (number, gaps possible)" },
                "content": { "type": "boolean", "description": "also return the content (default false)" } } })
        },
    },
    Tool {
        name: "sources_list",
        description: crate::connections::mcp::DESCRIPTION,
        schema: crate::connections::mcp::schema,
    },
    Tool {
        name: "idea_create",
        description: "Record an idea: keywords or title (one of them required), thought, marked text, source, \
                      Bible reference, optionally with work and section (key). Mind map: title, parent (sub-node of \
                      an idea), before (before a sibling), cloud false (free main topic at the work); without a \
                      place the idea lands in the cloud beside the mind map. Changes no book text; therefore \
                      assistants may also create, change and delete ideas. The origin is `persona` (must exist) or \
                      the client as @mcp:<name>; every action is a commit in the mirror.",
        schema: super::ideas::create_schema,
    },
    Tool {
        name: "add_idea",
        description: "Record an idea – same call as idea_create (second name): keywords or title (one of them \
                      required), thought, marked text, source, Bible reference, optionally with work and section \
                      (key); mind map: title, parent, before, cloud (without a place into the cloud). Changes no \
                      book text; origin `persona` or @mcp:<name>; one commit in the mirror.",
        schema: super::ideas::create_schema,
    },
    Tool {
        name: "idea_read",
        description: "Read an idea (also a deleted one): keywords, thought, relation, origin, status; mind map: \
                      title, parent (parent node), position, main_topic, cloud (lies in the cloud).",
        schema: super::ideas::read_schema,
    },
    Tool {
        name: "idea_list",
        description: "List ideas, newest first, with keyword cloud (words); filters work (\"none\" = without \
                      work), keyword and open/processed. Deleted ones are missing.",
        schema: super::ideas::list_schema,
    },
    Tool {
        name: "idea_search",
        description: "Search the ideas with snippet; deleted ones are missing. mode \"fts\" (default): full text \
                      (keywords, thought, marked text, source, Bible reference); mode \"rag\" (only with work): \
                      semantic search with score and weak per hit, failure as isError with reason.",
        schema: super::ideas::search_schema,
    },
    Tool {
        name: "idea_update",
        description: "Change an idea: title (title, renaming in the mind map), keywords, texts, work, relation, \
                      open/processed; re-attach with idea_move. Changes no book text; also allowed for assistants \
                      (origin `persona` or @mcp:<name>, one commit per change). Changing the work removes the \
                      links of the idea (links only within one work; `unlinked` names them).",
        schema: super::ideas::update_schema,
    },
    Tool {
        name: "idea_delete",
        description: "Delete an idea, without confirmation; its sub-nodes in the mind map are deleted with it \
                      (deleted_ids names all). Everything stays restorable (idea.restore in the user interface \
                      restores the idea with the sub-nodes deleted along; entry in the delete commit of the \
                      mirror). Changes no book text.",
        schema: super::ideas::delete_schema,
    },
    Tool { name: "idea_move", description: super::ideas::MOVE_DESCRIPTION, schema: super::ideas::move_schema },
    Tool { name: "idea_map", description: super::idea_map::MAP_DESCRIPTION, schema: super::idea_map::map_schema },
    Tool { name: "idea_tree", description: super::idea_map::TREE_DESCRIPTION, schema: super::idea_map::tree_schema },
    Tool { name: "idea_links", description: super::idea_map::LINKS_DESCRIPTION, schema: super::idea_map::links_schema },
    Tool { name: "idea_link", description: super::idea_map::LINK_DESCRIPTION, schema: super::idea_map::link_schema },
    Tool {
        name: "idea_unlink",
        description: super::idea_map::UNLINK_DESCRIPTION,
        schema: super::idea_map::unlink_schema,
    },
    Tool {
        name: "ask_persona",
        description: "Ask a persona about a section; she answers with her role prompt through the model backend \
                      (Eliezer), in the work language. mode ask (default): question as a post of the client, answer \
                      as a post of the persona; review: findings as a post – on a repeated review the persona asks \
                      about resolved own findings (status proposal \"erledigt\", the human decides); propose: new \
                      revision only as a proposal (proposed). Section only by key (required); node is refused. \
                      scope chapter reviews or revises the whole chapter (default section). Answer with model, \
                      text, truncated, compressed (null or {message, summarized, chunks}: context chunked and \
                      summarised), post_id or rev and url; if the persona reports own findings as resolved, also \
                      done_proposals (finding numbers with status proposal), done_notes (skipped reports with \
                      reason) and all_done (true) – each only when set. At most two model jobs per client at a \
                      time, all clients without a token together at most four. Backend errors → isError with an \
                      honest message.",
        schema: || {
            json!({ "type": "object", "required": ["work", "key", "persona"], "properties": {
                "work": work_schema(), "key": write_key_schema(),
                "persona": { "type": "string", "description": "persona handle, e.g. @lektor (active)" },
                "question": { "type": "string", "description": "question (ask, required), hint (review) or instruction (propose); at most 4000 characters" },
                "mode": { "enum": ["ask", "review", "propose"], "description": "default ask" },
                "scope": { "enum": ["section", "chapter"], "description": "review/propose for the section (default) or the whole chapter or sermon" },
                "rev": { "type": "integer", "minimum": 1, "description": "revision shown (number); default the current one" } } })
        },
    },
    Tool {
        name: "chat",
        description: "Talk to personas (author = the client as @mcp:<name>). With key into the section chat, \
                      without key into a thread without a section (new, or continue `thread`; the human reads it \
                      under /faeden). Mentioned active personas (@handle) and `persona` answer one after another \
                      with one post each; if an answer mentions further active personas, these answer once (one \
                      relay step, at most three answers per message, every persona at most once). Quote lines \
                      (\"> …\") call nobody. At most two model jobs (chat, ask_persona) per client at a time, all \
                      clients without a token together at most four, otherwise isError. @alle or RFC at the start \
                      calls all active personas, the lector last with a summary; writing jobs become proposals. \
                      Answer with posts, answers, errors, url, work and key or thread.",
        schema: || {
            json!({ "type": "object", "required": ["work", "message"], "properties": {
                "work": work_schema(),
                "key": { "type": "integer", "description": "stable key of the section (key from outline/read_node); \
                                                           without it chat writes into a thread without a section" },
                "thread": { "type": "integer", "description": "continue a thread without a section (thread from an earlier answer); only without key" },
                "message": { "type": "string", "description": "message, at most 4000 characters; @handle calls a persona" },
                "persona": { "type": "string", "description": "persona that shall answer even without a mention" } } })
        },
    },
    Tool { name: super::scripture::NAME, description: super::scripture::DESCRIPTION, schema: super::scripture::schema },
    Tool {
        name: super::scripture_map::NAME,
        description: super::scripture_map::DESCRIPTION,
        schema: super::scripture_map::schema,
    },
    // SWORD-Bibeln (2026-11-06), nur lesend.
    Tool {
        name: "bible_modules",
        description: super::bibles::MODULES_DESCRIPTION,
        schema: super::bibles::modules_schema,
    },
    Tool { name: "bible_lookup", description: super::bibles::LOOKUP_DESCRIPTION, schema: super::bibles::lookup_schema },
    Tool { name: "bible_search", description: super::bibles::SEARCH_DESCRIPTION, schema: super::bibles::search_schema },
    Tool {
        name: "commentary_lookup",
        description: super::bibles::COMMENTARY_LOOKUP_DESCRIPTION,
        schema: super::bibles::commentary_lookup_schema,
    },
    Tool {
        name: "lexicon_lookup",
        description: super::bibles::LEXICON_LOOKUP_DESCRIPTION,
        schema: super::bibles::lexicon_lookup_schema,
    },
    Tool {
        name: "lexicon_search",
        description: super::bibles::LEXICON_SEARCH_DESCRIPTION,
        schema: super::bibles::lexicon_search_schema,
    },
    // Bedeutungssuche (Runde 12), nur lesend; kein rag_reindex über MCP.
    Tool { name: "rag_search", description: super::rag::SEARCH_DESCRIPTION, schema: super::rag::search_schema },
    Tool { name: "rag_status", description: super::rag::STATUS_DESCRIPTION, schema: super::rag::status_schema },
    // Volltext einer Quelle über MultiGPT (Runde 13), nur lesend; Übernehmen in die Bibliothek nur der Mensch.
    Tool { name: "source_read", description: super::sources::DESCRIPTION, schema: super::sources::schema },
];

pub fn list() -> Value {
    let tools: Vec<Value> = TOOLS
        .iter()
        .map(|t| json!({ "name": t.name, "description": t.description, "inputSchema": (t.schema)() }))
        .collect();
    json!({ "tools": tools })
}

/// Werk als id oder slug.
#[derive(Deserialize)]
#[serde(untagged)]
pub(crate) enum WorkRef {
    Id(i64),
    Slug(String),
}

pub(crate) fn args<T: for<'de> Deserialize<'de>>(value: Value) -> Result<T, Failure> {
    let value = if value.is_null() { json!({}) } else { value };
    serde_json::from_value(value).map_err(|e| Failure::Args(format!("arguments: {e}")))
}

/// Werk über `Store::works()` finden: `(id, slug, title)`.
pub(crate) async fn resolve(store: &Store, work: &WorkRef) -> Result<(WorkId, String, String), Failure> {
    let works = store.works().await?;
    let found = works.into_iter().find(|(id, slug, _)| match work {
        WorkRef::Id(want) => id == want,
        WorkRef::Slug(want) => slug == want,
    });
    found.ok_or_else(|| match work {
        WorkRef::Id(id) => tool_error(format!("Unknown work {id}. list_works names the works.")),
        WorkRef::Slug(slug) => tool_error(format!("Unknown work \"{slug}\". list_works names the works.")),
    })
}

/// Abschnitt eines Werks: der feste Schlüssel `key` geht vor der Stelle `node` (die vergibt der
/// Mensch beim Umstellen der Gliederung neu; ein Assistent mit älterer `outline` träfe sonst einen
/// fremden Abschnitt). Keins von beiden → Argumentfehler.
async fn node_of(store: &Store, work: WorkId, node: Option<usize>, key: Option<i64>) -> Result<usize, Failure> {
    match (key, node) {
        (Some(key), _) => Ok(store.node_seq(work, key).await?),
        (None, Some(node)) => Ok(node),
        (None, None) => Err(Failure::Args("arguments: node or key is missing".into())),
    }
}

/// Hinweis, wenn ein schreibendes Werkzeug `node` bekommt (`what` = `arguments` bzw. `refs[i]`).
fn node_refused(what: &str) -> Failure {
    Failure::Args(format!(
        "{what}: node (position) is not accepted; pass key (stable key from outline or read_node). \
         The human reassigns positions whenever the outline is rearranged."
    ))
}

/// Fester Schlüssel eines schreibenden Werkzeugs: `node` vorhanden → Argumentfehler (die Stelle wäre
/// nach einer Umstellung ein fremder Abschnitt), `key` fehlt → Argumentfehler. Beides -32602.
pub(crate) fn write_key(arguments: &Value, key: Option<i64>, what: &str) -> Result<i64, Failure> {
    if arguments.get("node").is_some() {
        return Err(node_refused(what));
    }
    key.ok_or_else(|| {
        Failure::Args(format!("{what}: key is missing – name the section by its stable key (key from outline)."))
    })
}

/// Werk laden; Abschnitt (über `key` oder `node`) bestimmen und prüfen.
async fn work_with_node(
    store: &Store,
    work: &WorkRef,
    node: Option<usize>,
    key: Option<i64>,
) -> Result<(WorkId, Work, usize), Failure> {
    let (id, _, _) = resolve(store, work).await?;
    let node = node_of(store, id, node, key).await?;
    let loaded = store.load_work(id).await?;
    if node >= loaded.nodes.len() {
        return Err(StoreError::UnknownNode { work: id, node }.into());
    }
    Ok((id, loaded, node))
}

/// Revision mit der Nummer `number`. Bewusst über `number` statt über die Stelle in der Liste: Nach
/// dem Löschen einer Revision gibt es Lücken, Nummer und Stelle passen dann nicht mehr zusammen.
fn revision_by_number(node: &Node, number: u32) -> Option<&Revision> {
    node.revisions.iter().find(|r| r.number == number)
}

/// Nummer der aktuellen Fassung, falls der Abschnitt eine hat.
fn current_number(node: &Node) -> Option<u32> {
    node.revisions.iter().find(|r| r.status == RevisionStatus::Current).map(|r| r.number)
}

/// Gezeigte Revision für `read_node`: `rev` nach Nummer; ohne `rev` die aktuelle Fassung, in einem
/// Abschnitt ohne aktuelle die mit der höchsten Nummer; ohne Revisionen `(0, "")`.
/// Unbekannte Nummer → `UnknownRevision` mit allen vorhandenen Nummern (nach dem Löschen mit Lücken).
fn shown_revision(node: &Node, rev: Option<u32>) -> Result<(u32, &str), StoreError> {
    let highest = node.revisions.iter().map(|r| r.number).max();
    let wanted = match rev {
        Some(rev) => {
            let found = revision_by_number(node, rev);
            return found.map(|r| (r.number, r.body.as_str())).ok_or_else(|| {
                let mut existing: Vec<u32> = node.revisions.iter().map(|r| r.number).collect();
                existing.sort_unstable();
                StoreError::UnknownRevision { node: node.id, rev, existing }
            });
        }
        None => current_number(node).or(highest),
    };
    Ok(wanted.and_then(|n| revision_by_number(node, n)).map_or((0, ""), |r| (r.number, r.body.as_str())))
}

fn author_json(author: &Author) -> Value {
    match author {
        Author::Human { name } => json!({ "kind": "human", "name": name }),
        Author::Assistant { persona, model } => json!({ "kind": "assistant", "name": persona, "model": model }),
    }
}

/// Persona-Handle aus der Tabelle `persona`; ohne führendes '@' wird es ergänzt.
async fn persona_handle(store: &Store, wanted: &str) -> Result<String, Failure> {
    let wanted = wanted.trim();
    let handle = if wanted.starts_with('@') { wanted.to_owned() } else { format!("@{wanted}") };
    let found: Option<String> = sqlx::query_scalar("SELECT handle FROM persona WHERE handle = ?")
        .bind(&handle)
        .fetch_optional(store.pool())
        .await
        .map_err(|e| Failure::from(StoreError::from(e)))?;
    found.ok_or_else(|| tool_error(format!("Unknown persona \"{wanted}\". list_personas names the personas.")))
}

/// Meldung, wenn ein Client mit Token `persona` nennt.
pub const TOKEN_NO_PERSONA: &str = "With a token a client acts only in its own name (@mcp:<token name>); \
`persona` as author or origin works only without a token from the own machine. To let a persona answer herself: \
ask_persona or chat.";

/// Persona, in deren Namen der Client handeln will (`persona`, leer = keine). Ein Client mit Token
/// (anderer Rechner möglich) darf das nicht: Er könnte sonst Beiträge und Vorschläge als Werk einer Persona
/// ausgeben und ihre echten, vom Modell erzeugten Beiträge löschen oder verschieben → `isError`
/// [`TOKEN_NO_PERSONA`]. Über Loopback ohne Token bleibt es bei „Eigentum nach Angabe des Clients“.
pub(crate) async fn acting_persona(
    store: &Store,
    client: &Client,
    persona: Option<&str>,
) -> Result<Option<String>, Failure> {
    match persona.filter(|p| !p.trim().is_empty()) {
        None => Ok(None),
        Some(_) if client.token => Err(tool_error(TOKEN_NO_PERSONA.to_owned())),
        Some(p) => Ok(Some(persona_handle(store, p).await?)),
    }
}

/// Herkunft für Einfälle, Gliederung und Beiträge: die Persona aus `persona` ([`acting_persona`]) oder
/// der Client als `@mcp:<name>`.
pub(crate) async fn origin_of(store: &Store, client: &Client, persona: Option<&str>) -> Result<IdeaOrigin, Failure> {
    Ok(match acting_persona(store, client, persona).await? {
        Some(handle) => IdeaOrigin::Persona { handle },
        None => IdeaOrigin::Mcp { handle: client.handle() },
    })
}

async fn author_handle(store: &Store, persona: Option<&str>, client: &Client) -> Result<String, Failure> {
    Ok(acting_persona(store, client, persona).await?.unwrap_or_else(|| client.handle()))
}

/// Personas aus SQLite in der Reihenfolge `position`.
async fn personas(store: &Store) -> Result<Vec<Persona>, Failure> {
    let rows: Vec<(String, String, String, String, i64, i64)> =
        sqlx::query_as("SELECT handle, name, role, model, context_length, active FROM persona ORDER BY position, id")
            .fetch_all(store.pool())
            .await
            .map_err(|e| Failure::from(StoreError::from(e)))?;
    Ok(rows
        .into_iter()
        .map(|(handle, name, role, model, context_length, active)| Persona {
            handle,
            name,
            role,
            model,
            context_length: u32::try_from(context_length).unwrap_or(0),
            active: active != 0,
        })
        .collect())
}

#[derive(Deserialize)]
struct WorkArgs {
    work: WorkRef,
}

#[derive(Deserialize)]
struct ReadArgs {
    work: WorkRef,
    node: Option<usize>,
    key: Option<i64>,
    rev: Option<u32>,
}

#[derive(Deserialize)]
struct SearchArgs {
    query: String,
    kinds: Option<Vec<HitKind>>,
    work: Option<WorkRef>,
    #[serde(default)]
    all_revisions: bool,
    limit: Option<u32>,
}

#[derive(Deserialize)]
struct ProposeArgs {
    work: WorkRef,
    key: Option<i64>,
    body: String,
    note: Option<String>,
    persona: Option<String>,
    model: Option<String>,
}

#[derive(Deserialize)]
struct RefArgs {
    key: Option<i64>,
    rev: u32,
    quote: String,
}

#[derive(Deserialize)]
struct PostArgs {
    work: WorkRef,
    key: Option<i64>,
    body: String,
    #[serde(default)]
    refs: Vec<RefArgs>,
    #[serde(default)]
    sources: Vec<String>,
    persona: Option<String>,
}

/// Werkzeug `name` mit `arguments` ausführen. Ergebnis ist das JSON für `structuredContent`.
/// `None`: kein solches Werkzeug.
pub async fn call(state: &AppState, client: &Client, name: &str, arguments: Value) -> Option<Result<Value, Failure>> {
    let store = &state.store;
    Some(match name {
        "list_works" => list_works(store).await,
        "outline" => outline(store, arguments).await,
        "work_timing" => work_timing(state, arguments).await,
        "read_node" => read_node(store, arguments).await,
        "search" => search(store, arguments).await,
        "typst_check" => super::typst::call(state, arguments).await,
        // Runde 29: Satz-Einstellungen, nur lesend.
        "typeset_settings" => super::typeset::call(state, arguments).await,
        super::exchange::NAME => super::exchange::call(state, arguments).await,
        // Runde 20: EPUB, nur lesend (epub_check startet nie EPUBCheck).
        n if n == super::epub::NAME => super::epub::call(state, arguments).await,
        n if n == super::epub_check::NAME => super::epub_check::call(state, arguments).await,
        // Runde 31: Glossar lesen; Begriffe und Registereinträge nur als Vorschlag.
        n if n == super::glossary::GLOSSARY_LIST => super::glossary::call_list(state, arguments).await,
        n if n == super::glossary::GLOSSARY_PROPOSE => super::glossary::call_propose(state, client, arguments).await,
        n if n == super::glossary::INDEX_PROPOSE => super::glossary::call_index(state, client, arguments).await,
        // Runde 22: Verlauf der Fortschreibungen, nur lesend.
        n if n == super::revisions::NAME => super::revisions::call(state, arguments).await,
        // Unter der Schreibsperre: zwischen Übersetzen von `key` und Schreiben stellt niemand um.
        "propose_revision" => {
            let held = state.write_lock.lock().await;
            let answer = propose_revision(store, client, arguments).await;
            // Runde 22: Seitenzweig des neuen Vorschlags gleich anlegen (refs/revisions/<key>/<n>).
            if let Some(work) = answer.as_ref().ok().and_then(|a| a["work"].as_i64()) {
                crate::mirror::side_refs_after_proposal(state, &held, work).await;
            }
            answer
        }
        "add_post" => {
            let _held = state.write_lock.lock().await;
            add_post(store, client, arguments).await
        }
        // Eigene Beiträge verwalten (Runde 8); die Schreibsperre nimmt `posts::act` selbst.
        "post_move" | "post_delete" | "post_to_idea" => manage_post(state, client, name, arguments).await,
        "list_personas" => list_personas(store).await,
        "list_assets" => list_assets(state, arguments).await,
        "read_asset" => read_asset(state, arguments).await,
        "sources_list" => crate::connections::mcp::sources_list(state, arguments).await,
        name if super::ideas::NAMES.contains(&name) => return super::ideas::call(state, client, name, arguments).await,
        name if super::idea_map::NAMES.contains(&name) => {
            return super::idea_map::call(state, client, name, arguments).await;
        }
        // Bilder (Runde 11): anlegen sofort, alles andere als Antrag; die Sperre nimmt das Modul.
        name if super::assets::NAMES.contains(&name) => {
            return super::assets::call(state, client, name, arguments).await;
        }
        // Umschlag-Anordnung (Runde 27): lesen sofort, ändern nur als Antrag.
        name if super::cover::NAMES.contains(&name) => {
            return super::cover::call(state, client, name, arguments).await;
        }
        // Befunde (Runde 28): lesen sofort, Status nur als Vorschlag; die Sperre nimmt das Modul.
        name if super::findings::NAMES.contains(&name) => {
            return super::findings::call(state, client, name, arguments).await;
        }
        // Eigene Beiträge bearbeiten (Runde 28); die Sperre nimmt das Modul.
        name if super::posts::NAMES.contains(&name) => {
            return super::posts::call(state, client, name, arguments).await;
        }
        // Gliederung (Wunsch 2026-11-06): anlegen sofort, alles andere als Antrag; die Sperre nimmt das Modul.
        name if super::outline::NAMES.contains(&name) => {
            return super::outline::call(state, client, name, arguments).await;
        }
        // Modellaufruf ohne Schreibsperre; die nimmt `crate::assist` erst zum Speichern.
        "ask_persona" | "chat" => {
            // Höchstens zwei laufende bzw. wartende Modellaufträge je Client (Runde 15).
            let _slot = match quota::acquire(state, client) {
                Ok(slot) => slot,
                Err(message) => return Some(Err(tool_error(message))),
            };
            if name == "chat" {
                chat(state, client, arguments).await
            } else {
                ask_persona(state, client, arguments).await
            }
        }
        // Runde 30a: Suchen im Werk, nur lesend (Ersetzen gibt es über MCP nicht).
        super::search::NAME => super::search::call_tool(state, client, arguments).await,
        // Runde 30b: Varianten als Vorschläge; ein Modellauftrag, zählt zur Quote wie chat/ask_persona.
        super::solutions::NAME => {
            let _slot = match quota::acquire(state, client) {
                Ok(slot) => slot,
                Err(message) => return Some(Err(tool_error(message))),
            };
            super::solutions::call_tool(state, client, arguments).await
        }
        super::scripture::NAME => super::scripture::call(state, arguments).await,
        super::scripture_map::NAME => super::scripture_map::call(state, arguments).await,
        name if super::bibles::NAMES.contains(&name) => super::bibles::call(state, name, arguments).await,
        name if super::rag::NAMES.contains(&name) => super::rag::call(state, client, name, arguments).await,
        name if super::sources::NAMES.contains(&name) => super::sources::call(state, name, arguments).await,
        _ => return None,
    })
}

async fn list_works(store: &Store) -> Result<Value, Failure> {
    let mut works = Vec::new();
    for (id, slug, title) in store.works().await? {
        let language = store.work_language(id).await?;
        works.push(json!({ "id": id, "slug": slug, "title": title, "language": language }));
    }
    Ok(json!({ "works": works }))
}

async fn outline(store: &Store, arguments: Value) -> Result<Value, Failure> {
    let a: WorkArgs = args(arguments)?;
    let (id, slug, title) = resolve(store, &a.work).await?;
    let work = store.load_work(id).await?;
    let nodes: Vec<Value> = work
        .nodes
        .iter()
        .map(|n| {
            json!({
                "node": n.id, "key": n.key, "parent": n.parent, "number": n.number, "title": n.title, "kind": n.kind,
                "current_rev": current_number(n), "proposals": n.open_proposals(), "url": node_url(id, n.key),
            })
        })
        .collect();
    let language = store.work_language(id).await?;
    Ok(json!({ "work": { "id": id, "slug": slug, "title": title }, "language": language, "nodes": nodes }))
}

/// `work_timing` (Runde 16b): dasselbe wie RPC `work.timing`, nur lesend.
async fn work_timing(state: &AppState, arguments: Value) -> Result<Value, Failure> {
    let a: WorkArgs = args(arguments)?;
    let (id, _, _) = resolve(&state.store, &a.work).await?;
    Ok(crate::timing::timing_json(&crate::timing::work_timing(state, id).await?))
}

async fn read_node(store: &Store, arguments: Value) -> Result<Value, Failure> {
    let a: ReadArgs = args(arguments)?;
    let (work_id, work, at) = work_with_node(store, &a.work, a.node, a.key).await?;
    let node = &work.nodes[at];
    let (rev, text) = shown_revision(node, a.rev)?;
    let revisions: Vec<Value> = node
        .revisions
        .iter()
        .map(|r| json!({ "number": r.number, "status": r.status, "author": author_json(&r.author), "note": r.note }))
        .collect();
    // Beiträge mit id: eigene lassen sich über post_move/post_delete/post_to_idea verwalten (Runde 8).
    let posts: Vec<Value> = node
        .posts
        .iter()
        .map(|p| json!({ "id": p.id, "author": p.author, "body": p.body, "created": p.created }))
        .collect();
    Ok(json!({
        "node": node.id, "key": node.key, "number": node.number, "title": node.title, "rev": rev, "text": text,
        "revisions": revisions, "posts": posts, "url": node_url(work_id, node.key),
    }))
}

async fn search(store: &Store, arguments: Value) -> Result<Value, Failure> {
    let a: SearchArgs = args(arguments)?;
    let work = match &a.work {
        Some(w) => Some(resolve(store, w).await?.0),
        None => None,
    };
    let options = SearchOptions { kinds: a.kinds, work, all_revisions: a.all_revisions, limit: a.limit };
    let hits = baruch_library::search(store.pool(), &a.query, &options).await?;
    // Je Treffer mit Abschnitt die Adresse in der Oberfläche (mit `&rev=R` bei Revisionen), sonst null.
    let hits: Vec<Value> = hits
        .iter()
        .map(|hit| {
            let mut value = json!(hit);
            value["url"] = match (hit.work, hit.node_key) {
                (Some(work), Some(key)) => json!(crate::view::node_url(work, key, hit.rev, false, "", "")),
                _ => Value::Null,
            };
            value
        })
        .collect();
    Ok(json!({ "hits": hits }))
}

/// Grenzen der schreibenden Werkzeuge `add_post` und `propose_revision` (Runde 23, Codereview ki-2): Vorher nahm
/// MCP beliebig lange Texte bis zur Größe der Anfrage und beliebig viele an; SQLite und der Git-Spiegel wüchsen ohne
/// Bremse. Darüber antwortet das Werkzeug mit `isError` und einer ehrlichen Meldung, gespeichert wird nichts.
/// Längster Beitrag über `add_post` (Zeichen).
pub const MAX_MCP_POST_CHARS: usize = 20_000;
/// Höchstens so viele Quellen bzw. Verweise je Beitrag.
pub const MAX_MCP_POST_SOURCES: usize = 20;
/// Längste Quelle eines Beitrags (Zeichen).
pub const MAX_MCP_SOURCE_CHARS: usize = 500;
/// Längstes Zitat eines Verweises (Zeichen).
pub const MAX_MCP_REF_QUOTE_CHARS: usize = 2000;
/// Größter Vorschlag über `propose_revision` (Bytes; ein Abschnitt mit rund 500 Seiten Text).
pub const MAX_MCP_PROPOSAL_BYTES: usize = 1024 * 1024;
/// Längste Notiz eines Vorschlags (Zeichen) und längste Modellangabe.
pub const MAX_MCP_NOTE_CHARS: usize = 2000;
pub const MAX_MCP_MODEL_CHARS: usize = 200;
/// Höchstens so viele Beiträge je Autor und Werk in 24 Stunden über `add_post`; alle Clients ohne Token zählen
/// zusammen als ein Autor ([`Counted`]).
pub const MAX_MCP_POSTS_PER_DAY: usize = 300;
/// Höchstens so viele **offene** Vorschläge je Autor und Werk; alle Clients ohne Token zählen zusammen ([`Counted`]).
pub const MAX_MCP_OPEN_PROPOSALS: usize = 100;

fn too_long(what: &str, limit: usize, unit: &str, now: usize) -> Failure {
    tool_error(format!("{what}: at most {limit} {unit} (now {now}). Nothing stored."))
}

/// Autoren aller Tokens (auch widerrufener), je Handle und Alt-Handle: Sie zählen nie zur Gruppe der Clients ohne
/// Token ([`Counted::WithoutToken`], Bildkontingent in [`super::assets`]).
pub(crate) async fn token_handles(store: &Store) -> Result<Vec<String>, Failure> {
    use super::session::{handle_name, legacy_handle_name};
    Ok(store
        .mcp_token_list()
        .await?
        .iter()
        .flat_map(|t| [format!("@mcp:{}", handle_name(&t.name)), format!("@mcp:{}", legacy_handle_name(&t.name))])
        .collect())
}

/// Wessen Bestand für [`MAX_MCP_POSTS_PER_DAY`] und [`MAX_MCP_OPEN_PROPOSALS`] zählt (Nachbesserung Runde 23).
enum Counted {
    /// Genau dieser Autor: ein Token (sein Name steht fest) oder eine Persona, in deren Namen ein Client handelt.
    Author(String),
    /// Alle Clients ohne Token zusammen: jeder Autor `@mcp:…`, der keinem Token gehört (Liste der Token-Autoren als
    /// JSON). Ohne Token wählt der Client seinen Namen bei jedem `initialize` frei – je Name ein eigenes Kontingent
    /// wäre keines.
    WithoutToken(String),
}

impl Counted {
    async fn of(store: &Store, client: &Client, author: &str) -> Result<Counted, Failure> {
        if client.token || !author.starts_with("@mcp:") {
            return Ok(Counted::Author(author.to_owned()));
        }
        let handles = token_handles(store).await?;
        Ok(Counted::WithoutToken(serde_json::to_string(&handles).map_err(|e| Failure::Internal(e.to_string()))?))
    }

    /// SQL-Bedingung an die Autorenspalte `column` (genau ein Parameter: [`Counted::bound`]).
    fn condition(&self, column: &str) -> String {
        match self {
            Counted::Author(_) => format!("{column} = ?"),
            Counted::WithoutToken(_) => {
                format!("{column} LIKE '@mcp:%' AND {column} NOT IN (SELECT value FROM json_each(?))")
            }
        }
    }

    fn bound(&self) -> &str {
        match self {
            Counted::Author(author) => author,
            Counted::WithoutToken(handles) => handles,
        }
    }

    /// Satzanfang der Meldung: „<Autor> hat“ bzw. „Die Clients ohne Token (gemeinsames Kontingent) haben“.
    fn who_has(&self) -> String {
        match self {
            Counted::Author(author) => format!("{author} has"),
            Counted::WithoutToken(_) => "The clients without a token (shared quota) have".to_owned(),
        }
    }
}

/// Offene Vorschläge im Werk, die für `counted` zählen (für [`MAX_MCP_OPEN_PROPOSALS`]).
async fn open_proposals_by(store: &Store, work: WorkId, counted: &Counted) -> Result<usize, Failure> {
    let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT count(*) FROM revision r JOIN node n ON n.id = r.node_id \
         WHERE n.work_id = ? AND r.status = 'proposed' AND {}",
        counted.condition("r.author_name")
    )))
    .bind(work)
    .bind(counted.bound())
    .fetch_one(store.pool())
    .await
    .map_err(|e| Failure::from(StoreError::from(e)))?;
    Ok(usize::try_from(count).unwrap_or(usize::MAX))
}

/// Beiträge im Werk in den letzten 24 Stunden, die für `counted` zählen (für [`MAX_MCP_POSTS_PER_DAY`]).
async fn posts_today_by(store: &Store, work: WorkId, counted: &Counted) -> Result<usize, Failure> {
    let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
        "SELECT count(*) FROM post p JOIN node n ON n.id = p.node_id WHERE n.work_id = ? AND {} \
         AND p.created >= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-1 day')",
        counted.condition("p.author")
    )))
    .bind(work)
    .bind(counted.bound())
    .fetch_one(store.pool())
    .await
    .map_err(|e| Failure::from(StoreError::from(e)))?;
    Ok(usize::try_from(count).unwrap_or(usize::MAX))
}

async fn propose_revision(store: &Store, client: &Client, arguments: Value) -> Result<Value, Failure> {
    let a: ProposeArgs = args(arguments.clone())?;
    let key = write_key(&arguments, a.key, "arguments")?;
    if a.body.trim().is_empty() {
        return Err(tool_error("Empty text: a proposal needs the complete text of the section."));
    }
    if a.body.len() > MAX_MCP_PROPOSAL_BYTES {
        return Err(too_long("The proposal", MAX_MCP_PROPOSAL_BYTES, "bytes", a.body.len()));
    }
    let note_chars = a.note.as_deref().map_or(0, |n| n.chars().count());
    if note_chars > MAX_MCP_NOTE_CHARS {
        return Err(too_long("note", MAX_MCP_NOTE_CHARS, "characters", note_chars));
    }
    let model_chars = a.model.as_deref().map_or(0, |m| m.chars().count());
    if model_chars > MAX_MCP_MODEL_CHARS {
        return Err(too_long("model", MAX_MCP_MODEL_CHARS, "characters", model_chars));
    }
    let (id, work, at) = work_with_node(store, &a.work, None, Some(key)).await?;
    let node = &work.nodes[at];
    let body = a.body.replace("\r\n", "\n");
    if node.revisions.iter().any(|r| r.status == RevisionStatus::Current && r.body == body) {
        return Err(tool_error("The text equals the current revision; no proposal created."));
    }
    let author = author_handle(store, a.persona.as_deref(), client).await?;
    let counted = Counted::of(store, client, &author).await?;
    let open = open_proposals_by(store, id, &counted).await?;
    if open >= MAX_MCP_OPEN_PROPOSALS {
        let who = counted.who_has();
        return Err(tool_error(format!(
            "Too many open proposals: {who} already {open} in this work; allowed are \
             {MAX_MCP_OPEN_PROPOSALS}. Nothing created; first the human decides on the open ones."
        )));
    }
    let model = a.model.filter(|m| !m.trim().is_empty()).unwrap_or_else(|| client.model());
    let note = a.note.unwrap_or_default();
    let rev = store.add_proposal(id, at, &body, &author, &model, &note).await?;
    Ok(
        json!({ "work": id, "node": at, "key": node.key, "rev": rev, "status": "proposed", "author": author, "model": model }),
    )
}

async fn add_post(store: &Store, client: &Client, arguments: Value) -> Result<Value, Failure> {
    let a: PostArgs = args(arguments.clone())?;
    let key = write_key(&arguments, a.key, "arguments")?;
    // Runde 23 (Codereview ki-2): Größen prüfen, bevor irgendetwas gelesen oder geschrieben wird.
    let body_chars = a.body.chars().count();
    if body_chars > MAX_MCP_POST_CHARS {
        return Err(too_long("The post", MAX_MCP_POST_CHARS, "characters", body_chars));
    }
    if a.sources.len() > MAX_MCP_POST_SOURCES {
        return Err(too_long("sources", MAX_MCP_POST_SOURCES, "entries", a.sources.len()));
    }
    if let Some(n) = a.sources.iter().map(|s| s.chars().count()).find(|n| *n > MAX_MCP_SOURCE_CHARS) {
        return Err(too_long("sources: every source", MAX_MCP_SOURCE_CHARS, "characters", n));
    }
    if a.refs.len() > MAX_MCP_POST_SOURCES {
        return Err(too_long("refs", MAX_MCP_POST_SOURCES, "entries", a.refs.len()));
    }
    if let Some(n) = a.refs.iter().map(|r| r.quote.chars().count()).find(|n| *n > MAX_MCP_REF_QUOTE_CHARS) {
        return Err(too_long("refs: every quote", MAX_MCP_REF_QUOTE_CHARS, "characters", n));
    }
    // Jeden Verweis vorher prüfen: Auch dort gilt nur der Schlüssel.
    let raw_refs = arguments.get("refs").and_then(Value::as_array).cloned().unwrap_or_default();
    let mut ref_keys = Vec::with_capacity(a.refs.len());
    for (i, (r, raw)) in a.refs.iter().zip(&raw_refs).enumerate() {
        ref_keys.push(write_key(raw, r.key, &format!("refs[{i}]"))?);
    }
    let (id, work, at) = work_with_node(store, &a.work, None, Some(key)).await?;
    let mut refs = Vec::with_capacity(a.refs.len());
    for (r, key) in a.refs.into_iter().zip(ref_keys) {
        let ref_node =
            work.nodes.iter().find(|n| n.key == key).map(|n| n.id).ok_or_else(|| {
                tool_error(format!("Reference to key {key}: this section does not exist (any more)."))
            })?;
        let Some(target) = work.nodes.get(ref_node) else {
            return Err(tool_error(format!("Reference to unknown section {ref_node}.")));
        };
        let Some(revision) = revision_by_number(target, r.rev) else {
            let known: Vec<String> = target.revisions.iter().map(|r| r.number.to_string()).collect();
            return Err(tool_error(format!(
                "Reference to revision {} of section {}: it does not exist (existing: {}).",
                r.rev,
                ref_node,
                if known.is_empty() { "none".to_owned() } else { known.join(", ") }
            )));
        };
        let quote = r.quote.trim();
        if quote.is_empty() || !revision.body.contains(quote) {
            return Err(tool_error(format!(
                "Reference to {} revision {}: the quote is not literally in this revision.",
                ref_node, r.rev
            )));
        }
        refs.push(TextRef { node: ref_node, rev: r.rev, quote: quote.to_owned() });
    }
    let author = author_handle(store, a.persona.as_deref(), client).await?;
    let counted = Counted::of(store, client, &author).await?;
    let today = posts_today_by(store, id, &counted).await?;
    if today >= MAX_MCP_POSTS_PER_DAY {
        let who = counted.who_has();
        return Err(tool_error(format!(
            "Quota exhausted: {who} already written {today} posts in this work in the last 24 hours; \
             allowed are {MAX_MCP_POSTS_PER_DAY}. Nothing stored; try again later."
        )));
    }
    let post = Post { id: 0, created: String::new(), author: author.clone(), body: a.body, sources: a.sources, refs };
    let post_id = store.add_post(id, at, &post).await?;
    let position = store.load_work(id).await?.nodes[at].posts.len().saturating_sub(1);
    let key = work.nodes[at].key;
    Ok(json!({
        "work": id, "node": at, "key": key, "post": position, "id": post_id, "author": author,
        "url": node_url(id, key),
    }))
}

#[derive(Deserialize)]
struct ManagePostArgs {
    work: WorkRef,
    id: i64,
    key: Option<i64>,
    /// Runde 30c: Fadenbeitrag statt Abschnittsbeitrag (nur `post_delete`).
    thread: Option<i64>,
    words: Option<Value>,
    persona: Option<String>,
}

/// `post_move`, `post_delete`, `post_to_idea` (Vertrag Runde 8, 3e): nur eigene Beiträge (Autor =
/// Persona-Handle aus `persona` bzw. sonst der Client als `@mcp:<name>`); ein fremder ergibt `isError`
/// „Nur eigene Beiträge …“, ohne Änderung und ohne Commit. Antwort wie die RPC-Methode, dazu `url`.
/// Eigentum über den Handle des Clients (Runde 23, siehe Moduldoku): mit Token der Tokenname samt Alt-Handle,
/// ohne Token der angegebene Name – mit `lokal-`, wenn er einem Token gehört. `persona` muss es nur geben und geht
/// nur ohne Token ([`acting_persona`]).
async fn manage_post(state: &AppState, client: &Client, name: &str, arguments: Value) -> Result<Value, Failure> {
    let a: ManagePostArgs = args(arguments.clone())?;
    let (work, _, _) = resolve(&state.store, &a.work).await?;
    let op = match name {
        "post_move" => {
            let key = write_key(&arguments, a.key, "arguments")?;
            crate::posts::Op::Move(crate::posts::Target::Key(key))
        }
        "post_delete" => crate::posts::Op::Delete,
        _ => {
            let words = match &a.words {
                Some(value) if !value.is_null() => crate::ideas::split_words(value, "words")
                    .map_err(|e| Failure::Args(format!("arguments: {}", e.message)))?,
                _ => return Err(Failure::Args("arguments: words is missing (at least one keyword)".into())),
            };
            crate::posts::Op::ToIdea(words)
        }
    };
    let origin = origin_of(&state.store, client, a.persona.as_deref()).await?;
    // Runde 23: Eigentum nur über den Handle des Clients; mit Token zählt für alte Beiträge auch sein Alt-Handle.
    // Handelt der Client als Persona (nur ohne Token), gilt allein deren Handle.
    let also: Vec<String> = match &origin {
        IdeaOrigin::Mcp { handle } => client.owned_handles().into_iter().filter(|h| h != handle).collect(),
        _ => Vec::new(),
    };
    let actor = crate::posts::Actor::own(origin, also);
    // Runde 30c: `post_delete` mit `thread` löscht einen eigenen Fadenbeitrag (ein Commit, nur eigene).
    if let Some(thread) = a.thread {
        if name != "post_delete" {
            return Err(Failure::Args("arguments: thread works only with post_delete".into()));
        }
        return Ok(crate::threads::delete_post(state, work, thread, a.id, &actor).await?);
    }
    let mut answer = crate::posts::act(state, work, a.id, op, &actor).await?;
    answer["work"] = json!(work);
    if let Some(idea) = answer.get_mut("idea") {
        super::ideas::add_url(idea);
    }
    answer["url"] = match answer["key"].as_i64() {
        Some(key) => json!(node_url(work, key)),
        None => Value::Null,
    };
    Ok(answer)
}

#[derive(Deserialize)]
struct AssetListArgs {
    work: Option<WorkRef>,
}

#[derive(Deserialize)]
struct AssetReadArgs {
    work: Option<WorkRef>,
    name: String,
    rev: Option<u32>,
    #[serde(default)]
    content: bool,
}

/// Werk aus `work` (id oder slug); fehlt es, das Vorgabewerk.
pub(crate) async fn work_or_default(state: &AppState, work: Option<&WorkRef>) -> Result<WorkId, Failure> {
    match work {
        Some(work) => Ok(resolve(&state.store, work).await?.0),
        None => Ok(state.default_work().await?),
    }
}

/// `list_assets`: Bilder des Werks wie `asset.list` (nur lesend).
async fn list_assets(state: &AppState, arguments: Value) -> Result<Value, Failure> {
    let a: AssetListArgs = args(arguments)?;
    let work = work_or_default(state, a.work.as_ref()).await?;
    Ok(json!({ "work": work, "assets": state.store.asset_list(work).await? }))
}

/// `read_asset`: Angaben einer Fassung und aller Fassungen, mit `content: true` auch der Inhalt.
async fn read_asset(state: &AppState, arguments: Value) -> Result<Value, Failure> {
    let a: AssetReadArgs = args(arguments)?;
    let work = work_or_default(state, a.work.as_ref()).await?;
    let revisions = state.store.asset_revisions(work, &a.name).await?;
    let revision = match a.rev {
        Some(rev) => revisions.iter().find(|r| r.number == rev).cloned().ok_or_else(|| {
            let numbers: Vec<String> = revisions.iter().map(|r| r.number.to_string()).collect();
            tool_error(format!("Image {}: revision {rev} does not exist; existing: {}", a.name, numbers.join(", ")))
        })?,
        None => revisions
            .iter()
            .find(|r| r.current)
            .cloned()
            .ok_or_else(|| Failure::Internal(format!("Image {}: no current revision", a.name)))?,
    };
    let mut answer = json!({
        "work": work, "name": a.name, "revision": revision, "revisions": revisions, "encoding": revision.encoding,
    });
    if a.content {
        answer["content"] = json!(state.store.asset_data(work, &a.name, Some(revision.number)).await?.content);
    }
    Ok(answer)
}

async fn list_personas(store: &Store) -> Result<Value, Failure> {
    let personas = personas(store).await?;
    // Lektor nach der Regel aus baruch-core (fest gewählt, sonst die aktive Persona mit dem größten Kontext).
    let lector_choice = store.lector_choice().await?;
    let lector_mode = lector_choice.mode();
    let probe = Work {
        kind: WorkKind::Book,
        title: String::new(),
        author: String::new(),
        nodes: vec![],
        personas,
        lector_choice,
    };
    let lector = probe.lector().map(|p| p.handle.clone());
    let list: Vec<Value> = probe
        .personas
        .iter()
        .map(|p| {
            json!({
                "handle": p.handle, "name": p.name, "role": p.role, "model": p.model,
                "context_length": p.context_length, "active": p.active,
                "lector": lector.as_deref() == Some(p.handle.as_str()),
            })
        })
        .collect();
    Ok(json!({ "personas": list, "lector": lector, "lector_mode": lector_mode }))
}

#[derive(Deserialize)]
struct AskArgs {
    work: WorkRef,
    key: Option<i64>,
    persona: String,
    question: Option<String>,
    mode: Option<String>,
    rev: Option<u32>,
    scope: Option<String>,
}

/// `ask_persona` (Runde 10): ask/review/propose über [`crate::assist::run`].
async fn ask_persona(state: &AppState, client: &Client, arguments: Value) -> Result<Value, Failure> {
    let a: AskArgs = args(arguments.clone())?;
    let key = write_key(&arguments, a.key, "arguments")?;
    let (work, _, _) = resolve(&state.store, &a.work).await?;
    let mode = match a.mode.as_deref().map(str::trim).filter(|m| !m.is_empty()) {
        None => crate::assist::Mode::Ask,
        Some(m) => match crate::assist::Mode::parse(m) {
            Some(mode @ (crate::assist::Mode::Ask | crate::assist::Mode::Review | crate::assist::Mode::Propose)) => {
                mode
            }
            _ => return Err(Failure::Args("arguments: mode ask, review or propose expected".into())),
        },
    };
    let scope = match a.scope.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        None | Some("section") => crate::assist::Scope::Section,
        Some("chapter") => crate::assist::Scope::Chapter,
        Some(_) => return Err(Failure::Args("arguments: scope section or chapter expected".into())),
    };
    let order = crate::assist::Order {
        work,
        key,
        persona: a.persona,
        mode,
        text: a.question.unwrap_or_default(),
        rev: a.rev,
        asker: Some(client.handle()),
        post_question: mode == crate::assist::Mode::Ask,
        sources: vec![],
        // Keine Websuche über MCP (docs/MCP.md, docs/Bedienung.md „Websuche für @belege“).
        web: false,
        scope,
        frame: Default::default(),
        links: Default::default(),
        basis: None,
    };
    let outcome = crate::assist::run(state, &order, None).await?;
    let mut answer = json!({
        "work": work, "key": key, "persona": outcome.persona, "mode": mode, "model": outcome.model,
        "text": outcome.text, "truncated": outcome.truncated, "cut": outcome.cut, "url": outcome.url,
        "scope": scope, "compressed": outcome.compressed,
    });
    // Runde 30b: geprüfte Fassung (Prüfgrundlage) wie bei der RPC-Methode.
    if let Some(basis) = &outcome.basis {
        answer["basis"] = json!(basis);
    }
    if let Some(post) = &outcome.post {
        answer["post_id"] = json!(post.id);
    }
    if let Some(question) = &outcome.question {
        answer["question_post_id"] = json!(question.id);
    }
    if let Some(rev) = outcome.rev {
        answer["rev"] = json!(rev);
        answer["status"] = json!("proposed");
    }
    // Runde 23: Rückmeldungen „Erledigt: Befund n“ wie bei RPC `persona.ask` (nur gesetzt, wenn es welche gibt).
    crate::assist::chat::add_done(&mut answer, std::slice::from_ref(&outcome));
    Ok(answer)
}

#[derive(Deserialize)]
struct ChatArgs {
    work: WorkRef,
    key: Option<i64>,
    thread: Option<i64>,
    message: String,
    persona: Option<String>,
}

/// `chat`: mit `key` Beitrag des Clients im Abschnitts-Chat, ohne `key` im Faden ohne Abschnitt (Runde 15); dann
/// die Antworten der erwähnten Personas mit einer Relay-Stufe.
async fn chat(state: &AppState, client: &Client, arguments: Value) -> Result<Value, Failure> {
    if arguments.get("node").is_some() {
        return Err(node_refused("arguments"));
    }
    let a: ChatArgs = args(arguments)?;
    let (work, _, _) = resolve(&state.store, &a.work).await?;
    let author = client.handle();
    let Some(key) = a.key else {
        let order = crate::threads::ThreadOrder {
            work,
            thread: a.thread,
            message: &a.message,
            author: &author,
            extra: a.persona.as_deref(),
            relay: true,
        };
        let outcome = crate::threads::chat(state, &order).await?;
        return Ok(crate::threads::outcome_json(&outcome));
    };
    if a.thread.is_some() {
        return Err(Failure::Args("arguments: thread only without key (thread without a section)".into()));
    }
    let order = crate::assist::ChatOrder {
        work,
        key,
        message: &a.message,
        rev: None,
        author: &author,
        extra: a.persona.as_deref(),
        // Keine Websuche über MCP (docs/MCP.md, docs/Bedienung.md „Websuche für @belege“).
        web: false,
        as_order: false,
        basis: crate::assist::basis::BasisChoice::Shown,
        // Über MCP ruft auch eine bloße Erwähnung in einer Antwort die Persona (Runde 15), eine Stufe.
        relay: true,
    };
    let outcome = crate::assist::chat(state, &order, None).await?;
    // Runde 23: Das Relay läuft ganz in `assist::chat` (ein Weg, `ChatOrder::relay`); hier nur noch die Namen derer,
    // die über das Relay geantwortet haben.
    let answered: Vec<&str> = outcome.answers.iter().map(|a| a.persona.as_str()).collect();
    let relayed: Vec<&String> = outcome.relayed.iter().filter(|h| answered.contains(&h.as_str())).collect();
    let mut answer = crate::assist::rpc::chat_json(&outcome);
    answer["work"] = json!(work);
    answer["key"] = json!(key);
    answer["relayed"] = json!(relayed);
    Ok(answer)
}

#[cfg(test)]
mod tests {
    use baruch_core::NodeKind;

    use super::*;

    fn revision(number: u32, status: RevisionStatus) -> Revision {
        Revision {
            number,
            body: format!("Fassung {number}"),
            author: Author::Human { name: "Autor".into() },
            status,
            note: String::new(),
        }
    }

    /// Abschnitt mit den Nummern 1, 2, 4 (Revision 3 gelöscht): Stelle und Nummer passen ab 4 nicht
    /// mehr zusammen. Bis revtree läuft, lässt `load_work` keine Lücken zu, daher ein Abschnitt im
    /// Speicher statt einer Lücke in SQLite.
    fn node_with_gap(current: u32) -> Node {
        let status = |n| if n == current { RevisionStatus::Current } else { RevisionStatus::Superseded };
        Node {
            id: 5,
            key: 0,
            parent: None,
            number: "1".into(),
            title: "Mit Lücke".into(),
            kind: NodeKind::Chapter,
            revisions: [1, 2, 4].into_iter().map(|n| revision(n, status(n))).collect(),
            posts: Vec::new(),
            findings: 0,
        }
    }

    #[test]
    fn revisions_are_found_by_number_even_with_a_gap() {
        let node = node_with_gap(4);
        assert_eq!(revision_by_number(&node, 4).map(|r| r.body.as_str()), Some("Fassung 4"), "Nummer 4 an Stelle 2");
        assert!(revision_by_number(&node, 3).is_none(), "die gelöschte Nummer 3 gibt es nicht");
        assert_eq!(current_number(&node), Some(4), "outline.current_rev ist die Nummer, nicht Stelle + 1 (= 3)");
    }

    #[test]
    fn read_node_shows_the_current_number_and_refuses_a_missing_one() {
        let node = node_with_gap(4);
        assert_eq!(shown_revision(&node, None).unwrap(), (4, "Fassung 4"), "ohne rev die aktuelle Fassung 4");
        assert_eq!(shown_revision(&node, Some(2)).unwrap(), (2, "Fassung 2"), "rev 2 nach Nummer");
        let err = shown_revision(&node, Some(3)).unwrap_err();
        assert!(
            matches!(&err, StoreError::UnknownRevision { node: 5, rev: 3, existing } if existing == &[1, 2, 4]),
            "Lücke ist eine unbekannte Revision, vorhanden sind 1, 2 und 4: {err:?}"
        );
        assert_eq!(
            err.to_string(),
            "Fassung 3 gibt es nicht; vorhanden: 1, 2, 4",
            "die Meldung nennt die vorhandenen Nummern statt eines lückenlosen Bereichs"
        );
        let node = node_with_gap(2);
        assert_eq!(shown_revision(&node, None).unwrap(), (2, "Fassung 2"), "aktuell kann auch eine ältere sein");
        let mut empty = node_with_gap(0);
        assert_eq!(shown_revision(&empty, None).unwrap(), (4, "Fassung 4"), "ohne aktuelle die höchste Nummer");
        empty.revisions.clear();
        assert_eq!(shown_revision(&empty, None).unwrap(), (0, ""), "ohne Revisionen leer");
    }
}
