// SPDX-License-Identifier: AGPL-3.0-or-later
//! JSON-RPC 2.0 unter `POST /rpc`: der Kern als Dienst. Dieselben Methoden bedient `POST /act`
//! für Formulare ohne JavaScript.
//!
//! Jede Methode nimmt optional `work` (Werk-id, Zahl); fehlt es, gilt das Vorgabewerk
//! ([`AppState::default_work`]: jetzt die kleinste id), ein unbekanntes Werk ergibt `INVALID_PARAMS`.
//! `work.*` außer `work.get`/`work.export` beantwortet [`crate::works`] vor der Wahl des Vorgabewerks
//! (`work.update`/`work.delete` verlangen `work`), `outline.*` [`crate::outline`]. Ebenso ohne Werk
//! laufen `persona.*` ([`crate::personas`]) und `idea.*` ([`crate::ideas`]), Runde 7.
//!
//! Methoden:
//! - `work.list/meta/create/import/update/delete`: Werke verwalten ([`crate::works`], `docs/RPC.md` „Werke“)
//! - `work.get` → Werk mit Gliederung, Revisionen, Personas und Lektor
//! - `work.export` → `{typst}`: Werkvorlage und aktuelle Texte als eine Typst-Datei
//! - `node.get {node}` → ein Abschnitt mit Revisionen und Chat
//! - `revision.accept {node, rev}` / `revision.reject {node, rev}` → `{node, rev}` (danach zu zeigen);
//!   Übernehmen ist Umschalten auf einen Vorschlag, Verwerfen bleibt ein Status für Vorschläge
//! - `revision.set_current {node, rev}` → `{node, rev}`: Markierung „aktuell“ auf jede andere Fassung.
//!   Beim Umschalten und Übernehmen folgt die Gliederung der Überschriftszeile der Fassung
//!   (`Store::switch_to`); bringt sie eine andere Ebene mit, trägt die Antwort `outline`
//!   (Gliederungsantwort ohne Spiegel), und es entsteht genau ein Commit (`mirror::record_switch`)
//! - `revision.save {node, body, based_on?, based_on_amend?, mode?}` → neue Revision vom Menschen, sofort aktuell, Elternteil
//!   `based_on` (fehlt: die aktuelle Fassung); der Editor speichert über `outline.save`
//! - `outline.*`: Gliederung umstellen ([`crate::outline`], `docs/RPC.md` „Gliederung“)
//! - `revision.update {node, rev, body?, note?}` → `{node, rev, changed}`: Fassung ändern (nur Mensch)
//! - `revision.delete {node, rev, confirm: true}` → `{node, rev, deleted, reparented}`: Fassung löschen
//!   (nur Mensch, nicht die aktuelle), Kinder hängen am Elternteil; ohne `confirm: true` → `INVALID_PARAMS`
//! - `revision.tree {node?, chapter?}` → `{revisions}` bzw. `{sections}`: Revisionsbaum ohne Texte
//! - `post.add {node, body}` → `{node, key, id}`: Beitrag im Abschnitts-Chat
//! - `post.delete {id, confirm: true}`, `post.move {id, target_key | target}`, `post.to_idea {id, words}`:
//!   Beiträge verwalten ([`crate::posts`], Runde 8; nur Mensch, je ein Spiegel-Commit)
//! - `ui.render {key?, node?, rev?, view?, pick?, lq?}` → `{tree, editor, assistants, url, title}` (HTML der
//!   drei Bereiche; `key` geht vor `node`, `url` ist die Adresse in Key-Form `/?work=W&key=K&rev=R…`)
//! - `library.list` → `{titles: [TitleEntry]}` (Titel mit Zitaten)
//! - `library.search {query, kinds?, work?, all_revisions?, limit?}` → `{hits: [SearchHit]}`:
//!   Volltextsuche über Werk und Bibliothek, das Werkzeug des Lektors; `work` ist das aktuelle Werk.
//! - `literature.render`, `highlight.list/add/update/delete/cite`: Literatur-Popup und Hervorhebungen,
//!   beantwortet von [`crate::literature`] (`docs/RPC.md`, „Literatur und Hervorhebungen“).
//! - `wysiwyg.parse/serialize/style/diff`: Satzbild ([`crate::wysiwyg`], `docs/RPC.md` „Satzbild“);
//!   Bild-Inseln tragen `<img>` mit dem Alternativtext aus `Store::asset_list`
//! - `numbering.schemes/preview/set`: Gliederungsschemata ([`crate::numbering`], `docs/RPC.md`
//!   „Gliederungsschemata“); `outline.set_numbering` bleibt
//! - `asset.list/get/revisions/add/replace/set_current/update/delete`: Bilder als Base64- bzw.
//!   SVG-Text ([`crate::assets`], `docs/RPC.md` „Bilder“); nur Mensch, nicht über `/act` und MCP
//! - `persona.list/set_color`: Farben der Personas ([`crate::personas`], `docs/RPC.md` „Personas“);
//!   `set_color` nur Mensch, nicht über `/act` und MCP
//! - `idea.create/get/list/search/update/delete/restore/process`: Einfälle ([`crate::ideas`],
//!   `docs/RPC.md` „Einfälle“); Herkunft immer der Mensch, `delete` verlangt `confirm: true`
//! - `idea.map/links/link/unlink/link_restore/map_pos_set/map_pos_reset`: Gedankenkarte ([`crate::idea_map`],
//!   Runde 25, `docs/RPC.md` „Gedankenkarte“); Herkunft immer der Mensch
//! - `finding.list/get/set_status/decide`, `finding.matrix`: Befunde ([`crate::findings`], Runde 28, `docs/RPC.md`
//!   „Befunde“); Status setzt nur der Mensch
//! - `preview.typst {key?, page?}`: Typst-Satz des Werks mit Diagnosen ([`crate::typeset`], Runde 8), nur lesend
//! - `persona.ask/review/propose/chat/prompt_get/set_prompt/lector_get/set_lector`: Personas mit
//!   Modellaufruf und Rollen-Prompts ([`crate::assist`] über [`crate::personas`], Runde 10)
//! - `model.list/status/probe`: Modell-Backend Eliezer ([`crate::models`], Runde 10); nur Mensch,
//!   nicht über `/act` und MCP
//! - `scripture.check {text, work?, module?}`: Bibelstellen prüfen; Wortlaut nur mit gewählter SWORD-Bibel,
//!   sonst „ungeprüft“ ([`crate::scripture`], Runde 10, Wortlaut 2026-11-06), nur lesend
//! - `scripture.map {ref, from?, to?}`, `scripture.versifications {}`: Stellen zwischen Verszählungen umrechnen
//!   ([`crate::scripture_map`], Runde 17a), nur lesend
//! - `bible.modules/set_dirs/set_work/lookup/check/search`: SWORD-Bibeln ([`crate::bibles`], 2026-11-06);
//!   `set_dirs`/`set_work` nur Mensch, nicht über MCP
//! - `bible.marks {work?, key, rev?}` (Prüfanzeige, nur lesend) und `bible.cipher_key_set/_delete` (Schlüssel
//!   verschlüsselter SWORD-Module, nur Mensch, nicht über `/act` und MCP; [`crate::bibles`], Runde 17c)
//! - `work.timing/timing_set`, `timing.count`: Zeitwächter ([`crate::timing`], Runde 16b, `docs/RPC.md`
//!   „Zeitwächter“); `timing_set` nur Mensch, nicht über `/act` und MCP
//! - `work.exchange_export/exchange_preview/exchange_import`: Austausch zwischen Instanzen ([`crate::exchange`],
//!   Runde 26, `docs/RPC.md` „Austausch“), ohne Vorgabewerk; Einlesen nur Mensch, nicht über `/act` und MCP
//! - `instance.get/set_name`: Kennung und Name dieser Instanz ([`crate::instance`], Runde 26); `set_name` nur
//!   Mensch, nicht über `/act` und MCP
//! - `search.find/preview/replace/undo`: im Werk suchen und ersetzen ([`crate::search`], Runde 30a, `docs/RPC.md`
//!   „Suchen und Ersetzen“); `replace`/`undo` nur Mensch, nicht über `/act` und MCP
//! - `xref.targets/resolve/candidates/convert/broken`: Querverweise ([`crate::xref`], Runde 30a, `docs/RPC.md`
//!   „Querverweise“); `convert` nur Mensch
//! - `solution.list/adopt/variants/reject_siblings/compare`: Lösungsvorschläge und Varianten
//!   ([`crate::assist::solutions`], Runde 30b, `docs/RPC.md` „Lösungsvorschläge und Varianten“); Vorschläge nur als
//!   `proposed`, `reject_siblings` nur Mensch
//! - `mcp_token.list/create/revoke`: Tokens für den MCP-Zugang von anderen Rechnern
//!   ([`crate::mcp::tokens`], Runde 10); nur Mensch, nicht über `/act` und MCP
//!
//! Titel von Gliederungspunkten prüft der Store (`baruch_store::check_title`, Runde 7): leer, mit
//! Zeilenumbruch oder über 200 Zeichen → `INVALID_PARAMS` (`StoreError::Title`), nichts geändert.
//!
//! Übernehmen, Umschalten, Speichern, Ändern und Löschen schreiben atomar in die Datenbank und
//! spiegeln danach in Git; die Antwort trägt dann `commit` (Commit-Id) oder `mirror` (Fehlertext des
//! Spiegels). Alles läuft unter [`AppState::write_lock`], damit gleichzeitige Änderungen je ihren
//! eigenen Commit bekommen.
//!
//! Abschnitte: `node` ist die Stelle (`seq`), die jede Gliederungsänderung neu vergibt. Alle Methoden
//! mit `node` nehmen zusätzlich `key` (fester Schlüssel, `Node::key`); ist er da, gilt er statt
//! `node` ([`resolve_keys`]). Formulare und Editor schicken ihn immer mit.
//!
//! Revisionen werden über ihre Nummer angesprochen, nie über „Stelle ± 1“ (nach dem Löschen hat die
//! Nummernfolge Lücken). Assistenten legen nur Vorschläge an; Speichern, Umschalten, Ändern und
//! Löschen sind dem Menschen vorbehalten und gibt es darum nicht über MCP.

use baruch_core::{Post, Work};
use baruch_library::{LibraryError, SearchHit, SearchOptions, TitleEntry};
use baruch_store::{NodeRevisions, NumberingInfo, RevisionEdit, StoreError, WorkId};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::view::{self, Location, Settings};
use crate::{AppState, mirror};

pub const PARSE_ERROR: i64 = -32700;
pub const INVALID_REQUEST: i64 = -32600;
pub const METHOD_NOT_FOUND: i64 = -32601;
pub const INVALID_PARAMS: i64 = -32602;
pub const INTERNAL: i64 = -32603;
/// Fachlicher Fehler (z. B. „nur Vorschläge lassen sich übernehmen“).
pub const REFUSED: i64 = -32000;
/// Bedeutungssuche ausgefallen (Runde 19c, `idea.search` mit `mode: "rag"`): Einbettung/Index nicht nutzbar; die Nachricht nennt den Grund.
pub const RAG_UNAVAILABLE: i64 = -32001;

#[derive(Debug)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
}

impl RpcError {
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        Self { code, message: message.into() }
    }
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for RpcError {}

#[derive(Deserialize)]
struct NodeParams {
    node: usize,
}

#[derive(Deserialize)]
struct RevParams {
    node: usize,
    /// Revisionsnummer ab 1.
    rev: u32,
}

/// `revision.delete`; `confirm` muss `true` sein (vorher geprüft): Löschen ist endgültig (der Text
/// bleibt nur im Spiegel-Commit).
#[derive(Deserialize)]
struct DeleteParams {
    node: usize,
    rev: u32,
}

#[derive(Deserialize)]
struct BodyParams {
    node: usize,
    body: String,
}

/// Übernimmt der Mensch einen Vorschlag mit Markdown-Überschriften (`# Titel`, Nutzer 2026-11-07), gilt
/// dieselbe Regel wie beim Speichern: Der Vorschlag selbst bleibt unverändert (übernommen, im Revisionsbaum),
/// darauf folgt eine Fassung des Menschen mit den umgewandelten Überschriften, und die Gliederung gleicht
/// sich ab wie beim Speichern im Editor (weitere Überschriften werden neue Punkte; kein Zusammenführen).
/// Geht das nicht (z. B. eine Ebene, die die Gliederung nicht zulässt), bleibt es beim Übernommenen und
/// `markdown` in der Antwort sagt, warum.
async fn accept_markdown_headings(
    state: &AppState,
    held: &mirror::WriteGuard<'_>,
    work: WorkId,
    accepted: &baruch_store::Change,
    answer: &mut Value,
) -> Result<(), RpcError> {
    let store = &state.store;
    let loaded = store.load_work(work).await?;
    let Some(body) = loaded.nodes.get(accepted.node).and_then(|n| n.revision(accepted.rev)).map(|r| r.body.clone())
    else {
        return Ok(());
    };
    let converted = baruch_typst::markdown_headings(&body);
    if converted == body.as_str() {
        return Ok(());
    }
    let allow = baruch_store::Restructure { split: true, merge: false };
    match store.outline_save(work, accepted.node, &converted, Some(accepted.rev), allow, mirror::HUMAN_NAME).await {
        Ok(saved) => {
            let shown = saved.outline.as_ref().map_or(accepted.node, |o| o.node);
            let rev = store.load_work(work).await?.nodes.get(shown).and_then(|n| n.current_number());
            answer["node"] = json!(shown);
            answer["rev"] = json!(rev);
            answer["markdown"] = json!("Markdown-Überschriften umgewandelt.");
            match (&saved.outline, &saved.change) {
                (Some(outline), _) => {
                    answer["outline"] = crate::outline::outline_answer(outline);
                    add_mirror(answer, mirror::record_outline(state, held, outline).await);
                }
                (None, Some(change)) => add_mirror(answer, mirror::record(state, held, change).await),
                (None, None) => {}
            }
        }
        Err(e) => {
            answer["markdown"] = json!(format!("Markdown-Überschriften nicht umgewandelt: {e}"));
        }
    }
    Ok(())
}

#[derive(Deserialize)]
struct SaveParams {
    node: usize,
    body: String,
    /// Fassung, auf der die neue beruht (die gezeigte); fehlt: die aktuelle.
    based_on: Option<u32>,
    /// Nachbesserung R22: Stand der Basis, den der Client geladen hatte; fehlt: nur die Fassungsnummer zählt.
    based_on_amend: Option<u32>,
}

#[derive(Deserialize)]
struct UpdateParams {
    node: usize,
    rev: u32,
    body: Option<String>,
    note: Option<String>,
    /// Runde 23a (Codereview web-3): Stand der Fassung, den der Client geladen hatte. Steht sie inzwischen auf einem
    /// anderen Stand, wird nichts geändert (-32000 „Veraltet: …“); fehlt der Wert, wird nicht geprüft.
    based_on_amend: Option<u32>,
}

#[derive(Deserialize)]
struct TreeParams {
    node: Option<usize>,
    /// `true`: das ganze Kapitel des Abschnitts.
    #[serde(default)]
    chapter: bool,
}

#[derive(Deserialize)]
struct SearchParams {
    query: String,
    #[serde(flatten)]
    options: SearchOptions,
}

fn params<T: for<'de> Deserialize<'de>>(value: Value) -> Result<T, RpcError> {
    let value = if value.is_null() { json!({}) } else { value };
    serde_json::from_value(value).map_err(|e| RpcError::new(INVALID_PARAMS, e.to_string()))
}

impl From<StoreError> for RpcError {
    fn from(e: StoreError) -> Self {
        let code = match e {
            StoreError::UnknownWork(_)
            | StoreError::UnknownNode { .. }
            | StoreError::UnknownRevision { .. }
            | StoreError::EmptyPost
            | StoreError::Import(_) => INVALID_PARAMS,
            StoreError::Refused(_)
            | StoreError::AlreadyImported { .. }
            | StoreError::DuplicateTitle { .. }
            | StoreError::Heading(_)
            | StoreError::GoneNode { .. } => REFUSED,
            StoreError::EmptyTitle => INVALID_PARAMS,
            StoreError::Numbering(_) => INVALID_PARAMS,
            StoreError::UnknownAsset { .. } => INVALID_PARAMS,
            StoreError::Asset(_) => INVALID_PARAMS,
            StoreError::Title(_) => INVALID_PARAMS,
            StoreError::UnknownPersona(_) | StoreError::Color(_) => INVALID_PARAMS,
            StoreError::UnknownIdea(_) | StoreError::Idea(_) => INVALID_PARAMS,
            StoreError::UnknownPost { .. } | StoreError::UnknownRequest { .. } => INVALID_PARAMS,
            StoreError::Exchange(_) => INVALID_PARAMS,
            // Runde 28: Befunde und Beitragsfassungen.
            StoreError::UnknownFinding { .. } | StoreError::Finding(_) | StoreError::PostEdit(_) => INVALID_PARAMS,
            // Runde 30a: werkweites Ersetzen abgelehnt bzw. noch nicht umgesetzt.
            StoreError::Replace(_) => REFUSED,
            // Runde 29: Satz-Einstellungen ungültig (alle Meldungen); `typeset_settings` bildet Ablehnungen selbst ab.
            StoreError::Typeset(_) => INVALID_PARAMS,
            // Runde 31: Glossar und Register (abgelehnt, unbekannt oder noch nicht umgesetzt).
            StoreError::Glossary(_)
            | StoreError::UnknownGlossaryTerm { .. }
            | StoreError::UnknownIndexProposal { .. } => INVALID_PARAMS,
            // Runde 23 (Schnittstelle S1): Umschlagblock abgelehnt (web) bzw. Antrag veraltet (mcp-namen).
            StoreError::CoverBlock(_) => INVALID_PARAMS,
            StoreError::Stale(_) => REFUSED,
            StoreError::Db(_) | StoreError::Migrate(_) | StoreError::Io { .. } => INTERNAL,
        };
        RpcError::new(code, e.to_string())
    }
}

impl From<LibraryError> for RpcError {
    fn from(e: LibraryError) -> Self {
        let code = match e {
            LibraryError::EmptyQuery | LibraryError::UnknownTitle(_) => INVALID_PARAMS,
            LibraryError::Db(_) | LibraryError::Migrate(_) => INTERNAL,
        };
        RpcError::new(code, e.to_string())
    }
}

/// Werk aus dem Parameter `work` (Zahl); fehlt er, das Vorgabewerk. Unbekannt → `INVALID_PARAMS`.
pub async fn work_param(state: &AppState, params: &Value) -> Result<WorkId, RpcError> {
    match params.get("work") {
        None | Some(Value::Null) => Ok(state.default_work().await?),
        Some(value) => {
            let id = value.as_i64().ok_or_else(|| RpcError::new(INVALID_PARAMS, "work: Werk-id (Zahl) erwartet"))?;
            state.store.work_slug(id).await?;
            Ok(id)
        }
    }
}

/// Fester Schlüssel aus `params[name]` (Zahl oder Ziffern als Text, Formulare); fehlt, `null` oder
/// leer → `None`.
fn key_value(params: &Value, name: &str) -> Result<Option<i64>, RpcError> {
    let wrong = || RpcError::new(INVALID_PARAMS, format!("{name}: fester Schlüssel (Zahl) erwartet"));
    match params.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => n.as_i64().map(Some).ok_or_else(wrong),
        Some(Value::String(s)) if s.trim().is_empty() => Ok(None),
        Some(Value::String(s)) => s.trim().parse().map(Some).map_err(|_| wrong()),
        Some(_) => Err(wrong()),
    }
}

/// Feste Schlüssel in Stellen übersetzen: `key` → `node`, `target_key` → `target`.
///
/// Jede Gliederungsänderung vergibt die Stellen (`seq`) neu; ein veralteter Tab oder ein Assistent,
/// der die Gliederung vorher gelesen hat, träfe mit der alten Stelle einen fremden Abschnitt. Der
/// Schlüssel (`Node::key`, Datenbank-id) bleibt dagegen gleich. Ist er angegeben, **geht er vor**:
/// Die Stelle wird aus ihm bestimmt, eine mitgeschickte (vielleicht veraltete) Stelle zählt nicht.
/// Gibt es den Punkt nicht mehr → `GoneNode` (`REFUSED`). Schreibwege rufen das unter
/// [`AppState::write_lock`] auf, damit sich die Gliederung zwischen Übersetzen und Schreiben nicht ändert.
pub async fn resolve_keys(state: &AppState, work: WorkId, mut params: Value) -> Result<Value, RpcError> {
    for (key_name, node_name) in [("key", "node"), ("target_key", "target")] {
        if let Some(key) = key_value(&params, key_name)? {
            let seq = state.store.node_seq(work, key).await?;
            if !params.is_object() {
                params = json!({});
            }
            params[node_name] = json!(seq);
        }
    }
    Ok(params)
}

/// Stelle nach einer Änderung: die aktuelle Revision des Abschnitts (Nummer, nicht Stelle).
fn shown_current(work: &Work, node: usize) -> Value {
    let rev = work.nodes.get(node).and_then(|n| n.current_number()).unwrap_or(1);
    json!({ "node": node, "rev": rev })
}

/// Nummer der aktuellen Fassung (nach einer Änderung neu gelesen).
async fn current_of(state: &AppState, work: WorkId, node: usize) -> Result<Option<u32>, RpcError> {
    let work = state.store.load_work(work).await?;
    Ok(work.nodes.get(node).and_then(|n| n.current_number()))
}

/// Eine Fassung im Baum als JSON, mit `current: bool`.
fn info_json(info: &baruch_store::RevisionInfo) -> Value {
    let mut value = json!(info);
    value["current"] = json!(info.status == baruch_core::RevisionStatus::Current);
    value
}

/// Kapitel (bzw. Teil) des Knotens und alle Knoten darunter, in Baumreihenfolge.
fn chapter_members(work: &Work, node: usize) -> Vec<usize> {
    let mut top = node;
    let mut steps = 0;
    while let Some(n) = work.nodes.get(top) {
        match (n.kind, n.parent) {
            (baruch_core::NodeKind::Section, Some(parent)) if steps < work.nodes.len() => {
                top = parent;
                steps += 1;
            }
            _ => break,
        }
    }
    let mut out = Vec::new();
    let mut stack = vec![top];
    while let Some(id) = stack.pop() {
        if out.contains(&id) || out.len() > work.nodes.len() {
            continue;
        }
        out.push(id);
        let children: Vec<usize> = work.children(Some(id)).map(|c| c.id).collect();
        stack.extend(children.into_iter().rev());
    }
    out
}

/// `revision.tree`: ein Abschnitt (`{revisions}`) oder ein Kapitel bzw. das ganze Werk (`{sections}`).
fn tree_answer(work: &Work, tree: &[NodeRevisions], node: Option<usize>, chapter: bool) -> Value {
    let infos = |id: usize| tree.iter().find(|n| n.node == id).map_or(&[][..], |n| n.revisions.as_slice());
    let section = |id: usize| {
        let n = &work.nodes[id];
        json!({
            "node": id, "number": n.number, "title": n.title, "current": n.current_number(),
            "revisions": infos(id).iter().map(info_json).collect::<Vec<_>>(),
        })
    };
    match (node, chapter) {
        (Some(id), false) => json!({ "revisions": infos(id).iter().map(info_json).collect::<Vec<_>>() }),
        (Some(id), true) => {
            json!({ "sections": chapter_members(work, id).into_iter().map(section).collect::<Vec<_>>() })
        }
        (None, _) => json!({ "sections": (0..work.nodes.len()).map(section).collect::<Vec<_>>() }),
    }
}

/// Runde 22: Parameter `mode` von `revision.save`/`outline.save` (fehlt = „new“, unbekannt → -32602).
pub(crate) fn save_mode(params: &Value) -> Result<baruch_store::SaveMode, RpcError> {
    let mode = match params.get("mode") {
        None | Some(Value::Null) => None,
        Some(Value::String(s)) if s.trim().is_empty() => None,
        Some(Value::String(s)) => Some(s.trim().to_owned()),
        Some(_) => return Err(RpcError::new(INVALID_PARAMS, "mode: „auto“ oder „new“ erwartet")),
    };
    baruch_store::SaveMode::from_param(mode.as_deref()).map_err(|e| RpcError::new(INVALID_PARAMS, e))
}

/// Runde 22: Antwort beim Speichern ergänzen – `saved` („amended“ | „new“ | „unchanged“), bei „amended“ `amend_seq`,
/// bei einer neuen Fassung im Modus „auto“ `reason` und `reason_label`.
pub(crate) fn add_saved(
    answer: &mut Value,
    change: Option<&baruch_store::Change>,
    reason: Option<baruch_store::NewReason>,
) {
    match change {
        Some(change) if change.kind == baruch_store::ChangeKind::Amended => {
            answer["saved"] = json!("amended");
            answer["amend_seq"] = json!(change.amend_seq);
        }
        Some(_) => answer["saved"] = json!("new"),
        None => answer["saved"] = json!("unchanged"),
    }
    if let Some(reason) = reason {
        answer["reason"] = json!(reason.id());
        answer["reason_label"] = json!(reason.label());
    }
}

/// Ergebnis des Spiegels in die Antwort: `commit` bei neuem Commit, `mirror` bei Fehler.
pub(crate) fn add_mirror(answer: &mut Value, result: Result<Option<String>, String>) {
    match result {
        Ok(Some(commit)) => answer["commit"] = json!(commit),
        Ok(None) => {}
        Err(e) => answer["mirror"] = json!(e),
    }
}

/// Inhalt des Literatur-Popups: alle Titel, dazu die Treffer, wenn gesucht wurde. Dazu der
/// Revisionsbaum des Werks (`revisions`, ohne Texte) für Editor und Baumansicht: Elternteil, Zeit
/// und „geändert“ stehen nicht in `baruch_core::Revision`; und die Nummerierung des Werks
/// (`numbering`) für das Formular im Kopf der Gliederung (die Werkvorlage steht nicht in `Work`).
pub struct LibraryView {
    pub titles: Vec<TitleEntry>,
    pub query: Option<String>,
    pub hits: Vec<SearchHit>,
    pub revisions: Vec<NodeRevisions>,
    pub numbering: Option<NumberingInfo>,
    pub persona_colors: Vec<baruch_store::PersonaColor>,
    pub image_alts: std::collections::HashMap<String, String>,
    /// Runde 21c: Bilder des Werks und der Vorlage für Editorkopf, Einfügen und fehlende Bilder (vorher unter einem
    /// reservierten Schlüssel in `image_alts`).
    pub images: crate::wysiwyg::ImageView,
    /// Zahl der offenen Anträge zur Gliederung (Hinweis im Kopf des Baums, Migration 0012).
    pub outline_requests: usize,
    /// Umschlag des Werks (Runde 11, [`crate::cover::cover_info`]): `{name, caption, alt, url, thumb}`.
    pub cover: Option<Value>,
    /// Runde 16b: Zeitwächter und Freigaben der Personas für die Arbeitsseite ([`crate::view::extras`]).
    pub extras: view::ViewExtras,
    /// Runde 23a: fertiges, maskiertes HTML des Kastens „Bilder des Werks“ zur Suche `query` (leer ohne Suche oder
    /// ohne Treffer-Kasten); die Arbeitsseite setzt es unter die Treffer (`fragments/library.html`).
    pub images_html: String,
}

/// Titel laden und, falls `lq` Wörter enthält, das Werk `work` und die Bibliothek durchsuchen; dazu
/// den Revisionsbaum des Werks.
pub async fn library_view(state: &AppState, work: WorkId, lq: Option<&str>) -> Result<LibraryView, RpcError> {
    let titles = baruch_library::list_titles(state.store.pool()).await?;
    let query = lq.map(str::trim).filter(|q| !q.is_empty()).map(str::to_owned);
    let hits = match &query {
        Some(q) => {
            let options = SearchOptions { work: Some(work), ..SearchOptions::default() };
            match baruch_library::search(state.store.pool(), q, &options).await {
                Ok(hits) => hits,
                Err(LibraryError::EmptyQuery) => Vec::new(),
                Err(e) => return Err(e.into()),
            }
        }
        None => Vec::new(),
    };
    let revisions = state.store.revision_tree(work).await?;
    let numbering = Some(state.store.outline_numbering(work).await?);
    let persona_colors = state.store.persona_colors().await?;
    let (image_alts, images) = crate::wysiwyg::image_view(state, work).await?;
    let outline_requests = state.store.open_outline_requests(work).await?;
    let cover = crate::cover::cover_info(state, work).await?;
    let extras = view::extras(state, work).await?;
    let images_html = crate::literature::image_hits_box(state, work, query.as_deref()).await;
    Ok(LibraryView {
        images_html,
        titles,
        query,
        hits,
        revisions,
        numbering,
        persona_colors,
        image_alts,
        images,
        outline_requests,
        cover,
        extras,
    })
}

pub async fn call(state: &AppState, settings: &Settings, method: &str, params_value: Value) -> Result<Value, RpcError> {
    if crate::exchange::handles(method) {
        return crate::exchange::call(state, method, params_value).await;
    }
    if crate::instance::handles(method) {
        return crate::instance::call(state, method, params_value).await;
    }
    if crate::works::handles(method) {
        return crate::works::call(state, method, params_value).await;
    }
    // Runde 21 (Schnittstelle R21-S1): PDF/EPUB-Import, global ohne Werk.
    if crate::pdf_import::handles(method) {
        return crate::pdf_import::call(state, method, params_value).await;
    }
    if crate::personas::handles(method) {
        return crate::personas::call(state, method, params_value).await;
    }
    if crate::ideas::handles(method) {
        return crate::ideas::call(state, method, params_value).await;
    }
    // Runde 28: Befunde (`finding.*`) und ihre Übersicht (`finding.matrix`).
    if crate::findings::rpc::handles(method) {
        return crate::findings::rpc::call(state, method, params_value).await;
    }
    if crate::findings::page::handles(method) {
        return crate::findings::page::call(state, method, params_value).await;
    }
    if crate::idea_map::handles(method) {
        return crate::idea_map::call(state, method, params_value).await;
    }
    if crate::menu::handles(method) {
        return crate::menu::call(state, method, params_value).await;
    }
    if crate::settings::handles(method) {
        return crate::settings::call(state, settings, method, params_value).await;
    }
    if crate::connections::handles(method) {
        return crate::connections::call(state, method, params_value).await;
    }
    if crate::websearch::handles(method) {
        return crate::websearch::call(state, method, params_value).await;
    }
    if crate::threads::handles(method) {
        return crate::threads::call(state, method, params_value).await;
    }
    if crate::sync::handles(method) {
        return crate::sync::call(state, method, params_value).await;
    }
    if crate::rag::handles(method) {
        return crate::rag::call(state, method, params_value).await;
    }
    // Runde 10: Modell-Backend (`model.*`), Stellenprüfung (`scripture.check`) und MCP-Tokens
    // (`mcp_token.*`) brauchen kein Werk.
    if crate::models::handles(method) {
        return crate::models::call(state, method, params_value).await;
    }
    if crate::scripture::handles(method) {
        return crate::scripture::call(state, method, params_value).await;
    }
    if crate::scripture_map::handles(method) {
        return crate::scripture_map::call(state, method, params_value).await;
    }
    if crate::bibles::handles(method) {
        return crate::bibles::call(state, method, params_value).await;
    }
    if crate::mcp::tokens::handles(method) {
        return crate::mcp::tokens::call(state, method, params_value).await;
    }
    // Runde 20 (Schnittstelle R20-S1): Einstellung für EPUBCheck, werklos.
    if crate::epub_check::handles_global(method) {
        return crate::epub_check::call_global(state, method, params_value).await;
    }
    let store = &state.store;
    // Runde 23a (Codereview web-2): `ui.render` ohne `work` zeichnet das Werk aus dem Cookie `baruch_work` (wie die
    // Seite `/`), nicht das Vorgabewerk. Sonst zeigte „Zurück“ auf die Adresse `/` ohne Hinweis ein anderes Werk.
    let remembered = match (method, params_value.get("work"), settings.work) {
        ("ui.render", None | Some(Value::Null), Some(work)) => store.work_slug(work).await.is_ok().then_some(work),
        _ => None,
    };
    let work_id = match remembered {
        Some(work) => work,
        None => work_param(state, &params_value).await?,
    };
    if crate::posts::handles(method) {
        return crate::posts::call(state, work_id, method, params_value).await;
    }
    match method {
        "work.get" => {
            let work = store.load_work(work_id).await?;
            let lector = work.lector().map(|p| p.handle.clone());
            let cover = crate::cover::cover_info(state, work_id).await?;
            Ok(json!({ "work": work, "lector": lector, "cover": cover }))
        }
        "work.export" => Ok(json!({ "typst": store.export_typst(work_id).await? })),
        "node.get" => {
            let p: NodeParams = params(resolve_keys(state, work_id, params_value).await?)?;
            let work = store.load_work(work_id).await?;
            let Some(node) = work.nodes.get(p.node) else {
                return Err(StoreError::UnknownNode { work: work_id, node: p.node }.into());
            };
            // Runde 22 (Integrator): jede Fassung trägt ihren Stand (`amend_seq`) und bei Vorschlägen den Stand der
            // Basis (`based_on_amend`) wie in revision.tree.
            let mut answer = json!(node);
            let tree = store.revision_tree(work_id).await?;
            if let (Some(infos), Some(list)) =
                (tree.iter().find(|t| t.node == p.node), answer["revisions"].as_array_mut())
            {
                for revision in list {
                    let number = revision["number"].as_u64();
                    if let Some(info) = infos.revisions.iter().find(|i| Some(u64::from(i.number)) == number) {
                        revision["amend_seq"] = json!(info.amend_seq);
                        if let Some(k) = info.based_on_amend {
                            revision["based_on_amend"] = json!(k);
                        }
                    }
                }
            }
            Ok(answer)
        }
        "revision.accept" | "revision.set_current" => {
            // Übernehmen = Umschalten auf einen Vorschlag. Die Gliederung folgt der Überschriftszeile
            // der Fassung (andere Ebene: `outline` in der Antwort); genau ein Commit im Spiegel.
            let held = state.write_lock.lock().await;
            let p: RevParams = params(resolve_keys(state, work_id, params_value).await?)?;
            let only_proposal = method == "revision.accept";
            let switch = store.switch_to(work_id, p.node, p.rev, only_proposal, mirror::HUMAN_NAME).await?;
            let mut answer = json!({ "node": switch.change.node, "rev": switch.change.rev });
            if let Some(outline) = &switch.outline {
                answer["outline"] = crate::outline::outline_answer(outline);
            }
            add_mirror(&mut answer, mirror::record_switch(state, &held, &switch).await);
            if only_proposal {
                accept_markdown_headings(state, &held, work_id, &switch.change, &mut answer).await?;
            }
            Ok(answer)
        }
        "revision.reject" => {
            let held = state.write_lock.lock().await;
            let p: RevParams = params(resolve_keys(state, work_id, params_value).await?)?;
            let current = store.reject(work_id, p.node, p.rev).await?;
            // Runde 22: Verwerfen ist kein Commit; der Seitenzweig der Fassung trägt danach den Status „verworfen“.
            if let Err(e) = mirror::sync_side_refs(state, &held, work_id).await {
                eprintln!("Git-Spiegel (Werk {work_id}): Seitenzweige nicht nachgeführt: {e}");
            }
            Ok(json!({ "node": p.node, "rev": current }))
        }
        "revision.save" => {
            // Runde 22: `mode` „auto“ schreibt die eigene aktuelle Fassung fort, wenn die Regeln es erlauben;
            // fehlt er, gilt „new“ (wie vor Runde 22, alle alten Clients).
            let mode = save_mode(&params_value)?;
            let held = state.write_lock.lock().await;
            let mut p: SaveParams = params(resolve_keys(state, work_id, params_value).await?)?;
            // Eingabe des Menschen: Markdown-Überschriften werden Typst-Überschriften (Nutzer 2026-11-07).
            p.body = baruch_typst::markdown_headings(&p.body).into_owned();
            let (saved, reason) = store
                .save_human_at(work_id, p.node, &p.body, mirror::HUMAN_NAME, p.based_on, p.based_on_amend, mode)
                .await?;
            match saved {
                Some(change) => {
                    let mut answer = json!({ "node": change.node, "rev": change.rev });
                    add_saved(&mut answer, Some(&change), reason);
                    add_mirror(&mut answer, mirror::record(state, &held, &change).await);
                    Ok(answer)
                }
                None => {
                    let mut answer = shown_current(&store.load_work(work_id).await?, p.node);
                    add_saved(&mut answer, None, None);
                    Ok(answer)
                }
            }
        }
        "revision.update" => {
            let held = state.write_lock.lock().await;
            let p: UpdateParams = params(resolve_keys(state, work_id, params_value).await?)?;
            // Runde 18b: Eingabe des Menschen wie bei `revision.save` – Markdown-Überschriften werden
            // Typst-Überschriften (`baruch_typst::markdown`, Nutzer 2026-11-07).
            let body = p.body.as_deref().map(baruch_typst::markdown_headings);
            let edit = RevisionEdit { body: body.as_deref(), note: p.note.as_deref() };
            let done =
                store.update_revision_at(work_id, p.node, p.rev, &edit, mirror::HUMAN_NAME, p.based_on_amend).await?;
            match done {
                Some(change) => {
                    let mut answer = json!({ "node": change.node, "rev": change.rev, "changed": true });
                    // Runde 23a: neuer Stand, wenn das Ändern einen Schritt im Verlauf angelegt hat.
                    if let Some(seq) = change.amend_seq {
                        answer["amend_seq"] = json!(seq);
                    }
                    add_mirror(&mut answer, mirror::record(state, &held, &change).await);
                    Ok(answer)
                }
                None => Ok(json!({ "node": p.node, "rev": p.rev, "changed": false })),
            }
        }
        "revision.delete" => {
            if params_value.get("confirm") != Some(&Value::Bool(true)) {
                return Err(RpcError::new(INVALID_PARAMS, "Löschen bitte bestätigen (confirm)."));
            }
            let held = state.write_lock.lock().await;
            let p: DeleteParams = params(resolve_keys(state, work_id, params_value).await?)?;
            let change = store.delete_revision(work_id, p.node, p.rev, mirror::HUMAN_NAME).await?;
            let current = current_of(state, work_id, p.node).await?;
            let mut answer = json!({
                "node": change.node, "rev": current, "deleted": change.rev, "reparented": change.reparented,
            });
            add_mirror(&mut answer, mirror::record(state, &held, &change).await);
            Ok(answer)
        }
        "revision.tree" => {
            let p: TreeParams = params(params_value)?;
            let work = store.load_work(work_id).await?;
            if let Some(node) = p.node.filter(|&n| n >= work.nodes.len()) {
                return Err(StoreError::UnknownNode { work: work_id, node }.into());
            }
            let tree = store.revision_tree(work_id).await?;
            Ok(tree_answer(&work, &tree, p.node, p.chapter))
        }
        "post.add" => {
            let _held = state.write_lock.lock().await;
            let p: BodyParams = params(resolve_keys(state, work_id, params_value).await?)?;
            let post = Post {
                id: 0,
                created: String::new(),
                author: mirror::HUMAN_NAME.into(),
                body: p.body,
                sources: vec![],
                refs: vec![],
            };
            let id = store.add_post(work_id, p.node, &post).await?;
            let key = store.node_key(work_id, p.node).await?;
            Ok(json!({ "node": p.node, "key": key, "id": id }))
        }
        "library.list" => {
            let titles = baruch_library::list_titles(store.pool()).await?;
            Ok(json!({ "titles": titles }))
        }
        "library.search" => {
            let mut p: SearchParams = params(params_value)?;
            p.options.work.get_or_insert(work_id);
            let hits = baruch_library::search(store.pool(), &p.query, &p.options).await?;
            Ok(json!({ "hits": hits }))
        }
        "ui.render" => {
            let at: Location = params(params_value)?;
            let (work, shelf) = crate::shelf(state, work_id).await?;
            let library = library_view(state, work_id, at.lq.as_deref()).await?;
            let resolved = at.resolve(&work);
            let panes = view::panes(&state.templates, &work, &shelf, resolved.clone(), settings, &library)
                .map_err(|e| RpcError::new(INTERNAL, e.to_string()))?;
            let n = &work.nodes[resolved.node];
            Ok(json!({
                "tree": panes.tree,
                "editor": panes.editor,
                "assistants": panes.assistants,
                "url": resolved.url(work_id, library.query.as_deref()),
                "title": format!("{} – Baruch", n.title),
            }))
        }
        m if crate::literature::handles(m) => crate::literature::call(state, work_id, m, params_value).await,
        m if crate::outline::handles(m) => crate::outline::call(state, work_id, m, params_value).await,
        m if crate::wysiwyg::handles(m) => crate::wysiwyg::call(state, work_id, m, params_value).await,
        m if crate::numbering::handles(m) => crate::numbering::call(state, work_id, m, params_value).await,
        m if crate::assets::handles(m) => crate::assets::call(state, work_id, m, params_value).await,
        m if crate::typeset::handles(m) => crate::typeset::call(state, work_id, m, params_value).await,
        m if crate::pdf::handles(m) => crate::pdf::call(state, work_id, m, params_value).await,
        m if crate::asset_requests::handles(m) => crate::asset_requests::call(state, work_id, m, params_value).await,
        m if crate::cover::handles(m) => crate::cover::call(state, work_id, m, params_value).await,
        m if crate::cover_layout::handles(m) => crate::cover_layout::call(state, work_id, m, params_value).await,
        m if crate::timing::handles(m) => crate::timing::call(state, work_id, m, params_value).await,
        m if crate::ribbon::handles(m) => crate::ribbon::call(state, work_id, m, params_value).await,
        m if crate::sheet::handles(m) => crate::sheet::call(state, work_id, m, params_value).await,
        // Runde 29 (Schnittstelle R29-S1): Satz-Einstellungen und ihre Vorschau.
        m if crate::typeset_settings::handles(m) => {
            crate::typeset_settings::call(state, work_id, m, params_value).await
        }
        m if crate::settings::work_typeset::handles(m) => {
            crate::settings::work_typeset::call(state, work_id, m, params_value).await
        }
        // Runde 30 (Schnittstelle R30-S1).
        m if crate::search::handles(m) => crate::search::call(state, work_id, m, params_value).await,
        m if crate::xref::handles(m) => crate::xref::call(state, work_id, m, params_value).await,
        m if crate::assist::solutions::handles(m) => {
            crate::assist::solutions::call(state, work_id, m, params_value).await
        }
        // Runde 20 (Schnittstelle R20-S1): EPUB, Vorschau je Reader, Prüfung.
        m if crate::epub::handles(m) => crate::epub::call(state, work_id, m, params_value).await,
        m if crate::epub_preview::handles(m) => crate::epub_preview::call(state, work_id, m, params_value).await,
        m if crate::epub_check::handles(m) => crate::epub_check::call(state, work_id, m, params_value).await,
        // Runde 31 (Schnittstelle R31-S1): Glossar, Register und Registermarken.
        m if crate::glossary::handles(m) => crate::glossary::call(state, work_id, m, params_value).await,
        m if crate::index_marks::handles(m) => crate::index_marks::call(state, work_id, m, params_value).await,
        // Runde 22 (Schnittstelle R22-S1): Verlauf der Fortschreibungen und Einstellung „Fassungen“.
        m if crate::revisions::handles(m) => crate::revisions::call(state, work_id, m, params_value).await,
        m if crate::revision_fold::handles(m) => crate::revision_fold::call(state, work_id, m, params_value).await,
        _ => Err(RpcError::new(METHOD_NOT_FOUND, format!("unbekannte Methode {method}"))),
    }
}

/// Eine JSON-RPC-Anfrage (einzeln) beantworten.
pub async fn handle(state: &AppState, settings: &Settings, request: Value) -> Value {
    let id = request.get("id").cloned().unwrap_or(Value::Null);
    let method = request.get("method").and_then(Value::as_str);
    let result = match (request.get("jsonrpc").and_then(Value::as_str), method) {
        (Some("2.0"), Some(method)) => {
            call(state, settings, method, request.get("params").cloned().unwrap_or(Value::Null)).await
        }
        _ => Err(RpcError::new(INVALID_REQUEST, "jsonrpc 2.0 mit method erwartet")),
    };
    match result {
        Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
        Err(e) => json!({ "jsonrpc": "2.0", "id": id, "error": { "code": e.code, "message": e.message } }),
    }
}
