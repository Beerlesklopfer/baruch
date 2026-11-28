// SPDX-License-Identifier: AGPL-3.0-or-later
//! Retrieval of a page through `fetch_url` ([`fetch_arguments`], [`parse_page`]) and HTML → text without a new
//! crate ([`html_to_text`]): `script`, `style`, `noscript`, `template`, `svg` and `head` are dropped, block
//! elements become line breaks, the other tags disappear, entities are read (named ones of German
//! typography and numeric ones). A page over [`super::PAGE_MAX_BYTES`] or without text is an error (the
//! web search skips it with a note); for the check [`super::CHECK_CHARS`] characters remain.
//!
//! **Filter before the retrieval** (Round 23, [`fetch_allowed`]): addresses from `web_search` go to `fetch_url`
//! only if by their wording they point into the Internet – `http`/`https`, without user details, no IP literal
//! in the own network (loopback, private, link-local, CGNAT, unspecified, multicast, reserved; IPv6 including the
//! `::ffff:` mapping, ULA `fc00::/7`, `fe80::/10`), not `localhost` and no name on `.localhost`,
//! `.local`, `.internal`, `.home.arpa` or without a dot. **No DNS resolution:** a public name that
//! points to a private address is not recognised here – only MultiGPT, which retrieves the page, can do that
//! (whether it filters is unknown). For tests there is [`set_fetch_exceptions`] per data folder; no
//! setting, no RPC and no form leads there.

use std::collections::HashMap;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};

use baruch_rag::RagError;
use serde_json::{Map, Value, json};

use super::search::{check_required, text_of};
use super::{CHECK_CHARS, CONTENT_KEYS, FETCH_URL_TOOL, PAGE_MAX_BYTES, TITLE_KEYS, URL_KEYS};
use crate::connections::multigpt::{clean_line, content_text, tool_payload};

/// A retrieved page as text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fetched {
    /// Title (empty if the page names none).
    pub title: String,
    pub text: String,
}

/// Exceptions of the filter per data folder – set only from tests ([`set_fetch_exceptions`]).
static FETCH_EXCEPTIONS: LazyLock<Mutex<HashMap<PathBuf, Vec<String>>>> = LazyLock::new(Mutex::default);

/// **For tests only:** hosts (as in the address, e.g. `127.0.0.1`) that [`fetch_allowed`] lets through for this
/// data folder despite the rule – so that a test can have a mock on 127.0.0.1 retrieved.
/// Scheme and user details are still checked. In the delivered product nobody calls this: there is
/// neither a setting nor RPC, MCP tool or form for it.
pub fn set_fetch_exceptions(state: &crate::AppState, hosts: &[&str]) {
    let hosts: Vec<String> = hosts.iter().map(|h| h.trim().to_ascii_lowercase()).filter(|h| !h.is_empty()).collect();
    let mut all = FETCH_EXCEPTIONS.lock().unwrap_or_else(|e| e.into_inner());
    if hosts.is_empty() {
        all.remove(&state.data);
    } else {
        all.insert(state.data.clone(), hosts);
    }
}

/// Exceptions of this data folder (always empty in the delivered product).
pub(crate) fn fetch_exceptions(state: &crate::AppState) -> Vec<String> {
    FETCH_EXCEPTIONS.lock().unwrap_or_else(|e| e.into_inner()).get(&state.data).cloned().unwrap_or_default()
}

// Reasons of a refused address: catalog keys (`websearch_msg.fetch.*`), shown through `i18n::tc` in the note.
const REFUSED_SCHEME: &str = "websearch_msg.fetch.scheme";
const REFUSED_USERINFO: &str = "websearch_msg.fetch.userinfo";
const REFUSED_HOST: &str = "websearch_msg.fetch.host";
const REFUSED_ADDRESS: &str = "websearch_msg.fetch.address";
const REFUSED_NAME: &str = "websearch_msg.fetch.name";

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
        // 192.0.0.0/24 (protocol assignments) and 240.0.0.0/4 (reserved)
        || (a == 192 && b == 0 && ip.octets()[2] == 0)
        // 198.18.0.0/15 (benchmarking networks, RFC 2544)
        || (a == 198 && (18..20).contains(&b))
        || a >= 240
}

fn private_v6(ip: Ipv6Addr) -> bool {
    let segments = ip.segments();
    let embedded = |hi: u16, lo: u16| Ipv4Addr::new((hi >> 8) as u8, hi as u8, (lo >> 8) as u8, lo as u8);
    if ip.is_loopback() || ip.is_unspecified() || ip.is_multicast() {
        return true;
    }
    // ULA fc00::/7, link-local fe80::/10, old: site-local fec0::/10
    if segments[0] & 0xfe00 == 0xfc00 || segments[0] & 0xffc0 == 0xfe80 || segments[0] & 0xffc0 == 0xfec0 {
        return true;
    }
    // IPv4 in IPv6: `::ffff:a.b.c.d` (mapped), `::a.b.c.d` (old, compatible), NAT64 `64:ff9b::/96`,
    // 6to4 `2002:a.b.c.d::/16`.
    if segments[..5] == [0, 0, 0, 0, 0] && (segments[5] == 0xffff || segments[5] == 0) {
        return private_v4(embedded(segments[6], segments[7]));
    }
    if segments[..6] == [0x64, 0xff9b, 0, 0, 0, 0] {
        return private_v4(embedded(segments[6], segments[7]));
    }
    // NAT64 for local use `64:ff9b:1::/48` (RFC 8215): the position of the IPv4 address depends on the
    // prefix length, therefore the whole range.
    if segments[..3] == [0x64, 0xff9b, 1] {
        return true;
    }
    if segments[0] == 0x2002 {
        return private_v4(embedded(segments[1], segments[2]));
    }
    false
}

/// May this address from `web_search` go to `fetch_url`? Only the wording is checked (no
/// DNS resolution, see the module documentation); the message is a German half sentence for the note.
pub fn fetch_allowed(url: &str) -> Result<(), &'static str> {
    fetch_allowed_with(url, &[])
}

/// Like [`fetch_allowed`], with exceptions for hosts (tests only, [`set_fetch_exceptions`]).
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
        // IPv6 literal; unreadable counts as rejected (rather no retrieval).
        return match bare.parse::<Ipv6Addr>() {
            Ok(ip) if !private_v6(ip) => Ok(()),
            _ => Err(REFUSED_ADDRESS),
        };
    }
    // The address check of `Url` writes every IPv4 notation (decimal, hex, octal, shortened) as a.b.c.d.
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

/// Arguments for `fetch_url` from its `inputSchema`. Without `properties` only `url`.
pub fn fetch_arguments(schema: &Value, url: &str) -> Result<Value, RagError> {
    let empty = Map::new();
    let props = schema.get("properties").and_then(Value::as_object).unwrap_or(&empty);
    if props.is_empty() {
        return Ok(json!({ "url": url }));
    }
    let key = URL_KEYS.iter().find(|k| props.contains_key(**k)).copied().ok_or_else(|| {
        RagError::Schema(crate::i18n::tc(
            "websearch_msg.fetch.no_url_param",
            &[("tool", FETCH_URL_TOOL.into()), ("keys", URL_KEYS.join(", ").into())],
        ))
    })?;
    check_required(schema, FETCH_URL_TOOL, &[Some(key)])?;
    let mut args = Map::new();
    args.insert(key.to_owned(), json!(url));
    Ok(Value::Object(args))
}

/// Does the text look like HTML?
pub fn looks_like_html(text: &str) -> bool {
    let head: String = text.chars().take(4000).collect::<String>().to_ascii_lowercase();
    ["<!doctype html", "<html", "<body", "<head", "<p>", "<div"].iter().any(|m| head.contains(m))
        || head.matches("</").count() >= 3
}

/// Page from an answer of `fetch_url`: content from the data ([`CONTENT_KEYS`]) or from `content[].text`,
/// HTML becomes text, title from the data or from `<title>`.
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
        return Err(RagError::Failed(crate::i18n::tc(
            "websearch_msg.fetch.too_big",
            &[("max", (PAGE_MAX_BYTES / (1024 * 1024)).into()), ("size", (raw.len() / 1024).into())],
        )));
    }
    let raw = raw.replace("\r\n", "\n");
    let (text, html_title) = if looks_like_html(&raw) { (html_to_text(&raw), html_title(&raw)) } else { (raw, None) };
    let text = text.trim();
    if text.is_empty() {
        return Err(RagError::Failed(crate::i18n::tc("websearch_msg.fetch.no_text", &[("url", url.into())])));
    }
    let title = title.or(html_title).map(|t| clean_line(&t, 200)).unwrap_or_default();
    Ok(Fetched { title, text: text.chars().take(CHECK_CHARS).collect() })
}

/// Position of `needle` (ASCII, lower case) in `haystack` regardless of case.
fn find_ci(haystack: &str, needle: &str) -> Option<usize> {
    // `to_ascii_lowercase` changes only ASCII letters: the byte positions stay the same.
    haystack.to_ascii_lowercase().find(needle)
}

/// Title from `<title>`.
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

/// HTML → text (see the module documentation); whitespace per line collapsed, empty lines removed.
pub fn html_to_text(html: &str) -> String {
    let mut out = String::with_capacity(html.len() / 2);
    let mut rest = html;
    while let Some(lt) = rest.find('<') {
        // Line breaks in the source text are whitespace; lines arise only at block elements.
        out.push_str(&decode_entities(&rest[..lt]).replace(['\n', '\r'], " "));
        rest = &rest[lt..];
        // A tag begins with `<` and a letter, `/` or `!`; otherwise `<` is text (e.g. "a < b").
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
            // Skip up to the closing tag; if it is missing, the content stays (rather too much text than none).
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

/// Read entities: `&amp;` `&lt;` `&gt;` `&quot;` `&apos;` `&nbsp;`, German umlauts, typographic characters
/// and numeric ones (`&#8222;`, `&#x201E;`); unknown ones stay as they are.
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
        // (address, allowed?, reason for the expectation)
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
            crate::i18n::t(crate::i18n::Lang::De, fetch_allowed("http://10.0.0.5/").unwrap_err(), &[]),
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
        let e = crate::i18n::sync_scope(crate::i18n::Lang::De, || parse_page(&big, "https://a.example/")).unwrap_err();
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
