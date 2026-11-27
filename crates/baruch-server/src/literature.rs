// SPDX-License-Identifier: AGPL-3.0-or-later
//! Literatur als Popup und eigene Seiten: Suche über Werk und Bibliothek, Dokumentansicht einer
//! Quelle mit Hervorhebungen, Hervorhebungen anlegen, ändern, löschen und als Zitat übernehmen.
//!
//! Seiten (ohne JavaScript bedienbar, [`pages`]):
//! - `GET /literatur?work&key&rev&lq&lmode` (`node` nur Rückfall) – Suchformular, Treffer, Titel mit Zitaten und
//!   Hervorhebungen, Link zurück zum Werk. Mit JavaScript zeigt die Kopfzeile denselben Inhalt als `<dialog>`.
//!   `lmode` (Runde 12) wählt die Suche: `fts` (Vorgabe, Wortlaut über FTS5) oder `rag` (Bedeutung über
//!   [`crate::search_hooks::rag_search`], alle Wege des Werks). Antwortet kein Weg, sagt die Seite warum und
//!   verlinkt dieselbe Suche im Wortlaut – sie schaltet nie still um. `lmode` reist wie `lq` in allen Links
//!   und Formularen mit. Die Arbeitsseite bleibt bei FTS.
//! - `GET /quelle?work&key&title&quote?&highlight?&lq&teil?` – Dokumentansicht: Angaben, Zitate (Anker
//!   `#zitat-<id>`), alle Hervorhebungen (Anker `#hervorhebung-<id>`, mit Prüfung gegen den Volltext), Ziel
//!   mit Klasse `target` bzw. `<mark>`; bei einem mit MultiGPT verknüpften Titel der Volltext (Teil `teil`,
//!   Runde 13); darunter das Formular für markierten Text.
//! - `GET /quelle/multigpt?work&key&rev&lq&lmode&collection&document&section&teil&q` (Runde 13) – Volltext eines
//!   Dokuments aus MultiGPT, teilweise mit Links „vorheriger/nächster Teil“, Fundstelle `q` als `<mark>`, Knopf
//!   „In die Bibliothek übernehmen“; ohne JavaScript. Fällt MultiGPT aus, nennt die Seite den Grund (oder
//!   zeigt die lokale Kopie des Abgleichs). Siehe [`fulltext`]. Runde 18b: mit Abschnitt (`key`) das Formular
//!   „An Persona geben“ (`action=persona`, Text des gezeigten Teils vorbelegt, Quelle „MultiGPT: Titel (Dokument
//!   …)“, ohne übernommenen Titel).
//!
//! Formulare ([`writes`], hinter `guard::same_origin`): `POST /literatur/act` mit `action` =
//! `highlight`, `cite`, `persona`, `own`, `similar`, `update`, `delete`, `adopt`, `check`, danach Weiterleitung.
//! `persona` (Runde 15, „an Persona geben“): markierter Text (`text`, höchstens 4000 Zeichen) mit Quellenangabe
//! (`source`, eine Zeile) und Frage (`question`) an die gewählte Persona (`persona`, Vorgabe der Lektor); sie
//! antwortet als Beitrag im Abschnitt `key` (Pflicht, sonst Hinweisseite „Abschnitt wählen“) mit der Quelle
//! `source`. Nichts am Buchtext. Scheitert die Persona, zeigt eine Hinweisseite (Status 200) den Grund.
//! `own` (Runde 15): eine Persona-Hervorhebung als eigene übernehmen – neue Hervorhebung des Menschen (aktiv),
//! das Original bleibt Vorschlag.
//!
//! Bibeln in der Literatur (Runde 15, [`bibles`]): Kasten „Bibeln“ im Popup und auf `/literatur`, Treffergruppen
//! „Bibel“ und „Kommentar“ im Wortlaut-Modus, eigene Seite `/literatur/bibel`.
//!
//! Bilder in der Literatur (Runde 21c, [`image_hits`]): Mit einer Suche zeigen `/literatur` (ohne JavaScript) und das
//! Popup (`literature.render`) unter den Treffern den Kasten „Bilder des Werks“: jedes Bild, in dessen Name,
//! Bildunterschrift oder Alternativtext (aktuelle Fassung) alle Wörter der Suche vorkommen (ohne Groß/Klein), mit
//! Vorschaubild (`/werke/{id}/bilder/{name}/vorschau?b=160`) und Link auf die Seite des Bilds; höchstens
//! [`MAX_IMAGE_HITS`]. Texte maskiert (eigene kleine Vorlage [`IMAGE_HITS_TEMPLATE`], `fragments/library.html` bleibt
//! unberührt).
//!
//! Abschnitt: Beide Seiten tragen den **festen Schlüssel** `key` des Abschnitts selbst (aus `?key=`,
//! sonst aus der Stelle `?node=` beim Laden bestimmt) in ihren Links und Formularen. Zitieren
//! (`cite`, `highlight.cite`) nimmt nur `key` an und löst ihn unter der Schreibsperre auf; die
//! Stelle `node` aus der Adresse ist reine Anzeige (Vertrag Runde 5): Nach einer Umstellung in einem
//! anderen Tab landete das Zitat sonst in einem fremden Abschnitt.
//!
//! RPC ([`handles`], [`call`]; aus `rpc::call` heraus):
//! - `literature.render {work?, node?, key?, rev?, lq?, lmode?}` → `{html, lq, lmode, url}` (Inhalt des Popups)
//! - `highlight.list {title?}` → `{highlights}` (je Hervorhebung `check` oder `null`)
//! - `highlight.add {title, quote?, locator?, start?, end?, text, kind, note?}` → `{highlight, check}`: ohne
//!   Zitat mit Bereich an einem verknüpften Titel gegen den Volltext geprüft (Runde 13)
//! - `highlight.check {highlight}` → `{highlight, check}` (erneut prüfen)
//! - `source.read {title? | document, collection?, part?, q?}` → Teil des Volltexts (Runde 13)
//! - `source.adopt {document, collection?}` → `{title, remote}` (nur der Mensch; nicht über MCP)
//! - `highlight.update {highlight, kind?, note?, status?}` → `{highlight}`
//! - `highlight.delete {highlight}` → `{deleted}`
//! - `highlight.cite {highlight, key}` → `{node, key, rev}`: Vorschlag-Revision mit `#quote(...)[...]`
//!   über die unveränderte `Store::add_proposal` (Autor `@literatur`, Modell `baruch-zitat`); `key`
//!   ist Pflicht, `node` → -32602.
//! - `highlight.own {highlight}` → `{highlight, original}` (Runde 15): eigene Kopie einer Persona-Hervorhebung
//!   (aktiv, Autor der Mensch), das Original bleibt Vorschlag; nicht über MCP.
//! - `literature.to_persona {work, key, persona, text, source, title?, highlight?, question?}` (Runde 15) → wie
//!   `persona.ask` (Beitrag der Persona mit Quelle `source`, die Frage als Beitrag des Menschen).
//!
//! Hervorhebungen über RPC und Formular stammen immer vom Menschen (`active`); Personas legen sie
//! später nur als Vorschlag an (`HighlightAuthor::Persona`). Nach jedem Anlegen, Ändern und Löschen
//! einer Hervorhebung meldet die Literatur das der Bedeutungssuche ([`crate::search_hooks::changed`]).

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

/// Urheber und Modell der Zitat-Vorschläge aus der Literatur.
pub const CITE_PERSONA: &str = "@literatur";
pub const CITE_MODEL: &str = "baruch-zitat";

/// Längster markierter Text für „an Persona geben“ (Zeichen).
pub const MAX_PERSONA_TEXT: usize = 4000;
/// Längste Quellenangabe für „an Persona geben“ (Zeichen, eine Zeile).
pub const MAX_SOURCE_CHARS: usize = 300;
/// Hinweis, wenn „an Persona geben“ ohne Abschnitt kommt.
pub const PICK_SECTION: &str = "Abschnitt wählen: Die Persona antwortet im Abschnitts-Chat. Die Literatur aus einem \
     Abschnitt heraus öffnen (Arbeitsseite → Literatur) und den Text dann an die Persona geben.";

/// Hinweis bei einer Quelle ohne Verknüpfung zu MultiGPT (kein Stub: einen Volltext gibt es nur für
/// übernommene Dokumente).
pub const NOT_LINKED: &str = "Diese Quelle ist nicht mit MultiGPT verknüpft; einen Volltext gibt es nur für \
     Dokumente aus MultiGPT (Bedeutungssuche → „Volltext“ → „In die Bibliothek übernehmen“).";

/// Grund, wenn eine fremde Seite den Volltext per GET laden will (`Sec-Fetch-Site`/`Origin`).
pub const FOREIGN_FULLTEXT: &str = "Aufruf von einer fremden Seite; Baruch lädt den Volltext über MultiGPT nur, \
     wenn die Seite hier in Baruch geöffnet wird.";

/// Wortlaut-Suche (FTS5, Vorgabe) oder Bedeutungssuche (RAG) – `lmode` in Adresse und Formularen.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SearchMode {
    #[default]
    Fts,
    Rag,
}

impl SearchMode {
    /// `rag` → Bedeutung; alles andere (auch leer oder unbekannt) → Wortlaut.
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

// ---- Bilder des Werks (Runde 21c) ----------------------------------------------------------------

/// Höchstens so viele Bilder zeigt der Kasten „Bilder des Werks“.
pub const MAX_IMAGE_HITS: usize = 24;

/// Kasten „Bilder des Werks“ (Text automatisch maskiert, Adressen von [`href`]).
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

/// Bilder des Werks, in deren Name, Bildunterschrift oder Alternativtext alle Wörter von `query` vorkommen (ohne
/// Groß/Klein, auch Umlaute); Reihenfolge nach Namen. Leere Suche → keine. Ergebnis: höchstens `limit` Bilder und
/// ob es mehr gab.
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

/// HTML des Kastens „Bilder des Werks“ zur Suche von `at` (leer ohne Suche). Ein Fehler beim Lesen der Bilder
/// erscheint als Hinweis im Kasten, nie als stiller Wegfall.
async fn image_hits_html(state: &AppState, at: &At) -> String {
    image_hits_box(state, at.work, at.lq.as_deref()).await
}

/// Der Kasten „Bilder des Werks“ für die Suche `lq` im Werk `work` (leer ohne Suche). Runde 23a: auch für die
/// Arbeitsseite mit `?lq=` (`rpc::library_view`), damit der Kasten ohne JavaScript im ausgelieferten HTML steht.
pub(crate) async fn image_hits_box(state: &AppState, work: WorkId, lq: Option<&str>) -> String {
    let Some(query) = lq.map(str::trim).filter(|q| !q.is_empty()) else { return String::new() };
    let mut env = Environment::new();
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

/// Text für HTML maskieren (`&`, `<`, `>`, `"`, `'`).
fn escape_text(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#x27;")
}

// ---- Vorlagen ----------------------------------------------------------------------------------

/// Eigene Template-Umgebung der Literatur (die der Arbeitsseite bleibt unberührt).
fn templates() -> &'static Environment<'static> {
    static ENV: OnceLock<Environment<'static>> = OnceLock::new();
    ENV.get_or_init(|| {
        let mut env = Environment::new();
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

/// Thema aus den Cookies (`base.html` braucht `settings`); Spaltenbreiten gelten hier nicht.
fn page_settings(headers: &HeaderMap) -> Value {
    let settings = Settings::from_cookie_header(headers.get(header::COOKIE).and_then(|v| v.to_str().ok()));
    Value::from(Serde(settings))
}

/// Eine selbst gebaute Adresse für ein `href`: Sie besteht nur aus Ziffern, festen Namen und
/// prozentkodierten Werten ([`query_escape`]), darum genügt es, `&` zu maskieren. So bleibt `/`
/// lesbar (die Vorlagen maskieren es sonst als `&#x2f;`).
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

// ---- Fehler ------------------------------------------------------------------------------------

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

// ---- Stelle im Werk ----------------------------------------------------------------------------

/// Werk, Abschnitt, Revision und Suche, wie sie die Literatur-Seiten weitertragen.
#[derive(Debug, Clone, Default)]
struct At {
    work: WorkId,
    work_title: String,
    node: Option<usize>,
    /// Fester Schlüssel des Abschnitts (`Node::key`), gehört zu `node`.
    key: Option<i64>,
    rev: Option<u32>,
    preview: bool,
    lq: Option<String>,
    /// Art der Suche (`lmode`); reist mit `lq` mit.
    mode: SearchMode,
}

/// Zahl aus einem Formular- oder Adressfeld; leer oder ungültig → `None`.
fn number<T: std::str::FromStr>(value: &Option<String>) -> Option<T> {
    value.as_deref().map(str::trim).filter(|v| !v.is_empty()).and_then(|v| v.parse().ok())
}

/// Abschnitt aus Schlüssel und Stelle: Der Schlüssel geht vor (die Stelle kann veraltet sein); gibt
/// es ihn nicht (mehr), gilt kein Abschnitt. Ohne Schlüssel die Stelle, wenn es sie gibt, samt ihrem
/// Schlüssel.
fn place_of(work: &baruch_core::Work, node: Option<usize>, key: Option<i64>) -> (Option<usize>, Option<i64>) {
    let found = match key {
        Some(key) => work.nodes.iter().find(|n| n.key == key),
        None => node.and_then(|n| work.nodes.get(n)),
    };
    found.map_or((None, None), |n| (Some(n.id), Some(n.key)))
}

/// Steht das Feld (nicht `null`) in den Parametern? Für `node` in `highlight.cite`, das abgelehnt wird.
fn given(params: &serde_json::Value, name: &str) -> bool {
    params.get(name).is_some_and(|v| !v.is_null())
}

/// Fester Schlüssel aus RPC-Parametern: Zahl oder Ziffern als Text; fehlt → `None`.
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
    /// Werk prüfen (unbekannt → `UnknownWork`), Abschnitt nur übernehmen, wenn es ihn gibt
    /// ([`place_of`]: Schlüssel vor Stelle).
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

    /// `&lq=<kodiert>[&lmode=rag]` oder leer (die Vorgabe `fts` steht nicht in der Adresse).
    fn lq_tail(&self) -> String {
        let mut tail = self.lq.as_deref().map_or(String::new(), |q| format!("&lq={}", query_escape(q)));
        if self.mode == SearchMode::Rag {
            tail.push_str("&lmode=rag");
        }
        tail
    }

    /// `work=…[&key=…]`: Werk und fester Schlüssel des Abschnitts (Vertrag Runde 8, §1: erzeugte
    /// Adressen tragen kein `node=` mehr; `node` und `key` gehören immer zusammen, siehe [`place_of`]).
    fn place(&self) -> String {
        let mut q = format!("work={}", self.work);
        if let Some(key) = self.key {
            q.push_str(&format!("&key={key}"));
        }
        q
    }

    /// Zurück zum Werk: `/?work&key&rev[&view=preview][&lq]`.
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

    /// Dokumentansicht eines Titels an Teil `teil` des Volltexts (Anker `#volltext`).
    fn source_part_url(&self, title: i64, teil: usize) -> String {
        format!("/quelle?title={title}&{}&teil={teil}{}#volltext", self.place(), self.lq_tail())
    }

    /// Volltext eines Dokuments aus MultiGPT: `/quelle/multigpt?work&key[&rev]&collection&document[&section]
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

    /// Versteckte Felder für Formulare: work, node, key, rev, lq (nur die vorhandenen). Über `key`
    /// findet `cite` den Abschnitt.
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

    /// Kontext `work`, `node`, `rev`, `view` wie in der Arbeitsseite (für `fragments/library.html`).
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

// ---- Inhalt der Literatur ----------------------------------------------------------------------

/// Titel mit Zitaten und Hervorhebungen (flach serialisiert wie `TitleEntry`).
#[derive(Serialize)]
struct TitleView {
    #[serde(flatten)]
    entry: TitleEntry,
    highlights: Vec<Highlight>,
}

struct Literature {
    titles: Vec<TitleView>,
    hits: Vec<SearchHit>,
    /// Bedeutungssuche (`lmode=rag` mit Suche): [`rag_view`] bzw. [`rag_unavailable`].
    rag: Option<serde_json::Value>,
    /// Kasten „Bibeln“ und Treffergruppen „Bibel“/„Kommentar“ (Runde 15, [`bibles::Shelf`]).
    bibles: bibles::Shelf,
}

/// Alle Titel mit Hervorhebungen und, falls gesucht wurde, die Treffer im Werk und in der
/// Bibliothek. Eine Suche ohne Wörter (auch nur NUL oder Steuerzeichen) ergibt keine Treffer.
///
/// `own_page`: Die Anfrage kam aus Baruch selbst (RPC bzw. [`crate::guard::check`] bei GET). Sonst läuft die
/// Bedeutungssuche nicht – eine fremde Seite könnte sie sonst mit den Zugangsdaten der Wege auslösen
/// (Einbettung, pgvector, MultiGPT); die Seite sagt dann [`FOREIGN_PAGE`].
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

// ---- Bedeutungssuche (RAG) ---------------------------------------------------------------------

/// Kurzer Name eines Wegs in der Zustandszeile.
fn short_title(way: Way) -> &'static str {
    match way {
        Way::Lancedb => "LanceDB",
        Way::Pgvector => "pgvector",
        Way::Multigpt => "MultiGPT",
    }
}

/// Zustandszeile der Wege, z. B. „LanceDB: verbunden · pgvector: aus · MultiGPT: ausgefallen (…)“; nur
/// ausgefallene Wege nennen ihren Grund (der Grund für „aus“ steht in [`rag_unavailable`]).
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

/// Ist die Adresse ungefährlich für ein `href`? Nur eigene Adressen (`/…`, nicht `//…`) und http(s); kein
/// Leerraum, keine Steuerzeichen, keine Anführungszeichen oder spitzen Klammern, kein Backslash. Alles
/// andere (`javascript:`, `data:`, `//fremd`) wird verworfen.
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

/// Link eines Treffers: seine Adresse ([`safe_link`]) oder – bei LanceDB ohne Adresse – der Ort aus
/// `meta.work`/`meta.key` (`/?work=W&key=K`). Sonst kein Link.
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

/// Dokumentansicht eines MultiGPT-Treffers (Runde 13): `/quelle/multigpt?collection=…&document=…[&section=…]&q=…`
/// (alle Werte prozentkodiert); andere Wege → `None`.
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

/// Antwort der Bedeutungssuche für die Vorlage: Treffer (Titel, Weg-Abzeichen, Sammlung, Ausschnitt,
/// Ähnlichkeit in %, sicherer Link, bei MultiGPT seit Runde 13 `fulltext_link`), die Zustandszeile der Wege
/// und – wenn kein Weg geantwortet hat – der Grund. Treffer-Texte sind Daten; die Vorlage maskiert sie.
/// Runde 19c: je Treffer `weak` ([`baruch_rag::is_weak`]); schwache Treffer stehen hinter den übrigen (sonst in der
/// Reihenfolge des Rangs), `weak_count` zählt sie – die Vorlage zeigt sie zugeklappt in `<details>`.
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

/// Ausschnitt für die Anzeige: Typst-Überschriftszeilen am Anfang (`= Titel`, steht schon als Titel da) und
/// Leerzeilen fallen weg; der Text bleibt sonst unverändert (die Vorlage maskiert ihn).
fn display_snippet(snippet: &str) -> String {
    let lines: Vec<&str> = snippet.lines().map(str::trim_end).filter(|l| !l.trim().is_empty()).collect();
    let body: Vec<&str> = lines.iter().copied().skip_while(|l| l.trim_start().starts_with("= ")).collect();
    if body.is_empty() { lines.join("\n") } else { body.join("\n") }
}

/// Meldung eines Wegs ohne den Satz zur Wortlaut-Suche (den hängt [`rag_unavailable`] einmal an).
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

/// Grund, wenn eine fremde Seite die Bedeutungssuche per GET auslösen will (`Sec-Fetch-Site`/`Origin`).
pub const FOREIGN_PAGE: &str = "Aufruf von einer fremden Seite; Baruch sucht nach Bedeutung nur, wenn die Suche hier \
in Baruch abgeschickt wird";

/// Bedeutungssuche nicht verfügbar: „Bedeutungssuche nicht verfügbar: <Grund>. Die Wortlaut-Suche (FTS)
/// geht weiter.“ (mit Zustandszeile, wenn es eine Antwort gab).
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

// ---- Seiten ------------------------------------------------------------------------------------

/// Adressfelder der Seiten; alles als Text, damit ein leeres oder falsches Feld keinen 400 ergibt.
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
    /// Runde 13: Teil des Volltexts (ab 1), Dokument und Sammlung in MultiGPT, Fundstelle laut MultiGPT und
    /// die zu markierende Fundstelle `q`.
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

/// Hinweis aus `?notice=…`; nur feste Texte, nichts aus der Adresse wird gezeigt.
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

/// Routen der Seiten (lesend): `GET /literatur`, `GET /quelle`, `GET /quelle/multigpt`.
pub fn pages() -> Router<Arc<AppState>> {
    Router::new()
        .route("/literatur", get(literature_page))
        .route("/quelle", get(source_page))
        .route("/quelle/multigpt", get(multigpt_page))
}

/// Routen der Formulare (schreibend, im Router hinter `guard::same_origin`): `POST /literatur/act`.
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

/// Ein Stück Zitattext, ggf. hervorgehoben (Zeichenbereich einer Hervorhebung).
#[derive(Debug, PartialEq, Serialize)]
struct Segment {
    text: String,
    highlight: Option<i64>,
    kind: Option<HighlightKind>,
    target: bool,
}

/// Zerlegt `text` an den Zeichenbereichen der Hervorhebungen (Unicode-Zeichen ab 0). Bereiche,
/// die über das Ende hinausgehen, leer sind oder einen früheren überlappen, werden übergangen.
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
    // Verknüpfung mit MultiGPT oder lokaler Volltext aus einem PDF/EPUB-Import (Runde 21a).
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

/// Volltext-Teil für die Vorlagen: Stücke mit Fundstellen, Teil/Teile, Links auf den vorherigen und nächsten
/// Teil (`part_url`), Herkunft und Hinweis. Inhalte bleiben Text (die Vorlage maskiert).
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

/// Daten der Dokumentansicht eines Titels.
struct SourcePage<'a> {
    entry: &'a TitleEntry,
    highlights: &'a [Highlight],
    checks: &'a HashMap<i64, HighlightCheck>,
    /// Volltext-Bereich ([`fulltext_context`] bzw. Hinweis).
    fulltext: Value,
    /// Mit MultiGPT verknüpft: Das Formular bietet Anfang und Ende im Volltext an.
    linked: bool,
    /// Aktive Personas für „an Persona geben“ (Runde 15).
    personas: Vec<Value>,
}

/// Aktive Personas für „an Persona geben“; gewählt ist der Lektor des Werks.
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
                // Prüfzeit in Ortszeit (gespeichert UTC); RPC `highlight.*` liefert weiter den UTC-Stempel.
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

/// `GET /quelle/multigpt`: Volltext eines Dokuments aus MultiGPT (Runde 13).
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
    // Runde 18b: „an Persona geben“ auch hier – ohne übernommenen Titel, mit dem Text des gezeigten Teils (vorbelegt,
    // höchstens MAX_PERSONA_TEXT Zeichen) und der Quellenangabe „MultiGPT: Titel (Dokument …)“.
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

// ---- Formulare ---------------------------------------------------------------------------------

/// `POST /literatur/act`: alle Felder als Text (leere Felder sind „nicht angegeben“).
#[derive(Debug, Default, Deserialize)]
struct ActForm {
    action: String,
    work: Option<String>,
    node: Option<String>,
    /// Fester Schlüssel des Abschnitts; Pflicht bei `cite`.
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
    /// Pflicht-Kontrollkästchen beim Löschen.
    confirm: Option<String>,
    /// Runde 13 (`adopt`): Dokument und Sammlung in MultiGPT.
    document: Option<String>,
    collection: Option<String>,
    /// Runde 15 (`persona`): Persona, Quellenangabe und Frage.
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

/// Zeichenbereich `start..end` einer Hervorhebung (beide oder keiner).
type Span = (Option<i64>, Option<i64>);

/// Neue Hervorhebung des Menschen aus den Formular- bzw. RPC-Feldern.
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

/// Quellenangabe aus dem Formular bzw. aus dem Titel: eine Zeile, höchstens [`MAX_SOURCE_CHARS`] Zeichen.
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

/// `action=persona` (ohne JavaScript): an die Persona geben, dann zurück zum Abschnitt; ohne Abschnitt oder bei
/// einem Fehler eine Hinweisseite (Status 200) statt eines nackten Fehlers.
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

/// Auftrag „an Persona geben“ (Formular `action=persona`, RPC `literature.to_persona`, Bibelseite).
#[derive(Debug, Clone, Default)]
pub struct ToPersona {
    pub work: WorkId,
    pub key: i64,
    /// Handle; leer bzw. `None` = der Lektor des Werks.
    pub persona: Option<String>,
    /// Markierter Text (höchstens [`MAX_PERSONA_TEXT`] Zeichen).
    pub text: String,
    /// Quellenangabe (eine Zeile), steht als Quelle am Beitrag der Persona.
    pub source: String,
    pub title: Option<i64>,
    pub highlight: Option<i64>,
    pub question: Option<String>,
}

/// Markierten Text an eine Persona geben: Die Frage (mit Text und Quelle) steht als Beitrag des Menschen im
/// Abschnitt, die Antwort als Beitrag der Persona mit der Quelle. Nichts am Buchtext.
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

/// Persona-Hervorhebung als eigene übernehmen: neue Hervorhebung des Menschen (aktiv) mit Text, Bereich, Art und
/// Fundstelle des Originals; das Original bleibt Vorschlag. Eigene Hervorhebungen lassen sich nicht noch einmal
/// übernehmen, ein schon übernommener Vorschlag auch nicht.
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

/// Führt die Formular-Aktion aus und gibt die Adresse für die Weiterleitung zurück.
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
            // Ohne Zitat mit Bereich an einem verknüpften Titel: gegen den Volltext prüfen. Die Hervorhebung des
            // Menschen ist schon gespeichert; scheitert die Prüfung selbst, bleibt sie ungeprüft.
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
            // Ohne Abschnitt kein Zitat (früher landete es still in Abschnitt 0). Der Abschnitt kommt
            // nur aus dem festen Schlüssel (`key`), nicht aus der Stelle. Vor dem Anlegen einer
            // Hervorhebung prüfen, dass es ihn gibt, damit nichts halb entsteht.
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

/// „Ähnliches suchen“: die drei längsten Wörter des markierten Texts (die Suche verlangt alle).
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
    // In der Reihenfolge des Texts.
    chosen.sort_by_key(|w| text.find(w).unwrap_or(usize::MAX));
    chosen.join(" ")
}

// ---- Gemeinsame Abläufe von RPC und Formular ---------------------------------------------------

/// Status ändern: Eine Persona-Hervorhebung bleibt Vorschlag (oder wird abgelehnt), `active` gibt
/// es nur für Hervorhebungen des Menschen.
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

/// Zitat als Vorschlag-Revision: aktueller Text des Abschnitts + Leerzeile + `#quote(...)[...]`.
/// Läuft unter der Schreibsperre: Der feste Schlüssel `node_key` wird erst darunter in die Stelle
/// übersetzt (niemand stellt dazwischen um), und der Text wechselt nicht zwischen Lesen und Anhängen.
/// Die aktuelle Fassung bleibt, wie sie ist. Ergebnis: Stelle und Nummer des Vorschlags.
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

/// Gehört die Methode zur Literatur? (`literature.render`, `highlight.*`, seit Runde 13 `source.read` und
/// `source.adopt`)
pub fn handles(method: &str) -> bool {
    method == "literature.render"
        || method == "literature.to_persona"
        || method.starts_with("highlight.")
        || method == "source.read"
        || method == "source.adopt"
}

/// Meldung, wenn eine Hervorhebung nicht gegen den Volltext prüfbar ist.
pub const NOT_CHECKABLE: &str =
    "Gegen den Volltext prüfen lassen sich nur Hervorhebungen ohne Zitat mit Zeichenbereich (start, end).";

/// Hervorhebung als JSON mit ihrer Prüfung (`check`, sonst `null`).
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

/// `source.read` (RPC) und `source_read` (MCP): Teil des Volltexts eines verknüpften Titels oder Dokuments.
pub async fn source_read(state: &AppState, params_value: serde_json::Value) -> Result<serde_json::Value, RpcError> {
    let p: SourceReadParams = params(params_value)?;
    let target = match (p.title, p.document) {
        (Some(title), _) => Target::Title(title),
        (None, Some(document)) => Target::Document { document, collection: p.collection.unwrap_or_default() },
        (None, None) => return Err(invalid("title (verknüpfter Titel) oder document (Dokument in MultiGPT) angeben")),
    };
    let q = p.q.as_deref().map(str::trim).filter(|q| !q.is_empty());
    // Runde 23a (Codereview ki-1): `q` dient nur dem Markieren; eine überlange Anfrage wird abgelehnt, statt den
    // Teiltext für jedes ihrer Wörter zu durchsuchen (`/mcp` nimmt bis 32 MiB an).
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

/// RPC-Methoden der Literatur; `work` ist schon aufgelöst (`rpc::work_param`).
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
            // `POST /rpc` liegt hinter `guard::same_origin`.
            let lit = literature(state, &at, true).await?;
            let mut html = render("fragments/library.html", literature_context(&at, &lit))?;
            // Runde 21c: Bilder des Werks zur Suche unter den Treffern (auch im Popup).
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
