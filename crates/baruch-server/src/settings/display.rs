// SPDX-License-Identifier: AGPL-3.0-or-later
//! Settings page „Darstellung“: values in cookies of this browser (`baruch_theme`, `baruch_editor`,
//! `baruch_pv`, `baruch_accordion`, `baruch_left`/`baruch_right`). The form
//! `POST /einstellungen/darstellung/act` sets them in the format of `widgets.ts` `store`
//! (`baruch_<n>=<v>; Path=/; Max-Age=31536000; SameSite=Lax`, deleting with `Max-Age=0`) and redirects back with
//! 303; an invalid value → 422 with the page, then no cookie is set.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::extract::{Form, State};
use axum::http::{HeaderMap, HeaderValue, header};
use axum::response::{IntoResponse, Redirect, Response};
use baruch_store::WorkId;
use minijinja::context;
use serde::Deserialize;
use serde_json::{Value as Json, json};

use crate::AppState;
use crate::view::Settings;

/// Theme for display and RPC: `system` (no cookie), `light`, `dark`.
fn theme_of(settings: &Settings) -> &'static str {
    settings.theme.unwrap_or("system")
}

/// Chosen language of the interface for the form: `en`, `de` or `auto` (nothing chosen, like the browser).
fn ui_lang_setting() -> &'static str {
    crate::i18n::chosen().map_or("auto", crate::i18n::Lang::code)
}

/// `settings.get {page: "darstellung"}`: the cookies of the request.
pub(super) fn values(settings: &Settings) -> Json {
    json!({
        "theme": theme_of(settings), "editor": settings.editor, "pv": settings.pv, "accordion": settings.accordion,
        "widths": { "left": settings.left, "right": settings.right },
        // Round 39a: the chosen language of the interface (setting or cookie), `auto` = like the browser.
        "lang": ui_lang_setting(),
    })
}

pub(super) async fn render(
    _state: &AppState,
    work: Option<WorkId>,
    query: &BTreeMap<String, String>,
) -> Result<String, String> {
    let info = super::request();
    let s = &info.settings;
    let notice = query.contains_key("ok").then(|| crate::i18n::tc("settings_msg.display.saved", &[]));
    let width = |w: Option<f32>| w.map(|w| format!("{w}\u{a0}%"));
    super::render_template(
        "settings/darstellung.html",
        context! {
            work => work, notice => notice, error => info.error,
            theme => theme_of(s), editor => s.editor, pv => s.pv, accordion => s.accordion,
            left => width(s.left), right => width(s.right),
            ui_lang_setting => ui_lang_setting(),
        },
    )
}

/// Fields of the form; each is optional, only named ones are set.
#[derive(Deserialize, Default)]
pub(super) struct ActForm {
    work: Option<String>,
    theme: Option<String>,
    editor: Option<String>,
    pv: Option<String>,
    accordion: Option<String>,
    reset_widths: Option<String>,
}

/// A cookie in the format of `widgets.ts` `store`; `None` = delete.
fn cookie(name: &str, value: Option<&str>) -> String {
    match value {
        Some(value) => format!("baruch_{name}={value}; Path=/; Max-Age=31536000; SameSite=Lax"),
        None => format!("baruch_{name}=; Path=/; Max-Age=0; SameSite=Lax"),
    }
}

/// Cookies for the form fields; `Err` names the first invalid field (then nothing is set).
pub(crate) fn cookies_for(
    theme: Option<&str>,
    editor: Option<&str>,
    pv: Option<&str>,
    accordion: Option<&str>,
    reset_widths: bool,
) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    match theme {
        None => {}
        Some("system") => out.push(cookie("theme", None)),
        Some(v @ ("light" | "dark")) => out.push(cookie("theme", Some(v))),
        Some(_) => return Err(crate::i18n::tc("settings_msg.display.theme", &[])),
    }
    match editor {
        None => {}
        Some(v @ ("satz" | "quelltext")) => out.push(cookie("editor", Some(v))),
        Some(_) => return Err(crate::i18n::tc("settings_msg.display.editor", &[])),
    }
    match pv {
        None => {}
        // Open is the default: then no cookie is needed.
        Some("open") => out.push(cookie("pv", None)),
        Some("closed") => out.push(cookie("pv", Some("closed"))),
        Some(_) => return Err(crate::i18n::tc("settings_msg.display.proposals", &[])),
    }
    match accordion {
        None => {}
        Some(v @ ("chat" | "personas" | "revisions")) => out.push(cookie("accordion", Some(v))),
        Some(_) => return Err(crate::i18n::tc("settings_msg.display.accordion", &[])),
    }
    if reset_widths {
        out.push(cookie("left", None));
        out.push(cookie("right", None));
    }
    Ok(out)
}

/// `POST /einstellungen/darstellung/act`: `theme=system|light|dark`, `editor=satz|quelltext`,
/// `pv=open|closed`, `accordion=chat|personas|revisions`, `reset_widths=1` (each optional) → Set-Cookie,
/// 303 `/einstellungen/darstellung?ok=1[&work=W]`; invalid → 422 with the page.
pub(super) async fn act(State(state): State<Arc<AppState>>, headers: HeaderMap, Form(form): Form<ActForm>) -> Response {
    let work = form.work.as_deref().and_then(|w| w.trim().parse::<WorkId>().ok()).filter(|&w| w > 0);
    let pick = |v: &Option<String>| v.as_deref().map(str::trim).filter(|v| !v.is_empty()).map(str::to_owned);
    let (theme, editor, pv, accordion) = (pick(&form.theme), pick(&form.editor), pick(&form.pv), pick(&form.accordion));
    let reset = form.reset_widths.as_deref().is_some_and(|v| matches!(v, "1" | "on"));
    let cookies = match cookies_for(theme.as_deref(), editor.as_deref(), pv.as_deref(), accordion.as_deref(), reset) {
        Ok(cookies) => cookies,
        Err(message) => return super::error_page(&state, &headers, "darstellung", work, &message).await,
    };
    let mut response =
        Redirect::to(&format!("/einstellungen/darstellung?ok=1{}", super::work_suffix(work))).into_response();
    for value in cookies {
        // The values come only from the fixed lists above; an invalid header is ruled out.
        if let Ok(value) = HeaderValue::from_str(&value) {
            response.headers_mut().append(header::SET_COOKIE, value);
        }
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cookies_follow_the_store_format_of_widgets_ts() {
        let all = cookies_for(Some("dark"), Some("quelltext"), Some("closed"), Some("personas"), true).unwrap();
        assert_eq!(
            all,
            [
                "baruch_theme=dark; Path=/; Max-Age=31536000; SameSite=Lax",
                "baruch_editor=quelltext; Path=/; Max-Age=31536000; SameSite=Lax",
                "baruch_pv=closed; Path=/; Max-Age=31536000; SameSite=Lax",
                "baruch_accordion=personas; Path=/; Max-Age=31536000; SameSite=Lax",
                "baruch_left=; Path=/; Max-Age=0; SameSite=Lax",
                "baruch_right=; Path=/; Max-Age=0; SameSite=Lax",
            ],
            "Format wie store.set in widgets.ts, Löschen mit Max-Age=0"
        );
        assert_eq!(
            cookies_for(Some("system"), None, Some("open"), None, false).unwrap(),
            ["baruch_theme=; Path=/; Max-Age=0; SameSite=Lax", "baruch_pv=; Path=/; Max-Age=0; SameSite=Lax"],
            "System und offen sind Vorgaben: Cookie löschen"
        );
        assert!(cookies_for(None, None, None, None, false).unwrap().is_empty(), "nichts genannt, nichts gesetzt");
        for bad in [
            cookies_for(Some("blau"), None, None, None, false),
            cookies_for(None, Some("x"), None, None, false),
            cookies_for(None, None, Some("halb"), None, false),
            cookies_for(None, None, None, Some("ideas"), false),
        ] {
            assert!(bad.is_err(), "ungültiger Wert abgelehnt: {bad:?}");
        }
        // The accordion has three sections (fragments/assistants.html, fragments/revisions.html); each
        // must be selectable, also „Fassungen“, which view.rs reads as baruch_accordion=revisions.
        assert_eq!(
            cookies_for(None, None, None, Some("revisions"), false).unwrap(),
            ["baruch_accordion=revisions; Path=/; Max-Age=31536000; SameSite=Lax"],
            "„Fassungen“ ist als offener Bereich rechts wählbar"
        );
    }
}
