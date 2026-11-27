// SPDX-License-Identifier: AGPL-3.0-or-later
//! Protection of the writing endpoints (`POST /rpc`, `/act`, `/mcp`) against foreign web pages (CSRF).
//!
//! A browser sends `Origin` and (today's browsers) `Sec-Fetch-Site` with every POST. The request is
//! rejected with 403 if
//! - `Sec-Fetch-Site` is neither `same-origin` nor `none` (`none`: triggered by the user himself), or
//! - `Origin` is there and does not match `Host` (also `Origin: null`).
//!
//! Without both headers (curl, scripts, MCP tools) the request passes: those are not
//! browsers, and a browser does not omit `Origin` on POST. `/rpc` additionally demands
//! `Content-Type: application/json` (415 otherwise); so a foreign `fetch` needs a preflight
//! that the server does not answer.
//!
//! **DNS rebinding** (a foreign name suddenly points to 127.0.0.1, `Origin` and `Host` then
//! match) is caught by [`local_host`]: for **all** routes, also reading ones, `Host` must be an IP address
//! (`127.0.0.1`, `[::1]`, the address in the LAN …) or `localhost`, otherwise 421. An IP address cannot
//! be rebound. Without `Host` (possible only without a browser) the request passes.
//!
//! **Behind a reverse proxy** (Round 24): [`AllowedHosts`] names additionally allowed host names
//! (`--allow-host`, `BARUCH_ALLOWED_HOSTS`); only exactly these names (with any port) pass [`local_host`],
//! every other name stays 421. The proxy must pass `Host` through unchanged – [`check`] compares `Origin` with
//! `Host`. A request under an additional name never counts as local ([`via_proxy`]): `/mcp` then always
//! demands a token, even if the remote side (the proxy) is loopback. Baruch does not evaluate `X-Forwarded-*`
//! – with one exception: the mere presence of such a header ([`forwarded`]) takes away from a request the
//! exception „Am eigenen Rechner ohne Anmeldung“ and the setup of the first account (a proxy on the same
//! machine that rewrites `Host` would otherwise look like the own machine).
//!
//! **Login** (Round 35): without accounts Baruch has no login, and the access protection is provided by the proxy
//! (`docs/Betrieb.md`). With accounts [`crate::auth::gate`] demands a session for pages, forms and `/rpc`;
//! what a role may do is decided by [`crate::roles`]. The layers of this module are unaffected by that and
//! lie outside the login: a foreign page, a foreign host name or a foreign machine does not even
//! get as far as the login.
//!
//! **Reading requests of foreign pages** (Round 23): [`own_reads`] rejects for **all** routes a GET/HEAD that a
//! foreign page triggers as a sub-resource ([`cross_site_subresource`]: image, script, `fetch`, frame) – 403. A
//! navigation of the top-level window (link from outside) still passes.
//!
//! **Other machines** (Round 10): [`remote_only_mcp`] lets only `/mcp` through from a remote side outside of
//! loopback (there the token decides); UI, `/rpc` and forms stay reserved for the own
//! machine (403).
//!
//! **Clickjacking:** [`no_framing`] sets `X-Frame-Options: DENY` and `frame-ancestors
//! 'none'` in the `Content-Security-Policy` (appended to an existing CSP) on **every** response. Otherwise a
//! foreign page could e.g. frame the consent dialog of the outline or the release „extern“ invisibly and make the
//! user click – the form would then be sent from the framed document itself (same-origin), and
//! [`same_origin`] would not catch it. Only exception (Round 20b, contract §0.9): the EPUB preview frames the files
//! of the book itself (`/werke/{id}/epub/leser/…`). These responses set their CSP themselves
//! (`sandbox allow-same-origin; frame-ancestors 'self'`); if a response already carries `frame-ancestors`, its CSP
//! stays unchanged and
//! `X-Frame-Options` becomes `SAMEORIGIN` instead of `DENY` (with `frame-ancestors 'self'`) or is dropped (any other
//! own directive, then the CSP alone applies). All other responses keep `DENY` and `frame-ancestors 'none'`.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::{ConnectInfo, Request, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::AppState;

/// Directive against framing in the CSP.
pub const FRAME_ANCESTORS: &str = "frame-ancestors 'none'";

/// Middleware for all responses: never display in a frame (see module head).
pub async fn no_framing(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    let own = headers
        .get(header::CONTENT_SECURITY_POLICY)
        .and_then(|v| v.to_str().ok())
        .and_then(|csp| csp.split(';').map(str::trim).find(|d| d.starts_with("frame-ancestors")))
        .map(|d| d.split_whitespace().skip(1).collect::<Vec<_>>() == ["'self'"]);
    match own {
        None => {
            headers.insert(header::X_FRAME_OPTIONS, header::HeaderValue::from_static("DENY"));
        }
        Some(true) => {
            headers.insert(header::X_FRAME_OPTIONS, header::HeaderValue::from_static("SAMEORIGIN"));
        }
        Some(false) => {
            headers.remove(header::X_FRAME_OPTIONS);
        }
    }
    let csp = match headers.get(header::CONTENT_SECURITY_POLICY).and_then(|v| v.to_str().ok()) {
        Some(existing) if existing.contains("frame-ancestors") => None,
        Some(existing) => Some(format!("{}; {FRAME_ANCESTORS}", existing.trim_end_matches([';', ' ']))),
        None => Some(FRAME_ANCESTORS.to_owned()),
    };
    if let Some(value) = csp.and_then(|v| header::HeaderValue::from_str(&v).ok()) {
        headers.insert(header::CONTENT_SECURITY_POLICY, value);
    }
    response
}

/// GET of a foreign page (Round 18a): `Sec-Fetch-Site` is `cross-site` or `same-site`. If the header is missing
/// (old browsers, curl, tests) or it is `same-origin`/`none`, the request counts as own.
pub fn cross_site_get(headers: &HeaderMap) -> bool {
    headers
        .get("sec-fetch-site")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|site| site.eq_ignore_ascii_case("cross-site") || site.eq_ignore_ascii_case("same-site"))
}

/// Reading request (GET/HEAD) of a foreign page as a sub-resource (image, script, fetch, frame). Round 23.
///
/// Rule (contract R23 §4): method GET or HEAD, `Sec-Fetch-Site` is `cross-site` or `same-site`, and the request
/// is **not** a navigation of the top-level window (`Sec-Fetch-Mode: navigate` together with `Sec-Fetch-Dest:
/// document`). So a link from a foreign page or from a mail program still opens Baruch; an `<img src=…>`, `<script>`,
/// `fetch` or a frame (`Sec-Fetch-Dest: iframe`) of a foreign page gets 403. Thus a foreign page no longer triggers
/// a reading route with a side effect (e.g. `GET /einstellungen/assistenten?modelle=abfragen`, which queries the
/// remote side) and cannot measure any response. Without `Sec-Fetch-Site` (curl, tests, MCP clients, old browsers)
/// and with `same-origin`/`none` the request counts as own.
///
/// Delimitation (decision web, reported to the integrator): if **both** headers `Sec-Fetch-Mode`
/// and `Sec-Fetch-Dest` are missing, the kind of the request is not proven; it then does not count as a
/// sub-resource. Every browser that sends `Sec-Fetch-Site` also sends `Sec-Fetch-Mode` (Fetch Metadata) – so a real
/// sub-resource never falls into this gap. The routes with their own check ([`cross_site_get`]: PDF, typeset view,
/// cover, EPUB, search with preview, full text) still reject such a request themselves; older tests model the
/// “link from a foreign page” this way.
pub fn cross_site_subresource(method: &axum::http::Method, headers: &HeaderMap) -> bool {
    if method != axum::http::Method::GET && method != axum::http::Method::HEAD {
        return false;
    }
    if !cross_site_get(headers) {
        return false;
    }
    let value = |name: &str| headers.get(name).and_then(|v| v.to_str().ok()).map(|v| v.trim().to_ascii_lowercase());
    match (value("sec-fetch-mode"), value("sec-fetch-dest")) {
        (None, None) => false,
        (Some(mode), Some(dest)) => !(mode == "navigate" && dest == "document"),
        _ => true,
    }
}

/// Middleware for all routes: rejects `cross_site_subresource` with 403.
///
/// Only exception: the files of the EPUB preview (`/werke/{id}/epub/leser/…`). They stand in a frame with
/// `sandbox` (Round 20b); the browser reports images, style and jumps from this frame as `cross-site` although they
/// come from the own book. There the route itself still decides (`epub_preview::from_own_frame` through `Referer`).
pub async fn own_reads(request: Request, next: Next) -> Response {
    if cross_site_subresource(request.method(), request.headers()) && !epub_reader_path(request.uri().path()) {
        return (StatusCode::FORBIDDEN, "Abgelehnt: Anfrage von einer fremden Seite (lesend)").into_response();
    }
    next.run(request).await
}

/// Path of a file of the EPUB preview: `/werke/<number>/epub/leser/<…>`.
fn epub_reader_path(path: &str) -> bool {
    path.strip_prefix("/werke/").and_then(|rest| rest.split_once('/')).is_some_and(|(id, rest)| {
        id.bytes().all(|b| b.is_ascii_digit()) && !id.is_empty() && rest.starts_with("epub/leser/")
    })
}

/// Middleware for the writing routes.
pub async fn same_origin(request: Request, next: Next) -> Response {
    match check(request.headers()) {
        Ok(()) => next.run(request).await,
        Err(why) => (StatusCode::FORBIDDEN, format!("Abgelehnt: {why}")).into_response(),
    }
}

/// Middleware for all routes: from a remote side outside of loopback (`ConnectInfo`) only `/mcp`,
/// otherwise 403. Without `ConnectInfo` (tests, Unix socket) the request passes.
pub async fn remote_only_mcp(request: Request, next: Next) -> Response {
    let peer = request.extensions().get::<ConnectInfo<SocketAddr>>().map(|ConnectInfo(addr)| *addr);
    match peer {
        Some(addr) if !addr.ip().to_canonical().is_loopback() && request.uri().path() != "/mcp" => {
            (StatusCode::FORBIDDEN, "Abgelehnt: Von anderen Rechnern nur /mcp mit Token.").into_response()
        }
        _ => next.run(request).await,
    }
}

/// Additionally allowed host names (operation behind a reverse proxy): lower case, without port.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AllowedHosts(Vec<String>);

impl AllowedHosts {
    /// List, separated by comma or whitespace. `Err` names the invalid name.
    ///
    /// Valid (contract R24 §4): 1–253 characters, labels of 1–63 characters from `a–z0–9-` (no `-` at the edge), no
    /// port, no wildcard (`*`), no IP address and not `localhost` (both apply anyway, [`allowed_host`]). A name
    /// whose last label is a number (`127.1`, `0x7f.1`) is rejected too: browsers read it as an
    /// IPv4 address. Upper-case letters are stored lower case, duplicate names only once.
    pub fn parse(list: &str) -> Result<Self, String> {
        let mut names: Vec<String> = Vec::new();
        for raw in list.split(|c: char| c == ',' || c.is_whitespace()).filter(|part| !part.is_empty()) {
            let name = raw.to_ascii_lowercase();
            valid_name(&name).map_err(|why| format!("Ungültiger Rechnername „{raw}“ (--allow-host): {why}"))?;
            if !names.contains(&name) {
                names.push(name);
            }
        }
        Ok(Self(names))
    }

    pub fn names(&self) -> &[String] {
        &self.0
    }

    /// `allowed_host(host)` or one of the additional names (port as in `allowed_host`).
    pub fn allows(&self, host: &str) -> bool {
        allowed_host(host) || self.is_extra(host)
    }

    /// Only the additional names (not IP address, not `localhost`).
    pub fn is_extra(&self, host: &str) -> bool {
        let name = match host.rsplit_once(':') {
            Some((name, port)) if is_port(port) => name,
            Some(_) => return false,
            None => host,
        };
        self.0.iter().any(|allowed| allowed.eq_ignore_ascii_case(name))
    }
}

/// Checks a (already lower-case) host name; `Err` names the reason.
fn valid_name(name: &str) -> Result<(), &'static str> {
    if name.contains('*') {
        return Err("Platzhalter (*) sind nicht erlaubt; jeden Namen einzeln nennen.");
    }
    if name.contains(':') || name.contains('[') || name.contains(']') {
        return Err("ohne Port und ohne IPv6-Adresse angeben (der Port ist beliebig, IP-Adressen gelten ohnehin).");
    }
    if name.len() > 253 {
        return Err("höchstens 253 Zeichen.");
    }
    if name == "localhost" || name.parse::<std::net::Ipv4Addr>().is_ok() {
        return Err("IP-Adressen und localhost gelten ohnehin und werden hier nicht genannt.");
    }
    let mut last = "";
    for label in name.split('.') {
        if label.is_empty() || label.len() > 63 {
            return Err("jeder Teil zwischen den Punkten braucht 1 bis 63 Zeichen (kein Punkt am Anfang oder Ende).");
        }
        if !label.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-') {
            return Err("erlaubt sind nur Buchstaben a–z, Ziffern, Bindestrich und Punkt.");
        }
        if label.starts_with('-') || label.ends_with('-') {
            return Err("ein Teil darf nicht mit einem Bindestrich beginnen oder enden.");
        }
        last = label;
    }
    let hex = last.strip_prefix("0x").is_some_and(|digits| digits.bytes().all(|b| b.is_ascii_hexdigit()));
    if last.bytes().all(|b| b.is_ascii_digit()) || hex {
        return Err("der letzte Teil ist eine Zahl; Browser lesen so einen Namen als IP-Adresse.");
    }
    Ok(())
}

/// `Host` of the request is an additional name: the request came through a proxy and never counts as local.
///
/// If additional names are allowed, a request **without a readable `Host` header** does not count as local either:
/// then a proxy stands in front of Baruch, and a request without `Host` (e.g. forwarded over HTTP/2) could otherwise
/// not be told from one of the own machine. Without additional names it stays `false` (as before Round 24).
pub fn via_proxy(hosts: &AllowedHosts, headers: &HeaderMap) -> bool {
    if hosts.names().is_empty() {
        return false;
    }
    match headers.get(header::HOST).map(|h| h.to_str()) {
        Some(Ok(host)) => hosts.is_extra(host),
        Some(Err(_)) | None => true,
    }
}

/// Headers that a reverse proxy appends to a forwarded request.
pub const FORWARDED_HEADERS: [&str; 6] =
    ["forwarded", "x-forwarded-for", "x-forwarded-host", "x-forwarded-proto", "x-real-ip", "via"];

/// Does the request carry one of the headers of a proxy ([`FORWARDED_HEADERS`])? The content is not read: even
/// an empty or unreadable header counts. Such a request never counts as “own machine” for the login
/// (Round 35, decision 0.2) – not even if remote side and `Host` look like the own machine.
pub fn forwarded(headers: &HeaderMap) -> bool {
    FORWARDED_HEADERS.iter().any(|name| headers.contains_key(*name))
}

/// Middleware for all routes: `Host` must satisfy [`AllowedHosts::allows`] (protection against DNS rebinding).
pub async fn local_host(State(state): State<Arc<AppState>>, request: Request, next: Next) -> Response {
    let host = request.headers().get(header::HOST).map(|h| h.to_str().map_err(|_| ()));
    match host {
        None => next.run(request).await,
        Some(Ok(host)) if state.allowed_hosts.allows(host) => next.run(request).await,
        Some(_) => (
            StatusCode::MISDIRECTED_REQUEST,
            "Abgelehnt: Baruch antwortet nur unter einer IP-Adresse, localhost oder einem beim Start erlaubten \
             Rechnernamen (--allow-host, Betrieb hinter einem Reverse-Proxy), nicht unter einem fremden Namen \
             (Host-Kopfzeile, Schutz vor DNS-Rebinding).",
        )
            .into_response(),
    }
}

/// `Host` (with or without port) is `localhost`, an IPv4 address or an IPv6 address in `[…]`.
/// Host names are rejected because a foreign name can point to this machine through DNS.
pub fn allowed_host(host: &str) -> bool {
    let name = if let Some(rest) = host.strip_prefix('[') {
        let Some((inner, after)) = rest.split_once(']') else { return false };
        if !(after.is_empty() || after.strip_prefix(':').is_some_and(is_port)) {
            return false;
        }
        return inner.parse::<std::net::Ipv6Addr>().is_ok();
    } else {
        match host.rsplit_once(':') {
            Some((name, port)) if is_port(port) => name,
            Some(_) => return false,
            None => host,
        }
    };
    name.eq_ignore_ascii_case("localhost") || name.parse::<std::net::Ipv4Addr>().is_ok()
}

fn is_port(port: &str) -> bool {
    !port.is_empty() && port.len() <= 5 && port.bytes().all(|b| b.is_ascii_digit())
}

/// Checks `Sec-Fetch-Site` and `Origin` against `Host`. `Err` names the reason.
pub fn check(headers: &HeaderMap) -> Result<(), &'static str> {
    if let Some(site) = headers.get("sec-fetch-site") {
        match site.to_str() {
            Ok("same-origin" | "none") => {}
            _ => return Err("Anfrage von einer fremden Seite (Sec-Fetch-Site)"),
        }
    }
    if let Some(origin) = headers.get(header::ORIGIN) {
        let origin = origin.to_str().map_err(|_| "Origin nicht lesbar")?;
        let host = headers.get(header::HOST).and_then(|h| h.to_str().ok()).ok_or("Origin ohne Host")?;
        let authority = origin.strip_prefix("http://").or_else(|| origin.strip_prefix("https://"));
        if authority != Some(host) {
            return Err("Anfrage von einer fremden Seite (Origin passt nicht zu Host)");
        }
    }
    Ok(())
}

/// `Content-Type` is `application/json` (parameters like `charset` allowed).
pub fn is_json(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"))
}

/// Response if `is_json` fails.
pub fn need_json() -> Response {
    (StatusCode::UNSUPPORTED_MEDIA_TYPE, "Content-Type: application/json erwartet").into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&'static str, &'static str)]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for (k, v) in pairs {
            map.insert(*k, v.parse().unwrap());
        }
        map
    }

    #[test]
    fn every_proxy_header_alone_marks_a_request_as_forwarded() {
        assert!(!forwarded(&headers(&[])), "ohne Kopfzeilen kommt die Anfrage direkt");
        assert!(
            !forwarded(&headers(&[("host", "127.0.0.1:8765"), ("cookie", "a=b"), ("origin", "http://127.0.0.1")])),
            "gewöhnliche Kopfzeilen eines Browsers zählen nicht"
        );
        for name in ["forwarded", "x-forwarded-for", "x-forwarded-host", "x-forwarded-proto", "x-real-ip", "via"] {
            assert!(forwarded(&headers(&[(name, "127.0.0.1")])), "{name} allein genügt");
            assert!(forwarded(&headers(&[(name, "")])), "{name} zählt auch leer – der Inhalt wird nicht geglaubt");
        }
        assert_eq!(FORWARDED_HEADERS.len(), 6, "genau die sechs Kopfzeilen der Entscheidung 0.2");
    }

    #[test]
    fn same_origin_and_non_browser_pass() {
        assert!(check(&headers(&[])).is_ok(), "curl ohne Origin");
        assert!(check(&headers(&[("host", "127.0.0.1:8765"), ("origin", "http://127.0.0.1:8765")])).is_ok());
        assert!(check(&headers(&[("sec-fetch-site", "same-origin"), ("host", "localhost:8765")])).is_ok());
        assert!(check(&headers(&[("sec-fetch-site", "none")])).is_ok());
    }

    #[test]
    fn foreign_sites_are_refused() {
        let evil = [("host", "127.0.0.1:8765"), ("origin", "https://evil.example")];
        assert!(check(&headers(&evil)).is_err());
        assert!(check(&headers(&[("host", "127.0.0.1:8765"), ("origin", "null")])).is_err());
        assert!(check(&headers(&[("host", "127.0.0.1:8765"), ("origin", "http://127.0.0.1:9999")])).is_err());
        assert!(check(&headers(&[("sec-fetch-site", "cross-site")])).is_err());
        assert!(check(&headers(&[("sec-fetch-site", "same-site")])).is_err(), "anderer Port auf localhost");
    }

    #[test]
    fn a_foreign_page_cannot_read_as_a_subresource_but_may_navigate() {
        use axum::http::Method;
        let foreign_image =
            [("sec-fetch-site", "cross-site"), ("sec-fetch-mode", "no-cors"), ("sec-fetch-dest", "image")];
        assert!(
            cross_site_subresource(&Method::GET, &headers(&foreign_image)),
            "<img> einer fremden Seite ist eine Unterressource: abgelehnt"
        );
        let other_port = [("sec-fetch-site", "same-site"), ("sec-fetch-mode", "no-cors"), ("sec-fetch-dest", "image")];
        assert!(
            cross_site_subresource(&Method::GET, &headers(&other_port)),
            "same-site (anderer Port auf localhost) ist ebenso fremd"
        );
        let fetch = [("sec-fetch-site", "cross-site"), ("sec-fetch-mode", "cors"), ("sec-fetch-dest", "empty")];
        assert!(cross_site_subresource(&Method::GET, &headers(&fetch)), "fetch einer fremden Seite: abgelehnt");
        assert!(cross_site_subresource(&Method::HEAD, &headers(&fetch)), "HEAD zählt wie GET");
        let frame = [("sec-fetch-site", "cross-site"), ("sec-fetch-mode", "navigate"), ("sec-fetch-dest", "iframe")];
        assert!(
            cross_site_subresource(&Method::GET, &headers(&frame)),
            "Navigation in einem Rahmen ist keine des obersten Fensters: abgelehnt"
        );
        assert!(
            cross_site_subresource(
                &Method::GET,
                &headers(&[("sec-fetch-site", "cross-site"), ("sec-fetch-mode", "no-cors")])
            ),
            "Mode ohne Dest (ältere Browser): keine belegte Navigation, also abgelehnt"
        );
        assert!(
            cross_site_subresource(
                &Method::GET,
                &headers(&[("sec-fetch-site", "cross-site"), ("sec-fetch-dest", "image")])
            ),
            "Dest ohne Mode: ebenso abgelehnt"
        );
        assert!(
            !cross_site_subresource(&Method::GET, &headers(&[("sec-fetch-site", "cross-site")])),
            "fehlen Mode und Dest beide, ist die Art nicht belegt: kein Browser schickt Site ohne Mode; die Routen mit \
             eigener Prüfung (cross_site_get) lehnen selbst ab"
        );

        let link = [("sec-fetch-site", "cross-site"), ("sec-fetch-mode", "navigate"), ("sec-fetch-dest", "document")];
        assert!(
            !cross_site_subresource(&Method::GET, &headers(&link)),
            "ein Link von einer fremden Seite (navigate + document) öffnet Baruch weiter"
        );
        let own = [("sec-fetch-site", "same-origin"), ("sec-fetch-mode", "no-cors"), ("sec-fetch-dest", "image")];
        assert!(!cross_site_subresource(&Method::GET, &headers(&own)), "eigene Bilder (same-origin) gehen durch");
        let typed = [("sec-fetch-site", "none"), ("sec-fetch-mode", "navigate"), ("sec-fetch-dest", "document")];
        assert!(!cross_site_subresource(&Method::GET, &headers(&typed)), "Adresse selbst eingegeben (none)");
        assert!(
            !cross_site_subresource(&Method::GET, &headers(&[])),
            "ohne Sec-Fetch-Site (curl, Tests, MCP-Clients) ist es kein Browser einer fremden Seite"
        );
        assert!(
            !cross_site_subresource(&Method::POST, &headers(&foreign_image)),
            "nur lesende Methoden; schreibende prüft same_origin"
        );
    }

    #[test]
    fn only_the_files_of_the_epub_preview_are_left_to_their_own_route() {
        assert!(epub_reader_path("/werke/3/epub/leser/kindle/OEBPS/kapitel-1.xhtml"), "Datei der Vorschau");
        for path in ["/werke/3/epub/vorschau", "/werke/3/epub", "/werke/x/epub/leser/a", "/werke//epub/leser/a", "/"] {
            assert!(!epub_reader_path(path), "{path} ist keine Datei der Vorschau und bleibt hinter own_reads");
        }
    }

    #[test]
    fn only_ip_addresses_and_localhost_are_allowed_hosts() {
        for host in
            ["127.0.0.1:8765", "127.0.0.1", "localhost:8765", "LOCALHOST", "[::1]:8765", "[::1]", "192.168.1.5:8765"]
        {
            assert!(allowed_host(host), "{host} kann nicht umgebunden werden");
        }
        for host in [
            "rebind.example:8765",
            "rebind.example",
            "localhost.evil.example",
            "127.0.0.1.nip.io",
            "[::1",
            "[::1]x",
            "localhost:abc",
            "",
        ] {
            assert!(!allowed_host(host), "{host} ist ein Name oder unlesbar");
        }
    }

    #[test]
    fn allowed_hosts_parse_a_list_and_normalise_names() {
        assert_eq!(AllowedHosts::parse(""), Ok(AllowedHosts::default()), "leer: keine zusätzlichen Namen");
        assert_eq!(AllowedHosts::parse(" \t, ,"), Ok(AllowedHosts::default()), "nur Trenner: ebenfalls leer");
        let hosts = AllowedHosts::parse("Baruch.Example.org, buch.example.net\tbaruch  baruch.example.org").unwrap();
        assert_eq!(
            hosts.names(),
            ["baruch.example.org", "buch.example.net", "baruch"],
            "Komma und Leerraum trennen, Namen klein, doppelte nur einmal, ein Name ohne Punkt (Intranet) gilt"
        );
        let long_label = "a".repeat(63);
        assert!(AllowedHosts::parse(&format!("{long_label}.example")).is_ok(), "ein Label darf 63 Zeichen haben");
        let longest = format!("{0}.{0}.{0}.{1}", "a".repeat(63), "b".repeat(61));
        assert_eq!(longest.len(), 253, "Probe hat genau die Höchstlänge");
        assert!(AllowedHosts::parse(&longest).is_ok(), "253 Zeichen sind die Höchstlänge und noch gültig");
    }

    #[test]
    fn allowed_hosts_refuse_invalid_names_and_say_which() {
        let cases: [(&str, &str); 15] = [
            ("*.example.org", "Platzhalter"),
            ("*", "Platzhalter"),
            ("baruch.example.org:8443", "ohne Port"),
            ("[::1]", "ohne Port und ohne IPv6"),
            ("192.168.1.5", "gelten ohnehin"),
            ("localhost", "gelten ohnehin"),
            ("127.1", "letzte Teil ist eine Zahl"),
            ("0x7f.1.0x1", "letzte Teil ist eine Zahl"),
            ("baruch..example", "1 bis 63 Zeichen"),
            (".example.org", "1 bis 63 Zeichen"),
            ("example.org.", "1 bis 63 Zeichen"),
            ("bücher.example", "nur Buchstaben a–z"),
            ("baruch_intern.example", "nur Buchstaben a–z"),
            ("-baruch.example", "Bindestrich beginnen oder enden"),
            ("https://baruch.example.org", "ohne Port"),
        ];
        for (name, reason) in cases {
            let error = AllowedHosts::parse(name).expect_err(&format!("{name} ist kein gültiger Rechnername"));
            assert!(error.contains(&format!("„{name}“")), "die Meldung nennt den ungültigen Namen {name}: {error}");
            assert!(error.contains(reason), "die Meldung zu {name} nennt den Grund ({reason}): {error}");
        }
        let error = AllowedHosts::parse(&format!("{}.example", "a".repeat(64))).expect_err("Label mit 64 Zeichen");
        assert!(error.contains("1 bis 63 Zeichen"), "ein Label über 63 Zeichen ist abgelehnt: {error}");
        let longer = format!("{0}.{0}.{0}.{1}", "a".repeat(63), "b".repeat(62));
        let error = AllowedHosts::parse(&longer).expect_err("254 Zeichen");
        assert!(error.contains("höchstens 253 Zeichen"), "ein Name über 253 Zeichen ist abgelehnt: {error}");
        let error = AllowedHosts::parse("baruch.example.org, *.example.org").expect_err("ein ungültiger genügt");
        assert!(
            error.contains("„*.example.org“") && !error.contains("„baruch.example.org“"),
            "ein ungültiger Name verwirft die ganze Liste, genannt ist nur er: {error}"
        );
    }

    #[test]
    fn allowed_hosts_allow_exactly_their_names_with_any_port() {
        let hosts = AllowedHosts::parse("baruch.example.org").unwrap();
        for host in ["baruch.example.org", "baruch.example.org:8443", "BARUCH.Example.ORG:443"] {
            assert!(hosts.allows(host), "{host}: erlaubter Name, Port beliebig, Groß/klein egal");
            assert!(hosts.is_extra(host), "{host}: ein zusätzlicher Name (über den Proxy)");
        }
        for host in ["127.0.0.1:8765", "localhost", "[::1]:8765", "192.168.1.5"] {
            assert!(hosts.allows(host), "{host}: IP-Adresse und localhost gelten weiter");
            assert!(!hosts.is_extra(host), "{host}: kein zusätzlicher Name, also weiter lokal");
        }
        for host in [
            "rebind.example",
            "x.baruch.example.org",
            "baruch.example.org.evil.example",
            "baruch.example.org:",
            "baruch.example.org:abc",
            "baruch.example.org:123456",
            "baruch.example.org.",
            "baruch.example.org evil.example",
            " baruch.example.org",
            "*.example.org",
            "",
        ] {
            assert!(!hosts.allows(host), "„{host}“ ist nicht genau der erlaubte Name und bleibt abgelehnt");
            assert!(!hosts.is_extra(host), "„{host}“ ist kein zusätzlicher Name");
        }
        let none = AllowedHosts::default();
        assert!(!none.allows("baruch.example.org"), "ohne --allow-host bleibt jeder Name abgelehnt");
        assert!(!none.is_extra("baruch.example.org"), "ohne --allow-host gibt es keinen zusätzlichen Namen");
    }

    #[test]
    fn a_request_under_an_extra_name_is_never_local() {
        let hosts = AllowedHosts::parse("baruch.example.org").unwrap();
        assert!(via_proxy(&hosts, &headers(&[("host", "baruch.example.org")])), "zusätzlicher Name: über den Proxy");
        assert!(via_proxy(&hosts, &headers(&[("host", "baruch.example.org:8443")])), "auch mit Port");
        assert!(!via_proxy(&hosts, &headers(&[("host", "127.0.0.1:8765")])), "IP-Adresse: weiter lokal");
        assert!(
            via_proxy(&hosts, &headers(&[])),
            "sind Namen erlaubt, steht ein Proxy davor: ohne Host ist nicht belegt, dass die Anfrage lokal ist"
        );
        let none = AllowedHosts::default();
        assert!(!via_proxy(&none, &headers(&[])), "ohne --allow-host bleibt eine Anfrage ohne Host lokal (wie bisher)");
        assert!(
            !via_proxy(&none, &headers(&[("host", "baruch.example.org")])),
            "ohne --allow-host gibt es keinen Proxy-Namen (die Anfrage scheitert ohnehin an local_host)"
        );
    }

    #[test]
    fn json_content_type() {
        assert!(is_json(&headers(&[("content-type", "application/json")])));
        assert!(is_json(&headers(&[("content-type", "Application/JSON; charset=utf-8")])));
        assert!(!is_json(&headers(&[("content-type", "text/plain")])));
        assert!(!is_json(&headers(&[])));
    }
}
