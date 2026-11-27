// SPDX-License-Identifier: AGPL-3.0-or-later
//! Einstellungsseite „Werk“: Metadaten und Werkvorlage (Formularteil von `work_edit.html`) und das
//! Gliederungsschema (Formularteil von `work_numbering.html`) des gewählten Werks. Kein kopiertes
//! Formular: Beide Blöcke kommen mit demselben Kontextaufbau wie `/werke/{id}/bearbeiten` bzw.
//! `/werke/{id}/nummerierung` ([`crate::works::edit_context`], [`crate::numbering::page_context`]);
//! die Formulare tragen zusätzlich `back=/einstellungen/werk?work=W`, damit `POST /werke/act` und
//! `POST /werke/nummerierung/act` nach Erfolg hierher zurückführen.
//!
//! Runde 11 (Aufgabe cover): dazu die Gruppen „Buchangaben“ und „Umschlag“ aus [`crate::cover`]
//! (Formulare an `POST /werke/buchangaben/act` und `/werke/umschlag/act`, Rücksprung mit `ok=buchangaben`
//! bzw. `ok=umschlag`).
//!
//! Runde 13 (Aufgabe abgleich): dazu der Schalter „nach MultiGPT hochladen“ (Vorgabe aus, Einschalten nur
//! mit Pflicht-Kontrollkästchen), Formular an `POST /abgleich/act` mit `action=upload` und
//! `back=/einstellungen/werk?work=W` (Rücksprung mit `ok=hochladen_an` bzw. `ok=hochladen_aus`). Kein Netz.
//!
//! Runde 15 (Punkt 5a): dazu „Kommentare und Lexika“ – Kästchen je SWORD-Kommentar und -Lexikon der
//! Modulordner (mehrere wählbar), Formular an `POST /einstellungen/werk/bibeln` (`method=bible.set_work_sources`,
//! wiederholte Felder `commentary` und `lexicon`; Handler in `connections::pages`, Rücksprung `ok=kommentare`
//! bzw. `kommentarfehler=1`).
//!
//! Runde 16b (Zeitwächter): dazu der Block „Predigtzeit“ (`id="zeit"`): Wörter je Minute (60–250, Vorgabe 110)
//! und Zielzeit in Minuten (1–240, leer = keine), Formular an `POST /einstellungen/werk/zeit`
//! (`method=work.timing_set`; [`timing_act`], Rücksprung `ok=zeit#zeit` bzw. `zeitfehler=1#zeit`). Die Werte
//! stehen nur in SQLite (`work_timing`), nicht im Spiegel.
//!
//! Runde 18c (Satzbild je Werk): dazu der Block „Satzbild“ (`id="satzbild"`): Schrift, Schrift der Überschriften,
//! Schriftgröße, Satzbreite, Text-, Überschriften- und Blattfarbe; leer = aus der Werkvorlage (deren Werte stehen als
//! Platzhalter da). Formular an `POST /einstellungen/werk/satzbild` (`method=work.sheet_set`; [`sheet_act`],
//! Rücksprung `ok=satzbild#satzbild` bzw. `satzbildfehler=1#satzbild`). Nur SQLite (`work_sheet`), nicht im Spiegel;
//! gilt nur für das HTML-Satzbild, nie für Typst, Export oder PDF.
//!
//! Runde 27c (Umschlag-Editor): Die Gruppe „Umschlag“ trägt den Knopf „Impressum-Vorlage in die Werkvorlage
//! übernehmen“ (`POST /werke/umschlag/impressum`, Rücksprung `ok=impressum`, abgelehnt: Meldung bei der Gruppe), die
//! Buchangaben die Anschriften von Verlag und Druckerei; `ok=anordnung` meldet eine gespeicherte Umschlag-Anordnung.
//!
//! Runde 29c (Buchsatz): dazu der Block „Satz“ (`id="satz"`, [`crate::settings::work_typeset`]) direkt nach
//! „Satzbild“; Rücksprünge `ok=satz`, `ok=satz_zurueck`, `ok=satz_vorlage` (jeweils `#satz`), abgelehnte Formulare und
//! die Vorschau ohne JavaScript über [`WorkRetry::Typeset`].
//!
//! Runde 22 (fassungen): dazu der Block „Fassungen“ (`id="fassungen"`): Fortschreiben an/aus, größte Änderung in
//! Prozent (1–100, Vorgabe 20) und neue Fassung nach Minuten (0–1440, 0 = keine Zeitgrenze, Vorgabe 30); Formular an
//! `POST /einstellungen/werk/fassungen` (`method=revision.policy_set`, Handler in [`crate::revisions`]), Rücksprung
//! `ok=fassungen#fassungen` bzw. `fassungenfehler=1#fassungen`. Nur SQLite (`work_revision_policy`), nicht im
//! Austauschformat; jede Änderung ist ein Protokoll-Commit im Spiegel.

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

/// Hinweise zu `ok=…` nach einem Rücksprung.
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
        // Runde 29c: Block „Satz“.
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
    // Nach einem abgelehnten Formular: dessen Eingabe und Meldung (sonst ginge sie im Popup verloren).
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
    // Runde 11: Gruppen „Buchangaben“ und „Umschlag“ (src/cover.rs), nach einem abgelehnten Formular mit Eingabe.
    let (book_html, cover_html) = crate::cover::settings_blocks(state, work, book_retry.as_ref()).await?;
    // Runde 13: Schalter „nach MultiGPT hochladen“ (aus dem Store, ohne Netz).
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
        // 0.9.0: ältere Fassungen werkweit ausblenden bzw. alle wieder zeigen (src/revision_fold.rs).
        Some("ausgeblendet") => Some("Ältere Fassungen sind ausgeblendet (nichts gelöscht)."),
        Some("eingeblendet") => Some("Alle Fassungen werden wieder gezeigt."),
        _ => None,
    });
    let timing = timing_context(state, work, query).await?;
    let language = language_context(state, work, query).await?;
    let sheet = sheet_context(state, work, query).await?;
    let revisions = revisions_context(state, work, query).await?;
    // Runde 29c: Block „Satz“ (fertiges HTML aus settings/work_typeset.rs).
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

/// Runde 15 (Punkt 5a): Kommentare und Lexika des Werks zur Auswahl (Kästchen, mehrere), aus den Modulordnern
/// (Einstellungen → Quellen); gewählte, die fehlen, erscheinen mit Hinweis. Kein Netz.
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

/// Runde 16b: Block „Predigtzeit“ – Einstellungen, Gesamtzeit und Fehler nach einem abgelehnten Formular.
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

/// `POST /einstellungen/werk/zeit` (Runde 16b, hinter `guard::same_origin`): `method=work.timing_set`, `work`,
/// `words_per_minute`, `target_minutes` (leer = keine Zielzeit). 303 auf
/// `/einstellungen/werk?work=W&ok=zeit#zeit` bzw. `…&zeitfehler=1#zeit` (nichts geändert).
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

/// Runde 22: Block „Fassungen“ – Einstellung des Werks, Vorgaben, Grenzen und der Fehler nach einem abgelehnten
/// Formular.
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
        // 0.9.0: Abschnitte mit Marke „ältere Fassungen ausblenden“ und der Fehler nach einem abgelehnten Formular.
        folds => state.store.work_revision_folds(work).await.map_err(|e| e.to_string())?.len(),
        fold_error => query.get("ausblendenfehler").map(|_| "Ältere Fassungen ließen sich nicht ausblenden: Tag als Datum angeben; nichts geändert."),
    })
}

/// Runde 18c: Block „Satzbild“ – gespeicherte Einstellungen, die Werte der Vorlage als Platzhalter und der
/// Fehler nach einem abgelehnten Formular.
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

/// `POST /einstellungen/werk/satzbild` (Runde 18c, hinter `guard::same_origin`): `method=work.sheet_set`,
/// `work` und die Felder der Satzbild-Einstellungen (leer = aus der Vorlage), `reset=1` nimmt alle zurück.
/// 303 auf `/einstellungen/werk?work=W&ok=satzbild#satzbild` bzw. `…&satzbildfehler=1#satzbild` (nichts
/// geändert).
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
        // Das Formular schickt alle Felder; ein leeres heißt „aus der Vorlage“.
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

/// `settings.set {page: "werk", values}`: dieselbe Funktion wie `work.update` (Werk aus `work`).
pub(super) async fn set(state: &AppState, work: Option<WorkId>, mut values: Json) -> Result<Json, RpcError> {
    let work = work.ok_or_else(|| RpcError::new(INVALID_PARAMS, "Es gibt kein Werk."))?;
    values["work"] = json!(work);
    crate::works::call(state, "work.update", values).await
}
