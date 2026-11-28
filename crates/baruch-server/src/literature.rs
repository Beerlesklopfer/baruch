// SPDX-License-Identifier: AGPL-3.0-or-later
//! Literature as a popup and as pages of its own: search across work and library, document view of a
//! source with highlights, creating, changing and deleting highlights and taking them over as a quote.
//!
//! Pages (usable without JavaScript, [`pages`]):
//! - `GET /literatur?work&key&rev&lq&lmode` (`node` only as a fallback) – search form, hits, titles with quotes and
//!   highlights, link back to the work. With JavaScript the header shows the same content as a `<dialog>`.
//!   `lmode` (Round 12) selects the search: `fts` (default, wording through FTS5) or `rag` (meaning through
//!   [`crate::search_hooks::rag_search`], all ways of the work). If no way answers, the page says why and
//!   links to the same search by wording – it never switches silently. `lmode` travels along like `lq` in all
//!   links and forms. The work page stays with FTS.
//! - `GET /quelle?work&key&title&quote?&highlight?&lq&teil?` – document view: details, quotes (anchor
//!   `#zitat-<id>`), all highlights (anchor `#hervorhebung-<id>`, with a check against the full text), target
//!   with class `target` or `<mark>`; for a title linked with MultiGPT the full text (part `teil`,
//!   Round 13); below it the form for selected text.
//! - `GET /quelle/multigpt?work&key&rev&lq&lmode&collection&document&section&teil&q` (Round 13) – full text of a
//!   document from MultiGPT, in parts with links „vorheriger/nächster Teil“, passage `q` as `<mark>`, button
//!   „In die Bibliothek übernehmen“; without JavaScript. If MultiGPT is down, the page names the reason (or
//!   shows the local copy of the sync). See [`fulltext`]. Round 18b: with a section (`key`) the form
//!   „An Persona geben“ (`action=persona`, text of the part shown prefilled, source „MultiGPT: Titel (Dokument
//!   …)“, without an adopted title).
//!
//! Forms ([`writes`], behind `guard::same_origin`): `POST /literatur/act` with `action` =
//! `highlight`, `cite`, `persona`, `own`, `similar`, `update`, `delete`, `adopt`, `check`, then a redirect.
//! `persona` (Round 15, „an Persona geben“): selected text (`text`, at most 4000 characters) with a source
//! reference (`source`, one line) and a question (`question`) to the chosen persona (`persona`, default the
//! lector); it answers as a post in the section `key` (mandatory, otherwise the notice page „Abschnitt wählen“)
//! with the source `source`. Nothing in the book text. If the persona fails, a notice page (status 200) shows the
//! reason.
//! `own` (Round 15): take over a persona highlight as one's own – a new highlight of the human (active),
//! the original stays a proposal.
//!
//! Bibles in the literature (Round 15, [`bibles`]): box „Bibeln“ in the popup and on `/literatur`, hit groups
//! „Bibel“ and „Kommentar“ in wording mode, own page `/literatur/bibel`.
//!
//! Images in the literature (Round 21c, [`image_hits`]): with a search, `/literatur` (without JavaScript) and the
//! popup (`literature.render`) show the box „Bilder des Werks“ below the hits: every image in whose name,
//! caption or alternative text (current revision) all words of the search occur (ignoring case), with a
//! preview image (`/werke/{id}/bilder/{name}/vorschau?b=160`) and a link to the page of the image; at most
//! [`MAX_IMAGE_HITS`]. Texts escaped (own small template [`IMAGE_HITS_TEMPLATE`], `fragments/library.html` stays
//! untouched).
//!
//! Section: both pages carry the **fixed key** `key` of the section itself (from `?key=`,
//! otherwise determined from the position `?node=` on loading) in their links and forms. Quoting
//! (`cite`, `highlight.cite`) accepts only `key` and resolves it under the write lock; the
//! position `node` from the address is pure display (contract Round 5): after a rearrangement in
//! another tab the quote would otherwise land in a foreign section.
//!
//! RPC ([`handles`], [`call`]; called from `rpc::call`):
//! - `literature.render {work?, node?, key?, rev?, lq?, lmode?}` → `{html, lq, lmode, url}` (content of the popup)
//! - `highlight.list {title?}` → `{highlights}` (per highlight `check` or `null`)
//! - `highlight.add {title, quote?, locator?, start?, end?, text, kind, note?}` → `{highlight, check}`: without
//!   a quote, with a range on a linked title, checked against the full text (Round 13)
//! - `highlight.check {highlight}` → `{highlight, check}` (check again)
//! - `source.read {title? | document, collection?, part?, q?}` → part of the full text (Round 13)
//! - `source.adopt {document, collection?}` → `{title, remote}` (only the human; not through MCP)
//! - `highlight.update {highlight, kind?, note?, status?}` → `{highlight}`
//! - `highlight.delete {highlight}` → `{deleted}`
//! - `highlight.cite {highlight, key}` → `{node, key, rev}`: proposal revision with `#quote(...)[...]`
//!   through the unchanged `Store::add_proposal` (author `@literatur`, model `baruch-zitat`); `key`
//!   is mandatory, `node` → -32602.
//! - `highlight.own {highlight}` → `{highlight, original}` (Round 15): own copy of a persona highlight
//!   (active, author the human), the original stays a proposal; not through MCP.
//! - `literature.to_persona {work, key, persona, text, source, title?, highlight?, question?}` (Round 15) → like
//!   `persona.ask` (post of the persona with the source `source`, the question as a post of the human).
//!
//! Highlights through RPC and form always come from the human (`active`); personas create them
//! later only as a proposal (`HighlightAuthor::Persona`). After every creating, changing and deleting
//! of a highlight the literature reports that to the semantic search ([`crate::search_hooks::changed`]).

use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use axum::Router;
use axum::extract::{Form, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use baruch_library::{
    Highlight, HighlightAuthor, HighlightCheck, HighlightError, HighlightKind, HighlightStatus, LibraryError,
    NewHighlight, SearchHit, SearchOptions, Title, TitleEntry,
};
use baruch_rag::{Answer, DEFAULT_LIMIT, MergedHit, RagError, SearchRequest, Way, WayState};
use baruch_store::{StoreError, WorkId};
use minijinja::value::Serde;
use minijinja::{Environment, Value, context};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::rpc::{INTERNAL, INVALID_PARAMS, METHOD_NOT_FOUND, REFUSED, RpcError};
use crate::view::{Settings, query_escape};
use crate::{AppState, mirror};

pub mod bibles;
mod fulltext;

pub use fulltext::{
    LocalCopy, MAX_MARK_QUERY_CHARS, MAX_MARK_WORDS, Reading, SourceInfo, Target, WAY as FULLTEXT_WAY,
    adopt as adopt_source, check_highlight, local_copy, marks, read as read_source,
};

/// Originator and model of the quote proposals from the literature.
pub const CITE_PERSONA: &str = "@literatur";
pub const CITE_MODEL: &str = "baruch-zitat";

/// Longest selected text for „an Persona geben“ (characters).
pub const MAX_PERSONA_TEXT: usize = 4000;
/// Longest source reference for „an Persona geben“ (characters, one line).
pub const MAX_SOURCE_CHARS: usize = 300;
/// Notice when „an Persona geben“ comes without a section.
pub const PICK_SECTION: &str = "Abschnitt wählen: Die Persona antwortet im Abschnitts-Chat. Die Literatur aus einem \
     Abschnitt heraus öffnen (Arbeitsseite → Literatur) und den Text dann an die Persona geben.";

/// Notice for a source without a link to MultiGPT (not a stub: a full text exists only for
/// adopted documents).
pub const NOT_LINKED: &str = "Diese Quelle ist nicht mit MultiGPT verknüpft; einen Volltext gibt es nur für \
     Dokumente aus MultiGPT (Bedeutungssuche → „Volltext“ → „In die Bibliothek übernehmen“).";

/// Reason when a foreign page wants to load the full text by GET (`Sec-Fetch-Site`/`Origin`).
pub const FOREIGN_FULLTEXT: &str = "Aufruf von einer fremden Seite; Baruch lädt den Volltext über MultiGPT nur, \
     wenn die Seite hier in Baruch geöffnet wird.";

/// Wording search (FTS5, default) or semantic search (RAG) – `lmode` in address and forms.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SearchMode {
    #[default]
    Fts,
    Rag,
}

impl SearchMode {
    /// `rag` → meaning; everything else (also empty or unknown) → wording.
    pub fn parse(value: Option<&str>) -> SearchMode {
        match value.map(str::trim) {
            Some("rag") => SearchMode::Rag,
            _ => SearchMode::Fts,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            SearchMode::Fts => "fts",
            SearchMode::Rag => "rag",
        }
    }
}

// ---- Images of the work (Round 21c) --------------------------------------------------------------

/// The box „Bilder des Werks“ shows at most this many images.
pub const MAX_IMAGE_HITS: usize = 24;

/// Box „Bilder des Werks“ (text escaped automatically, addresses from [`href`]).
const IMAGE_HITS_TEMPLATE: &str = r#"{#- Bilder des Werks zur Suche (Runde 21c): images [{name, caption, alt, href, thumb}], query. -#}
<section class="library-images" data-library-images aria-labelledby="library-images-title">
  <div class="muted tiny" id="library-images-title">Bilder des Werks für „{{ query }}“</div>
  {%- if images %}
  <ul class="library-image-list">
  {%- for i in images %}
    <li class="library-image" data-image="{{ i.name }}">
      <a href="{{ i.href }}"><img src="{{ i.thumb }}" alt="{{ i.alt }}" loading="eager" width="80"></a>
      <span class="library-image-text"><a href="{{ i.href }}">{{ i.name }}</a>
        {%- if i.caption %}<br><span class="small">{{ i.caption }}</span>{% endif %}
        {%- if i.alt %}<br><span class="muted tiny">Alternativtext: {{ i.alt }}</span>{% endif %}</span>
    </li>
  {%- endfor %}
  </ul>
  {%- if more %}<p class="muted tiny">Nur die ersten {{ images | length }} Bilder; mehr auf der Seite <a href="{{ all_href }}">Bilder</a>.</p>{% endif %}
  {%- else %}
  <p class="muted small">Kein Bild passt (gesucht in Name, Bildunterschrift und Alternativtext).</p>
  {%- endif %}
</section>
"#;

/// Images of the work in whose name, caption or alternative text all words of `query` occur (ignoring
/// case, umlauts included); ordered by name. Empty search → none. Result: at most `limit` images and
/// whether there were more.
pub async fn image_hits(
    state: &AppState,
    work: WorkId,
    query: &str,
    limit: usize,
) -> Result<(Vec<baruch_store::AssetSummary>, bool), StoreError> {
    let words: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
    if words.is_empty() {
        return Ok((Vec::new(), false));
    }
    let mut found: Vec<baruch_store::AssetSummary> = state
        .store
        .asset_list(work)
        .await?
        .into_iter()
        .filter(|a| {
            let text = format!("{}\n{}\n{}", a.name, a.caption, a.alt).to_lowercase();
            words.iter().all(|w| text.contains(w.as_str()))
        })
        .collect();
    let more = found.len() > limit;
    found.truncate(limit);
    Ok((found, more))
}

/// HTML of the box „Bilder des Werks“ for the search of `at` (empty without a search). An error while reading the
/// images appears as a notice in the box, never as a silent omission.
async fn image_hits_html(state: &AppState, at: &At) -> String {
    image_hits_box(state, at.work, at.lq.as_deref()).await
}

/// The box „Bilder des Werks“ for the search `lq` in the work `work` (empty without a search). Round 23a: also for
/// the work page with `?lq=` (`rpc::library_view`), so that the box is in the served HTML without JavaScript.
pub(crate) async fn image_hits_box(state: &AppState, work: WorkId, lq: Option<&str>) -> String {
    let Some(query) = lq.map(str::trim).filter(|q| !q.is_empty()) else { return String::new() };
    let mut env = Environment::new();
    crate::i18n::install(&mut env);
    env.set_auto_escape_callback(|_| minijinja::AutoEscape::Html);
    if let Err(e) = env.add_template("images", IMAGE_HITS_TEMPLATE) {
        return format!("<p class=\"notice error\">Bilder: Vorlage fehlerhaft ({})</p>", escape_text(&e.to_string()));
    }
    let (images, more) = match image_hits(state, work, query, MAX_IMAGE_HITS).await {
        Ok(found) => found,
        Err(e) => {
            return format!("<p class=\"notice error\">Bilder des Werks: {}</p>", escape_text(&e.to_string()));
        }
    };
    let rows: Vec<Value> = images
        .iter()
        .map(|a| {
            let base = format!("/werke/{}/bilder/{}", work, a.name);
            context! {
                name => a.name.as_str(), caption => a.caption.as_str(), alt => a.alt.as_str(),
                href => href(base.clone()), thumb => href(format!("{base}/vorschau?b=160")),
            }
        })
        .collect();
    let ctx = context! {
        images => rows, query => query, more => more, all_href => href(format!("/werke/{}/bilder", work)),
    };
    env.get_template("images")
        .and_then(|t| t.render(ctx))
        .unwrap_or_else(|e| format!("<p class=\"notice error\">Bilder: {}</p>", escape_text(&e.to_string())))
}

/// Escape text for HTML (`&`, `<`, `>`, `"`, `'`).
fn escape_text(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#x27;")
}

// ---- Templates ---------------------------------------------------------------------------------

/// Own template environment of the literature (that of the work page stays untouched).
fn templates() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
        crate::i18n::install(&mut env);
        for (name, source) in [
            ("base.html", include_str!("../templates/base.html")),
            ("literature.html", include_str!("../templates/literature.html")),
            ("source.html", include_str!("../templates/source.html")),
            ("source_multigpt.html", include_str!("../templates/source_multigpt.html")),
            ("fragments/library.html", include_str!("../templates/fragments/library.html")),
            ("fragments/highlights.html", include_str!("../templates/fragments/highlights.html")),
            ("fragments/literature_bibles.html", include_str!("../templates/fragments/literature_bibles.html")),
            ("literature_bible.html", include_str!("../templates/literature_bible.html")),
        ] {
            env.add_template(name, source).expect("Vorlage der Literatur fehlerhaft");
        }
        env
    })
}

/// Theme from the cookies (`base.html` needs `settings`); column widths do not apply here.
fn page_settings(headers: &HeaderMap) -> Value {
    let settings = Settings::from_cookie_header(headers.get(header::COOKIE).and_then(|v| v.to_str().ok()));
    Value::from(Serde(settings))
}

/// A self-built address for an `href`: it consists only of digits, fixed names and
/// percent-encoded values ([`query_escape`]), so it is enough to escape `&`. This keeps `/`
/// readable (the templates otherwise escape it as `&#x2f;`).
fn href(url: String) -> Value {
    debug_assert!(!url.contains(['"', '<', '>', '\'', ' ']), "unsichere Adresse {url:?}");
    Value::from_safe_string(url.replace('&', "&amp;"))
}

fn render(name: &str, ctx: Value) -> Result<String, RpcError> {
    templates()
        .get_template(name)
        .and_then(|t| t.render(ctx))
        .map_err(|e| RpcError::new(INTERNAL, format!("Vorlage {name}: {e}")))
}

// ---- Errors ------------------------------------------------------------------------------------

impl From<HighlightError> for RpcError {
    fn from(e: HighlightError) -> Self {
        match e {
            HighlightError::Library(e) => e.into(),
            HighlightError::Unknown(_)
            | HighlightError::Empty
            | HighlightError::InvalidRange
            | HighlightError::TextMismatch(_) => RpcError::new(INVALID_PARAMS, e.to_string()),
        }
    }
}

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError::new(INVALID_PARAMS, message)
}

fn status_of(e: &RpcError) -> StatusCode {
    match e.code {
        INVALID_PARAMS => StatusCode::UNPROCESSABLE_ENTITY,
        REFUSED => StatusCode::CONFLICT,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn error_response(e: RpcError) -> Response {
    (status_of(&e), format!("Fehler: {}", e.message)).into_response()
}

// ---- Position in the work ----------------------------------------------------------------------

/// Work, section, revision and search as the literature pages pass them on.
#[derive(Debug, Clone, Default)]
struct At {
    work: WorkId,
    work_title: String,
    node: Option<usize>,
    /// Fixed key of the section (`Node::key`), belongs to `node`.
    key: Option<i64>,
    rev: Option<u32>,
    preview: bool,
    lq: Option<String>,
    /// Kind of search (`lmode`); travels along with `lq`.
    mode: SearchMode,
}

/// Number from a form or address field; empty or invalid → `None`.
fn number<T: std::str::FromStr>(value: &Option<String>) -> Option<T> {
    value.as_deref().map(str::trim).filter(|v| !v.is_empty()).and_then(|v| v.parse().ok())
}

/// Section from key and position: the key takes precedence (the position can be outdated); if it
/// does not exist (any more), no section applies. Without a key the position, if it exists, together with
/// its key.
fn place_of(work: &baruch_core::Work, node: Option<usize>, key: Option<i64>) -> (Option<usize>, Option<i64>) {
    let found = match key {
        Some(key) => work.nodes.iter().find(|n| n.key == key),
        None => node.and_then(|n| work.nodes.get(n)),
    };
    found.map_or((None, None), |n| (Some(n.id), Some(n.key)))
}

/// Is the field (not `null`) in the parameters? For `node` in `highlight.cite`, which is refused.
fn given(params: &serde_json::Value, name: &str) -> bool {
    params.get(name).is_some_and(|v| !v.is_null())
}

/// Fixed key from RPC parameters: a number or digits as text; missing → `None`.
fn key_param(params: &serde_json::Value) -> Result<Option<i64>, RpcError> {
    let wrong = || invalid("key: fester Schlüssel des Abschnitts (Zahl) erwartet");
    match params.get("key") {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(serde_json::Value::Number(n)) => n.as_i64().map(Some).ok_or_else(wrong),
        Some(serde_json::Value::String(t)) if t.trim().is_empty() => Ok(None),
        Some(serde_json::Value::String(t)) => t.trim().parse().map(Some).map_err(|_| wrong()),
        Some(_) => Err(wrong()),
    }
}

impl At {
    /// Check the work (unknown → `UnknownWork`), take over the section only if it exists
    /// ([`place_of`]: key before position).
    async fn resolve(
        state: &AppState,
        work: Option<WorkId>,
        (node, key): (Option<usize>, Option<i64>),
        rev: Option<u32>,
        view: Option<&str>,
        (lq, lmode): (Option<&str>, Option<&str>),
    ) -> Result<At, StoreError> {
        let work = match work {
            Some(work) => work,
            None => state.default_work().await?,
        };
        let loaded = state.store.load_work(work).await?;
        let (node, key) = place_of(&loaded, node, key);
        Ok(At {
            work,
            work_title: loaded.title,
            node,
            key,
            rev: rev.filter(|_| node.is_some()),
            preview: view == Some("preview"),
            lq: lq.map(str::trim).filter(|q| !q.is_empty()).map(str::to_owned),
            mode: SearchMode::parse(lmode),
        })
    }

    /// `&lq=<encoded>[&lmode=rag]` or empty (the default `fts` is not in the address).
    fn lq_tail(&self) -> String {
        let mut tail = self.lq.as_deref().map_or(String::new(), |q| format!("&lq={}", query_escape(q)));
        if self.mode == SearchMode::Rag {
            tail.push_str("&lmode=rag");
        }
        tail
    }

    /// `work=…[&key=…]`: work and fixed key of the section (contract Round 8, §1: generated
    /// addresses no longer carry `node=`; `node` and `key` always belong together, see [`place_of`]).
    fn place(&self) -> String {
        let mut q = format!("work={}", self.work);
        if let Some(key) = self.key {
            q.push_str(&format!("&key={key}"));
        }
        q
    }

    /// Back to the work: `/?work&key&rev[&view=preview][&lq]`.
    fn work_url(&self) -> String {
        let mut url = format!("/?{}", self.place());
        if let Some(rev) = self.rev {
            url.push_str(&format!("&rev={rev}"));
        }
        if self.preview {
            url.push_str("&view=preview");
        }
        url.push_str(&self.lq_tail());
        url
    }

    fn literature_url(&self) -> String {
        let mut url = format!("/literatur?{}", self.place());
        if let Some(rev) = self.rev {
            url.push_str(&format!("&rev={rev}"));
        }
        url.push_str(&self.lq_tail());
        url
    }

    fn source_url(&self, title: i64, quote: Option<i64>, highlight: Option<i64>, notice: Option<&str>) -> String {
        let mut url = format!("/quelle?title={title}&{}", self.place());
        if let Some(quote) = quote {
            url.push_str(&format!("&quote={quote}"));
        }
        if let Some(highlight) = highlight {
            url.push_str(&format!("&highlight={highlight}"));
        }
        if let Some(notice) = notice {
            url.push_str(&format!("&notice={notice}"));
        }
        url.push_str(&self.lq_tail());
        match (highlight, quote) {
            (Some(h), _) => url.push_str(&format!("#hervorhebung-{h}")),
            (None, Some(q)) => url.push_str(&format!("#zitat-{q}")),
            _ => {}
        }
        url
    }

    /// Document view of a title at part `teil` of the full text (anchor `#volltext`).
    fn source_part_url(&self, title: i64, teil: usize) -> String {
        format!("/quelle?title={title}&{}&teil={teil}{}#volltext", self.place(), self.lq_tail())
    }

    /// Full text of a document from MultiGPT: `/quelle/multigpt?work&key[&rev]&collection&document[&section]
    /// &teil[&q][&lq][&lmode]`.
    fn multigpt_url(
        &self,
        collection: &str,
        document: &str,
        section: Option<&str>,
        teil: usize,
        q: Option<&str>,
    ) -> String {
        let mut url = format!(
            "/quelle/multigpt?{}&collection={}&document={}",
            self.place(),
            query_escape(collection),
            query_escape(document)
        );
        if let Some(rev) = self.rev {
            url.push_str(&format!("&rev={rev}"));
        }
        if let Some(section) = section {
            url.push_str(&format!("&section={}", query_escape(section)));
        }
        url.push_str(&format!("&teil={teil}"));
        if let Some(q) = q {
            url.push_str(&format!("&q={}", query_escape(q)));
        }
        url.push_str(&self.lq_tail());
        url
    }

    /// Hidden fields for forms: work, node, key, rev, lq (only those present). Through `key`
    /// `cite` finds the section.
    fn fields(&self) -> Vec<Value> {
        let mut fields = vec![context! { name => "work", value => self.work.to_string() }];
        if let Some(node) = self.node {
            fields.push(context! { name => "node", value => node.to_string() });
        }
        if let Some(key) = self.key {
            fields.push(context! { name => "key", value => key.to_string() });
        }
        if let Some(rev) = self.rev {
            fields.push(context! { name => "rev", value => rev.to_string() });
        }
        if let Some(lq) = &self.lq {
            fields.push(context! { name => "lq", value => lq.as_str() });
        }
        if self.mode == SearchMode::Rag {
            fields.push(context! { name => "lmode", value => "rag" });
        }
        fields
    }

    /// Context `work`, `node`, `rev`, `view` as in the work page (for `fragments/library.html`).
    fn context(&self) -> (Value, Value, Value) {
        let work = context! { id => self.work, title => self.work_title.as_str() };
        let node = match (self.node, self.key) {
            (Some(id), Some(key)) => context! { id => id, key => key },
            (Some(id), None) => context! { id => id },
            _ => Value::UNDEFINED,
        };
        let rev = self.rev.map_or(Value::UNDEFINED, |number| context! { number => number });
        (work, node, rev)
    }
}

// ---- Content of the literature -----------------------------------------------------------------

/// Title with quotes and highlights (serialised flat like `TitleEntry`).
#[derive(Serialize)]
struct TitleView {
    #[serde(flatten)]
    entry: TitleEntry,
    highlights: Vec<Highlight>,
}

struct Literature {
    titles: Vec<TitleView>,
    hits: Vec<SearchHit>,
    /// Semantic search (`lmode=rag` with a search): [`rag_view`] or [`rag_unavailable`].
    rag: Option<serde_json::Value>,
    /// Box „Bibeln“ and hit groups „Bibel“/„Kommentar“ (Round 15, [`bibles::Shelf`]).
    bibles: bibles::Shelf,
}

/// All titles with highlights and, if a search was made, the hits in the work and in the
/// library. A search without words (also only NUL or control characters) yields no hits.
///
/// `own_page`: the request came from Baruch itself (RPC or [`crate::guard::check`] for GET). Otherwise the
/// semantic search does not run – a foreign page could otherwise trigger it with the credentials of the ways
/// (embedding, pgvector, MultiGPT); the page then says [`FOREIGN_PAGE`].
async fn literature(state: &AppState, at: &At, own_page: bool) -> Result<Literature, RpcError> {
    let pool = state.store.pool();
    let entries = baruch_library::list_titles(pool).await?;
    let mut highlights = baruch_library::list_highlights(pool, None).await?;
    let titles = entries
        .into_iter()
        .map(|entry| {
            let (mine, rest): (Vec<_>, Vec<_>) =
                std::mem::take(&mut highlights).into_iter().partition(|h| h.title_id == entry.title.id);
            highlights = rest;
            TitleView { entry, highlights: mine }
        })
        .collect();
    if let (Some(_), SearchMode::Rag, false) = (&at.lq, at.mode, own_page) {
        let bibles = bibles::shelf(state, at, None).await;
        return Ok(Literature { titles, hits: Vec::new(), rag: Some(rag_unavailable(FOREIGN_PAGE, None)), bibles });
    }
    if let (Some(q), SearchMode::Rag) = (&at.lq, at.mode) {
        let request = SearchRequest {
            work: at.work,
            query: q.clone(),
            ways: None,
            collections: None,
            limit: DEFAULT_LIMIT,
            origin: mirror::HUMAN_NAME.to_owned(),
        };
        let view = match crate::search_hooks::rag_search(state, &request).await {
            Ok(answer) => rag_view(&answer),
            Err(RagError::Invalid(message)) => return Err(invalid(message)),
            Err(e) => rag_unavailable(e.message(), None),
        };
        let bibles = bibles::shelf(state, at, None).await;
        return Ok(Literature { titles, hits: Vec::new(), rag: Some(view), bibles });
    }
    let hits = match &at.lq {
        Some(q) => {
            let options = SearchOptions { work: Some(at.work), ..SearchOptions::default() };
            match baruch_library::search(pool, q, &options).await {
                Ok(hits) => hits,
                Err(LibraryError::EmptyQuery) => Vec::new(),
                Err(e) => return Err(e.into()),
            }
        }
        None => Vec::new(),
    };
    let bibles = bibles::shelf(state, at, at.lq.as_deref()).await;
    Ok(Literature { titles, hits, rag: None, bibles })
}

// ---- Semantic search (RAG) ---------------------------------------------------------------------

/// Short name of a way in the status line.
fn short_title(way: Way) -> &'static str {
    match way {
        Way::Lancedb => "LanceDB",
        Way::Pgvector => "pgvector",
        Way::Multigpt => "MultiGPT",
    }
}

/// Status line of the ways, e.g. „LanceDB: verbunden · pgvector: aus · MultiGPT: ausgefallen (…)“; only
/// failed ways name their reason (the reason for „aus“ is in [`rag_unavailable`]).
pub fn ways_line(answer: &Answer) -> String {
    answer
        .ways
        .iter()
        .map(|w| match w.state {
            WayState::Failed if !w.message.is_empty() => {
                format!("{}: {} ({})", short_title(w.way), w.label, without_fts_note(&w.message))
            }
            _ => format!("{}: {}", short_title(w.way), w.label),
        })
        .collect::<Vec<_>>()
        .join(" · ")
}

/// Is the address harmless for an `href`? Only own addresses (`/…`, not `//…`) and http(s); no
/// whitespace, no control characters, no quotation marks or angle brackets, no backslash. Everything
/// else (`javascript:`, `data:`, `//fremd`) is discarded.
pub fn safe_link(url: &str) -> Option<String> {
    let url = url.trim();
    let bad = url.is_empty()
        || url.chars().count() > 2000
        || url.chars().any(|c| c.is_whitespace() || c.is_control() || matches!(c, '"' | '\'' | '<' | '>' | '\\' | '`'));
    if bad {
        return None;
    }
    let lower = url.to_ascii_lowercase();
    let own = url.starts_with('/') && !url.starts_with("//");
    let web = (lower.starts_with("https://") || lower.starts_with("http://")) && url.len() > "https://".len();
    (own || web).then(|| url.to_owned())
}

/// Link of a hit: its address ([`safe_link`]) or – for LanceDB without an address – the place from
/// `meta.work`/`meta.key` (`/?work=W&key=K`). Otherwise no link.
pub fn hit_link(hit: &MergedHit) -> Option<String> {
    if let Some(url) = hit.hit.url.as_deref() {
        return safe_link(url);
    }
    if hit.hit.way == Way::Lancedb {
        let meta = &hit.hit.meta;
        if let (Some(work), Some(key)) =
            (meta.get("work").and_then(serde_json::Value::as_i64), meta.get("key").and_then(serde_json::Value::as_i64))
        {
            return Some(format!("/?work={work}&key={key}"));
        }
    }
    None
}

/// Document view of a MultiGPT hit (Round 13): `/quelle/multigpt?collection=…&document=…[&section=…]&q=…`
/// (all values percent-encoded); other ways → `None`.
pub fn fulltext_link(hit: &MergedHit, query: &str) -> Option<String> {
    if hit.hit.way != Way::Multigpt || hit.hit.document.trim().is_empty() {
        return None;
    }
    let mut url = format!(
        "/quelle/multigpt?collection={}&document={}",
        query_escape(&hit.hit.collection),
        query_escape(&hit.hit.document)
    );
    if let Some(section) = hit.hit.section.as_deref().filter(|s| !s.trim().is_empty()) {
        url.push_str(&format!("&section={}", query_escape(section)));
    }
    if !query.trim().is_empty() {
        url.push_str(&format!("&q={}", query_escape(query.trim())));
    }
    Some(url)
}

/// Answer of the semantic search for the template: hits (title, way badge, collection, snippet,
/// similarity in %, safe link, for MultiGPT since Round 13 `fulltext_link`), the status line of the ways
/// and – if no way answered – the reason. Hit texts are data; the template escapes them.
/// Round 19c: per hit `weak` ([`baruch_rag::is_weak`]); weak hits come after the others (otherwise in the
/// order of the rank), `weak_count` counts them – the template shows them collapsed in `<details>`.
pub fn rag_view(answer: &Answer) -> serde_json::Value {
    let connected = answer.ways.iter().any(|w| w.state == WayState::Connected);
    if !connected {
        let reasons: Vec<String> = answer
            .ways
            .iter()
            .filter(|w| !w.message.is_empty())
            .map(|w| format!("{}: {}", short_title(w.way), without_fts_note(&w.message)))
            .collect();
        let reason = if reasons.is_empty() { "kein Weg hat geantwortet".to_owned() } else { reasons.join("; ") };
        return rag_unavailable(&reason, Some(ways_line(answer)));
    }
    let mut ordered: Vec<&MergedHit> = answer.hits.iter().collect();
    ordered.sort_by_key(|h| baruch_rag::is_weak(h.hit.score));
    let weak_count = ordered.iter().filter(|h| baruch_rag::is_weak(h.hit.score)).count();
    let hits: Vec<serde_json::Value> = ordered
        .into_iter()
        .map(|h| {
            let found_by: Vec<&str> = h.found_by.iter().map(|f| short_title(f.way)).collect();
            json!({
                "rank": h.rank,
                "title": h.hit.title,
                "way": h.hit.way.as_str(),
                "way_title": short_title(h.hit.way),
                "collection": h.hit.collection,
                "document": h.hit.document,
                "section": h.hit.section,
                "snippet": display_snippet(&h.hit.snippet),
                "percent": (f64::from(h.hit.score.clamp(0.0, 1.0)) * 100.0).round() as i64,
                "link": hit_link(h),
                "fulltext_link": fulltext_link(h, &answer.query),
                "found_by": found_by,
                "weak": baruch_rag::is_weak(h.hit.score),
            })
        })
        .collect();
    json!({ "available": true, "hits": hits, "weak_count": weak_count, "weak_label": baruch_rag::WEAK_LABEL,
            "ways_line": ways_line(answer) })
}

/// Snippet for display: Typst heading lines at the beginning (`= Titel`, already shown as the title) and
/// empty lines are dropped; otherwise the text stays unchanged (the template escapes it).
fn display_snippet(snippet: &str) -> String {
    let lines: Vec<&str> = snippet.lines().map(str::trim_end).filter(|l| !l.trim().is_empty()).collect();
    let body: Vec<&str> = lines.iter().copied().skip_while(|l| l.trim_start().starts_with("= ")).collect();
    if body.is_empty() { lines.join("\n") } else { body.join("\n") }
}

/// Message of a way without the sentence about the wording search ([`rag_unavailable`] appends it once).
fn without_fts_note(message: &str) -> String {
    let mut text = message.trim().to_owned();
    for note in [
        "; die Wortlaut-Suche (FTS) geht weiter.",
        ". Die Wortlaut-Suche (FTS) geht weiter.",
        "Die Wortlaut-Suche (FTS) geht weiter.",
    ] {
        if let Some(cut) = text.strip_suffix(note) {
            text = cut.trim_end().to_owned();
            break;
        }
    }
    text.trim_end_matches('.').to_owned()
}

/// Reason when a foreign page wants to trigger the semantic search by GET (`Sec-Fetch-Site`/`Origin`).
pub const FOREIGN_PAGE: &str = "Aufruf von einer fremden Seite; Baruch sucht nach Bedeutung nur, wenn die Suche hier \
in Baruch abgeschickt wird";

/// Semantic search not available: „Bedeutungssuche nicht verfügbar: <Grund>. Die Wortlaut-Suche (FTS)
/// geht weiter.“ (with the status line if there was an answer).
pub fn rag_unavailable(reason: &str, ways_line: Option<String>) -> serde_json::Value {
    let reason = reason.trim().trim_end_matches('.');
    json!({
        "available": false,
        "message": format!("Bedeutungssuche nicht verfügbar: {reason}. Die Wortlaut-Suche (FTS) geht weiter."),
        "ways_line": ways_line,
        "hits": [],
    })
}

fn literature_context(at: &At, lit: &Literature) -> Value {
    let (work, node, rev) = at.context();
    context! {
        work => work, node => node, rev => rev,
        view => if at.preview { "preview" } else { "edit" },
        library => Value::from(Serde(&lit.titles)),
        library_query => at.lq.as_deref(),
        library_hits => Value::from(Serde(&lit.hits)),
        library_mode => at.mode.as_str(),
        library_rag => lit.rag.as_ref().map_or(Value::UNDEFINED, |r| Value::from(Serde(r))),
        fts_url => href(At { mode: SearchMode::Fts, ..at.clone() }.literature_url()),
        work_url => href(at.work_url()),
        bibles => lit.bibles.context(),
    }
}

// ---- Pages -------------------------------------------------------------------------------------

/// Address fields of the pages; everything as text, so that an empty or wrong field does not yield a 400.
#[derive(Debug, Default, Deserialize)]
struct PageQuery {
    work: Option<String>,
    node: Option<String>,
    key: Option<String>,
    rev: Option<String>,
    view: Option<String>,
    lq: Option<String>,
    lmode: Option<String>,
    title: Option<String>,
    quote: Option<String>,
    highlight: Option<String>,
    notice: Option<String>,
    /// Round 13: part of the full text (from 1), document and collection in MultiGPT, passage according to MultiGPT
    /// and the passage `q` to be marked.
    teil: Option<String>,
    document: Option<String>,
    collection: Option<String>,
    section: Option<String>,
    q: Option<String>,
}

impl PageQuery {
    async fn at(&self, state: &AppState) -> Result<At, Box<Response>> {
        At::resolve(
            state,
            number(&self.work),
            (number(&self.node), number(&self.key)),
            number(&self.rev),
            self.view.as_deref(),
            (self.lq.as_deref(), self.lmode.as_deref()),
        )
        .await
        .map_err(|e| {
            Box::new(match e {
                StoreError::UnknownWork(_) => (StatusCode::NOT_FOUND, "unbekanntes Werk").into_response(),
                e => error_response(e.into()),
            })
        })
    }
}

/// Notice from `?notice=…`; only fixed texts, nothing from the address is shown.
fn notice_text(notice: Option<&str>) -> Option<&'static str> {
    match notice? {
        "owned" => Some("Als eigene Hervorhebung übernommen; der Vorschlag der Persona bleibt, wie er war."),
        "deleted" => Some("Hervorhebung gelöscht."),
        "saved" => Some("Hervorhebung gespeichert."),
        "adopted" => Some("In die Bibliothek übernommen; der Titel ist mit dem Dokument in MultiGPT verknüpft."),
        "checked" => Some("Hervorhebung gegen den Volltext geprüft."),
        _ => None,
    }
}

/// Routes of the pages (reading): `GET /literatur`, `GET /quelle`, `GET /quelle/multigpt`.
pub fn pages() -> Router<Arc<AppState>> {
    Router::new()
        .route("/literatur", get(literature_page))
        .route("/quelle", get(source_page))
        .route("/quelle/multigpt", get(multigpt_page))
}

/// Routes of the forms (writing, in the router behind `guard::same_origin`): `POST /literatur/act`.
pub fn writes() -> Router<Arc<AppState>> {
    Router::new().route("/literatur/act", post(act))
}

async fn literature_page(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(q): Query<PageQuery>,
) -> Response {
    let at = match q.at(&state).await {
        Ok(at) => at,
        Err(response) => return *response,
    };
    let lit = match literature(&state, &at, crate::guard::check(&headers).is_ok()).await {
        Ok(lit) => lit,
        Err(e) => return error_response(e),
    };
    let ctx = literature_context(&at, &lit);
    let images_html = Value::from_safe_string(image_hits_html(&state, &at).await);
    let ctx = context! {
        notice => notice_text(q.notice.as_deref()), settings => page_settings(&headers), images_html => images_html, ..ctx
    };
    match render("literature.html", ctx) {
        Ok(html) => Html(html).into_response(),
        Err(e) => error_response(e),
    }
}

/// A piece of quote text, possibly highlighted (character range of a highlight).
#[derive(Debug, PartialEq, Serialize)]
struct Segment {
    text: String,
    highlight: Option<i64>,
    kind: Option<HighlightKind>,
    target: bool,
}

/// Splits `text` at the character ranges of the highlights (Unicode characters from 0). Ranges
/// that reach beyond the end, are empty or overlap an earlier one are skipped.
fn segments(text: &str, marks: &[&Highlight], target: Option<i64>) -> Vec<Segment> {
    let chars: Vec<char> = text.chars().collect();
    let mut ranges: Vec<(usize, usize, &Highlight)> = marks
        .iter()
        .filter_map(|h| {
            let (start, end) = (usize::try_from(h.start?).ok()?, usize::try_from(h.end?).ok()?);
            (start < end && end <= chars.len()).then_some((start, end, *h))
        })
        .collect();
    ranges.sort_by_key(|(start, end, h)| (*start, *end, h.id));
    let mut out = Vec::new();
    let mut cursor = 0;
    let plain = |from: usize, to: usize| Segment {
        text: chars[from..to].iter().collect(),
        highlight: None,
        kind: None,
        target: false,
    };
    for (start, end, h) in ranges {
        if start < cursor {
            continue;
        }
        if start > cursor {
            out.push(plain(cursor, start));
        }
        out.push(Segment {
            text: chars[start..end].iter().collect(),
            highlight: Some(h.id),
            kind: Some(h.kind),
            target: target == Some(h.id),
        });
        cursor = end;
    }
    if cursor < chars.len() || out.is_empty() {
        out.push(plain(cursor, chars.len()));
    }
    out
}

fn author_label(author: &HighlightAuthor) -> String {
    match author {
        HighlightAuthor::Human { name } => name.clone(),
        HighlightAuthor::Persona { handle, model } => format!("{handle} ({model})"),
    }
}

async fn source_page(State(state): State<Arc<AppState>>, headers: HeaderMap, Query(q): Query<PageQuery>) -> Response {
    let at = match q.at(&state).await {
        Ok(at) => at,
        Err(response) => return *response,
    };
    let Some(title_id) = number::<i64>(&q.title) else {
        return (StatusCode::NOT_FOUND, "Quelle fehlt: title=<id> angeben").into_response();
    };
    let pool = state.store.pool();
    let entry = match baruch_library::get_title(pool, title_id).await {
        Ok(entry) => entry,
        Err(LibraryError::UnknownTitle(_)) => return (StatusCode::NOT_FOUND, "unbekannte Quelle").into_response(),
        Err(e) => return error_response(e.into()),
    };
    let highlights = match baruch_library::list_highlights(pool, Some(title_id)).await {
        Ok(list) => list,
        Err(e) => return error_response(e.into()),
    };
    let checks = match baruch_library::highlight_check_entries(pool, Some(title_id)).await {
        Ok(checks) => checks,
        Err(e) => return error_response(e.into()),
    };
    // Link with MultiGPT or local full text from a PDF/EPUB import (Round 21a).
    let linked = match fulltext::has_fulltext(&state, title_id).await {
        Ok(found) => found,
        Err(e) => return error_response(e),
    };
    let teil = number::<usize>(&q.teil).unwrap_or(1).max(1);
    let fulltext = if !linked {
        context! { linked => false, note => NOT_LINKED }
    } else if crate::guard::check(&headers).is_err() {
        let load = href(at.source_part_url(title_id, teil));
        context! { linked => true, available => false, reason => FOREIGN_FULLTEXT, load_url => load }
    } else {
        let mark = text(&q.q).or(at.lq.as_deref());
        match read_source(&state, &Target::Title(title_id), teil, mark).await {
            Ok(reading) => fulltext_context(&reading, |n| at.source_part_url(title_id, n)),
            Err(e) => context! { linked => true, available => false, reason => e.message },
        }
    };
    let target_quote = number::<i64>(&q.quote);
    let target_highlight = number::<i64>(&q.highlight);
    let settings = page_settings(&headers);
    let personas = persona_choices(&state, at.work).await;
    let page = SourcePage { entry: &entry, highlights: &highlights, checks: &checks, fulltext, linked, personas };
    match source_html(&at, &page, target_quote, target_highlight, q.notice.as_deref(), settings) {
        Ok(html) => Html(html).into_response(),
        Err(e) => error_response(e),
    }
}

/// Full-text part for the templates: pieces with passages, part/parts, links to the previous and next
/// part (`part_url`), origin and notice. Contents stay text (the template escapes).
fn fulltext_context(reading: &Reading, part_url: impl Fn(usize) -> String) -> Value {
    let pieces = fulltext::pieces(&reading.text, &reading.marks);
    let url = reading.source.url.as_deref().and_then(safe_link);
    context! {
        linked => true,
        available => true,
        pieces => Value::from(Serde(pieces)),
        part => reading.part,
        parts => reading.parts,
        more => reading.more,
        prev_url => (reading.part > 1).then(|| href(part_url(reading.part - 1))),
        next_url => reading.more.then(|| href(part_url(reading.part + 1))),
        origin => reading.origin,
        message => reading.message.as_deref(),
        offset => reading.offset,
        marks => reading.marks.len(),
        paged_by_remote => reading.paged_by_remote,
        title => reading.source.title.as_str(),
        url => url,
        document => reading.source.document.as_str(),
        collection => reading.source.collection.as_str(),
        title_id => reading.source.title_id,
    }
}

/// Data of the document view of a title.
struct SourcePage<'a> {
    entry: &'a TitleEntry,
    highlights: &'a [Highlight],
    checks: &'a HashMap<i64, HighlightCheck>,
    /// Full-text area ([`fulltext_context`] or a notice).
    fulltext: Value,
    /// Linked with MultiGPT: the form offers start and end in the full text.
    linked: bool,
    /// Active personas for „an Persona geben“ (Round 15).
    personas: Vec<Value>,
}

/// Active personas for „an Persona geben“; the lector of the work is selected.
async fn persona_choices(state: &AppState, work: WorkId) -> Vec<Value> {
    let lector = state.store.load_work(work).await.ok().and_then(|w| w.lector().map(|p| p.handle.clone()));
    state
        .store
        .persona_list()
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|p| p.active)
        .map(|p| {
            let selected = lector.as_deref() == Some(p.handle.as_str());
            context! { handle => p.handle, name => p.name, selected => selected }
        })
        .collect()
}

fn source_html(
    at: &At,
    page: &SourcePage<'_>,
    target_quote: Option<i64>,
    target_highlight: Option<i64>,
    notice: Option<&str>,
    settings: Value,
) -> Result<String, RpcError> {
    let (entry, highlights) = (page.entry, page.highlights);
    let title: &Title = &entry.title;
    let quotes: Vec<Value> = entry
        .quotes
        .iter()
        .map(|quote| {
            let marks: Vec<&Highlight> = highlights
                .iter()
                .filter(|h| h.quote_id == Some(quote.id) && h.status != HighlightStatus::Rejected)
                .collect();
            context! {
                id => quote.id, page => quote.page.as_str(), note => quote.note.as_str(),
                segments => Value::from(Serde(segments(&quote.text, &marks, target_highlight))),
                target => target_quote == Some(quote.id),
                url => href(at.source_url(title.id, Some(quote.id), None, None)),
            }
        })
        .collect();
    let page_of = |quote: Option<i64>| {
        quote.and_then(|id| entry.quotes.iter().find(|q| q.id == id)).map(|q| q.page.clone()).unwrap_or_default()
    };
    let marks: Vec<Value> = highlights
        .iter()
        .map(|h| {
            context! {
                id => h.id, quote_id => h.quote_id, quote_page => page_of(h.quote_id),
                locator => h.locator.as_str(), text => h.text.as_str(), note => h.note.as_str(),
                kind => h.kind.as_str(), kind_label => h.kind.label(),
                status => h.status.as_str(), status_label => h.status.label(),
                author => author_label(&h.author), persona => matches!(h.author, HighlightAuthor::Persona { .. }),
                created => h.created.as_str(),
                target => target_highlight == Some(h.id),
                url => href(at.source_url(title.id, h.quote_id, Some(h.id), None)),
                checkable => fulltext::checkable(h),
                // Check time in local time (stored as UTC); RPC `highlight.*` keeps delivering the UTC stamp.
                check => page.checks.get(&h.id).map(|c| context! {
                    result => Value::from(Serde(&c.result)), label => c.label, message => c.message.as_str(),
                    checked => crate::localtime::minutes(&c.checked),
                }),
            }
        })
        .collect();
    let kinds: Vec<Value> =
        HighlightKind::ALL.iter().map(|k| context! { value => k.as_str(), label => k.label() }).collect();
    let (work, node, rev) = at.context();
    render(
        "source.html",
        context! {
            work => work, node => node, rev => rev, settings => settings,
            title => Value::from(Serde(title)),
            quotes => quotes,
            highlights => marks,
            kinds => kinds,
            place_fields => at.fields(),
            fulltext => page.fulltext.clone(),
            range_inputs => page.linked,
            personas => page.personas.clone(),
            notice => notice_text(notice),
            library_query => at.lq.as_deref(),
            library_mode => at.mode.as_str(),
            work_url => href(at.work_url()),
            literature_url => href(at.literature_url()),
        },
    )
}

/// `GET /quelle/multigpt`: full text of a document from MultiGPT (Round 13).
async fn multigpt_page(State(state): State<Arc<AppState>>, headers: HeaderMap, Query(q): Query<PageQuery>) -> Response {
    let at = match q.at(&state).await {
        Ok(at) => at,
        Err(response) => return *response,
    };
    let Some(document) = text(&q.document) else {
        return (StatusCode::NOT_FOUND, "Dokument fehlt: document=<id> angeben").into_response();
    };
    let document = match fulltext::checked_document(document) {
        Ok(document) => document,
        Err(e) => return error_response(e),
    };
    let collection = match fulltext::checked_collection(q.collection.as_deref()) {
        Ok(collection) => collection,
        Err(e) => return error_response(e),
    };
    let section = text(&q.section).map(|s| s.chars().filter(|c| !c.is_control()).take(200).collect::<String>());
    let search = text(&q.q).map(|s| s.chars().take(1000).collect::<String>());
    let teil = number::<usize>(&q.teil).unwrap_or(1).max(1);
    let pool = state.store.pool();
    let remote = baruch_library::RemoteRef {
        way: fulltext::WAY.into(),
        collection: collection.clone(),
        document: document.clone(),
    };
    let linked = match baruch_library::title_for_remote(pool, &remote).await {
        Ok(Some(id)) => baruch_library::get_title(pool, id).await.ok(),
        Ok(None) => None,
        Err(e) => return error_response(e.into()),
    };
    let part_url = |n: usize| at.multigpt_url(&collection, &document, section.as_deref(), n, search.as_deref());
    // Round 18b: „an Persona geben“ here as well – without an adopted title, with the text of the part shown
    // (prefilled, at most MAX_PERSONA_TEXT characters) and the source reference „MultiGPT: Titel (Dokument …)“.
    let mut give_text = String::new();
    let mut give_title = document.clone();
    let view = if crate::guard::check(&headers).is_err() {
        context! { linked => true, available => false, reason => FOREIGN_FULLTEXT, load_url => href(part_url(teil)) }
    } else {
        let target = Target::Document { document: document.clone(), collection: collection.clone() };
        match read_source(&state, &target, teil, search.as_deref()).await {
            Ok(reading) => {
                give_text = reading.text.trim().chars().take(MAX_PERSONA_TEXT).collect();
                if !reading.source.title.trim().is_empty() {
                    give_title = reading.source.title.clone();
                }
                fulltext_context(&reading, part_url)
            }
            Err(e) => context! { linked => true, available => false, reason => e.message },
        }
    };
    let give_source: String = format!("MultiGPT: {give_title} (Dokument {document})")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(MAX_SOURCE_CHARS)
        .collect();
    let personas = if at.key.is_some() { persona_choices(&state, at.work).await } else { Vec::new() };
    let (work, node, rev) = at.context();
    let linked_view = linked.as_ref().map(|entry| {
        context! {
            id => entry.title.id, title => entry.title.title.as_str(), key => entry.title.key.as_str(),
            url => href(at.source_url(entry.title.id, None, None, None)),
        }
    });
    let ctx = context! {
        work => work, node => node, rev => rev, settings => page_settings(&headers),
        document => document.as_str(), collection => collection.as_str(), section => section.as_deref(),
        q => search.as_deref(), fulltext => view, linked => linked_view,
        place_fields => at.fields(), notice => notice_text(q.notice.as_deref()),
        library_query => at.lq.as_deref(), library_mode => at.mode.as_str(),
        work_url => href(at.work_url()), literature_url => href(at.literature_url()),
        has_key => at.key.is_some(), personas => personas, give_text => give_text, give_source => give_source,
        max_persona_text => MAX_PERSONA_TEXT,
    };
    match render("source_multigpt.html", ctx) {
        Ok(html) => Html(html).into_response(),
        Err(e) => error_response(e),
    }
}

// ---- Forms -------------------------------------------------------------------------------------

/// `POST /literatur/act`: all fields as text (empty fields are „nicht angegeben“).
#[derive(Debug, Default, Deserialize)]
struct ActForm {
    action: String,
    work: Option<String>,
    node: Option<String>,
    /// Fixed key of the section; mandatory for `cite`.
    key: Option<String>,
    rev: Option<String>,
    view: Option<String>,
    lq: Option<String>,
    lmode: Option<String>,
    title: Option<String>,
    quote: Option<String>,
    highlight: Option<String>,
    locator: Option<String>,
    start: Option<String>,
    end: Option<String>,
    text: Option<String>,
    kind: Option<String>,
    note: Option<String>,
    status: Option<String>,
    /// Mandatory checkbox when deleting.
    confirm: Option<String>,
    /// Round 13 (`adopt`): document and collection in MultiGPT.
    document: Option<String>,
    collection: Option<String>,
    /// Round 15 (`persona`): persona, source reference and question.
    persona: Option<String>,
    source: Option<String>,
    question: Option<String>,
}

fn text(value: &Option<String>) -> Option<&str> {
    value.as_deref().map(str::trim).filter(|v| !v.is_empty())
}

fn kind_param(value: Option<&str>) -> Result<Option<HighlightKind>, RpcError> {
    value
        .map(|k| {
            HighlightKind::parse(k)
                .ok_or_else(|| invalid(format!("unbekannte Art {k:?} (quote, core, contradiction, check)")))
        })
        .transpose()
}

fn status_param(value: Option<&str>) -> Result<Option<HighlightStatus>, RpcError> {
    value
        .map(|s| {
            HighlightStatus::parse(s)
                .ok_or_else(|| invalid(format!("unbekannter Status {s:?} (active, proposed, rejected)")))
        })
        .transpose()
}

/// Character range `start..end` of a highlight (both or neither).
type Span = (Option<i64>, Option<i64>);

/// New highlight of the human from the form or RPC fields.
fn new_highlight(
    title: i64,
    quote: Option<i64>,
    locator: &str,
    (start, end): Span,
    text: &str,
    kind: HighlightKind,
    note: &str,
) -> NewHighlight {
    NewHighlight {
        title_id: title,
        quote_id: quote,
        locator: locator.to_owned(),
        start,
        end,
        text: text.to_owned(),
        kind,
        note: note.to_owned(),
        author: HighlightAuthor::Human { name: mirror::HUMAN_NAME.into() },
    }
}

async fn act(State(state): State<Arc<AppState>>, Form(form): Form<ActForm>) -> Response {
    let at = match At::resolve(
        &state,
        number(&form.work),
        (number(&form.node), number(&form.key)),
        number(&form.rev),
        form.view.as_deref(),
        (form.lq.as_deref(), form.lmode.as_deref()),
    )
    .await
    {
        Ok(at) => at,
        Err(e) => return error_response(e.into()),
    };
    if form.action == "persona" {
        return persona_act(&state, &at, &form).await;
    }
    match act_on(&state, &at, &form).await {
        Ok(url) => Redirect::to(&url).into_response(),
        Err(e) => error_response(e),
    }
}

/// Source reference from the form or from the title: one line, at most [`MAX_SOURCE_CHARS`] characters.
async fn source_line(state: &AppState, form_source: Option<&str>, title: Option<i64>, locator: Option<&str>) -> String {
    let given = form_source.map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned);
    let line = match (given, title) {
        (Some(line), _) => line,
        (None, Some(title)) => match baruch_library::get_title(state.store.pool(), title).await {
            Ok(entry) => {
                let mut line = format!("@{}", entry.title.key);
                if let Some(loc) = locator.map(str::trim).filter(|l| !l.is_empty()) {
                    line.push_str(&format!(", {loc}"));
                }
                line
            }
            Err(_) => "Literatur".to_owned(),
        },
        (None, None) => "Literatur".to_owned(),
    };
    line.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(MAX_SOURCE_CHARS).collect()
}

/// `action=persona` (without JavaScript): give to the persona, then back to the section; without a section or on
/// an error a notice page (status 200) instead of a bare error.
async fn persona_act(state: &AppState, at: &At, form: &ActForm) -> Response {
    let back = at.work_url();
    let Some(key) = number::<i64>(&form.key) else {
        return crate::assist::routes::notice_page(PICK_SECTION, &back, None);
    };
    let source = source_line(state, text(&form.source), number(&form.title), text(&form.locator)).await;
    let order = ToPersona {
        work: at.work,
        key,
        persona: text(&form.persona).map(str::to_owned),
        text: form.text.clone().unwrap_or_default(),
        source,
        title: number(&form.title),
        highlight: number(&form.highlight),
        question: text(&form.question).map(str::to_owned),
    };
    match to_persona(state, &order).await {
        Ok(answer) => {
            let url = crate::view::node_url(at.work, key, at.rev, at.preview, "", &at.lq_tail());
            if let Some(compressed) = answer.get("compressed").filter(|c| c.is_object()) {
                return crate::assist::routes::notice_page(
                    "Die Antwort der Persona ist gespeichert.",
                    &url,
                    Some(compressed),
                );
            }
            Redirect::to(&url).into_response()
        }
        Err(e) => crate::assist::routes::notice_page(&e.message, &back, None),
    }
}

/// Order „an Persona geben“ (form `action=persona`, RPC `literature.to_persona`, Bible page).
#[derive(Debug, Clone, Default)]
pub struct ToPersona {
    pub work: WorkId,
    pub key: i64,
    /// Handle; empty or `None` = the lector of the work.
    pub persona: Option<String>,
    /// Selected text (at most [`MAX_PERSONA_TEXT`] characters).
    pub text: String,
    /// Source reference (one line), stands as the source on the post of the persona.
    pub source: String,
    pub title: Option<i64>,
    pub highlight: Option<i64>,
    pub question: Option<String>,
}

/// Give selected text to a persona: the question (with text and source) stands as a post of the human in the
/// section, the answer as a post of the persona with the source. Nothing in the book text.
pub async fn to_persona(state: &AppState, order: &ToPersona) -> Result<serde_json::Value, RpcError> {
    let text = order.text.trim();
    if text.is_empty() {
        return Err(invalid("text: markierten Text angeben"));
    }
    if text.chars().count() > MAX_PERSONA_TEXT {
        return Err(invalid(format!("text: höchstens {MAX_PERSONA_TEXT} Zeichen")));
    }
    let source: String = order.source.split_whitespace().collect::<Vec<_>>().join(" ");
    if source.chars().count() > MAX_SOURCE_CHARS {
        return Err(invalid(format!("source: höchstens {MAX_SOURCE_CHARS} Zeichen in einer Zeile")));
    }
    let source = if source.is_empty() { "Literatur".to_owned() } else { source };
    state.store.node_seq(order.work, order.key).await?;
    let persona = match order.persona.as_deref().map(str::trim).filter(|p| !p.is_empty()) {
        Some(p) => p.to_owned(),
        None => {
            let loaded = state.store.load_work(order.work).await?;
            loaded
                .lector()
                .map(|p| p.handle.clone())
                .ok_or_else(|| invalid("Keine Persona ist aktiv (kein Lektor)."))?
        }
    };
    let question = order
        .question
        .as_deref()
        .map(str::trim)
        .filter(|q| !q.is_empty())
        .unwrap_or("Was sagt dieser Text zu meinem Abschnitt?");
    let head = format!("{question}\n\nAus der Literatur ({source}):\n„");
    let room = crate::assist::MAX_QUESTION_CHARS.saturating_sub(head.chars().count() + 12);
    let quoted = if text.chars().count() > room {
        format!("{} …“ (gekürzt)", text.chars().take(room).collect::<String>())
    } else {
        format!("{text}“")
    };
    let mut sources = vec![source];
    if let Some(h) = order.highlight {
        sources.push(format!("Hervorhebung {h}"));
    }
    let order = crate::assist::Order {
        work: order.work,
        key: order.key,
        persona,
        mode: crate::assist::Mode::Ask,
        text: format!("{head}{quoted}"),
        rev: None,
        asker: None,
        post_question: true,
        sources,
        web: true,
        scope: crate::assist::Scope::Section,
        frame: Default::default(),
        links: Default::default(),
        basis: None,
    };
    let outcome = crate::assist::run(state, &order, None).await?;
    Ok(crate::assist::rpc::outcome_json(&outcome))
}

/// Take over a persona highlight as one's own: a new highlight of the human (active) with text, range, kind and
/// locator of the original; the original stays a proposal. Own highlights cannot be taken over once more,
/// nor can a proposal that was already taken over.
pub async fn own_highlight(state: &AppState, id: i64) -> Result<(Highlight, Highlight), RpcError> {
    let pool = state.store.pool();
    let original = baruch_library::get_highlight(pool, id).await?;
    let HighlightAuthor::Persona { handle, .. } = &original.author else {
        return Err(RpcError::new(REFUSED, "Diese Hervorhebung ist schon deine eigene."));
    };
    let note = match original.note.trim() {
        "" => format!("übernommen von {handle} (Hervorhebung {id})"),
        note => format!("{note} – übernommen von {handle} (Hervorhebung {id})"),
    };
    let existing = baruch_library::list_highlights(pool, Some(original.title_id)).await?;
    let marker = format!("(Hervorhebung {id})");
    if existing.iter().any(|h| matches!(h.author, HighlightAuthor::Human { .. }) && h.note.ends_with(&marker)) {
        return Err(RpcError::new(REFUSED, "Diesen Vorschlag hast du schon als eigene Hervorhebung übernommen."));
    }
    let new = NewHighlight {
        title_id: original.title_id,
        quote_id: original.quote_id,
        locator: original.locator.clone(),
        start: original.start,
        end: original.end,
        text: original.text.clone(),
        kind: original.kind,
        note,
        author: HighlightAuthor::Human { name: mirror::HUMAN_NAME.into() },
    };
    let own = baruch_library::add_highlight(pool, &new).await?;
    let saved = baruch_library::get_highlight(pool, own).await?;
    let _ = check_highlight(state, &saved).await;
    let original = baruch_library::get_highlight(pool, id).await?;
    Ok((saved, original))
}

/// Carries out the form action and returns the address for the redirect.
async fn act_on(state: &AppState, at: &At, form: &ActForm) -> Result<String, RpcError> {
    let pool = state.store.pool();
    let title = number::<i64>(&form.title);
    let quote = number::<i64>(&form.quote);
    let highlight = number::<i64>(&form.highlight);
    let need_title = || title.ok_or_else(|| invalid("title: Quelle fehlt"));
    let add = |kind: HighlightKind| -> Result<NewHighlight, RpcError> {
        Ok(new_highlight(
            need_title()?,
            quote,
            text(&form.locator).unwrap_or(""),
            (number(&form.start), number(&form.end)),
            form.text.as_deref().unwrap_or(""),
            kind,
            text(&form.note).unwrap_or(""),
        ))
    };
    match form.action.as_str() {
        "highlight" => {
            let kind = kind_param(text(&form.kind))?.unwrap_or(HighlightKind::Core);
            let new = add(kind)?;
            let id = baruch_library::add_highlight(pool, &new).await?;
            crate::search_hooks::changed(state, at.work);
            // Without a quote, with a range on a linked title: check against the full text. The highlight of the
            // human is already stored; if the check itself fails, it stays unchecked.
            let saved = baruch_library::get_highlight(pool, id).await?;
            let _ = check_highlight(state, &saved).await;
            Ok(at.source_url(new.title_id, new.quote_id, Some(id), None))
        }
        "adopt" => {
            let document = text(&form.document).ok_or_else(|| invalid("document: Dokument fehlt"))?;
            let (entry, _) = adopt_source(state, document, text(&form.collection)).await?;
            Ok(at.source_url(entry.title.id, None, None, Some("adopted")))
        }
        "check" => {
            let id = highlight.ok_or_else(|| invalid("highlight: Hervorhebung fehlt"))?;
            let h = baruch_library::get_highlight(pool, id).await?;
            if !fulltext::checkable(&h) {
                return Err(invalid(NOT_CHECKABLE));
            }
            check_highlight(state, &h).await?;
            Ok(at.source_url(h.title_id, h.quote_id, Some(id), Some("checked")))
        }
        "cite" => {
            // Without a section no quote (formerly it landed silently in section 0). The section comes
            // only from the fixed key (`key`), not from the position. Before creating a
            // highlight check that it exists, so that nothing is created halfway.
            let key = number::<i64>(&form.key).ok_or_else(|| invalid("Abschnitt fehlt."))?;
            state.store.node_seq(at.work, key).await?;
            let id = match highlight {
                Some(id) => id,
                None => {
                    let id = baruch_library::add_highlight(pool, &add(HighlightKind::Quote)?).await?;
                    crate::search_hooks::changed(state, at.work);
                    id
                }
            };
            let (_, rev) = cite(state, at.work, id, key).await?;
            Ok(crate::view::node_url(at.work, key, Some(rev), false, "", &at.lq_tail()))
        }
        "own" => {
            let id = highlight.ok_or_else(|| invalid("highlight: Hervorhebung fehlt"))?;
            let (own, _) = own_highlight(state, id).await?;
            crate::search_hooks::changed(state, at.work);
            Ok(at.source_url(own.title_id, own.quote_id, Some(own.id), Some("owned")))
        }
        "similar" => {
            let words = similar_query(form.text.as_deref().unwrap_or(""));
            let target = At { lq: (!words.is_empty()).then_some(words), ..at.clone() };
            Ok(target.literature_url())
        }
        "update" => {
            let id = highlight.ok_or_else(|| invalid("highlight: Hervorhebung fehlt"))?;
            let kind = kind_param(text(&form.kind))?;
            let status = status_param(text(&form.status))?;
            let updated = update(state, id, kind, form.note.as_deref(), status).await?;
            crate::search_hooks::changed(state, at.work);
            Ok(at.source_url(updated.title_id, updated.quote_id, Some(id), None))
        }
        "delete" => {
            let id = highlight.ok_or_else(|| invalid("highlight: Hervorhebung fehlt"))?;
            if form.confirm.as_deref() != Some("on") && form.confirm.as_deref() != Some("1") {
                return Err(RpcError::new(REFUSED, "Zum Löschen das Kästchen „wirklich löschen“ ankreuzen."));
            }
            let old = baruch_library::get_highlight(pool, id).await?;
            baruch_library::delete_highlight(pool, id).await?;
            crate::search_hooks::changed(state, at.work);
            Ok(at.source_url(old.title_id, None, None, Some("deleted")))
        }
        other => Err(invalid(format!("unbekannte Aktion {other:?}"))),
    }
}

/// „Ähnliches suchen“: the three longest words of the selected text (the search requires all of them).
fn similar_query(text: &str) -> String {
    let mut words: Vec<&str> = text.split(|c: char| !c.is_alphanumeric()).filter(|w| w.chars().count() >= 3).collect();
    words.sort_by_key(|w| std::cmp::Reverse(w.chars().count()));
    let mut chosen: Vec<&str> = Vec::new();
    for word in words {
        if chosen.len() == 3 {
            break;
        }
        if !chosen.iter().any(|c| c.to_lowercase() == word.to_lowercase()) {
            chosen.push(word);
        }
    }
    // In the order of the text.
    chosen.sort_by_key(|w| text.find(w).unwrap_or(usize::MAX));
    chosen.join(" ")
}

// ---- Common procedures of RPC and form ---------------------------------------------------------

/// Change the status: a persona highlight stays a proposal (or is rejected), `active` exists
/// only for highlights of the human.
async fn update(
    state: &AppState,
    id: i64,
    kind: Option<HighlightKind>,
    note: Option<&str>,
    status: Option<HighlightStatus>,
) -> Result<Highlight, RpcError> {
    let pool = state.store.pool();
    let old = baruch_library::get_highlight(pool, id).await?;
    if status == Some(HighlightStatus::Active) && matches!(old.author, HighlightAuthor::Persona { .. }) {
        return Err(RpcError::new(REFUSED, "Hervorhebungen von Personas bleiben Vorschläge; aktiv sind nur eigene."));
    }
    Ok(baruch_library::update_highlight(pool, id, kind, note, status).await?)
}

/// Quote as a proposal revision: current text of the section + empty line + `#quote(...)[...]`.
/// Runs under the write lock: the fixed key `node_key` is translated into the position only under it
/// (nobody rearranges in between), and the text does not change between reading and appending.
/// The current revision stays as it is. Result: position and number of the proposal.
async fn cite(state: &AppState, work: WorkId, highlight: i64, node_key: i64) -> Result<(usize, u32), RpcError> {
    let pool = state.store.pool();
    let h = baruch_library::get_highlight(pool, highlight).await?;
    let key = baruch_library::get_title(pool, h.title_id).await?.title.key;
    let _held = state.write_lock.lock().await;
    let node = state.store.node_seq(work, node_key).await?;
    let loaded = state.store.load_work(work).await?;
    let n = loaded.nodes.get(node).ok_or(StoreError::UnknownNode { work, node })?;
    let current = n
        .revisions
        .iter()
        .find(|r| r.status == baruch_core::RevisionStatus::Current)
        .ok_or(StoreError::Refused("Vorschläge brauchen eine aktuelle Revision."))?;
    let mut body = current.body.clone();
    if !body.is_empty() && !body.ends_with('\n') {
        body.push('\n');
    }
    if !body.is_empty() {
        body.push('\n');
    }
    body.push_str(&baruch_library::cite_typst(&h, &key));
    body.push('\n');
    let rev =
        state.store.add_proposal(work, node, &body, CITE_PERSONA, CITE_MODEL, &format!("Zitat aus @{key}")).await?;
    crate::mirror::side_refs_after_proposal(state, &_held, work).await;
    Ok((node, rev))
}

// ---- RPC ---------------------------------------------------------------------------------------

/// Does the method belong to the literature? (`literature.render`, `highlight.*`, since Round 13 `source.read` and
/// `source.adopt`)
pub fn handles(method: &str) -> bool {
    method == "literature.render"
        || method == "literature.to_persona"
        || method.starts_with("highlight.")
        || method == "source.read"
        || method == "source.adopt"
}

/// Message when a highlight cannot be checked against the full text.
pub const NOT_CHECKABLE: &str =
    "Gegen den Volltext prüfen lassen sich nur Hervorhebungen ohne Zitat mit Zeichenbereich (start, end).";

/// Highlight as JSON with its check (`check`, otherwise `null`).
fn with_check(h: &Highlight, check: Option<&HighlightCheck>) -> serde_json::Value {
    let mut value = json!(h);
    value["check"] = json!(check);
    value
}

#[derive(Deserialize)]
struct SourceReadParams {
    title: Option<i64>,
    document: Option<String>,
    collection: Option<String>,
    part: Option<usize>,
    q: Option<String>,
}

#[derive(Deserialize)]
struct AdoptParams {
    document: String,
    collection: Option<String>,
}

/// `source.read` (RPC) and `source_read` (MCP): part of the full text of a linked title or document.
pub async fn source_read(state: &AppState, params_value: serde_json::Value) -> Result<serde_json::Value, RpcError> {
    let p: SourceReadParams = params(params_value)?;
    let target = match (p.title, p.document) {
        (Some(title), _) => Target::Title(title),
        (None, Some(document)) => Target::Document { document, collection: p.collection.unwrap_or_default() },
        (None, None) => return Err(invalid("title (verknüpfter Titel) oder document (Dokument in MultiGPT) angeben")),
    };
    let q = p.q.as_deref().map(str::trim).filter(|q| !q.is_empty());
    // Round 23a (code review ki-1): `q` only serves the marking; an overlong request is refused instead of searching
    // the part text for each of its words (`/mcp` accepts up to 32 MiB).
    if q.is_some_and(|q| q.chars().count() > fulltext::MAX_MARK_QUERY_CHARS) {
        return Err(invalid(format!(
            "q: höchstens {} Zeichen (die Anfrage dient nur dem Markieren der Fundstellen).",
            fulltext::MAX_MARK_QUERY_CHARS
        )));
    }
    let reading = read_source(state, &target, p.part.unwrap_or(1), q).await?;
    serde_json::to_value(reading).map_err(|e| RpcError::new(INTERNAL, format!("Antwort: {e}")))
}

#[derive(Deserialize)]
struct RenderParams {
    node: Option<usize>,
    key: Option<i64>,
    rev: Option<u32>,
    view: Option<String>,
    lq: Option<String>,
    lmode: Option<String>,
}

#[derive(Deserialize)]
struct ListParams {
    title: Option<i64>,
}

#[derive(Deserialize)]
struct AddParams {
    title: i64,
    quote: Option<i64>,
    #[serde(default)]
    locator: String,
    start: Option<i64>,
    end: Option<i64>,
    text: String,
    kind: HighlightKind,
    #[serde(default)]
    note: String,
}

#[derive(Deserialize)]
struct UpdateParams {
    highlight: i64,
    kind: Option<HighlightKind>,
    note: Option<String>,
    status: Option<HighlightStatus>,
}

#[derive(Deserialize)]
struct IdParams {
    highlight: i64,
}

#[derive(Deserialize)]
struct CiteParams {
    highlight: i64,
}

#[derive(Deserialize)]
struct ToPersonaParams {
    persona: Option<String>,
    text: String,
    #[serde(default)]
    source: String,
    title: Option<i64>,
    highlight: Option<i64>,
    question: Option<String>,
}

fn params<T: for<'de> Deserialize<'de>>(value: serde_json::Value) -> Result<T, RpcError> {
    let value = if value.is_null() { json!({}) } else { value };
    serde_json::from_value(value).map_err(|e| invalid(e.to_string()))
}

/// RPC methods of the literature; `work` is already resolved (`rpc::work_param`).
pub async fn call(
    state: &AppState,
    work: WorkId,
    method: &str,
    params_value: serde_json::Value,
) -> Result<serde_json::Value, RpcError> {
    let pool = state.store.pool();
    match method {
        "literature.render" => {
            let p: RenderParams = params(params_value)?;
            let search = (p.lq.as_deref(), p.lmode.as_deref());
            let at = At::resolve(state, Some(work), (p.node, p.key), p.rev, p.view.as_deref(), search).await?;
            // `POST /rpc` is behind `guard::same_origin`.
            let lit = literature(state, &at, true).await?;
            let mut html = render("fragments/library.html", literature_context(&at, &lit))?;
            // Round 21c: images of the work for the search below the hits (in the popup as well).
            html.push_str(&image_hits_html(state, &at).await);
            Ok(json!({ "html": html, "lq": at.lq, "lmode": at.mode.as_str(), "url": at.literature_url() }))
        }
        "highlight.list" => {
            let p: ListParams = params(params_value)?;
            let checks = baruch_library::highlight_check_entries(pool, p.title).await?;
            let list: Vec<serde_json::Value> = baruch_library::list_highlights(pool, p.title)
                .await?
                .iter()
                .map(|h| with_check(h, checks.get(&h.id)))
                .collect();
            Ok(json!({ "highlights": list }))
        }
        "highlight.add" => {
            let p: AddParams = params(params_value)?;
            let new = new_highlight(p.title, p.quote, &p.locator, (p.start, p.end), &p.text, p.kind, &p.note);
            let id = baruch_library::add_highlight(pool, &new).await?;
            crate::search_hooks::changed(state, work);
            let saved = baruch_library::get_highlight(pool, id).await?;
            let check = check_highlight(state, &saved).await?;
            Ok(json!({ "highlight": with_check(&saved, check.as_ref()), "check": check }))
        }
        "highlight.check" => {
            let p: IdParams = params(params_value)?;
            let h = baruch_library::get_highlight(pool, p.highlight).await?;
            if !fulltext::checkable(&h) {
                return Err(invalid(NOT_CHECKABLE));
            }
            let check = check_highlight(state, &h).await?;
            Ok(json!({ "highlight": with_check(&h, check.as_ref()), "check": check }))
        }
        "source.read" => source_read(state, params_value).await,
        "source.adopt" => {
            let p: AdoptParams = params(params_value)?;
            let (title, remote) = adopt_source(state, &p.document, p.collection.as_deref()).await?;
            Ok(json!({ "title": title, "remote": remote }))
        }
        "highlight.own" => {
            let p: IdParams = params(params_value)?;
            let (own, original) = own_highlight(state, p.highlight).await?;
            crate::search_hooks::changed(state, work);
            Ok(json!({ "highlight": own, "original": original }))
        }
        "literature.to_persona" => {
            if given(&params_value, "node") {
                return Err(invalid("node (Stelle) wird nicht angenommen; key mitgeben"));
            }
            let key = key_param(&params_value)?.ok_or_else(|| invalid(PICK_SECTION))?;
            let p: ToPersonaParams = params(params_value)?;
            let order = ToPersona {
                work,
                key,
                persona: p.persona,
                text: p.text,
                source: p.source,
                title: p.title,
                highlight: p.highlight,
                question: p.question,
            };
            to_persona(state, &order).await
        }
        "highlight.update" => {
            let p: UpdateParams = params(params_value)?;
            let updated = update(state, p.highlight, p.kind, p.note.as_deref(), p.status).await?;
            crate::search_hooks::changed(state, work);
            Ok(json!({ "highlight": updated }))
        }
        "highlight.delete" => {
            let p: IdParams = params(params_value)?;
            baruch_library::delete_highlight(pool, p.highlight).await?;
            crate::search_hooks::changed(state, work);
            Ok(json!({ "deleted": p.highlight }))
        }
        "highlight.cite" => {
            if given(&params_value, "node") {
                return Err(invalid("node (Stelle) wird nicht angenommen; key mitgeben"));
            }
            let key = key_param(&params_value)?
                .ok_or_else(|| invalid("key fehlt: Abschnitt über seinen festen Schlüssel angeben"))?;
            let p: CiteParams = params(params_value)?;
            let (node, rev) = cite(state, work, p.highlight, key).await?;
            Ok(json!({ "node": node, "key": key, "rev": rev }))
        }
        _ => Err(RpcError::new(METHOD_NOT_FOUND, format!("unknown method {method}"))),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn snippets_drop_the_leading_heading_line_and_blank_lines() {
        assert_eq!(
            super::display_snippet("= Die Bienen\n\nIm Frühling schwärmen sie.\n\nDer Imker."),
            "Im Frühling schwärmen sie.\nDer Imker.",
            "die Überschrift steht schon als Titel da, Leerzeilen erzeugten Lücken in der Liste"
        );
        assert_eq!(super::display_snippet("= Nur Titel"), "= Nur Titel", "nur eine Überschrift bleibt stehen");
        assert_eq!(super::display_snippet("Ein Satz = kein Titel."), "Ein Satz = kein Titel.", "sonst unverändert");
    }

    #[test]
    fn the_word_search_note_appears_once_when_no_way_answers() {
        use baruch_rag::{Answer, Way, WayReport, WayState};
        let answer = Answer {
            work: 1,
            query: "x".into(),
            hits: vec![],
            ways: vec![
                WayReport::new(
                    Way::Lancedb,
                    WayState::Failed,
                    "Kein Einbettungsmodell erreichbar: aus. Die Wortlaut-Suche (FTS) geht weiter.",
                    0,
                    1,
                ),
                WayReport::new(Way::Pgvector, WayState::Off, "Für dieses Werk nicht gewählt.", 0, 0),
                WayReport::new(Way::Multigpt, WayState::Off, "In dieser Anfrage nicht gefragt.", 0, 0),
            ],
        };
        let view = super::rag_view(&answer);
        let message = view["message"].as_str().unwrap();
        assert_eq!(
            message.matches("Wortlaut-Suche (FTS) geht weiter").count(),
            1,
            "der Satz zur Wortlaut-Suche steht genau einmal (am Ende), nicht je Weg: {message}"
        );
        assert!(message.contains("LanceDB: Kein Einbettungsmodell erreichbar: aus;"), "Grund je Weg bleibt: {message}");
    }

    use super::*;

    fn mark(id: i64, start: i64, end: i64) -> Highlight {
        Highlight {
            id,
            title_id: 1,
            quote_id: Some(1),
            locator: String::new(),
            start: Some(start),
            end: Some(end),
            text: String::new(),
            kind: HighlightKind::Core,
            note: String::new(),
            author: HighlightAuthor::Human { name: "Autor".into() },
            status: HighlightStatus::Active,
            created: String::new(),
        }
    }

    #[test]
    fn segments_split_at_character_ranges_and_skip_bad_ones() {
        let (a, b, c, d) = (mark(1, 4, 9), mark(2, 6, 12), mark(3, 15, 99), mark(4, 14, 15));
        let parts = segments("Die Liebe ist langmütig", &[&a, &b, &c, &d], Some(4));
        let texts: Vec<(&str, Option<i64>, bool)> =
            parts.iter().map(|s| (s.text.as_str(), s.highlight, s.target)).collect();
        assert_eq!(
            texts,
            vec![
                ("Die ", None, false),
                ("Liebe", Some(1), false),
                (" ist ", None, false),
                ("l", Some(4), true),
                ("angmütig", None, false),
            ],
            "Bereich 2 überlappt 1, Bereich 3 geht übers Ende – beide fallen weg; Umlaut zählt als ein Zeichen"
        );
        assert_eq!(segments("", &[], None).len(), 1, "leerer Text ergibt ein leeres Stück");
    }

    #[test]
    fn similar_search_takes_the_three_longest_words_in_text_order() {
        assert_eq!(similar_query("Die Liebe ist langmütig und freundlich, die Liebe"), "Liebe langmütig freundlich");
        assert_eq!(similar_query(" , "), "", "ohne Wörter keine Suche");
    }
}
