// SPDX-License-Identifier: AGPL-3.0-or-later
//! Container `.baruch` (ZIP) lesen und prüfen (Vertrag R26 F1/F7).
//!
//! Reihenfolge der Prüfungen, jede bricht mit einem Fehler ab (dann wird nichts geschrieben):
//! 1. Größe der Datei, Zahl der Einträge; das Zentralverzeichnis wird selbst durchgezählt, weil die
//!    ZIP-Bibliothek doppelte Namen still zusammenfasst (Doppel → [`ExchangeError::Archive`]).
//! 2. Namen: kein absoluter Pfad, kein `..`, kein Backslash, kein Laufwerk, nur `mimetype`, `werk.xml`,
//!    `MANIFEST.xml` und `bilder/<uuid>.<ext>`; keine Verknüpfung, kein Ordner, keine Verschlüsselung, nur
//!    STORED oder DEFLATE.
//! 3. Erster Eintrag `mimetype` (am Dateianfang, STORED, genau [`ZIP_MIMETYPE`]).
//! 4. Entpacken mit `take(Grenze + 1)` (den Größen im Kopf wird nicht getraut), Grenzen je Eintrag, für
//!    `werk.xml` und in Summe.
//! 5. Manifest: jeder Eintrag außer `mimetype` und dem Manifest steht darin und umgekehrt; SHA-256 und
//!    Bytezahl stimmen ([`ExchangeError::Checksum`]).
//! 6. `werk.xml` lesen; jede Bildfassung mit `datei` bekommt die Bytes ihres Eintrags (Name passend zur UUID
//!    und zum Typ, Bytezahl und SHA-256 wie im XML).

use std::collections::HashMap;
use std::io::{Cursor as IoCursor, Read};

use zip::CompressionMethod;
use zip::ZipArchive;

use super::{dom, map};
use crate::model::*;
use crate::{ExchangeError, Limits};

fn archive(message: impl Into<String>) -> ExchangeError {
    ExchangeError::Archive(message.into())
}

/// Namen aller Einträge des Zentralverzeichnisses in dessen Reihenfolge, selbst gezählt (mit Doppeln).
fn central_names(bytes: &[u8], start: u64) -> Result<Vec<Vec<u8>>, ExchangeError> {
    let mut names = Vec::new();
    let mut at = usize::try_from(start).map_err(|_| archive("Zentralverzeichnis außerhalb der Datei"))?;
    while bytes.len() >= at + 46 && bytes[at..at + 4] == [0x50, 0x4b, 0x01, 0x02] {
        let u16_at = |offset: usize| usize::from(u16::from_le_bytes([bytes[at + offset], bytes[at + offset + 1]]));
        let (name_len, extra_len, comment_len) = (u16_at(28), u16_at(30), u16_at(32));
        let end = at + 46 + name_len;
        if end > bytes.len() {
            return Err(archive("Zentralverzeichnis abgeschnitten"));
        }
        names.push(bytes[at + 46..end].to_vec());
        at = end + extra_len + comment_len;
        if names.len() > 1_000_000 {
            break;
        }
    }
    Ok(names)
}

/// Erlaubter Name eines Eintrags? Gibt einen sprechenden Fehler für unsichere Namen.
fn check_name(name: &str) -> Result<(), ExchangeError> {
    if name.starts_with('/') {
        return Err(archive(format!("absoluter Pfad „{name}“ im Archiv")));
    }
    if name.contains('\\') {
        return Err(archive(format!("Backslash im Namen „{name}“")));
    }
    if name.split('/').any(|part| part == "..") {
        return Err(archive(format!("„..“ im Pfad „{name}“ (führte aus dem Archiv heraus)")));
    }
    if name.len() >= 2 && name.as_bytes()[1] == b':' {
        return Err(archive(format!("Laufwerksangabe im Namen „{name}“")));
    }
    if name.chars().any(char::is_control) {
        return Err(archive(format!("Steuerzeichen im Namen „{name:?}“")));
    }
    if name == ENTRY_MIMETYPE || name == ENTRY_WORK || name == ENTRY_MANIFEST {
        return Ok(());
    }
    if let Some(file) = name.strip_prefix(IMAGE_DIR)
        && let Some((uuid, ext)) = file.split_once('.')
        && is_uuid(uuid)
        && matches!(ext, "png" | "jpg" | "gif" | "webp" | "svg")
    {
        return Ok(());
    }
    Err(archive(format!(
        "unerlaubter Eintrag „{name}“ (erlaubt: mimetype, werk.xml, MANIFEST.xml, bilder/<uuid>.<png|jpg|gif|webp|svg>)"
    )))
}

/// Container lesen (siehe Moduldoc).
pub(crate) fn read(bytes: &[u8], limits: &Limits) -> Result<ExchangeDocument, ExchangeError> {
    if bytes.len() as u64 > limits.max_file_bytes {
        return Err(ExchangeError::Limit(format!("Datei größer als {} Bytes", limits.max_file_bytes)));
    }
    if !bytes.starts_with(b"PK\x03\x04") {
        return Err(ExchangeError::NotExchange("kein ZIP-Archiv (Anfang „PK\\x03\\x04“ fehlt)".into()));
    }
    let mut zip = ZipArchive::new(IoCursor::new(bytes)).map_err(|e| archive(format!("kein lesbares ZIP ({e})")))?;
    if zip.len() > limits.max_entries {
        return Err(ExchangeError::Limit(format!("mehr als {} Einträge im Archiv", limits.max_entries)));
    }
    if zip.is_empty() {
        return Err(archive("leeres Archiv"));
    }

    // 1. Zentralverzeichnis selbst zählen: Doppel erkennen.
    let start = (0..zip.len())
        .filter_map(|i| zip.by_index_raw(i).ok().map(|f| f.central_header_start()))
        .min()
        .ok_or_else(|| archive("Zentralverzeichnis nicht lesbar"))?;
    let names = central_names(bytes, start)?;
    if names.len() > limits.max_entries {
        return Err(ExchangeError::Limit(format!("mehr als {} Einträge im Archiv", limits.max_entries)));
    }
    let mut seen = std::collections::HashSet::new();
    for name in &names {
        if !seen.insert(name.clone()) {
            return Err(archive(format!("Eintrag „{}“ kommt doppelt vor", String::from_utf8_lossy(name))));
        }
    }
    if names.len() != zip.len() {
        return Err(archive(format!("Zentralverzeichnis nennt {} Einträge, lesbar sind {}", names.len(), zip.len())));
    }

    // 2. Namen und Art jedes Eintrags (ohne zu entpacken).
    let mut entries: Vec<(String, CompressionMethod)> = Vec::with_capacity(zip.len());
    for index in 0..zip.len() {
        let file = zip.by_index_raw(index).map_err(|e| archive(format!("Eintrag {index} nicht lesbar ({e})")))?;
        let name = std::str::from_utf8(file.name_raw())
            .map_err(|_| archive(format!("Name des Eintrags {index} ist kein UTF-8")))?
            .to_owned();
        check_name(&name)?;
        if file.is_symlink() {
            return Err(archive(format!("„{name}“ ist eine Verknüpfung")));
        }
        if file.is_dir() {
            return Err(archive(format!("„{name}“ ist ein Ordner")));
        }
        if file.encrypted() {
            return Err(archive(format!("„{name}“ ist verschlüsselt")));
        }
        let method = file.compression();
        if !matches!(method, CompressionMethod::Stored | CompressionMethod::Deflated) {
            return Err(archive(format!(
                "„{name}“: Kompression {method:?} nicht unterstützt (nur STORED oder DEFLATE)"
            )));
        }
        entries.push((name, method));
    }

    // 3. mimetype zuerst, am Dateianfang, unkomprimiert.
    match entries.first() {
        Some((name, method)) if name == ENTRY_MIMETYPE => {
            if *method != CompressionMethod::Stored {
                return Err(archive("„mimetype“ muss unkomprimiert (STORED) sein"));
            }
            let header =
                zip.by_index_raw(0).map_err(|e| archive(format!("„mimetype“ nicht lesbar ({e})")))?.header_start();
            if header != 0 || bytes.get(30..38) != Some(b"mimetype".as_slice()) {
                return Err(archive("„mimetype“ steht nicht am Anfang der Datei"));
            }
        }
        _ => return Err(archive("erster Eintrag ist nicht „mimetype“")),
    }

    // 4. Entpacken mit Grenzen.
    let mut contents: HashMap<String, Vec<u8>> = HashMap::new();
    let mut total: u64 = 0;
    for (index, (name, _)) in entries.iter().enumerate() {
        let limit =
            if name == ENTRY_WORK { limits.max_entry_bytes.min(limits.max_xml_bytes) } else { limits.max_entry_bytes };
        let mut file = zip.by_index(index).map_err(|e| archive(format!("„{name}“ nicht lesbar ({e})")))?;
        let mut data = Vec::new();
        (&mut file)
            .take(limit + 1)
            .read_to_end(&mut data)
            .map_err(|e| archive(format!("„{name}“ lässt sich nicht entpacken ({e})")))?;
        if data.len() as u64 > limit {
            return Err(ExchangeError::Limit(format!("„{name}“ ist entpackt größer als {limit} Bytes")));
        }
        total += data.len() as u64;
        if total > limits.max_unpacked_bytes {
            return Err(ExchangeError::Limit(format!(
                "Archiv ist entpackt größer als {} Bytes",
                limits.max_unpacked_bytes
            )));
        }
        contents.insert(name.clone(), data);
    }
    if contents.get(ENTRY_MIMETYPE).map(Vec::as_slice) != Some(ZIP_MIMETYPE.as_bytes()) {
        return Err(archive(format!("„mimetype“ enthält nicht genau „{ZIP_MIMETYPE}“")));
    }

    // 5. Manifest.
    let manifest_xml = contents.get(ENTRY_MANIFEST).ok_or_else(|| archive("„MANIFEST.xml“ fehlt"))?;
    let manifest = super::read_manifest(manifest_xml, limits)?;
    let mut listed: HashMap<&str, &ExchangeManifestEntry> = HashMap::new();
    for entry in &manifest {
        if entry.path == ENTRY_MIMETYPE || entry.path == ENTRY_MANIFEST {
            return Err(archive(format!("das Manifest nennt „{}“ (nicht vorgesehen)", entry.path)));
        }
        if listed.insert(entry.path.as_str(), entry).is_some() {
            return Err(archive(format!("das Manifest nennt „{}“ doppelt", entry.path)));
        }
    }
    for (name, _) in &entries {
        if name == ENTRY_MIMETYPE || name == ENTRY_MANIFEST {
            continue;
        }
        let Some(entry) = listed.remove(name.as_str()) else {
            return Err(archive(format!("„{name}“ steht nicht im Manifest")));
        };
        let data = &contents[name];
        if entry.bytes != data.len() as u64 || entry.sha256 != sha256_hex(data) {
            return Err(ExchangeError::Checksum { path: name.clone() });
        }
    }
    if let Some(missing) = listed.keys().next() {
        return Err(archive(format!("das Manifest nennt „{missing}“, das Archiv enthält es nicht")));
    }

    // 6. werk.xml und Bilder.
    let xml = contents.remove(ENTRY_WORK).ok_or_else(|| archive("„werk.xml“ fehlt"))?;
    let root = dom::parse(&xml, limits)?;
    let (mut doc, refs) = map::document(&root)?;
    let mut used = std::collections::HashSet::new();
    for file_ref in refs {
        let revision = &mut doc.work.assets[file_ref.asset].revisions[file_ref.revision];
        let expected = image_entry(&revision.uuid, &revision.mime);
        if expected.as_deref() != Some(file_ref.entry.as_str()) {
            return Err(ExchangeError::Invalid(format!(
                "Bildfassung {} verweist auf „{}“ statt auf „{}“",
                revision.uuid,
                file_ref.entry,
                expected.unwrap_or_default()
            )));
        }
        let data = contents.get(&file_ref.entry).ok_or_else(|| {
            archive(format!("Bildfassung {} nennt „{}“, das Archiv enthält es nicht", revision.uuid, file_ref.entry))
        })?;
        if data.len() as u64 != file_ref.bytes {
            return Err(ExchangeError::Checksum { path: file_ref.entry.clone() });
        }
        revision.data = data.clone();
        used.insert(file_ref.entry);
    }
    if let Some(stray) = contents.keys().find(|k| k.starts_with(IMAGE_DIR) && !used.contains(*k)) {
        return Err(archive(format!("„{stray}“ gehört zu keiner Bildfassung in werk.xml")));
    }
    super::validate(&doc)?;
    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_outside_the_layout_are_refused_with_a_reason() {
        assert!(check_name("werk.xml").is_ok(), "werk.xml");
        assert!(check_name("bilder/26000000-0000-4000-8000-00000000003d.png").is_ok(), "Bild");
        for (name, word) in [
            ("/etc/passwd", "absolut"),
            ("bilder\\x.png", "Backslash"),
            ("../werk.xml", ".."),
            ("C:werk.xml", "Laufwerk"),
            ("bilder/kein-uuid.png", "unerlaubt"),
            ("bilder/26000000-0000-4000-8000-00000000003d.bmp", "unerlaubt"),
        ] {
            let message = check_name(name).unwrap_err().to_string();
            assert!(message.contains(word), "{name}: Meldung nennt {word}: {message}");
        }
    }
}
