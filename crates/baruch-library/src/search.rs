// SPDX-License-Identifier: AGPL-3.0-or-later
//! Volltextsuche über Werk (Revisionen, Beiträge) und Bibliothek (Titel, Zitate, Hervorhebungen).

use sqlx::SqlitePool;

use crate::LibraryError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HitKind {
    Revision,
    Post,
    Title,
    Quote,
    Highlight,
}

impl HitKind {
    fn as_str(self) -> &'static str {
        match self {
            HitKind::Revision => "revision",
            HitKind::Post => "post",
            HitKind::Title => "title",
            HitKind::Quote => "quote",
            HitKind::Highlight => "highlight",
        }
    }

    fn parse(s: &str) -> Option<HitKind> {
        Some(match s {
            "revision" => HitKind::Revision,
            "post" => HitKind::Post,
            "title" => HitKind::Title,
            "quote" => HitKind::Quote,
            "highlight" => HitKind::Highlight,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct SearchOptions {
    /// `None` = alle Arten.
    pub kinds: Option<Vec<HitKind>>,
    /// Werk-Treffer nur aus diesem Werk; Bibliothekstreffer immer.
    pub work: Option<i64>,
    /// `false` = nur Revisionen mit Status `current`.
    #[serde(default)]
    pub all_revisions: bool,
    /// Vorgabe 20, höchstens 100.
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchHit {
    pub kind: HitKind,
    pub work: Option<i64>,
    /// seq des Abschnitts (revision, post); vergibt jede Gliederungsänderung neu
    pub node: Option<usize>,
    /// Fester Schlüssel des Abschnitts (`node.id` = `Node::key`; revision, post): bleibt beim
    /// Umstellen der Gliederung gleich, darum tragen Links ihn (`/?work=W&key=K`, Runde 8)
    pub node_key: Option<i64>,
    /// revision.number (revision)
    pub rev: Option<u32>,
    /// title, quote, highlight
    pub title_id: Option<i64>,
    /// quote (das Zitat selbst), highlight (das Zitat, in dem sie liegt, falls eines)
    pub quote_id: Option<i64>,
    /// highlight
    pub highlight_id: Option<i64>,
    /// Zitierschlüssel (title, quote, highlight)
    pub key: Option<String>,
    /// „2.1 Erste Anzeichen“ bzw. „Autor: Titel (Jahr)“
    pub label: String,
    /// Ausschnitt als HTML: Text maskiert (`&` `<` `>` `"` `'`), Treffer in `<mark>…</mark>`.
    /// Typst-Klammern `[` `]` bleiben, wie sie im Text stehen.
    pub snippet: String,
    /// bm25, kleiner = besser
    pub rank: f64,
}

/// Marken für Trefferanfang und -ende im SQL-`snippet()` (Zeichen aus dem privaten Bereich,
/// U+E000 und U+E001), die [`snippet_html`] in `<mark>` und `</mark>` übersetzt.
const MARK_OPEN: char = '\u{E000}';
const MARK_CLOSE: char = '\u{E001}';

pub const DEFAULT_LIMIT: u32 = 20;
pub const MAX_LIMIT: u32 = 100;

/// Nutzertext → sichere FTS5-Abfrage: jedes Wort in `"…"` (innere `"` verdoppelt), durch
/// Leerzeichen verbunden (UND). Operatoren wie `OR`, `NOT`, `(`, `*`, `:` verlieren so ihre
/// Bedeutung. NUL und andere Steuerzeichen trennen Wörter wie ein Leerzeichen (FTS5 bricht an
/// NUL sonst die Abfrage ab). Wörter ohne Buchstaben oder Ziffern fallen weg; bleibt nichts
/// übrig → `None`.
pub fn fts_query(input: &str) -> Option<String> {
    let cleaned: String = input.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    let words: Vec<String> = cleaned
        .split_whitespace()
        .filter(|w| w.chars().any(char::is_alphanumeric))
        .map(|w| format!("\"{}\"", w.replace('"', "\"\"")))
        .collect();
    if words.is_empty() { None } else { Some(words.join(" ")) }
}

#[derive(sqlx::FromRow)]
struct Row {
    kind: String,
    work: Option<i64>,
    node_seq: Option<i64>,
    node_key: Option<i64>,
    node_number: Option<String>,
    node_title: Option<String>,
    rev: Option<i64>,
    title_id: Option<i64>,
    quote_id: Option<i64>,
    highlight_id: Option<i64>,
    locator: Option<String>,
    key: Option<String>,
    lib_title: Option<String>,
    authors: Option<String>,
    year: Option<i64>,
    page: Option<String>,
    snippet: String,
    score: f64,
}

/// Werktreffer ohne lebenden Abschnitt (verwaiste Indexzeilen, wie vor Migration 0105 möglich) fallen weg.
const SEARCH_SQL: &str = "
SELECT search_fts.kind AS kind,
       node.work_id AS work,
       node.seq AS node_seq,
       node.id AS node_key,
       node.number AS node_number,
       node.title AS node_title,
       revision.number AS rev,
       library_title.id AS title_id,
       library_quote.id AS quote_id,
       library_highlight.id AS highlight_id,
       library_highlight.locator AS locator,
       library_title.key AS key,
       library_title.title AS lib_title,
       library_title.authors AS authors,
       library_title.year AS year,
       library_quote.page AS page,
       snippet(search_fts, 4, char(57344), char(57345), '…', 12) AS snippet,
       bm25(search_fts) AS score
  FROM search_fts
  LEFT JOIN revision ON search_fts.kind = 'revision' AND revision.id = search_fts.row_id
  LEFT JOIN post ON search_fts.kind = 'post' AND post.id = search_fts.row_id
  LEFT JOIN node ON node.id = coalesce(revision.node_id, post.node_id)
  LEFT JOIN library_highlight ON search_fts.kind = 'highlight' AND library_highlight.id = search_fts.row_id
  LEFT JOIN library_quote ON library_quote.id =
       CASE search_fts.kind WHEN 'quote' THEN search_fts.row_id WHEN 'highlight' THEN library_highlight.quote_id END
  LEFT JOIN library_title ON library_title.id =
       CASE search_fts.kind WHEN 'title' THEN search_fts.row_id WHEN 'quote' THEN library_quote.title_id
                            WHEN 'highlight' THEN library_highlight.title_id END
 WHERE search_fts MATCH ?1
   AND instr(?2, ',' || search_fts.kind || ',') > 0
   AND (search_fts.kind NOT IN ('revision', 'post') OR node.id IS NOT NULL)
   AND (search_fts.kind IN ('title', 'quote', 'highlight') OR ?3 IS NULL OR node.work_id = ?3)
   AND (search_fts.kind <> 'revision' OR ?4 OR revision.status = 'current')
   AND (search_fts.kind <> 'highlight' OR library_highlight.status IN ('active', 'proposed'))
 ORDER BY score, search_fts.kind, search_fts.row_id
 LIMIT ?5";

/// Sucht im Werk und in der Bibliothek. Leere Eingabe → [`LibraryError::EmptyQuery`].
pub async fn search(pool: &SqlitePool, query: &str, options: &SearchOptions) -> Result<Vec<SearchHit>, LibraryError> {
    let fts = fts_query(query).ok_or(LibraryError::EmptyQuery)?;
    let kinds = match &options.kinds {
        None => ",revision,post,title,quote,highlight,".to_owned(),
        Some(kinds) if kinds.is_empty() => return Ok(Vec::new()),
        Some(kinds) => {
            let mut s = String::from(",");
            for kind in kinds {
                s.push_str(kind.as_str());
                s.push(',');
            }
            s
        }
    };
    let limit = options.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let rows: Vec<Row> = sqlx::query_as(SEARCH_SQL)
        .bind(fts)
        .bind(kinds)
        .bind(options.work)
        .bind(options.all_revisions)
        .bind(i64::from(limit))
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().filter_map(hit).collect())
}

fn hit(row: Row) -> Option<SearchHit> {
    let kind = HitKind::parse(&row.kind)?;
    let label = match kind {
        HitKind::Revision | HitKind::Post => {
            let number = row.node_number.unwrap_or_default();
            let title = row.node_title.unwrap_or_default();
            if number.is_empty() { title } else { format!("{number} {title}") }
        }
        HitKind::Title | HitKind::Quote | HitKind::Highlight => {
            let mut label = String::new();
            let authors = row.authors.unwrap_or_default();
            if !authors.is_empty() {
                label.push_str(&authors);
                label.push_str(": ");
            }
            label.push_str(row.lib_title.as_deref().unwrap_or_default());
            if let Some(year) = row.year {
                label.push_str(&format!(" ({year})"));
            }
            // Fundstelle: bei Hervorhebungen ihre eigene, sonst die Seite des Zitats.
            let page = row.locator.filter(|l| !l.is_empty()).or(row.page);
            if let Some(page) = page.filter(|p| !p.is_empty()) {
                label.push_str(", ");
                label.push_str(&page);
            }
            label
        }
    };
    Some(SearchHit {
        kind,
        work: row.work,
        node: row.node_seq.and_then(|s| usize::try_from(s).ok()),
        node_key: row.node_key,
        rev: row.rev.and_then(|r| u32::try_from(r).ok()),
        title_id: row.title_id,
        quote_id: row.quote_id,
        highlight_id: row.highlight_id,
        key: row.key,
        label,
        snippet: snippet_html(&row.snippet),
        rank: row.score,
    })
}

/// Roh-Ausschnitt mit den Marken U+E000/U+E001 → HTML. Text wird maskiert, Marken werden zu
/// `<mark>`/`</mark>`. Die Ausgabe ist immer ausgeglichen: Eine Marke, die im Text selbst steht
/// (falsche Reihenfolge, doppelt), fällt weg, ein offenes `<mark>` wird am Ende geschlossen.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snippet_html_escapes_text_and_marks_hits() {
        let raw = "<script>\u{E000}a&b\u{E001} \"x\" 'y' #quote[z]";
        assert_eq!(
            snippet_html(raw),
            "&lt;script&gt;<mark>a&amp;b</mark> &quot;x&quot; &#39;y&#39; #quote[z]",
            "Text maskiert, Marken als <mark>, Typst-Klammern unverändert"
        );
    }

    #[test]
    fn snippet_html_stays_balanced_with_stray_marks() {
        assert_eq!(
            snippet_html("\u{E001}a\u{E000}b\u{E000}c"),
            "a<mark>bc</mark>",
            "verirrte Marken fallen weg, offenes <mark> wird geschlossen"
        );
        assert_eq!(snippet_html("ohne"), "ohne", "ohne Marken bleibt der Text, wie er ist");
    }

    #[test]
    fn fts_query_drops_nul_and_control_characters() {
        assert_eq!(fts_query("\0"), None, "nur NUL ergibt keine Abfrage");
        assert_eq!(fts_query("\u{1}\u{7f}\u{9b}"), None, "nur Steuerzeichen ergeben keine Abfrage");
        assert_eq!(fts_query("lang\0mut"), Some("\"lang\" \"mut\"".to_owned()), "NUL trennt wie ein Leerzeichen");
        assert_eq!(fts_query("a\u{1b}[b"), Some("\"a\" \"[b\"".to_owned()), "ESC fällt weg");
    }
}
