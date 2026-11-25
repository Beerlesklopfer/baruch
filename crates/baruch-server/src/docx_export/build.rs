// SPDX-License-Identifier: AGPL-3.0-or-later
//! Das Werk als [`DocxDocument`]: Titelei, aktuelle Fassungen in Lesereihenfolge, Bilder und – auf Wunsch – je
//! Abschnitt der jüngste offene Vorschlag als Änderungsverfolgung. Nur lesende Zugriffe auf den Speicher.

use std::collections::{BTreeMap, HashMap};

use baruch_core::{Author, Node, Revision, RevisionStatus, revision_fold};
use baruch_docx::diff::track_changes;
use baruch_docx::from_typst::{WARN_ISLAND, from_typst};
use baruch_docx::{Block, Change, ChangeKind, DocxDocument, DocxImage, DocxMeta, DocxWarning, Inline};
use baruch_store::{StoreError, WorkId};

use super::{DocxExportOptions, WARN_MORE_PROPOSALS};
use crate::AppState;
use crate::rpc::RpcError;

/// So viele einzelne Meldungen über Typst-Inseln stehen in der Antwort; der Rest wird gezählt.
const MAX_ISLAND_WARNINGS: usize = 5;

/// Ergebnis von [`document`].
pub(super) struct Built {
    pub document: DocxDocument,
    /// Jüngster Zeitpunkt des Stands (für `docProps/core.xml`); gleicher Stand → gleiche Datei.
    pub modified: String,
    /// Zahl der Vorschläge, die als Änderungsverfolgung im Dokument stehen.
    pub proposals: usize,
    pub warnings: Vec<DocxWarning>,
}

/// Offene Vorschläge des Werks, wie die Oberfläche sie zählt (ohne ausgeblendete Fassungen).
pub(super) struct OpenProposals {
    /// Abschnitte mit mindestens einem offenen Vorschlag.
    pub sections: usize,
    pub total: usize,
}

/// Offene Vorschläge eines Abschnitts nach Nummer: Status „vorgeschlagen“ und nicht unter der Marke „bis hierhin“
/// ausgeblendet (dieselbe Regel wie in der Oberfläche, [`revision_fold::is_hidden`]).
fn open_of(node: &Node, mark: Option<u32>) -> Vec<&Revision> {
    let mut open: Vec<&Revision> = node
        .revisions
        .iter()
        .filter(|r| r.status == RevisionStatus::Proposed && !revision_fold::is_hidden(node, mark, r))
        .collect();
    open.sort_by_key(|r| r.number);
    open
}

pub(super) async fn open_proposals(state: &AppState, work: WorkId) -> Result<OpenProposals, StoreError> {
    let loaded = state.store.load_work(work).await?;
    let folds = state.store.work_revision_folds(work).await?;
    let counts: Vec<usize> =
        loaded.nodes.iter().map(|node| open_of(node, folds.get(&node.key).copied()).len()).collect();
    Ok(OpenProposals { sections: counts.iter().filter(|n| **n > 0).count(), total: counts.iter().sum() })
}

/// Zeitpunkt der Datenbank (`JJJJ-MM-TTThh:mm:ss[.fff][Z]`, auch mit Leerzeichen statt `T`) als ISO 8601 UTC ohne
/// Sekundenbruchteile; unlesbar → `None`.
fn normalize_stamp(stamp: &str) -> Option<String> {
    let head: String = stamp.trim().chars().take(19).collect();
    let well_formed = head.len() == 19
        && head.char_indices().all(|(i, c)| match i {
            4 | 7 => c == '-',
            10 => c == 'T' || c == ' ',
            13 | 16 => c == ':',
            _ => c.is_ascii_digit(),
        });
    well_formed.then(|| format!("{}T{}Z", &head[..10], &head[11..]))
}

/// Alternativtext aus den Bildangaben des Werks eintragen, wo der Quelltext keinen nennt.
fn fill_alt(blocks: &mut [Block], alts: &[String]) {
    for block in blocks {
        match block {
            Block::Figure { image, alt, .. } if alt.trim().is_empty() => {
                *alt = alts.get(*image).cloned().unwrap_or_default();
            }
            Block::Quote { blocks, .. } | Block::Typst { shown: blocks, .. } => fill_alt(blocks, alts),
            Block::Table { rows, .. } => rows.iter_mut().flatten().for_each(|cell| fill_alt(cell, alts)),
            _ => {}
        }
    }
}

fn author_handle(author: &Author) -> &str {
    match author {
        Author::Assistant { persona, .. } => persona,
        Author::Human { name } => name,
    }
}

/// Meldungen ordnen: gleiche nur einmal, von den Typst-Inseln die ersten [`MAX_ISLAND_WARNINGS`] einzeln und der
/// Rest als Zahl.
fn tidy(warnings: Vec<DocxWarning>) -> Vec<DocxWarning> {
    let mut out: Vec<DocxWarning> = Vec::new();
    let (mut islands, mut more_islands) = (0usize, 0usize);
    for warning in warnings {
        if out.contains(&warning) {
            continue;
        }
        if warning.code == WARN_ISLAND {
            islands += 1;
            if islands > MAX_ISLAND_WARNINGS {
                more_islands += 1;
                continue;
            }
        }
        out.push(warning);
    }
    if more_islands > 0 {
        out.push(DocxWarning {
            code: WARN_ISLAND.to_owned(),
            message: format!(
                "{more_islands} weitere Typst-Inseln stehen ebenfalls nur als sichtbarer Quelltext in der Datei."
            ),
        });
    }
    out
}

/// Steckt in den Blöcken eine verfolgte Änderung?
fn has_changes(blocks: &[Block]) -> bool {
    fn inlines(items: &[Inline]) -> bool {
        items.iter().any(|item| matches!(item, Inline::Tracked { .. }))
    }
    blocks.iter().any(|block| match block {
        Block::Part { content, mark } | Block::Heading { content, mark, .. } | Block::Paragraph { content, mark } => {
            mark.is_some() || inlines(content)
        }
        Block::Quote { blocks, attribution } => has_changes(blocks) || inlines(attribution),
        Block::List { items, .. } => items.iter().any(|item| item.mark.is_some() || inlines(&item.content)),
        Block::Table { rows, .. } => rows.iter().flatten().any(|cell| has_changes(cell)),
        Block::Figure { caption, .. } => inlines(caption),
        Block::Typst { .. } | Block::PageBreak => false,
    })
}

pub(super) async fn document(state: &AppState, work: WorkId, options: &DocxExportOptions) -> Result<Built, RpcError> {
    let store = &state.store;
    let meta = store.work_meta(work).await?;
    let loaded = store.load_work(work).await?;
    let tree = store.revision_tree(work).await?;
    let folds = if options.proposals { store.work_revision_folds(work).await? } else { HashMap::new() };

    // Bilder: aktuelle Fassung je Pfad; eingebettet wird, was Word sicher zeigt (PNG, JPEG, GIF).
    let asset_list = store.asset_list(work).await?;
    let mut alt_of: BTreeMap<String, String> = BTreeMap::new();
    for asset in &asset_list {
        for path in &asset.paths {
            if let Some(normal) = baruch_store::normalize_asset_path(path) {
                alt_of.insert(normal, asset.alt.clone());
            }
        }
    }
    let (mut images, mut paths, mut alts) = (Vec::new(), Vec::new(), Vec::new());
    for file in store.export_assets(work).await? {
        let Some(media_type) =
            baruch_store::detect_mime(&file.bytes).filter(|m| matches!(*m, "image/png" | "image/jpeg" | "image/gif"))
        else {
            continue;
        };
        alts.push(alt_of.get(&file.path).cloned().unwrap_or_default());
        paths.push(file.path.clone());
        images.push(DocxImage {
            file_name: file.path.rsplit('/').next().unwrap_or(&file.path).to_owned(),
            path: file.path,
            media_type: media_type.to_owned(),
            bytes: file.bytes,
        });
    }

    // Zeitpunkte der Fassungen: für das Datum der Änderung und den Stand der Datei.
    let mut created: HashMap<(usize, u32), String> = HashMap::new();
    let mut stamps: Vec<String> = vec![meta.created.clone()];
    stamps.extend(meta.updated.clone());
    stamps.extend(asset_list.iter().map(|a| a.updated.clone()));
    for node in &tree {
        for revision in &node.revisions {
            created.insert((node.node, revision.number), revision.created.clone());
            stamps.push(revision.created.clone());
            stamps.extend(revision.edited.clone());
        }
    }
    let modified =
        stamps.iter().filter_map(|s| normalize_stamp(s)).max().unwrap_or_else(|| "2000-01-01T00:00:00Z".to_owned());

    let (mut blocks, mut warnings, mut proposals) = (Vec::new(), Vec::new(), 0usize);
    for node in &loaded.nodes {
        let Some(current) = node.current_index().map(|i| &node.revisions[i]) else { continue };
        let place =
            if node.title.trim().is_empty() { "ohne Titel".to_owned() } else { format!("„{}“", node.title.trim()) };
        let read = |body: &str, warnings: &mut Vec<DocxWarning>| -> Vec<Block> {
            let mut read = from_typst(body, &paths);
            fill_alt(&mut read.blocks, &alts);
            warnings.extend(
                read.warnings
                    .into_iter()
                    .map(|w| DocxWarning { code: w.code, message: format!("Abschnitt {place}: {}", w.message) }),
            );
            read.blocks
        };
        let current_blocks = read(&current.body, &mut warnings);
        let open = if options.proposals { open_of(node, folds.get(&node.key).copied()) } else { Vec::new() };
        let Some(latest) = open.last() else {
            blocks.extend(current_blocks);
            continue;
        };
        let proposed_blocks = read(&latest.body, &mut warnings);
        let change = Change {
            // Die Art je Stelle setzt `track_changes` selbst.
            kind: ChangeKind::Insert,
            author: author_handle(&latest.author).to_owned(),
            date: created.get(&(node.id, latest.number)).and_then(|s| normalize_stamp(s)).unwrap_or_default(),
        };
        let tracked = track_changes(&current_blocks, &proposed_blocks, &change);
        // Ein Vorschlag, der der aktuellen Fassung gleicht, hinterlässt keine Änderung und zählt nicht.
        if has_changes(&tracked) {
            proposals += 1;
        }
        blocks.extend(tracked);
        if open.len() > 1 {
            let more = open.len() - 1;
            warnings.push(DocxWarning {
                code: WARN_MORE_PROPOSALS.to_owned(),
                message: format!(
                    "Abschnitt {place}: {more} {} nicht in der Datei – als Änderungsverfolgung steht nur der jüngste \
                     (Fassung {} von {}).",
                    if more == 1 { "weiterer offener Vorschlag steht" } else { "weitere offene Vorschläge stehen" },
                    latest.number,
                    change.author,
                ),
            });
        }
    }

    // Sprache und Impressum wie im EPUB: Satz-Einstellungen vor Werkvorlage; Impressum nur mit Titelei.
    let typeset = crate::cover::typeset_settings(state, work).await?;
    let lang = typeset
        .lang
        .clone()
        .filter(|l| baruch_epub::template::is_language_tag(l.trim()))
        .or_else(|| baruch_epub::template::template_lang(&meta.template))
        .unwrap_or_else(|| "de".to_owned());
    let book = store.book_meta(work).await?;
    let imprint = if book.front_matter {
        let info = crate::cover::book_info(state, work, &book).await?;
        baruch_typst::cover::imprint_lines(&info).into_iter().filter(|line| !line.trim().is_empty()).collect()
    } else {
        Vec::new()
    };

    Ok(Built {
        document: DocxDocument {
            meta: DocxMeta { title: meta.title, subtitle: meta.subtitle, author: meta.author, lang, imprint },
            blocks,
            comments: Vec::new(),
            images,
        },
        modified,
        proposals,
        warnings: tidy(warnings),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamps_of_the_database_become_iso_utc() {
        assert_eq!(
            normalize_stamp("2026-11-24T18:05:09.123Z").as_deref(),
            Some("2026-11-24T18:05:09Z"),
            "Sekundenbruchteile entfallen – Word nimmt den Zeitpunkt sonst nicht überall an"
        );
        assert_eq!(
            normalize_stamp("2026-11-24 18:05:09").as_deref(),
            Some("2026-11-24T18:05:09Z"),
            "die Form von SQLite mit Leerzeichen ist dieselbe Zeit in UTC"
        );
        assert_eq!(normalize_stamp("gestern"), None, "Unlesbares wird kein Datum");
    }

    #[test]
    fn many_island_warnings_are_counted_instead_of_listed() {
        let island = |n: usize| DocxWarning { code: WARN_ISLAND.to_owned(), message: format!("Insel {n}") };
        let mut all: Vec<DocxWarning> = (0..8).map(island).collect();
        all.push(island(0));
        all.push(DocxWarning { code: "docx_bild_fehlt".into(), message: "Bild".into() });
        let tidy = tidy(all);
        assert_eq!(tidy.len(), MAX_ISLAND_WARNINGS + 2, "fünf einzelne Inseln, das Bild und eine Zeile mit dem Rest");
        assert!(
            tidy.last().is_some_and(|w| w.message.starts_with("3 weitere Typst-Inseln")),
            "die doppelte Meldung zählt nicht, die übrigen drei stehen als Zahl da: {tidy:?}"
        );
        assert!(tidy.iter().any(|w| w.code == "docx_bild_fehlt"), "andere Meldungen bleiben vollständig");
    }
}
