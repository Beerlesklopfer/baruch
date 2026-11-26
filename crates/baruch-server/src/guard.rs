// SPDX-License-Identifier: AGPL-3.0-or-later
//! Schutz der schreibenden Endpunkte (`POST /rpc`, `/act`, `/mcp`) gegen fremde Webseiten (CSRF).
//!
//! Ein Browser schickt bei jedem POST `Origin` und (heutige Browser) `Sec-Fetch-Site` mit. Abgelehnt
//! wird mit 403, wenn
//! - `Sec-Fetch-Site` weder `same-origin` noch `none` ist (`none`: vom Nutzer selbst ausgelöst), oder
//! - `Origin` da ist und nicht zu `Host` passt (auch `Origin: null`).
//!
//! Ohne beide Kopfzeilen (curl, Skripte, MCP-Werkzeuge) geht die Anfrage durch: Das sind keine
//! Browser, und ein Browser lässt `Origin` bei POST nicht weg. `/rpc` verlangt zusätzlich
//! `Content-Type: application/json` (415 sonst); so braucht ein fremder `fetch` einen Preflight,
//! den der Server nicht beantwortet.
//!
//! **DNS-Rebinding** (ein fremder Name zeigt plötzlich auf 127.0.0.1, `Origin` und `Host` passen dann
//! zusammen) fängt [`local_host`] ab: Für **alle** Routen, auch lesende, muss `Host` eine IP-Adresse
//! (`127.0.0.1`, `[::1]`, die Adresse im LAN …) oder `localhost` sein, sonst 421. Eine IP-Adresse lässt
//! sich nicht umbinden. Ohne `Host` (nur ohne Browser möglich) geht die Anfrage durch.
//!
//! **Hinter einem Reverse-Proxy** (Runde 24): [`AllowedHosts`] nennt zusätzlich erlaubte Rechnernamen
//! (`--allow-host`, `BARUCH_ALLOWED_HOSTS`); nur genau diese Namen (mit beliebigem Port) passieren [`local_host`],
//! jeder andere Name bleibt 421. Der Proxy muss `Host` unverändert durchreichen – [`check`] vergleicht `Origin` mit
//! `Host`. Eine Anfrage unter einem zusätzlichen Namen gilt nie als lokal ([`via_proxy`]): `/mcp` verlangt dann
//! immer einen Token, auch wenn die Gegenstelle (der Proxy) Loopback ist. `X-Forwarded-*` wertet Baruch nicht aus
//! – mit einer Ausnahme: Schon das Vorhandensein einer solchen Kopfzeile ([`forwarded`]) nimmt einer Anfrage die
//! Ausnahme „Am eigenen Rechner ohne Anmeldung“ und das Einrichten des ersten Kontos (ein Proxy auf demselben
//! Rechner, der `Host` umschreibt, sähe sonst wie der eigene Rechner aus).
//!
//! **Anmeldung** (Runde 35): Ohne Konten hat Baruch keine Anmeldung, und den Zugangsschutz stellt der Proxy
//! (`docs/Betrieb.md`). Mit Konten verlangt [`crate::auth::gate`] für Seiten, Formulare und `/rpc` eine Sitzung;
//! was eine Rolle darf, entscheidet [`crate::roles`]. Die Schichten dieses Moduls bleiben davon unberührt und
//! liegen außerhalb der Anmeldung: Eine fremde Seite, ein fremder Rechnername oder ein fremder Rechner kommt gar
//! nicht bis zur Anmeldung.
//!
//! **Lesende Anfragen fremder Seiten** (Runde 23): [`own_reads`] lehnt für **alle** Routen ein GET/HEAD ab, das eine
//! fremde Seite als Unterressource auslöst ([`cross_site_subresource`]: Bild, Skript, `fetch`, Rahmen) – 403. Eine
//! Navigation des obersten Fensters (Link von außen) geht weiter durch.
//!
//! **Andere Rechner** (Runde 10): [`remote_only_mcp`] lässt von einer Gegenstelle außerhalb von Loopback
//! nur `/mcp` durch (dort entscheidet der Token); Oberfläche, `/rpc` und Formulare bleiben dem eigenen
//! Rechner vorbehalten (403).
//!
//! **Clickjacking:** [`no_framing`] setzt auf **jede** Antwort `X-Frame-Options: DENY` und `frame-ancestors
//! 'none'` in der `Content-Security-Policy` (an eine vorhandene CSP angehängt). Sonst könnte eine fremde Seite
//! etwa den Zustimmungsdialog der Gliederung oder die Freigabe „extern“ unsichtbar einrahmen und den Nutzer
//! klicken lassen – das Formular ginge dann aus dem eingerahmten Dokument selbst ab (same-origin), und
//! [`same_origin`] griffe nicht. Einzige Ausnahme (Runde 20b, Vertrag §0.9): Die EPUB-Vorschau rahmt die Dateien
//! des Buchs selbst ein (`/werke/{id}/epub/leser/…`). Diese Antworten setzen ihre CSP selbst
//! (`sandbox allow-same-origin; frame-ancestors 'self'`); trägt eine Antwort schon `frame-ancestors`, bleibt ihre CSP unverändert und
//! `X-Frame-Options` wird `SAMEORIGIN` statt `DENY` (bei `frame-ancestors 'self'`) bzw. entfällt (jede andere eigene
//! Angabe, dann gilt allein die CSP). Alle übrigen Antworten behalten `DENY` und `frame-ancestors 'none'`.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::{ConnectInfo, Request, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::AppState;

/// Anweisung gegen das Einrahmen in der CSP.
pub const FRAME_ANCESTORS: &str = "frame-ancestors 'none'";

/// Middleware für alle Antworten: nie in einem Rahmen anzeigen (siehe Modulkopf).
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

/// GET einer fremden Seite (Runde 18a): `Sec-Fetch-Site` ist `cross-site` oder `same-site`. Fehlt der Kopf
/// (alte Browser, curl, Tests) oder ist er `same-origin`/`none`, gilt die Anfrage als eigene.
pub fn cross_site_get(headers: &HeaderMap) -> bool {
    headers
        .get("sec-fetch-site")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|site| site.eq_ignore_ascii_case("cross-site") || site.eq_ignore_ascii_case("same-site"))
}

/// Lesende Anfrage (GET/HEAD) einer fremden Seite als Unterressource (Bild, Skript, fetch, Rahmen). Runde 23.
///
/// Regel (Vertrag R23 §4): Methode GET oder HEAD, `Sec-Fetch-Site` ist `cross-site` oder `same-site`, und die Anfrage
/// ist **keine** Navigation des obersten Fensters (`Sec-Fetch-Mode: navigate` zusammen mit `Sec-Fetch-Dest: document`).
/// Ein Link von einer fremden Seite oder aus einem Mailprogramm öffnet Baruch also weiter; ein `<img src=…>`, `<script>`,
/// `fetch` oder ein Rahmen (`Sec-Fetch-Dest: iframe`) einer fremden Seite bekommt 403. Damit löst eine fremde Seite keine
/// lesende Route mit Nebenwirkung mehr aus (etwa `GET /einstellungen/assistenten?modelle=abfragen`, das die Gegenstelle
/// abfragt) und kann keine Antwort vermessen. Ohne `Sec-Fetch-Site` (curl, Tests, MCP-Clients, alte Browser) und bei
/// `same-origin`/`none` gilt die Anfrage als eigene.
///
/// Abgrenzung (Entscheidung web, im Bericht an den Integrator gemeldet): Fehlen **beide** Kopfzeilen `Sec-Fetch-Mode`
/// und `Sec-Fetch-Dest`, ist die Art der Anfrage nicht belegt; sie gilt dann nicht als Unterressource. Jeder Browser,
/// der `Sec-Fetch-Site` schickt, schickt auch `Sec-Fetch-Mode` (Fetch Metadata) – eine echte Unterressource fällt also
/// nie in diese Lücke. Die Routen mit eigener Prüfung ([`cross_site_get`]: PDF, Satz, Umschlag, EPUB, Suche mit
/// Vorschau, Volltext) lehnen eine solche Anfrage weiter selbst ab; ältere Tests bilden so den „Link von einer fremden
/// Seite“ nach.
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

/// Middleware für alle Routen: lehnt `cross_site_subresource` mit 403 ab.
///
/// Einzige Ausnahme: die Dateien der EPUB-Vorschau (`/werke/{id}/epub/leser/…`). Sie stehen in einem Rahmen mit
/// `sandbox` (Runde 20b); der Browser meldet Bilder, Stil und Sprünge aus diesem Rahmen als `cross-site`, obwohl sie
/// vom eigenen Buch kommen. Dort entscheidet weiter die Route selbst (`epub_preview::from_own_frame` über `Referer`).
pub async fn own_reads(request: Request, next: Next) -> Response {
    if cross_site_subresource(request.method(), request.headers()) && !epub_reader_path(request.uri().path()) {
        return (StatusCode::FORBIDDEN, "Abgelehnt: Anfrage von einer fremden Seite (lesend)").into_response();
    }
    next.run(request).await
}

/// Pfad einer Datei der EPUB-Vorschau: `/werke/<Zahl>/epub/leser/<…>`.
fn epub_reader_path(path: &str) -> bool {
    path.strip_prefix("/werke/").and_then(|rest| rest.split_once('/')).is_some_and(|(id, rest)| {
        id.bytes().all(|b| b.is_ascii_digit()) && !id.is_empty() && rest.starts_with("epub/leser/")
    })
}

/// Middleware für die schreibenden Routen.
pub async fn same_origin(request: Request, next: Next) -> Response {
    match check(request.headers()) {
        Ok(()) => next.run(request).await,
        Err(why) => (StatusCode::FORBIDDEN, format!("Abgelehnt: {why}")).into_response(),
    }
}

/// Middleware für alle Routen: Von einer Gegenstelle außerhalb von Loopback (`ConnectInfo`) nur `/mcp`,
/// sonst 403. Ohne `ConnectInfo` (Tests, Unix-Socket) geht die Anfrage durch.
pub async fn remote_only_mcp(request: Request, next: Next) -> Response {
    let peer = request.extensions().get::<ConnectInfo<SocketAddr>>().map(|ConnectInfo(addr)| *addr);
    match peer {
        Some(addr) if !addr.ip().to_canonical().is_loopback() && request.uri().path() != "/mcp" => {
            (StatusCode::FORBIDDEN, "Abgelehnt: Von anderen Rechnern nur /mcp mit Token.").into_response()
        }
        _ => next.run(request).await,
    }
}

/// Zusätzlich erlaubte Rechnernamen (Betrieb hinter einem Reverse-Proxy): klein geschrieben, ohne Port.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AllowedHosts(Vec<String>);

impl AllowedHosts {
    /// Liste, getrennt durch Komma oder Leerraum. `Err` nennt den ungültigen Namen.
    ///
    /// Gültig (Vertrag R24 §4): 1–253 Zeichen, Labels zu 1–63 Zeichen aus `a–z0–9-` (nicht am Rand `-`), kein Port,
    /// kein Platzhalter (`*`), keine IP-Adresse und nicht `localhost` (beide gelten ohnehin, [`allowed_host`]). Auch
    /// ein Name, dessen letztes Label eine Zahl ist (`127.1`, `0x7f.1`), ist abgelehnt: Browser lesen ihn als
    /// IPv4-Adresse. Großbuchstaben werden klein gespeichert, doppelte Namen nur einmal.
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

    /// `allowed_host(host)` oder einer der zusätzlichen Namen (Port wie bei `allowed_host`).
    pub fn allows(&self, host: &str) -> bool {
        allowed_host(host) || self.is_extra(host)
    }

    /// Nur die zusätzlichen Namen (nicht IP-Adresse, nicht `localhost`).
    pub fn is_extra(&self, host: &str) -> bool {
        let name = match host.rsplit_once(':') {
            Some((name, port)) if is_port(port) => name,
            Some(_) => return false,
            None => host,
        };
        self.0.iter().any(|allowed| allowed.eq_ignore_ascii_case(name))
    }
}

/// Prüft einen (schon klein geschriebenen) Rechnernamen; `Err` nennt den Grund.
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

/// `Host` der Anfrage ist ein zusätzlicher Name: Die Anfrage kam über einen Proxy und gilt nie als lokal.
///
/// Sind zusätzliche Namen erlaubt, gilt auch eine Anfrage **ohne lesbare `Host`-Kopfzeile** nicht als lokal: Dann
/// steht ein Proxy vor Baruch, und eine Anfrage ohne `Host` (etwa über HTTP/2 weitergereicht) ließe sich sonst nicht
/// von einer des eigenen Rechners unterscheiden. Ohne zusätzliche Namen bleibt es bei `false` (wie vor Runde 24).
pub fn via_proxy(hosts: &AllowedHosts, headers: &HeaderMap) -> bool {
    if hosts.names().is_empty() {
        return false;
    }
    match headers.get(header::HOST).map(|h| h.to_str()) {
        Some(Ok(host)) => hosts.is_extra(host),
        Some(Err(_)) | None => true,
    }
}

/// Kopfzeilen, die ein Reverse-Proxy an eine weitergereichte Anfrage hängt.
pub const FORWARDED_HEADERS: [&str; 6] =
    ["forwarded", "x-forwarded-for", "x-forwarded-host", "x-forwarded-proto", "x-real-ip", "via"];

/// Trägt die Anfrage eine der Kopfzeilen eines Proxys ([`FORWARDED_HEADERS`])? Der Inhalt wird nicht gelesen: Schon
/// eine leere oder unlesbare Kopfzeile zählt. Eine solche Anfrage gilt für die Anmeldung nie als „eigener Rechner“
/// (Runde 35, Entscheidung 0.2) – auch dann nicht, wenn Gegenstelle und `Host` nach dem eigenen Rechner aussehen.
pub fn forwarded(headers: &HeaderMap) -> bool {
    FORWARDED_HEADERS.iter().any(|name| headers.contains_key(*name))
}

/// Middleware für alle Routen: `Host` muss [`AllowedHosts::allows`] erfüllen (Schutz vor DNS-Rebinding).
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

/// `Host` (mit oder ohne Port) ist `localhost`, eine IPv4-Adresse oder eine IPv6-Adresse in `[…]`.
/// Hostnamen sind abgelehnt, weil ein fremder Name per DNS auf diesen Rechner zeigen kann.
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

/// Prüft `Sec-Fetch-Site` und `Origin` gegen `Host`. `Err` nennt den Grund.
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

/// `Content-Type` ist `application/json` (Parameter wie `charset` erlaubt).
pub fn is_json(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"))
}

/// Antwort, wenn `is_json` fehlschlägt.
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
