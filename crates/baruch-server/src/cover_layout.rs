// SPDX-License-Identifier: AGPL-3.0-or-later
//! Umschlag-Anordnung über RPC (Vertrag Runde 27, Abschnitt 4; Aufgabe umschlag-daten).
//!
//! - `cover.layout {work}` → [`LayoutView`]: gespeicherte bzw. Vorgabeblöcke mit Höhe, Maße, Druckprüfung,
//!   Vorgabeschrift. Satzfehler → `REFUSED` (wie `cover::build_wrap`).
//! - `cover.layout_set {work, blocks}` → `{layout, commit?, mirror?}`: jeder genannte Block ersetzt seine Art ganz,
//!   ungenannte bleiben; ein Commit „Umschlag: Anordnung geändert“ (`Baruch-Action: cover-layout`) mit vorher/nachher
//!   je Block (nur der Mensch). Ungültig → -32602 mit deutscher Meldung, die Art und Feld nennt; nichts geändert.
//!   Die Felder `stored` und `height_mm` aus `cover.layout` dürfen mitkommen (werden übergangen).
//! - `cover.layout_reset {work, faces?, area?}` → wie `layout_set`: Zeilen der Flächen löschen (fehlt `faces`: alle;
//!   Grabsteine), `area` setzt den Titelbereich in derselben Transaktion (Schnellwahl); ein Commit „Umschlag:
//!   Anordnung zurückgesetzt“ bzw. „Umschlag: Schnellwahl oben|Mitte|unten“ (`cover-layout-reset`). Ändert sich nichts,
//!   gibt es keinen Commit.
//! - Lässt sich die Ansicht nach dem Speichern nicht bauen (Satzfehler im Buch), steht in der Antwort `layout: null`
//!   und `layout_error`; gespeichert ist trotzdem.
//! - `cover.fonts {work?}` → `{families, default}`.
//! - [`decide_request`]: ein bestätigter Antrag `cover_layout` aus den Bildanträgen (MCP `cover_layout_set`): passt
//!   der Fingerabdruck `expected_layout` nicht mehr zur gespeicherten Anordnung, wird er `stale` mit Grund; sonst
//!   ausgeführt mit einem Commit „Umschlag: Anordnung geändert (Antrag N)“ samt Antragsteller und Bestätiger.

use baruch_gitmirror::{CommitRequest, MirrorFile, Signature};
use baruch_store::{
    AssetOp, AssetRequest, CoverBlock, CoverBlockKind, CoverFace, CoverGeometry, CoverLayout, CoverLayoutChange,
    Decision, Store, StoreError, TitleArea, WorkId,
};
use baruch_typst::cover_layout::{Backdrop, CoverAlign, FontStyle, SpineDirection, effective_blocks};
use serde_json::{Map, Value, json};

use crate::AppState;
use crate::asset_requests::Decided;
use crate::cover_print::CoverWarning;
use crate::mirror::{HUMAN_EMAIL, HUMAN_NAME, WriteGuard};
use crate::rpc::{INTERNAL, INVALID_PARAMS, METHOD_NOT_FOUND, RpcError};

/// Vorgabeschrift, wenn die Werkvorlage keine nennt (eingebettet in Typst).
pub const DEFAULT_FONT: &str = "Libertinus Serif";

/// Methoden dieses Moduls.
pub fn handles(method: &str) -> bool {
    matches!(method, "cover.layout" | "cover.layout_set" | "cover.layout_reset" | "cover.fonts")
}

/// Antwort von `cover.layout` (JSON-Form eingefroren, Vertrag Abschnitt 4).
#[derive(Debug, Clone, serde::Serialize)]
pub struct LayoutView {
    pub work: WorkId,
    /// Hat das Werk eine eigene Anordnung (mindestens eine Zeile)?
    pub stored: bool,
    pub title_area: TitleArea,
    pub blocks: Vec<ViewBlock>,
    pub geometry: CoverGeometry,
    pub warnings: Vec<CoverWarning>,
    pub default_font: String,
}

/// Ein Block der Ansicht: der Block selbst, ob er in der Datenbank steht, und seine gesetzte Höhe.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ViewBlock {
    #[serde(flatten)]
    pub block: CoverBlock,
    pub stored: bool,
    pub height_mm: f64,
}

/// Ansicht der Anordnung; `preview` ersetzt je Art die gespeicherten Blöcke (ohne zu speichern).
pub async fn layout_view(
    state: &AppState,
    work: WorkId,
    preview: Option<&CoverLayout>,
) -> Result<LayoutView, RpcError> {
    // 1. gespeicherte Anordnung
    let stored = state.store.cover_layout(work).await?;
    // 2. Vorschau ersetzt je Art
    let mut merged = stored.clone();
    if let Some(preview) = preview {
        for block in &preview.blocks {
            merged.blocks.retain(|b| b.kind != block.kind);
            merged.blocks.push(block.clone());
        }
    }
    let shown = preview.map(|_| &merged);
    // 3. Maße aus dem gesetzten Umschlag
    let wrap = crate::cover::build_wrap_for(state, work, shown).await?;
    let geometry = wrap.geometry;
    // 4. wirksame Blöcke
    let meta = state.store.book_meta(work).await?;
    let effective = effective_blocks(&merged, crate::cover::title_area(meta.title_area), &geometry);
    // 5. Höhen
    let plain: Vec<CoverBlock> = effective.iter().map(|(b, _)| b.clone()).collect();
    let heights = crate::cover::block_heights(state, work, &plain, &geometry).await?;
    // 6. Druckprüfung
    let warnings = crate::cover_print::cover_checks(state, work, shown).await?;
    // 7. Vorgabeschrift
    let default_font = default_font(state, work).await?;
    let blocks = effective
        .into_iter()
        .zip(heights)
        .map(|((block, _), height_mm)| ViewBlock { stored: stored.get(block.kind).is_some(), block, height_mm })
        .collect();
    Ok(LayoutView {
        work,
        stored: !stored.is_empty(),
        title_area: meta.title_area,
        blocks,
        geometry,
        warnings,
        default_font,
    })
}

/// Grundschrift des Buchs (Satz-Einstellung, sonst Werkvorlage; Runde 29) bzw. [`DEFAULT_FONT`].
pub(crate) async fn default_font(state: &AppState, work: WorkId) -> Result<String, RpcError> {
    let template = state.store.work_template(work).await?;
    let settings = crate::cover::typeset_settings(state, work).await?;
    Ok(crate::cover::book_font(&template, &settings).unwrap_or_else(|| DEFAULT_FONT.to_owned()))
}

// ---- Blöcke aus JSON -------------------------------------------------------------------------------

/// „Titel (Vorderseite)“.
pub fn kind_label(kind: CoverBlockKind) -> String {
    format!("{} ({})", kind.label(), kind.face().label())
}

/// Felder eines Blocks im JSON (Vertrag 1.3); `stored` und `height_mm` aus `cover.layout` werden übergangen.
const FIELDS: [&str; 14] = [
    "kind",
    "x_mm",
    "y_mm",
    "width_mm",
    "align",
    "size_pt",
    "leading_em",
    "color",
    "font",
    "style",
    "backdrop",
    "visible",
    "direction",
    "text",
];

/// Block aus einem JSON-Objekt lesen. Ohne `base` müssen alle Felder außer `font`, `direction` und `text` da sein
/// (RPC); mit `base` (MCP) füllen dessen Werte fehlende Felder. Fehler nennen Art und Feld (deutsch). Geprüft auf
/// Grenzen wird danach im Store ([`baruch_store::check_cover_block`]).
pub fn parse_block(
    index: usize,
    value: &Value,
    base: impl FnOnce(CoverBlockKind) -> Option<CoverBlock>,
) -> Result<CoverBlock, String> {
    let Some(object) = value.as_object() else {
        return Err(format!("Umschlag-Block Nr. {}: ein Objekt mit kind, x_mm, … erwartet", index + 1));
    };
    let kind = match object.get("kind") {
        Some(Value::String(word)) => CoverBlockKind::parse(word).ok_or_else(|| {
            format!("Umschlag-Block Nr. {}, Feld kind: unbekannte Art „{word}“ (erlaubt: {})", index + 1, kind_list())
        })?,
        _ => {
            return Err(format!(
                "Umschlag-Block Nr. {}, Feld kind: Art als Text erwartet ({})",
                index + 1,
                kind_list()
            ));
        }
    };
    let label = format!("Umschlag-Block „{}“ ({})", kind_label(kind), kind.as_str());
    let wrong = |field: &str, what: &str| format!("{label}, Feld {field}: {what}");
    for key in object.keys() {
        if !FIELDS.contains(&key.as_str()) && key != "stored" && key != "height_mm" {
            return Err(wrong(key, "unbekanntes Feld"));
        }
    }
    let base = base(kind);
    let missing = |field: &str| wrong(field, "fehlt");
    let number = |field: &str, current: Option<f64>| -> Result<f64, String> {
        match object.get(field) {
            Some(Value::Number(n)) => n.as_f64().ok_or_else(|| wrong(field, "Zahl erwartet")),
            Some(_) => Err(wrong(field, "Zahl erwartet")),
            None => current.ok_or_else(|| missing(field)),
        }
    };
    fn word<T: Copy>(
        object: &Map<String, Value>,
        field: &str,
        current: Option<T>,
        parse: fn(&str) -> Option<T>,
        allowed: &str,
        wrong: &dyn Fn(&str, &str) -> String,
    ) -> Result<T, String> {
        match object.get(field) {
            Some(Value::String(s)) => parse(s).ok_or_else(|| wrong(field, &format!("„{s}“ – erlaubt: {allowed}"))),
            Some(_) => Err(wrong(field, &format!("Text erwartet ({allowed})"))),
            None => current.ok_or_else(|| wrong(field, "fehlt")),
        }
    }
    let optional_text = |field: &str, current: Option<Option<String>>| -> Result<Option<String>, String> {
        match object.get(field) {
            Some(Value::String(s)) => Ok(Some(s.clone())),
            Some(Value::Null) => Ok(None),
            Some(_) => Err(wrong(field, "Text oder null erwartet")),
            None => Ok(current.flatten()),
        }
    };
    let b = base.as_ref();
    Ok(CoverBlock {
        kind,
        x_mm: number("x_mm", b.map(|b| b.x_mm))?,
        y_mm: number("y_mm", b.map(|b| b.y_mm))?,
        width_mm: number("width_mm", b.map(|b| b.width_mm))?,
        align: word(object, "align", b.map(|b| b.align), CoverAlign::parse, "left, center, right, justify", &wrong)?,
        size_pt: number("size_pt", b.map(|b| b.size_pt))?,
        leading_em: number("leading_em", b.map(|b| b.leading_em))?,
        color: match object.get("color") {
            Some(Value::String(s)) => s.clone(),
            Some(_) => return Err(wrong("color", "Text erwartet (light, dark oder #rrggbb)")),
            None => b.map(|b| b.color.clone()).ok_or_else(|| missing("color"))?,
        },
        font: optional_text("font", b.map(|b| b.font.clone()))?,
        style: word(object, "style", b.map(|b| b.style), FontStyle::parse, "normal, bold, italic", &wrong)?,
        backdrop: word(object, "backdrop", b.map(|b| b.backdrop), Backdrop::parse, "none, shadow, gradient", &wrong)?,
        visible: match object.get("visible") {
            Some(Value::Bool(v)) => *v,
            Some(_) => return Err(wrong("visible", "true oder false erwartet")),
            None => b.map(|b| b.visible).ok_or_else(|| missing("visible"))?,
        },
        direction: match object.get("direction") {
            Some(Value::String(s)) => Some(
                SpineDirection::parse(s).ok_or_else(|| wrong("direction", &format!("„{s}“ – erlaubt: down, up")))?,
            ),
            Some(Value::Null) => None,
            Some(_) => return Err(wrong("direction", "down, up oder null erwartet")),
            None => b.and_then(|b| b.direction),
        },
        text: optional_text("text", b.map(|b| b.text.clone()))?,
    })
}

fn kind_list() -> String {
    CoverBlockKind::ALL.iter().map(|k| k.as_str()).collect::<Vec<_>>().join(", ")
}

/// `blocks` aus den Parametern (Liste von Objekten).
pub fn parse_blocks(
    value: Option<&Value>,
    mut base: impl FnMut(CoverBlockKind) -> Option<CoverBlock>,
) -> Result<Vec<CoverBlock>, String> {
    let Some(Value::Array(items)) = value else {
        return Err("blocks: Liste von Blöcken erwartet (je {kind, x_mm, y_mm, width_mm, …})".into());
    };
    items.iter().enumerate().map(|(i, v)| parse_block(i, v, &mut base)).collect()
}

// ---- Commit-Texte ----------------------------------------------------------------------------------

fn mm(v: f64) -> String {
    format!("{v:.1}").replace('.', ",")
}

fn align_word(a: CoverAlign) -> &'static str {
    match a {
        CoverAlign::Left => "links",
        CoverAlign::Center => "zentriert",
        CoverAlign::Right => "rechts",
        CoverAlign::Justify => "Blocksatz",
    }
}

fn style_word(s: FontStyle) -> &'static str {
    match s {
        FontStyle::Normal => "normal",
        FontStyle::Bold => "fett",
        FontStyle::Italic => "kursiv",
    }
}

fn backdrop_word(b: Backdrop) -> &'static str {
    match b {
        Backdrop::None => "keiner",
        Backdrop::Shadow => "Schatten",
        Backdrop::Gradient => "Verlauf",
    }
}

fn color_word(c: &str) -> String {
    match c {
        "light" => "hell".into(),
        "dark" => "dunkel".into(),
        other => other.into(),
    }
}

fn area_word(area: TitleArea) -> &'static str {
    match area {
        TitleArea::Top => "oben",
        TitleArea::Middle => "Mitte",
        TitleArea::Bottom => "unten",
    }
}

/// Text in einer Zeile, höchstens 60 Zeichen.
fn short(text: &str) -> String {
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if line.chars().count() > 60 {
        format!("„{}…“", line.chars().take(60).collect::<String>())
    } else {
        format!("„{line}“")
    }
}

/// Felder eines Blocks als (Name, Wert) für den Commit-Rumpf.
fn fields(block: &CoverBlock) -> Vec<(&'static str, String)> {
    let mut out = vec![
        ("x", format!("{} mm", mm(block.x_mm))),
        ("y", format!("{} mm", mm(block.y_mm))),
        ("Breite", format!("{} mm", mm(block.width_mm))),
        ("Ausrichtung", align_word(block.align).to_owned()),
        ("Größe", format!("{} pt", mm(block.size_pt))),
        ("Zeilenabstand", format!("{} em", format!("{:.2}", block.leading_em).replace('.', ","))),
        ("Farbe", color_word(&block.color)),
        ("Schrift", block.font.clone().unwrap_or_else(|| "der Werkvorlage".into())),
        ("Schnitt", style_word(block.style).to_owned()),
        ("Hintergrund", backdrop_word(block.backdrop).to_owned()),
        ("sichtbar", if block.visible { "ja" } else { "nein" }.to_owned()),
    ];
    if let Some(direction) = block.direction {
        out.push(("Laufrichtung", if direction == SpineDirection::Down { "abwärts" } else { "aufwärts" }.to_owned()));
    }
    if let Some(text) = &block.text {
        out.push(("Text", short(text)));
    }
    out
}

/// „x 12,0 mm → 15,0 mm“ je geändertes Feld; ein Feld mit Einheit nennt sie einmal am Ende („x 12,0 → 15,0 mm“).
fn diff(before: &CoverBlock, after: &CoverBlock) -> String {
    let (old, new) = (fields(before), fields(after));
    let mut parts = Vec::new();
    for (name, value) in &new {
        let previous = old.iter().find(|(n, _)| n == name).map_or("–".to_owned(), |(_, v)| v.clone());
        if &previous != value {
            let unit = [" mm", " pt", " em"].into_iter().find(|u| previous.ends_with(u) && value.ends_with(u));
            parts.push(match unit {
                Some(u) => format!("{name} {} → {value}", previous.trim_end_matches(u)),
                None => format!("{name} {previous} → {value}"),
            });
        }
    }
    for (name, value) in &old {
        if !new.iter().any(|(n, _)| n == name) {
            parts.push(format!("{name} {value} → –"));
        }
    }
    parts.join(", ")
}

fn all_fields(block: &CoverBlock) -> String {
    fields(block).into_iter().map(|(n, v)| format!("{n} {v}")).collect::<Vec<_>>().join(", ")
}

/// Eine Zeile je Block für die Liste der Anträge: „Titel (Vorderseite): x 5,0 mm, y 40,0 mm, …“.
pub fn block_line(block: &CoverBlock) -> String {
    format!("{}: {}", kind_label(block.kind), all_fields(block))
}

/// Rumpf je geänderten Block: „Titel (Vorderseite): x 12,0 → 15,0 mm, …“. Stand ein Block vorher nicht in der
/// Datenbank, wird gegen den Vorgabeblock aus `defaults` verglichen („… (vorher Vorgabe)“); ohne Vorgabe (Buch ohne
/// Seitenmaß) „…: neu (x …)“. Zurückgesetzt: „…: Vorgabe statt eigener Anordnung (war x …)“.
pub fn change_body(change: &CoverLayoutChange, defaults: &[CoverBlock]) -> String {
    let mut body = String::new();
    for kind in &change.changed {
        let label = kind_label(*kind);
        let default = defaults.iter().find(|b| b.kind == *kind);
        let line = match (change.before.get(*kind), change.after.get(*kind)) {
            (Some(before), Some(after)) => format!("{label}: {}", diff(before, after)),
            (None, Some(after)) if default.is_some() => {
                format!("{label}: {} (vorher Vorgabe)", diff(default.unwrap_or(after), after))
            }
            (None, Some(after)) => format!("{label}: neu ({})", all_fields(after)),
            (Some(before), None) => format!("{label}: Vorgabe statt eigener Anordnung (war {})", all_fields(before)),
            (None, None) => format!("{label}: unverändert"),
        };
        body.push_str(&line);
        body.push('\n');
    }
    body
}

// ---- Schreiben ---------------------------------------------------------------------------------------

fn human() -> Signature {
    Signature { name: HUMAN_NAME.to_owned(), email: HUMAN_EMAIL.to_owned() }
}

/// Commit im Spiegel (leer erlaubt: die Anordnung steht nicht im Schnappschuss). Mit `request` (bestätigter Antrag)
/// stehen Antragsteller und Bestätiger im Rumpf und in den Trailern wie bei den Bildanträgen.
async fn commit(
    state: &AppState,
    _held: &WriteGuard<'_>,
    work: WorkId,
    mut summary: String,
    body: String,
    action: &str,
    request: Option<&AssetRequest>,
) -> Result<Option<String>, String> {
    let files = state.store.mirror_snapshot(work).await.map_err(|e| e.to_string())?;
    let mirror = state.mirror(work).await?;
    let mut trailers = vec![("Baruch-Action".to_owned(), action.to_owned())];
    let mut text = body;
    if let Some(asked) = request {
        let confirmer = asked.decided_by.clone().unwrap_or_else(|| HUMAN_NAME.to_owned());
        let mut head =
            format!("Antrag {} von {}, bestätigt von {confirmer}: {}\n", asked.id, asked.origin.name(), asked.summary);
        if !asked.note.is_empty() {
            head.push_str(&format!("Begründung: {}\n", asked.note.split_whitespace().collect::<Vec<_>>().join(" ")));
        }
        text = if text.is_empty() { head } else { format!("{head}\n{text}") };
        summary.push_str(&format!(" (Antrag {})", asked.id));
        trailers.push(("Baruch-Origin".to_owned(), asked.origin.kind().to_owned()));
        trailers.push(("Baruch-Request".to_owned(), asked.id.to_string()));
        trailers.push(("Baruch-Requested-By".to_owned(), asked.origin.name().to_owned()));
        trailers.push(("Baruch-Confirmed-By".to_owned(), confirmer));
    }
    let request = CommitRequest {
        files: files.into_iter().map(|f| MirrorFile { path: f.path, contents: f.contents }).collect(),
        author: human(),
        committer: human(),
        summary,
        body: text,
        trailers,
        allow_empty: true,
    };
    let result = tokio::task::spawn_blocking(move || mirror.commit(&request))
        .await
        .map_err(|e| format!("Git: {e}"))?
        .map_err(|e| e.to_string());
    if let Err(e) = &result {
        eprintln!("Git-Spiegel (Werk {work}, Umschlag-Anordnung): {e}");
        state.forget_mirror(work);
    }
    result
}

fn add_mirror(answer: &mut Value, result: Option<Result<Option<String>, String>>) {
    match result {
        Some(Ok(Some(commit))) => answer["commit"] = json!(commit),
        Some(Err(e)) => answer["mirror"] = json!(e),
        _ => {}
    }
}

/// Antwort `{layout, commit?, mirror?}`; gelingt die Ansicht nicht, `layout: null` und `layout_error`.
async fn answer(state: &AppState, work: WorkId, mirrored: Option<Result<Option<String>, String>>) -> Value {
    let mut answer = match layout_view(state, work, None).await {
        Ok(view) => json!({ "work": work, "layout": view }),
        Err(e) => json!({ "work": work, "layout": null, "layout_error": e.message }),
    };
    add_mirror(&mut answer, mirrored);
    answer
}

/// Vorgabeblöcke des Werks für den Commit-Rumpf (vorher → nachher); ohne Seitenmaß (Satzfehler) leer.
async fn defaults(state: &AppState, work: WorkId) -> Vec<CoverBlock> {
    let Ok(wrap) = crate::cover::build_wrap_for(state, work, None).await else { return Vec::new() };
    let Ok(meta) = state.store.book_meta(work).await else { return Vec::new() };
    baruch_typst::cover_layout::default_blocks(crate::cover::title_area(meta.title_area), &wrap.geometry)
}

/// `cover.layout_set` (nur der Mensch): Blöcke speichern, ein Commit.
pub async fn layout_set(state: &AppState, work: WorkId, blocks: Vec<CoverBlock>) -> Result<Value, RpcError> {
    // Erst prüfen (ohne Satz), dann die Vorgaben für den Rumpf setzen lassen – außerhalb der Schreibsperre. Die
    // maßgebliche Prüfung macht der Store beim Schreiben noch einmal.
    for block in &blocks {
        baruch_store::check_cover_block(block)?;
    }
    let defaults = if blocks.is_empty() { Vec::new() } else { defaults(state, work).await };
    let mirrored = {
        let held = state.write_lock.lock().await;
        let change = state.store.set_cover_blocks(work, &blocks).await?;
        if change.changed.is_empty() {
            None
        } else {
            let body = change_body(&change, &defaults);
            Some(commit(state, &held, work, "Umschlag: Anordnung geändert".into(), body, "cover-layout", None).await)
        }
    };
    Ok(answer(state, work, mirrored).await)
}

/// `cover.layout_reset`: Zeilen der Flächen `faces` löschen (leer = alle), `area` setzt den Titelbereich.
pub async fn layout_reset(
    state: &AppState,
    work: WorkId,
    faces: &[CoverFace],
    area: Option<TitleArea>,
) -> Result<Value, RpcError> {
    let mirrored = {
        let held = state.write_lock.lock().await;
        let area_before = state.store.book_meta(work).await?.title_area;
        let change = state.store.reset_cover_blocks(work, faces, area).await?;
        let area_after = state.store.book_meta(work).await?.title_area;
        if change.changed.is_empty() && area_before == area_after {
            None
        } else {
            let summary = match area {
                Some(area) if faces == [CoverFace::Front] => format!("Umschlag: Schnellwahl {}", area_word(area)),
                _ => "Umschlag: Anordnung zurückgesetzt".to_owned(),
            };
            let mut body = change_body(&change, &[]);
            if area_before != area_after {
                body.push_str(&format!("Titelbereich: {} → {}\n", area_word(area_before), area_word(area_after)));
            }
            Some(commit(state, &held, work, summary, body, "cover-layout-reset", None).await)
        }
    };
    Ok(answer(state, work, mirrored).await)
}

/// `cover.fonts`: alle Schriftfamilien und die Vorgabeschrift des Werks.
pub async fn fonts(state: &AppState, work: WorkId) -> Result<Value, RpcError> {
    let families = tokio::task::spawn_blocking(baruch_render::font_families)
        .await
        .map_err(|e| RpcError::new(INTERNAL, format!("Schriften: {e}")))?;
    let default = default_font(state, work).await?;
    Ok(json!({ "families": families, "default": default }))
}

/// Antrag als veraltet schließen (unter der Schreibsperre).
async fn stale(state: &AppState, work: WorkId, id: i64, message: &str) -> Result<Decided, RpcError> {
    let _held = state.write_lock.lock().await;
    let request = state.store.asset_request_decide(work, id, Decision::Stale, HUMAN_NAME, Some(message)).await?;
    Ok(Decided { request, commit: None, mirror: None, cover: None })
}

/// Bestätigten Antrag `cover_layout` ausführen (bzw. als `stale` schließen, wenn `expected_layout` nicht mehr passt
/// oder ein Block nicht mehr gültig ist). Aufgerufen von `asset_requests::decide` nach der Prüfung „noch offen“.
pub async fn decide_request(state: &AppState, work: WorkId, request: &AssetRequest) -> Result<Decided, RpcError> {
    let AssetOp::CoverLayout { blocks, expected_layout } = &request.op else {
        return Err(RpcError::new(INVALID_PARAMS, "Dieser Antrag betrifft nicht die Umschlag-Anordnung."));
    };
    let defaults = defaults(state, work).await;
    let held = state.write_lock.lock().await;
    let now = Store::cover_layout_fingerprint(&state.store.cover_layout(work).await?);
    if &now != expected_layout {
        drop(held);
        return stale(
            state,
            work,
            request.id,
            "Die Umschlag-Anordnung wurde inzwischen geändert; der Antrag bezog sich auf den Stand davor.",
        )
        .await;
    }
    let change = match state.store.set_cover_blocks(work, blocks).await {
        Ok(change) => change,
        Err(e @ (StoreError::Refused(_) | StoreError::Asset(_) | StoreError::CoverBlock(_))) => {
            drop(held);
            return stale(state, work, request.id, &format!("Nicht mehr ausführbar: {e}")).await;
        }
        Err(e) => return Err(e.into()),
    };
    let decided = state.store.asset_request_decide(work, request.id, Decision::Confirmed, HUMAN_NAME, None).await?;
    let body = change_body(&change, &defaults);
    let result =
        commit(state, &held, work, "Umschlag: Anordnung geändert".into(), body, "cover-layout", Some(&decided)).await;
    let (commit, mirror) = match result {
        Ok(commit) => (commit, None),
        Err(e) => (None, Some(e)),
    };
    Ok(Decided { request: decided, commit, mirror, cover: None })
}

pub async fn call(state: &AppState, work: WorkId, method: &str, params: Value) -> Result<Value, RpcError> {
    match method {
        "cover.layout" => {
            let view = layout_view(state, work, None).await?;
            Ok(json!(view))
        }
        "cover.layout_set" => {
            let blocks = parse_blocks(params.get("blocks"), |_| None).map_err(|e| RpcError::new(INVALID_PARAMS, e))?;
            layout_set(state, work, blocks).await
        }
        "cover.layout_reset" => {
            let faces = match params.get("faces") {
                None | Some(Value::Null) => Vec::new(),
                Some(value) => serde_json::from_value::<Vec<CoverFace>>(value.clone())
                    .map_err(|_| RpcError::new(INVALID_PARAMS, "faces: Liste aus „front“, „spine“, „back“ erwartet"))?,
            };
            let area = match params.get("area") {
                None | Some(Value::Null) => None,
                Some(Value::String(word)) => Some(TitleArea::parse(word).ok_or_else(|| {
                    RpcError::new(
                        INVALID_PARAMS,
                        format!("area: „top“, „middle“ oder „bottom“ erwartet, nicht „{word}“"),
                    )
                })?),
                Some(_) => return Err(RpcError::new(INVALID_PARAMS, "area: „top“, „middle“ oder „bottom“ erwartet")),
            };
            layout_reset(state, work, &faces, area).await
        }
        "cover.fonts" => fonts(state, work).await,
        _ => Err(RpcError::new(METHOD_NOT_FOUND, format!("unbekannte Methode {method}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block() -> CoverBlock {
        CoverBlock {
            kind: CoverBlockKind::FrontTitle,
            x_mm: 12.0,
            y_mm: 20.0,
            width_mm: 100.0,
            align: CoverAlign::Center,
            size_pt: 24.0,
            leading_em: 0.65,
            color: "light".into(),
            font: None,
            style: FontStyle::Normal,
            backdrop: Backdrop::None,
            visible: true,
            direction: None,
            text: None,
        }
    }

    #[test]
    fn the_body_names_block_field_and_both_values() {
        let before = block();
        let mut after = before.clone();
        after.x_mm = 15.0;
        after.style = FontStyle::Bold;
        let change = CoverLayoutChange {
            changed: vec![CoverBlockKind::FrontTitle],
            before: CoverLayout { blocks: vec![before] },
            after: CoverLayout { blocks: vec![after] },
        };
        let body = change_body(&change, &[]);
        assert!(body.starts_with("Titel (Vorderseite): x 12,0 → 15,0 mm"), "Vertrag 1.10, Muster des Rumpfs: {body}");
        assert!(body.contains("Schnitt normal → fett"), "jedes geänderte Feld: {body}");
        assert!(!body.contains("Breite"), "unveränderte Felder fehlen: {body}");
        // Stand der Block vorher nicht in der Datenbank, zählt der Vorgabeblock als „vorher“.
        let fresh = CoverLayoutChange { changed: change.changed.clone(), before: CoverLayout::default(), ..change };
        let body = change_body(&fresh, &[block()]);
        assert!(body.contains("x 12,0 → 15,0 mm") && body.contains("(vorher Vorgabe)"), "gegen die Vorgabe: {body}");
        assert!(change_body(&fresh, &[]).contains(": neu (x 15,0 mm"), "ohne Vorgabe alle Werte: {body}");
    }

    #[test]
    fn json_blocks_name_kind_and_field_on_errors() {
        let full = json!(block());
        assert_eq!(parse_block(0, &full, |_| None).unwrap(), block(), "serde-Form liest sich zurück");
        let mut with_view = full.clone();
        with_view["stored"] = json!(true);
        with_view["height_mm"] = json!(8.5);
        assert!(parse_block(0, &with_view, |_| None).is_ok(), "Felder aus cover.layout werden übergangen");
        let mut bad = full.clone();
        bad["x_mm"] = json!("zwölf");
        let e = parse_block(0, &bad, |_| None).unwrap_err();
        assert!(e.contains("front_title") && e.contains("x_mm"), "Art und Feld: {e}");
        let e = parse_block(2, &json!({ "kind": "front_cloud" }), |_| None).unwrap_err();
        assert!(e.contains("Nr. 3") && e.contains("front_cloud") && e.contains("kind"), "unbekannte Art: {e}");
        let e = parse_block(0, &json!({ "kind": "back_blurb" }), |_| None).unwrap_err();
        assert!(
            e.contains("back_blurb") && e.contains("x_mm") && e.contains("fehlt"),
            "ohne Grundlage fehlt x_mm: {e}"
        );
        let partial = parse_block(0, &json!({ "kind": "front_title", "y_mm": 40 }), |_| Some(block())).unwrap();
        assert_eq!((partial.x_mm, partial.y_mm), (12.0, 40.0), "MCP: fehlende Felder aus dem wirksamen Block");
    }
}
