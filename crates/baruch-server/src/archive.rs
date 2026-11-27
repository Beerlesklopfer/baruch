// SPDX-License-Identifier: AGPL-3.0-or-later
//! Reading ZIP archives on import (Round 21, interface R21-S1; then owned by markdown-import, signatures fixed).
//!
//! Used by pdf-import (EPUB is a ZIP) and markdown-import (ZIP upload with main file and images).
//! Modelled on the reader of the exchange format (`baruch_exchange`, `src/read/archive.rs`, limits as in R26), but
//! without its fixed set of names: here all that counts is that every entry is a regular file with a safe path.
//!
//! Rules of [`read_zip`], every violation aborts (the reader then returns nothing):
//! - Regular files only: folders are skipped, links and special files (device, FIFO, socket) →
//!   [`ArchiveError::Unsafe`]; encrypted entries likewise.
//! - Paths with `/`; rejected are absolute paths, `..`, `\`, drive letters (`C:`), NUL and control characters,
//!   empty names, names that are not UTF-8, and duplicate names – also those that differ only in upper/lower case
//!   (otherwise one file would overwrite the other on a case-insensitive file system). For this the
//!   central directory is counted by hand, because the ZIP library silently merges equal names.
//! - Sizes are counted while unpacking (`take(limit + 1)`), not taken from the header of the entry: per
//!   entry at most [`ArchiveLimits::max_entry_bytes`], in total at most [`ArchiveLimits::max_total_bytes`],
//!   at most [`ArchiveLimits::max_entries`] entries → otherwise [`ArchiveError::TooLarge`].
//! - Ratio unpacked/packed of an entry above [`ArchiveLimits::max_ratio`] → [`ArchiveError::TooLarge`]
//!   (zip bomb). Entries up to [`RATIO_FREE_BYTES`] unpacked are exempt: with files that small, a single
//!   line of equal characters already reaches high ratios, and the total limit covers them anyway.
//! - Only STORED and DEFLATE (as the dependency is built); other methods and faulty data (also a
//!   wrong checksum) → [`ArchiveError::Corrupt`]. What does not start with `PK` → [`ArchiveError::NotZip`].
//! - Order of the entries as in the archive.

use std::collections::HashSet;
use std::io::{Cursor, Read};

use zip::{CompressionMethod, ZipArchive};

/// Limits when reading an archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArchiveLimits {
    pub max_entries: usize,
    pub max_entry_bytes: u64,
    pub max_total_bytes: u64,
    pub max_ratio: u64,
}

/// Limits for imports (EPUB, Markdown ZIP): 2000 entries, 16 MiB each, 80 MiB in total, ratio at most 200.
pub const IMPORT_LIMITS: ArchiveLimits = ArchiveLimits {
    max_entries: 2000,
    max_entry_bytes: 16 * 1024 * 1024,
    max_total_bytes: 80 * 1024 * 1024,
    max_ratio: 200,
};

/// Up to this unpacked size no ratio applies to an entry (see module doc).
pub const RATIO_FREE_BYTES: u64 = 64 * 1024;

/// An entry that was read: path in the archive (with `/`) and content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchiveEntry {
    pub path: String,
    pub bytes: Vec<u8>,
}

/// Why an archive was not read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArchiveError {
    /// Not a ZIP archive (with reason).
    NotZip(String),
    /// An entry is unsafe (path, link, special file, duplicate, encrypted).
    Unsafe { path: String, reason: String },
    /// A limit is exceeded (count, size, ratio).
    TooLarge(String),
    /// The archive is damaged or uses an unsupported method.
    Corrupt(String),
}

impl std::fmt::Display for ArchiveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ArchiveError::NotZip(reason) => write!(f, "kein ZIP-Archiv: {reason}"),
            ArchiveError::Unsafe { path, reason } => write!(f, "unsicherer Eintrag „{path}“ im Archiv: {reason}"),
            ArchiveError::TooLarge(reason) => write!(f, "Archiv zu groß: {reason}"),
            ArchiveError::Corrupt(reason) => write!(f, "Archiv beschädigt: {reason}"),
        }
    }
}

impl std::error::Error for ArchiveError {}

const S_IFMT: u32 = 0o170000;
const S_IFREG: u32 = 0o100000;
const S_IFDIR: u32 = 0o040000;
const S_IFLNK: u32 = 0o120000;

fn unsafe_entry(path: &str, reason: impl Into<String>) -> ArchiveError {
    ArchiveError::Unsafe { path: path.to_owned(), reason: reason.into() }
}

/// Names of all entries of the central directory in its order, counted by hand (with duplicates). Stops after
/// `max + 1` names so that a huge directory is not read completely.
fn central_names(bytes: &[u8], start: u64, max: usize) -> Result<Vec<Vec<u8>>, ArchiveError> {
    let mut names = Vec::new();
    let mut at =
        usize::try_from(start).map_err(|_| ArchiveError::Corrupt("Zentralverzeichnis außerhalb der Datei".into()))?;
    while bytes.len() >= at + 46 && bytes[at..at + 4] == [0x50, 0x4b, 0x01, 0x02] {
        let u16_at = |offset: usize| usize::from(u16::from_le_bytes([bytes[at + offset], bytes[at + offset + 1]]));
        let (name_len, extra_len, comment_len) = (u16_at(28), u16_at(30), u16_at(32));
        let end = at + 46 + name_len;
        if end > bytes.len() {
            return Err(ArchiveError::Corrupt("Zentralverzeichnis abgeschnitten".into()));
        }
        names.push(bytes[at + 46..end].to_vec());
        if names.len() > max {
            break;
        }
        at = end + extra_len + comment_len;
    }
    Ok(names)
}

/// Start of the central directory according to the end record (EOCD, signature `PK\x05\x06`, at most 64 KiB of
/// comment after it); `None` without end record or with ZIP64 (`0xFFFFFFFF`), then the smallest known start applies.
fn central_start(bytes: &[u8]) -> Option<u64> {
    let floor = bytes.len().saturating_sub(22 + 65_535);
    let at = (floor..bytes.len().saturating_sub(21)).rev().find(|&i| bytes[i..i + 4] == [0x50, 0x4b, 0x05, 0x06])?;
    let offset = u32::from_le_bytes([bytes[at + 16], bytes[at + 17], bytes[at + 18], bytes[at + 19]]);
    (offset != u32::MAX && (offset as usize) < at).then_some(u64::from(offset))
}

/// Check the path of an entry (without the folder slash at the end).
fn check_path(path: &str) -> Result<(), ArchiveError> {
    if path.is_empty() {
        return Err(unsafe_entry(path, "leerer Name"));
    }
    if path.chars().any(char::is_control) {
        return Err(unsafe_entry(&path.escape_debug().to_string(), "Steuerzeichen im Namen"));
    }
    if path.starts_with('/') {
        return Err(unsafe_entry(path, "absoluter Pfad"));
    }
    if path.contains('\\') {
        return Err(unsafe_entry(path, "Backslash im Namen (Pfade trennt „/“)"));
    }
    if path.len() >= 2 && path.as_bytes()[1] == b':' && path.as_bytes()[0].is_ascii_alphabetic() {
        return Err(unsafe_entry(path, "Laufwerksangabe im Namen"));
    }
    if path.split('/').any(|part| part == "..") {
        return Err(unsafe_entry(path, "„..“ im Pfad (führte aus dem Archiv heraus)"));
    }
    Ok(())
}

/// Read and check a ZIP archive (rules in the module doc). Returns the regular files in the order of the archive.
pub fn read_zip(bytes: &[u8], limits: &ArchiveLimits) -> Result<Vec<ArchiveEntry>, ArchiveError> {
    if !bytes.starts_with(b"PK\x03\x04") && !bytes.starts_with(b"PK\x05\x06") {
        return Err(ArchiveError::NotZip("der Anfang „PK“ eines ZIP-Archivs fehlt".into()));
    }
    let mut zip = ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| ArchiveError::Corrupt(format!("Inhaltsverzeichnis nicht lesbar ({e})")))?;
    if zip.len() > limits.max_entries {
        return Err(ArchiveError::TooLarge(format!("mehr als {} Einträge", limits.max_entries)));
    }
    if zip.is_empty() {
        return Ok(Vec::new());
    }

    // Count the central directory by hand: the library merges equal names. The start comes from the
    // end record (EOCD), not from the library's entries: their first entry may already be the second duplicate.
    let listed_start = (0..zip.len())
        .filter_map(|i| zip.by_index_raw(i).ok().map(|f| f.central_header_start()))
        .min()
        .ok_or_else(|| ArchiveError::Corrupt("Zentralverzeichnis nicht lesbar".into()))?;
    let start = central_start(bytes).map_or(listed_start, |eocd| eocd.min(listed_start));
    let names = central_names(bytes, start, limits.max_entries)?;
    if names.len() > limits.max_entries {
        return Err(ArchiveError::TooLarge(format!("mehr als {} Einträge", limits.max_entries)));
    }
    let mut seen = HashSet::new();
    for name in &names {
        if !seen.insert(name.as_slice()) {
            return Err(unsafe_entry(&String::from_utf8_lossy(name), "Name kommt doppelt vor"));
        }
    }
    if names.len() != zip.len() {
        return Err(ArchiveError::Corrupt(format!(
            "Zentralverzeichnis nennt {} Einträge, lesbar sind {}",
            names.len(),
            zip.len()
        )));
    }

    // Name and kind of every entry, without unpacking.
    let mut files: Vec<(usize, String, u64)> = Vec::new();
    let mut folded = HashSet::new();
    for index in 0..zip.len() {
        let file = zip
            .by_index_raw(index)
            .map_err(|e| ArchiveError::Corrupt(format!("Eintrag {index} nicht lesbar ({e})")))?;
        let raw = std::str::from_utf8(file.name_raw())
            .map_err(|_| unsafe_entry(&String::from_utf8_lossy(file.name_raw()), "Name ist kein UTF-8"))?;
        let kind = file.unix_mode().map(|mode| mode & S_IFMT).unwrap_or(0);
        let is_dir = raw.ends_with('/') || kind == S_IFDIR;
        let path = raw.strip_suffix('/').unwrap_or(raw);
        check_path(path)?;
        if is_dir {
            continue;
        }
        if kind == S_IFLNK {
            return Err(unsafe_entry(path, "Verknüpfung (Symlink)"));
        }
        if kind != 0 && kind != S_IFREG {
            return Err(unsafe_entry(path, "keine reguläre Datei (Gerät, FIFO oder Socket)"));
        }
        if file.encrypted() {
            return Err(unsafe_entry(path, "verschlüsselt"));
        }
        let method = file.compression();
        if !matches!(method, CompressionMethod::Stored | CompressionMethod::Deflated) {
            return Err(ArchiveError::Corrupt(format!(
                "„{path}“: Kompression {method:?} nicht unterstützt (nur STORED oder DEFLATE)"
            )));
        }
        if !folded.insert(path.to_lowercase()) {
            return Err(unsafe_entry(path, "Name kommt doppelt vor (Groß-/Kleinschreibung zählt nicht)"));
        }
        files.push((index, path.to_owned(), file.compressed_size()));
    }

    // Unpack with limits, counted on the bytes read.
    let mut entries = Vec::with_capacity(files.len());
    let mut total: u64 = 0;
    for (index, path, packed) in files {
        let room = limits.max_entry_bytes.min(limits.max_total_bytes - total);
        let mut file =
            zip.by_index(index).map_err(|e| ArchiveError::Corrupt(format!("„{path}“ nicht lesbar ({e})")))?;
        let mut data = Vec::new();
        (&mut file)
            .take(room + 1)
            .read_to_end(&mut data)
            .map_err(|e| ArchiveError::Corrupt(format!("„{path}“ lässt sich nicht entpacken ({e})")))?;
        let size = data.len() as u64;
        if size > limits.max_entry_bytes {
            return Err(ArchiveError::TooLarge(format!(
                "„{path}“ ist entpackt größer als {} Bytes",
                limits.max_entry_bytes
            )));
        }
        if size > room {
            return Err(ArchiveError::TooLarge(format!(
                "die Einträge sind entpackt zusammen größer als {} Bytes",
                limits.max_total_bytes
            )));
        }
        if size > RATIO_FREE_BYTES && size > packed.max(1).saturating_mul(limits.max_ratio) {
            return Err(ArchiveError::TooLarge(format!(
                "„{path}“ wächst beim Entpacken auf mehr als das {}-fache (Zip-Bombe?)",
                limits.max_ratio
            )));
        }
        total += size;
        entries.push(ArchiveEntry { path, bytes: data });
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use zip::write::SimpleFileOptions;

    use super::*;

    /// Build an archive from (name, content, Unix mode); DEFLATE.
    fn zip_of(entries: &[(&str, &[u8], u32)]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, bytes, mode) in entries {
            let options =
                SimpleFileOptions::default().compression_method(CompressionMethod::Deflated).unix_permissions(*mode);
            writer.start_file(*name, options).unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    fn file(name: &str, bytes: &[u8]) -> (String, Vec<u8>) {
        (name.to_owned(), bytes.to_vec())
    }

    #[test]
    fn regular_files_come_back_in_archive_order_and_folders_are_skipped() {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default();
        writer.start_file("text/haupt.md", options).unwrap();
        writer.write_all(b"# Ein erfundener Titel\n").unwrap();
        writer.add_directory("bilder/", options).unwrap();
        writer.start_file("bilder/leuchtturm.png", options).unwrap();
        writer.write_all(b"\x89PNG erfunden").unwrap();
        writer.start_file("anhang.txt", options.compression_method(CompressionMethod::Stored)).unwrap();
        writer.write_all(b"ungepackt").unwrap();
        let bytes = writer.finish().unwrap().into_inner();
        let entries = read_zip(&bytes, &IMPORT_LIMITS).expect("ein sauberes Archiv wird gelesen");
        let got: Vec<(String, Vec<u8>)> = entries.into_iter().map(|e| (e.path, e.bytes)).collect();
        assert_eq!(
            got,
            vec![
                file("text/haupt.md", b"# Ein erfundener Titel\n"),
                file("bilder/leuchtturm.png", b"\x89PNG erfunden"),
                file("anhang.txt", b"ungepackt"),
            ],
            "Reihenfolge wie im Archiv, Inhalte unverändert, der Ordner-Eintrag fehlt (nur reguläre Dateien)"
        );
    }

    #[test]
    fn something_that_is_not_a_zip_is_refused_as_not_zip() {
        let error = read_zip(b"%PDF-1.7 erfunden", &IMPORT_LIMITS).unwrap_err();
        assert!(matches!(error, ArchiveError::NotZip(_)), "ein PDF ist kein ZIP und wird so benannt: {error:?}");
        assert!(error.to_string().contains("kein ZIP-Archiv"), "deutsche Meldung: {error}");
    }

    #[test]
    fn an_empty_archive_yields_no_entries() {
        let bytes = zip::ZipWriter::new(Cursor::new(Vec::new())).finish().unwrap().into_inner();
        assert_eq!(
            read_zip(&bytes, &IMPORT_LIMITS),
            Ok(Vec::new()),
            "leer ist kein Fehler; was fehlt, sagt der Aufrufer"
        );
    }

    #[test]
    fn unsafe_paths_are_refused_with_the_path_and_a_reason() {
        for (name, word) in [
            ("/etc/haupt.md", "absolut"),
            ("bilder\\a.png", "Backslash"),
            ("../haupt.md", ".."),
            ("text/../../haupt.md", ".."),
            ("C:haupt.md", "Laufwerk"),
            ("a\u{1}b.md", "Steuerzeichen"),
        ] {
            let bytes = zip_of(&[(name, b"x", 0o644)]);
            let error = read_zip(&bytes, &IMPORT_LIMITS).unwrap_err();
            assert!(
                matches!(&error, ArchiveError::Unsafe { reason, .. } if reason.contains(word)),
                "{name:?} führte aus dem Zielordner heraus bzw. ist kein portabler Pfad, der Grund nennt „{word}“: {error:?}"
            );
        }
    }

    #[test]
    fn symlinks_and_special_files_are_refused() {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer.add_symlink("bild.png", "/etc/passwd", SimpleFileOptions::default()).unwrap();
        let bytes = writer.finish().unwrap().into_inner();
        let error = read_zip(&bytes, &IMPORT_LIMITS).unwrap_err();
        assert!(
            matches!(&error, ArchiveError::Unsafe { path, reason } if path == "bild.png" && reason.contains("Verknüpfung")),
            "eine Verknüpfung zeigte beim Auspacken auf fremde Dateien: {error:?}"
        );
        // FIFO (0o010000): the writer accepts permission bits only, so set the mode in the external attributes of the
        // central directory (upper 16 bits, offset 38) by hand.
        let mut bytes = zip_of(&[("rohr", b"", 0o644)]);
        let central = bytes.windows(4).position(|w| w == [0x50, 0x4b, 0x01, 0x02]).unwrap();
        bytes[central + 38..central + 42].copy_from_slice(&(0o010644u32 << 16).to_le_bytes());
        let error = read_zip(&bytes, &IMPORT_LIMITS).unwrap_err();
        assert!(
            matches!(&error, ArchiveError::Unsafe { reason, .. } if reason.contains("reguläre Datei")),
            "eine Sonderdatei ist keine Datei, die ein Import lesen kann: {error:?}"
        );
    }

    #[test]
    fn duplicate_names_are_refused_also_when_only_the_case_differs() {
        let bytes = zip_of(&[("Kapitel.md", b"a", 0o644), ("kapitel.md", b"b", 0o644)]);
        let error = read_zip(&bytes, &IMPORT_LIMITS).unwrap_err();
        assert!(
            matches!(&error, ArchiveError::Unsafe { path, reason } if path == "kapitel.md" && reason.contains("doppelt")),
            "auf einem Dateisystem ohne Groß/klein überschriebe die zweite Datei die erste: {error:?}"
        );
        // Exactly equal names: the writer does not allow that, so rewrite the second name in the bytes
        // (header and central directory, same length). The library otherwise merges such duplicates silently.
        let mut bytes = zip_of(&[("eins.md", b"a", 0o644), ("zwei.md", b"b", 0o644)]);
        let mut at = 0;
        while let Some(found) = bytes[at..].windows(7).position(|w| w == b"zwei.md") {
            bytes[at + found..at + found + 7].copy_from_slice(b"eins.md");
            at += found + 7;
        }
        let error = read_zip(&bytes, &IMPORT_LIMITS).unwrap_err();
        assert!(
            matches!(&error, ArchiveError::Unsafe { path, reason } if path == "eins.md" && reason.contains("doppelt")),
            "zwei Einträge mit gleichem Namen: welcher gilt, wäre Zufall: {error:?}"
        );
    }

    #[test]
    fn limits_are_counted_while_unpacking() {
        let small = ArchiveLimits { max_entries: 2, max_entry_bytes: 1000, max_total_bytes: 1500, max_ratio: 200 };
        let error =
            read_zip(&zip_of(&[("a", b"1", 0o644), ("b", b"2", 0o644), ("c", b"3", 0o644)]), &small).unwrap_err();
        assert!(matches!(error, ArchiveError::TooLarge(_)), "drei Einträge über der Grenze 2: {error:?}");
        let error = read_zip(&zip_of(&[("a", &[b'x'; 1001], 0o644)]), &small).unwrap_err();
        assert!(
            matches!(&error, ArchiveError::TooLarge(m) if m.contains("„a“")),
            "1001 Bytes über der Grenze je Eintrag (1000), der Eintrag ist genannt: {error:?}"
        );
        let error = read_zip(&zip_of(&[("a", &[b'x'; 800], 0o644), ("b", &[b'y'; 800], 0o644)]), &small).unwrap_err();
        assert!(
            matches!(&error, ArchiveError::TooLarge(m) if m.contains("zusammen")),
            "je 800 Bytes passen einzeln, zusammen (1600) nicht in 1500: {error:?}"
        );
        assert!(
            read_zip(&zip_of(&[("a", &[b'x'; 700], 0o644), ("b", &[b'y'; 800], 0o644)]), &small).is_ok(),
            "genau 1500 Bytes zusammen liegen noch in der Grenze"
        );
    }

    #[test]
    fn the_size_in_the_header_is_not_trusted() {
        // Forge the size in the central directory to 1 byte: reading still stops only at the limit, and the
        // error comes from counting (limit) or from the checksum – never a silent truncation.
        let small = ArchiveLimits { max_entries: 10, max_entry_bytes: 100, max_total_bytes: 1000, max_ratio: 200 };
        let mut bytes = zip_of(&[("a", &[b'x'; 500], 0o644)]);
        let central = bytes.windows(4).position(|w| w == [0x50, 0x4b, 0x01, 0x02]).unwrap();
        bytes[central + 24..central + 28].copy_from_slice(&1u32.to_le_bytes());
        let result = read_zip(&bytes, &small);
        assert!(
            matches!(result, Err(ArchiveError::TooLarge(_)) | Err(ArchiveError::Corrupt(_))),
            "500 entpackte Bytes bei angeblich 1 Byte: abgelehnt, nicht abgeschnitten übernommen: {result:?}"
        );
    }

    #[test]
    fn a_zip_bomb_is_refused_by_its_ratio() {
        let zeros = vec![0u8; 2 * 1024 * 1024];
        let bytes = zip_of(&[("bombe.md", &zeros, 0o644)]);
        let error = read_zip(&bytes, &IMPORT_LIMITS).unwrap_err();
        assert!(
            matches!(&error, ArchiveError::TooLarge(m) if m.contains("200")),
            "2 MiB Nullen packen sich auf wenige KiB (Verhältnis ≫ 200): {error:?}"
        );
        let text = "Ein erfundener Absatz über den Leuchtturm am Kap. ".repeat(20);
        assert!(
            read_zip(&zip_of(&[("klein.md", text.as_bytes(), 0o644)]), &IMPORT_LIMITS).is_ok(),
            "kleine Dateien (bis 64 KiB) sind vom Verhältnis frei, auch wenn sie sich gut packen"
        );
    }

    #[test]
    fn a_broken_archive_is_corrupt() {
        let mut bytes = zip_of(&[("a.md", b"Ein erfundener Satz, lang genug zum Packen.", 0o644)]);
        // Corrupt the checksum (CRC-32) in the central directory.
        let central = bytes.windows(4).position(|w| w == [0x50, 0x4b, 0x01, 0x02]).unwrap();
        bytes[central + 16] ^= 0xff;
        let error = read_zip(&bytes, &IMPORT_LIMITS).unwrap_err();
        assert!(matches!(error, ArchiveError::Corrupt(_)), "falsche Prüfsumme heißt beschädigt: {error:?}");
        let error = read_zip(b"PK\x03\x04abgeschnitten", &IMPORT_LIMITS).unwrap_err();
        assert!(matches!(error, ArchiveError::Corrupt(_)), "abgeschnittenes Archiv: {error:?}");
    }
}
