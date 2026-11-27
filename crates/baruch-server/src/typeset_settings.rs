// SPDX-License-Identifier: AGPL-3.0-or-later
//! Typeset settings of a work (Round 29, contract §6.1; interface R29-S1, afterwards the module belongs to satz-daten).
//!
//! - RPC (user interface only = human; `work` missing → default work): `work.typeset_get`, `work.typeset_set`,
//!   `work.typeset_reset`, `work.typeset_adopt_rule` (`docs/RPC.md`, „Satz-Einstellungen“).
//! - Reading ([`load`], [`view_json`]) is shared by RPC, the MCP tool `typeset_settings` (`mcp/typeset.rs`) and the
//!   block „Satz“ of the settings page (`settings/work_typeset.rs`).
//! - Writing ([`save`], [`reset`], [`adopt_template_rule`]) only as the human (`HUMAN_NAME`), under the write lock,
//!   one mirror commit per change (`mirror::describe_typeset`, trailer `Baruch-Action: typeset-set|typeset-reset|
//!   typeset-adopt`); no change → no commit.
//! - Notes (`warnings`): a body font Typst does not know (`baruch_render::font_families`); running head
//!   „Werktitel“ although the work template sets no title ([`NO_DOCUMENT_TITLE`]).

use baruch_store::{TypesetChange, WorkId, WorkTypeset};
use baruch_typst::typeset::{
    ChapterBreakRule, FIELDS, TemplateConflict, TypesetSettings, detect_chapter_break, field_label, remove_rule, rules,
    template_conflicts,
};
use serde_json::{Value, json};

use crate::AppState;
use crate::mirror::{self, HUMAN_NAME, TypesetAction, WriteGuard};
use crate::rpc::{INVALID_PARAMS, METHOD_NOT_FOUND, REFUSED, RpcError};

/// Does this module serve the RPC method?
pub fn handles(method: &str) -> bool {
    matches!(method, "work.typeset_get" | "work.typeset_set" | "work.typeset_reset" | "work.typeset_adopt_rule")
}

/// RPC `work.typeset_*`; `work` is already resolved (missing → default work).
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
            // Round 23a (contract §5): the method rewrites the work template (the rule moves into the setting).
            // Through RPC it therefore requires the explicit confirmation, as the form requires its checkbox. No
            // widget calls it (`grep -rn typeset_adopt_rule crates/baruch-server/web/src` without a hit).
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

/// `params.settings` as [`TypesetSettings`]; missing or not fitting (unknown fields) → `INVALID_PARAMS`.
fn settings_param(params: &Value) -> Result<TypesetSettings, RpcError> {
    let value = params.get("settings").ok_or_else(|| RpcError::new(INVALID_PARAMS, "settings fehlt"))?;
    serde_json::from_value(value.clone()).map_err(|e| RpcError::new(INVALID_PARAMS, format!("settings: {e}")))
}

/// `{work, changed, commit?, mirror?}` of a change.
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

/// Everything readers need to know about the typeset settings of a work.
#[derive(Debug, Clone)]
pub struct TypesetView {
    pub work: WorkId,
    /// Stored row; `None` = no typeset settings.
    pub stored: Option<WorkTypeset>,
    /// Settings in force (without a row the default, everything from the work template).
    pub settings: TypesetSettings,
    /// Generated Typst block ("" without settings).
    pub rules: String,
    /// Places of the work template that set fields that are set themselves.
    pub conflicts: Vec<TemplateConflict>,
    /// Detected chapter start rule of the work template (offer „als Einstellung übernehmen“).
    pub template_rule: Option<ChapterBreakRule>,
    /// Notes, e.g. a missing font.
    pub warnings: Vec<String>,
}

/// Reads settings, rules, conflicts and template rule of a work. Unknown work → `INVALID_PARAMS`.
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

/// JSON like `work.typeset_get` without `fields` (also the answer of the MCP tool `typeset_settings`).
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

/// Result of a writing function.
#[derive(Debug, Clone, Default)]
pub struct Saved {
    /// Changed field ids (empty = nothing changed, no commit).
    pub changed: Vec<&'static str>,
    /// Mirror commit if one was created.
    pub commit: Option<String>,
    /// Error of the mirror (the change in SQLite holds nevertheless).
    pub mirror_error: Option<String>,
    /// New revision of the work template (only [`adopt_template_rule`]).
    pub template_rev: Option<u32>,
}

/// Note of the new template revision for [`adopt_template_rule`] (contract §6.1).
pub const ADOPT_NOTE: &str = "Kapitelanfang in die Satz-Einstellungen übernommen (Runde 29)";

/// Message when the work template has no recognisable chapter start rule (contract §6.1).
pub const NO_RULE: &str = "Die Werkvorlage hat keine erkennbare Regel für den Kapitelanfang; nichts geändert.";

/// Note when the running head „Werktitel“ is chosen but the work template sets no title (integrator R29).
pub const NO_DOCUMENT_TITLE: &str = "Kolumnentitel „Werktitel“: Die Werkvorlage setzt keinen Titel \
     (#set document(title: …)); die Kolumne bleibt leer";

/// Does the work template set `document(… title: …)`? Comments (`//`, `/* */`) do not count; the arguments may
/// span several lines (up to the matching closing bracket).
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

/// Removes `// …` up to the end of the line and `/* … */` (line breaks stay). Strings are not
/// treated separately – for the search for `document(` that is enough.
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

/// Notes on the settings: a body font Typst does not know (neither embedded nor on the system);
/// running head „Werktitel“ without a title in the work template (the rule block reads `document.title`).
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

/// After writing (under the write lock): exactly one mirror commit if something has changed (always for the
/// adoption, because the work template has a new revision); otherwise none.
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

/// Replaces the typeset settings (human). Invalid → `INVALID_PARAMS` with all messages, nothing changed. Empty
/// settings reset (commit „Satz: Einstellungen zurückgesetzt“); equal values → no commit.
pub async fn save(state: &AppState, work: WorkId, settings: &TypesetSettings) -> Result<Saved, RpcError> {
    let held = state.write_lock.lock().await;
    let change = state.store.set_work_typeset(work, settings, HUMAN_NAME).await?;
    let action = if change.after.is_empty() { TypesetAction::Reset } else { TypesetAction::Set };
    Ok(mirrored(state, &held, work, &change, action, None).await)
}

/// Resets the typeset settings (human; tombstone). Without settings → nothing, no commit.
pub async fn reset(state: &AppState, work: WorkId) -> Result<Saved, RpcError> {
    let held = state.write_lock.lock().await;
    let change = state.store.reset_work_typeset(work, HUMAN_NAME).await?;
    Ok(mirrored(state, &held, work, &change, TypesetAction::Reset, None).await)
}

/// Adopts the detected chapter start rule of the work template into the settings and removes it from the template
/// (human; one transaction, one commit). No detected rule → `REFUSED`. The other fields stay as they are.
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
