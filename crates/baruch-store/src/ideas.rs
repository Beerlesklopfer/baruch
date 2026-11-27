// SPDX-License-Identifier: AGPL-3.0-or-later
//! Einfälle (Vertrag Runde 7, Abschnitt 6; Migration `0011_ideas.sql`): Schlagwörter und Gedanken,
//! die beim Schreiben, Lesen oder Beten einfallen – je Werk oder ohne Werk, mit Herkunft (Mensch,
//! Persona, MCP-Client), offen oder verarbeitet.
//!
//! Regeln:
//! - Vollständiges CRUD auch für Assistenten und MCP (Entscheidung IDEA_CRUD): Einfälle ändern keinen
//!   Buchtext. Jede Aktion protokolliert der Server mit Herkunft im Git-Spiegel.
//! - Jeder Schreibvorgang ist genau eine Transaktion (`BEGIN IMMEDIATE`); Tabelle, Schlagwörter und
//!   Volltextindex `idea_fts` (rowid = `idea.id`) ändern sich darin gemeinsam.
//! - Löschen setzt nur `deleted` (wiederherstellbar über [`Store::idea_restore`]); gelöschte Einfälle
//!   stehen weder in Listen noch im Index, [`Store::idea_get`] findet sie weiter.
//! - Grenzen: 1 bis [`MAX_IDEA_WORDS`] Schlagwörter, je höchstens [`MAX_WORD_CHARS`] Zeichen und ohne
//!   Zeilenumbruch, Doppel (gleicher `word_key`: klein, Leerraum zusammengefasst) fallen zusammen;
//!   Gedanke und Markierung höchstens 4000, Quelle und Bibelstelle höchstens 200 Zeichen. Sonst
//!   [`StoreError::Idea`], nichts geändert.
//! - Der Bezug (`key` = `Node::key`) muss zum Werk des Einfalls gehören. Ein Werk zu löschen macht
//!   seine Einfälle zu Einfällen ohne Werk (`ON DELETE SET NULL`), ein gelöschter Abschnitt nimmt nur
//!   den Bezug mit.
//! - Spiegel: `einfaelle.md` im Werkspiegel ([`mirror_files`], nur bei mindestens einem nicht
//!   gelöschten Einfall), Einfälle ohne Werk über [`Store::idea_mirror_file`] mit `None`. Seit Runde 25 je
//!   Einfall mit Verbindungen die Zeile „- Verbindungen: → 79 führt weiter; ↔ 48 widerspricht („Notiz“)“
//!   (`idea_links::mirror_lines`).
//!
//! Runde 34 (Mindmap, Migration `0052_idea_tree.sql`): Einfälle bilden einen Baum.
//! - `title` (höchstens [`MAX_IDEA_TITLE_CHARS`] Zeichen, darf leer sein): Ein Einfall braucht Titel **oder**
//!   mindestens ein Schlagwort. Der Titel steht im Volltextindex in der Spalte `words` mit.
//! - `parent` (Elternknoten): ein nicht gelöschter Einfall desselben Werks, kein Kreis, Kette höchstens
//!   [`MAX_IDEA_DEPTH`] Einfälle tief. Einfälle ohne Werk haben keinen Elternknoten.
//! - `position`: Reihenfolge unter Geschwistern (gleicher Elternknoten bzw. alle Einfälle des Werks ohne
//!   Elternknoten), dann nach `id`. Neue kommen ans Ende (solange noch nichts geordnet wurde, behalten alle die
//!   Vorgabe 0 und stehen nach `id`); „vor ein Geschwister“ nummeriert die Geschwister lückenlos ab 1 neu (deren
//!   `updated` bleibt, die Reihenfolge ist keine Änderung ihres Inhalts).
//! - Platz (eine Regel, überall gleich): mit `parent` Unterknoten; sonst mit `key` am Kapitel des Bezugs; sonst
//!   mit `main_topic` freies Hauptthema am Werk; sonst **in der Wolke** (Ablage neben der Mindmap, Nachtrag des
//!   Nutzers vom 2026-11-25). `main_topic` ist nur ohne Elternknoten und Bezug `true`.
//! - Löschen nimmt die nicht gelöschten Nachfahren mit demselben Zeitstempel mit ([`Store::idea_delete_tree`]),
//!   Wiederherstellen genau die mit diesem Zeitstempel ([`Store::idea_restore_tree`]); ist der Elternknoten dann
//!   noch gelöscht, wird der Einfall gelöst (bleibt am Bezug bzw. liegt in der Wolke).
//! - Wechselt ein Einfall das Werk ([`Store::idea_update`]), löst er sich vom Elternknoten, und seine Unterknoten
//!   bleiben im alten Werk (sie verlieren den Elternknoten).

use sqlx::SqliteConnection;

use crate::{MirrorFile, Store, StoreError, WorkId, broken};

pub type IdeaId = i64;

/// Höchstzahl der Schlagwörter eines Einfalls.
pub const MAX_IDEA_WORDS: usize = 20;
/// Höchstlänge eines Schlagworts in Zeichen.
pub const MAX_WORD_CHARS: usize = 60;
/// Höchstlänge des Titels eines Einfalls in Zeichen (Runde 34).
pub const MAX_IDEA_TITLE_CHARS: usize = 200;
/// Höchste Tiefe einer Kette von Einfällen (Einfall ohne Elternknoten = 1; Runde 34).
pub const MAX_IDEA_DEPTH: usize = 32;
/// Höchstzahl der Knoten eines Baums in [`Store::idea_import_tree`] (Runde 34).
pub const MAX_IDEA_IMPORT_NODES: usize = 2000;
/// Höchstlänge von Gedanke und Markierung in Zeichen.
const MAX_TEXT_CHARS: usize = 4000;
/// Höchstlänge von Quelle und Bibelstelle in Zeichen.
pub(crate) const MAX_SHORT_CHARS: usize = 200;
/// Vorgabe und Obergrenze für `IdeaFilter::limit`.
const DEFAULT_LIMIT: u32 = 100;
const MAX_LIMIT: u32 = 500;

/// Name der Datei im Spiegel.
pub(crate) const MIRROR_FILE: &str = "einfaelle.md";

/// Wer den Einfall festgehalten hat.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum IdeaOrigin {
    /// Der Mensch (Name, im Server `mirror::HUMAN_NAME`).
    Human { name: String },
    /// Eine Persona (`@handle`, es muss sie geben – prüft der Aufrufer).
    Persona { handle: String },
    /// Ein MCP-Client als `@mcp:<name>`.
    Mcp { handle: String },
}

impl IdeaOrigin {
    fn columns(&self) -> (&'static str, &str) {
        match self {
            IdeaOrigin::Human { name } => ("human", name),
            IdeaOrigin::Persona { handle } => ("persona", handle),
            IdeaOrigin::Mcp { handle } => ("mcp", handle),
        }
    }

    fn from_columns(kind: &str, name: String) -> Result<IdeaOrigin, StoreError> {
        match kind {
            "human" => Ok(IdeaOrigin::Human { name }),
            "persona" => Ok(IdeaOrigin::Persona { handle: name }),
            "mcp" => Ok(IdeaOrigin::Mcp { handle: name }),
            _ => Err(broken(format!("unbekannte Herkunft eines Einfalls {kind:?}"))),
        }
    }

    /// Wert des Trailers `Baruch-Origin`: `human`, `persona` oder `mcp`.
    pub fn kind(&self) -> &'static str {
        self.columns().0
    }

    /// Name bzw. Handle.
    pub fn name(&self) -> &str {
        self.columns().1
    }

    /// Anzeige: „Mensch Autor“, „Persona @lektor“, „MCP @mcp:claude-code“.
    pub fn label(&self) -> String {
        match self {
            IdeaOrigin::Human { name } => format!("Mensch {name}"),
            IdeaOrigin::Persona { handle } => format!("Persona {handle}"),
            IdeaOrigin::Mcp { handle } => format!("MCP {handle}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdeaStatus {
    Open,
    Done,
}

impl IdeaStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            IdeaStatus::Open => "open",
            IdeaStatus::Done => "done",
        }
    }

    pub fn parse(s: &str) -> Option<IdeaStatus> {
        match s {
            "open" => Some(IdeaStatus::Open),
            "done" => Some(IdeaStatus::Done),
            _ => None,
        }
    }
}

/// Wozu ein Einfall verarbeitet wurde.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdeaUse {
    /// Neuer Gliederungspunkt.
    Outline,
    /// Beitrag im Abschnitts-Chat.
    Post,
    /// Auftrag an eine Persona.
    Persona,
    /// Notiz an einem Abschnitt (oder von Hand als erledigt markiert).
    Note,
}

impl IdeaUse {
    pub fn as_str(self) -> &'static str {
        match self {
            IdeaUse::Outline => "outline",
            IdeaUse::Post => "post",
            IdeaUse::Persona => "persona",
            IdeaUse::Note => "note",
        }
    }

    pub fn parse(s: &str) -> Option<IdeaUse> {
        match s {
            "outline" => Some(IdeaUse::Outline),
            "post" => Some(IdeaUse::Post),
            "persona" => Some(IdeaUse::Persona),
            "note" => Some(IdeaUse::Note),
            _ => None,
        }
    }

    /// Deutsches Wort für Oberfläche und Spiegel-Betreff.
    pub fn label(self) -> &'static str {
        match self {
            IdeaUse::Outline => "Gliederungspunkt",
            IdeaUse::Post => "Chat-Beitrag",
            IdeaUse::Persona => "Persona-Auftrag",
            IdeaUse::Note => "Notiz",
        }
    }
}

/// Welche Einfälle eine Liste zeigt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IdeaScope {
    /// Alle, mit und ohne Werk.
    #[default]
    All,
    /// Nur Einfälle ohne Werk.
    NoWork,
    /// Nur die eines Werks.
    Work(WorkId),
}

/// Neuer Einfall ([`Store::idea_create`]).
#[derive(Debug, Clone)]
pub struct NewIdea<'a> {
    pub work: Option<WorkId>,
    /// Bezug: fester Schlüssel eines Abschnitts des Werks.
    pub key: Option<i64>,
    pub words: &'a [String],
    pub note: &'a str,
    pub quote: &'a str,
    pub source: &'a str,
    pub bible: &'a str,
    pub origin: IdeaOrigin,
}

/// Ein Einfall, wie ihn Listen, RPC und MCP zeigen.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Idea {
    pub id: IdeaId,
    pub work: Option<WorkId>,
    pub key: Option<i64>,
    /// Titel des Abschnitts im Bezug (jetzt), falls es ihn gibt.
    pub node_title: Option<String>,
    pub words: Vec<String>,
    pub note: String,
    pub quote: String,
    pub source: String,
    pub bible: String,
    pub origin: IdeaOrigin,
    pub status: IdeaStatus,
    pub done_as: Option<IdeaUse>,
    pub done_ref: Option<String>,
    pub created: String,
    pub updated: String,
    /// Zeitpunkt des Löschens; `None` = vorhanden.
    pub deleted: Option<String>,
    /// Titel des Knotens in der Mindmap (Runde 34); darf leer sein.
    pub title: String,
    /// Elternknoten (ein Einfall desselben Werks).
    pub parent: Option<IdeaId>,
    /// Reihenfolge unter Geschwistern (dann nach `id`).
    pub position: i64,
    /// Freies Hauptthema am Werk; nur ohne `parent` und `key` `true`. Ohne alle drei liegt der Einfall in der Wolke.
    pub main_topic: bool,
    /// Abgeleitet: liegt in der Wolke (Einfall eines Werks ohne Elternknoten, Bezug und Hauptthema).
    pub cloud: bool,
}

/// Platz eines neuen Einfalls in der Mindmap ([`Store::idea_create_at`]). Vorgabe: ohne Titel, ohne Elternknoten,
/// ans Ende, nicht als Hauptthema (ohne Bezug also in der Wolke).
#[derive(Debug, Clone, Copy, Default)]
pub struct IdeaSpot<'a> {
    pub title: &'a str,
    /// Elternknoten; das Werk des neuen Einfalls muss das des Elternknotens sein.
    pub parent: Option<IdeaId>,
    /// Geschwister, vor das der neue Einfall kommt (`None` = ans Ende).
    pub before: Option<IdeaId>,
    /// Freies Hauptthema am Werk (nur ohne `parent` und Bezug erlaubt).
    pub main_topic: bool,
}

/// Umhängen und Ordnen ([`Store::idea_move`]); `None` = bleibt.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IdeaMove {
    /// `Some(None)` löst vom Elternknoten; bleibt danach auch kein Bezug, wird der Einfall freies Hauptthema am
    /// Werk (außer `cloud` sagt etwas anderes).
    pub parent: Option<Option<IdeaId>>,
    /// Bezug setzen bzw. löschen.
    pub key: Option<Option<i64>>,
    /// `Some(Some(id))` vor dieses Geschwister, `Some(None)` ans Ende. Ohne Angabe: ans Ende, wenn sich der Platz
    /// ändert, sonst bleibt die Reihenfolge.
    pub before: Option<Option<IdeaId>>,
    /// `Some(true)`: in die Wolke (löst Elternknoten und Bezug; die Unterknoten bleiben am Einfall).
    /// `Some(false)`: ohne Elternknoten und Bezug freies Hauptthema am Werk.
    pub cloud: Option<bool>,
}

/// Ergebnis von [`Store::idea_delete_tree`] und [`Store::idea_restore_tree`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdeaTreeChange {
    pub idea: Idea,
    /// Alle betroffenen Einfälle: der Einfall selbst zuerst, dann die Nachfahren nach id.
    pub ids: Vec<IdeaId>,
    /// Nur beim Wiederherstellen: Der Elternknoten ist noch gelöscht, der Einfall wurde gelöst.
    pub detached: bool,
}

/// Knoten eines Baums, der als Einfälle angelegt wird ([`Store::idea_import_tree`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IdeaSeed {
    pub title: String,
    pub note: String,
    pub words: Vec<String>,
    pub children: Vec<IdeaSeed>,
}

/// Verbindung zwischen zwei Knoten des Baums; Pfad = Indizes von der obersten Liste abwärts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IdeaSeedLink {
    pub from: Vec<usize>,
    pub to: Vec<usize>,
    pub note: String,
}

/// Änderung eines Einfalls ([`Store::idea_update`]); `None` = bleibt. `work: Some(None)` = ohne Werk,
/// `key: Some(None)` = ohne Bezug. Wechselt das Werk und ist `key` nicht angegeben, entfällt der
/// Bezug (er gehörte zum alten Werk). `status: Some(Open)` setzt `done_as`/`done_ref` zurück,
/// `Some(Done)` bei einem offenen Einfall markiert ihn als von Hand erledigt (`done_as` = `note`).
#[derive(Debug, Clone, Default)]
pub struct IdeaEdit<'a> {
    pub words: Option<&'a [String]>,
    pub note: Option<&'a str>,
    pub quote: Option<&'a str>,
    pub source: Option<&'a str>,
    pub bible: Option<&'a str>,
    pub work: Option<Option<WorkId>>,
    pub key: Option<Option<i64>>,
    pub status: Option<IdeaStatus>,
    /// Titel des Knotens (Runde 34); leer erlaubt, solange ein Schlagwort bleibt.
    pub title: Option<&'a str>,
}

/// Filter für Liste, Schlagwort-Wolke und Suche.
#[derive(Debug, Clone)]
pub struct IdeaFilter {
    pub scope: IdeaScope,
    /// Nur Einfälle mit diesem Schlagwort (verglichen über den `word_key`).
    pub word: Option<String>,
    /// `None` = offen und verarbeitet.
    pub status: Option<IdeaStatus>,
    /// `true`: nur gelöschte (zum Wiederherstellen), sonst nur vorhandene.
    pub deleted: bool,
    /// Höchstzahl (0 = Vorgabe 100, höchstens 500).
    pub limit: u32,
    pub offset: u32,
}

impl Default for IdeaFilter {
    fn default() -> Self {
        IdeaFilter { scope: IdeaScope::All, word: None, status: None, deleted: false, limit: DEFAULT_LIMIT, offset: 0 }
    }
}

impl IdeaFilter {
    fn limit(&self) -> i64 {
        i64::from(if self.limit == 0 { DEFAULT_LIMIT } else { self.limit.min(MAX_LIMIT) })
    }
}

/// Ein Schlagwort der Wolke mit der Zahl der Einfälle, die es tragen.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct WordCount {
    pub word: String,
    pub count: u32,
}

// ---- Prüfen ------------------------------------------------------------------------------------

fn refuse(message: impl Into<String>) -> StoreError {
    StoreError::Idea(message.into())
}

/// Vergleichsschlüssel eines Schlagworts: klein geschrieben, Leerraum zu einem Leerzeichen.
pub(crate) fn word_key(word: &str) -> String {
    word.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// Schlagwörter prüfen: Ränder getrimmt, Leerraum im Wort zusammengefasst, leere übergangen, Doppel
/// (gleicher Schlüssel) zusammengefasst (das erste bleibt). Ergebnis `(Wort, Schlüssel)`.
/// Seit Runde 34 darf die Liste leer sein (Titel statt Schlagwort; das prüft der Aufrufer).
fn clean_words_or_none(words: &[String]) -> Result<Vec<(String, String)>, StoreError> {
    let mut out: Vec<(String, String)> = Vec::new();
    for raw in words {
        if raw.chars().any(baruch_typst::is_newline) {
            return Err(refuse("Ein Schlagwort darf keinen Zeilenumbruch enthalten."));
        }
        let word = raw.split_whitespace().collect::<Vec<_>>().join(" ");
        if word.is_empty() {
            continue;
        }
        let chars = word.chars().count();
        if chars > MAX_WORD_CHARS {
            return Err(refuse(format!(
                "Das Schlagwort „{}…“ ist zu lang (höchstens {MAX_WORD_CHARS} Zeichen, hier {chars}).",
                word.chars().take(20).collect::<String>()
            )));
        }
        let key = word_key(&word);
        if !out.iter().any(|(_, k)| *k == key) {
            out.push((word, key));
        }
    }
    if out.len() > MAX_IDEA_WORDS {
        return Err(refuse(format!("Zu viele Schlagwörter (höchstens {MAX_IDEA_WORDS}, hier {}).", out.len())));
    }
    Ok(out)
}

/// Freitext prüfen: CRLF → LF, Ränder getrimmt, höchstens `max` Zeichen.
fn clean_text(value: &str, max: usize, what: &str) -> Result<String, StoreError> {
    let text = value.replace("\r\n", "\n").trim().to_owned();
    let chars = text.chars().count();
    if chars > max {
        return Err(refuse(format!("{what} ist zu lang (höchstens {max} Zeichen, hier {chars}).")));
    }
    Ok(text)
}

/// Titel prüfen: eine Zeile (Zeilenumbrüche und Leerraum zu einem Leerzeichen), Ränder getrimmt, höchstens
/// [`MAX_IDEA_TITLE_CHARS`] Zeichen.
fn clean_title(value: &str) -> Result<String, StoreError> {
    let title = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let chars = title.chars().count();
    if chars > MAX_IDEA_TITLE_CHARS {
        return Err(refuse(format!("Der Titel ist zu lang (höchstens {MAX_IDEA_TITLE_CHARS} Zeichen, hier {chars}).")));
    }
    Ok(title)
}

/// Gibt es das Werk? Sonst `UnknownWork`.
async fn check_work(conn: &mut SqliteConnection, work: WorkId) -> Result<(), StoreError> {
    let found: Option<i64> =
        sqlx::query_scalar("SELECT id FROM work WHERE id = ?").bind(work).fetch_optional(&mut *conn).await?;
    found.map(|_| ()).ok_or(StoreError::UnknownWork(work))
}

/// Gehört der Abschnitt `key` zum Werk? Ohne Werk ist kein Bezug möglich.
async fn check_key(conn: &mut SqliteConnection, work: Option<WorkId>, key: Option<i64>) -> Result<(), StoreError> {
    let Some(key) = key else { return Ok(()) };
    let Some(work) = work else {
        return Err(refuse("Ein Abschnitt als Bezug geht nur bei einem Einfall mit Werk."));
    };
    let found: Option<i64> = sqlx::query_scalar("SELECT id FROM node WHERE id = ? AND work_id = ?")
        .bind(key)
        .bind(work)
        .fetch_optional(&mut *conn)
        .await?;
    found.map(|_| ()).ok_or_else(|| refuse("Der Abschnitt gehört nicht zu diesem Werk."))
}

// ---- Lesen -------------------------------------------------------------------------------------

#[derive(sqlx::FromRow)]
struct Row {
    id: i64,
    work_id: Option<i64>,
    node_id: Option<i64>,
    node_title: Option<String>,
    note: String,
    quote: String,
    source: String,
    bible: String,
    origin_kind: String,
    origin_name: String,
    status: String,
    done_as: Option<String>,
    done_ref: Option<String>,
    created: String,
    updated: String,
    deleted: Option<String>,
    title: String,
    parent_id: Option<i64>,
    position: i64,
    main_topic: i64,
}

const SELECT: &str = "SELECT i.id, i.work_id, i.node_id, n.title AS node_title, i.note, i.quote, i.source, i.bible, \
     i.origin_kind, i.origin_name, i.status, i.done_as, i.done_ref, i.created, i.updated, i.deleted, i.title, \
     i.parent_id, i.position, i.main_topic FROM idea i LEFT JOIN node n ON n.id = i.node_id";

async fn words_of(conn: &mut SqliteConnection, id: IdeaId) -> Result<Vec<String>, StoreError> {
    Ok(sqlx::query_scalar("SELECT word FROM idea_word WHERE idea_id = ? ORDER BY position")
        .bind(id)
        .fetch_all(&mut *conn)
        .await?)
}

async fn to_idea(conn: &mut SqliteConnection, row: Row) -> Result<Idea, StoreError> {
    let status = IdeaStatus::parse(&row.status)
        .ok_or_else(|| broken(format!("unbekannter Status eines Einfalls {:?}", row.status)))?;
    let done_as = row
        .done_as
        .as_deref()
        .map(|s| IdeaUse::parse(s).ok_or_else(|| broken(format!("unbekannte Verarbeitung {s:?}"))))
        .transpose()?;
    Ok(Idea {
        id: row.id,
        work: row.work_id,
        key: row.node_id,
        node_title: row.node_title,
        words: words_of(conn, row.id).await?,
        note: row.note,
        quote: row.quote,
        source: row.source,
        bible: row.bible,
        origin: IdeaOrigin::from_columns(&row.origin_kind, row.origin_name)?,
        status,
        done_as,
        done_ref: row.done_ref,
        created: row.created,
        updated: row.updated,
        deleted: row.deleted,
        title: row.title,
        parent: row.parent_id,
        position: row.position,
        main_topic: row.main_topic != 0,
        cloud: row.work_id.is_some() && row.parent_id.is_none() && row.node_id.is_none() && row.main_topic == 0,
    })
}

pub(crate) async fn get(conn: &mut SqliteConnection, id: IdeaId) -> Result<Idea, StoreError> {
    let row: Option<Row> = sqlx::query_as(sqlx::AssertSqlSafe(format!("{SELECT} WHERE i.id = ?")))
        .bind(id)
        .fetch_optional(&mut *conn)
        .await?;
    let row = row.ok_or(StoreError::UnknownIdea(id))?;
    to_idea(conn, row).await
}

/// Bedingungen des Filters als SQL (über `i`) und die Werte dazu.
enum Bind {
    Int(i64),
    Text(String),
}

fn conditions(filter: &IdeaFilter, with_word: bool) -> (String, Vec<Bind>) {
    let mut parts = vec![if filter.deleted { "i.deleted IS NOT NULL" } else { "i.deleted IS NULL" }.to_owned()];
    let mut binds = Vec::new();
    match filter.scope {
        IdeaScope::All => {}
        IdeaScope::NoWork => parts.push("i.work_id IS NULL".to_owned()),
        IdeaScope::Work(work) => {
            parts.push("i.work_id = ?".to_owned());
            binds.push(Bind::Int(work));
        }
    }
    if let Some(status) = filter.status {
        parts.push("i.status = ?".to_owned());
        binds.push(Bind::Text(status.as_str().to_owned()));
    }
    if with_word && let Some(word) = filter.word.as_deref().map(word_key).filter(|w| !w.is_empty()) {
        parts.push("EXISTS (SELECT 1 FROM idea_word w WHERE w.idea_id = i.id AND w.word_key = ?)".to_owned());
        binds.push(Bind::Text(word));
    }
    (parts.join(" AND "), binds)
}

macro_rules! bind_all {
    ($query:expr, $binds:expr) => {{
        let mut query = $query;
        for bind in $binds {
            query = match bind {
                Bind::Int(v) => query.bind(*v),
                Bind::Text(v) => query.bind(v.clone()),
            };
        }
        query
    }};
}

// ---- Schreiben ---------------------------------------------------------------------------------

/// Index-Zeile des Einfalls neu schreiben (vorher entfernen); gelöschte kommen nicht hinein.
async fn reindex(conn: &mut SqliteConnection, id: IdeaId) -> Result<(), StoreError> {
    sqlx::query("DELETE FROM idea_fts WHERE rowid = ?").bind(id).execute(&mut *conn).await?;
    let row: Option<(String, String, String, String, Option<String>, String)> =
        sqlx::query_as("SELECT note, quote, source, bible, deleted, title FROM idea WHERE id = ?")
            .bind(id)
            .fetch_optional(&mut *conn)
            .await?;
    let Some((note, quote, source, bible, deleted, title)) = row else { return Ok(()) };
    if deleted.is_some() {
        return Ok(());
    }
    // Runde 34: Der Titel steht in der Spalte `words` mit (kein Umbau der FTS-Tabelle).
    let mut words = words_of(conn, id).await?.join(" ");
    if !title.is_empty() {
        words = if words.is_empty() { title } else { format!("{title} {words}") };
    }
    sqlx::query("INSERT INTO idea_fts (rowid, words, note, quote, source, bible) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(id)
        .bind(words)
        .bind(note)
        .bind(quote)
        .bind(source)
        .bind(bible)
        .execute(&mut *conn)
        .await?;
    Ok(())
}

async fn write_words(conn: &mut SqliteConnection, id: IdeaId, words: &[(String, String)]) -> Result<(), StoreError> {
    sqlx::query("DELETE FROM idea_word WHERE idea_id = ?").bind(id).execute(&mut *conn).await?;
    for (position, (word, key)) in words.iter().enumerate() {
        sqlx::query("INSERT INTO idea_word (idea_id, position, word, word_key) VALUES (?, ?, ?, ?)")
            .bind(id)
            .bind(position as i64)
            .bind(word)
            .bind(key)
            .execute(&mut *conn)
            .await?;
    }
    Ok(())
}

const NOW: &str = "strftime('%Y-%m-%dT%H:%M:%fZ','now')";

/// Einfall als verarbeitet markieren, in der Transaktion des Aufrufers (auch für
/// `Store::idea_to_outline`, Runde 14): `status = done`, `done_as`, `done_ref`. Gelöscht → `Idea`.
pub(crate) async fn mark_done_in(
    conn: &mut SqliteConnection,
    id: IdeaId,
    used: IdeaUse,
    done_ref: &str,
) -> Result<Idea, StoreError> {
    let before = get(conn, id).await?;
    not_deleted(&before)?;
    let done_ref = done_ref.trim();
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "UPDATE idea SET status = 'done', done_as = ?, done_ref = ?, updated = {NOW} WHERE id = ?"
    )))
    .bind(used.as_str())
    .bind((!done_ref.is_empty()).then_some(done_ref))
    .bind(id)
    .execute(&mut *conn)
    .await?;
    get(conn, id).await
}

/// Gelöschte Einfälle lassen sich nicht ändern oder verarbeiten.
fn not_deleted(idea: &Idea) -> Result<(), StoreError> {
    if idea.deleted.is_some() {
        return Err(refuse(format!("Einfall {} ist gelöscht; erst wiederherstellen.", idea.id)));
    }
    Ok(())
}

// ---- Spiegel -----------------------------------------------------------------------------------

/// Einen Wert in die Zeile `- <label>: <wert>` schreiben; weitere Zeilen zwei Leerzeichen eingerückt.
/// Der Wert steht wörtlich da (keine Umwandlung, keine Maskierung), damit er sich unverändert
/// wiederherstellen lässt.
fn push_value(out: &mut String, label: &str, value: &str) {
    if value.is_empty() {
        return;
    }
    out.push_str("- ");
    out.push_str(label);
    out.push_str(": ");
    let mut lines = value.split('\n');
    out.push_str(lines.next().unwrap_or_default());
    out.push('\n');
    for line in lines {
        if !line.is_empty() {
            out.push_str("  ");
            out.push_str(line);
        }
        out.push('\n');
    }
}

impl Idea {
    /// Der Eintrag dieses Einfalls in `einfaelle.md` (auch der Rumpf des Lösch-Commits): Überschrift
    /// „## Einfall <id> · offen|verarbeitet (<done_as>)“, dann je vorhandenem Feld eine Zeile.
    pub fn entry_markdown(&self) -> String {
        let status = match (self.status, self.done_as) {
            (IdeaStatus::Done, Some(used)) => format!("verarbeitet ({})", used.as_str()),
            (IdeaStatus::Done, None) => "verarbeitet".to_owned(),
            (IdeaStatus::Open, _) => "offen".to_owned(),
        };
        let mut out = format!("## Einfall {} · {status}\n\n", self.id);
        push_value(&mut out, "Titel", &self.title);
        push_value(&mut out, "Schlagwörter", &self.words.join(", "));
        push_value(&mut out, "Gedanke", &self.note);
        push_value(&mut out, "Markiert", &self.quote);
        if let Some(key) = self.key {
            let title = self.node_title.as_deref().unwrap_or("");
            push_value(&mut out, "Bezug", &format!("Abschnitt „{title}“ (key {key})"));
        }
        push_value(&mut out, "Quelle", &self.source);
        push_value(&mut out, "Bibelstelle", &self.bible);
        if let Some(parent) = self.parent {
            push_value(&mut out, "Unterknoten von", &format!("Einfall {parent}"));
        } else if self.main_topic {
            push_value(&mut out, "Platz", "Hauptthema am Werk");
        }
        push_value(&mut out, "Herkunft", &format!("{} · {}", self.origin.label(), self.created));
        out
    }

    /// Angezeigter Titel, nie leer (Runde 34): der Titel, sonst die Schlagwörter „a, b“, sonst der Anfang des
    /// Gedankens, sonst „Einfall <id>“; höchstens 60 Zeichen aus Schlagwörtern bzw. Gedanke (gekürzt mit „…“).
    pub fn display_title(&self) -> String {
        if !self.title.trim().is_empty() {
            return self.title.trim().to_owned();
        }
        let joined = self.words.join(", ");
        let source =
            if joined.trim().is_empty() { self.note.split_whitespace().collect::<Vec<_>>().join(" ") } else { joined };
        if source.is_empty() {
            return format!("Einfall {}", self.id);
        }
        if source.chars().count() <= 60 {
            source
        } else {
            let mut cut: String = source.chars().take(59).collect();
            cut.push('…');
            cut
        }
    }

    /// Trägt der Einfall eine Notiz (Gedanke, Markierung, Quelle oder Bibelstelle)?
    pub fn has_note(&self) -> bool {
        [&self.note, &self.quote, &self.source, &self.bible].iter().any(|t| !t.trim().is_empty())
    }
}

/// Abschnitt „## Baum“ für `einfaelle.md` (Runde 34): die Einfälle eingerückt nach Elternknoten, Geschwister nach
/// `position`, dann `id`. Nur wenn mindestens ein Einfall einen Titel, einen Elternknoten oder den Platz
/// „Hauptthema“ hat – sonst leer, damit Spiegel ohne Baum byte-gleich bleiben.
fn tree_markdown(ideas: &[Idea]) -> String {
    if !ideas.iter().any(|i| !i.title.is_empty() || i.parent.is_some() || i.main_topic) {
        return String::new();
    }
    let known: std::collections::HashSet<IdeaId> = ideas.iter().map(|i| i.id).collect();
    let mut below: std::collections::BTreeMap<Option<IdeaId>, Vec<&Idea>> = std::collections::BTreeMap::new();
    for idea in ideas {
        // Ein Elternknoten, der nicht (mehr) in der Liste steht, zählt wie keiner.
        below.entry(idea.parent.filter(|p| known.contains(p))).or_default().push(idea);
    }
    for list in below.values_mut() {
        list.sort_by_key(|i| (i.position, i.id));
    }
    fn walk(
        out: &mut String,
        below: &std::collections::BTreeMap<Option<IdeaId>, Vec<&Idea>>,
        parent: Option<IdeaId>,
        depth: usize,
    ) {
        // Tiefe begrenzt (der Store erlaubt höchstens MAX_IDEA_DEPTH; ein Kreis in kaputten Daten endete sonst nie).
        if depth > MAX_IDEA_DEPTH {
            return;
        }
        for idea in below.get(&parent).map(Vec::as_slice).unwrap_or_default() {
            let place = match (depth, idea.key, idea.main_topic) {
                (0, Some(key), _) => format!(" (am Abschnitt key {key})"),
                (0, None, true) => " (Hauptthema)".to_owned(),
                (0, None, false) if idea.work.is_some() => " (Wolke)".to_owned(),
                _ => String::new(),
            };
            out.push_str(&"  ".repeat(depth));
            out.push_str(&format!("- Einfall {}: {}{place}\n", idea.id, idea.display_title()));
            walk(out, below, Some(idea.id), depth + 1);
        }
    }
    let mut out = "\n## Baum\n\n".to_owned();
    walk(&mut out, &below, None, 0);
    out
}

/// `einfaelle.md` für ein Werk (`Some`) oder die Einfälle ohne Werk (`None`); ohne nicht gelöschte
/// Einfälle `None`.
async fn mirror_file(conn: &mut SqliteConnection, work: Option<WorkId>) -> Result<Option<MirrorFile>, StoreError> {
    let scope = match work {
        Some(work) => IdeaScope::Work(work),
        None => IdeaScope::NoWork,
    };
    let (condition, binds) = conditions(&IdeaFilter { scope, ..IdeaFilter::default() }, false);
    let sql = format!("{SELECT} WHERE {condition} ORDER BY i.created DESC, i.id DESC");
    let rows: Vec<Row> =
        bind_all!(sqlx::query_as(sqlx::AssertSqlSafe(sql.as_str())), &binds).fetch_all(&mut *conn).await?;
    if rows.is_empty() {
        return Ok(None);
    }
    let mut contents = match work {
        Some(work) => {
            let title: String = sqlx::query_scalar("SELECT title FROM work WHERE id = ?")
                .bind(work)
                .fetch_optional(&mut *conn)
                .await?
                .ok_or(StoreError::UnknownWork(work))?;
            format!("# Einfälle – {title}\n")
        }
        None => "# Einfälle ohne Werk\n".to_owned(),
    };
    // Runde 25: nach den übrigen Feldern die Zeile „- Verbindungen: …“ (nur aktive, zu nicht gelöschten Einfällen).
    let links = crate::idea_links::mirror_lines(conn, work).await?;
    let mut ideas = Vec::with_capacity(rows.len());
    for row in rows {
        ideas.push(to_idea(conn, row).await?);
    }
    contents.push_str(&tree_markdown(&ideas));
    for idea in ideas {
        contents.push('\n');
        contents.push_str(&idea.entry_markdown());
        if let Some(line) = links.get(&idea.id) {
            push_value(&mut contents, "Verbindungen", line);
        }
    }
    Ok(Some(MirrorFile { path: MIRROR_FILE.to_owned(), contents }))
}

/// Dateien der Einfälle im Werkspiegel: `einfaelle.md`, nur wenn das Werk mindestens einen nicht
/// gelöschten Einfall hat (sonst leer, damit Spiegel und Export der übrigen Werke gleich bleiben).
pub(crate) async fn mirror_files(conn: &mut SqliteConnection, work: WorkId) -> Result<Vec<MirrorFile>, StoreError> {
    Ok(mirror_file(conn, Some(work)).await?.into_iter().collect())
}

// ---- Suche -------------------------------------------------------------------------------------

const MARK_OPEN: char = '\u{E000}';
const MARK_CLOSE: char = '\u{E001}';

/// Roh-Ausschnitt mit den Marken U+E000/U+E001 → maskiertes HTML mit `<mark>`, immer ausgeglichen.
fn snippet_html(raw: &str) -> String {
    let mut html = String::with_capacity(raw.len() + 16);
    let mut open = false;
    for c in raw.chars() {
        match c {
            MARK_OPEN if !open => {
                html.push_str("<mark>");
                open = true;
            }
            MARK_CLOSE if open => {
                html.push_str("</mark>");
                open = false;
            }
            MARK_OPEN | MARK_CLOSE => {}
            '&' => html.push_str("&amp;"),
            '<' => html.push_str("&lt;"),
            '>' => html.push_str("&gt;"),
            '"' => html.push_str("&quot;"),
            '\'' => html.push_str("&#39;"),
            c => html.push(c),
        }
    }
    if open {
        html.push_str("</mark>");
    }
    html
}

// ---- Baum (Runde 34) ---------------------------------------------------------------------------

/// Geschwister in ihrer Reihenfolge (`position`, dann `id`; auch gelöschte, damit sie beim Wiederherstellen ihren
/// Platz behalten), ohne den Einfall `except`.
async fn siblings(
    conn: &mut SqliteConnection,
    work: Option<WorkId>,
    parent: Option<IdeaId>,
    except: IdeaId,
) -> Result<Vec<(IdeaId, i64)>, StoreError> {
    Ok(match parent {
        Some(parent) => {
            sqlx::query_as("SELECT id, position FROM idea WHERE parent_id = ? AND id <> ? ORDER BY position, id")
                .bind(parent)
                .bind(except)
                .fetch_all(&mut *conn)
                .await?
        }
        None => {
            sqlx::query_as(
                "SELECT id, position FROM idea WHERE parent_id IS NULL AND work_id IS ? AND id <> ? \
                 ORDER BY position, id",
            )
            .bind(work)
            .bind(except)
            .fetch_all(&mut *conn)
            .await?
        }
    })
}

/// Setzt den Einfall `id` unter seinen Geschwistern an seinen Platz: vor `before` (ein nicht gelöschtes
/// Geschwister; nummeriert die Geschwister lückenlos ab 1 neu) oder ans Ende.
async fn place(
    conn: &mut SqliteConnection,
    id: IdeaId,
    work: Option<WorkId>,
    parent: Option<IdeaId>,
    before: Option<IdeaId>,
) -> Result<(), StoreError> {
    let Some(before) = before else {
        // Ans Ende. Solange alle Geschwister noch die Vorgabe 0 tragen und älter sind (kleinere id), bleibt auch der
        // neue bei 0: Die Reihenfolge nach id stellt ihn ohnehin ans Ende, und ein Werk ohne geordneten Baum
        // behält lauter Nullen (im Austausch dann ohne `platz`, wie vor Runde 34).
        let order = siblings(conn, work, parent, id).await?;
        let last = order.iter().map(|(_, position)| *position).max().unwrap_or(0);
        if last > 0 {
            sqlx::query("UPDATE idea SET position = ? WHERE id = ?")
                .bind(last + 1)
                .bind(id)
                .execute(&mut *conn)
                .await?;
        } else if order.iter().all(|(sibling, _)| *sibling < id) {
            sqlx::query("UPDATE idea SET position = 0 WHERE id = ?").bind(id).execute(&mut *conn).await?;
        } else {
            // Lauter Nullen, aber ein jüngeres Geschwister stünde nach id dahinter: einmal lückenlos nummerieren.
            for (index, (sibling, _)) in order.iter().enumerate() {
                sqlx::query("UPDATE idea SET position = ? WHERE id = ?")
                    .bind(index as i64 + 1)
                    .bind(sibling)
                    .execute(&mut *conn)
                    .await?;
            }
            sqlx::query("UPDATE idea SET position = ? WHERE id = ?")
                .bind(order.len() as i64 + 1)
                .bind(id)
                .execute(&mut *conn)
                .await?;
        }
        return Ok(());
    };
    if before == id {
        return Err(refuse("Ein Einfall lässt sich nicht vor sich selbst einordnen."));
    }
    let target = get(conn, before).await?;
    not_deleted(&target)?;
    if target.work != work || target.parent != parent {
        return Err(refuse(format!(
            "Einfall {before} ist kein Geschwister: Einordnen geht nur vor einen Einfall mit demselben Elternknoten."
        )));
    }
    let mut order: Vec<(IdeaId, i64)> = siblings(conn, work, parent, id).await?;
    let at = order.iter().position(|(sibling, _)| *sibling == before).unwrap_or(order.len());
    order.insert(at, (id, -1));
    for (index, (sibling, position)) in order.into_iter().enumerate() {
        let wanted = index as i64 + 1;
        if position != wanted {
            sqlx::query("UPDATE idea SET position = ? WHERE id = ?")
                .bind(wanted)
                .bind(sibling)
                .execute(&mut *conn)
                .await?;
        }
    }
    Ok(())
}

/// Tiefe eines Einfalls: 1 ohne Elternknoten, sonst Tiefe des Elternknotens + 1. Dabei darf `forbidden` nicht
/// unter den Vorfahren (oder der Einfall selbst) sein – sonst entstünde ein Kreis.
async fn depth_of(conn: &mut SqliteConnection, id: IdeaId, forbidden: Option<IdeaId>) -> Result<usize, StoreError> {
    let mut depth = 0usize;
    let mut at = Some(id);
    while let Some(current) = at {
        if Some(current) == forbidden {
            return Err(refuse(
                "Ein Einfall lässt sich nicht unter sich selbst oder einen eigenen Unterknoten hängen.",
            ));
        }
        depth += 1;
        if depth > 4 * MAX_IDEA_DEPTH {
            return Err(broken("Kreis in den Elternknoten der Einfälle".to_owned()));
        }
        at = sqlx::query_scalar::<_, Option<i64>>("SELECT parent_id FROM idea WHERE id = ?")
            .bind(current)
            .fetch_optional(&mut *conn)
            .await?
            .flatten();
    }
    Ok(depth)
}

/// Der Einfall und alle seine Nachfahren (auch gelöschte) mit ihrer Tiefe ab 1, der Einfall zuerst, dann nach id.
async fn subtree(conn: &mut SqliteConnection, id: IdeaId) -> Result<Vec<(IdeaId, i64, Option<String>)>, StoreError> {
    Ok(sqlx::query_as(
        "WITH RECURSIVE sub(id, depth) AS (SELECT ?, 1 UNION ALL SELECT i.id, sub.depth + 1 FROM idea i \
         JOIN sub ON i.parent_id = sub.id WHERE sub.depth < 200) \
         SELECT sub.id, sub.depth, i.deleted FROM sub JOIN idea i ON i.id = sub.id ORDER BY sub.depth > 1, sub.id",
    )
    .bind(id)
    .fetch_all(&mut *conn)
    .await?)
}

/// Elternknoten prüfen: vorhanden, nicht gelöscht, im Werk `work` (das es geben muss); liefert seine Tiefe.
/// `moving` = der Einfall, der darunter gehängt wird (darf weder der Elternknoten noch einer seiner Vorfahren sein).
async fn check_parent(
    conn: &mut SqliteConnection,
    work: Option<WorkId>,
    parent: IdeaId,
    moving: Option<IdeaId>,
) -> Result<usize, StoreError> {
    let above = get(conn, parent).await?;
    if above.deleted.is_some() {
        return Err(refuse(format!("Einfall {parent} ist gelöscht; darunter lässt sich nichts hängen.")));
    }
    if work.is_none() {
        return Err(refuse("Ein Unterknoten geht nur bei einem Einfall mit Werk."));
    }
    if above.work != work {
        return Err(refuse(format!(
            "Einfall {parent} gehört zu einem anderen Werk; ein Unterknoten bleibt im Werk seines Elternknotens."
        )));
    }
    depth_of(conn, parent, moving).await
}

fn too_deep(depth: usize) -> StoreError {
    refuse(format!("Die Mindmap wäre hier {depth} Einfälle tief; erlaubt sind höchstens {MAX_IDEA_DEPTH}."))
}

/// Einfall anlegen in der Transaktion des Aufrufers (auch für `Store::post_to_idea`, Runde 8): erst
/// alles prüfen, dann schreiben; ein `Err` lässt den Aufrufer zurückrollen.
pub(crate) async fn create_in(conn: &mut SqliteConnection, new: &NewIdea<'_>) -> Result<Idea, StoreError> {
    create_at_in(conn, new, &IdeaSpot::default()).await
}

/// Wie [`create_in`], mit Titel und Platz in der Mindmap (Runde 34).
pub(crate) async fn create_at_in(
    conn: &mut SqliteConnection,
    new: &NewIdea<'_>,
    spot: &IdeaSpot<'_>,
) -> Result<Idea, StoreError> {
    let title = clean_title(spot.title)?;
    let words = clean_words_or_none(new.words)?;
    if title.is_empty() && words.is_empty() {
        return Err(refuse("Ein Einfall braucht einen Titel oder mindestens ein Schlagwort."));
    }
    let note = clean_text(new.note, MAX_TEXT_CHARS, "Der Gedanke")?;
    let quote = clean_text(new.quote, MAX_TEXT_CHARS, "Der markierte Text")?;
    let source = clean_text(new.source, MAX_SHORT_CHARS, "Die Quelle")?;
    let bible = clean_text(new.bible, MAX_SHORT_CHARS, "Die Bibelstelle")?;
    let (kind, name) = new.origin.columns();
    if name.trim().is_empty() {
        return Err(refuse("Die Herkunft eines Einfalls braucht einen Namen."));
    }
    if let Some(work) = new.work {
        check_work(conn, work).await?;
    }
    check_key(conn, new.work, new.key).await?;
    if let Some(parent) = spot.parent {
        let depth = check_parent(conn, new.work, parent, None).await? + 1;
        if depth > MAX_IDEA_DEPTH {
            return Err(too_deep(depth));
        }
    }
    if spot.main_topic && new.work.is_none() {
        return Err(refuse("Ein Hauptthema der Mindmap geht nur bei einem Einfall mit Werk."));
    }
    let main_topic = spot.main_topic && spot.parent.is_none() && new.key.is_none();
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO idea (work_id, node_id, note, quote, source, bible, origin_kind, origin_name, title, parent_id, \
         main_topic) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(new.work)
    .bind(new.key)
    .bind(&note)
    .bind(&quote)
    .bind(&source)
    .bind(&bible)
    .bind(kind)
    .bind(name)
    .bind(&title)
    .bind(spot.parent)
    .bind(i64::from(main_topic))
    .fetch_one(&mut *conn)
    .await?;
    place(conn, id, new.work, spot.parent, spot.before).await?;
    write_words(conn, id, &words).await?;
    reindex(conn, id).await?;
    get(conn, id).await
}

/// Zahl der Knoten und größte Tiefe eines Baums aus [`IdeaSeed`] (ohne Rekursion, damit eine sehr tiefe Eingabe
/// den Stapel nicht sprengt).
fn seed_size(nodes: &[IdeaSeed]) -> (usize, usize) {
    let mut count = 0usize;
    let mut deepest = 0usize;
    let mut stack: Vec<(&IdeaSeed, usize)> = nodes.iter().map(|n| (n, 1)).collect();
    while let Some((node, depth)) = stack.pop() {
        count += 1;
        deepest = deepest.max(depth);
        stack.extend(node.children.iter().map(|c| (c, depth + 1)));
    }
    (count, deepest)
}

impl Store {
    /// Neuer Einfall (atomar). Unbekanntes Werk → `UnknownWork`, Bezug außerhalb des Werks oder
    /// verletzte Grenze → `Idea`, nichts angelegt.
    pub async fn idea_create(&self, new: &NewIdea<'_>) -> Result<Idea, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let idea = create_in(&mut tx, new).await?;
        tx.commit().await?;
        Ok(idea)
    }

    /// Neuer Einfall an einem Platz der Mindmap (Runde 34, atomar): mit Titel (dann dürfen die Schlagwörter
    /// fehlen), unter einem Elternknoten, vor einem Geschwister oder als freies Hauptthema. Verletzte Regel
    /// (anderes Werk, gelöschter Elternknoten, Tiefe, fremdes Geschwister) → `Idea`, nichts angelegt.
    pub async fn idea_create_at(&self, new: &NewIdea<'_>, spot: &IdeaSpot<'_>) -> Result<Idea, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let idea = create_at_in(&mut tx, new, spot).await?;
        tx.commit().await?;
        Ok(idea)
    }

    /// Ein Einfall, auch ein gelöschter. Unbekannt → `UnknownIdea`.
    pub async fn idea_get(&self, id: IdeaId) -> Result<Idea, StoreError> {
        let mut conn = self.pool.acquire().await?;
        get(&mut conn, id).await
    }

    /// Einfälle nach dem Filter, neueste zuerst.
    pub async fn idea_list(&self, filter: &IdeaFilter) -> Result<Vec<Idea>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        let (condition, binds) = conditions(filter, true);
        let sql = format!("{SELECT} WHERE {condition} ORDER BY i.created DESC, i.id DESC LIMIT ? OFFSET ?");
        let rows: Vec<Row> = bind_all!(sqlx::query_as(sqlx::AssertSqlSafe(sql.as_str())), &binds)
            .bind(filter.limit())
            .bind(i64::from(filter.offset))
            .fetch_all(&mut *conn)
            .await?;
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(to_idea(&mut conn, row).await?);
        }
        Ok(out)
    }

    /// Schlagwort-Wolke über Werk, Status und gelöscht/vorhanden des Filters (der Wortfilter gilt
    /// hier nicht): je Schlüssel ein Wort mit der Zahl der Einfälle, häufigste zuerst.
    pub async fn idea_words(&self, filter: &IdeaFilter) -> Result<Vec<WordCount>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        let (condition, binds) = conditions(filter, false);
        let sql = format!(
            "SELECT min(w.word), count(*) FROM idea_word w JOIN idea i ON i.id = w.idea_id WHERE {condition} \
             GROUP BY w.word_key ORDER BY count(*) DESC, w.word_key LIMIT ?"
        );
        let rows: Vec<(String, i64)> = bind_all!(sqlx::query_as(sqlx::AssertSqlSafe(sql.as_str())), &binds)
            .bind(filter.limit())
            .fetch_all(&mut *conn)
            .await?;
        Ok(rows
            .into_iter()
            .map(|(word, count)| WordCount { word, count: u32::try_from(count).unwrap_or(u32::MAX) })
            .collect())
    }

    /// Volltextsuche (FTS5) über Schlagwörter, Gedanke, Markierung, Quelle und Bibelstelle. `fts` ist
    /// ein fertiger FTS5-Ausdruck (im Server aus `baruch_library::fts_query`); dazu der maskierte
    /// Ausschnitt (HTML mit `<mark>`). Gelöschte stehen nicht im Index. Ungültiger Ausdruck → `Idea`.
    pub async fn idea_search(&self, fts: &str, filter: &IdeaFilter) -> Result<Vec<(Idea, String)>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        let filter = IdeaFilter { deleted: false, ..filter.clone() };
        let (condition, binds) = conditions(&filter, true);
        let sql = format!(
            "SELECT i.id, snippet(idea_fts, -1, char(57344), char(57345), '…', 12) FROM idea_fts \
             JOIN idea i ON i.id = idea_fts.rowid WHERE idea_fts MATCH ? AND {condition} \
             ORDER BY rank LIMIT ? OFFSET ?"
        );
        let query = sqlx::query_as(sqlx::AssertSqlSafe(sql.as_str())).bind(fts.to_owned());
        let rows: Vec<(i64, String)> = match bind_all!(query, &binds)
            .bind(filter.limit())
            .bind(i64::from(filter.offset))
            .fetch_all(&mut *conn)
            .await
        {
            Ok(rows) => rows,
            Err(sqlx::Error::Database(e)) => return Err(refuse(format!("Suche: {}", e.message()))),
            Err(e) => return Err(e.into()),
        };
        let mut out = Vec::with_capacity(rows.len());
        for (id, snippet) in rows {
            out.push((get(&mut conn, id).await?, snippet_html(&snippet)));
        }
        Ok(out)
    }

    /// Einfall ändern (atomar). Gleich wie vorher → `Ok(None)`, nichts geändert. Gelöscht → `Idea`.
    pub async fn idea_update(&self, id: IdeaId, edit: &IdeaEdit<'_>) -> Result<Option<Idea>, StoreError> {
        let words = edit.words.map(clean_words_or_none).transpose()?;
        let title = edit.title.map(clean_title).transpose()?;
        let text = |value: Option<&str>, max, what| value.map(|v| clean_text(v, max, what)).transpose();
        let note = text(edit.note, MAX_TEXT_CHARS, "Der Gedanke")?;
        let quote = text(edit.quote, MAX_TEXT_CHARS, "Der markierte Text")?;
        let source = text(edit.source, MAX_SHORT_CHARS, "Die Quelle")?;
        let bible = text(edit.bible, MAX_SHORT_CHARS, "Die Bibelstelle")?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let before = get(&mut tx, id).await?;
        not_deleted(&before)?;
        let work = edit.work.unwrap_or(before.work);
        if let Some(work) = work.filter(|w| Some(*w) != before.work) {
            check_work(&mut tx, work).await?;
        }
        let key = match edit.key {
            Some(key) => key,
            None if work != before.work => None,
            None => before.key,
        };
        check_key(&mut tx, work, key).await?;
        let (status, done_as, done_ref) = match edit.status {
            Some(IdeaStatus::Open) => (IdeaStatus::Open, None, None),
            Some(IdeaStatus::Done) if before.status == IdeaStatus::Open => {
                (IdeaStatus::Done, Some(IdeaUse::Note), None)
            }
            _ => (before.status, before.done_as, before.done_ref.clone()),
        };
        let new_words: Option<Vec<String>> = words.as_ref().map(|w| w.iter().map(|(word, _)| word.clone()).collect());
        if title.as_deref().unwrap_or(&before.title).is_empty()
            && new_words.as_ref().map_or(before.words.len(), Vec::len) == 0
        {
            return Err(refuse("Ein Einfall braucht einen Titel oder mindestens ein Schlagwort."));
        }
        // Runde 34: Ein Werkwechsel löst vom Elternknoten (er gehört zum alten Werk); verliert ein Einfall ohne
        // Elternknoten seinen Bezug, bleibt `main_topic` wie es war (Vorgabe: Wolke). Mit Bezug ist er kein
        // freies Hauptthema mehr.
        let moved = work != before.work;
        let parent = if moved { None } else { before.parent };
        let main_topic = before.main_topic && !moved && key.is_none() && parent.is_none();
        let changed = new_words.as_ref().is_some_and(|w| *w != before.words)
            || title.as_ref().is_some_and(|v| *v != before.title)
            || main_topic != before.main_topic
            || note.as_ref().is_some_and(|v| *v != before.note)
            || quote.as_ref().is_some_and(|v| *v != before.quote)
            || source.as_ref().is_some_and(|v| *v != before.source)
            || bible.as_ref().is_some_and(|v| *v != before.bible)
            || work != before.work
            || key != before.key
            || status != before.status
            || done_as != before.done_as
            || done_ref != before.done_ref;
        if !changed {
            return Ok(None);
        }
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "UPDATE idea SET work_id = ?, node_id = ?, title = ?, parent_id = ?, main_topic = ?, note = ?, quote = ?, \
             source = ?, bible = ?, status = ?, done_as = ?, done_ref = ?, updated = {NOW} WHERE id = ?"
        )))
        .bind(work)
        .bind(key)
        .bind(title.as_deref().unwrap_or(&before.title))
        .bind(parent)
        .bind(i64::from(main_topic))
        .bind(note.as_deref().unwrap_or(&before.note))
        .bind(quote.as_deref().unwrap_or(&before.quote))
        .bind(source.as_deref().unwrap_or(&before.source))
        .bind(bible.as_deref().unwrap_or(&before.bible))
        .bind(status.as_str())
        .bind(done_as.map(IdeaUse::as_str))
        .bind(done_ref)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        if work != before.work {
            // Verbindungen gibt es nur innerhalb eines Werks (idea_links::check_pair): Beim Umzug werden die
            // aktiven Verbindungen des Einfalls weich gelöst (wiederherstellbar, sobald beide wieder in einem
            // Werk sind) und seine Lagen auf Karten hart entfernt (Grabstein im alten Werk); sonst verbände eine
            // Verbindung zwei Werke und eine alte Lage tauchte beim Zurückziehen wieder auf.
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "UPDATE idea_link SET deleted = {NOW}, updated = {NOW} \
                 WHERE deleted IS NULL AND (from_id = ? OR to_id = ?)"
            )))
            .bind(id)
            .bind(id)
            .execute(&mut *tx)
            .await?;
            sqlx::query("DELETE FROM idea_map_pos WHERE idea_id = ?").bind(id).execute(&mut *tx).await?;
            // Runde 34: Die Unterknoten bleiben im alten Werk und verlieren den Elternknoten; der Einfall selbst
            // kommt im neuen Werk ans Ende.
            sqlx::query("UPDATE idea SET parent_id = NULL WHERE parent_id = ?").bind(id).execute(&mut *tx).await?;
            place(&mut tx, id, work, None, None).await?;
        }
        if let Some(words) = &words {
            write_words(&mut tx, id, words).await?;
        }
        reindex(&mut tx, id).await?;
        let idea = get(&mut tx, id).await?;
        tx.commit().await?;
        Ok(Some(idea))
    }

    /// Einfall löschen (atomar): `deleted` gesetzt, aus dem Index; wiederherstellbar. Schon gelöscht
    /// → `Idea`. Seit Runde 34 samt nicht gelöschten Unterknoten ([`Store::idea_delete_tree`]).
    pub async fn idea_delete(&self, id: IdeaId) -> Result<Idea, StoreError> {
        Ok(self.idea_delete_tree(id).await?.idea)
    }

    /// Einfall samt nicht gelöschten Nachfahren weich löschen (Runde 34, atomar): alle mit demselben Zeitstempel,
    /// damit [`Store::idea_restore_tree`] genau diese wiederfindet. Verbindungen und Lagen bleiben stehen.
    pub async fn idea_delete_tree(&self, id: IdeaId) -> Result<IdeaTreeChange, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let before = get(&mut tx, id).await?;
        if before.deleted.is_some() {
            return Err(refuse(format!("Einfall {id} ist schon gelöscht.")));
        }
        let stamp: String =
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT {NOW}"))).fetch_one(&mut *tx).await?;
        let ids: Vec<IdeaId> = subtree(&mut tx, id)
            .await?
            .into_iter()
            .filter(|(_, _, deleted)| deleted.is_none())
            .map(|(node, _, _)| node)
            .collect();
        for node in &ids {
            sqlx::query("UPDATE idea SET deleted = ?, updated = ? WHERE id = ?")
                .bind(&stamp)
                .bind(&stamp)
                .bind(node)
                .execute(&mut *tx)
                .await?;
            reindex(&mut tx, *node).await?;
        }
        let idea = get(&mut tx, id).await?;
        tx.commit().await?;
        Ok(IdeaTreeChange { idea, ids, detached: false })
    }

    /// Gelöschten Einfall wiederherstellen (atomar), wieder im Index. Nicht gelöscht → `Idea`. Seit Runde 34 samt
    /// den mit ihm gelöschten Unterknoten ([`Store::idea_restore_tree`]).
    pub async fn idea_restore(&self, id: IdeaId) -> Result<Idea, StoreError> {
        Ok(self.idea_restore_tree(id).await?.idea)
    }

    /// Gelöschten Einfall samt den Nachfahren mit genau seinem Lösch-Zeitstempel wiederherstellen (Runde 34,
    /// atomar). Ist der Elternknoten noch gelöscht, wird der Einfall gelöst (`detached`): Er bleibt an seinem Bezug
    /// bzw. liegt in der Wolke.
    pub async fn idea_restore_tree(&self, id: IdeaId) -> Result<IdeaTreeChange, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let before = get(&mut tx, id).await?;
        let Some(stamp) = before.deleted.clone() else {
            return Err(refuse(format!("Einfall {id} ist nicht gelöscht.")));
        };
        // Nur über Knoten mit demselben Zeitstempel abwärts: Was früher einzeln gelöscht wurde, bleibt gelöscht.
        let ids: Vec<IdeaId> = sqlx::query_scalar(
            "WITH RECURSIVE sub(id, depth) AS (SELECT ?, 1 UNION ALL SELECT i.id, sub.depth + 1 FROM idea i \
             JOIN sub ON i.parent_id = sub.id WHERE i.deleted = ? AND sub.depth < 200) \
             SELECT id FROM sub ORDER BY depth > 1, id",
        )
        .bind(id)
        .bind(&stamp)
        .fetch_all(&mut *tx)
        .await?;
        let mut detached = false;
        if let Some(parent) = before.parent {
            let gone: Option<Option<String>> = sqlx::query_scalar("SELECT deleted FROM idea WHERE id = ?")
                .bind(parent)
                .fetch_optional(&mut *tx)
                .await?;
            if gone.flatten().is_some() {
                sqlx::query("UPDATE idea SET parent_id = NULL WHERE id = ?").bind(id).execute(&mut *tx).await?;
                place(&mut tx, id, before.work, None, None).await?;
                detached = true;
            }
        }
        for node in &ids {
            sqlx::query(sqlx::AssertSqlSafe(format!("UPDATE idea SET deleted = NULL, updated = {NOW} WHERE id = ?")))
                .bind(node)
                .execute(&mut *tx)
                .await?;
            reindex(&mut tx, *node).await?;
        }
        let idea = get(&mut tx, id).await?;
        tx.commit().await?;
        Ok(IdeaTreeChange { idea, ids, detached })
    }

    /// Einfall umhängen und ordnen (Runde 34, atomar). Nichts geändert → `Ok(None)`. Abgelehnt (`Idea`, nichts
    /// geändert): gelöschter Einfall, Elternknoten in einem anderen Werk, gelöscht oder ein eigener Nachfahre
    /// (Kreis), Tiefe über [`MAX_IDEA_DEPTH`], Bezug außerhalb des Werks, `before` kein Geschwister, `cloud` zusammen
    /// mit Elternknoten oder Bezug. Die Unterknoten wandern immer mit (sie hängen am Einfall).
    pub async fn idea_move(&self, id: IdeaId, mv: &IdeaMove) -> Result<Option<Idea>, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let before = get(&mut tx, id).await?;
        not_deleted(&before)?;
        if mv.cloud == Some(true) && (matches!(mv.parent, Some(Some(_))) || matches!(mv.key, Some(Some(_)))) {
            return Err(refuse("In die Wolke geht nur ohne Elternknoten und ohne Bezug."));
        }
        let (parent, key) = if mv.cloud == Some(true) {
            (None, None)
        } else {
            (mv.parent.unwrap_or(before.parent), mv.key.unwrap_or(before.key))
        };
        if before.work.is_none() && (parent.is_some() || mv.cloud.is_some()) {
            return Err(refuse("Die Mindmap gibt es nur für Einfälle mit Werk."));
        }
        if key != before.key {
            check_key(&mut tx, before.work, key).await?;
        }
        if let Some(parent) = parent.filter(|p| Some(*p) != before.parent) {
            let above = check_parent(&mut tx, before.work, parent, Some(id)).await?;
            let height = subtree(&mut tx, id).await?.iter().map(|(_, depth, _)| *depth).max().unwrap_or(1) as usize;
            if above + height > MAX_IDEA_DEPTH {
                return Err(too_deep(above + height));
            }
        }
        let main_topic = if parent.is_some() || key.is_some() {
            false
        } else {
            match mv.cloud {
                Some(cloud) => !cloud,
                // Ausdrücklich vom Elternknoten gelöst und ohne Bezug: ans Werk (Vertrag §7 „aufs Werk“).
                None if mv.parent == Some(None) => true,
                None => before.main_topic,
            }
        };
        let place_changed = parent != before.parent || key != before.key || main_topic != before.main_topic;
        let order_before = siblings(&mut tx, before.work, before.parent, -1).await?;
        if place_changed {
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "UPDATE idea SET parent_id = ?, node_id = ?, main_topic = ?, updated = {NOW} WHERE id = ?"
            )))
            .bind(parent)
            .bind(key)
            .bind(i64::from(main_topic))
            .bind(id)
            .execute(&mut *tx)
            .await?;
        }
        match mv.before {
            Some(target) => place(&mut tx, id, before.work, parent, target).await?,
            // Ohne Angabe ans Ende, wenn der Einfall unter andere Geschwister kommt (neuer Elternknoten bzw. ohne
            // Elternknoten ein anderer Bezug oder Platz); ändert sich nur der Bezug eines Unterknotens (der Link
            // zum Abschnitt), bleibt die Reihenfolge.
            None if parent != before.parent
                || (parent.is_none() && (key != before.key || main_topic != before.main_topic)) =>
            {
                place(&mut tx, id, before.work, parent, None).await?
            }
            None => {}
        }
        if !place_changed {
            let order_after = siblings(&mut tx, before.work, parent, -1).await?;
            let ids = |list: &[(IdeaId, i64)]| list.iter().map(|(sibling, _)| *sibling).collect::<Vec<_>>();
            if ids(&order_before) == ids(&order_after) {
                // Nichts geändert: die Transaktion (höchstens eine neue Nummer am selben Platz) verfällt.
                return Ok(None);
            }
            sqlx::query(sqlx::AssertSqlSafe(format!("UPDATE idea SET updated = {NOW} WHERE id = ?")))
                .bind(id)
                .execute(&mut *tx)
                .await?;
        }
        let idea = get(&mut tx, id).await?;
        tx.commit().await?;
        Ok(Some(idea))
    }

    /// Alle nicht gelöschten Einfälle eines Werks in einem Abruf, nach id (Runde 34: Baum der Mindmap).
    pub async fn idea_tree_rows(&self, work: WorkId) -> Result<Vec<Idea>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        check_work(&mut conn, work).await?;
        let rows: Vec<Row> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "{SELECT} WHERE i.work_id = ? AND i.deleted IS NULL ORDER BY i.id"
        )))
        .bind(work)
        .fetch_all(&mut *conn)
        .await?;
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            out.push(to_idea(&mut conn, row).await?);
        }
        Ok(out)
    }

    /// Legt einen Baum als Einfälle des Werks an (Runde 34, eine Transaktion): unter `parent` oder – ohne – als
    /// freie Hauptthemen am Werk, in der Reihenfolge der Eingabe; `links` werden Verbindungen der Art `leads_to`
    /// mit ihrer Notiz. Antwort: die neuen Einfälle in Tiefensuche und die Zahl der angelegten Verbindungen.
    ///
    /// Abgelehnt (`Idea`, nichts angelegt): leerer Baum, mehr als [`MAX_IDEA_IMPORT_NODES`] Knoten, (samt
    /// Elternknoten) tiefer als [`MAX_IDEA_DEPTH`], Titel/Notiz/Schlagwörter über den Grenzen der Einfälle, ein
    /// Pfad einer Verbindung ohne Knoten. Ein Knoten ohne Titel und Schlagwort heißt „(ohne Titel)“; eine
    /// Verbindung eines Knotens mit sich selbst oder eine doppelte wird übergangen.
    pub async fn idea_import_tree(
        &self,
        work: WorkId,
        parent: Option<IdeaId>,
        nodes: &[IdeaSeed],
        links: &[IdeaSeedLink],
        origin: &IdeaOrigin,
    ) -> Result<(Vec<IdeaId>, u32), StoreError> {
        let (max_nodes, max_depth) = (MAX_IDEA_IMPORT_NODES, MAX_IDEA_DEPTH);
        let (count, deepest) = seed_size(nodes);
        if count == 0 {
            return Err(refuse("Der Baum enthält keinen Knoten."));
        }
        if count > max_nodes {
            return Err(refuse(format!("Der Baum hat {count} Knoten; erlaubt sind höchstens {max_nodes}.")));
        }
        if deepest > max_depth {
            return Err(refuse(format!("Der Baum ist {deepest} Ebenen tief; erlaubt sind höchstens {max_depth}.")));
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        check_work(&mut tx, work).await?;
        let above = match parent {
            Some(parent) => check_parent(&mut tx, Some(work), parent, None).await?,
            None => 0,
        };
        if above + deepest > MAX_IDEA_DEPTH {
            return Err(too_deep(above + deepest));
        }
        // Tiefensuche in Reihenfolge der Eingabe, ohne Rekursion: (Knoten, Pfad, Elternknoten).
        let mut created: Vec<IdeaId> = Vec::with_capacity(count);
        let mut by_path: std::collections::HashMap<Vec<usize>, IdeaId> = std::collections::HashMap::new();
        let mut stack: Vec<(&IdeaSeed, Vec<usize>, Option<IdeaId>)> =
            nodes.iter().enumerate().rev().map(|(index, node)| (node, vec![index], parent)).collect();
        while let Some((node, path, above)) = stack.pop() {
            let mut title = clean_title(&node.title)?;
            if title.is_empty() && clean_words_or_none(&node.words)?.is_empty() {
                title = "(ohne Titel)".to_owned();
            }
            let new = NewIdea {
                work: Some(work),
                key: None,
                words: &node.words,
                note: &node.note,
                quote: "",
                source: "",
                bible: "",
                origin: origin.clone(),
            };
            let spot = IdeaSpot { title: &title, parent: above, before: None, main_topic: above.is_none() };
            let idea = create_at_in(&mut tx, &new, &spot).await?;
            created.push(idea.id);
            for (index, child) in node.children.iter().enumerate().rev() {
                let mut below = path.clone();
                below.push(index);
                stack.push((child, below, Some(idea.id)));
            }
            by_path.insert(path, idea.id);
        }
        let mut linked = 0u32;
        for link in links {
            let find = |path: &Vec<usize>| {
                by_path.get(path).copied().ok_or_else(|| {
                    refuse(format!(
                        "Eine Verbindung zeigt auf einen Knoten, den es im Baum nicht gibt (Pfad {path:?})."
                    ))
                })
            };
            let (from, to) = (find(&link.from)?, find(&link.to)?);
            if from == to {
                continue;
            }
            let new = crate::idea_links::NewIdeaLink {
                from,
                to,
                kind: crate::idea_links::LinkKind::LeadsTo,
                note: &link.note,
                origin: origin.clone(),
            };
            let (_, changed) = crate::idea_links::create_in(&mut tx, &new).await?;
            linked += u32::from(changed);
        }
        tx.commit().await?;
        Ok((created, linked))
    }

    /// Als verarbeitet markieren (atomar): `status = done`, `done_as`, `done_ref` (z. B. „key 12“).
    /// Gelöscht → `Idea`. Schon verarbeitet: wird mit der neuen Verarbeitung überschrieben.
    pub async fn idea_mark_done(&self, id: IdeaId, used: IdeaUse, done_ref: &str) -> Result<Idea, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let idea = mark_done_in(&mut tx, id, used, done_ref).await?;
        tx.commit().await?;
        Ok(idea)
    }

    /// `einfaelle.md` eines Werks (`Some`) bzw. der Einfälle ohne Werk (`None`, für den Git-Ordner
    /// `<data>/einfaelle/`); ohne nicht gelöschte Einfälle `None`. Unbekanntes Werk → `UnknownWork`.
    pub async fn idea_mirror_file(&self, work: Option<WorkId>) -> Result<Option<MirrorFile>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        if let Some(work) = work {
            check_work(&mut conn, work).await?;
        }
        mirror_file(&mut conn, work).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(list: &[&str]) -> Vec<String> {
        list.iter().map(|w| (*w).to_owned()).collect()
    }

    #[test]
    fn words_are_trimmed_merged_and_limited() {
        let cleaned = clean_words_or_none(&words(&["  Laube ", "laube", "Anna's   Garten", "", "LAUBE"])).unwrap();
        assert_eq!(
            cleaned,
            vec![("Laube".to_owned(), "laube".to_owned()), ("Anna's Garten".to_owned(), "anna's garten".to_owned())],
            "Ränder weg, Leerraum zusammengefasst, Doppel (gleicher Schlüssel) zusammengefasst, leere übergangen"
        );
        assert!(
            clean_words_or_none(&words(&["", "  "])).unwrap().is_empty(),
            "leere Wörter fallen weg; ob ein Einfall ohne Schlagwort geht (nur mit Titel), prüft der Aufrufer"
        );
        assert!(clean_words_or_none(&words(&["Rose\nDorn"])).is_err(), "Zeilenumbruch im Schlagwort abgelehnt");
        assert!(clean_words_or_none(&words(&["Rose\u{2028}Dorn"])).is_err(), "auch Unicode-Zeilentrenner");
        let long = "a".repeat(MAX_WORD_CHARS + 1);
        assert!(clean_words_or_none(&[long]).is_err(), "über 60 Zeichen abgelehnt");
        assert!(clean_words_or_none(&["ä".repeat(MAX_WORD_CHARS)]).is_ok(), "60 Zeichen (nicht Bytes) gehen");
        let many: Vec<String> = (0..=MAX_IDEA_WORDS).map(|i| format!("Wort{i}")).collect();
        assert!(clean_words_or_none(&many).is_err(), "21 Schlagwörter abgelehnt");
        assert!(clean_words_or_none(&many[..MAX_IDEA_WORDS]).is_ok(), "20 gehen");
    }

    #[test]
    fn entries_keep_values_verbatim_and_indent_further_lines() {
        let idea = Idea {
            id: 7,
            work: Some(1),
            key: Some(12),
            node_title: Some("Die Laube".into()),
            words: words(&["Laube", "Rose"]),
            note: "Erste Zeile\n# keine Überschrift".into(),
            quote: String::new(),
            source: "Literatur".into(),
            bible: String::new(),
            origin: IdeaOrigin::Mcp { handle: "@mcp:probe".into() },
            status: IdeaStatus::Done,
            done_as: Some(IdeaUse::Outline),
            done_ref: None,
            created: "2026-11-06T10:00:00.000Z".into(),
            updated: "2026-11-06T10:00:00.000Z".into(),
            deleted: None,
            title: String::new(),
            parent: None,
            position: 0,
            main_topic: false,
            cloud: false,
        };
        let entry = idea.entry_markdown();
        assert!(
            !entry.contains("- Titel:") && !entry.contains("Unterknoten von") && !entry.contains("- Platz:"),
            "ohne Titel, Elternknoten und Hauptthema bleibt der Eintrag wie vor Runde 34: {entry}"
        );
        assert!(entry.starts_with("## Einfall 7 · verarbeitet (outline)\n"), "Überschrift mit Status: {entry}");
        assert!(entry.contains("- Gedanke: Erste Zeile\n  # keine Überschrift\n"), "zweite Zeile eingerückt: {entry}");
        assert!(entry.contains("- Bezug: Abschnitt „Die Laube“ (key 12)\n"), "Bezug mit Titel und key: {entry}");
        assert!(entry.contains("- Herkunft: MCP @mcp:probe · 2026-11-06T10:00:00.000Z\n"), "Herkunft: {entry}");
        assert!(!entry.contains("Markiert") && !entry.contains("Bibelstelle"), "leere Felder fehlen: {entry}");
    }

    #[test]
    fn snippets_are_escaped_and_balanced() {
        assert_eq!(
            snippet_html("<b>\u{E000}Rose\u{E001} & 'x'\u{E000}"),
            "&lt;b&gt;<mark>Rose</mark> &amp; &#39;x&#39;<mark></mark>",
            "Text maskiert, Marken zu <mark>, offenes am Ende geschlossen"
        );
    }

    /// Ein Einfall für die Tests des Baums: nur, was Titel und Baum brauchen.
    fn node(id: IdeaId, title: &str, word: &str, parent: Option<IdeaId>, position: i64) -> Idea {
        Idea {
            id,
            work: Some(1),
            key: None,
            node_title: None,
            words: if word.is_empty() { Vec::new() } else { words(&[word]) },
            note: String::new(),
            quote: String::new(),
            source: String::new(),
            bible: String::new(),
            origin: IdeaOrigin::Human { name: "Autor".into() },
            status: IdeaStatus::Open,
            done_as: None,
            done_ref: None,
            created: "2026-11-25T10:00:00.000Z".into(),
            updated: "2026-11-25T10:00:00.000Z".into(),
            deleted: None,
            title: title.to_owned(),
            parent,
            position,
            main_topic: false,
            cloud: parent.is_none(),
        }
    }

    #[test]
    fn the_shown_title_is_the_title_else_the_words_else_the_note_and_never_empty() {
        // Vertrag R34 §0 „Titel“: angezeigter Titel = title, sonst Schlagwörter bzw. Anfang der Notiz, nie leer.
        assert_eq!(node(1, " Der Steg ", "Holz", None, 0).display_title(), "Der Steg", "der Titel geht vor");
        assert_eq!(node(2, "", "Holz", None, 0).display_title(), "Holz", "ohne Titel die Schlagwörter");
        let mut noted = node(3, "", "", None, 0);
        noted.note = "Ein\nlanger Gedanke".into();
        assert_eq!(noted.display_title(), "Ein langer Gedanke", "ohne Schlagwort der Anfang der Notiz, eine Zeile");
        assert_eq!(node(4, "", "", None, 0).display_title(), "Einfall 4", "sonst die Nummer – nie leer");
        let mut long = node(5, "", "", None, 0);
        long.note = "x".repeat(80);
        let shown = long.display_title();
        assert!(shown.chars().count() == 60 && shown.ends_with('…'), "gekürzt auf 60 Zeichen mit Auslassung: {shown}");
        assert!(!node(6, "", "Holz", None, 0).has_note(), "ohne Notiz, Zitat, Quelle, Bibelstelle kein Notiz-Symbol");
        assert!(noted.has_note(), "mit Gedanke ein Notiz-Symbol");
    }

    #[test]
    fn the_mirror_tree_indents_children_orders_siblings_and_names_the_place() {
        // Spiegel `einfaelle.md` zeigt Titel und Einrückung nach Baum (Vertrag R34 §10).
        let mut topic = node(1, "Ufer", "", None, 2);
        topic.main_topic = true;
        topic.cloud = false;
        let mut anchored = node(5, "", "Pegel", None, 1);
        anchored.key = Some(12);
        anchored.cloud = false;
        let ideas = [
            topic,
            node(2, "Steg", "", Some(1), 2),
            node(3, "Boot", "", Some(1), 1),
            node(4, "Ruder", "", Some(3), 0),
            anchored,
            node(6, "", "Nebel", None, 3),
            // Elternknoten 99 steht nicht in der Liste (gelöscht): zählt wie keiner.
            node(7, "Verwaist", "", Some(99), 4),
        ];
        let tree = tree_markdown(&ideas);
        assert_eq!(
            tree,
            "\n## Baum\n\n\
             - Einfall 5: Pegel (am Abschnitt key 12)\n\
             - Einfall 1: Ufer (Hauptthema)\n\
             \x20 - Einfall 3: Boot\n\
             \x20   - Einfall 4: Ruder\n\
             \x20 - Einfall 2: Steg\n\
             - Einfall 6: Nebel (Wolke)\n\
             - Einfall 7: Verwaist (Wolke)\n",
            "oberste Ebene und Geschwister nach position, dann id; je Ebene zwei Leerzeichen; Platz nur oben genannt"
        );
        let flat = [node(1, "", "Holz", None, 0), node(2, "", "Teer", None, 0)];
        assert_eq!(tree_markdown(&flat), "", "ohne Titel, Elternknoten und Hauptthema kein Abschnitt „Baum“");
    }
}
