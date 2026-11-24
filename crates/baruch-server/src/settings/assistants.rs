// SPDX-License-Identifier: AGPL-3.0-or-later
//! Einstellungsseite „Assistenten“: Wahl des Lektors (fest eine Persona oder automatisch nach Kontext;
//! Vorgabe des Versuchs seit 2026-11-06: fest `@lektor`), je Persona Farbe, Rolle, Modell (Auswahl aus
//! der Liste des Modell-Backends über `<datalist>`, Freitext bleibt), Kontextlänge, aktiv und der
//! Rollen-Prompt (Vorgabe aus dem Code oder eigener Text, zurücksetzbar). Warnungen: Modell beim Backend
//! unbekannt, alter Name mit Alias, aktive Personas auf verschiedenen Modellen (VRAM). Formulare an
//! `POST /einstellungen/personas/act` ([`crate::personas`]), ohne JavaScript bedienbar.
//!
//! Runde 11: je Persona ein Tab (`?persona=<handle ohne @>`, unbekannt → der erste mit Hinweis; mit
//! JavaScript `web/src/persona_tabs.ts`), im Tab **ein** Formular `action=save` mit allen Feldern (Name,
//! Farbe, Rolle als Textfeld, Modell, Kontext, aktiv, Rollen-Prompt groß) und „Vorgabe wiederherstellen“
//! (`action=reset_prompt`, Rückfrage über `?rueckfrage=vorgabe`). Nach einem abgelehnten „Speichern“
//! zeigt die Seite den Tab mit der Eingabe ([`crate::personas::retry`]).
//!
//! Runde 12: im Tab von `@belege` und des Lektors ein eigenes Formular (`action=rag`, Kästchen `on`)
//! „Treffer der Bedeutungssuche als Belege mitgeben“ ([`crate::assist::rag_facts`]).
//!
//! Runde 13: im Tab von `@belege` der Schalter der Websuche (Formular an `POST /websuche/act`, `action=switch`,
//! Kästchen `on`, Vorgabe aus; [`crate::websearch`]); `settings.get` nennt ihn als `websearch: {on}`.
//!
//! Runde 15b: im Tab je Persona das Formular `action=backend` (Auswahl der Modell-Verbindung, Kästchen
//! `external_allowed` „Buchtext an externe Verbindung schicken“, Vorgabe aus) mit Anzeige „lokal“/„extern“,
//! Warnung, wenn eine externe Verbindung ohne Freigabe gewählt ist, und der wirksamen Kontextlänge
//! (Abgleich mit der Gegenstelle, nur für den gezeigten Tab live, sonst aus dem Zwischenspeicher); im Tab von
//! `@belege` und des Lektors der Schalter `action=bible` „Bibel-Belege aus SWORD mitgeben“ (Vorgabe: @belege
//! an, Lektor aus). Die Modellliste gehört zur Verbindung des gezeigten Tabs.

use std::collections::{BTreeMap, BTreeSet};

use baruch_core::LectorChoice;
use baruch_store::{PERSONA_PALETTE, WorkId};
use minijinja::value::Serde;
use minijinja::{Value, context};
use serde_json::{Value as Json, json};

use crate::AppState;
use crate::personas::{rows_and_lector_cached, style};
use crate::rpc::{INVALID_PARAMS, RpcError};

fn notice(query: &BTreeMap<String, String>) -> Option<&'static str> {
    match query.get("ok").map(String::as_str) {
        Some("farbe") => Some("Die Farbe ist gespeichert."),
        Some("persona") => Some("Die Persona ist gespeichert."),
        Some("lektor") => Some("Die Wahl des Lektors ist gespeichert."),
        Some("prompt") => Some("Der Rollen-Prompt ist gespeichert."),
        Some("vorgabe") => Some("Der Rollen-Prompt gilt wieder in der Vorgabe."),
        Some("rag") => Some("Die Einstellung zu den Treffern der Bedeutungssuche ist gespeichert."),
        Some("websuche") => Some("Die Einstellung zur Websuche von @belege ist gespeichert."),
        Some("verbindung") => Some("Verbindung und Freigabe der Persona sind gespeichert."),
        Some("bibel") => Some("Die Einstellung zu den Bibel-Belegen aus SWORD ist gespeichert."),
        Some("chat") => Some("Der Umfang des Chats ist gespeichert."),
        _ => None,
    }
}

/// Stand eines Modellnamens gegenüber der Liste des Backends: `bekannt`, `alias` (mit Ziel) oder
/// `unbekannt`; ohne Liste `None`.
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
    // Runde 16a: der Lektor mit der wirksamen Kontextlänge (wie bei den Aufträgen); Runde 17a (L4): beim Seitenladen
    // ohne Netzabfrage – Kontext und Modellliste aus dem Zwischenspeicher. Die Gegenstelle fragt die Seite nur auf
    // ausdrücklichen Wunsch (`?modelle=abfragen`, Link „Modellliste abfragen“).
    let (rows, choice, lector_handle) = rows_and_lector_cached(state).await.map_err(|e| e.message)?;
    let lector = lector_handle.and_then(|h| rows.iter().find(|p| p.handle == h)).cloned();
    let lector_budget = match &lector {
        Some(l) => Some(state.model.context_cached(state, l).await),
        None => None,
    };
    let chat_scope = state.store.chat_scope().await.map_err(|e| e.to_string())?.word();
    let fetch = query.get("modelle").is_some_and(|m| m == "abfragen");
    // Runde 11: je Persona ein Tab; gewählt über `?persona=<handle ohne @>` (unbekannt → der erste mit
    // Hinweis). Nach einem abgelehnten „Speichern“ gilt der Tab der Eingabe, mit der Eingabe.
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
    // Runde 15b: Verbindung je Persona, lokal/extern, Freigabe, Kontext und Bibel-Belege.
    let connections = state.store.model_connection_list().await.map_err(|e| e.to_string())?;
    let backends = state.store.persona_backends().await.map_err(|e| e.to_string())?;
    let mut extra: Vec<Value> = Vec::with_capacity(rows.len());
    for (index, p) in rows.iter().enumerate() {
        let backend = backends.iter().find(|(h, _)| *h == p.handle).map(|(_, b)| *b).unwrap_or_default();
        let connection = connections.iter().find(|c| c.id == backend.connection).or(connections.first());
        // Ohne Netz (Runde 17a); nur „Modellliste abfragen“ fragt für den gezeigten Tab die Gegenstelle.
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
                locality => crate::models::locality(c), selected => c.id == backend.connection }
            })
            .collect();
        extra.push(context! {
            connections => options,
            connection_name => connection.map(|c| c.name.clone()),
            locality => connection.map_or("lokal", crate::models::locality),
            external => external, external_allowed => backend.external_allowed,
            blocked => external && !backend.external_allowed,
            context_tokens => budget.tokens, context_reported => budget.reported, context_warning => budget.warning,
            bible_allowed => bible_allowed, bible_context => bible_on,
            bible_default => crate::personas::bible_default(&p.handle),
        });
    }
    // Modellliste der Verbindung des gezeigten Tabs (Eliezer über den Port, andere direkt).
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
                (Some(list.models), Some(format!("Liste aus dem Zwischenspeicher (Stand {}).", list.fetched)))
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
                // Gespeicherter Name für Tab und Überschrift; `name_value` für das Feld (ggf. die Eingabe).
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
                // Kurzbeschreibung für den Tab-Titel.
                summary => format!("{} · {}", p.role, p.model),
                // Runde 12: Treffer der Bedeutungssuche (nur @belege und der Lektor).
                rag_allowed => crate::assist::rag_facts::allowed(&p.handle, lector_handle.as_deref()),
                rag_context => rag_on[index],
                // Runde 13: Websuche (nur @belege).
                web_allowed => crate::assist::web_facts::allowed(&p.handle, crate::assist::Mode::Ask),
                // Runde 15b: Verbindung, lokal/extern, Freigabe, Kontext, Bibel-Belege.
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
/// über `persona.update` und `persona.set_color` → `{persona: PersonaRow, lector}`. Erst wird alles
/// geprüft, dann geschrieben: Ein ungültiger Wert ändert nichts.
pub(super) async fn set(state: &AppState, values: Json) -> Result<Json, RpcError> {
    let update = crate::personas::update_params(&values)?;
    let handle = update.handle.clone();
    let color = crate::personas::optional_color(&values)?;
    if let Some(Some(c)) = &color
        && baruch_store::valid_color(c).is_none()
    {
        return Err(RpcError::new(INVALID_PARAMS, "Farbe als #rrggbb angeben."));
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
