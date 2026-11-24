// SPDX-License-Identifier: AGPL-3.0-or-later
//! RPC des Satzbild-Editors (`wysiwyg.*`, Vertrag Runde 5): Abschnittstexte in Blöcke übersetzen und
//! zurück ([`baruch_wysiwyg`]), Darstellung nach der Werkvorlage, Satzbild-Diff eines Vorschlags.
//! Alle Methoden lesen nur: kein Schreiben in die Datenbank, keine Schreibsperre, nur über `/rpc`
//! (nicht `/act`, nicht MCP). Gespeichert wird weiter über das Editor-Formular (`outline.save`).
//!
//! - `wysiwyg.parse {work?, body}` → `{blocks, html}`; Inseln mit Bildpfad zeigen das Bild
//!   (`<img class="ws-image" src="/werke/{work}/bilder/pfad?p=…">`, siehe [`image_ref`])
//! - `wysiwyg.serialize {work?, body, edits}` → `{body}`; ungültige Anweisungen → `INVALID_PARAMS`; seit Runde 18b
//!   auch `{replaces, raw}` (Insel an Ort und Stelle, wörtlich), ein neuer Teil-Block wird abgelehnt
//! - `wysiwyg.style {work?}` → `{css, style}` aus der Werkvorlage
//! - `wysiwyg.diff {work?, key|node, rev, base?}` → `{html, base}`: Fassung `rev` gegen `base`; fehlt
//!   `base`, gilt ihr Elternteil (`based_on`), sonst die aktuelle Fassung

use std::collections::HashMap;

use baruch_store::{StoreError, WorkId};
use baruch_wysiwyg::{Edit, ImageRef};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::AppState;
use crate::rpc::{INVALID_PARAMS, RpcError, resolve_keys};

/// Methoden dieses Moduls.
pub fn handles(method: &str) -> bool {
    matches!(method, "wysiwyg.parse" | "wysiwyg.serialize" | "wysiwyg.style" | "wysiwyg.diff")
}

#[derive(Deserialize)]
struct ParseParams {
    body: String,
}

#[derive(Deserialize)]
struct SerializeParams {
    body: String,
    edits: Vec<Edit>,
}

#[derive(Deserialize)]
struct DiffParams {
    node: usize,
    rev: u32,
    base: Option<u32>,
}

/// Bild zu einem Pfad aus dem Quelltext (`image("…")`): die Seite `/werke/{work}/bilder/pfad?p=…`
/// liefert das aktuelle Bild zum Pfad (Vertrag Runde 6, „images“; ohne Bild 404, dann zeigt der
/// Browser nur den Alternativtext). `alt` ist der Alternativtext aus [`image_alts`] (leer, wenn
/// zum Pfad kein Bild angelegt ist). Der Pfad geht in Normalform in die Adresse
/// ([`baruch_store::normalize_asset_path`]: `./bilder/a.png` und `/bilder/a.png` → `bilder/a.png`, Runde 7;
/// seit Runde 18b auch ohne führendes `/`, die Seite sucht ohnehin in Normalform). Ohne Normalform (z. B. mit
/// `..`) bleibt der Pfad, wie er ist – die Seite liefert dann kein Bild.
pub fn image_ref(work: WorkId, path: &str, alt: &str) -> ImageRef {
    let path = baruch_store::normalize_asset_path(path).unwrap_or_else(|| path.to_owned());
    ImageRef {
        src: format!("/werke/{work}/bilder/pfad?p={}", crate::view::query_escape(&path)),
        alt: alt.to_owned(),
        missing: false,
    }
}

/// Wie [`image_ref`], dazu `missing`, wenn zum Pfad kein Bild im Werk angelegt ist (Runde 11: das
/// Satzbild meldet „Bild fehlt: …“). `alts` aus [`image_alts`].
pub fn image_ref_in(work: WorkId, path: &str, alts: &HashMap<String, String>) -> ImageRef {
    // Steuerzeichen hat kein gespeicherter Pfad (baruch_store::check_asset_path).
    let normal = baruch_store::normalize_asset_path(path).filter(|p| !p.chars().any(char::is_control));
    let known = normal.and_then(|p| alts.get(&p));
    ImageRef { missing: known.is_none(), ..image_ref(work, path, known.map_or("", String::as_str)) }
}

/// Was Editor und Vorschau über die Bilder des Werks wissen (Runde 11): alle Bilder (Name, Unterschrift,
/// Alternativtext, Pfade in Normalform) und die berechneten Bilder der Werkvorlage
/// ([`baruch_typst::images::computed_images`]). Seit Runde 21c ein eigenes Feld `rpc::LibraryView::images`
/// ([`image_view`]); vorher reiste die Ansicht als JSON unter einem reservierten Schlüssel in `image_alts`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ImageView {
    pub assets: Vec<AssetBrief>,
    pub template: Vec<TemplateImage>,
    /// Schlüssel der Knoten, deren Überschrift Typst als Kapitel zählt
    /// ([`baruch_store::Store::counted_headings`]); `None` = nicht sicher bekannt. Nur gefüllt, wenn die
    /// Vorlage berechnete Bilder hat.
    #[serde(default)]
    pub counted: Option<Vec<i64>>,
}

/// Ein Bild des Werks, kurz.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AssetBrief {
    pub name: String,
    pub caption: String,
    pub alt: String,
    /// Pfade in Normalform.
    pub paths: Vec<String>,
}

/// Berechnetes Bild der Vorlage (Spiegel von [`baruch_typst::images::ComputedImage`], serialisierbar).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TemplateImage {
    PerChapter { list: Vec<String>, offset: i64 },
    Fixed { path: String },
    Unknown { expr: String },
}

impl From<baruch_typst::images::ComputedImage> for TemplateImage {
    fn from(image: baruch_typst::images::ComputedImage) -> Self {
        use baruch_typst::images::ComputedImage as C;
        match image {
            C::PerChapter { list, offset } => TemplateImage::PerChapter { list, offset },
            C::Fixed { path } => TemplateImage::Fixed { path },
            C::Unknown { expr } => TemplateImage::Unknown { expr },
        }
    }
}

/// Kapitelbild im Editorkopf ([`ImageView::chapter_image`]).
#[derive(Debug, Clone, PartialEq)]
pub enum ChapterImage {
    /// Sicher ermittelt: Pfad aus der Vorlage und, falls angelegt, das Bild dazu.
    Known { path: String, asset: Option<AssetBrief> },
    /// Die Vorlage berechnet ein Bild, das sich ohne Typst nicht sicher ermitteln lässt.
    FromTemplate { expr: String },
}

impl ImageView {
    /// Bild zu einem Pfad (verglichen in Normalform).
    pub fn asset_by_path(&self, path: &str) -> Option<&AssetBrief> {
        let normal = baruch_store::normalize_asset_path(path)?;
        self.assets.iter().find(|a| a.paths.contains(&normal))
    }

    /// Bild der Vorlage für den Kapitelzähler `chapter` (`counter(heading).get().first()`, 0 vor dem
    /// ersten nummerierten Kapitel, `None` = nicht sicher bekannt): das erste berechnete Bild, das für dieses Kapitel sicher ermittelt
    /// ist (ein negativer Index zählt wie Typsts `array.at` vom Ende); sonst, wenn die Vorlage überhaupt berechnete Bilder hat,
    /// [`ChapterImage::FromTemplate`]; ohne berechnete Bilder `None`.
    pub fn chapter_image(&self, chapter: Option<usize>) -> Option<ChapterImage> {
        for image in &self.template {
            let path = match image {
                TemplateImage::PerChapter { list, offset } => {
                    let index = chapter.and_then(|c| i64::try_from(c).ok()).and_then(|c| c.checked_add(*offset));
                    let len = i64::try_from(list.len()).ok();
                    let index = index.zip(len).map(|(i, len)| if i < 0 { len + i } else { i });
                    index.and_then(|i| usize::try_from(i).ok()).and_then(|i| list.get(i))
                }
                TemplateImage::Fixed { path } => Some(path),
                TemplateImage::Unknown { .. } => None,
            };
            if let Some(path) = path {
                return Some(ChapterImage::Known { path: path.clone(), asset: self.asset_by_path(path).cloned() });
            }
        }
        // Hier ist kein Bild sicher ermittelt: ein `Fixed` hätte oben schon gegriffen.
        let expr = match self.template.first()? {
            TemplateImage::Unknown { expr } => expr.clone(),
            TemplateImage::PerChapter { list, offset } => {
                let at = chapter
                    .map_or_else(|| "unbekannt (Nummerierung der Vorlage nicht lesbar)".to_owned(), |c| c.to_string());
                format!(
                    "Liste mit {} Bildern, Index Kapitel {offset:+}; Kapitelzähler {at} liegt außerhalb",
                    list.len()
                )
            }
            TemplateImage::Fixed { path } => path.clone(),
        };
        Some(ChapterImage::FromTemplate { expr })
    }
}

/// Pfad in Normalform ([`baruch_store::normalize_asset_path`]) → Alternativtext der aktuellen
/// Fassung, für alle Bilder des Werks. Der Pfad im Text darf mit `/` oder `./` beginnen, wie bei
/// `/werke/{id}/bilder/pfad`. Seit Runde 21c nur noch Pfade (die [`ImageView`] liefert [`image_view`]).
pub async fn image_alts(state: &AppState, work: WorkId) -> Result<HashMap<String, String>, StoreError> {
    Ok(alts_and_briefs(state.store.asset_list(work).await?).0)
}

/// Tabelle wie [`image_alts`] und die Bilder kurz ([`AssetBrief`]) aus der Bilderliste.
fn alts_and_briefs(list: Vec<baruch_store::AssetSummary>) -> (HashMap<String, String>, Vec<AssetBrief>) {
    let mut alts = HashMap::new();
    let mut briefs = Vec::with_capacity(list.len());
    for asset in list {
        let mut paths = Vec::new();
        for path in &asset.paths {
            if let Some(normal) = baruch_store::normalize_asset_path(path) {
                alts.insert(normal.clone(), asset.alt.clone());
                paths.push(normal);
            }
        }
        briefs.push(AssetBrief { name: asset.name, caption: asset.caption, alt: asset.alt, paths });
    }
    (alts, briefs)
}

/// Tabelle wie [`image_alts`] und die [`ImageView`] (Bilder und berechnete Bilder der Werkvorlage) in einem Gang
/// (Runde 21c: für `rpc::LibraryView::image_alts` und `rpc::LibraryView::images`).
pub async fn image_view(state: &AppState, work: WorkId) -> Result<(HashMap<String, String>, ImageView), StoreError> {
    let (alts, assets) = alts_and_briefs(state.store.asset_list(work).await?);
    let mut view = ImageView { assets, ..ImageView::default() };
    let template = state.store.work_template(work).await?;
    view.template = baruch_typst::images::computed_images(&template).into_iter().map(TemplateImage::from).collect();
    if !view.template.is_empty() {
        view.counted = state.store.counted_headings(work).await?;
    }
    Ok((alts, view))
}

fn params<T: for<'de> Deserialize<'de>>(value: Value) -> Result<T, RpcError> {
    let value = if value.is_null() { json!({}) } else { value };
    serde_json::from_value(value).map_err(|e| RpcError::new(INVALID_PARAMS, e.to_string()))
}

pub async fn call(state: &AppState, work: WorkId, method: &str, params_value: Value) -> Result<Value, RpcError> {
    match method {
        "wysiwyg.parse" => {
            let p: ParseParams = params(params_value)?;
            let doc = baruch_wysiwyg::parse(&p.body);
            let alts = image_alts(state, work).await?;
            let html = baruch_wysiwyg::render_html_with(&doc, &|path| Some(image_ref_in(work, path, &alts)));
            Ok(json!({ "blocks": doc.blocks, "html": html }))
        }
        "wysiwyg.serialize" => {
            let p: SerializeParams = params(params_value)?;
            let doc = baruch_wysiwyg::parse(&p.body);
            let body =
                baruch_wysiwyg::serialize(&doc, &p.edits).map_err(|e| RpcError::new(INVALID_PARAMS, e.to_string()))?;
            Ok(json!({ "body": body }))
        }
        "wysiwyg.style" => {
            let style = crate::sheet::sheet_style_for(state, work).await?;
            Ok(json!({ "css": baruch_wysiwyg::sheet_css(&style), "style": style }))
        }
        "wysiwyg.diff" => {
            let p: DiffParams = params(resolve_keys(state, work, params_value).await?)?;
            let loaded = state.store.load_work(work).await?;
            let node = loaded.nodes.get(p.node).ok_or(StoreError::UnknownNode { work, node: p.node })?;
            let unknown = |rev: u32| StoreError::UnknownRevision {
                node: p.node,
                rev,
                existing: node.revisions.iter().map(|r| r.number).collect(),
            };
            let shown = node.revision(p.rev).ok_or_else(|| unknown(p.rev))?;
            let base = match p.base {
                Some(base) => node.revision(base).ok_or_else(|| unknown(base))?.number,
                None => {
                    // Wie die Varianten im Lesefluss (view.rs): das Elternteil, sonst die aktuelle Fassung.
                    let tree = state.store.revision_tree(work).await?;
                    let parent = tree
                        .iter()
                        .find(|n| n.node == p.node)
                        .and_then(|n| n.revisions.iter().find(|r| r.number == p.rev))
                        .and_then(|info| info.based_on)
                        .filter(|&b| node.revision(b).is_some());
                    parent.or(node.current_number()).unwrap_or(p.rev)
                }
            };
            let base_body = node.revision(base).map_or("", |r| r.body.as_str());
            let alts = image_alts(state, work).await?;
            let html =
                baruch_wysiwyg::diff_html_with(base_body, &shown.body, &|path| Some(image_ref_in(work, path, &alts)));
            Ok(json!({ "html": html, "base": base }))
        }
        _ => Err(RpcError::new(crate::rpc::METHOD_NOT_FOUND, format!("unbekannte Methode {method}"))),
    }
}
