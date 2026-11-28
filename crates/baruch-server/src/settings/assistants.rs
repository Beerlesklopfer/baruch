// SPDX-License-Identifier: AGPL-3.0-or-later
//! Settings page „Assistenten“: choice of the lector (fixed one persona or automatically by context;
//! default of the trial since 2026-11-06: fixed `@lektor`), per persona colour, role, model (selection from
//! the list of the model backend through `<datalist>`, free text stays), context length, active and the
//! role prompt (default from the code or own text, resettable). Warnings: model unknown at the backend,
//! old name with an alias, active personas on different models (VRAM). Forms to
//! `POST /einstellungen/personas/act` ([`crate::personas`]), usable without JavaScript.
//!
//! Round 11: one tab per persona (`?persona=<handle without @>`, unknown → the first with a hint; with
//! JavaScript `web/src/persona_tabs.ts`), in the tab **one** form `action=save` with all fields (name,
//! colour, role as a text field, model, context, active, role prompt large) and „Vorgabe wiederherstellen“
//! (`action=reset_prompt`, confirmation question through `?rueckfrage=vorgabe`). After a rejected „Speichern“
//! the page shows the tab with the input ([`crate::personas::retry`]).
//!
//! Round 12: in the tab of `@belege` and of the lector a form of its own (`action=rag`, checkbox `on`)
//! „Treffer der Bedeutungssuche als Belege mitgeben“ ([`crate::assist::rag_facts`]).
//!
//! Round 13: in the tab of `@belege` the switch of the web search (form to `POST /websuche/act`, `action=switch`,
//! checkbox `on`, default off; [`crate::websearch`]); `settings.get` names it as `websearch: {on}`.
//!
//! Round 15b: in the tab per persona the form `action=backend` (selection of the model connection, checkbox
//! `external_allowed` „Buchtext an externe Verbindung schicken“, default off) with the display „lokal“/„extern“,
//! a warning if an external connection is chosen without a release, and the effective context length
//! (reconciliation with the remote side, live only for the shown tab, otherwise from the cache); in the tab of
//! `@belege` and of the lector the switch `action=bible` „Bibel-Belege aus SWORD mitgeben“ (default: @belege
//! on, lector off). The model list belongs to the connection of the shown tab.

use std::collections::{BTreeMap, BTreeSet};

use baruch_core::LectorChoice;
use baruch_store::{PERSONA_PALETTE, WorkId};
use minijinja::value::Serde;
use minijinja::{Value, context};
use serde_json::{Value as Json, json};

use crate::AppState;
use crate::personas::{rows_and_lector_cached, style};
use crate::rpc::{INVALID_PARAMS, RpcError};

fn notice(query: &BTreeMap<String, String>) -> Option<String> {
    let key = match query.get("ok").map(String::as_str) {
        Some("farbe") => "settings_msg.assistants.color",
        Some("persona") => "settings_msg.assistants.persona",
        Some("lektor") => "settings_msg.assistants.lector",
        Some("prompt") => "settings_msg.assistants.prompt",
        Some("vorgabe") => "settings_msg.assistants.prompt_default",
        Some("rag") => "settings_msg.assistants.rag",
        Some("websuche") => "settings_msg.assistants.websearch",
        Some("verbindung") => "settings_msg.assistants.connection",
        Some("bibel") => "settings_msg.assistants.bible",
        Some("chat") => "settings_msg.assistants.chat",
        _ => return None,
    };
    Some(crate::i18n::tc(key, &[]))
}

/// State of a model name compared with the list of the backend: `bekannt`, `alias` (with target) or
/// `unbekannt`; without a list `None`.
pub fn model_state(
    model: &str,
    list: Option<&[String]>,
    alias: Option<String>,
) -> Option<(&'static str, Option<String>)> {
    let list = list?;
    if list.iter().any(|m| m == model) {
        Some(("bekannt", None))
    } else if let Some(target) = alias {
        Some(("alias", Some(target)))
    } else {
        Some(("unbekannt", None))
    }
}

pub(super) async fn render(
    state: &AppState,
    work: Option<WorkId>,
    query: &BTreeMap<String, String>,
) -> Result<String, String> {
    let info = super::request();
    // Round 16a: the lector with the effective context length (as for the jobs); Round 17a (L4): on page load
    // without a network query – context and model list from the cache. The page asks the remote side only on
    // explicit wish (`?modelle=abfragen`, link „Modellliste abfragen“).
    let (rows, choice, lector_handle) = rows_and_lector_cached(state).await.map_err(|e| e.message)?;
    let lector = lector_handle.and_then(|h| rows.iter().find(|p| p.handle == h)).cloned();
    let lector_budget = match &lector {
        Some(l) => Some(state.model.context_cached(state, l).await),
        None => None,
    };
    let chat_scope = state.store.chat_scope().await.map_err(|e| e.to_string())?.word();
    let fetch = query.get("modelle").is_some_and(|m| m == "abfragen");
    // Round 11: one tab per persona; chosen through `?persona=<handle without @>` (unknown → the first with a
    // hint). After a rejected „Speichern“ the tab of the input applies, with the input.
    let retry = crate::personas::retry();
    let asked = retry
        .as_ref()
        .map(|r| r.handle.clone())
        .or_else(|| query.get("persona").cloned())
        .map(|h| h.trim().trim_start_matches('@').to_owned())
        .filter(|h| !h.is_empty());
    let found = asked.as_deref().and_then(|h| rows.iter().position(|p| p.handle.trim_start_matches('@') == h));
    let selected = found.unwrap_or(0);
    let unknown_persona = asked.filter(|_| found.is_none() && !rows.is_empty());
    let mut rag_on = Vec::with_capacity(rows.len());
    for p in &rows {
        rag_on.push(state.store.persona_rag(&p.handle).await.map_err(|e| e.to_string())?);
    }
    // Round 15b: connection per persona, local/external, release, context and Bible evidence.
    let connections = state.store.model_connection_list().await.map_err(|e| e.to_string())?;
    let backends = state.store.persona_backends().await.map_err(|e| e.to_string())?;
    let mut extra: Vec<Value> = Vec::with_capacity(rows.len());
    for (index, p) in rows.iter().enumerate() {
        let backend = backends.iter().find(|(h, _)| *h == p.handle).map(|(_, b)| *b).unwrap_or_default();
        let connection = connections.iter().find(|c| c.id == backend.connection).or(connections.first());
        // Without network (Round 17a); only „Modellliste abfragen“ asks the remote side for the shown tab.
        let budget = if fetch && index == selected {
            state.model.context(state, p).await
        } else {
            state.model.context_cached(state, p).await
        };
        let (bible_allowed, bible_on) = match crate::personas::bible_state(state, &p.handle).await {
            Ok((_, allowed, on, _)) => (allowed, on),
            Err(e) => return Err(e.message),
        };
        let external = connection.is_some_and(|c| c.external);
        let options: Vec<Value> = connections
            .iter()
            .map(|c| {
                context! { id => c.id, name => c.name.as_str(), kind => crate::models::kind_title(&c.kind),
                locality => crate::models::locality(c),
                locality_label => crate::models::locality_label(crate::models::locality(c)),
                selected => c.id == backend.connection }
            })
            .collect();
        extra.push(context! {
            connections => options,
            connection_name => connection.map(|c| c.name.clone()),
            locality => connection.map_or("lokal", crate::models::locality),
            locality_label => crate::models::locality_label(connection.map_or("lokal", crate::models::locality)),
            external => external, external_allowed => backend.external_allowed,
            blocked => external && !backend.external_allowed,
            context_tokens => budget.tokens, context_reported => budget.reported, context_warning => budget.warning,
            bible_allowed => bible_allowed, bible_context => bible_on,
            bible_default => crate::personas::bible_default(&p.handle),
        });
    }
    // Model list of the connection of the shown tab (Eliezer through the port, others directly).
    let shown_connection = rows
        .get(selected)
        .and_then(|p| backends.iter().find(|(h, _)| *h == p.handle).map(|(_, b)| b.connection))
        .unwrap_or(baruch_store::ELIEZER_CONNECTION);
    let (models, models_note) = match connections.iter().find(|c| c.id == shown_connection) {
        Some(c) if fetch && c.id != baruch_store::ELIEZER_CONNECTION => match crate::models::list_on(state, c).await {
            Ok(list) => (Some(list.models), list.message),
            Err(e) => (None, Some(e.to_string())),
        },
        _ if fetch => match state.model.models(state).await {
            Ok(list) => (Some(list.models), list.message),
            Err(e) => (None, Some(e.to_string())),
        },
        Some(c) => match crate::models::cached_list_on(state, c).await {
            Some(list) => {
                let stamp = list.fetched.to_string();
                (
                    Some(list.models),
                    Some(crate::i18n::tc("settings_msg.assistants.cached_list", &[("stamp", stamp.into())])),
                )
            }
            None => (None, None),
        },
        None => (None, None),
    };
    let models_pending = models.is_none() && models_note.is_none();
    let lector_handle = lector.as_ref().map(|l| l.handle.clone());
    let websearch_on = crate::websearch::enabled(state).await.map_err(|e| e.to_string())?;
    let websearch_configured = crate::websearch::configured(state).await;
    let personas: Vec<Value> = rows
        .iter()
        .enumerate()
        .map(|(index, p)| {
            let known = model_state(&p.model, models.as_deref(), state.model.alias(&p.model));
            let prompt = crate::assist::prompts::effective_prompt(&p.handle, &p.role, p.prompt.as_deref());
            let mine = retry.as_ref().filter(|r| r.handle.trim_start_matches('@') == p.handle.trim_start_matches('@'));
            let field = |typed: Option<&String>, stored: String| typed.cloned().unwrap_or(stored);
            context! {
                handle => p.handle.as_str(), anchor => p.handle.trim_start_matches('@'),
                // Stored name for tab and heading; `name_value` for the field (the input, if any).
                name => p.name.as_str(), name_value => field(mine.map(|r| &r.name), p.name.clone()),
                role => field(mine.map(|r| &r.role), p.role.clone()),
                model => field(mine.map(|r| &r.model), p.model.clone()),
                context_length => field(mine.map(|r| &r.context_length), p.context_length.to_string()),
                active => mine.map_or(p.active, |r| r.active), stored_active => p.active,
                color => p.color.as_str(), chosen => p.chosen, style => style(&p.color),
                color_text => mine.map(|r| r.color.clone()).filter(|c| baruch_store::valid_color(c).is_none()),
                lector => lector.as_ref().is_some_and(|l| l.handle == p.handle),
                model_state => known.as_ref().map(|k| k.0), model_target => known.and_then(|k| k.1),
                prompt => field(mine.map(|r| &r.prompt), prompt), prompt_custom => p.prompt.is_some(),
                selected => index == selected, tab_q => crate::view::query_escape(p.handle.trim_start_matches('@')),
                // Short description for the tab title.
                summary => format!("{} · {}", p.role, p.model),
                // Round 12: hits of the meaning search (only @belege and the lector).
                rag_allowed => crate::assist::rag_facts::allowed(&p.handle, lector_handle.as_deref()),
                rag_context => rag_on[index],
                // Round 13: web search (only @belege).
                web_allowed => crate::assist::web_facts::allowed(&p.handle, crate::assist::Mode::Ask),
                // Round 15b: connection, local/external, release, context, Bible evidence.
                backend => extra[index].clone(),
            }
        })
        .collect();
    let confirm_reset = query.get("rueckfrage").is_some_and(|r| r == "vorgabe");
    let active_models: BTreeSet<&str> = rows.iter().filter(|p| p.active).map(|p| p.model.as_str()).collect();
    let fixed = match &choice {
        LectorChoice::Fixed(handle) => Some(handle.as_str()),
        LectorChoice::Auto => None,
    };
    super::render_template(
        "settings/assistenten.html",
        context! {
            work => work, notice => notice(query), error => info.error, personas => personas,
            unknown_persona => unknown_persona, confirm_reset => confirm_reset,
            lector => lector.map(|l| context! { name => l.name, handle => l.handle, context_length => l.context_length,
                tokens => lector_budget.as_ref().map_or(l.context_length, |b| b.tokens),
                reported => lector_budget.as_ref().and_then(|b| b.reported),
                warning => lector_budget.as_ref().and_then(|b| b.warning.clone()) }),
            lector_mode => choice.mode(), lector_fixed => fixed, chat_scope => chat_scope,
            fixed_fallback => fixed.is_some_and(|h| !rows.iter().any(|p| p.active && p.handle.trim_start_matches('@') == h.trim_start_matches('@'))),
            palette => Value::from(Serde(PERSONA_PALETTE)),
            websearch => context! { on => websearch_on, configured => websearch_configured },
            models => models, models_note => models_note, models_pending => models_pending,
            fetch_q => format!("?persona={}&modelle=abfragen{}", rows.get(selected).map_or(String::new(), |p| crate::view::query_escape(p.handle.trim_start_matches('@'))), work.map_or(String::new(), |w| format!("&work={w}"))),
            mixed_models => (active_models.len() > 1).then(|| active_models.iter().map(|m| (*m).to_owned()).collect::<Vec<_>>()),
        },
    )
}

/// `settings.get {page: "assistenten"}` → `{personas: [PersonaRow], lector, lector_mode, lector_handle,
/// websearch: {on}}`.
pub(super) async fn values(state: &AppState) -> Result<Json, RpcError> {
    let (rows, choice, lector) = rows_and_lector_cached(state).await?;
    let on = crate::websearch::enabled(state)
        .await
        .map_err(|e| RpcError::new(crate::rpc::INTERNAL, format!("Speicher: {e}")))?;
    Ok(json!({ "personas": rows, "lector": lector, "lector_mode": choice.mode(), "lector_handle": choice.handle(),
               "websearch": { "on": on }, "chat_scope": state.store.chat_scope().await?.word() }))
}

/// `settings.set {page: "assistenten", values: {handle, role?, model?, context_length?, active?, color?}}`
/// through `persona.update` and `persona.set_color` → `{persona: PersonaRow, lector}`. First everything is
/// checked, then written: an invalid value changes nothing.
pub(super) async fn set(state: &AppState, values: Json) -> Result<Json, RpcError> {
    let update = crate::personas::update_params(&values)?;
    let handle = update.handle.clone();
    let color = crate::personas::optional_color(&values)?;
    if let Some(Some(c)) = &color
        && baruch_store::valid_color(c).is_none()
    {
        return Err(RpcError::new(INVALID_PARAMS, crate::i18n::tc("settings_msg.assistants.color_format", &[])));
    }
    let mut row = crate::personas::update(state, &update).await?;
    if let Some(color) = color {
        crate::personas::set_color(state, &handle, color.as_deref()).await?;
        let rows = state.store.persona_list().await?;
        row = rows.into_iter().find(|p| p.handle == row.handle).unwrap_or(row);
    }
    let (_, _, lector) = rows_and_lector_cached(state).await?;
    Ok(json!({ "persona": row, "lector": lector }))
}
