// SPDX-License-Identifier: AGPL-3.0-or-later
//! RPC of the typeset view editor (`wysiwyg.*`, contract Round 5): translating section texts into blocks and
//! back ([`baruch_wysiwyg`]), appearance according to the work template, typeset view diff of a proposal.
//! All methods only read: no writing into the database, no write lock, only through `/rpc`
//! (not `/act`, not MCP). Saving still goes through the editor form (`outline.save`).
//!
//! - `wysiwyg.parse {work?, body}` → `{blocks, html}`; islands with an image path show the image
//!   (`<img class="ws-image" src="/werke/{work}/bilder/pfad?p=…">`, see [`image_ref`])
//! - `wysiwyg.serialize {work?, body, edits}` → `{body}`; invalid instructions → `INVALID_PARAMS`; since Round 18b
//!   also `{replaces, raw}` (island in place, literally), a new part block is rejected
//! - `wysiwyg.style {work?}` → `{css, style}` from the work template
//! - `wysiwyg.diff {work?, key|node, rev, base?}` → `{html, base}`: revision `rev` against `base`; if
//!   `base` is missing, its parent (`based_on`) applies, otherwise the current revision

use std::collections::HashMap;

use baruch_store::{StoreError, WorkId};
use baruch_wysiwyg::{Edit, ImageRef};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::AppState;
use crate::i18n::tc;
use crate::rpc::{INVALID_PARAMS, RpcError, resolve_keys};

/// Tooltip of a cross reference in the typeset view in the interface language (Round 39c; `baruch-wysiwyg` writes
/// only the German text itself and takes this function through `render_html_titled`).
pub fn xref_title(target: &str) -> String {
    crate::i18n::tc("reference_msg.xref.tooltip", &[("target", target.into())])
}

/// Methods of this module.
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

/// Image for a path from the source text (`image("…")`): the page `/werke/{work}/bilder/pfad?p=…`
/// delivers the current image for the path (contract Round 6, "images"; without an image 404, then the
/// browser shows only the alternative text). `alt` is the alternative text from [`image_alts`] (empty if
/// no image is created for the path). The path goes into the address in normal form
/// ([`baruch_store::normalize_asset_path`]: `./bilder/a.png` and `/bilder/a.png` → `bilder/a.png`, Round 7;
/// since Round 18b also without a leading `/`, the page searches in normal form anyway). Without a normal form
/// (e.g. with `..`) the path stays as it is – the page then delivers no image.
pub fn image_ref(work: WorkId, path: &str, alt: &str) -> ImageRef {
    let path = baruch_store::normalize_asset_path(path).unwrap_or_else(|| path.to_owned());
    ImageRef {
        src: format!("/werke/{work}/bilder/pfad?p={}", crate::view::query_escape(&path)),
        alt: alt.to_owned(),
        missing: false,
    }
}

/// Like [`image_ref`], plus `missing` if no image is created in the work for the path (Round 11: the
/// typeset view reports „Bild fehlt: …“). `alts` from [`image_alts`].
pub fn image_ref_in(work: WorkId, path: &str, alts: &HashMap<String, String>) -> ImageRef {
    // No stored path has control characters (baruch_store::check_asset_path).
    let normal = baruch_store::normalize_asset_path(path).filter(|p| !p.chars().any(char::is_control));
    let known = normal.and_then(|p| alts.get(&p));
    ImageRef { missing: known.is_none(), ..image_ref(work, path, known.map_or("", String::as_str)) }
}

/// What editor and preview know about the images of the work (Round 11): all images (name, caption,
/// alternative text, paths in normal form) and the computed images of the work template
/// ([`baruch_typst::images::computed_images`]). Since Round 21c a field of its own `rpc::LibraryView::images`
/// ([`image_view`]); before, the view travelled as JSON under a reserved key in `image_alts`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ImageView {
    pub assets: Vec<AssetBrief>,
    pub template: Vec<TemplateImage>,
    /// Keys of the nodes whose heading Typst counts as a chapter
    /// ([`baruch_store::Store::counted_headings`]); `None` = not known for certain. Only filled if the
    /// template has computed images.
    #[serde(default)]
    pub counted: Option<Vec<i64>>,
}

/// An image of the work, in brief.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AssetBrief {
    pub name: String,
    pub caption: String,
    pub alt: String,
    /// Paths in normal form.
    pub paths: Vec<String>,
}

/// Computed image of the template (mirror of [`baruch_typst::images::ComputedImage`], serialisable).
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

/// Chapter image in the editor header ([`ImageView::chapter_image`]).
#[derive(Debug, Clone, PartialEq)]
pub enum ChapterImage {
    /// Determined for certain: path from the template and, if created, the image for it.
    Known { path: String, asset: Option<AssetBrief> },
    /// The template computes an image that cannot be determined for certain without Typst.
    FromTemplate { expr: String },
}

impl ImageView {
    /// Image for a path (compared in normal form).
    pub fn asset_by_path(&self, path: &str) -> Option<&AssetBrief> {
        let normal = baruch_store::normalize_asset_path(path)?;
        self.assets.iter().find(|a| a.paths.contains(&normal))
    }

    /// Image of the template for the chapter counter `chapter` (`counter(heading).get().first()`, 0 before the
    /// first numbered chapter, `None` = not known for certain): the first computed image that is determined for
    /// certain for this chapter (a negative index counts from the end like Typst's `array.at`); otherwise, if the
    /// template has computed images at all,
    /// [`ChapterImage::FromTemplate`]; without computed images `None`.
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
        // No image is determined for certain here: a `Fixed` would already have applied above.
        let expr = match self.template.first()? {
            TemplateImage::Unknown { expr } => expr.clone(),
            TemplateImage::PerChapter { list, offset } => {
                let at = chapter.map_or_else(|| tc("view_msg.wysiwyg.chapter_unknown", &[]), |c| c.to_string());
                tc(
                    "view_msg.wysiwyg.image_list",
                    &[("count", list.len().into()), ("offset", format!("{offset:+}").into()), ("at", at.into())],
                )
            }
            TemplateImage::Fixed { path } => path.clone(),
        };
        Some(ChapterImage::FromTemplate { expr })
    }
}

/// Path in normal form ([`baruch_store::normalize_asset_path`]) → alternative text of the current
/// revision, for all images of the work. The path in the text may begin with `/` or `./`, as for
/// `/werke/{id}/bilder/pfad`. Since Round 21c only paths (the [`ImageView`] is delivered by [`image_view`]).
pub async fn image_alts(state: &AppState, work: WorkId) -> Result<HashMap<String, String>, StoreError> {
    Ok(alts_and_briefs(state.store.asset_list(work).await?).0)
}

/// Table like [`image_alts`] and the images in brief ([`AssetBrief`]) from the image list.
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

/// Table like [`image_alts`] and the [`ImageView`] (images and computed images of the work template) in one pass
/// (Round 21c: for `rpc::LibraryView::image_alts` and `rpc::LibraryView::images`).
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
            // Round 39b: the page number addition of a cross reference in the work language.
            let lang = state.store.work_language(work).await?;
            let html = baruch_wysiwyg::render_html_titled(
                &doc,
                &|path| Some(image_ref_in(work, path, &alts)),
                lang,
                &xref_title,
            );
            Ok(json!({ "blocks": doc.blocks, "html": html }))
        }
        "wysiwyg.serialize" => {
            let p: SerializeParams = params(params_value)?;
            let doc = baruch_wysiwyg::parse(&p.body);
            // Round 39b: a newly written cross reference with page number gets the abbreviation of the work language
            // (`p.` in an English work), not always the German `S.`.
            let lang = state.store.work_language(work).await?;
            let body = baruch_wysiwyg::serialize_in(&doc, &p.edits, lang)
                .map_err(|e| RpcError::new(INVALID_PARAMS, e.to_string()))?;
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
                    // Like the variants in the reading flow (view.rs): the parent, otherwise the current revision.
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
            let lang = state.store.work_language(work).await?;
            let images = |path: &str| Some(image_ref_in(work, path, &alts));
            let html = baruch_wysiwyg::diff_html_in(base_body, &shown.body, &images, lang);
            Ok(json!({ "html": html, "base": base }))
        }
        _ => Err(RpcError::new(crate::rpc::METHOD_NOT_FOUND, format!("unknown method {method}"))),
    }
}
