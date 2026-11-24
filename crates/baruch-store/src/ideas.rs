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

use sqlx::SqliteConnection;

use crate::{MirrorFile, Store, StoreError, WorkId, broken};

pub type IdeaId = i64;

/// Höchstzahl der Schlagwörter eines Einfalls.
pub const MAX_IDEA_WORDS: usize = 20;
/// Höchstlänge eines Schlagworts in Zeichen.
pub const MAX_WORD_CHARS: usize = 60;
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
fn clean_words(words: &[String]) -> Result<Vec<(String, String)>, StoreError> {
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
    if out.is_empty() {
        return Err(refuse("Ein Einfall braucht mindestens ein Schlagwort."));
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
}

const SELECT: &str = "SELECT i.id, i.work_id, i.node_id, n.title AS node_title, i.note, i.quote, i.source, i.bible, \
     i.origin_kind, i.origin_name, i.status, i.done_as, i.done_ref, i.created, i.updated, i.deleted \
     FROM idea i LEFT JOIN node n ON n.id = i.node_id";

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
    })
}

pub(crate) async fn get(conn: &mut SqliteConnection, id: IdeaId) -> Result<Idea, StoreError> {
    let row: Option<Row> =
        sqlx::query_as(&format!("{SELECT} WHERE i.id = ?")).bind(id).fetch_optional(&mut *conn).await?;
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
    let row: Option<(String, String, String, String, Option<String>)> =
        sqlx::query_as("SELECT note, quote, source, bible, deleted FROM idea WHERE id = ?")
            .bind(id)
            .fetch_optional(&mut *conn)
            .await?;
    let Some((note, quote, source, bible, deleted)) = row else { return Ok(()) };
    if deleted.is_some() {
        return Ok(());
    }
    let words = words_of(conn, id).await?.join(" ");
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
    sqlx::query(&format!("UPDATE idea SET status = 'done', done_as = ?, done_ref = ?, updated = {NOW} WHERE id = ?"))
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
        push_value(&mut out, "Schlagwörter", &self.words.join(", "));
        push_value(&mut out, "Gedanke", &self.note);
        push_value(&mut out, "Markiert", &self.quote);
        if let Some(key) = self.key {
            let title = self.node_title.as_deref().unwrap_or("");
            push_value(&mut out, "Bezug", &format!("Abschnitt „{title}“ (key {key})"));
        }
        push_value(&mut out, "Quelle", &self.source);
        push_value(&mut out, "Bibelstelle", &self.bible);
        push_value(&mut out, "Herkunft", &format!("{} · {}", self.origin.label(), self.created));
        out
    }
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
    let rows: Vec<Row> = bind_all!(sqlx::query_as(&sql), &binds).fetch_all(&mut *conn).await?;
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
    for row in rows {
        let idea = to_idea(conn, row).await?;
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

/// Einfall anlegen in der Transaktion des Aufrufers (auch für `Store::post_to_idea`, Runde 8): erst
/// alles prüfen, dann schreiben; ein `Err` lässt den Aufrufer zurückrollen.
pub(crate) async fn create_in(conn: &mut SqliteConnection, new: &NewIdea<'_>) -> Result<Idea, StoreError> {
    let words = clean_words(new.words)?;
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
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO idea (work_id, node_id, note, quote, source, bible, origin_kind, origin_name) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(new.work)
    .bind(new.key)
    .bind(&note)
    .bind(&quote)
    .bind(&source)
    .bind(&bible)
    .bind(kind)
    .bind(name)
    .fetch_one(&mut *conn)
    .await?;
    write_words(conn, id, &words).await?;
    reindex(conn, id).await?;
    get(conn, id).await
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
        let rows: Vec<Row> = bind_all!(sqlx::query_as(&sql), &binds)
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
        let rows: Vec<(String, i64)> =
            bind_all!(sqlx::query_as(&sql), &binds).bind(filter.limit()).fetch_all(&mut *conn).await?;
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
        let query = sqlx::query_as(&sql).bind(fts.to_owned());
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
        let words = edit.words.map(clean_words).transpose()?;
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
        let changed = new_words.as_ref().is_some_and(|w| *w != before.words)
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
        sqlx::query(&format!(
            "UPDATE idea SET work_id = ?, node_id = ?, note = ?, quote = ?, source = ?, bible = ?, status = ?, \
             done_as = ?, done_ref = ?, updated = {NOW} WHERE id = ?"
        ))
        .bind(work)
        .bind(key)
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
            sqlx::query(&format!(
                "UPDATE idea_link SET deleted = {NOW}, updated = {NOW} \
                 WHERE deleted IS NULL AND (from_id = ? OR to_id = ?)"
            ))
            .bind(id)
            .bind(id)
            .execute(&mut *tx)
            .await?;
            sqlx::query("DELETE FROM idea_map_pos WHERE idea_id = ?").bind(id).execute(&mut *tx).await?;
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
    /// → `Idea`.
    pub async fn idea_delete(&self, id: IdeaId) -> Result<Idea, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let before = get(&mut tx, id).await?;
        if before.deleted.is_some() {
            return Err(refuse(format!("Einfall {id} ist schon gelöscht.")));
        }
        sqlx::query(&format!("UPDATE idea SET deleted = {NOW}, updated = {NOW} WHERE id = ?"))
            .bind(id)
            .execute(&mut *tx)
            .await?;
        reindex(&mut tx, id).await?;
        let idea = get(&mut tx, id).await?;
        tx.commit().await?;
        Ok(idea)
    }

    /// Gelöschten Einfall wiederherstellen (atomar), wieder im Index. Nicht gelöscht → `Idea`.
    pub async fn idea_restore(&self, id: IdeaId) -> Result<Idea, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let before = get(&mut tx, id).await?;
        if before.deleted.is_none() {
            return Err(refuse(format!("Einfall {id} ist nicht gelöscht.")));
        }
        sqlx::query(&format!("UPDATE idea SET deleted = NULL, updated = {NOW} WHERE id = ?"))
            .bind(id)
            .execute(&mut *tx)
            .await?;
        reindex(&mut tx, id).await?;
        let idea = get(&mut tx, id).await?;
        tx.commit().await?;
        Ok(idea)
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
        let cleaned = clean_words(&words(&["  Laube ", "laube", "Anna's   Garten", "", "LAUBE"])).unwrap();
        assert_eq!(
            cleaned,
            vec![("Laube".to_owned(), "laube".to_owned()), ("Anna's Garten".to_owned(), "anna's garten".to_owned())],
            "Ränder weg, Leerraum zusammengefasst, Doppel (gleicher Schlüssel) zusammengefasst, leere übergangen"
        );
        assert!(clean_words(&words(&["", "  "])).is_err(), "ohne Schlagwort abgelehnt");
        assert!(clean_words(&words(&["Rose\nDorn"])).is_err(), "Zeilenumbruch im Schlagwort abgelehnt");
        assert!(clean_words(&words(&["Rose\u{2028}Dorn"])).is_err(), "auch Unicode-Zeilentrenner");
        let long = "a".repeat(MAX_WORD_CHARS + 1);
        assert!(clean_words(&[long]).is_err(), "über 60 Zeichen abgelehnt");
        assert!(clean_words(&["ä".repeat(MAX_WORD_CHARS)]).is_ok(), "60 Zeichen (nicht Bytes) gehen");
        let many: Vec<String> = (0..=MAX_IDEA_WORDS).map(|i| format!("Wort{i}")).collect();
        assert!(clean_words(&many).is_err(), "21 Schlagwörter abgelehnt");
        assert!(clean_words(&many[..MAX_IDEA_WORDS]).is_ok(), "20 gehen");
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
        };
        let entry = idea.entry_markdown();
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
}
