// SPDX-License-Identifier: AGPL-3.0-or-later
//! Satz-Einstellungen eines Werks (Runde 29, Vertrag §6.1; Schnittstelle R29-S1, danach gehört das Modul satz-daten).
//!
//! - RPC (nur Oberfläche = Mensch; `work` fehlt → Vorgabewerk): `work.typeset_get`, `work.typeset_set`,
//!   `work.typeset_reset`, `work.typeset_adopt_rule` (`docs/RPC.md`, „Satz-Einstellungen“).
//! - Lesen ([`load`], [`view_json`]) teilen RPC, MCP-Werkzeug `typeset_settings` (`mcp/typeset.rs`) und der Block
//!   „Satz“ der Einstellungsseite (`settings/work_typeset.rs`).
//! - Schreiben ([`save`], [`reset`], [`adopt_template_rule`]) nur als Mensch (`HUMAN_NAME`), unter der Schreibsperre,
//!   ein Spiegel-Commit je Änderung (`mirror::describe_typeset`, Trailer `Baruch-Action: typeset-set|typeset-reset|
//!   typeset-adopt`); keine Änderung → kein Commit.
//! - Hinweise (`warnings`): eine Grundschrift, die Typst nicht kennt (`baruch_render::font_families`); Kolumnentitel
//!   „Werktitel“, obwohl die Werkvorlage keinen Titel setzt ([`NO_DOCUMENT_TITLE`]).

use baruch_store::{TypesetChange, WorkId, WorkTypeset};
use baruch_typst::typeset::{
    ChapterBreakRule, FIELDS, TemplateConflict, TypesetSettings, detect_chapter_break, field_label, remove_rule, rules,
    template_conflicts,
};
use serde_json::{Value, json};

use crate::AppState;
use crate::mirror::{self, HUMAN_NAME, TypesetAction, WriteGuard};
use crate::rpc::{INVALID_PARAMS, METHOD_NOT_FOUND, REFUSED, RpcError};

/// Bedient dieses Modul die RPC-Methode?
pub fn handles(method: &str) -> bool {
    matches!(method, "work.typeset_get" | "work.typeset_set" | "work.typeset_reset" | "work.typeset_adopt_rule")
}

/// RPC `work.typeset_*`; `work` ist schon aufgelöst (fehlt → Vorgabewerk).
pub async fn call(state: &AppState, work: WorkId, method: &str, params: Value) -> Result<Value, RpcError> {
    match method {
        "work.typeset_get" => {
            let view = load(state, work).await?;
            let mut out = view_json(&view);
            out["fields"] = FIELDS.iter().map(|(id, label)| json!({ "id": id, "label": label })).collect();
            Ok(out)
        }
        "work.typeset_set" => {
            let settings = settings_param(&params)?;
            let saved = save(state, work, &settings).await?;
            let mut out = saved_json(work, &saved);
            out["settings"] = serde_json::to_value(settings.normalized()).unwrap_or(Value::Null);
            Ok(out)
        }
        "work.typeset_reset" => Ok(saved_json(work, &reset(state, work).await?)),
        "work.typeset_adopt_rule" => {
            // Runde 23a (Vertrag §5): Die Methode schreibt die Werkvorlage um (die Regel wandert in die Einstellung).
            // Über RPC verlangt sie darum die ausdrückliche Bestätigung, wie das Formular sein Kästchen. Kein Widget
            // ruft sie auf (`grep -rn typeset_adopt_rule crates/baruch-server/web/src` ohne Treffer).
            if params.get("confirm") != Some(&Value::Bool(true)) {
                return Err(RpcError::new(INVALID_PARAMS, "confirm: true erwartet"));
            }
            let saved = adopt_template_rule(state, work).await?;
            let after = load(state, work).await?;
            let mut out = saved_json(work, &saved);
            out["chapter_start"] = json!(after.settings.chapter_start.map(|c| c.id()));
            out["template_rev"] = json!(saved.template_rev);
            Ok(out)
        }
        _ => Err(RpcError::new(METHOD_NOT_FOUND, format!("unknown method {method}"))),
    }
}

/// `params.settings` als [`TypesetSettings`]; fehlt oder passt nicht (unbekannte Felder) → `INVALID_PARAMS`.
fn settings_param(params: &Value) -> Result<TypesetSettings, RpcError> {
    let value = params.get("settings").ok_or_else(|| RpcError::new(INVALID_PARAMS, "settings fehlt"))?;
    serde_json::from_value(value.clone()).map_err(|e| RpcError::new(INVALID_PARAMS, format!("settings: {e}")))
}

/// `{work, changed, commit?, mirror?}` einer Änderung.
fn saved_json(work: WorkId, saved: &Saved) -> Value {
    let mut out = json!({ "work": work, "changed": saved.changed });
    if let Some(commit) = &saved.commit {
        out["commit"] = json!(commit);
    }
    if let Some(error) = &saved.mirror_error {
        out["mirror"] = json!(error);
    }
    out
}

/// Alles, was Lesende über die Satz-Einstellungen eines Werks wissen müssen.
#[derive(Debug, Clone)]
pub struct TypesetView {
    pub work: WorkId,
    /// Gespeicherte Zeile; `None` = keine Satz-Einstellungen.
    pub stored: Option<WorkTypeset>,
    /// Geltende Einstellungen (ohne Zeile die Vorgabe, alles aus der Werkvorlage).
    pub settings: TypesetSettings,
    /// Erzeugter Typst-Block ("" ohne Einstellungen).
    pub rules: String,
    /// Stellen der Werkvorlage, die gesetzte Felder selbst setzen.
    pub conflicts: Vec<TemplateConflict>,
    /// Erkannte Kapitelanfangs-Regel der Werkvorlage (Angebot „als Einstellung übernehmen“).
    pub template_rule: Option<ChapterBreakRule>,
    /// Hinweise, z. B. eine fehlende Schrift.
    pub warnings: Vec<String>,
}

/// Liest Einstellungen, Regeln, Konflikte und Vorlagenregel eines Werks. Unbekanntes Werk → `INVALID_PARAMS`.
pub async fn load(state: &AppState, work: WorkId) -> Result<TypesetView, RpcError> {
    let stored = state.store.work_typeset(work).await?;
    let template = state.store.work_template(work).await?;
    let settings = stored.as_ref().map(|s| s.settings.clone()).unwrap_or_default();
    let warnings = warnings(&settings, &template).await;
    Ok(TypesetView {
        work,
        rules: rules(&settings),
        conflicts: template_conflicts(&template, &settings),
        template_rule: detect_chapter_break(&template),
        stored,
        settings,
        warnings,
    })
}

/// JSON wie `work.typeset_get` ohne `fields` (auch die Antwort des MCP-Werkzeugs `typeset_settings`).
pub fn view_json(view: &TypesetView) -> Value {
    let conflicts: Vec<Value> = view
        .conflicts
        .iter()
        .map(|c| {
            json!({ "field": c.field, "label": field_label(c.field), "line": c.line, "text": c.text,
                    "effect": c.effect, "message": c.message })
        })
        .collect();
    let template_rule = view
        .template_rule
        .as_ref()
        .map(|r| json!({ "line": r.line, "text": r.text, "chapter_start": r.chapter_start.id() }));
    json!({
        "work": view.work,
        "set": view.stored.is_some(),
        "uuid": view.stored.as_ref().map(|s| s.uuid.as_str()),
        "updated": view.stored.as_ref().map(|s| s.updated.as_str()),
        "updated_by": view.stored.as_ref().map(|s| s.updated_by.as_str()),
        "settings": serde_json::to_value(&view.settings).unwrap_or_else(|_| json!({})),
        "rules": view.rules,
        "conflicts": conflicts,
        "template_rule": template_rule,
        "warnings": view.warnings,
    })
}

/// Ergebnis einer schreibenden Funktion.
#[derive(Debug, Clone, Default)]
pub struct Saved {
    /// Geänderte Feld-ids (leer = nichts geändert, kein Commit).
    pub changed: Vec<&'static str>,
    /// Spiegel-Commit, falls einer entstand.
    pub commit: Option<String>,
    /// Fehler des Spiegels (die Änderung in SQLite gilt trotzdem).
    pub mirror_error: Option<String>,
    /// Neue Fassung der Werkvorlage (nur [`adopt_template_rule`]).
    pub template_rev: Option<u32>,
}

/// Notiz der neuen Vorlagenfassung bei [`adopt_template_rule`] (Vertrag §6.1).
pub const ADOPT_NOTE: &str = "Kapitelanfang in die Satz-Einstellungen übernommen (Runde 29)";

/// Meldung, wenn die Werkvorlage keine erkennbare Kapitelanfangs-Regel hat (Vertrag §6.1).
pub const NO_RULE: &str = "Die Werkvorlage hat keine erkennbare Regel für den Kapitelanfang; nichts geändert.";

/// Hinweis, wenn der Kolumnentitel „Werktitel“ gewählt ist, die Werkvorlage aber keinen Titel setzt (Integrator R29).
pub const NO_DOCUMENT_TITLE: &str = "Kolumnentitel „Werktitel“: Die Werkvorlage setzt keinen Titel \
     (#set document(title: …)); die Kolumne bleibt leer";

/// Setzt die Werkvorlage `document(… title: …)`? Kommentare (`//`, `/* */`) zählen nicht; die Argumente dürfen über
/// mehrere Zeilen gehen (bis zur passenden schließenden Klammer).
fn template_sets_title(template: &str) -> bool {
    let clean = strip_comments(template);
    let mut rest = clean.as_str();
    while let Some(at) = rest.find("document(") {
        let args = &rest[at + "document(".len()..];
        let mut depth = 1usize;
        let end = args
            .char_indices()
            .find(|&(_, c)| {
                match c {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    _ => {}
                }
                depth == 0
            })
            .map_or(args.len(), |(i, _)| i);
        if args[..end].contains("title:") {
            return true;
        }
        rest = &args[end..];
    }
    false
}

/// Entfernt `// …` bis zum Zeilenende und `/* … */` (Zeilenumbrüche bleiben). Zeichenketten werden nicht
/// gesondert behandelt – für die Suche nach `document(` genügt das.
fn strip_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match (c, chars.peek()) {
            ('/', Some('/')) => {
                for c in chars.by_ref() {
                    if c == '\n' {
                        out.push('\n');
                        break;
                    }
                }
            }
            ('/', Some('*')) => {
                chars.next();
                let mut prev = ' ';
                for c in chars.by_ref() {
                    if c == '\n' {
                        out.push('\n');
                    }
                    if prev == '*' && c == '/' {
                        break;
                    }
                    prev = c;
                }
            }
            _ => out.push(c),
        }
    }
    out
}

/// Hinweise zu den Einstellungen: eine Grundschrift, die Typst nicht kennt (weder eingebettet noch auf dem System);
/// Kolumnentitel „Werktitel“ ohne Titel in der Werkvorlage (der Regelblock liest `document.title`).
async fn warnings(settings: &TypesetSettings, template: &str) -> Vec<String> {
    let mut out = Vec::new();
    if settings.running_head == Some(baruch_typst::typeset::RunningHead::WorkTitle) && !template_sets_title(template) {
        out.push(NO_DOCUMENT_TITLE.to_owned());
    }
    if let Some(font) = &settings.font {
        let families = tokio::task::spawn_blocking(baruch_render::font_families).await.unwrap_or_default();
        if !families.iter().any(|f| f.name.eq_ignore_ascii_case(font)) {
            out.push(format!("Schrift „{font}“ fehlt; Typst setzt mit der eingebetteten Schrift"));
        }
    }
    out
}

/// Nach dem Schreiben (unter der Schreibsperre): genau ein Spiegel-Commit, wenn sich etwas geändert hat (bei der
/// Übernahme immer, denn die Werkvorlage hat eine neue Fassung); sonst keiner.
async fn mirrored(
    state: &AppState,
    held: &WriteGuard<'_>,
    work: WorkId,
    change: &TypesetChange,
    action: TypesetAction,
    template_rev: Option<u32>,
) -> Saved {
    let mut saved = Saved { changed: change.changed.clone(), template_rev, ..Saved::default() };
    if change.changed.is_empty() && template_rev.is_none() {
        return saved;
    }
    match mirror::record_typeset(state, held, work, mirror::describe_typeset(change, action)).await {
        Ok(commit) => saved.commit = commit,
        Err(e) => saved.mirror_error = Some(e),
    }
    saved
}

/// Ersetzt die Satz-Einstellungen (Mensch). Ungültig → `INVALID_PARAMS` mit allen Meldungen, nichts geändert. Leere
/// Einstellungen setzen zurück (Commit „Satz: Einstellungen zurückgesetzt“); gleiche Werte → kein Commit.
pub async fn save(state: &AppState, work: WorkId, settings: &TypesetSettings) -> Result<Saved, RpcError> {
    let held = state.write_lock.lock().await;
    let change = state.store.set_work_typeset(work, settings, HUMAN_NAME).await?;
    let action = if change.after.is_empty() { TypesetAction::Reset } else { TypesetAction::Set };
    Ok(mirrored(state, &held, work, &change, action, None).await)
}

/// Setzt die Satz-Einstellungen zurück (Mensch; Grabstein). Ohne Einstellungen → nichts, kein Commit.
pub async fn reset(state: &AppState, work: WorkId) -> Result<Saved, RpcError> {
    let held = state.write_lock.lock().await;
    let change = state.store.reset_work_typeset(work, HUMAN_NAME).await?;
    Ok(mirrored(state, &held, work, &change, TypesetAction::Reset, None).await)
}

/// Übernimmt die erkannte Kapitelanfangs-Regel der Werkvorlage in die Einstellungen und entfernt sie aus der Vorlage
/// (Mensch; eine Transaktion, ein Commit). Keine erkannte Regel → `REFUSED`. Die übrigen Felder bleiben, wie sie sind.
pub async fn adopt_template_rule(state: &AppState, work: WorkId) -> Result<Saved, RpcError> {
    let held = state.write_lock.lock().await;
    let template = state.store.work_template(work).await?;
    let rule = detect_chapter_break(&template).ok_or_else(|| RpcError::new(REFUSED, NO_RULE))?;
    let mut settings = state.store.work_typeset(work).await?.map(|s| s.settings).unwrap_or_default();
    settings.chapter_start = Some(rule.chapter_start);
    let without = remove_rule(&template, &rule);
    let adopted = state.store.adopt_typeset_rule(work, &settings, &without, ADOPT_NOTE, HUMAN_NAME).await?;
    let action = TypesetAction::Adopt { template_rev: adopted.template_rev };
    Ok(mirrored(state, &held, work, &adopted.change, action, Some(adopted.template_rev)).await)
}

#[cfg(test)]
mod tests {
    use super::template_sets_title;

    #[test]
    fn document_title_is_found_across_lines_and_not_in_comments() {
        assert!(template_sets_title("#set document(title: \"Fährhaus\")\n"), "einzeilig");
        assert!(
            template_sets_title("#set document(\n  author: \"A\",\n  title: \"Fährhaus\",\n)\n"),
            "Argumente über mehrere Zeilen (Gutachten R29: sonst falscher Hinweis NO_DOCUMENT_TITLE)"
        );
        assert!(
            !template_sets_title("/* #set document(title: \"x\") */\n#set text(lang: \"de\")\n"),
            "Blockkommentar zählt nicht"
        );
        assert!(!template_sets_title("// #set document(title: \"x\")\n"), "Zeilenkommentar zählt nicht");
        assert!(
            !template_sets_title("#set document(author: \"A\")\n#let title: x\n"),
            "title: außerhalb der Klammer zählt nicht"
        );
        assert!(!template_sets_title(""), "leere Werkvorlage");
    }
}
