// SPDX-License-Identifier: AGPL-3.0-or-later
//! Switch of the interface language (Round 39a, interface R39a-S1, contract §3.1): `POST /einstellungen/sprache`,
//! a plain form (`lang=en|de|auto`, optional `back`) that works without JavaScript – from the menu „Baruch ▾“ of
//! `base.html` and from the settings page „Darstellung“.
//!
//! Sets the cookie [`crate::i18n::COOKIE`] (one year; `auto` deletes it) and stores the choice: an account under its
//! own key, without accounts and for the author on the own machine the instance setting; an anonymous visitor only
//! gets the cookie. Then 303 back: to `back` if it is a local path, else to the page of the `Referer` if it is this
//! host, else to `/`. An invalid language → 422 with `common.lang.invalid`, nothing set.
//!
//! The route sits among the writing routes (`guard::same_origin`), is allowed for Lektors (`roles::LEKTOR_ROUTES`)
//! and public (`auth::PUBLIC_PATHS`). There is no RPC method and no MCP tool for it.

use std::sync::Arc;

use axum::Router;
use axum::extract::{Form, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::post;
use serde::Deserialize;

use crate::AppState;
use crate::auth::{self, Who};
use crate::i18n::{self, Lang};

/// Writing route of the switch (merged into the `writes` router of `lib.rs`).
pub fn writes() -> Router<Arc<AppState>> {
    Router::new().route("/einstellungen/sprache", post(switch))
}

#[derive(Deserialize, Default)]
struct SwitchForm {
    lang: Option<String>,
    back: Option<String>,
}

async fn switch(State(state): State<Arc<AppState>>, headers: HeaderMap, Form(form): Form<SwitchForm>) -> Response {
    let choice = match form.lang.as_deref().map(str::trim) {
        Some("auto") => None,
        Some(code) => match Lang::ALL.into_iter().find(|lang| lang.code() == code) {
            Some(lang) => Some(lang),
            None => return invalid(),
        },
        None => return invalid(),
    };
    let code = choice.map(Lang::code);
    let saved = match auth::current() {
        Who::Account(identity) => state.store.set_ui_language(Some(identity.id), code).await,
        Who::Open | Who::LocalAuthor => state.store.set_ui_language(None, code).await,
        Who::Anonymous => Ok(()),
    };
    if let Err(e) = saved {
        return crate::internal(e);
    }
    let cookie = match code {
        Some(code) => format!("{}={code}; Path=/; Max-Age=31536000; SameSite=Lax", i18n::COOKIE),
        None => format!("{}=; Path=/; Max-Age=0; SameSite=Lax", i18n::COOKIE),
    };
    let mut response = Redirect::to(&target(form.back.as_deref(), &headers)).into_response();
    if let Ok(value) = HeaderValue::from_str(&cookie) {
        response.headers_mut().append(header::SET_COOKIE, value);
    }
    response
}

fn invalid() -> Response {
    (StatusCode::UNPROCESSABLE_ENTITY, i18n::tc("common.lang.invalid", &[])).into_response()
}

/// A path of this server: starts with `/`, not `//` or `/\` (another host), only visible ASCII (header value).
fn local_path(path: &str) -> bool {
    path.starts_with('/')
        && !path.starts_with("//")
        && !path.starts_with("/\\")
        && path.bytes().all(|b| b.is_ascii_graphic())
}

/// Where the switch leads back to (module head).
fn target(back: Option<&str>, headers: &HeaderMap) -> String {
    if let Some(back) = back.map(str::trim).filter(|back| local_path(back)) {
        return back.to_owned();
    }
    let host = headers.get(header::HOST).and_then(|v| v.to_str().ok());
    let referer = headers.get(header::REFERER).and_then(|v| v.to_str().ok());
    if let (Some(host), Some(referer)) = (host, referer)
        && let Ok(uri) = referer.parse::<axum::http::Uri>()
        && uri.authority().is_some_and(|authority| authority.as_str().eq_ignore_ascii_case(host))
        && let Some(path) = uri.path_and_query().map(|pq| pq.as_str())
        && local_path(path)
    {
        return path.to_owned();
    }
    "/".to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(host: &str, referer: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, HeaderValue::from_str(host).unwrap());
        headers.insert(header::REFERER, HeaderValue::from_str(referer).unwrap());
        headers
    }

    #[test]
    fn the_way_back_stays_on_this_server() {
        let same = headers("127.0.0.1:8765", "http://127.0.0.1:8765/werke?work=2");
        assert_eq!(target(Some("/einstellungen/darstellung?work=2"), &same), "/einstellungen/darstellung?work=2");
        assert_eq!(target(None, &same), "/werke?work=2", "without back the page of the Referer on this host");
        assert_eq!(target(Some("//evil.example/"), &same), "/werke?work=2", "a protocol-relative back is foreign");
        assert_eq!(target(Some("/\\evil.example"), &same), "/werke?work=2", "a backslash path is foreign too");
        assert_eq!(target(Some("https://evil.example/"), &same), "/werke?work=2", "an absolute back is not local");
        let foreign = headers("127.0.0.1:8765", "http://evil.example/x");
        assert_eq!(target(None, &foreign), "/", "a Referer of another host is never followed");
        assert_eq!(target(Some("/a\u{e4}"), &HeaderMap::new()), "/", "non-ASCII cannot go into a Location header");
    }
}
