// SPDX-License-Identifier: AGPL-3.0-or-later
//! Sessions (Round 35, contract §1.2): token of 32 random bytes in the cookie [`super::SESSION_COOKIE`], stored only
//! as SHA-256; valid 30 days without use, at most 90 days.
//!
//! The token leaves the server only in the `Set-Cookie` of the login (and when renewing the cookie); the
//! database holds its SHA-256 (`baruch_store::Store::session_token_hash`). Which session the running request
//! carries is kept – like [`super::current`] – in a task-local slot ([`current_hash`]): this way a
//! password change keeps exactly this session and deletes all others.

use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::Response;
use baruch_store::{Store, StoreError};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

use super::SESSION_COOKIE;
use crate::AppState;

/// Lifetime of the cookie in the browser: 30 days (the period “without use”; the server checks both periods itself).
pub const COOKIE_MAX_AGE: u64 = 2_592_000;
/// If the previous use is longer ago, the browser gets the cookie anew (fresh 30 days).
pub const RENEW_AFTER_IDLE_SECONDS: i64 = 3600;
/// Length of a token: 32 bytes as Base64url without padding.
const TOKEN_CHARS: usize = 43;

tokio::task_local! {
    static SESSION: Option<String>;
}

/// Token hash of the session under which the running request is logged in; `None` without a session (and outside
/// of a request). Like [`super::current`] never evaluate in spawned tasks.
pub fn current_hash() -> Option<String> {
    SESSION.try_with(Clone::clone).ok().flatten()
}

/// Runs `f` under the session with this token hash.
pub async fn scope<F: std::future::Future>(hash: Option<String>, f: F) -> F::Output {
    SESSION.scope(hash, f).await
}

/// New token: 32 random bytes of the system as Base64url (43 characters).
pub fn new_token() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).map_err(|e| format!("Zufallszahlen des Systems: {e}"))?;
    Ok(URL_SAFE_NO_PAD.encode(bytes))
}

/// The token from the `Cookie` headers if it looks like one (43 characters Base64url); everything else is not
/// even looked up.
pub fn token_from(headers: &HeaderMap) -> Option<&str> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|line| line.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == SESSION_COOKIE)
        .map(|(_, token)| token)
        .filter(|token| {
            token.len() == TOKEN_CHARS && token.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        })
}

/// `Set-Cookie` of the session: `HttpOnly` (no script reads it), `SameSite=Strict` (no foreign page sends it
/// along), `Path=/`; `Secure` if the request came through the proxy (TLS ends there).
pub fn cookie(token: &str, secure: bool) -> String {
    let secure = if secure { "; Secure" } else { "" };
    format!("{SESSION_COOKIE}={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age={COOKIE_MAX_AGE}{secure}")
}

/// `Set-Cookie` that deletes the session cookie in the browser.
pub fn clear_cookie(secure: bool) -> String {
    let secure = if secure { "; Secure" } else { "" };
    format!("{SESSION_COOKIE}=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0{secure}")
}

/// Appends a `Set-Cookie` to the response.
pub fn set_cookie(response: &mut Response, cookie: &str) {
    if let Ok(value) = HeaderValue::from_str(cookie) {
        response.headers_mut().append(header::SET_COOKIE, value);
    }
}

/// New session for the account: creates it (only the hash) and returns the token for the cookie.
pub async fn start(state: &AppState, account: i64) -> Result<String, String> {
    let token = new_token()?;
    state
        .store
        .session_create(&Store::session_token_hash(&token), account)
        .await
        .map_err(|e: StoreError| format!("Sitzung: {e}"))?;
    Ok(token)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(cookies: &[&str]) -> HeaderMap {
        let mut map = HeaderMap::new();
        for line in cookies {
            map.append(header::COOKIE, HeaderValue::from_str(line).unwrap());
        }
        map
    }

    #[test]
    fn tokens_are_43_url_safe_characters_and_never_repeat() {
        let first = new_token().unwrap();
        let second = new_token().unwrap();
        assert_eq!(first.len(), 43, "32 Byte als Base64url ohne Füllung sind 43 Zeichen");
        assert!(
            first.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'),
            "nur Zeichen, die in einem Cookie ohne Anführung stehen dürfen"
        );
        assert_ne!(first, second, "jedes Token ist neu gewürfelt");
        assert_eq!(URL_SAFE_NO_PAD.decode(&first).unwrap().len(), 32, "dahinter stehen 32 Zufallsbytes");
    }

    #[test]
    fn the_token_is_found_among_other_cookies_and_malformed_ones_are_ignored() {
        let token = "A".repeat(43);
        let line = format!("baruch_work=2; baruch_session={token}; theme=dark");
        assert_eq!(token_from(&headers(&[&line])), Some(token.as_str()), "zwischen anderen Cookies gefunden");
        let second = format!("baruch_session={token}");
        assert_eq!(
            token_from(&headers(&["baruch_work=2", &second])),
            Some(token.as_str()),
            "auch in einer zweiten Cookie-Kopfzeile"
        );
        assert_eq!(token_from(&headers(&["baruch_work=2"])), None, "ohne Cookie der Sitzung keines");
        assert_eq!(token_from(&headers(&["baruch_session=kurz"])), None, "zu kurz: kein Token");
        let odd = format!("baruch_session={}", "ä".repeat(43));
        assert_eq!(token_from(&headers(&[&odd])), None, "fremde Zeichen: kein Token");
        let other = format!("xbaruch_session={token}");
        assert_eq!(token_from(&headers(&[&other])), None, "ein Cookie mit ähnlichem Namen zählt nicht");
    }

    #[test]
    fn the_cookie_carries_the_attributes_of_the_contract() {
        assert_eq!(
            cookie("TOKEN", false),
            "baruch_session=TOKEN; Path=/; HttpOnly; SameSite=Strict; Max-Age=2592000",
            "wörtlich nach Vertrag §1.2"
        );
        assert!(cookie("TOKEN", true).ends_with("; Secure"), "hinter dem Proxy zusätzlich Secure");
        assert_eq!(
            clear_cookie(false),
            "baruch_session=; Path=/; HttpOnly; SameSite=Strict; Max-Age=0",
            "Abmelden löscht das Cookie mit denselben Attributen"
        );
    }

    #[tokio::test]
    async fn the_session_of_a_request_is_only_known_inside_its_scope() {
        assert_eq!(current_hash(), None, "außerhalb einer Anfrage gibt es keine Sitzung");
        let seen = scope(Some("abc".to_owned()), async { current_hash() }).await;
        assert_eq!(seen.as_deref(), Some("abc"), "innerhalb gilt die gesetzte Sitzung");
        assert_eq!(scope(None, async { current_hash() }).await, None, "eine Anfrage ohne Sitzung hat keine");
    }
}
