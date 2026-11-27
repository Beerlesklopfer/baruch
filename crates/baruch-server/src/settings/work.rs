// SPDX-License-Identifier: AGPL-3.0-or-later
//! Settings page „Werk“: metadata and work template (form part of `work_edit.html`) and the
//! outline scheme (form part of `work_numbering.html`) of the chosen work. No copied
//! form: both blocks come with the same context construction as `/werke/{id}/bearbeiten` and
//! `/werke/{id}/nummerierung` ([`crate::works::edit_context`], [`crate::numbering::page_context`]);
//! the forms additionally carry `back=/einstellungen/werk?work=W`, so that `POST /werke/act` and
//! `POST /werke/nummerierung/act` lead back here after success.
//!
//! Round 11 (task cover): plus the groups „Buchangaben“ and „Umschlag“ from [`crate::cover`]
//! (forms to `POST /werke/buchangaben/act` and `/werke/umschlag/act`, return with `ok=buchangaben`
//! or `ok=umschlag`).
//!
//! Round 13 (task abgleich): plus the switch „nach MultiGPT hochladen“ (default off, switching on only
//! with a mandatory checkbox), form to `POST /abgleich/act` with `action=upload` and
//! `back=/einstellungen/werk?work=W` (return with `ok=hochladen_an` or `ok=hochladen_aus`). No network.
//!
//! Round 15 (item 5a): plus „Kommentare und Lexika“ – a checkbox per SWORD commentary and lexicon of the
//! module folders (several selectable), form to `POST /einstellungen/werk/bibeln`
//! (`method=bible.set_work_sources`, repeated fields `commentary` and `lexicon`; handler in
//! `connections::pages`, return `ok=kommentare` or `kommentarfehler=1`).
//!
//! Round 16b (time guard): plus the block „Predigtzeit“ (`id="zeit"`): words per minute (60–250, default 110)
//! and target time in minutes (1–240, empty = none), form to `POST /einstellungen/werk/zeit`
//! (`method=work.timing_set`; [`timing_act`], return `ok=zeit#zeit` or `zeitfehler=1#zeit`). The values
//! are only in SQLite (`work_timing`), not in the mirror.
//!
//! Round 18c (typeset view per work): plus the block „Satzbild“ (`id="satzbild"`): font, font of the headings,
//! font size, measure, text, heading and sheet colour; empty = from the work template (its values stand there as
//! placeholders). Form to `POST /einstellungen/werk/satzbild` (`method=work.sheet_set`; [`sheet_act`],
//! return `ok=satzbild#satzbild` or `satzbildfehler=1#satzbild`). SQLite only (`work_sheet`), not in the mirror;
//! applies only to the HTML typeset view, never to Typst, export or PDF.
//!
//! Round 27c (cover editor): the group „Umschlag“ carries the button „Impressum-Vorlage in die Werkvorlage
//! übernehmen“ (`POST /werke/umschlag/impressum`, return `ok=impressum`, rejected: message at the group), the
//! book details the addresses of publisher and printer; `ok=anordnung` reports a stored cover layout.
//!
//! Round 29c (book typesetting): plus the block „Satz“ (`id="satz"`, [`crate::settings::work_typeset`]) directly
//! after „Satzbild“; returns `ok=satz`, `ok=satz_zurueck`, `ok=satz_vorlage` (each `#satz`), rejected forms and
//! the preview without JavaScript through [`WorkRetry::Typeset`].
//!
//! Round 22 (fassungen): plus the block „Fassungen“ (`id="fassungen"`): amending on/off, largest change in
//! percent (1–100, default 20) and new revision after minutes (0–1440, 0 = no time limit, default 30); form to
//! `POST /einstellungen/werk/fassungen` (`method=revision.policy_set`, handler in [`crate::revisions`]), return
//! `ok=fassungen#fassungen` or `fassungenfehler=1#fassungen`. SQLite only (`work_revision_policy`), not in the
//! exchange format; every change is a log commit in the mirror.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::extract::{Form, State};
use axum::response::{IntoResponse, Redirect, Response};
use baruch_store::WorkId;
use minijinja::context;
use serde_json::{Value as Json, json};

use super::WorkRetry;
use crate::AppState;
use crate::numbering::Shown;
use crate::rpc::{INVALID_PARAMS, RpcError};

/// Notes on `ok=…` after a return.
fn notice(query: &BTreeMap<String, String>) -> (Option<&'static str>, Option<String>) {
    match query.get("ok").map(String::as_str) {
        Some("werk") => (Some("Das Werk ist gespeichert."), None),
        Some("schema") => (None, Some("Das Gliederungsschema ist übernommen.".to_owned())),
        Some("buchangaben") => (Some("Die Buchangaben sind gespeichert."), None),
        Some("umschlag") => (Some("Der Umschlag ist übernommen."), None),
        Some("impressum") => {
            (Some("Die Impressum-Vorlage ist in die Werkvorlage übernommen (neue Fassung der Vorlage)."), None)
        }
        Some("anordnung") => (Some("Die Umschlag-Anordnung ist gespeichert."), None),
        Some("hochladen_an") => (
            Some(
                "Hochladen nach MultiGPT ist eingeschaltet; es geschieht beim nächsten Abgleich (Werkzeuge → Abgleichen).",
            ),
            None,
        ),
        Some("hochladen_aus") => (Some("Hochladen nach MultiGPT ist ausgeschaltet."), None),
        // Round 29c: block „Satz“.
        Some("satz") => (Some("Die Satz-Einstellungen sind gespeichert."), None),
        Some("satz_zurueck") => {
            (Some("Die Satz-Einstellungen sind zurückgesetzt; es gilt wieder nur die Werkvorlage."), None)
        }
        Some("satz_vorlage") => (
            Some(
                "Der Kapitelanfang ist aus der Werkvorlage in die Satz-Einstellungen übernommen (neue Fassung der Vorlage).",
            ),
            None,
        ),
        _ => (None, None),
    }
}

pub(super) async fn render(
    state: &AppState,
    work: Option<WorkId>,
    query: &BTreeMap<String, String>,
) -> Result<String, String> {
    let info = super::request();
    let Some(work) = work else {
        return super::render_template("settings/werk.html", context! { error => info.error });
    };
    let back = format!("/einstellungen/werk?work={work}");
    let shown = query.get("vorlage").and_then(|v| v.trim().parse().ok());
    // After a rejected form: its input and message (otherwise it would be lost in the popup).
    let (edit_form, edit_error, numbering_retry, book_retry, typeset_retry) = match info.work_retry {
        Some(WorkRetry::Edit { form, error }) => (Some(form), Some(error), None, None, None),
        Some(WorkRetry::Numbering(shown)) => (None, None, Some(shown), None, None),
        Some(WorkRetry::Book(retry)) => (None, None, None, Some(retry), None),
        Some(WorkRetry::Typeset(retry)) => (None, None, None, None, Some(retry)),
        None => (None, None, None, None, None),
    };
    let edit = crate::works::edit_context(state, work, shown, edit_form, edit_error, Some(&back))
        .await
        .map_err(|e| e.to_string())?;
    let (saved, numbering_notice) = notice(query);
    let get = |k: &str| query.get(k).map(String::as_str);
    let chosen = match numbering_retry {
        Some(shown) => shown,
        None => {
            let mut chosen = Shown::from_query(get("schema"), get("scheme"), get("muster"), get("custom"));
            chosen.notice = numbering_notice;
            chosen
        }
    };
    let numbering = crate::numbering::page_context(state, work, chosen, Some(&back)).await.map_err(|e| e.message)?;
    let edit_html = super::render_content_block("work_edit.html", edit)?;
    let numbering_html = super::render_content_block("work_numbering.html", numbering)?;
    // Round 11: groups „Buchangaben“ and „Umschlag“ (src/cover.rs), after a rejected form with the input.
    let (book_html, cover_html) = crate::cover::settings_blocks(state, work, book_retry.as_ref()).await?;
    // Round 13: switch „nach MultiGPT hochladen“ (from the store, without network).
    let switch = state.store.sync_work(work).await.map_err(|e| e.to_string())?;
    let tally = state.store.sync_work_tally(work).await.map_err(|e| e.to_string())?;
    let sync = context! {
        upload => switch.upload,
        collection => if switch.collection.is_empty() { crate::sync::collection_for(&switch.title) } else { switch.collection },
        documents => tally.documents,
        synced => tally.synced.as_deref().map(crate::sync::display_time),
    };
    let sources = sword_context(state, work, query).await?;
    let saved = saved.or_else(|| match query.get("ok").map(String::as_str) {
        Some("kommentare") => Some("Kommentare und Lexika des Werks sind gespeichert."),
        Some("zeit") => Some("Die Predigtzeit ist gespeichert."),
        Some("sprache") => Some("Die Werksprache ist gespeichert."),
        Some("satzbild") => Some("Die Satzbild-Einstellungen sind gespeichert."),
        Some("fassungen") => Some("Die Einstellung „Fassungen“ ist gespeichert."),
        // 0.9.0: hide older revisions work-wide or show all again (src/revision_fold.rs).
        Some("ausgeblendet") => Some("Ältere Fassungen sind ausgeblendet (nichts gelöscht)."),
        Some("eingeblendet") => Some("Alle Fassungen werden wieder gezeigt."),
        _ => None,
    });
    let timing = timing_context(state, work, query).await?;
    let language = language_context(state, work, query).await?;
    let sheet = sheet_context(state, work, query).await?;
    let revisions = revisions_context(state, work, query).await?;
    // Round 29c: block „Satz“ (finished HTML from settings/work_typeset.rs).
    let typeset = crate::settings::work_typeset::block(state, work, query, typeset_retry.as_ref()).await?;
    super::render_template(
        "settings/werk.html",
        context! {
            work => work, notice => saved, error => info.error, edit => edit_html, numbering => numbering_html,
            book => book_html, cover => cover_html, sync => sync, sword => sources, timing => timing,
            sheet => sheet, typeset => typeset, revisions => revisions, language => language,
        },
    )
}

/// Round 15 (item 5a): commentaries and lexicons of the work for selection (checkboxes, several), from the module
/// folders (Einstellungen → Quellen); chosen ones that are missing appear with a note. No network.
async fn sword_context(
    state: &AppState,
    work: WorkId,
    query: &BTreeMap<String, String>,
) -> Result<minijinja::Value, String> {
    let (found, ..) = crate::bibles::scan(state).await.map_err(|e| e.message)?;
    let chosen = state.store.work_sword_sources(work).await.map_err(|e| e.to_string())?;
    let list = |modules: &[baruch_sword::BibleInfo], picked: &[String]| -> Vec<minijinja::Value> {
        let mut out: Vec<minijinja::Value> = modules
            .iter()
            .map(|m| {
                context! { module => m.name.as_str(), description => m.description.as_str(),
                notice => m.notice.as_str(), supported => m.supported, problem => m.problem.clone(),
                checked => picked.iter().any(|p| p.eq_ignore_ascii_case(&m.name)), missing => false }
            })
            .collect();
        for p in picked.iter().filter(|p| !modules.iter().any(|m| m.name.eq_ignore_ascii_case(p))) {
            out.push(context! { module => p.as_str(), description => "", notice => "", supported => false,
            problem => Some("in keinem Modulordner gefunden"), checked => true, missing => true });
        }
        out
    };
    let error = query.get("kommentarfehler").map(|_| {
        "Die Auswahl ließ sich nicht speichern: Ein Modul fehlt oder ist nicht lesbar; nichts geändert.".to_owned()
    });
    Ok(context! {
        commentaries => list(&found.commentaries, &chosen.commentaries),
        lexicons => list(&found.lexicons, &chosen.lexicons),
        error => error,
    })
}

/// Round 16b: block „Predigtzeit“ – settings, total time and error after a rejected form.
async fn timing_context(
    state: &AppState,
    work: WorkId,
    query: &BTreeMap<String, String>,
) -> Result<minijinja::Value, String> {
    let timing = crate::timing::work_timing(state, work).await.map_err(|e| e.message)?;
    let error = query.get("zeitfehler").map(|_| {
        "Die Predigtzeit ließ sich nicht speichern: Wörter je Minute 60 bis 250, Zielzeit 1 bis 240 Minuten (oder leer); \
         nichts geändert."
    });
    Ok(context! {
        words_per_minute => timing.words_per_minute,
        target_minutes => timing.target_seconds.map(|s| s / 60),
        total_clock => baruch_typst::timing::clock(timing.seconds),
        words => timing.words,
        over_clock => timing.over_seconds.map(baruch_typst::timing::clock),
        sermon => timing.kind == baruch_core::WorkKind::Sermon,
        default_wpm => baruch_typst::timing::DEFAULT_WORDS_PER_MINUTE,
        error => error,
    })
}

/// `POST /einstellungen/werk/zeit` (Round 16b, behind `guard::same_origin`): `method=work.timing_set`, `work`,
/// `words_per_minute`, `target_minutes` (empty = no target time). 303 to
/// `/einstellungen/werk?work=W&ok=zeit#zeit` or `…&zeitfehler=1#zeit` (nothing changed).
pub(super) async fn timing_act(
    State(state): State<Arc<AppState>>,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Response {
    let field = |name: &str| pairs.iter().find(|(k, _)| k == name).map(|(_, v)| v.trim().to_owned());
    let work = match field("work").and_then(|w| w.parse::<WorkId>().ok()).filter(|&w| w > 0) {
        Some(w) => match state.store.work_slug(w).await {
            Ok(_) => w,
            Err(_) => return Redirect::to("/einstellungen/werk").into_response(),
        },
        None => match state.default_work().await {
            Ok(w) => w,
            Err(_) => return Redirect::to("/einstellungen/werk").into_response(),
        },
    };
    let failed = format!("/einstellungen/werk?work={work}&zeitfehler=1#zeit");
    if field("method").as_deref().is_some_and(|m| m != "work.timing_set") {
        return Redirect::to(&failed).into_response();
    }
    let mut params = json!({ "work": work });
    for name in ["words_per_minute", "target_minutes"] {
        if let Some(value) = field(name) {
            params[name] = json!(value);
        }
    }
    match crate::timing::set_timing(&state, work, &params).await {
        Ok(_) => Redirect::to(&format!("/einstellungen/werk?work={work}&ok=zeit#zeit")).into_response(),
        Err(_) => Redirect::to(&failed).into_response(),
    }
}

/// Round 38: block "Werksprache" – the language of the work and the error after a refused form.
async fn language_context(
    state: &AppState,
    work: WorkId,
    query: &BTreeMap<String, String>,
) -> Result<minijinja::Value, String> {
    let language = state.store.work_language(work).await.map_err(|e| e.to_string())?;
    let error = query
        .get("sprachfehler")
        .map(|_| "Die Werksprache ließ sich nicht speichern: erlaubt sind Deutsch und Englisch; nichts geändert.");
    Ok(context! { code => language.as_str(), error => error })
}

/// `POST /einstellungen/werk/sprache` (Round 38, behind `guard::same_origin`): `work`, `language` (`de`|`en`). 303 to
/// `/einstellungen/werk?work=W&ok=sprache#sprache` or `…&sprachfehler=1#sprache` (nothing changed).
pub(super) async fn language_act(
    State(state): State<Arc<AppState>>,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Response {
    let field = |name: &str| pairs.iter().find(|(k, _)| k == name).map(|(_, v)| v.trim().to_owned());
    let work = match field("work").and_then(|w| w.parse::<WorkId>().ok()).filter(|&w| w > 0) {
        Some(w) => match state.store.work_slug(w).await {
            Ok(_) => w,
            Err(_) => return Redirect::to("/einstellungen/werk").into_response(),
        },
        None => match state.default_work().await {
            Ok(w) => w,
            Err(_) => return Redirect::to("/einstellungen/werk").into_response(),
        },
    };
    let value = field("language").map(Json::String);
    match crate::work_language::set(&state, work, value.as_ref()).await {
        Ok(_) => Redirect::to(&format!("/einstellungen/werk?work={work}&ok=sprache#sprache")).into_response(),
        Err(_) => Redirect::to(&format!("/einstellungen/werk?work={work}&sprachfehler=1#sprache")).into_response(),
    }
}

/// Round 22: block „Fassungen“ – setting of the work, defaults, limits and the error after a rejected
/// form.
async fn revisions_context(
    state: &AppState,
    work: WorkId,
    query: &BTreeMap<String, String>,
) -> Result<minijinja::Value, String> {
    let policy = state.store.revision_policy(work).await.map_err(|e| e.to_string())?;
    let defaults = baruch_store::RevisionPolicy::default();
    let (lo, hi) = baruch_core::revision_policy::MAX_CHANGE_PERCENT;
    let (min_lo, min_hi) = baruch_core::revision_policy::NEW_AFTER_MINUTES;
    let error = query.get("fassungenfehler").map(|_| {
        format!(
            "Die Einstellung „Fassungen“ ließ sich nicht speichern: größte Änderung {lo} bis {hi} Prozent, neue Fassung \
             nach {min_lo} bis {min_hi} Minuten (0 = keine Zeitgrenze); nichts geändert."
        )
    });
    Ok(context! {
        amend => policy.amend,
        max_change_percent => policy.max_change_percent,
        new_after_minutes => policy.new_after_minutes,
        default_percent => defaults.max_change_percent,
        default_minutes => defaults.new_after_minutes,
        percent_min => lo, percent_max => hi, minutes_min => min_lo, minutes_max => min_hi,
        error => error,
        // 0.9.0: sections with the mark „ältere Fassungen ausblenden“ and the error after a rejected form.
        folds => state.store.work_revision_folds(work).await.map_err(|e| e.to_string())?.len(),
        fold_error => query.get("ausblendenfehler").map(|_| "Ältere Fassungen ließen sich nicht ausblenden: Tag als Datum angeben; nichts geändert."),
    })
}

/// Round 18c: block „Satzbild“ – stored settings, the values of the template as placeholders and the
/// error after a rejected form.
async fn sheet_context(
    state: &AppState,
    work: WorkId,
    query: &BTreeMap<String, String>,
) -> Result<minijinja::Value, String> {
    let settings = state.store.sheet_settings(work).await.map_err(|e| e.to_string())?;
    let template = state.store.work_template(work).await.map_err(|e| e.to_string())?;
    let from_template = baruch_wysiwyg::sheet_style(&template);
    let error = query.get("satzbildfehler").map(|_| {
        "Die Satzbild-Einstellungen ließen sich nicht speichern: Schrift höchstens 200 Zeichen (Namen durch Kommas \
         getrennt, ohne ; < > { }), Schriftgröße 6 bis 48 pt, Satzbreite 6 bis 60 cm (Einheit pt, mm, cm oder em), \
         Farben als #rrggbb; nichts geändert."
    });
    Ok(context! {
        settings => minijinja::Value::from(minijinja::value::Serde(&settings)),
        template => minijinja::Value::from(minijinja::value::Serde(&from_template)),
        empty => settings.is_empty(),
        error => error,
    })
}

/// `POST /einstellungen/werk/satzbild` (Round 18c, behind `guard::same_origin`): `method=work.sheet_set`,
/// `work` and the fields of the typeset view settings (empty = from the template), `reset=1` takes all back.
/// 303 to `/einstellungen/werk?work=W&ok=satzbild#satzbild` or `…&satzbildfehler=1#satzbild` (nothing
/// changed).
pub(super) async fn sheet_act(
    State(state): State<Arc<AppState>>,
    Form(pairs): Form<Vec<(String, String)>>,
) -> Response {
    let field = |name: &str| pairs.iter().find(|(k, _)| k == name).map(|(_, v)| v.trim().to_owned());
    let work = match field("work").and_then(|w| w.parse::<WorkId>().ok()).filter(|&w| w > 0) {
        Some(w) => match state.store.work_slug(w).await {
            Ok(_) => w,
            Err(_) => return Redirect::to("/einstellungen/werk").into_response(),
        },
        None => match state.default_work().await {
            Ok(w) => w,
            Err(_) => return Redirect::to("/einstellungen/werk").into_response(),
        },
    };
    let failed = format!("/einstellungen/werk?work={work}&satzbildfehler=1#satzbild");
    if field("method").as_deref().is_some_and(|m| m != "work.sheet_set") {
        return Redirect::to(&failed).into_response();
    }
    let mut params = json!({ "work": work });
    if field("reset").is_some_and(|r| !r.is_empty()) {
        params["reset"] = json!(true);
    } else {
        // The form sends all fields; an empty one means "from the template".
        for name in crate::sheet::FIELDS {
            params[name] = json!(field(name).unwrap_or_default());
        }
    }
    match crate::sheet::set_sheet(&state, work, &params).await {
        Ok(_) => Redirect::to(&format!("/einstellungen/werk?work={work}&ok=satzbild#satzbild")).into_response(),
        Err(_) => Redirect::to(&failed).into_response(),
    }
}

/// `settings.get {page: "werk"}`.
pub(super) async fn values(state: &AppState, work: Option<WorkId>) -> Result<Json, RpcError> {
    let work = work.ok_or_else(|| RpcError::new(INVALID_PARAMS, "Es gibt kein Werk."))?;
    let meta = state.store.work_meta(work).await?;
    let numbering = state.store.outline_numbering(work).await?;
    let timing = state.store.timing_settings(work).await?;
    Ok(json!({
        "work": work, "title": meta.title, "subtitle": meta.subtitle, "author": meta.author,
        "kind": crate::works::kind_word(meta.kind), "audience": meta.audience, "bible": meta.bible,
        "template_rev": meta.template_rev,
        "numbering": { "scheme": numbering.scheme, "custom": numbering.custom },
        "timing": { "words_per_minute": timing.words_per_minute, "target_seconds": timing.target_seconds },
    }))
}

/// `settings.set {page: "werk", values}`: the same function as `work.update` (work from `work`).
pub(super) async fn set(state: &AppState, work: Option<WorkId>, mut values: Json) -> Result<Json, RpcError> {
    let work = work.ok_or_else(|| RpcError::new(INVALID_PARAMS, "Es gibt kein Werk."))?;
    values["work"] = json!(work);
    crate::works::call(state, "work.update", values).await
}
