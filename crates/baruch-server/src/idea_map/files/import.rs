// SPDX-License-Identifier: AGPL-3.0-or-later
//! Importing foreign mind maps (contract Round 34 §8, owner mindmap_austausch): limits, cleaning and the path from
//! the file read to the [`ImportTree`].
//!
//! A foreign file is foreign input. The readers ([`super::freemind::read`], [`super::xmind::read`]) return a
//! raw tree ([`Raw`]); [`finish`] turns it into the [`ImportTree`] and [`check`] enforces the limits:
//! at most [`MAX_IMPORT_NODES`] nodes, depth at most [`MAX_TREE_DEPTH`], titles at most [`MAX_TITLE_CHARS`]
//! and notes at most [`MAX_NOTE_CHARS`] characters (shortened, with a note in `warnings`), control characters removed.
//! [`check`] runs a second time when the preview is confirmed: the tree then comes back from a hidden
//! form field and is foreign input again.
//!
//! The root of the foreign file never becomes an idea itself: its title stands in `ImportTree::title`, its children
//! are `ImportTree::nodes`. The branch „Wolke“ from an own export ([`super::export::CLOUD_ID`]) is dissolved;
//! its children stand on the top level again and carry `ImportNode::cloud` (Round 36: they go
//! back into the cloud when created). The branch is recognised by its id, never by the title „Wolke“.

use std::collections::HashMap;

use baruch_store::{MAX_IDEA_WORDS, MAX_LINK_NOTE_CHARS, MAX_WORD_CHARS};

use crate::idea_map::tree::{ImportLink, ImportNode, ImportTree, MAX_IMPORT_NODES, MAX_TITLE_CHARS, MAX_TREE_DEPTH};

/// Largest uploaded file (also `DefaultBodyLimit` of the route, there with some air for the form framing).
pub const MAX_UPLOAD_BYTES: usize = 10 * 1024 * 1024;
/// At most this many entries a `.xmind` archive may have.
pub const MAX_ZIP_ENTRIES: usize = 50;
/// At most this many bytes are unpacked from an archive (counted while reading, the header value is not trusted).
pub const MAX_UNPACKED_BYTES: u64 = 20 * 1024 * 1024;
/// Longest note of an imported node.
pub const MAX_NOTE_CHARS: usize = 4000;
/// At most this much text (titles, notes, tags together) an imported tree carries.
pub const MAX_TEXT_BYTES: usize = 4 * 1024 * 1024;
/// At most this many relations are taken over.
pub const MAX_IMPORT_LINKS: usize = 2000;
/// Title of a node that carries none in the file.
pub const UNTITLED: &str = "(ohne Titel)";

/// A node as a reader takes it from the file (still unchecked).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RawNode {
    /// Id in the file (target of the relations); empty if the file names none.
    pub id: String,
    pub title: String,
    pub note: String,
    pub words: Vec<String>,
    /// The branch „Wolke“ of an own export.
    pub cloud: bool,
    pub children: Vec<RawNode>,
}

/// A relation of the file (ids as in [`RawNode::id`]).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RawLink {
    pub from: String,
    pub to: String,
    pub note: String,
}

/// Result of a reader.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Raw {
    pub root: RawNode,
    pub links: Vec<RawLink>,
    pub warnings: Vec<String>,
}

/// Message “too many nodes” (same for reader and [`check`]).
pub fn too_many_nodes() -> String {
    crate::i18n::tc("ideas_msg.import.file_too_many", &[("max", MAX_IMPORT_NODES.into())])
}

/// Message “too deep” (same for reader and [`check`]).
pub fn too_deep() -> String {
    crate::i18n::tc("ideas_msg.import.file_too_deep", &[("max", MAX_TREE_DEPTH.into())])
}

/// Read a file by its content: ZIP („PK“) as XMind, XML as FreeMind/Freeplane. The file name decides nothing.
pub fn read(bytes: &[u8]) -> Result<ImportTree, String> {
    if bytes.len() > MAX_UPLOAD_BYTES {
        return Err(too_large());
    }
    if bytes.is_empty() {
        return Err(crate::i18n::tc("ideas_msg.import.empty", &[]));
    }
    let raw = if bytes.starts_with(b"PK") {
        super::xmind::read(bytes)?
    } else {
        let text = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
        let first = text.iter().find(|b| !b.is_ascii_whitespace());
        if first != Some(&b'<') {
            return Err(crate::i18n::tc("ideas_msg.import.wrong_format", &[]));
        }
        super::freemind::read(text)?
    };
    finish(raw)
}

/// Message “file too large”.
pub fn too_large() -> String {
    crate::i18n::tc("ideas_msg.import.too_large", &[("mib", (MAX_UPLOAD_BYTES / (1024 * 1024)).into())])
}

fn convert(node: RawNode, path: &mut Vec<usize>, ids: &mut HashMap<String, Vec<usize>>) -> ImportNode {
    if !node.id.is_empty() {
        ids.entry(node.id).or_insert_with(|| path.clone());
    }
    let mut children = Vec::with_capacity(node.children.len());
    for (index, child) in node.children.into_iter().enumerate() {
        path.push(index);
        children.push(convert(child, path, ids));
        path.pop();
    }
    ImportNode { title: node.title, note: node.note, words: node.words, children, cloud: false }
}

/// From the raw tree to the checked [`ImportTree`]: root becomes title, the branch „Wolke“ is dissolved, relations
/// become paths; then [`check`]. The readers already limit node count and depth while reading.
pub fn finish(raw: Raw) -> Result<ImportTree, String> {
    // Round 36: the children of the own branch „Wolke“ stand on the top level again and remember that
    // they come from the cloud (`ImportNode::cloud`) – when created they go back there.
    let mut top = Vec::new();
    for child in raw.root.children {
        if child.cloud {
            top.extend(child.children.into_iter().map(|node| (node, true)));
        } else {
            top.push((child, false));
        }
    }
    let mut ids = HashMap::new();
    let mut nodes = Vec::with_capacity(top.len());
    for (index, (node, cloud)) in top.into_iter().enumerate() {
        nodes.push(ImportNode { cloud, ..convert(node, &mut vec![index], &mut ids) });
    }
    let mut warnings = raw.warnings;
    let mut lost = 0usize;
    let mut links = Vec::new();
    for link in raw.links {
        match (ids.get(&link.from), ids.get(&link.to)) {
            (Some(from), Some(to)) if from != to => {
                links.push(ImportLink { from: from.clone(), to: to.clone(), note: link.note })
            }
            _ => lost += 1,
        }
    }
    if lost > 0 {
        warnings.push(crate::i18n::tc("ideas_msg.import.links_lost", &[("count", lost.into())]));
    }
    check(ImportTree { title: raw.root.title, nodes, links, warnings })
}

/// One line: whitespace collapsed, control characters removed.
fn one_line(text: &str) -> String {
    let clean: String = text.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    clean.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Shortened to `max` characters (the last is then „…“); `true` if it was shortened.
fn cut(text: String, max: usize) -> (String, bool) {
    if text.chars().count() <= max {
        return (text, false);
    }
    let mut short: String = text.chars().take(max.saturating_sub(1)).collect();
    short = short.trim_end().to_owned();
    short.push('…');
    (short, true)
}

/// Multi-line text: line ends unified, control characters (except line break and tab) removed.
fn many_lines(text: &str) -> String {
    let unified = text.replace("\r\n", "\n").replace('\r', "\n");
    let clean: String = unified.chars().filter(|c| !c.is_control() || matches!(c, '\n' | '\t')).collect();
    clean.trim().to_owned()
}

#[derive(Default)]
struct Stats {
    nodes: usize,
    bytes: usize,
    cut_titles: usize,
    cut_notes: usize,
    untitled: usize,
    cut_words: usize,
}

fn clean(node: &mut ImportNode, depth: usize, stats: &mut Stats) -> Result<(), String> {
    if depth > MAX_TREE_DEPTH {
        return Err(too_deep());
    }
    stats.nodes += 1;
    if stats.nodes > MAX_IMPORT_NODES {
        return Err(too_many_nodes());
    }
    let (title, was_cut) = cut(one_line(&node.title), MAX_TITLE_CHARS);
    stats.cut_titles += usize::from(was_cut);
    node.title = title;
    if node.title.is_empty() {
        node.title = UNTITLED.to_owned();
        stats.untitled += 1;
    }
    let (note, was_cut) = cut(many_lines(&node.note), MAX_NOTE_CHARS);
    stats.cut_notes += usize::from(was_cut);
    node.note = note;
    let mut words: Vec<String> = Vec::new();
    let mut changed = false;
    for word in &node.words {
        let (word, was_cut) = cut(one_line(&word.replace([',', ';'], " ")), MAX_WORD_CHARS);
        changed |= was_cut;
        if !word.is_empty() && !words.iter().any(|w| w.to_lowercase() == word.to_lowercase()) {
            words.push(word);
        }
    }
    if words.len() > MAX_IDEA_WORDS {
        words.truncate(MAX_IDEA_WORDS);
        changed = true;
    }
    // Empty and duplicate tags are dropped silently; a note only if something was shortened or cut off.
    stats.cut_words += usize::from(changed);
    node.words = words;
    stats.bytes += node.title.len() + node.note.len() + node.words.iter().map(String::len).sum::<usize>();
    if stats.bytes > MAX_TEXT_BYTES {
        return Err(crate::i18n::tc(
            "ideas_msg.import.too_much_text",
            &[("mib", (MAX_TEXT_BYTES / (1024 * 1024)).into())],
        ));
    }
    for child in &mut node.children {
        clean(child, depth + 1, stats)?;
    }
    Ok(())
}

/// Node at the path (indices from `nodes` downwards).
pub fn at<'a>(nodes: &'a [ImportNode], path: &[usize]) -> Option<&'a ImportNode> {
    let (first, rest) = path.split_first()?;
    let mut node = nodes.get(*first)?;
    for index in rest {
        node = node.children.get(*index)?;
    }
    Some(node)
}

/// Number of nodes of a tree.
pub fn count(nodes: &[ImportNode]) -> usize {
    nodes.iter().map(|n| 1 + count(&n.children)).sum()
}

/// Enforce limits and clean texts (see module head). Applies to the freshly read tree and – a second
/// time – to the tree from the hidden form field of the preview. An already checked tree stays unchanged.
pub fn check(mut tree: ImportTree) -> Result<ImportTree, String> {
    let mut stats = Stats::default();
    tree.title = cut(one_line(&tree.title), MAX_TITLE_CHARS).0;
    for node in &mut tree.nodes {
        clean(node, 1, &mut stats)?;
    }
    if tree.nodes.is_empty() {
        return Err(crate::i18n::tc("ideas_msg.import.only_root", &[]));
    }
    let mut warnings: Vec<String> =
        tree.warnings.iter().map(|w| cut(one_line(w), 300).0).filter(|w| !w.is_empty()).take(40).collect();
    let mut note = |count: usize, text: String| {
        if count > 0 {
            warnings.push(text);
        }
    };
    note(
        stats.cut_titles,
        crate::i18n::tc(
            "ideas_msg.import.cut_titles",
            &[("count", stats.cut_titles.into()), ("max", MAX_TITLE_CHARS.into())],
        ),
    );
    note(
        stats.cut_notes,
        crate::i18n::tc(
            "ideas_msg.import.cut_notes",
            &[("count", stats.cut_notes.into()), ("max", MAX_NOTE_CHARS.into())],
        ),
    );
    note(
        stats.untitled,
        crate::i18n::tc(
            "ideas_msg.import.untitled",
            &[("count", stats.untitled.into()), ("untitled", UNTITLED.into())],
        ),
    );
    note(
        stats.cut_words,
        crate::i18n::tc(
            "ideas_msg.import.cut_words",
            &[("count", stats.cut_words.into()), ("max", MAX_IDEA_WORDS.into()), ("chars", MAX_WORD_CHARS.into())],
        ),
    );
    let before = tree.links.len();
    let nodes = &tree.nodes;
    tree.links.retain(|l| l.from != l.to && at(nodes, &l.from).is_some() && at(nodes, &l.to).is_some());
    tree.links.truncate(MAX_IMPORT_LINKS);
    if tree.links.len() < before {
        warnings
            .push(crate::i18n::tc("ideas_msg.import.links_dropped", &[("count", (before - tree.links.len()).into())]));
    }
    for link in &mut tree.links {
        link.note = cut(one_line(&link.note), MAX_LINK_NOTE_CHARS).0;
    }
    tree.warnings = warnings;
    Ok(tree)
}

/// HTML to plain text: tags are dropped (paragraphs and line breaks become lines), the common entities become
/// characters, everything else stays as text. Scripts and styles are removed with their content.
pub fn html_to_text(html: &str) -> String {
    let mut out = String::new();
    let mut rest = html;
    let mut skip_until: Option<&'static str> = None;
    while let Some(start) = rest.find('<') {
        if skip_until.is_none() {
            out.push_str(&rest[..start]);
        }
        let after = &rest[start + 1..];
        let Some(end) = after.find('>') else {
            rest = "";
            break;
        };
        let tag = after[..end].trim().to_lowercase();
        let name: String = tag.trim_start_matches('/').chars().take_while(|c| c.is_ascii_alphanumeric()).collect();
        match skip_until {
            Some(wanted) if tag.starts_with('/') && name == wanted => skip_until = None,
            Some(_) => {}
            None if matches!(name.as_str(), "script" | "style") && !tag.starts_with('/') && !tag.ends_with('/') => {
                skip_until = Some(if name == "script" { "script" } else { "style" });
            }
            None => {
                let block =
                    matches!(name.as_str(), "p" | "div" | "li" | "tr" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6");
                if name == "br" || (block && tag.starts_with('/')) {
                    out.push('\n');
                }
            }
        }
        rest = &after[end + 1..];
    }
    if skip_until.is_none() {
        out.push_str(rest);
    }
    let mut text = String::with_capacity(out.len());
    let mut rest = out.as_str();
    while let Some(start) = rest.find('&') {
        text.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after.find(';').filter(|end| *end <= 10).and_then(|end| entity(&after[..end]).map(|c| (c, end))) {
            Some((c, end)) => {
                text.push(c);
                rest = &after[end + 1..];
            }
            None => {
                text.push('&');
                rest = after;
            }
        }
    }
    text.push_str(rest);
    let lines: Vec<String> = text.lines().map(|l| l.split_whitespace().collect::<Vec<_>>().join(" ")).collect();
    let mut joined = lines.join("\n");
    while joined.contains("\n\n\n") {
        joined = joined.replace("\n\n\n", "\n\n");
    }
    joined.trim().to_owned()
}

/// Character for an entity (without `&` and `;`): the five of XML, `nbsp` and character references. Else `None`.
pub fn entity(name: &str) -> Option<char> {
    Some(match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" => ' ',
        _ => {
            let number = name.strip_prefix('#')?;
            let code = match number.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => number.parse().ok()?,
            };
            char::from_u32(code).filter(|c| *c != '\0')?
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn leaf(title: &str) -> ImportNode {
        ImportNode { title: title.to_owned(), ..ImportNode::default() }
    }

    #[test]
    fn html_becomes_plain_text() {
        assert_eq!(
            html_to_text(
                "<p>Erste <b>Zeile</b></p><p>Zweite &amp; dritte<br/>Zeile&nbsp;4</p><script>alert(1)</script>"
            ),
            "Erste Zeile\nZweite & dritte\nZeile 4",
            "Tags fallen weg, Absätze werden Zeilen, Entitäten werden Zeichen, Skripte verschwinden samt Inhalt"
        );
        assert_eq!(
            html_to_text("3 < 4 &unbekannt; &#x41;"),
            "3",
            "ein offenes < schneidet ab, statt Auszeichnung durchzulassen"
        );
        assert_eq!(html_to_text("&unbekannt; &#x41;"), "&unbekannt; A", "unbekannte Entitäten bleiben Text");
    }

    #[test]
    fn check_cuts_long_texts_and_says_so() {
        crate::i18n::sync_scope(crate::i18n::Lang::De, || {
            let mut long = leaf(&"t".repeat(MAX_TITLE_CHARS + 50));
            long.note = "n".repeat(MAX_NOTE_CHARS + 1);
            long.words = vec!["Deich".to_owned(), "deich".to_owned(), " ".to_owned(), "w".repeat(MAX_WORD_CHARS + 5)];
            let tree = check(ImportTree {
                title: "Karte".to_owned(),
                nodes: vec![long, leaf(" \u{7}")],
                ..ImportTree::default()
            })
            .expect("überlange Texte sind kein Grund zur Ablehnung");
            assert_eq!(tree.nodes[0].title.chars().count(), MAX_TITLE_CHARS, "Titel auf die Höchstlänge gekürzt");
            assert_eq!(tree.nodes[0].note.chars().count(), MAX_NOTE_CHARS, "Notiz auf die Höchstlänge gekürzt");
            assert_eq!(tree.nodes[0].words.len(), 2, "doppelte und leere Schlagwörter fallen weg");
            assert_eq!(tree.nodes[0].words[1].chars().count(), MAX_WORD_CHARS, "zu langes Schlagwort gekürzt");
            assert_eq!(
                tree.nodes[1].title, UNTITLED,
                "ein Knoten ohne Titel bekommt einen (ein Einfall braucht einen)"
            );
            let all = tree.warnings.join(" | ");
            assert!(all.contains("1 Titel war länger"), "a count of 1 reads singular (gekürzte Titel): {all}");
            assert!(all.contains("1 Notiz war länger"), "a count of 1 reads singular (gekürzte Notizen): {all}");
            assert!(all.contains("ohne Titel"), "Hinweis auf Knoten ohne Titel: {all}");
            assert!(all.contains("Schlagwörter gekürzt"), "Hinweis auf gekürzte Schlagwörter: {all}");
            let again = check(tree.clone()).unwrap();
            assert_eq!(
                again, tree,
                "ein geprüfter Baum bleibt beim zweiten Prüfen (Bestätigung der Vorschau) unverändert"
            );
        })
    }

    #[test]
    fn check_refuses_too_many_nodes_and_too_deep_trees() {
        let wide = ImportTree {
            nodes: (0..=MAX_IMPORT_NODES).map(|i| leaf(&format!("k{i}"))).collect(),
            ..ImportTree::default()
        };
        assert_eq!(check(wide).unwrap_err(), too_many_nodes(), "ein Knoten über der Grenze → abgelehnt");
        let mut deep = leaf("unten");
        for level in 0..MAX_TREE_DEPTH {
            deep = ImportNode { title: format!("e{level}"), children: vec![deep], ..ImportNode::default() };
        }
        assert_eq!(
            check(ImportTree { nodes: vec![deep.clone()], ..ImportTree::default() }).unwrap_err(),
            too_deep(),
            "eine Ebene über der Grenze → abgelehnt"
        );
        let ok = deep.children.remove(0);
        assert!(check(ImportTree { nodes: vec![ok], ..ImportTree::default() }).is_ok(), "genau die Grenze ist erlaubt");
        assert!(check(ImportTree::default()).is_err(), "ein Baum ohne Knoten ist nichts zum Einlesen");
    }

    #[test]
    fn finish_resolves_the_cloud_branch_and_turns_ids_into_paths() {
        crate::i18n::sync_scope(crate::i18n::Lang::De, || {
            let raw = Raw {
                root: RawNode {
                    id: "r".to_owned(),
                    title: "Karte".to_owned(),
                    children: vec![
                        RawNode { id: "a".to_owned(), title: "A".to_owned(), ..RawNode::default() },
                        RawNode {
                            id: "c".to_owned(),
                            title: "Wolke".to_owned(),
                            cloud: true,
                            children: vec![RawNode { id: "b".to_owned(), title: "B".to_owned(), ..RawNode::default() }],
                            ..RawNode::default()
                        },
                    ],
                    ..RawNode::default()
                },
                links: vec![
                    RawLink { from: "a".to_owned(), to: "b".to_owned(), note: "passt".to_owned() },
                    RawLink { from: "a".to_owned(), to: "r".to_owned(), note: String::new() },
                    RawLink { from: "a".to_owned(), to: "fehlt".to_owned(), note: String::new() },
                ],
                warnings: Vec::new(),
            };
            let tree = finish(raw).unwrap();
            assert_eq!(tree.title, "Karte", "die Wurzel wird Titel, nicht Knoten");
            assert_eq!(
                tree.nodes.iter().map(|n| n.title.as_str()).collect::<Vec<_>>(),
                vec!["A", "B"],
                "der Ast „Wolke“ ist aufgelöst: sein Kind steht auf der obersten Ebene"
            );
            assert_eq!(
                tree.nodes.iter().map(|n| n.cloud).collect::<Vec<_>>(),
                vec![false, true],
                "Runde 36: Das Kind des Astes „Wolke“ merkt sich die Wolke, der gewöhnliche Ast nicht"
            );
            assert_eq!(
                tree.links,
                vec![ImportLink { from: vec![0], to: vec![1], note: "passt".to_owned() }],
                "Beziehungen als Pfade; Beziehungen zur Wurzel oder ins Leere fallen weg"
            );
            assert!(
                tree.warnings.iter().any(|w| w.starts_with("2 Beziehung(en) nicht übernommen")),
                "mit Hinweis: {:?}",
                tree.warnings
            );
        })
    }
}
