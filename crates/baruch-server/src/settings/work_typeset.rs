// SPDX-License-Identifier: AGPL-3.0-or-later
//! Block „Satz“ of the settings page „Werk“ (Round 29c, contract §6.3; ownership satz-oberflaeche).
//!
//! - Block `id="satz"` (template `templates/settings/werk_satz.html`, [`block`]): all 16 fields from
//!   [`baruch_typst::typeset::FIELDS`] in five groups, per field „aus der Werkvorlage“ as the first choice or an
//!   empty field with a placeholder, hints from the conflicts with the work template at the field, offer for the
//!   detected chapter start rule of the template (mandatory checkbox), the generated Typst rules in `<details>`,
//!   link to the typesetting page.
//! - Forms without JavaScript, both behind `guard::same_origin`:
//!   `POST /einstellungen/werk/satz` ([`act`]): `action=save` → [`crate::typeset_settings::save`] → 303
//!   `…&ok=satz#satz`; `action=reset` (only with `confirm=1`) → [`crate::typeset_settings::reset`] → 303
//!   `…&ok=satz_zurueck#satz`; `action=preview` → 200, whole settings page with the unsaved values and the
//!   preview images. `POST /einstellungen/werk/satz/vorlage` ([`adopt_act`], only with `confirm=1`) →
//!   [`crate::typeset_settings::adopt_template_rule`] → 303 `…&ok=satz_vorlage#satz`. Invalid or rejected → 422
//!   through [`super::work_retry_page`] with [`WorkRetry::Typeset`](super::WorkRetry::Typeset) (inputs stay, message
//!   per field).
//! - Preview: source = [`baruch_typst::typeset::preview_source`] (work template + rules of the unsaved
//!   settings + invented sample text) with the images of the work, typeset through [`crate::typeset::compile`]
//!   (lock `RUNS`, time limit); the pages are delivered by [`preview_svg`] only from the cache
//!   ([`crate::typeset::cached_page`]). RPC `work.typeset_preview` ([`call`]) for the widget `typeset-settings`
//!   (`web/src/typeset_settings.ts`): read only, no commit.

use std::collections::BTreeMap;
use std::sync::Arc;

use axum::extract::{Form, Path, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Redirect, Response};
use baruch_render::{Severity, Source};
use baruch_store::{StoreError, WorkId};
use baruch_typst::typeset::{
    ChapterStart, DRAFT_OPACITY_PERCENT, Draft, DraftPlacement, FIELDS, FieldError, FootnoteNumbering, FootnoteRestart,
    Footnotes, GlossaryPosition, INDEX_COLUMNS, IndexPages, MAX_DRAFT_NOTE_CHARS, MAX_DRAFT_TEXT_CHARS, Margins,
    PageFormat, PageNumberPosition, PageNumberStart, PageNumbers, Paragraph, PartPage, PartSide, RunningHead,
    TemplateConflict, TypesetSettings, preview_source, rules, template_conflicts,
};
use minijinja::context;
use minijinja::value::{Serde, Value as JinjaValue};
use serde::Serialize;
use serde_json::{Value, json};

use super::WorkRetry;
use crate::AppState;
use crate::rpc::{INTERNAL, INVALID_PARAMS, METHOD_NOT_FOUND, RpcError};
use crate::typeset::ui_labels;
use baruch_core::WorkLanguage;
use baruch_core::book_text::book_t;

/// A form of the block „Satz“ that was rejected or sent for preview: input, messages per field
/// (`(field id or subfield such as "margins.inside_mm", message)`; "form" = the whole form) and – after
/// `action=preview` – the typeset preview.
#[derive(Debug, Clone, Default)]
pub struct Retry {
    pub form: Vec<(String, String)>,
    pub errors: Vec<(String, String)>,
    /// Result of `action=preview` (else `None`).
    pub preview: Option<Preview>,
}

/// A typeset preview of unsaved settings (answer of `work.typeset_preview`).
#[derive(Debug, Clone, Serialize)]
pub struct Preview {
    pub stand: String,
    pub pages: usize,
    /// `/einstellungen/werk/satz/vorschau/<stand>/<n>.svg` per page.
    pub urls: Vec<String>,
    /// Number of errors and warnings of Typst.
    pub errors: usize,
    pub warnings: usize,
    pub diagnostics: Vec<PreviewDiagnostic>,
    pub conflicts: Vec<TemplateConflict>,
}

/// A message of Typst in the preview.
#[derive(Debug, Clone, Serialize)]
pub struct PreviewDiagnostic {
    pub severity: &'static str,
    pub message: String,
}

// ---- Form fields -----------------------------------------------------------------------------------

/// First value of a form field, trimmed; missing → "".
fn field<'a>(fields: &'a [(String, String)], name: &str) -> &'a str {
    fields.iter().find(|(k, _)| k == name).map_or("", |(_, v)| v.trim())
}

/// Number from the form (decimal comma allowed): empty → `None`, not a number → `Some(NaN)` – then
/// [`TypesetSettings::validate`] reports the field with label and limit (the same message as through RPC).
fn number(raw: &str) -> Option<f64> {
    if raw.is_empty() {
        return None;
    }
    Some(raw.replace(',', ".").parse::<f64>().unwrap_or(f64::NAN))
}

/// Choice from a list: "" → `None`, unknown → message.
fn choice<T>(
    raw: &str,
    from_id: impl Fn(&str) -> Option<T>,
    field: &'static str,
    errors: &mut Vec<FieldError>,
) -> Option<T> {
    if raw.is_empty() {
        return None;
    }
    let found = from_id(raw);
    if found.is_none() {
        errors.push(FieldError {
            field,
            message: crate::i18n::tc(
                "settings_msg.typeset.unknown_choice",
                &[("field", ui_labels::field_label(crate::i18n::current(), field).into()), ("raw", raw.into())],
            ),
        });
    }
    found
}

/// Switch `""|on|off` → `None | Some(true) | Some(false)`.
fn switch(raw: &str, field: &'static str, errors: &mut Vec<FieldError>) -> Option<bool> {
    choice(
        raw,
        |v| match v {
            "on" => Some(true),
            "off" => Some(false),
            _ => None,
        },
        field,
        errors,
    )
}

/// Round 31c: number of columns of the index pages if the field stays empty.
pub const DEFAULT_INDEX_COLUMNS: u8 = 2;

/// Round 31c: form field per chosen index (checkbox, value = identifier; order = typesetting order).
pub const INDEX_REGISTER_FIELD: &str = "index_register";

/// Position of a field in [`FIELDS`] (subfields like the field) – for the order of the messages.
fn field_rank(field: &str) -> usize {
    let head = field.split('.').next().unwrap_or(field);
    FIELDS.iter().position(|(id, _)| *id == head).unwrap_or(FIELDS.len())
}

/// Settings from the form fields (contract §6.3; "" = from the work template, decimal comma allowed).
///
/// Values are trimmed; margins only all four or none (gutter empty = 0); `part_blank_back` applies only with a
/// chosen page, `page_number_start`/`footnote_restart` only with a chosen position or counting mode (empty = first
/// page or continuous), `paragraph_em` empty = default of the kind (indent 1 em, spacing 0.5 em), width/height only
/// for the custom format. Invalid values are reported by [`TypesetSettings::validate`] with the same messages as
/// through RPC; in addition come „unbekannte Wahl“ and incomplete margins. All messages at once, in the order of
/// [`FIELDS`].
pub fn settings_from_form(fields: &[(String, String)]) -> Result<TypesetSettings, Vec<FieldError>> {
    let get = |name: &str| field(fields, name);
    let mut errors = Vec::new();
    let mut s = TypesetSettings {
        chapter_start: choice(get("chapter_start"), ChapterStart::from_id, "chapter_start", &mut errors),
        ..TypesetSettings::default()
    };
    if let Some(side) = choice(get("part_side"), PartSide::from_id, "part_page", &mut errors) {
        s.part_page = Some(PartPage { side, blank_back: get("part_blank_back") == "1" });
    }
    s.page_format = match get("page_format") {
        "custom" => Some(PageFormat::Custom {
            width_mm: number(get("page_width_mm")).unwrap_or(f64::NAN),
            height_mm: number(get("page_height_mm")).unwrap_or(f64::NAN),
        }),
        raw => choice(raw, PageFormat::from_id, "page_format", &mut errors),
    };
    let sides = ["margin_inside_mm", "margin_outside_mm", "margin_top_mm", "margin_bottom_mm"].map(|n| number(get(n)));
    let binding = number(get("binding_mm"));
    match sides {
        [Some(inside_mm), Some(outside_mm), Some(top_mm), Some(bottom_mm)] => {
            s.margins = Some(Margins { inside_mm, outside_mm, top_mm, bottom_mm, binding_mm: binding.unwrap_or(0.0) });
        }
        [None, None, None, None] if binding.is_none() => {}
        _ => {
            errors.push(FieldError { field: "margins", message: crate::i18n::tc("settings_msg.typeset.margins", &[]) })
        }
    }
    s.font = Some(get("font").to_owned()).filter(|f| !f.is_empty());
    s.font_size_pt = number(get("font_size_pt"));
    s.leading_em = number(get("leading_em"));
    s.paragraph = choice(get("paragraph"), Paragraph::from_id, "paragraph", &mut errors).map(|p| {
        let em = number(get("paragraph_em")).unwrap_or(p.em());
        match p {
            Paragraph::Indent { .. } => Paragraph::Indent { em },
            Paragraph::Spacing { .. } => Paragraph::Spacing { em },
        }
    });
    s.justify = switch(get("justify"), "justify", &mut errors);
    s.hyphenate = switch(get("hyphenate"), "hyphenate", &mut errors);
    s.lang = Some(get("lang").to_owned()).filter(|l| !l.is_empty());
    if let Some(position) =
        choice(get("page_number_position"), PageNumberPosition::from_id, "page_numbers", &mut errors)
    {
        let start = choice(get("page_number_start"), PageNumberStart::from_id, "page_numbers", &mut errors)
            .unwrap_or(PageNumberStart::FirstPage);
        s.page_numbers = Some(PageNumbers { position, start });
    }
    s.running_head = choice(get("running_head"), RunningHead::from_id, "running_head", &mut errors);
    if let Some(numbering) = choice(get("footnote_numbering"), FootnoteNumbering::from_id, "footnotes", &mut errors) {
        let restart = choice(get("footnote_restart"), FootnoteRestart::from_id, "footnotes", &mut errors)
            .unwrap_or(FootnoteRestart::Continuous);
        s.footnotes = Some(Footnotes { numbering, restart });
    }
    s.widows_orphans = switch(get("widows_orphans"), "widows_orphans", &mut errors);
    s.xref_with_page = switch(get("xref_with_page"), "xref_with_page", &mut errors);
    // Round 31: glossary page (empty = none), index pages (no checkboxes = none; columns empty = 2, not a number = 0,
    // then validate reports the limits), first mention.
    s.glossary_page = choice(get("glossary_page"), GlossaryPosition::from_id, "glossary_page", &mut errors);
    let chosen: Vec<String> = fields
        .iter()
        .filter(|(k, _)| k == INDEX_REGISTER_FIELD)
        .map(|(_, v)| v.trim().to_owned())
        .filter(|v| !v.is_empty())
        .collect();
    if !chosen.is_empty() {
        let raw = get("index_columns");
        let columns = if raw.is_empty() { DEFAULT_INDEX_COLUMNS } else { raw.parse::<u8>().unwrap_or(0) };
        s.index_pages = Some(IndexPages { registers: chosen, columns });
    }
    s.glossary_first_mention = switch(get("glossary_first_mention"), "glossary_first_mention", &mut errors);
    // Round 22: draft (checkbox `draft`); empty text = „ENTWURF“, empty opacity = default, not a number = 0 (then
    // validate reports the limits), empty position = diagonal.
    if get("draft") == "1" {
        let defaults = Draft::default();
        let opacity = match get("draft_opacity") {
            "" => defaults.opacity_percent,
            raw => raw.parse::<u8>().unwrap_or(0),
        };
        let placement =
            choice(get("draft_placement"), DraftPlacement::from_id, "draft", &mut errors).unwrap_or(defaults.placement);
        let text = match get("draft_text") {
            "" => defaults.text,
            text => text.to_owned(),
        };
        s.draft = Some(Draft {
            text,
            note: get("draft_note").to_owned(),
            date: get("draft_date") == "1",
            opacity_percent: opacity,
            placement,
            cover: get("draft_cover") == "1",
        });
    }
    if let Err(more) = s.validate() {
        errors.extend(more);
    }
    if errors.is_empty() {
        return Ok(s);
    }
    errors.sort_by_key(|e| field_rank(e.field));
    Err(errors)
}

/// Number for the form in German notation („10,5“, „20“).
fn de(x: f64) -> String {
    format!("{x}").replace('.', ",")
}

/// Form fields for settings (inverse of [`settings_from_form`]; `None` does not appear).
pub fn form_from_settings(s: &TypesetSettings) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut put = |k: &str, v: String| out.push((k.to_owned(), v));
    let on_off = |b: bool| if b { "on" } else { "off" }.to_owned();
    if let Some(c) = s.chapter_start {
        put("chapter_start", c.id().into());
    }
    if let Some(p) = &s.part_page {
        put("part_side", p.side.id().into());
        if p.blank_back {
            put("part_blank_back", "1".into());
        }
    }
    if let Some(f) = s.page_format {
        put("page_format", f.id().into());
        if let PageFormat::Custom { width_mm, height_mm } = f {
            put("page_width_mm", de(width_mm));
            put("page_height_mm", de(height_mm));
        }
    }
    if let Some(m) = &s.margins {
        put("margin_inside_mm", de(m.inside_mm));
        put("margin_outside_mm", de(m.outside_mm));
        put("margin_top_mm", de(m.top_mm));
        put("margin_bottom_mm", de(m.bottom_mm));
        put("binding_mm", de(m.binding_mm));
    }
    if let Some(font) = &s.font {
        put("font", font.clone());
    }
    if let Some(x) = s.font_size_pt {
        put("font_size_pt", de(x));
    }
    if let Some(x) = s.leading_em {
        put("leading_em", de(x));
    }
    if let Some(p) = s.paragraph {
        put("paragraph", p.id().into());
        put("paragraph_em", de(p.em()));
    }
    if let Some(b) = s.justify {
        put("justify", on_off(b));
    }
    if let Some(b) = s.hyphenate {
        put("hyphenate", on_off(b));
    }
    if let Some(lang) = &s.lang {
        put("lang", lang.clone());
    }
    if let Some(n) = &s.page_numbers {
        put("page_number_position", n.position.id().into());
        put("page_number_start", n.start.id().into());
    }
    if let Some(r) = s.running_head {
        put("running_head", r.id().into());
    }
    if let Some(f) = &s.footnotes {
        put("footnote_numbering", f.numbering.id().into());
        put("footnote_restart", f.restart.id().into());
    }
    if let Some(b) = s.widows_orphans {
        put("widows_orphans", on_off(b));
    }
    if let Some(b) = s.xref_with_page {
        put("xref_with_page", on_off(b));
    }
    if let Some(g) = s.glossary_page {
        put("glossary_page", g.id().into());
    }
    if let Some(p) = &s.index_pages {
        for register in &p.registers {
            put(INDEX_REGISTER_FIELD, register.clone());
        }
        put("index_columns", p.columns.to_string());
    }
    if let Some(b) = s.glossary_first_mention {
        put("glossary_first_mention", on_off(b));
    }
    if let Some(d) = &s.draft {
        put("draft", "1".into());
        put("draft_text", d.text.clone());
        if !d.note.is_empty() {
            put("draft_note", d.note.clone());
        }
        if d.date {
            put("draft_date", "1".into());
        }
        put("draft_opacity", d.opacity_percent.to_string());
        put("draft_placement", d.placement.id().into());
        if d.cover {
            put("draft_cover", "1".into());
        }
    }
    out
}

// ---- Preview ---------------------------------------------------------------------------------------

/// All messages in one sentence („Rand innen: 5 bis 80 mm; Schriftgröße: 6 bis 24 pt“); every message already
/// begins with the label of the field.
fn joined(errors: &[FieldError]) -> String {
    errors.iter().map(|e| e.message.as_str()).collect::<Vec<_>>().join("; ")
}

/// Address of a preview image.
fn preview_url(stand: &str, n: usize) -> String {
    format!("/einstellungen/werk/satz/vorschau/{stand}/{n}.svg")
}

/// Typesets the preview of unsaved settings (read only, no commit). Invalid → `INVALID_PARAMS` with all
/// messages; unknown work → `INVALID_PARAMS`; time limit/abort → `REFUSED`.
pub(crate) async fn preview(state: &AppState, work: WorkId, settings: &TypesetSettings) -> Result<Preview, RpcError> {
    let settings = settings.normalized();
    settings.validate().map_err(|e| RpcError::new(INVALID_PARAMS, joined(&e)))?;
    let template = state.store.work_template(work).await?;
    let title = state.store.work_meta(work).await?.title;
    let files = state.store.export_assets(work).await?.into_iter().map(|f| (f.path, f.bytes)).collect();
    let mut main = preview_source(&settings, &template, &title);
    // Round 22: with a draft the preview carries the watermark as well – behind the work template, before the rule
    // block (`preview_source` = template + "\n" + rules + …; the page rules complement each other).
    if let Some(draft) = &settings.draft
        && main.is_char_boundary(template.len() + 1)
    {
        main.insert_str(template.len() + 1, &crate::typeset::draft_rule_for(draft));
    }
    let source = Source { main, files };
    let stand = baruch_render::stand(&source);
    let (compiled, _) = crate::typeset::compile(source, &stand, crate::typeset::COMPILE_TIMEOUT).await?;
    let rendered = &compiled.rendered;
    let errors = rendered.diagnostics.iter().filter(|d| d.severity == Severity::Error).count();
    Ok(Preview {
        pages: rendered.pages.len(),
        urls: (1..=rendered.pages.len()).map(|n| preview_url(&stand, n)).collect(),
        errors,
        warnings: rendered.diagnostics.len() - errors,
        diagnostics: rendered
            .diagnostics
            .iter()
            .map(|d| PreviewDiagnostic { severity: d.severity.as_str(), message: d.message.clone() })
            .collect(),
        conflicts: template_conflicts(&template, &settings),
        stand,
    })
}

/// Conflicts as in `work.typeset_get` (with label).
fn conflicts_json(conflicts: &[TemplateConflict]) -> Vec<Value> {
    conflicts
        .iter()
        .map(|c| {
            json!({ "field": c.field, "label": ui_labels::field_label(crate::i18n::current(), c.field),
                    "line": c.line, "text": c.text,
                    "effect": c.effect, "message": c.message })
        })
        .collect()
}

/// Does this module serve the RPC method? `work.typeset_preview`.
pub fn handles(method: &str) -> bool {
    method == "work.typeset_preview"
}

/// RPC `work.typeset_preview {work?, settings}` (read only, no commit) → `{work, stand, pages, urls, errors,
/// warnings, diagnostics [{severity, message}], conflicts}`.
pub async fn call(state: &AppState, work: WorkId, method: &str, params: Value) -> Result<Value, RpcError> {
    match method {
        "work.typeset_preview" => {
            let value = params.get("settings").ok_or_else(|| {
                RpcError::new(INVALID_PARAMS, crate::i18n::tc("settings_msg.typeset.settings_missing", &[]))
            })?;
            let settings: TypesetSettings = serde_json::from_value(value.clone())
                .map_err(|e| RpcError::new(INVALID_PARAMS, format!("settings: {e}")))?;
            let p = preview(state, work, &settings).await?;
            Ok(json!({
                "work": work, "stand": p.stand, "pages": p.pages, "urls": p.urls, "errors": p.errors,
                "warnings": p.warnings, "diagnostics": p.diagnostics, "conflicts": conflicts_json(&p.conflicts),
            }))
        }
        _ => Err(RpcError::new(METHOD_NOT_FOUND, format!("unknown method {method}"))),
    }
}

// ---- Block -----------------------------------------------------------------------------------------

/// `[{id, label}]` of a selection list `group` (the label in the interface language, Round 39b).
fn options<T: Copy>(all: &[T], id: impl Fn(T) -> &'static str, group: &str) -> Vec<JinjaValue> {
    let lang = crate::i18n::current();
    all.iter().map(|&v| context! { id => id(v), label => ui_labels::choice_label(lang, group, id(v)) }).collect()
}

/// What the block knows about the work (from [`crate::typeset_settings::load`]) – separate, so that the drawing
/// can be checked without a server.
pub(crate) struct BlockInput<'a> {
    pub work: WorkId,
    pub view: &'a crate::typeset_settings::TypesetView,
    pub template: &'a str,
    pub retry: Option<&'a Retry>,
    /// Round 31c: indexes of the work for the checkboxes „Registerseiten“ ([`crate::glossary::registers`]).
    pub registers: &'a [baruch_typst::index::Register],
    /// Round 39b: language of the work; the default watermark of the form follows it (`book.draft.watermark`).
    pub language: WorkLanguage,
}

/// Checkboxes of the index pages: first the chosen indexes in their order (this keeps the typesetting order on
/// saving), then the others of the work; a chosen one that no longer exists stays visible, so that it can be
/// deselected. `[{slug, title, checked}]`.
fn index_choices(form: &[(String, String)], registers: &[baruch_typst::index::Register]) -> Vec<JinjaValue> {
    let chosen: Vec<&str> = form
        .iter()
        .filter(|(k, _)| k == INDEX_REGISTER_FIELD)
        .map(|(_, v)| v.trim())
        .filter(|v| !v.is_empty())
        .collect();
    let mut out = Vec::new();
    for slug in &chosen {
        let title = registers.iter().find(|r| r.slug == *slug).map_or_else(
            || crate::i18n::tc("settings_msg.typeset.register_gone", &[("slug", (*slug).into())]),
            |r| r.title.clone(),
        );
        out.push(context! { slug => *slug, title => title, checked => true });
    }
    for r in registers.iter().filter(|r| !chosen.contains(&r.slug.as_str())) {
        out.push(context! { slug => r.slug.as_str(), title => r.title.as_str(), checked => false });
    }
    out
}

/// Context of the template `settings/werk_satz.html`.
pub(crate) fn block_context(input: &BlockInput<'_>) -> JinjaValue {
    let view = input.view;
    // Unsaved values exist only with input (reset/adopt without the checkbox send no fields along:
    // then the stored values stand there).
    let unsaved = input.retry.filter(|r| !r.form.is_empty());
    let (form, shown) = match unsaved {
        Some(retry) => {
            // For rules and hints, as far as the input can be read.
            let shown = settings_from_form(&retry.form).unwrap_or_else(|_| view.settings.clone());
            (retry.form.clone(), shown)
        }
        None => (form_from_settings(&view.settings), view.settings.clone()),
    };
    let index_registers = index_choices(&form, input.registers);
    let values: BTreeMap<String, String> = form.into_iter().rev().collect();
    let mut errors: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (field, message) in input.retry.map(|r| r.errors.clone()).unwrap_or_default() {
        let mut head = field.split('.').next().unwrap_or(&field).to_owned();
        // Without an offer (no detected rule) the message on the adoption stands at the top of the block.
        if head == "template_rule" && view.template_rule.is_none() {
            head = "form".to_owned();
        }
        errors.entry(head).or_default().push(message);
    }
    let conflicts = if unsaved.is_some() { template_conflicts(input.template, &shown) } else { view.conflicts.clone() };
    let mut hints: BTreeMap<String, Vec<JinjaValue>> = BTreeMap::new();
    for c in &conflicts {
        let ineffective = matches!(c.effect, baruch_typst::typeset::ConflictEffect::Ineffective);
        hints.entry(c.field.to_owned()).or_default().push(context! {
            message => c.message.as_str(), line => c.line, text => c.text.as_str(), ineffective => ineffective,
        });
    }
    let template_rule = view.template_rule.as_ref().map(|r| {
        let chapter_start = ui_labels::choice_label(crate::i18n::current(), "chapter_start", r.chapter_start.id());
        context! { line => r.line, text => r.text.as_str(), chapter_start => chapter_start }
    });
    let mut formats = options(PageFormat::ALL, PageFormat::id, "page_format");
    formats.push(context! {
        id => "custom", label => ui_labels::choice_label(crate::i18n::current(), "page_format", "custom"),
    });
    let preview = input.retry.and_then(|r| r.preview.as_ref()).map(|p| {
        context! {
            urls => p.urls.clone(), pages => p.pages, errors => p.errors, warnings => p.warnings,
            diagnostics => JinjaValue::from(Serde(&p.diagnostics)),
        }
    });
    let labels: BTreeMap<&str, String> =
        FIELDS.iter().map(|(id, _)| (*id, ui_labels::field_label(crate::i18n::current(), id))).collect();
    context! {
        work => input.work,
        v => values,
        errors => errors,
        hints => hints,
        labels => labels,
        warnings => view.warnings.clone(),
        set => view.stored.is_some(),
        updated => view.stored.as_ref().map(|s| crate::localtime::minutes(&s.updated)),
        updated_by => view.stored.as_ref().map(|s| s.updated_by.clone()),
        rules => rules(&shown),
        // Note „Ungespeicherte Werte“ only if the input differs from the stored state (a preview of the
        // stored values does not show it; integrator R29). Unreadable inputs always differ.
        unsaved => unsaved.is_some_and(|r| settings_from_form(&r.form).map_or(true, |s| s.normalized() != view.settings)),
        template_rule => template_rule,
        preview => preview,
        satz_url => format!("/werke/{}/satz", input.work),
        index_registers => index_registers,
        index_columns => (INDEX_COLUMNS.0..=INDEX_COLUMNS.1).collect::<Vec<u8>>(),
        default_index_columns => DEFAULT_INDEX_COLUMNS,
        opts => context! {
            chapter_start => options(ChapterStart::ALL, ChapterStart::id, "chapter_start"),
            part_side => options(PartSide::ALL, PartSide::id, "part_side"),
            page_format => formats,
            paragraph => options(Paragraph::ALL, Paragraph::id, "paragraph"),
            page_number_position => options(PageNumberPosition::ALL, PageNumberPosition::id, "page_number_position"),
            page_number_start => options(PageNumberStart::ALL, PageNumberStart::id, "page_number_start"),
            running_head => options(RunningHead::ALL, RunningHead::id, "running_head"),
            footnote_numbering => options(FootnoteNumbering::ALL, FootnoteNumbering::id, "footnote_numbering"),
            footnote_restart => options(FootnoteRestart::ALL, FootnoteRestart::id, "footnote_restart"),
            glossary_page => options(GlossaryPosition::ALL, GlossaryPosition::id, "glossary_page"),
            draft_placement => options(DraftPlacement::ALL, DraftPlacement::id, "draft_placement"),
        },
        draft_defaults => context! {
            text => book_t(input.language, "book.draft.watermark", &[]), opacity => Draft::default().opacity_percent,
            min_opacity => DRAFT_OPACITY_PERCENT.0, max_opacity => DRAFT_OPACITY_PERCENT.1,
            max_text => MAX_DRAFT_TEXT_CHARS, max_note => MAX_DRAFT_NOTE_CHARS,
        },
    }
}

/// HTML of the block „Satz“ (`id="satz"`); after a rejected form or a preview with its input.
pub async fn block(
    state: &AppState,
    work: WorkId,
    query: &BTreeMap<String, String>,
    retry: Option<&Retry>,
) -> Result<String, String> {
    let _ = query;
    let view = crate::typeset_settings::load(state, work).await.map_err(|e| e.message)?;
    let template = state.store.work_template(work).await.map_err(|e| e.to_string())?;
    let registers =
        crate::glossary::registers(state, work).await.unwrap_or_else(|_| baruch_typst::index::builtin_registers());
    super::render_template(
        "settings/werk_satz.html",
        block_context(&BlockInput {
            work,
            view: &view,
            template: &template,
            retry,
            registers: &registers,
            language: crate::work_language::of(state, work).await,
        }),
    )
}

// ---- Forms -----------------------------------------------------------------------------------------

/// Work of a form: field `work` (must exist), otherwise the default work.
// `Response` as the error is intended here (finished answer for `?` in the handler); a `Box` would change all callers.
#[allow(clippy::result_large_err)]
async fn form_work(state: &AppState, fields: &[(String, String)]) -> Result<WorkId, Response> {
    let gone = || (StatusCode::NOT_FOUND, crate::i18n::tc("settings_msg.typeset.unknown_work", &[])).into_response();
    match field(fields, "work").parse::<WorkId>().ok().filter(|&w| w > 0) {
        Some(w) => match state.store.work_slug(w).await {
            Ok(_) => Ok(w),
            Err(StoreError::UnknownWork(_)) => Err(gone()),
            Err(e) => Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                crate::i18n::tc("settings_msg.frame.error", &[("detail", e.to_string().into())]),
            )
                .into_response()),
        },
        None => state.default_work().await.map_err(|_| gone()),
    }
}

/// The settings page „Werk“ with the block „Satz“ in the state `retry`.
async fn retry_page(state: &AppState, headers: &HeaderMap, status: StatusCode, work: WorkId, retry: Retry) -> Response {
    super::work_retry_page(state, headers, status, work, WorkRetry::Typeset(retry)).await
}

/// A rejected RPC call as a message at the form (422; internal errors 500).
async fn refused(
    state: &AppState,
    headers: &HeaderMap,
    work: WorkId,
    form: Vec<(String, String)>,
    e: RpcError,
) -> Response {
    let status = if e.code == INTERNAL { StatusCode::INTERNAL_SERVER_ERROR } else { StatusCode::UNPROCESSABLE_ENTITY };
    let retry = Retry { form, errors: vec![("form".to_owned(), e.message)], preview: None };
    retry_page(state, headers, status, work, retry).await
}

/// Messages of [`settings_from_form`] as `(field, message)`.
fn pairs(errors: Vec<FieldError>) -> Vec<(String, String)> {
    errors.into_iter().map(|e| (e.field.to_owned(), e.message)).collect()
}

/// Target after success.
fn done(work: WorkId, ok: &str) -> Response {
    Redirect::to(&format!("/einstellungen/werk?work={work}&ok={ok}#satz")).into_response()
}

/// `POST /einstellungen/werk/satz`: `action` = `save` (default) | `preview` | `reset` (only with `confirm=1`).
pub async fn act(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Form(fields): Form<Vec<(String, String)>>,
) -> Response {
    let work = match form_work(&state, &fields).await {
        Ok(work) => work,
        Err(response) => return response,
    };
    let form: Vec<(String, String)> = fields.iter().filter(|(k, _)| k != "action" && k != "confirm").cloned().collect();
    match field(&fields, "action") {
        "reset" => {
            if field(&fields, "confirm") != "1" {
                let message = crate::i18n::tc("settings_msg.typeset.reset_confirm", &[]);
                let retry = Retry { form: Vec::new(), errors: vec![("reset".to_owned(), message)], preview: None };
                return retry_page(&state, &headers, StatusCode::UNPROCESSABLE_ENTITY, work, retry).await;
            }
            match crate::typeset_settings::reset(&state, work).await {
                Ok(_) => done(work, "satz_zurueck"),
                Err(e) => refused(&state, &headers, work, Vec::new(), e).await,
            }
        }
        action @ ("save" | "" | "preview") => {
            let settings = match settings_from_form(&form) {
                Ok(settings) => settings,
                Err(errors) => {
                    let retry = Retry { form, errors: pairs(errors), preview: None };
                    return retry_page(&state, &headers, StatusCode::UNPROCESSABLE_ENTITY, work, retry).await;
                }
            };
            if action == "preview" {
                return match preview(&state, work, &settings).await {
                    Ok(p) => {
                        let retry = Retry { form, errors: Vec::new(), preview: Some(p) };
                        retry_page(&state, &headers, StatusCode::OK, work, retry).await
                    }
                    Err(e) => refused(&state, &headers, work, form, e).await,
                };
            }
            match crate::typeset_settings::save(&state, work, &settings).await {
                Ok(_) => done(work, "satz"),
                Err(e) => refused(&state, &headers, work, form, e).await,
            }
        }
        _ => (StatusCode::BAD_REQUEST, crate::i18n::tc("settings_msg.typeset.unknown_action", &[])).into_response(),
    }
}

/// `POST /einstellungen/werk/satz/vorlage`: adopt the template rule as a setting and remove the line from the
/// work template (only with `confirm=1`).
pub async fn adopt_act(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Form(fields): Form<Vec<(String, String)>>,
) -> Response {
    let work = match form_work(&state, &fields).await {
        Ok(work) => work,
        Err(response) => return response,
    };
    if field(&fields, "confirm") != "1" {
        let message = crate::i18n::tc("settings_msg.typeset.apply_confirm", &[]);
        let retry = Retry { form: Vec::new(), errors: vec![("template_rule".to_owned(), message)], preview: None };
        return retry_page(&state, &headers, StatusCode::UNPROCESSABLE_ENTITY, work, retry).await;
    }
    match crate::typeset_settings::adopt_template_rule(&state, work).await {
        Ok(_) => done(work, "satz_vorlage"),
        Err(e) => {
            let retry = Retry {
                form: Vec::new(),
                errors: vec![("template_rule".to_owned(), e.message.clone())],
                preview: None,
            };
            let status =
                if e.code == INTERNAL { StatusCode::INTERNAL_SERVER_ERROR } else { StatusCode::UNPROCESSABLE_ENTITY };
            retry_page(&state, &headers, status, work, retry).await
        }
    }
}

/// `GET /einstellungen/werk/satz/vorschau/{stand}/{file}`: preview image `n.svg` from the cache (only states
/// lying there, otherwise 404; foreign page → 403, without Typst).
pub async fn preview_svg(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path((stand, file)): Path<(String, String)>,
) -> Response {
    let _ = state;
    if crate::guard::cross_site_get(&headers) {
        return crate::typeset::foreign_get();
    }
    let missing = || (StatusCode::NOT_FOUND, crate::i18n::tc("settings_msg.typeset.preview_gone", &[])).into_response();
    let Some(n) = file.strip_suffix(".svg").and_then(|n| n.parse::<usize>().ok()).filter(|&n| n >= 1) else {
        return missing();
    };
    if stand.len() != 64 || !stand.bytes().all(|b| b.is_ascii_hexdigit()) {
        return missing();
    }
    let Some(page) = crate::typeset::cached_page(&stand, n) else { return missing() };
    (
        [
            (header::CONTENT_TYPE, HeaderValue::from_static("image/svg+xml")),
            (header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff")),
            (header::CONTENT_SECURITY_POLICY, HeaderValue::from_static(crate::typeset::SVG_CSP)),
            (header::CACHE_CONTROL, HeaderValue::from_static("no-cache")),
        ],
        page,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn form(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect()
    }

    /// The example from the contract (§2) with all 16 fields.
    fn example() -> TypesetSettings {
        TypesetSettings::from_json(r#"{"chapter_start":"right_page","part_page":{"side":"right","blank_back":true},"page_format":{"kind":"a5"},"margins":{"inside_mm":20.0,"outside_mm":15.0,"top_mm":18.0,"bottom_mm":22.0,"binding_mm":5.0},"font":"Libertinus Serif","font_size_pt":10.5,"leading_em":0.65,"paragraph":{"kind":"indent","em":1.0},"justify":true,"hyphenate":true,"lang":"de","page_numbers":{"position":"bottom_outside","start":"first_chapter"},"running_head":"chapter","footnotes":{"numbering":"arabic","restart":"per_chapter"},"widows_orphans":true,"xref_with_page":true}"#).unwrap()
    }

    #[test]
    fn every_field_of_the_contract_example_survives_the_form() {
        let s = example();
        let back = settings_from_form(&form_from_settings(&s)).expect("das Vertragsbeispiel ist gültig");
        // Form and settings are inverses: what the block shows, „Speichern“ stores unchanged.
        assert_eq!(back, s, "Rundreise über die Formularfelder verliert nichts");
        // Round 22: also the draft with all fields (addition, date, position, cover).
        let draft = TypesetSettings {
            draft: Some(Draft {
                text: "Vorabzug".into(),
                note: "für die Lektorin".into(),
                date: true,
                opacity_percent: 25,
                placement: DraftPlacement::Footer,
                cover: true,
            }),
            ..s
        };
        let back = settings_from_form(&form_from_settings(&draft)).expect("gültiger Entwurf");
        assert_eq!(back, draft, "Rundreise des Entwurfs über die Formularfelder");
        let errors = settings_from_form(&form(&[("draft", "1"), ("draft_opacity", "viel")])).expect_err("Deckkraft");
        assert_eq!(errors[0].field, "draft.opacity_percent", "keine Zahl → Grenze gemeldet wie über RPC");
    }

    #[test]
    fn empty_fields_mean_from_the_template() {
        let s = settings_from_form(&form(&[
            ("work", "1"),
            ("chapter_start", ""),
            ("part_side", ""),
            ("part_blank_back", "1"),
            ("page_format", ""),
            ("margin_inside_mm", " "),
            ("font", "  "),
            ("justify", ""),
            ("page_number_position", ""),
            ("page_number_start", "first_chapter"),
            ("footnote_numbering", ""),
            ("footnote_restart", "per_chapter"),
        ]))
        .expect("lauter leere Felder sind gültig");
        // "" = from the work template: no setting, even if subfields (back side empty, start) carry something.
        assert!(s.is_empty(), "alles aus der Werkvorlage: {s:?}");
    }

    #[test]
    fn decimal_commas_and_custom_formats_are_read() {
        let s = settings_from_form(&form(&[
            ("page_format", "custom"),
            ("page_width_mm", "125,5"),
            ("page_height_mm", "200"),
            ("font_size_pt", "10,5"),
            ("leading_em", "0.7"),
            ("paragraph", "spacing"),
            ("paragraph_em", ""),
            ("margin_inside_mm", "18"),
            ("margin_outside_mm", "14"),
            ("margin_top_mm", "16"),
            ("margin_bottom_mm", "20"),
            ("binding_mm", ""),
        ]))
        .expect("gültige Werte");
        assert_eq!(
            s.page_format,
            Some(PageFormat::Custom { width_mm: 125.5, height_mm: 200.0 }),
            "Komma als Dezimalzeichen"
        );
        assert_eq!(s.font_size_pt, Some(10.5), "10,5 pt");
        assert_eq!(s.leading_em, Some(0.7), "Punkt geht auch");
        assert_eq!(s.paragraph, Some(Paragraph::Spacing { em: 0.5 }), "leeres Maß = Vorgabe der Art (0,5 em Abstand)");
        assert_eq!(s.margins.as_ref().map(|m| m.binding_mm), Some(0.0), "Bundsteg leer = 0");
    }

    #[test]
    fn partial_margins_and_bad_values_are_reported_all_at_once_like_validate() {
        let errors = crate::i18n::sync_scope(crate::i18n::Lang::De, || {
            settings_from_form(&form(&[
                ("margin_inside_mm", "20"),
                ("margin_top_mm", "18"),
                ("font_size_pt", "dreißig"),
                ("lang", "Deutsch"),
                ("running_head", "kopf"),
                ("page_format", "custom"),
                ("page_width_mm", "50"),
            ]))
        })
        .expect_err("mehrere Fehler");
        let fields: Vec<&str> = errors.iter().map(|e| e.field).collect();
        // Order of FIELDS: page_format before margins before font_size_pt before lang before running_head.
        assert_eq!(
            fields,
            ["page_format.width_mm", "page_format.height_mm", "margins", "font_size_pt", "lang", "running_head"],
            "alle Meldungen in Feldreihenfolge: {errors:?}"
        );
        assert_eq!(errors[2].message, "Ränder: alle vier Ränder angeben oder alle leer", "teilweise Ränder");
        // Not a number → the same message validate gives for a value outside the limits.
        let reference = TypesetSettings { font_size_pt: Some(99.0), ..Default::default() }.validate().unwrap_err();
        assert_eq!(errors[3].message, reference[0].message, "Meldung wie TypesetSettings::validate");
        assert_eq!(errors[5].message, "Kolumnentitel: unbekannte Wahl „kopf“", "unbekannte Wahl mit Bezeichnung");
    }

    #[test]
    fn binding_alone_counts_as_partial_margins() {
        let errors = settings_from_form(&form(&[("binding_mm", "5")])).expect_err("Bundsteg ohne Ränder");
        // Otherwise the gutter would be lost silently.
        assert_eq!(errors[0].field, "margins", "{errors:?}");
    }

    fn view(template_rule: bool, conflict: bool) -> crate::typeset_settings::TypesetView {
        use baruch_typst::typeset::{CANONICAL_CHAPTER_BREAK, ConflictEffect, detect_chapter_break};
        let settings = TypesetSettings { chapter_start: Some(ChapterStart::Continuous), ..Default::default() };
        crate::typeset_settings::TypesetView {
            work: 7,
            stored: None,
            rules: String::new(),
            conflicts: if conflict {
                vec![TemplateConflict {
                    field: "chapter_start",
                    line: 3,
                    text: CANONICAL_CHAPTER_BREAK.into(),
                    effect: ConflictEffect::Ineffective,
                    message: "Die Werkvorlage setzt den Kapitelanfang selbst (Zeile 3); die Einstellung wirkt nicht."
                        .into(),
                }]
            } else {
                Vec::new()
            },
            template_rule: if template_rule { detect_chapter_break(CANONICAL_CHAPTER_BREAK) } else { None },
            warnings: vec!["Schrift „Erfundene Antiqua“ fehlt".into()],
            settings,
        }
    }

    fn render(view: &crate::typeset_settings::TypesetView, retry: Option<&Retry>) -> String {
        super::super::render_template(
            "settings/werk_satz.html",
            block_context(&BlockInput {
                work: 7,
                view,
                template: "",
                retry,
                registers: &baruch_typst::index::builtin_registers(),
                language: WorkLanguage::De,
            }),
        )
        .expect("Vorlage zeichnet")
    }

    #[test]
    fn a_conflict_stands_at_its_field_and_warnings_are_shown() {
        let html = render(&view(false, true), None);
        let field = html.find("name=\"chapter_start\"").expect("Feld Kapitelanfang");
        let hint = html.find("data-typeset-hint=\"chapter_start\"").expect("Hinweis zum Kapitelanfang");
        let next = html.find("name=\"part_side\"").expect("Feld Teil-Seite");
        // The hint stands at the field (between it and the next field), not collected at the end.
        assert!(field < hint && hint < next, "Hinweis beim Feld: {html}");
        assert!(html.contains("die Einstellung wirkt nicht"), "Meldung des Konflikts: {html}");
        assert!(html.contains("Erfundene Antiqua"), "Warnung: {html}");
    }

    #[test]
    fn the_offer_names_line_and_text_of_the_template_rule() {
        let html = crate::i18n::sync_scope(crate::i18n::Lang::De, || render(&view(true, false), None));
        assert!(html.contains("action=\"/einstellungen/werk/satz/vorlage\""), "Formular der Übernahme: {html}");
        assert!(html.contains("Zeile 1"), "Zeile der Regel: {html}");
        assert!(html.contains("pagebreak(weak: true, to: &quot;odd&quot;)"), "Text der Regel (maskiert): {html}");
        assert!(html.contains("name=\"confirm\" value=\"1\" required"), "Pflicht-Häkchen: {html}");
        let none = render(&view(false, false), None);
        assert!(!none.contains("/einstellungen/werk/satz/vorlage"), "ohne erkannte Regel kein Angebot");
    }
}
