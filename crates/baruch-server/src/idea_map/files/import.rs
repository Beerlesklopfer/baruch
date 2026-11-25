// SPDX-License-Identifier: AGPL-3.0-or-later
//! Einlesen fremder Mindmaps (Vertrag Runde 34 §8, Besitz mindmap_austausch): Grenzen, Reinigung und der Weg von
//! der gelesenen Datei zum [`ImportTree`].
//!
//! Eine fremde Datei ist fremde Eingabe. Die Leser ([`super::freemind::read`], [`super::xmind::read`]) liefern einen
//! rohen Baum ([`Raw`]); [`finish`] macht daraus den [`ImportTree`] und [`check`] setzt die Grenzen durch:
//! höchstens [`MAX_IMPORT_NODES`] Knoten, Tiefe höchstens [`MAX_TREE_DEPTH`], Titel höchstens [`MAX_TITLE_CHARS`]
//! und Notizen höchstens [`MAX_NOTE_CHARS`] Zeichen (gekürzt, mit Hinweis in `warnings`), Steuerzeichen entfernt.
//! [`check`] läuft ein zweites Mal, wenn die Vorschau bestätigt wird: Der Baum kommt dann aus einem verborgenen
//! Formularfeld zurück und ist wieder fremde Eingabe.
//!
//! Die Wurzel der fremden Datei wird nie selbst Einfall: Ihr Titel steht in `ImportTree::title`, ihre Kinder sind
//! `ImportTree::nodes`. Der Ast „Wolke“ aus einer eigenen Ausgabe ([`super::export::CLOUD_ID`]) wird aufgelöst;
//! seine Kinder stehen wieder auf der obersten Ebene.

use std::collections::HashMap;

use baruch_store::{MAX_IDEA_WORDS, MAX_LINK_NOTE_CHARS, MAX_WORD_CHARS};

use crate::idea_map::tree::{ImportLink, ImportNode, ImportTree, MAX_IMPORT_NODES, MAX_TITLE_CHARS, MAX_TREE_DEPTH};

/// Größte hochgeladene Datei (auch `DefaultBodyLimit` der Route, dort mit etwas Luft für den Formularrahmen).
pub const MAX_UPLOAD_BYTES: usize = 10 * 1024 * 1024;
/// Höchstens so viele Einträge darf ein `.xmind`-Archiv haben.
pub const MAX_ZIP_ENTRIES: usize = 50;
/// Höchstens so viele Bytes werden aus einem Archiv entpackt (gezählt beim Lesen, nicht der Kopfangabe vertraut).
pub const MAX_UNPACKED_BYTES: u64 = 20 * 1024 * 1024;
/// Längste Notiz eines eingelesenen Knotens.
pub const MAX_NOTE_CHARS: usize = 4000;
/// Höchstens so viel Text (Titel, Notizen, Schlagwörter zusammen) trägt ein eingelesener Baum.
pub const MAX_TEXT_BYTES: usize = 4 * 1024 * 1024;
/// Höchstens so viele Beziehungen werden übernommen.
pub const MAX_IMPORT_LINKS: usize = 2000;
/// Titel eines Knotens, der in der Datei keinen trägt.
pub const UNTITLED: &str = "(ohne Titel)";

/// Ein Knoten, wie ihn ein Leser aus der Datei holt (noch ungeprüft).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RawNode {
    /// Kennung in der Datei (Ziel der Beziehungen); leer, wenn die Datei keine nennt.
    pub id: String,
    pub title: String,
    pub note: String,
    pub words: Vec<String>,
    /// Der Ast „Wolke“ einer eigenen Ausgabe.
    pub cloud: bool,
    pub children: Vec<RawNode>,
}

/// Eine Beziehung der Datei (Kennungen wie in [`RawNode::id`]).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RawLink {
    pub from: String,
    pub to: String,
    pub note: String,
}

/// Ergebnis eines Lesers.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Raw {
    pub root: RawNode,
    pub links: Vec<RawLink>,
    pub warnings: Vec<String>,
}

/// Meldung „zu viele Knoten“ (Leser und [`check`] gleich).
pub fn too_many_nodes() -> String {
    format!("Die Datei hat mehr als {MAX_IMPORT_NODES} Knoten; so große Mindmaps liest Baruch nicht ein.")
}

/// Meldung „zu tief“ (Leser und [`check`] gleich).
pub fn too_deep() -> String {
    format!("Die Mindmap ist tiefer als {MAX_TREE_DEPTH} Ebenen; so tiefe Mindmaps liest Baruch nicht ein.")
}

/// Datei nach ihrem Inhalt lesen: ZIP („PK“) als XMind, XML als FreeMind/Freeplane. Der Dateiname entscheidet nichts.
pub fn read(bytes: &[u8]) -> Result<ImportTree, String> {
    if bytes.len() > MAX_UPLOAD_BYTES {
        return Err(too_large());
    }
    if bytes.is_empty() {
        return Err("Die Datei ist leer.".to_owned());
    }
    let raw = if bytes.starts_with(b"PK") {
        super::xmind::read(bytes)?
    } else {
        let text = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
        let first = text.iter().find(|b| !b.is_ascii_whitespace());
        if first != Some(&b'<') {
            return Err(
                "Die Datei ist weder eine FreeMind-/Freeplane-Datei (.mm) noch eine XMind-Datei (.xmind).".to_owned()
            );
        }
        super::freemind::read(text)?
    };
    finish(raw)
}

/// Meldung „Datei zu groß“.
pub fn too_large() -> String {
    format!(
        "Die Datei ist größer als {} MiB; so große Mindmaps liest Baruch nicht ein.",
        MAX_UPLOAD_BYTES / (1024 * 1024)
    )
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
    ImportNode { title: node.title, note: node.note, words: node.words, children }
}

/// Vom rohen Baum zum geprüften [`ImportTree`]: Wurzel wird Titel, der Ast „Wolke“ wird aufgelöst, Beziehungen
/// werden Pfade; danach [`check`]. Die Leser begrenzen Knotenzahl und Tiefe schon beim Lesen.
pub fn finish(raw: Raw) -> Result<ImportTree, String> {
    let mut top = Vec::new();
    for child in raw.root.children {
        if child.cloud {
            top.extend(child.children);
        } else {
            top.push(child);
        }
    }
    let mut ids = HashMap::new();
    let mut nodes = Vec::with_capacity(top.len());
    for (index, node) in top.into_iter().enumerate() {
        nodes.push(convert(node, &mut vec![index], &mut ids));
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
        warnings.push(format!(
            "{lost} Beziehung(en) nicht übernommen: Ein Ende fehlt in der Datei, ist das Hauptthema oder beide Enden sind derselbe Knoten."
        ));
    }
    check(ImportTree { title: raw.root.title, nodes, links, warnings })
}

/// Eine Zeile: Leerraum zusammengefasst, Steuerzeichen entfernt.
fn one_line(text: &str) -> String {
    let clean: String = text.chars().map(|c| if c.is_control() { ' ' } else { c }).collect();
    clean.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Auf `max` Zeichen gekürzt (das letzte ist dann „…“); `true`, wenn gekürzt wurde.
fn cut(text: String, max: usize) -> (String, bool) {
    if text.chars().count() <= max {
        return (text, false);
    }
    let mut short: String = text.chars().take(max.saturating_sub(1)).collect();
    short = short.trim_end().to_owned();
    short.push('…');
    (short, true)
}

/// Mehrzeiliger Text: Zeilenenden vereinheitlicht, Steuerzeichen (außer Zeilenumbruch und Tabulator) entfernt.
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
    // Leere und doppelte Schlagwörter fallen still weg; ein Hinweis nur, wenn etwas gekürzt oder abgeschnitten wurde.
    stats.cut_words += usize::from(changed);
    node.words = words;
    stats.bytes += node.title.len() + node.note.len() + node.words.iter().map(String::len).sum::<usize>();
    if stats.bytes > MAX_TEXT_BYTES {
        return Err(format!(
            "Die Mindmap trägt mehr als {} MiB Text; so viel liest Baruch nicht ein.",
            MAX_TEXT_BYTES / (1024 * 1024)
        ));
    }
    for child in &mut node.children {
        clean(child, depth + 1, stats)?;
    }
    Ok(())
}

/// Knoten am Pfad (Indizes von `nodes` abwärts).
pub fn at<'a>(nodes: &'a [ImportNode], path: &[usize]) -> Option<&'a ImportNode> {
    let (first, rest) = path.split_first()?;
    let mut node = nodes.get(*first)?;
    for index in rest {
        node = node.children.get(*index)?;
    }
    Some(node)
}

/// Zahl der Knoten eines Baums.
pub fn count(nodes: &[ImportNode]) -> usize {
    nodes.iter().map(|n| 1 + count(&n.children)).sum()
}

/// Grenzen durchsetzen und Texte reinigen (siehe Modulkopf). Gilt für den frisch gelesenen Baum und – ein zweites
/// Mal – für den Baum aus dem verborgenen Formularfeld der Vorschau. Ein schon geprüfter Baum bleibt unverändert.
pub fn check(mut tree: ImportTree) -> Result<ImportTree, String> {
    let mut stats = Stats::default();
    tree.title = cut(one_line(&tree.title), MAX_TITLE_CHARS).0;
    for node in &mut tree.nodes {
        clean(node, 1, &mut stats)?;
    }
    if tree.nodes.is_empty() {
        return Err("Die Datei enthält außer dem Hauptthema keine Knoten; es gibt nichts einzulesen.".to_owned());
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
        format!("{} Titel waren länger als {MAX_TITLE_CHARS} Zeichen und wurden gekürzt.", stats.cut_titles),
    );
    note(
        stats.cut_notes,
        format!("{} Notiz(en) waren länger als {MAX_NOTE_CHARS} Zeichen und wurden gekürzt.", stats.cut_notes),
    );
    note(stats.untitled, format!("{} Knoten ohne Titel heißen jetzt „{UNTITLED}“.", stats.untitled));
    note(
        stats.cut_words,
        format!(
            "Bei {} Knoten wurden Schlagwörter gekürzt (höchstens {MAX_IDEA_WORDS} je Knoten, je {MAX_WORD_CHARS} Zeichen).",
            stats.cut_words
        ),
    );
    let before = tree.links.len();
    let nodes = &tree.nodes;
    tree.links.retain(|l| l.from != l.to && at(nodes, &l.from).is_some() && at(nodes, &l.to).is_some());
    tree.links.truncate(MAX_IMPORT_LINKS);
    if tree.links.len() < before {
        warnings
            .push(format!("{} Beziehung(en) nicht übernommen (ungültig oder zu viele).", before - tree.links.len()));
    }
    for link in &mut tree.links {
        link.note = cut(one_line(&link.note), MAX_LINK_NOTE_CHARS).0;
    }
    tree.warnings = warnings;
    Ok(tree)
}

/// HTML zu reinem Text: Tags fallen weg (Absätze und Zeilenumbrüche werden Zeilen), die gängigen Entitäten werden
/// Zeichen, alles Übrige bleibt als Text stehen. Skripte und Stile werden samt Inhalt entfernt.
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

/// Zeichen zu einer Entität (ohne `&` und `;`): die fünf von XML, `nbsp` und Zeichenreferenzen. Sonst `None`.
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
        let mut long = leaf(&"t".repeat(MAX_TITLE_CHARS + 50));
        long.note = "n".repeat(MAX_NOTE_CHARS + 1);
        long.words = vec!["Deich".to_owned(), "deich".to_owned(), " ".to_owned(), "w".repeat(MAX_WORD_CHARS + 5)];
        let tree =
            check(ImportTree { title: "Karte".to_owned(), nodes: vec![long, leaf(" \u{7}")], ..ImportTree::default() })
                .expect("überlange Texte sind kein Grund zur Ablehnung");
        assert_eq!(tree.nodes[0].title.chars().count(), MAX_TITLE_CHARS, "Titel auf die Höchstlänge gekürzt");
        assert_eq!(tree.nodes[0].note.chars().count(), MAX_NOTE_CHARS, "Notiz auf die Höchstlänge gekürzt");
        assert_eq!(tree.nodes[0].words.len(), 2, "doppelte und leere Schlagwörter fallen weg");
        assert_eq!(tree.nodes[0].words[1].chars().count(), MAX_WORD_CHARS, "zu langes Schlagwort gekürzt");
        assert_eq!(tree.nodes[1].title, UNTITLED, "ein Knoten ohne Titel bekommt einen (ein Einfall braucht einen)");
        let all = tree.warnings.join(" | ");
        assert!(all.contains("1 Titel waren länger"), "Hinweis auf gekürzte Titel: {all}");
        assert!(all.contains("1 Notiz(en) waren länger"), "Hinweis auf gekürzte Notizen: {all}");
        assert!(all.contains("ohne Titel"), "Hinweis auf Knoten ohne Titel: {all}");
        assert!(all.contains("Schlagwörter gekürzt"), "Hinweis auf gekürzte Schlagwörter: {all}");
        let again = check(tree.clone()).unwrap();
        assert_eq!(again, tree, "ein geprüfter Baum bleibt beim zweiten Prüfen (Bestätigung der Vorschau) unverändert");
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
            tree.links,
            vec![ImportLink { from: vec![0], to: vec![1], note: "passt".to_owned() }],
            "Beziehungen als Pfade; Beziehungen zur Wurzel oder ins Leere fallen weg"
        );
        assert!(
            tree.warnings.iter().any(|w| w.starts_with("2 Beziehung(en) nicht übernommen")),
            "mit Hinweis: {:?}",
            tree.warnings
        );
    }
}
