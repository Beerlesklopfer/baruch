// SPDX-License-Identifier: AGPL-3.0-or-later
//! Suchen und Abrufen ([`research`]): erst bis zu drei Aufrufe `web_search` mit „site:<Rechner>“ für die
//! bevorzugten Quellen (in ihrer Reihenfolge, gleiche Rechner nur einmal), dann ein allgemeiner Aufruf
//! (entfällt, wenn schon [`super::MAX_HITS`] Treffer da sind). Danach werden die ersten
//! [`super::MAX_FETCHES`] Treffer über `fetch_url` abgerufen ([`super::fetch`]). Alles unter
//! [`super::TOTAL_TIMEOUT`]; was bis dahin abgerufen ist, bleibt erhalten.
//!
//! Die Argumente leiten [`search_arguments`] und [`super::fetch::fetch_arguments`] aus dem `inputSchema` ab
//! (Anfrage über [`QUERY_KEYS`], Trefferzahl über [`LIMIT_KEYS`]); verlangt das Schema einen Parameter, den
//! Baruch nicht kennt, ist das ehrlich [`RagError::Schema`]. Treffer liest [`parse_hits`] aus
//! `structuredContent` bzw. JSON in `content[].text`, ersatzweise aus Markdown-Links und Adressen im Text.

use baruch_rag::RagError;
use baruch_store::PreferredSource;
use serde_json::{Map, Value, json};

use super::{
    FETCH_LIMITS, FETCH_URL_TOOL, MAX_FETCHES, MAX_HITS, MAX_PREFERRED_CALLS, QUERY_CHARS, RESULT_LIST_KEYS, Research,
    SEARCH_LIMITS, SNIPPET_KEYS, TITLE_KEYS, TOTAL_TIMEOUT, URL_KEYS, WEB_SEARCH_TOOL, WebHit, WebPage,
};
use crate::AppState;
use crate::connections::multigpt::{
    LIMIT_KEYS, Limits, QUERY_KEYS, Session, clean_line, content_text, tool_payload, web_url,
};

/// Rechner einer http(s)-Adresse, klein, ohne `www.`; sonst `None`.
pub fn host_of(url: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(url.trim()).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    let host = parsed.host_str()?.trim_end_matches('.').to_ascii_lowercase();
    let host = host.strip_prefix("www.").unwrap_or(&host).to_owned();
    (!host.is_empty()).then_some(host)
}

/// Passt der Rechner `host` zu einer bevorzugten Quelle `source` (gleich oder Unterrechner)?
pub fn host_matches(host: &str, source: &str) -> bool {
    host == source || host.strip_suffix(source).is_some_and(|rest| rest.ends_with('.'))
}

/// Rechner der bevorzugten Quellen in ihrer Reihenfolge, ohne Doppelte.
pub fn source_hosts(sources: &[PreferredSource]) -> Vec<String> {
    let mut hosts: Vec<String> = Vec::new();
    for source in sources {
        if let Some(host) = host_of(&source.url)
            && !hosts.contains(&host)
        {
            hosts.push(host);
        }
    }
    hosts
}

/// Suchanfrage: einzeilig, ohne Steuerzeichen, höchstens [`QUERY_CHARS`] Zeichen.
pub fn query_line(text: &str) -> String {
    clean_line(text, QUERY_CHARS).trim_end_matches('…').trim().to_owned()
}

/// Argumente für `web_search` aus dessen `inputSchema`. Ohne `properties` nur `query`.
pub fn search_arguments(schema: &Value, query: &str, limit: usize) -> Result<Value, RagError> {
    let empty = Map::new();
    let props = schema.get("properties").and_then(Value::as_object).unwrap_or(&empty);
    if props.is_empty() {
        return Ok(json!({ "query": query }));
    }
    let pick = |keys: &[&'static str]| keys.iter().find(|k| props.contains_key(**k)).copied();
    let query_key = pick(QUERY_KEYS).ok_or_else(|| {
        RagError::Schema(format!(
            "{WEB_SEARCH_TOOL} nennt im inputSchema keinen Parameter für die Anfrage ({}).",
            QUERY_KEYS.join(", ")
        ))
    })?;
    let limit_key = pick(LIMIT_KEYS);
    check_required(schema, WEB_SEARCH_TOOL, &[Some(query_key), limit_key])?;
    let mut args = Map::new();
    args.insert(query_key.to_owned(), json!(query));
    if let Some(key) = limit_key {
        let spec = &props[key];
        let mut n = limit as u64;
        if let Some(max) = spec.get("maximum").and_then(Value::as_u64) {
            n = n.min(max);
        }
        if let Some(min) = spec.get("minimum").and_then(Value::as_u64) {
            n = n.max(min);
        }
        args.insert(key.to_owned(), json!(n));
    }
    Ok(Value::Object(args))
}

/// Verlangt das Schema einen Parameter außer `known`? Dann ehrlich [`RagError::Schema`].
pub(crate) fn check_required(schema: &Value, tool: &str, known: &[Option<&str>]) -> Result<(), RagError> {
    let required = schema.get("required").and_then(Value::as_array).cloned().unwrap_or_default();
    if let Some(unknown) = required.iter().filter_map(Value::as_str).find(|r| !known.contains(&Some(*r))) {
        return Err(RagError::Schema(format!("{tool} verlangt den Parameter „{unknown}“, den Baruch nicht kennt.")));
    }
    Ok(())
}

/// Erster Text unter einem der Schlüssel (auch in `metadata`).
pub(crate) fn text_of(item: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    let mut places = vec![item];
    if let Some(Value::Object(inner)) = item.get("metadata") {
        places.push(inner);
    }
    places.into_iter().find_map(|place| {
        keys.iter().find_map(|k| match place.get(*k) {
            Some(Value::String(s)) if !s.trim().is_empty() => Some(s.trim().to_owned()),
            _ => None,
        })
    })
}

/// Liste in den Daten: die Daten selbst oder unter einem der Schlüssel (auch eine Ebene tiefer).
fn list_in(payload: &Value, depth: usize) -> Option<&Vec<Value>> {
    match payload {
        Value::Array(list) => Some(list),
        Value::Object(map) => {
            RESULT_LIST_KEYS.iter().filter_map(|k| map.get(*k)).find_map(Value::as_array).or_else(|| {
                (depth > 0).then(|| map.values().filter(|v| v.is_object()).find_map(|v| list_in(v, depth - 1)))?
            })
        }
        _ => None,
    }
}

/// Adresse am Anfang von `text` bis zu Leerraum oder einem Zeichen, das in Text sie beendet; Satzzeichen am
/// Ende gehören nicht dazu.
pub(crate) fn url_at(text: &str) -> &str {
    let end = text
        .find(|c: char| {
            c.is_whitespace() || matches!(c, ')' | ']' | '>' | '<' | '"' | '“' | '”' | '„' | '«' | '»' | '\'')
        })
        .unwrap_or(text.len());
    text[..end].trim_end_matches(['.', ',', ';', ':', '!', '?'])
}

/// Treffer aus Text: Markdown-Links `[Titel](Adresse)` und sonst freie Adressen.
fn hits_from_text(text: &str) -> Vec<WebHit> {
    let mut hits: Vec<WebHit> = Vec::new();
    for line in text.lines() {
        let mut rest = line;
        while let Some(at) = rest.find("http") {
            let url = url_at(&rest[at..]);
            let before = &rest[..at];
            let title = before
                .strip_suffix("](")
                .and_then(|b| b.rfind('[').map(|open| b[open + 1..].to_owned()))
                .unwrap_or_default();
            if let Some(url) = web_url(Some(url.to_owned()))
                && !hits.iter().any(|h| h.url == url)
            {
                let title = clean_line(if title.trim().is_empty() { &url } else { &title }, 200);
                hits.push(WebHit { url, title, snippet: String::new(), preferred: false });
            }
            rest = &rest[at + url.len().max(4)..];
        }
    }
    hits
}

/// Treffer aus einer Antwort von `web_search` (nur http(s)-Adressen, ohne Doppelte). Weder Liste noch
/// Adresse noch Text → [`RagError::Schema`].
pub fn parse_hits(result: &Value) -> Result<Vec<WebHit>, RagError> {
    if let Some(list) = tool_payload(result).as_ref().and_then(|p| list_in(p, 1).cloned()) {
        let mut hits: Vec<WebHit> = Vec::new();
        for item in &list {
            let (url, title, snippet) = match item {
                Value::String(s) => (web_url(Some(s.trim().to_owned())), None, None),
                Value::Object(map) => {
                    (web_url(text_of(map, URL_KEYS)), text_of(map, TITLE_KEYS), text_of(map, SNIPPET_KEYS))
                }
                _ => continue,
            };
            let Some(url) = url else { continue };
            if hits.iter().any(|h| h.url == url) {
                continue;
            }
            let title = clean_line(title.as_deref().unwrap_or(&url), 200);
            let snippet = clean_line(snippet.as_deref().unwrap_or(""), 300);
            hits.push(WebHit { url, title, snippet, preferred: false });
        }
        return Ok(hits);
    }
    let text = content_text(result);
    if text.trim().is_empty() {
        return Err(RagError::Schema(format!(
            "{WEB_SEARCH_TOOL}: Antwort nicht lesbar (weder Trefferliste noch Text)."
        )));
    }
    Ok(hits_from_text(&text))
}

/// Treffer anhängen (ohne Doppelte, höchstens [`MAX_HITS`]); gibt zurück, ob noch Platz ist.
fn add_hits(research: &mut Research, found: Vec<WebHit>, hosts: &[String]) -> bool {
    for mut hit in found {
        if research.hits.len() >= MAX_HITS {
            break;
        }
        if research.hits.iter().any(|h| h.url == hit.url) {
            continue;
        }
        hit.preferred = host_of(&hit.url).is_some_and(|h| hosts.iter().any(|s| host_matches(&h, s)));
        research.hits.push(hit);
    }
    research.hits.len() < MAX_HITS
}

/// Kurzform eines Fehlers für einen Hinweis (eine Zeile, ohne Token – das hat die Sitzung schon entfernt).
pub(crate) fn reason(e: &RagError) -> String {
    clean_line(e.message(), 200)
}

/// Websuche (siehe Moduldoku). Scheitert sie als Ganzes, steht der Grund in [`Research::failure`];
/// abgerufene Seiten bleiben auch nach dem Zeitlimit erhalten.
pub async fn research(state: &AppState, query: &str) -> Research {
    let mut out = Research { query: query_line(query), ..Research::default() };
    if out.query.is_empty() {
        out.failure = Some("keine Suchanfrage (Frage und Abschnitt sind leer)".into());
        return out;
    }
    let sources = match state.store.sources_list().await {
        Ok(list) => list,
        Err(e) => {
            out.notes.push(format!("Hinweis: bevorzugte Quellen nicht lesbar ({})", clean_line(&e.to_string(), 200)));
            Vec::new()
        }
    };
    let hosts = source_hosts(&sources);
    match tokio::time::timeout(TOTAL_TIMEOUT, run(state, &hosts, &mut out)).await {
        Ok(Ok(())) => {}
        Ok(Err(e)) => out.failure = Some(reason(&e)),
        Err(_) => {
            out.failure = Some(format!(
                "Zeitlimit von {} s für die ganze Websuche erreicht; {} Seite(n) bis dahin abgerufen",
                TOTAL_TIMEOUT.as_secs(),
                out.pages.len()
            ));
        }
    }
    out
}

/// Der eigentliche Ablauf; schreibt Treffer, Seiten und Hinweise sofort nach `out`.
async fn run(state: &AppState, hosts: &[String], out: &mut Research) -> Result<(), RagError> {
    let opened = Limits { timeout: SEARCH_LIMITS.timeout, max_body: FETCH_LIMITS.max_body };
    let mut session = Session::open(state, opened).await?;
    let exceptions = super::fetch::fetch_exceptions(state);
    let found = gather(&mut session, hosts, &exceptions, out).await;
    match found {
        Ok(()) => session.release().await,
        Err(e) => {
            session.close().await;
            return Err(e);
        }
    }
    Ok(())
}

/// `exceptions`: Rechner, die der Filter [`super::fetch::fetch_allowed`] durchlässt (nur in Tests gesetzt).
async fn gather(
    session: &mut Session,
    hosts: &[String],
    exceptions: &[String],
    out: &mut Research,
) -> Result<(), RagError> {
    let search_schema = session.schema(WEB_SEARCH_TOOL).ok_or_else(|| {
        RagError::Schema(format!("MultiGPT bietet das Werkzeug {WEB_SEARCH_TOOL} nicht an (tools/list)."))
    })?;
    let fetch_schema = session.schema(FETCH_URL_TOOL).ok_or_else(|| {
        RagError::Schema(format!("MultiGPT bietet das Werkzeug {FETCH_URL_TOOL} nicht an (tools/list)."))
    })?;
    let query = out.query.clone();
    let mut room = true;
    for host in hosts.iter().take(MAX_PREFERRED_CALLS) {
        if !room {
            break;
        }
        let site = format!("site:{host} {query}");
        let args = search_arguments(&search_schema, &site, MAX_HITS)?;
        match session.call(WEB_SEARCH_TOOL, args, Some(SEARCH_LIMITS)).await.and_then(|r| parse_hits(&r)) {
            Ok(found) => room = add_hits(out, found, hosts),
            Err(e) => out.notes.push(format!("Hinweis: Suche „site:{host}“ ausgefallen ({})", reason(&e))),
        }
    }
    if room {
        let args = search_arguments(&search_schema, &query, MAX_HITS)?;
        match session.call(WEB_SEARCH_TOOL, args, Some(SEARCH_LIMITS)).await.and_then(|r| parse_hits(&r)) {
            Ok(found) => {
                add_hits(out, found, hosts);
            }
            Err(e) => out.notes.push(format!("Hinweis: allgemeine Suche ausgefallen ({})", reason(&e))),
        }
    }
    // Runde 23: Adressen, die nach ihrem Wortlaut ins eigene Netz zeigen, gehen nicht an `fetch_url`
    // (MultiGPT riefe sie im Intranet ab). Der Treffer bleibt ein Treffer, nur ohne Abruf und mit Hinweis;
    // er verbraucht keinen der Abrufe.
    let mut wanted: Vec<WebHit> = Vec::new();
    for hit in &out.hits {
        match super::fetch::fetch_allowed_with(&hit.url, exceptions) {
            Ok(()) if wanted.len() < MAX_FETCHES => wanted.push(hit.clone()),
            Ok(()) => {}
            Err(why) => out.notes.push(format!("Hinweis: {} nicht abgerufen ({why})", clean_line(&hit.url, 300))),
        }
    }
    for hit in wanted {
        let args = super::fetch::fetch_arguments(&fetch_schema, &hit.url)?;
        let page = session
            .call(FETCH_URL_TOOL, args, Some(FETCH_LIMITS))
            .await
            .and_then(|r| super::fetch::parse_page(&r, &hit.url));
        match page {
            Ok(fetched) => {
                let (accessed, day) = super::now_stamp();
                let label = out.pages.len() + 1;
                let title = if fetched.title.is_empty() { hit.title.clone() } else { fetched.title };
                out.pages.push(WebPage {
                    label,
                    url: hit.url.clone(),
                    title: session.scrub(&title),
                    preferred: hit.preferred,
                    text: session.scrub(&fetched.text),
                    accessed,
                    day,
                });
            }
            Err(e) => out.notes.push(format!("Hinweis: {} übersprungen ({})", clean_line(&hit.url, 300), reason(&e))),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(url: &str) -> PreferredSource {
        PreferredSource {
            id: 1,
            title: "t".into(),
            url: url.into(),
            note: String::new(),
            position: 1,
            created: String::new(),
            updated: String::new(),
        }
    }

    #[test]
    fn preferred_sources_match_by_host_and_subdomain_only() {
        let hosts = source_hosts(&[
            source("https://www.Lichtkunde.example/start"),
            source("http://lichtkunde.example/anders"),
            source("https://archiv.example/"),
        ]);
        assert_eq!(
            hosts,
            vec!["lichtkunde.example", "archiv.example"],
            "klein, ohne www., ohne Doppelte, in Reihenfolge"
        );
        assert!(host_matches("lichtkunde.example", "lichtkunde.example"), "gleicher Rechner");
        assert!(host_matches("blog.lichtkunde.example", "lichtkunde.example"), "Unterrechner zählt");
        // Begründung: Ein Rechner, der nur so endet, ist eine fremde Seite (Täuschung über den Namen).
        assert!(!host_matches("falschlichtkunde.example", "lichtkunde.example"), "kein bloßes Endstück");
        assert_eq!(host_of("javascript:alert(1)"), None, "nur http(s)");
    }

    #[test]
    fn search_arguments_follow_the_schema_and_refuse_unknown_requirements() {
        assert_eq!(
            search_arguments(&json!({}), "Licht", 8).unwrap(),
            json!({ "query": "Licht" }),
            "ohne Schema nur query"
        );
        let schema = json!({ "properties": { "q": { "type": "string" },
            "max_results": { "type": "integer", "maximum": 5 }, "lang": { "type": "string" } }, "required": ["q"] });
        assert_eq!(
            search_arguments(&schema, "Licht", 8).unwrap(),
            json!({ "q": "Licht", "max_results": 5 }),
            "Name der Anfrage aus dem Schema, Trefferzahl an maximum geklemmt, freiwillige Parameter bleiben weg"
        );
        let strict = json!({ "properties": { "query": {}, "engine": {} }, "required": ["query", "engine"] });
        assert_eq!(search_arguments(&strict, "x", 8).unwrap_err().code(), "schema", "unbekannte Pflicht → Schema");
        let none = json!({ "properties": { "frage": {} } });
        assert_eq!(search_arguments(&none, "x", 8).unwrap_err().code(), "schema", "keine Anfrage → Schema");
    }

    #[test]
    fn hits_come_from_structured_lists_or_from_links_in_text() {
        let structured = json!({ "structuredContent": { "results": [
            { "title": "Lichtkunde", "url": "https://lichtkunde.example/a", "content": "Über Licht" },
            { "title": "Doppelt", "url": "https://lichtkunde.example/a" },
            { "title": "Böse", "url": "javascript:alert(1)" },
            "https://archiv.example/b",
        ] } });
        let hits = parse_hits(&structured).unwrap();
        assert_eq!(
            hits.iter().map(|h| (h.url.as_str(), h.title.as_str(), h.snippet.as_str())).collect::<Vec<_>>(),
            vec![
                ("https://lichtkunde.example/a", "Lichtkunde", "Über Licht"),
                ("https://archiv.example/b", "https://archiv.example/b", ""),
            ],
            "nur http(s), ohne Doppelte; ohne Titel die Adresse"
        );
        let text = json!({ "content": [{ "type": "text",
            "text": "1. [Leuchtfeuer](https://leucht.example/x). Siehe auch https://archiv.example/c, danach." }] });
        let hits = parse_hits(&text).unwrap();
        assert_eq!(
            hits.iter().map(|h| (h.url.as_str(), h.title.as_str())).collect::<Vec<_>>(),
            vec![("https://leucht.example/x", "Leuchtfeuer"), ("https://archiv.example/c", "https://archiv.example/c")],
            "Markdown-Link mit Titel, freie Adresse ohne Satzzeichen am Ende"
        );
        assert_eq!(parse_hits(&json!({ "content": [] })).unwrap_err().code(), "schema", "leere Antwort → Schema");
    }
}
