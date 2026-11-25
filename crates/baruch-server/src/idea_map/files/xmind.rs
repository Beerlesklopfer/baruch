// SPDX-License-Identifier: AGPL-3.0-or-later
//! XMind (`.xmind`) schreiben und lesen (Vertrag Runde 34 §8, Besitz mindmap_austausch).
//!
//! **Schreiben** ([`write`]): ein ZIP mit `content.json` (ein Blatt: `rootTopic` mit `children.attached` in
//! Reihenfolge, `notes.plain.content`, `labels` = Schlagwörter, `relationships` mit `end1Id`/`end2Id`/`title`),
//! `metadata.json` und `manifest.json`. Der Aufbau folgt der offenen Beschreibung des Formats aus dem Gedächtnis;
//! was nicht mit echtem XMind geprüft ist, steht als Annahme in `docs/r34/austausch.md`.
//!
//! **Lesen** ([`read`]): Das Archiv ist fremde Eingabe. Höchstens [`MAX_ZIP_ENTRIES`] Einträge; gelesen wird allein
//! der Eintrag mit dem Namen `content.json` (Vergleich der Namen als Bytes – kein Name wird je als Pfad benutzt,
//! nichts wird auf Platte entpackt); beim Entpacken wird mitgezählt und bei [`MAX_UNPACKED_BYTES`] abgebrochen (der
//! Größenangabe im Kopf wird nicht vertraut). Archive mit Namen, die aus dem Archiv herausführten (`..`, absoluter
//! Pfad, Laufwerk, `\`), werden ganz abgelehnt. Das alte XMind-8-Format (nur `content.xml`) wird mit klarer
//! Meldung abgelehnt.

use std::io::{Cursor, Read, Write as _};

use serde_json::{Value as Json, json};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use super::export::{CLOUD_ID, OutNode, Sheet};
use super::import::{
    MAX_UNPACKED_BYTES, MAX_ZIP_ENTRIES, Raw, RawLink, RawNode, html_to_text, too_deep, too_many_nodes,
};
use crate::idea_map::tree::{MAX_IMPORT_NODES, MAX_TREE_DEPTH};

/// Name des einzigen Eintrags, den der Leser öffnet.
pub const CONTENT: &str = "content.json";

fn topic_of(node: &OutNode, root: bool) -> Json {
    let mut topic = json!({ "id": node.id, "class": "topic", "title": node.title });
    if root {
        topic["structureClass"] = json!("org.xmind.ui.map.unbalanced");
    }
    if !node.note.trim().is_empty() {
        topic["notes"] = json!({ "plain": { "content": node.note.trim() } });
    }
    if !node.words.is_empty() {
        topic["labels"] = json!(node.words);
    }
    if !node.children.is_empty() {
        let attached: Vec<Json> = node.children.iter().map(|child| topic_of(child, false)).collect();
        topic["children"] = json!({ "attached": attached });
    }
    topic
}

/// `content.json` des Blatts (ein Feld mit genau einem Blatt).
pub fn content(sheet: &Sheet) -> Json {
    let relationships: Vec<Json> = sheet
        .links
        .iter()
        .map(|l| json!({ "id": format!("v:{}", l.id), "end1Id": l.from, "end2Id": l.to, "title": l.label }))
        .collect();
    json!([{
        "id": "baruch-blatt",
        "class": "sheet",
        "title": sheet.title,
        "rootTopic": topic_of(&sheet.root, true),
        "relationships": relationships,
    }])
}

/// Das Blatt als `.xmind`-Archiv (siehe Modulkopf).
pub fn write(sheet: &Sheet) -> Result<Vec<u8>, String> {
    let failed = |e: &dyn std::fmt::Display| format!("XMind-Datei ließ sich nicht schreiben: {e}");
    let content = serde_json::to_vec(&content(sheet)).map_err(|e| failed(&e))?;
    let metadata = serde_json::to_vec(&json!({
        "creator": { "name": "Baruch", "version": env!("CARGO_PKG_VERSION") },
    }))
    .map_err(|e| failed(&e))?;
    let manifest = serde_json::to_vec(&json!({ "file-entries": { "content.json": {}, "metadata.json": {} } }))
        .map_err(|e| failed(&e))?;
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (name, bytes) in [(CONTENT, &content), ("metadata.json", &metadata), ("manifest.json", &manifest)] {
        writer.start_file(name, options).map_err(|e| failed(&e))?;
        writer.write_all(bytes).map_err(|e| failed(&e))?;
    }
    Ok(writer.finish().map_err(|e| failed(&e))?.into_inner())
}

// ---- Lesen ---------------------------------------------------------------------------------------

fn broken(what: impl std::fmt::Display) -> String {
    format!("Die Datei ist kein lesbares XMind-Archiv ({what}).")
}

/// Führte dieser Name als Pfad aus dem Archiv heraus? (Er wird nie als Pfad benutzt; solche Archive lehnt Baruch
/// trotzdem ab.)
fn unsafe_name(name: &[u8]) -> bool {
    let text = String::from_utf8_lossy(name);
    text.starts_with('/')
        || text.contains('\\')
        || text.contains('\0')
        || text.split('/').any(|part| part == "..")
        || (text.len() >= 2 && text.as_bytes()[1] == b':' && text.as_bytes()[0].is_ascii_alphabetic())
}

/// `content.json` aus dem Archiv holen (Grenzen im Modulkopf).
pub fn unpack(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut zip = ZipArchive::new(Cursor::new(bytes)).map_err(|e| broken(format!("Inhaltsverzeichnis: {e}")))?;
    if zip.len() > MAX_ZIP_ENTRIES {
        return Err(format!(
            "Das Archiv hat {} Einträge, erlaubt sind höchstens {MAX_ZIP_ENTRIES}; so etwas ist keine gewöhnliche XMind-Datei.",
            zip.len()
        ));
    }
    let mut found = None;
    let mut old_format = false;
    for index in 0..zip.len() {
        let entry = zip.by_index_raw(index).map_err(|e| broken(format!("Eintrag {index}: {e}")))?;
        let name = entry.name_raw();
        if unsafe_name(name) {
            return Err(format!(
                "Das Archiv enthält einen Eintrag, dessen Name aus dem Archiv herausführte („{}“); Baruch liest es nicht.",
                String::from_utf8_lossy(name).chars().filter(|c| !c.is_control()).take(80).collect::<String>()
            ));
        }
        if name == CONTENT.as_bytes() && found.is_none() {
            found = Some(index);
        }
        old_format |= name == b"content.xml";
    }
    let Some(index) = found else {
        return Err(if old_format {
            "Das ist eine Datei im alten XMind-8-Format (content.xml). Baruch liest nur das neuere Format (XMind 2020 und neuer, content.json): Bitte die Datei in einem aktuellen XMind öffnen und neu speichern oder als FreeMind (.mm) ausgeben.".to_owned()
        } else {
            broken("der Eintrag content.json fehlt")
        });
    };
    let entry = zip.by_index(index).map_err(|e| broken(format!("content.json: {e}")))?;
    let mut content = Vec::new();
    // Eins mehr lesen als erlaubt: so fällt auf, wenn der Eintrag größer ist, als sein Kopf behauptet.
    entry
        .take(MAX_UNPACKED_BYTES + 1)
        .read_to_end(&mut content)
        .map_err(|e| broken(format!("content.json lässt sich nicht entpacken: {e}")))?;
    if content.len() as u64 > MAX_UNPACKED_BYTES {
        return Err(format!(
            "Der Inhalt des Archivs ist entpackt größer als {} MiB; Baruch bricht hier ab.",
            MAX_UNPACKED_BYTES / (1024 * 1024)
        ));
    }
    Ok(content)
}

fn text_of(value: Option<&Json>) -> String {
    value.and_then(Json::as_str).unwrap_or_default().to_owned()
}

struct Walk {
    count: usize,
    auto: usize,
    detached: usize,
}

fn raw_topic(topic: &Json, depth: usize, walk: &mut Walk) -> Result<RawNode, String> {
    // Wurzel = Tiefe 0; sie zählt nicht mit (sie wird nie Einfall).
    if depth > MAX_TREE_DEPTH {
        return Err(too_deep());
    }
    walk.count += 1;
    if walk.count > MAX_IMPORT_NODES + 1 {
        return Err(too_many_nodes());
    }
    let id = match topic.get("id").and_then(Json::as_str).filter(|i| !i.is_empty()) {
        Some(id) => id.to_owned(),
        None => {
            walk.auto += 1;
            format!("\u{1}auto-{}", walk.auto)
        }
    };
    let notes = topic.get("notes");
    let plain = text_of(notes.and_then(|n| n.pointer("/plain/content")));
    let note = if plain.trim().is_empty() {
        html_to_text(&text_of(notes.and_then(|n| n.pointer("/realHTML/content"))))
    } else {
        plain
    };
    let words = topic
        .get("labels")
        .and_then(Json::as_array)
        .map(|labels| labels.iter().filter_map(Json::as_str).map(str::to_owned).collect())
        .unwrap_or_default();
    let mut children = Vec::new();
    for (kind, list) in topic.get("children").and_then(Json::as_object).into_iter().flatten() {
        // `attached` sind die gewöhnlichen Unterthemen, `detached` frei schwebende; anderes (Zusammenfassungen,
        // Sprechblasen) wird nicht übernommen.
        if !matches!(kind.as_str(), "attached" | "detached") {
            continue;
        }
        for child in list.as_array().into_iter().flatten() {
            if kind == "detached" {
                walk.detached += 1;
            }
            children.push((kind == "detached", raw_topic(child, depth + 1, walk)?));
        }
    }
    // Erst die angehängten in ihrer Reihenfolge, dann die frei schwebenden (die Schlüssel eines JSON-Objekts haben
    // keine verlässliche Reihenfolge).
    children.sort_by_key(|(detached, _)| *detached);
    Ok(RawNode {
        cloud: id == CLOUD_ID,
        id,
        title: text_of(topic.get("title")),
        note,
        words,
        children: children.into_iter().map(|(_, node)| node).collect(),
    })
}

/// `content.json` deuten: das erste Blatt, sein `rootTopic` und seine `relationships`.
pub fn parse(content: &[u8]) -> Result<Raw, String> {
    let value: Json = serde_json::from_slice(content).map_err(|e| {
        if e.to_string().contains("recursion limit") {
            too_deep()
        } else {
            broken(format!("content.json ist kein gültiges JSON: {e}"))
        }
    })?;
    let sheets = match &value {
        Json::Array(sheets) => sheets.as_slice(),
        Json::Object(_) => std::slice::from_ref(&value),
        _ => return Err(broken("content.json enthält kein Blatt")),
    };
    let sheet = sheets.first().ok_or_else(|| broken("content.json enthält kein Blatt"))?;
    let root = sheet.get("rootTopic").filter(|t| t.is_object()).ok_or_else(|| broken("dem Blatt fehlt rootTopic"))?;
    let mut walk = Walk { count: 0, auto: 0, detached: 0 };
    let mut root = raw_topic(root, 0, &mut walk)?;
    if root.title.trim().is_empty() {
        root.title = text_of(sheet.get("title"));
    }
    let mut warnings = Vec::new();
    if sheets.len() > 1 {
        warnings.push(format!("Die Datei hat {} Blätter; gelesen wurde nur das erste.", sheets.len()));
    }
    if walk.detached > 0 {
        warnings.push(format!(
            "{} frei schwebende(s) Thema(-en) wurden als gewöhnliche Unterknoten übernommen.",
            walk.detached
        ));
    }
    let links = sheet
        .get("relationships")
        .and_then(Json::as_array)
        .into_iter()
        .flatten()
        .take(super::import::MAX_IMPORT_LINKS * 2)
        .map(|r| RawLink {
            from: text_of(r.get("end1Id")),
            to: text_of(r.get("end2Id")),
            note: text_of(r.get("title")),
        })
        .collect();
    Ok(Raw { root, links, warnings })
}

/// XMind-Datei lesen (siehe Modulkopf).
pub fn read(bytes: &[u8]) -> Result<Raw, String> {
    parse(&unpack(bytes)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_that_would_leave_the_archive_are_recognised() {
        for name in ["../boese.txt", "a/../../b", "/etc/passwd", "C:\\x", "c:/x", "a\\b"] {
            assert!(unsafe_name(name.as_bytes()), "{name} führte als Pfad aus dem Archiv heraus");
        }
        for name in ["content.json", "resources/bild.png", "Thumbnails/thumbnail.png", "a..b/c"] {
            assert!(!unsafe_name(name.as_bytes()), "{name} ist ein gewöhnlicher Name in einer XMind-Datei");
        }
    }

    #[test]
    fn foreign_content_is_read_tolerantly() {
        let content = json!([
            { "title": "Blatt 1", "rootTopic": { "id": "r", "title": "", "children": {
                "detached": [{ "id": "frei", "title": "Schwebt" }],
                "attached": [
                    { "id": "a", "title": "Erstes", "labels": ["Tau", 7], "notes": { "realHTML": { "content": "<div>Fett <b>und</b> so</div>" } } },
                    { "title": "Zweites", "children": { "summary": [{ "id": "s", "title": "Summe" }] } }
                ] } },
              "relationships": [{ "end1Id": "a", "end2Id": "frei", "title": "zieht" }] },
            { "title": "Blatt 2", "rootTopic": { "title": "anderes" } }
        ]);
        let raw = parse(content.to_string().as_bytes()).expect("gültiges JSON mit Lücken wird gelesen");
        assert_eq!(raw.root.title, "Blatt 1", "ein leerer Titel der Wurzel wird der Titel des Blatts");
        let titles: Vec<&str> = raw.root.children.iter().map(|c| c.title.as_str()).collect();
        assert_eq!(
            titles,
            vec!["Erstes", "Zweites", "Schwebt"],
            "erst die angehängten in Reihenfolge, dann die freien"
        );
        assert_eq!(raw.root.children[0].words, vec!["Tau"], "labels als Schlagwörter; was kein Text ist, fällt weg");
        assert_eq!(raw.root.children[0].note, "Fett und so", "nur HTML-Notiz vorhanden → reiner Text");
        assert!(raw.root.children[1].children.is_empty(), "Zusammenfassungen werden nicht übernommen");
        assert_eq!(raw.links.len(), 1, "relationships gelesen");
        assert_eq!(raw.warnings.len(), 2, "Hinweise auf weitere Blätter und freie Themen: {:?}", raw.warnings);
    }

    #[test]
    fn broken_json_and_missing_parts_are_refused() {
        assert!(parse(b"{ kaputt").unwrap_err().contains("kein gültiges JSON"), "kaputtes JSON");
        assert!(parse(b"[]").unwrap_err().contains("kein Blatt"), "leeres Feld");
        assert!(parse(b"[{\"title\":\"x\"}]").unwrap_err().contains("rootTopic"), "Blatt ohne Wurzel");
        assert!(parse(b"7").is_err(), "eine Zahl ist keine Mindmap");
    }
}
