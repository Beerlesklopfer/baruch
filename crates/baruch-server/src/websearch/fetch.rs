// SPDX-License-Identifier: AGPL-3.0-or-later
//! Abruf einer Seite über `fetch_url` ([`fetch_arguments`], [`parse_page`]) und HTML → Text ohne neue Crate
//! ([`html_to_text`]): `script`, `style`, `noscript`, `template`, `svg` und `head` fallen weg, Block-Elemente
//! werden Zeilenumbrüche, die übrigen Tags verschwinden, Entitäten werden gelesen (benannte der deutschen
//! Typografie und numerische). Eine Seite über [`super::PAGE_MAX_BYTES`] oder ohne Text ist ein Fehler (die
//! Websuche überspringt sie mit Hinweis); für die Prüfung bleiben [`super::CHECK_CHARS`] Zeichen.
//!
//! **Filter vor dem Abruf** (Runde 23, [`fetch_allowed`]): Adressen aus `web_search` gehen nur an `fetch_url`,
//! wenn sie nach ihrem Wortlaut ins Internet zeigen – `http`/`https`, ohne Benutzerangaben, kein IP-Literal
//! im eigenen Netz (Loopback, privat, Link-Local, CGNAT, unspezifiziert, Multicast, reserviert; IPv6 samt
//! `::ffff:`-Abbildung, ULA `fc00::/7`, `fe80::/10`), nicht `localhost` und kein Name auf `.localhost`,
//! `.local`, `.internal`, `.home.arpa` oder ohne Punkt. **Keine DNS-Auflösung:** Ein öffentlicher Name, der
//! auf eine private Adresse zeigt, wird hier nicht erkannt – das kann nur MultiGPT, das die Seite abruft
//! (ob es filtert, ist unbekannt). Für Tests gibt es [`set_fetch_exceptions`] je Datenordner; keine
//! Einstellung, kein RPC und kein Formular führt dorthin.

use std::collections::HashMap;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};

use baruch_rag::RagError;
use serde_json::{Map, Value, json};

use super::search::{check_required, text_of};
use super::{CHECK_CHARS, CONTENT_KEYS, FETCH_URL_TOOL, PAGE_MAX_BYTES, TITLE_KEYS, URL_KEYS};
use crate::connections::multigpt::{clean_line, content_text, tool_payload};

/// Eine abgerufene Seite als Text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fetched {
    /// Titel (leer, wenn die Seite keinen nennt).
    pub title: String,
    pub text: String,
}

/// Ausnahmen des Filters je Datenordner – nur aus Tests gesetzt ([`set_fetch_exceptions`]).
static FETCH_EXCEPTIONS: LazyLock<Mutex<HashMap<PathBuf, Vec<String>>>> = LazyLock::new(Mutex::default);

/// **Nur für Tests:** Rechner (wie in der Adresse, z. B. `127.0.0.1`), die [`fetch_allowed`] für diesen
/// Datenordner trotz der Regel durchlässt – damit ein Test einen Nachbau auf 127.0.0.1 abrufen lassen kann.
/// Schema und Benutzerangaben werden weiter geprüft. In der Auslieferung ruft das niemand: Es gibt dafür
/// weder Einstellung noch RPC, MCP-Werkzeug oder Formular.
pub fn set_fetch_exceptions(state: &crate::AppState, hosts: &[&str]) {
    let hosts: Vec<String> = hosts.iter().map(|h| h.trim().to_ascii_lowercase()).filter(|h| !h.is_empty()).collect();
    let mut all = FETCH_EXCEPTIONS.lock().unwrap_or_else(|e| e.into_inner());
    if hosts.is_empty() {
        all.remove(&state.data);
    } else {
        all.insert(state.data.clone(), hosts);
    }
}

/// Ausnahmen dieses Datenordners (in der Auslieferung immer leer).
pub(crate) fn fetch_exceptions(state: &crate::AppState) -> Vec<String> {
    FETCH_EXCEPTIONS.lock().unwrap_or_else(|e| e.into_inner()).get(&state.data).cloned().unwrap_or_default()
}

const REFUSED_SCHEME: &str = "nur http- und https-Adressen werden abgerufen";
const REFUSED_USERINFO: &str = "Adresse enthält Benutzerangaben";
const REFUSED_HOST: &str = "Adresse ohne Rechner";
const REFUSED_ADDRESS: &str = "Adresse im eigenen Netz (Loopback, privat oder Link-Local)";
const REFUSED_NAME: &str = "Rechnername des eigenen Netzes (localhost, .local, .internal, .home.arpa oder ohne Punkt)";

fn private_v4(ip: Ipv4Addr) -> bool {
    let [a, b, ..] = ip.octets();
    ip.is_loopback()
        || ip.is_private()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_multicast()
        || ip.is_broadcast()
        || a == 0
        // CGNAT 100.64.0.0/10
        || (a == 100 && (64..128).contains(&b))
        // 192.0.0.0/24 (Protokollzuweisungen) und 240.0.0.0/4 (reserviert)
        || (a == 192 && b == 0 && ip.octets()[2] == 0)
        // 198.18.0.0/15 (Messnetze, RFC 2544)
        || (a == 198 && (18..20).contains(&b))
        || a >= 240
}

fn private_v6(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    let embedded = |hi: u16, lo: u16| Ipv4Addr::new((hi >> 8) as u8, hi as u8, (lo >> 8) as u8, lo as u8);
    if ip.is_loopback() || ip.is_unspecified() || ip.is_multicast() {
        return true;
    }
    // ULA fc00::/7, Link-Local fe80::/10, alt: Site-Local fec0::/10
    if segments[0] & 0xfe00 == 0xfc00 || segments[0] & 0xffc0 == 0xfe80 || segments[0] & 0xffc0 == 0xfec0 {
        return true;
    }
    // IPv4 in IPv6: `::ffff:a.b.c.d` (abgebildet), `::a.b.c.d` (alt, kompatibel), NAT64 `64:ff9b::/96`,
    // 6to4 `2002:a.b.c.d::/16`.
    if segments[..5] == [0, 0, 0, 0, 0] && (segments[5] == 0xffff || segments[5] == 0) {
        return private_v4(embedded(segments[6], segments[7]));
    }
    if segments[..6] == [0x64, 0xff9b, 0, 0, 0, 0] {
        return private_v4(embedded(segments[6], segments[7]));
    }
    // NAT64 für den eigenen Gebrauch `64:ff9b:1::/48` (RFC 8215): die Lage der IPv4-Adresse hängt von der
    // Präfixlänge ab, darum der ganze Bereich.
    if segments[..3] == [0x64, 0xff9b, 1] {
        return true;
    }
    if segments[0] == 0x2002 {
        return private_v4(embedded(segments[1], segments[2]));
    }
    false
}

/// Darf diese Adresse aus `web_search` an `fetch_url` gehen? Geprüft wird nur der Wortlaut (keine
/// DNS-Auflösung, siehe Moduldoku); die Meldung ist ein deutscher Halbsatz für den Hinweis.
pub fn fetch_allowed(url: &str) -> Result<(), &'static str> {
    fetch_allowed_with(url, &[])
}

/// Wie [`fetch_allowed`], mit Ausnahmen für Rechner (nur Tests, [`set_fetch_exceptions`]).
pub(crate) fn fetch_allowed_with(url: &str, exceptions: &[String]) -> Result<(), &'static str> {
    let parsed = reqwest::Url::parse(url.trim()).map_err(|_| REFUSED_SCHEME)?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(REFUSED_SCHEME);
    }
    if !parsed.username().is_empty() || parsed.password().is_some() {
        return Err(REFUSED_USERINFO);
    }
    let host = parsed.host_str().unwrap_or("").trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty() {
        return Err(REFUSED_HOST);
    }
    let bare = host.trim_start_matches('[').trim_end_matches(']');
    if exceptions.iter().any(|e| e == &host || e == bare) {
        return Ok(());
    }
    if host.starts_with('[') {
        // IPv6-Literal; unlesbar gilt als abgelehnt (lieber kein Abruf).
        return match bare.parse::<Ipv6Addr>() {
            Ok(ip) if !private_v6(ip) => Ok(()),
            _ => Err(REFUSED_ADDRESS),
        };
    }
    // Die Adressprüfung von `Url` schreibt jede IPv4-Schreibweise (dezimal, hex, oktal, verkürzt) als a.b.c.d.
    if let Ok(ip) = host.parse::<Ipv4Addr>() {
        return if private_v4(ip) { Err(REFUSED_ADDRESS) } else { Ok(()) };
    }
    let local_name = host == "localhost"
        || !host.contains('.')
        || [".localhost", ".local", ".internal", ".home.arpa"].iter().any(|suffix| host.ends_with(suffix));
    if local_name {
        return Err(REFUSED_NAME);
    }
    Ok(())
}

/// Argumente für `fetch_url` aus dessen `inputSchema`. Ohne `properties` nur `url`.
pub fn fetch_arguments(schema: &Value, url: &str) -> Result<Value, RagError> {
    let empty = Map::new();
    let props = schema.get("properties").and_then(Value::as_object).unwrap_or(&empty);
    if props.is_empty() {
        return Ok(json!({ "url": url }));
    }
    let key = URL_KEYS.iter().find(|k| props.contains_key(**k)).copied().ok_or_else(|| {
        RagError::Schema(format!(
            "{FETCH_URL_TOOL} nennt im inputSchema keinen Parameter für die Adresse ({}).",
            URL_KEYS.join(", ")
        ))
    })?;
    check_required(schema, FETCH_URL_TOOL, &[Some(key)])?;
    let mut args = Map::new();
    args.insert(key.to_owned(), json!(url));
    Ok(Value::Object(args))
}

/// Sieht der Text wie HTML aus?
pub fn looks_like_html(text: &str) -> bool {
    let head: String = text.chars().take(4000).collect::<String>().to_ascii_lowercase();
    ["<!doctype html", "<html", "<body", "<head", "<p>", "<div"].iter().any(|m| head.contains(m))
        || head.matches("</").count() >= 3
}

/// Seite aus einer Antwort von `fetch_url`: Inhalt aus den Daten ([`CONTENT_KEYS`]) bzw. aus `content[].text`,
/// HTML wird Text, Titel aus den Daten oder aus `<title>`.
pub fn parse_page(result: &Value, url: &str) -> Result<Fetched, RagError> {
    let payload = tool_payload(result).and_then(|p| p.as_object().cloned());
    let (raw, title) = match &payload {
        Some(map) => match text_of(map, CONTENT_KEYS) {
            Some(content) => (content, text_of(map, TITLE_KEYS)),
            None => (content_text(result), None),
        },
        None => (content_text(result), None),
    };
    if raw.len() > PAGE_MAX_BYTES {
        return Err(RagError::Failed(format!(
            "Seite ist größer als {} MiB ({} KiB)",
            PAGE_MAX_BYTES / (1024 * 1024),
            raw.len() / 1024
        )));
    }
    let raw = raw.replace("\r\n", "\n");
    let (text, html_title) = if looks_like_html(&raw) { (html_to_text(&raw), html_title(&raw)) } else { (raw, None) };
    let text = text.trim();
    if text.is_empty() {
        return Err(RagError::Failed(format!("{url}: die Seite enthält keinen Text")));
    }
    let title = title.or(html_title).map(|t| clean_line(&t, 200)).unwrap_or_default();
    Ok(Fetched { title, text: text.chars().take(CHECK_CHARS).collect() })
}

/// Position von `needle` (ASCII, klein) in `haystack` ohne Rücksicht auf Groß-/Kleinschreibung.
fn find_ci(haystack: &str, needle: &str) -> Option<usize> {
    // `to_ascii_lowercase` ändert nur ASCII-Buchstaben: die Byte-Stellen bleiben gleich.
    haystack.to_ascii_lowercase().find(needle)
}

/// Titel aus `<title>`.
pub fn html_title(html: &str) -> Option<String> {
    let open = find_ci(html, "<title")?;
    let after = &html[open..];
    let start = after.find('>')? + 1;
    let end = find_ci(&after[start..], "</title")?;
    let title = clean_line(&decode_entities(&after[start..start + end]), 200);
    (!title.is_empty()).then_some(title)
}

const SKIPPED: &[&str] = &["script", "style", "noscript", "template", "svg", "head"];
const BLOCKS: &[&str] = &[
    "p",
    "br",
    "div",
    "li",
    "ul",
    "ol",
    "tr",
    "td",
    "th",
    "table",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "blockquote",
    "pre",
    "section",
    "article",
    "header",
    "footer",
    "nav",
    "aside",
    "main",
    "hr",
    "dd",
    "dt",
    "figcaption",
    "title",
];

/// HTML → Text (siehe Moduldoku); Leerraum je Zeile zusammengezogen, leere Zeilen entfernt.
pub fn html_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len() / 2);
    let mut rest = html;
    while let Some(lt) = rest.find('<') {
        // Zeilenumbrüche im Quelltext sind Leerraum; Zeilen entstehen nur an Block-Elementen.
        out.push_str(&decode_entities(&rest[..lt]).replace(['\n', '\r'], " "));
        rest = &rest[lt..];
        // Ein Tag beginnt mit `<` und einem Buchstaben, `/` oder `!`; sonst ist `<` Text (z. B. „a < b“).
        if !rest[1..].starts_with(|c: char| c.is_ascii_alphabetic() || c == '/' || c == '!') {
            out.push('<');
            rest = &rest[1..];
            continue;
        }
        if rest.starts_with("<!--") {
            rest = rest.find("-->").map_or("", |end| &rest[end + 3..]);
            continue;
        }
        let Some(gt) = rest.find('>') else {
            rest = "";
            break;
        };
        let tag = &rest[1..gt];
        rest = &rest[gt + 1..];
        let closing = tag.starts_with('/');
        let name = tag
            .trim_start_matches('/')
            .split(|c: char| c.is_whitespace() || c == '/')
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        if !closing && SKIPPED.contains(&name.as_str()) && !tag.ends_with('/') {
            // Bis zum schließenden Tag überspringen; fehlt es, bleibt der Inhalt (lieber zu viel Text als keiner).
            match find_ci(rest, &format!("</{name}")) {
                Some(close) => {
                    let after = &rest[close..];
                    rest = after.find('>').map_or("", |e| &after[e + 1..]);
                }
                None => out.push('\n'),
            }
            continue;
        }
        if BLOCKS.contains(&name.as_str()) {
            out.push('\n');
        }
    }
    out.push_str(&decode_entities(rest).replace(['\n', '\r'], " "));
    out.lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Entitäten lesen: `&amp;` `&lt;` `&gt;` `&quot;` `&apos;` `&nbsp;`, deutsche Umlaute, typografische Zeichen
/// und numerische (`&#8222;`, `&#x201E;`); Unbekanntes bleibt stehen.
pub fn decode_entities(text: &str) -> String {
    if !text.contains('&') {
        return text.to_owned();
    }
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let end = rest.char_indices().take(12).find(|(_, c)| *c == ';').map(|(i, _)| i);
        let decoded = end.and_then(|end| entity(&rest[1..end]).map(|c| (c, end)));
        match decoded {
            Some((c, end)) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn entity(name: &str) -> Option<char> {
    if let Some(num) = name.strip_prefix('#') {
        let code = match num.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, 16).ok()?,
            None => num.parse().ok()?,
        };
        return char::from_u32(code).filter(|c| !c.is_control() || matches!(c, '\n' | '\t'));
    }
    Some(match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => '\u{a0}',
        "auml" => 'ä',
        "ouml" => 'ö',
        "uuml" => 'ü',
        "Auml" => 'Ä',
        "Ouml" => 'Ö',
        "Uuml" => 'Ü',
        "szlig" => 'ß',
        "bdquo" => '„',
        "ldquo" => '“',
        "rdquo" => '”',
        "sbquo" => '‚',
        "lsquo" => '‘',
        "rsquo" => '’',
        "laquo" => '«',
        "raquo" => '»',
        "ndash" => '–',
        "mdash" => '—',
        "hellip" => '…',
        "shy" => '\u{ad}',
        "eacute" => 'é',
        "egrave" => 'è',
        "copy" => '©',
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_becomes_text_without_scripts_styles_and_tags() {
        let html = "<!doctype html><html><head><title>Licht &amp; Schatten</title><style>p{color:red}</style>\
            </head><body><script>alert('weg')</script><h1>Über das Licht</h1><p>Das Licht\n   kreist&nbsp;über \
            dem <b>Wasser</b>.</p><!-- versteckt --><p>&bdquo;Zitat&ldquo; &#8222;zwei&#x201C; &unbekannt; A&B</p></body></html>";
        assert_eq!(
            html_to_text(html),
            "Über das Licht\nDas Licht kreist über dem Wasser.\n„Zitat“ „zwei“ &unbekannt; A&B",
            "Kopf, Skript, Stil und Kommentar fallen weg; Blöcke werden Zeilen; Entitäten gelesen, Unbekanntes bleibt"
        );
        assert_eq!(html_title(html).as_deref(), Some("Licht & Schatten"), "Titel aus <title>");
        assert_eq!(
            html_to_text("<p>Anfang<script>ohne Ende"),
            "Anfang\nohne Ende",
            "fehlt das schließende Tag, bleibt der Text (lieber zu viel als nichts)"
        );
        assert_eq!(html_to_text("<p>a < b >>> c</p>"), "a < b >>> c", "ein < ohne Tag ist Text, ebenso >");
    }

    #[test]
    fn fetch_allowed_refuses_addresses_of_the_own_network_and_lets_public_ones_through() {
        // (Adresse, erlaubt?, Begründung der Erwartung)
        let table: &[(&str, bool, &str)] = &[
            ("https://lichtkunde.example/feuer", true, "öffentlicher Name über https"),
            ("http://archiv.example:8080/chronik?x=1", true, "öffentlicher Name über http mit Port"),
            ("https://93.184.216.34/", true, "öffentliche IPv4-Adresse"),
            ("https://[2001:4860:4860::8888]/", true, "öffentliche IPv6-Adresse"),
            ("https://xn--mnchen-3ya.example/", true, "Umlaut-Name (Punycode)"),
            ("ftp://lichtkunde.example/datei", false, "fremdes Schema ftp"),
            ("file:///etc/passwd", false, "fremdes Schema file"),
            ("javascript:alert(1)", false, "fremdes Schema javascript"),
            ("gopher://lichtkunde.example/", false, "fremdes Schema gopher"),
            ("lichtkunde.example/feuer", false, "ohne Schema keine Adresse"),
            ("https://leser@lichtkunde.example/", false, "Benutzername in der Adresse"),
            ("https://leser:pass@lichtkunde.example/", false, "Benutzer und Passwort in der Adresse"),
            ("https://:pass@lichtkunde.example/", false, "nur Passwort in der Adresse"),
            ("http://127.0.0.1/", false, "Loopback"),
            ("http://127.8.9.10:8765/mcp", false, "ganzes Netz 127/8 ist Loopback"),
            ("http://2130706433/", false, "Loopback als Dezimalzahl"),
            ("http://0x7f.0.0.1/", false, "Loopback hexadezimal"),
            ("http://0177.0.0.1/", false, "Loopback oktal"),
            ("http://127.1/", false, "Loopback verkürzt"),
            ("http://10.0.0.5/", false, "privat 10/8"),
            ("http://172.16.0.1/", false, "privat 172.16/12 (Anfang)"),
            ("http://172.31.255.254/", false, "privat 172.16/12 (Ende)"),
            ("http://172.32.0.1/", true, "172.32 liegt außerhalb von 172.16/12"),
            ("http://192.168.178.1/", false, "privat 192.168/16"),
            ("http://169.254.169.254/latest/meta-data", false, "Link-Local (Metadatendienst)"),
            ("http://100.64.0.1/", false, "CGNAT 100.64/10 (Anfang)"),
            ("http://100.127.255.254/", false, "CGNAT 100.64/10 (Ende)"),
            ("http://100.128.0.1/", true, "100.128 liegt außerhalb von 100.64/10"),
            ("http://0.0.0.0/", false, "unspezifiziert"),
            ("http://224.0.0.251/", false, "Multicast"),
            ("http://255.255.255.255/", false, "Broadcast"),
            ("http://[::1]/", false, "IPv6-Loopback"),
            ("http://[::]/", false, "IPv6 unspezifiziert"),
            ("http://[::ffff:127.0.0.1]/", false, "abgebildetes Loopback"),
            ("http://[::ffff:192.168.1.1]/", false, "abgebildete private Adresse"),
            ("http://[::ffff:c0a8:0101]/", false, "abgebildete private Adresse in Hex-Schreibweise"),
            ("http://[::ffff:93.184.216.34]/", true, "abgebildete öffentliche Adresse"),
            ("http://[fc00::1]/", false, "ULA fc00::/7"),
            ("http://[fd12:3456:789a::1]/", false, "ULA fd00::/8"),
            ("http://[fe80::1]/", false, "Link-Local fe80::/10"),
            ("http://[febf::1]/", false, "Ende von fe80::/10"),
            ("http://[ff02::1]/", false, "IPv6-Multicast"),
            ("http://[64:ff9b::7f00:1]/", false, "NAT64 auf Loopback"),
            ("http://[64:ff9b:1::1]/", false, "NAT64 für den eigenen Gebrauch (64:ff9b:1::/48) ist nie öffentlich"),
            ("http://198.18.0.1/", false, "Messnetz 198.18/15 (Anfang)"),
            ("http://198.19.255.254/", false, "Messnetz 198.18/15 (Ende)"),
            ("http://198.20.0.1/", true, "198.20 liegt außerhalb von 198.18/15"),
            ("http://localhost/", false, "localhost"),
            ("http://LOCALHOST:8765/", false, "localhost groß geschrieben"),
            ("http://localhost./", false, "localhost mit Schlusspunkt"),
            ("http://baruch.localhost/", false, "Name auf .localhost"),
            ("http://drucker.local/", false, "Name auf .local (mDNS)"),
            ("http://dienst.internal/", false, "Name auf .internal"),
            ("http://nas.home.arpa/", false, "Name auf .home.arpa"),
            ("http://router/", false, "Name ohne Punkt (Kurzname im eigenen Netz)"),
            ("https://local.example/", true, "„local“ nur als Teil eines öffentlichen Namens"),
            ("https://internal-news.example/", true, "„internal“ nur am Anfang des Namens"),
        ];
        for (url, allowed, why) in table {
            assert_eq!(fetch_allowed(url).is_ok(), *allowed, "{why}: {url} → {:?}", fetch_allowed(url));
        }
        assert_eq!(
            fetch_allowed("http://10.0.0.5/").unwrap_err(),
            "Adresse im eigenen Netz (Loopback, privat oder Link-Local)",
            "der Grund ist ein deutscher Halbsatz für den Hinweis"
        );
    }

    #[test]
    fn exceptions_only_lift_the_host_rule() {
        let only = vec!["127.0.0.1".to_owned()];
        assert!(fetch_allowed_with("http://127.0.0.1:4711/seite", &only).is_ok(), "Ausnahme für den Nachbau im Test");
        assert!(fetch_allowed_with("http://127.0.0.2/", &only).is_err(), "nur genau der genannte Rechner");
        assert!(fetch_allowed_with("ftp://127.0.0.1/", &only).is_err(), "das Schema bleibt geprüft");
        assert!(fetch_allowed_with("http://u:p@127.0.0.1/", &only).is_err(), "Benutzerangaben bleiben verboten");
        assert!(fetch_allowed("http://127.0.0.1:4711/seite").is_err(), "ohne Ausnahme (Auslieferung) abgelehnt");
    }

    #[test]
    fn fetch_arguments_take_the_url_key_from_the_schema() {
        assert_eq!(
            fetch_arguments(&json!({}), "https://a.example/").unwrap(),
            json!({ "url": "https://a.example/" }),
            "ohne Schema gilt der übliche Name url"
        );
        let schema = json!({ "properties": { "uri": {}, "max_length": {} }, "required": ["uri"] });
        assert_eq!(
            fetch_arguments(&schema, "https://a.example/").unwrap(),
            json!({ "uri": "https://a.example/" }),
            "Name aus dem Schema, freiwillige Parameter bleiben weg"
        );
        let strict = json!({ "properties": { "url": {}, "token": {} }, "required": ["url", "token"] });
        assert_eq!(fetch_arguments(&strict, "x").unwrap_err().code(), "schema", "unbekannte Pflicht → Schema");
    }

    #[test]
    fn pages_over_one_mebibyte_or_without_text_are_refused() {
        let big = json!({ "structuredContent": { "content": "x".repeat(PAGE_MAX_BYTES + 1) } });
        let e = parse_page(&big, "https://a.example/").unwrap_err();
        assert!(e.message().contains("größer als 1 MiB"), "Größe genannt: {e:?}");
        let empty = json!({ "structuredContent": { "content": "<html><body><script>x</script></body></html>" } });
        assert!(parse_page(&empty, "https://a.example/").is_err(), "ohne Text keine Seite");
        let plain = json!({ "content": [{ "type": "text", "text": "Nur Text,\r\nzwei Zeilen." }] });
        assert_eq!(
            parse_page(&plain, "https://a.example/").unwrap(),
            Fetched { title: String::new(), text: "Nur Text,\nzwei Zeilen.".into() },
            "Text ohne JSON aus content[], Zeilenenden vereinheitlicht"
        );
    }
}
