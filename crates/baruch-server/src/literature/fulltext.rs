// SPDX-License-Identifier: AGPL-3.0-or-later
//! Full text of the literature through MultiGPT (Round 13, „quellen“): read parts of a document
//! ([`read`], RPC `source.read`, MCP `source_read`, pages `/quelle/multigpt` and `/quelle`), adopt a document
//! into the author's library ([`adopt`], only the human) and check highlights without a quote against the
//! full text ([`check_highlight`]).
//!
//! Contents of the remote side are data, not instructions: they enter pages only escaped (templates with
//! autoescape) and RPC/MCP as data. If MultiGPT is down, [`read`] names the reason honestly; if there is a
//! local copy ([`local_copy`]), it shows it with the notice „lokale Kopie vom …“. A check is never
//! invented: without a link or full text it is „nicht prüfbar“.

use baruch_library::{
    FulltextCheck, Highlight, HighlightCheck, NewTitle, RemoteRef, TitleEntry, check_range, highlight_check,
    set_highlight_check, set_title_remote, title_for_remote, title_remote,
};
use baruch_rag::RagError;
use serde::Serialize;

use crate::AppState;
use crate::connections::multigpt::{Limits, MultigptWay, local_part};
use crate::rpc::{INVALID_PARAMS, REFUSED, RpcError};

/// Way of the link (`library_title_remote.way`).
pub const WAY: &str = "multigpt";
/// A page marks at most this many passages.
const MAX_MARKS: usize = 200;

/// Local copy of a document from the sync (Round 13, „abgleich“).
#[derive(Debug, Clone, PartialEq)]
pub struct LocalCopy {
    pub title: String,
    pub url: Option<String>,
    pub text: String,
    pub fetched: String,
}

/// Local copy of a document if the sync created one ([`crate::sync::local_document`]).
/// Without a collection (link without `collection`) the copy whose document id matches applies if there is
/// exactly one; with several the choice would be a guess, then there is none.
pub async fn local_copy(state: &AppState, collection: &str, document: &str) -> Option<LocalCopy> {
    let document = document.trim();
    let mut collection = collection.trim().to_owned();
    if collection.is_empty() {
        let ids = state.store.sync_document_ids().await.ok()?;
        let mut matching = ids.into_iter().filter(|(_, id)| id == document).map(|(c, _)| c);
        collection = matching.next()?;
        if matching.next().is_some() {
            return None;
        }
    }
    let copy = crate::sync::local_document(state, &collection, document).await.ok()??;
    Some(LocalCopy { title: copy.title, url: copy.url, text: copy.text, fetched: copy.fetched })
}

/// Source of a part that was read.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SourceInfo {
    pub title: String,
    pub url: Option<String>,
    pub document: String,
    pub collection: String,
    /// Linked title of the library if there is one.
    pub title_id: Option<i64>,
}

/// A part that was read (answer of `source.read`, without the extra fields of the pages).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Reading {
    pub source: SourceInfo,
    pub part: usize,
    pub parts: Option<usize>,
    pub more: bool,
    pub text: String,
    /// Passages of `q` in the text of the part: `[start, end]` in Unicode characters from 0.
    pub marks: Vec<[usize; 2]>,
    /// `"multigpt"` or `"lokal"` (local copy, MultiGPT not reachable).
    pub origin: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Position of the first character in the full text if Baruch split it itself (otherwise `null`).
    pub offset: Option<usize>,
    pub paged_by_remote: bool,
}

/// What is to be read: a linked title or a document (with collection).
#[derive(Debug, Clone)]
pub enum Target {
    Title(i64),
    Document { document: String, collection: String },
}

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError::new(INVALID_PARAMS, message)
}

/// Check the document id: trimmed, 1–500 characters, one line without control characters.
pub fn checked_document(document: &str) -> Result<String, RpcError> {
    let document = document.trim();
    if document.is_empty() || document.chars().count() > 500 || document.chars().any(char::is_control) {
        return Err(invalid("document: Dokument-ID von MultiGPT (1–500 Zeichen, eine Zeile)."));
    }
    Ok(document.to_owned())
}

/// Check the collection: trimmed, at most 200 characters, one line (empty = unknown).
pub fn checked_collection(collection: Option<&str>) -> Result<String, RpcError> {
    let collection = collection.unwrap_or("").trim();
    if collection.chars().count() > 200 || collection.chars().any(char::is_control) {
        return Err(invalid("collection: höchstens 200 Zeichen in einer Zeile."));
    }
    Ok(collection.to_owned())
}

/// One character for the case-insensitive comparison (one to one, so that positions stay the same).
fn fold(c: char) -> char {
    c.to_lowercase().next().unwrap_or(c)
}

fn find_all(text: &[char], needle: &[char], out: &mut Vec<[usize; 2]>) {
    if needle.is_empty() || needle.len() > text.len() {
        return;
    }
    let mut i = 0;
    while i + needle.len() <= text.len() && out.len() < MAX_MARKS {
        if text[i..i + needle.len()] == *needle {
            out.push([i, i + needle.len()]);
            i += needle.len();
        } else {
            i += 1;
        }
    }
}

/// Round 23a (code review ki-1): maximum length of the search query `q` for marking in the full text (characters).
/// `source.read` (RPC, MCP `source_read`) refuses longer ones; [`marks`] shortens for all other callers.
pub const MAX_MARK_QUERY_CHARS: usize = 500;

/// Round 23a (code review ki-1): maximum number of different words that [`marks`] searches individually. Every
/// word without a hit costs one pass through the whole part text; without a limit a request made of millions of
/// words that do not occur would occupy a worker thread for minutes.
pub const MAX_MARK_WORDS: usize = 20;

/// Passages of `q` in `text` (case-insensitive), as character ranges: first the whole query;
/// if it does not occur, every word of three characters or more. Sorted, without overlap, at most 200. The query
/// counts up to [`MAX_MARK_QUERY_CHARS`] characters, at most [`MAX_MARK_WORDS`] different words are searched
/// individually.
pub fn marks(text: &str, q: Option<&str>) -> Vec<[usize; 2]> {
    let Some(q) = q.map(str::trim).filter(|q| !q.is_empty()) else { return Vec::new() };
    let q: String = q.chars().take(MAX_MARK_QUERY_CHARS).collect();
    let q = q.trim();
    let hay: Vec<char> = text.chars().map(fold).collect();
    let phrase: Vec<char> = q.chars().map(fold).collect();
    let mut found = Vec::new();
    find_all(&hay, &phrase, &mut found);
    if found.is_empty() {
        let mut words: Vec<Vec<char>> = Vec::new();
        for word in q.split(|c: char| !c.is_alphanumeric()).filter(|w| w.chars().count() >= 3) {
            let word: Vec<char> = word.chars().map(fold).collect();
            if !words.contains(&word) {
                words.push(word);
            }
            if words.len() == MAX_MARK_WORDS {
                break;
            }
        }
        for word in &words {
            find_all(&hay, word, &mut found);
        }
    }
    found.sort();
    let mut out: Vec<[usize; 2]> = Vec::new();
    for range in found {
        if out.last().is_none_or(|last| range[0] >= last[1]) {
            out.push(range);
        }
    }
    out.truncate(MAX_MARKS);
    out
}

/// A piece of text, possibly marked as a passage (for the templates).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Piece {
    pub text: String,
    pub mark: bool,
}

/// Split the text at the passages.
pub fn pieces(text: &str, marks: &[[usize; 2]]) -> Vec<Piece> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut cursor = 0;
    for [start, end] in marks.iter().copied() {
        if start < cursor || end > chars.len() || start >= end {
            continue;
        }
        if start > cursor {
            out.push(Piece { text: chars[cursor..start].iter().collect(), mark: false });
        }
        out.push(Piece { text: chars[start..end].iter().collect(), mark: true });
        cursor = end;
    }
    if cursor < chars.len() || out.is_empty() {
        out.push(Piece { text: chars[cursor..].iter().collect(), mark: false });
    }
    out
}

/// Error of the remote side as an RPC error: invalid input → -32602, otherwise -32000 with an honest sentence.
fn unavailable(e: &RagError) -> RpcError {
    match e {
        RagError::Invalid(m) => invalid(m.clone()),
        other => RpcError::new(REFUSED, format!("Der Volltext ist gerade nicht abrufbar: {}", other.message())),
    }
}

/// Read one part (`source.read`, `source_read`, pages). Title without a link → -32602.
pub async fn read(state: &AppState, target: &Target, part: usize, q: Option<&str>) -> Result<Reading, RpcError> {
    if part == 0 {
        return Err(invalid("part: Teile zählen ab 1."));
    }
    let pool = state.store.pool();
    let (document, collection, linked) = match target {
        Target::Title(id) => {
            let entry = baruch_library::get_title(pool, *id).await?;
            let Some(remote) = title_remote(pool, *id).await? else {
                // Round 21a: local full text of an imported source (without MultiGPT).
                if let Some(text) = baruch_library::title_text(pool, *id).await? {
                    return read_imported(state, &entry, &text, part, q).await;
                }
                return Err(invalid(format!(
                    "Titel {id} hat keinen Volltext: weder mit MultiGPT verknüpft noch als PDF/EPUB importiert."
                )));
            };
            (remote.document, remote.collection, Some(entry))
        }
        Target::Document { document, collection } => {
            let document = checked_document(document)?;
            let collection = checked_collection(Some(collection))?;
            let remote = RemoteRef { way: WAY.into(), collection: collection.clone(), document: document.clone() };
            let linked = match title_for_remote(pool, &remote).await? {
                Some(id) => Some(baruch_library::get_title(pool, id).await?),
                None => None,
            };
            (document, collection, linked)
        }
    };
    let title_id = linked.as_ref().map(|e| e.title.id);
    let linked_url = linked.as_ref().map(|e| e.title.url.clone()).filter(|u| crate::literature::safe_link(u).is_some());
    let fetched = match MultigptWay::from_settings(state, Limits::SEARCH).await {
        Ok(way) => way.read_part_with_info(&document, part).await,
        Err(e) => Err(e),
    };
    match fetched {
        Ok(reading) => {
            let title = linked
                .as_ref()
                .map(|e| e.title.title.clone())
                .or(reading.info.title)
                .unwrap_or_else(|| document.clone());
            let marks = marks(&reading.part.text, q);
            Ok(Reading {
                source: SourceInfo { title, url: linked_url.or(reading.info.url), document, collection, title_id },
                part: reading.part.part,
                parts: reading.part.parts,
                more: reading.part.more,
                text: reading.part.text,
                marks,
                origin: "multigpt",
                message: None,
                offset: reading.offset,
                paged_by_remote: reading.part.paged_by_remote,
            })
        }
        Err(RagError::Invalid(m)) => Err(invalid(m)),
        Err(e) => {
            let Some(copy) = local_copy(state, &collection, &document).await else { return Err(unavailable(&e)) };
            let (found, offset) = local_part(&document, &copy.text, part).map_err(|e| unavailable(&e))?;
            let marks = marks(&found.text, q);
            Ok(Reading {
                source: SourceInfo {
                    title: linked.as_ref().map(|e| e.title.title.clone()).unwrap_or(copy.title),
                    url: linked_url.or(copy.url.filter(|u| crate::literature::safe_link(u).is_some())),
                    document,
                    collection,
                    title_id,
                },
                part: found.part,
                parts: found.parts,
                more: found.more,
                text: found.text,
                marks,
                origin: "lokal",
                message: Some(format!(
                    "MultiGPT ist nicht erreichbar ({}); gezeigt wird die lokale Kopie vom {}.",
                    e.message().trim_end_matches('.'),
                    crate::sync::display_time(&copy.fetched)
                )),
                offset: Some(offset),
                paged_by_remote: false,
            })
        }
    }
}

/// Origin of a part from the local full text of an imported source (Round 21a).
pub const ORIGIN_IMPORT: &str = "import";

/// Part of the local full text of an imported source (`library_title_text`), split like a local copy
/// ([`local_part`]); the notice names file and date, never MultiGPT.
async fn read_imported(
    state: &AppState,
    entry: &TitleEntry,
    text: &baruch_library::TitleText,
    part: usize,
    q: Option<&str>,
) -> Result<Reading, RpcError> {
    let id = entry.title.id;
    let (found, offset) = local_part(&text.source_name, &text.text, part).map_err(|e| invalid(e.message()))?;
    let created = baruch_library::title_text_created(state.store.pool(), id).await?.unwrap_or_default();
    let marks = marks(&found.text, q);
    Ok(Reading {
        source: SourceInfo {
            title: entry.title.title.clone(),
            url: Some(entry.title.url.clone()).filter(|u| crate::literature::safe_link(u).is_some()),
            document: text.source_name.clone(),
            collection: String::new(),
            title_id: Some(id),
        },
        part: found.part,
        parts: found.parts,
        more: found.more,
        text: found.text,
        marks,
        origin: ORIGIN_IMPORT,
        message: Some(format!(
            "Volltext aus der importierten Datei „{}“ (lokal gelesen{}, ohne MultiGPT).",
            text.source_name,
            if created.is_empty() { String::new() } else { format!(" am {}", crate::localtime::minutes(&created)) }
        )),
        offset: Some(offset),
        paged_by_remote: false,
    })
}

/// Does the title have a full text (link with MultiGPT or local full text from an import, Round 21a)?
/// For the document view `/quelle` (`literature.rs`, `source_page`): instead of checking only `title_remote`, with
/// this function it also shows the local full text in parts and offers start/end in the full text.
pub async fn has_fulltext(state: &AppState, title_id: i64) -> Result<bool, RpcError> {
    let pool = state.store.pool();
    if title_remote(pool, title_id).await?.is_some() {
        return Ok(true);
    }
    Ok(baruch_library::title_text(pool, title_id).await?.is_some())
}

/// Citation key from a title: `mg-` and lower-case letters/digits/hyphens (at most 40 characters).
fn key_base(title: &str) -> String {
    let mut slug = String::new();
    for c in title.chars().flat_map(char::to_lowercase) {
        let c = match c {
            'ä' => "ae".to_owned(),
            'ö' => "oe".to_owned(),
            'ü' => "ue".to_owned(),
            'ß' => "ss".to_owned(),
            c if c.is_ascii_alphanumeric() => c.to_string(),
            _ => "-".to_owned(),
        };
        if c == "-" && (slug.is_empty() || slug.ends_with('-')) {
            continue;
        }
        slug.push_str(&c);
        if slug.len() >= 36 {
            break;
        }
    }
    let slug = slug.trim_end_matches('-');
    if slug.is_empty() { "mg-dokument".to_owned() } else { format!("mg-{slug}") }
}

/// Adopt a document from MultiGPT into the author's library (`source.adopt`; only the human through RPC and
/// form, never through MCP): title with details from `document_info` (if they are missing or MultiGPT is not
/// reachable, the document id), link in `library_title_remote`, afterwards the library reports to the
/// index (`rag::notify(state, None)`). If the document is already adopted, it stays with the one title.
pub async fn adopt(
    state: &AppState,
    document: &str,
    collection: Option<&str>,
) -> Result<(TitleEntry, RemoteRef), RpcError> {
    let document = checked_document(document)?;
    let collection = checked_collection(collection)?;
    let remote = RemoteRef { way: WAY.into(), collection: collection.clone(), document: document.clone() };
    let pool = state.store.pool();
    if let Some(id) = title_for_remote(pool, &remote).await? {
        let remote = title_remote(pool, id).await?.unwrap_or(remote);
        return Ok((baruch_library::get_title(pool, id).await?, remote));
    }
    let info = match MultigptWay::from_settings(state, Limits::SEARCH).await {
        Ok(way) => way.info(&document).await.unwrap_or_default(),
        Err(_) => Default::default(),
    };
    let title = info.title.unwrap_or_else(|| document.clone());
    let note = if collection.is_empty() {
        format!("Aus MultiGPT übernommen (Dokument {document}).")
    } else {
        format!("Aus MultiGPT übernommen (Sammlung „{collection}“, Dokument {document}).")
    };
    let id = {
        let _held = state.write_lock.lock().await;
        if let Some(id) = title_for_remote(pool, &remote).await? {
            id
        } else {
            let base = key_base(&title);
            let mut key = base.clone();
            let mut n = 2;
            loop {
                let taken: Option<i64> = sqlx::query_scalar("SELECT id FROM library_title WHERE key = ?1")
                    .bind(&key)
                    .fetch_optional(pool)
                    .await
                    .map_err(|e| RpcError::new(crate::rpc::INTERNAL, format!("Bibliothek: {e}")))?;
                if taken.is_none() {
                    break;
                }
                key = format!("{base}-{n}");
                n += 1;
            }
            let new = NewTitle {
                key,
                kind: "other".into(),
                title,
                authors: String::new(),
                year: None,
                publisher: String::new(),
                place: String::new(),
                url: info.url.unwrap_or_default(),
                note,
            };
            let id = baruch_library::add_title(pool, &new).await?;
            set_title_remote(pool, id, &remote).await?;
            id
        }
    };
    crate::rag::notify(state, None);
    Ok((baruch_library::get_title(pool, id).await?, remote))
}

/// Can the highlight be checked (without a quote, with a character range)?
pub fn checkable(h: &Highlight) -> bool {
    h.quote_id.is_none() && h.start.is_some() && h.end.is_some()
}

/// Check a highlight without a quote against the full text of its source and store the result: found or
/// not found only with a full text (MultiGPT, otherwise the local copy, then with a notice), otherwise „nicht
/// prüfbar“ with a reason. Highlights that cannot be checked (with a quote or without a range) → `None`, nothing
/// stored.
pub async fn check_highlight(state: &AppState, h: &Highlight) -> Result<Option<HighlightCheck>, RpcError> {
    let (Some(start), Some(end), None) = (h.start, h.end, h.quote_id) else { return Ok(None) };
    let pool = state.store.pool();
    let (result, message) = match title_remote(pool, h.title_id).await? {
        None => match baruch_library::title_text(pool, h.title_id).await? {
            // Round 21a: local full text of an imported source.
            Some(text) => (
                check_range(&text.text, start, end, &h.text),
                format!("Geprüft gegen den lokalen Volltext aus „{}“.", text.source_name),
            ),
            None => (
                FulltextCheck::Unverifiable,
                "Die Quelle ist nicht mit MultiGPT verknüpft und hat keinen importierten Volltext; ohne Volltext lässt \
                 sich der Bereich nicht prüfen."
                    .to_owned(),
            ),
        },
        Some(remote) => {
            let full = match MultigptWay::from_settings(state, Limits::SEARCH).await {
                Ok(way) => way.read_full(&remote.document).await,
                Err(e) => Err(e),
            };
            match full {
                Ok(full) => (check_range(&full, start, end, &h.text), String::new()),
                Err(e) => match local_copy(state, &remote.collection, &remote.document).await {
                    Some(copy) => (
                        check_range(&copy.text, start, end, &h.text),
                        format!(
                            "Geprüft gegen die lokale Kopie vom {} (MultiGPT nicht erreichbar).",
                            crate::sync::display_time(&copy.fetched)
                        ),
                    ),
                    None => (FulltextCheck::Unverifiable, format!("Volltext nicht abrufbar: {}", e.message())),
                },
            }
        }
    };
    set_highlight_check(pool, h.id, result, &message).await?;
    Ok(highlight_check(pool, h.id).await?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marks_find_the_phrase_or_else_its_words_without_case() {
        let text = "Das Licht kreist. Über dem Wasser kreist das LICHT.";
        assert_eq!(
            marks(text, Some("licht")),
            vec![[4, 9], [45, 50]],
            "ohne Groß-/Kleinschreibung, Zeichenbereiche (Ü zählt als ein Zeichen)"
        );
        assert_eq!(
            marks(text, Some("Wasser Licht")),
            vec![[4, 9], [27, 33], [45, 50]],
            "kommt die ganze Anfrage nicht vor, zählen ihre Wörter ab drei Zeichen"
        );
        assert!(marks(text, Some("  ")).is_empty() && marks(text, None).is_empty(), "ohne Anfrage keine Marken");
        let parts = pieces(text, &marks(text, Some("licht")));
        assert_eq!(parts.iter().filter(|p| p.mark).count(), 2, "zwei markierte Stücke: {parts:?}");
        assert_eq!(parts.iter().map(|p| p.text.as_str()).collect::<String>(), text, "Stücke ergeben den Text");
    }

    #[test]
    fn keys_for_adopted_documents_are_plain_ascii() {
        assert_eq!(key_base("Möwen über dem Kai!"), "mg-moewen-ueber-dem-kai", "Umlaute umschrieben, Satzzeichen weg");
        assert_eq!(key_base("¿?"), "mg-dokument", "ohne Buchstaben ein fester Name");
        assert!(key_base(&"Leuchtturm ".repeat(20)).len() <= 40, "kurz genug für einen Zitierschlüssel");
    }
}
