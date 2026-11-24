// SPDX-License-Identifier: AGPL-3.0-or-later
//! Bilder und Grafiken eines Werks (Migration `0009_assets.sql`, Vertrag Runde 6, Abschnitt 4).
//!
//! - Ein Bild (`asset`) hat einen Namen, eindeutig je Werk ([`valid_asset_name`]: `[a-z0-9._-]`,
//!   höchstens 80 Zeichen, Endung passend zum Typ), und Fassungen (`asset_revision`) wie ein
//!   Abschnitt: genau eine trägt `current` (Teilindex `asset_one_current`), jede andere lässt sich
//!   aktuell schalten ([`crate::Store::set_current_asset`]). Ersetzen ist immer eine neue Fassung
//!   ([`crate::Store::replace_asset`]), der Inhalt einer Fassung ist per Trigger unveränderlich;
//!   ändern lassen sich nur Bildunterschrift, Alternativtext und Notiz
//!   ([`crate::Store::update_asset_revision`], setzt `edited`). Ändern und Löschen sind dem Menschen
//!   vorbehalten (über MCP gibt es nur lesende Werkzeuge).
//! - Inhalt als TEXT, nie BLOB: SVG als UTF-8-Text (`encoding = 'svg'`), alles andere (und SVG auf
//!   Wunsch) als Base64 im Standardalphabet mit `=` und ohne Zeilenumbrüche (`'base64'`). `bytes` und
//!   `sha256` beziehen sich auf die entschlüsselten Bytes. Höchstens [`MAX_ASSET_BYTES`] je Fassung.
//! - Pfade (`asset_path`): so, wie sie in Vorlage oder Text stehen (`/bilder/haus.svg`), je Werk
//!   eindeutig. Export ([`crate::Store::export_assets`]) schreibt je Pfad die aktuelle Fassung, der
//!   Spiegel ([`mirror_files`]) je Bild `bilder/<name>` (SVG-Text) bzw. `bilder/<name>.b64`.
//! - Jede schreibende Methode ist eine Transaktion (`BEGIN IMMEDIATE`) und gibt eine [`AssetChange`]
//!   für den Spiegel-Commit zurück.

use std::collections::{BTreeSet, HashMap};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::SqliteConnection;

use crate::{Imported, MirrorFile, Store, StoreError, WorkId, broken, import, to_u32};

/// Größte Bilddatei (entschlüsselt, je Fassung): 8 MiB.
pub const MAX_ASSET_BYTES: usize = 8 * 1024 * 1024;

/// Unterstützte Typen: MIME, erlaubte Endungen (die erste ist die Vorgabe).
const TYPES: &[(&str, &[&str])] = &[
    ("image/png", &["png"]),
    ("image/jpeg", &["jpg", "jpeg"]),
    ("image/gif", &["gif"]),
    ("image/webp", &["webp"]),
    ("image/svg+xml", &["svg"]),
];

/// Zeilenlänge der Base64-Dateien im Spiegel.
const MIRROR_LINE: usize = 76;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetEncoding {
    /// Base64 (Standardalphabet, mit `=`, ohne Zeilenumbrüche).
    Base64,
    /// SVG als UTF-8-Text.
    Svg,
}

impl AssetEncoding {
    /// Wort der Datenbank (= serde-Name).
    pub fn as_str(self) -> &'static str {
        match self {
            AssetEncoding::Base64 => "base64",
            AssetEncoding::Svg => "svg",
        }
    }

    pub fn parse(s: &str) -> Option<AssetEncoding> {
        match s {
            "base64" => Some(AssetEncoding::Base64),
            "svg" => Some(AssetEncoding::Svg),
            _ => None,
        }
    }
}

/// Ein Bild in der Übersicht ([`Store::asset_list`]); Angaben der aktuellen Fassung.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssetSummary {
    pub name: String,
    pub mime: String,
    pub encoding: AssetEncoding,
    pub bytes: u64,
    /// Nummer der aktuellen Fassung.
    pub current: u32,
    /// Anzahl der Fassungen.
    pub revisions: usize,
    pub caption: String,
    pub alt: String,
    /// Pfade, unter denen Vorlage und Text das Bild nennen, sortiert.
    pub paths: Vec<String>,
    /// Letzte Änderung: größter Zeitpunkt aus `created`/`edited` der Fassungen.
    pub updated: String,
}

/// Eine Fassung eines Bilds (ohne Inhalt).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssetRevisionInfo {
    pub number: u32,
    pub based_on: Option<u32>,
    pub current: bool,
    pub mime: String,
    pub encoding: AssetEncoding,
    pub bytes: u64,
    pub sha256: String,
    pub caption: String,
    pub alt: String,
    pub author: String,
    pub note: String,
    pub created: String,
    pub edited: Option<String>,
}

/// Inhalt einer Fassung, wie er in der Datenbank steht (Base64 bzw. SVG-Text).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssetData {
    pub name: String,
    pub rev: u32,
    pub mime: String,
    pub encoding: AssetEncoding,
    pub content: String,
}

impl AssetData {
    /// Die Bytes der Datei (Base64 entschlüsselt bzw. der SVG-Text als UTF-8).
    pub fn bytes(&self) -> Result<Vec<u8>, StoreError> {
        match self.encoding {
            AssetEncoding::Base64 => decode_base64(&self.content),
            AssetEncoding::Svg => Ok(self.content.as_bytes().to_vec()),
        }
    }
}

/// Neues Bild für [`Store::add_asset`].
#[derive(Debug, Clone)]
pub struct NewAsset<'a> {
    /// Dateiname beim Hochladen; daraus der Name, wenn `name` fehlt.
    pub file_name: &'a str,
    /// Eigener Name; vergeben → `Refused`.
    pub name: Option<&'a str>,
    pub bytes: &'a [u8],
    /// SVG als Base64 speichern statt als Text (andere Typen sind immer Base64).
    pub as_base64: bool,
    /// Pfad(e) in Vorlage/Text, eine je Zeile; leer = keiner.
    pub path: Option<&'a str>,
    pub caption: &'a str,
    pub alt: &'a str,
    pub note: &'a str,
}

/// Bild für [`Store::import_typst_with_assets`]: Pfad, wie er im Text steht, Dateiname, Bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportAsset {
    pub path: String,
    pub file_name: String,
    pub bytes: Vec<u8>,
}

/// Datei für den Export ([`Store::export_assets`]): Pfad ohne führendes `/`, Bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportedFile {
    pub path: String,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetChangeKind {
    /// Neues Bild (Fassung 1).
    Added,
    /// Neue Fassung, sofort aktuell.
    Replaced,
    /// Markierung „aktuell“ auf eine andere Fassung umgeschaltet.
    Switched,
    /// Bildunterschrift, Alternativtext, Notiz oder Pfade geändert.
    Updated,
    /// Eine (nicht aktuelle) Fassung gelöscht.
    Deleted,
    /// Das ganze Bild samt Fassungen und Pfaden gelöscht.
    Removed,
}

/// Ergebnis einer Bildänderung für den Spiegel-Commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssetChange {
    pub work: WorkId,
    pub name: String,
    /// Added/Replaced/Switched: die jetzt aktuelle Fassung; Updated/Deleted: die betroffene;
    /// Removed: die zuletzt aktuelle.
    pub rev: u32,
    pub kind: AssetChangeKind,
    /// Replaced/Switched: die vorher aktuelle Fassung; Updated/Deleted/Removed: die betroffene vorher.
    pub before: Option<AssetRevisionInfo>,
    /// Deleted/Removed: der gelöschte Inhalt (damit er aus Git wiederherstellbar bleibt).
    pub removed: Option<AssetData>,
    /// Lesbare Einzelheiten für den Commit-Rumpf (Typ, Größe, Pfade vorher/nachher …).
    pub detail: String,
}

/// Eine Schreibweise eines Pfads und das Bild, dem sie gehört ([`PathConflict`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PathOwner {
    /// Pfad, wie er gespeichert ist (wie im Text).
    pub path: String,
    /// Name des Bilds.
    pub asset: String,
}

/// Altbestand (Runde 21c): mehrere Schreibweisen derselben Normalform gehören verschiedenen Bildern
/// ([`Store::asset_path_conflicts`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PathConflict {
    /// Pfad in Normalform ([`normalize_asset_path`]).
    pub normal: String,
    /// Alle Schreibweisen mit ihrem Bild, nach Pfad sortiert.
    pub paths: Vec<PathOwner>,
}

/// Kollisionen eines Werks (siehe [`Store::asset_path_conflicts`]).
async fn path_conflicts(conn: &mut SqliteConnection, work: WorkId) -> Result<Vec<PathConflict>, StoreError> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT p.path, a.name FROM asset_path p JOIN asset a ON a.id = p.asset_id WHERE p.work_id = ? ORDER BY p.path",
    )
    .bind(work)
    .fetch_all(&mut *conn)
    .await?;
    let mut groups: std::collections::BTreeMap<String, Vec<PathOwner>> = std::collections::BTreeMap::new();
    for (path, asset) in rows {
        if let Some(normal) = normalize_asset_path(&path) {
            groups.entry(normal).or_default().push(PathOwner { path, asset });
        }
    }
    Ok(groups
        .into_iter()
        .filter(|(_, paths)| paths.iter().map(|p| p.asset.as_str()).collect::<BTreeSet<_>>().len() > 1)
        .map(|(normal, paths)| PathConflict { normal, paths })
        .collect())
}

// ---- Prüfen und Umwandeln ----------------------------------------------------------------------

/// MIME-Typ aus den ersten Bytes: PNG, JPEG, GIF, WebP, SVG (UTF-8-Text, der nach optionalem
/// BOM, XML-Prolog, Kommentaren und DOCTYPE mit `<svg` beginnt); sonst `None`.
pub fn detect_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else if is_svg(bytes) {
        Some("image/svg+xml")
    } else {
        None
    }
}

fn is_svg(bytes: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else { return false };
    let mut rest = text.strip_prefix('\u{feff}').unwrap_or(text);
    loop {
        rest = rest.trim_start();
        let skipped = if rest.starts_with("<?") {
            rest.find("?>").map(|end| &rest[end + 2..])
        } else if rest.starts_with("<!--") {
            rest.find("-->").map(|end| &rest[end + 3..])
        } else if rest.starts_with("<!") {
            // DOCTYPE, auch mit internem Teil in eckigen Klammern.
            let close = match (rest.find('['), rest.find('>')) {
                (Some(open), Some(gt)) if open < gt => rest.find("]>").map(|e| e + 2),
                (_, Some(gt)) => Some(gt + 1),
                _ => None,
            };
            close.map(|end| &rest[end..])
        } else {
            break;
        };
        match skipped {
            Some(next) => rest = next,
            None => return false,
        }
    }
    rest.strip_prefix("<svg")
        .is_some_and(|after| after.chars().next().is_some_and(|c| c.is_whitespace() || c == '>' || c == '/'))
}

/// Erlaubte Endungen eines MIME-Typs.
fn extensions(mime: &str) -> &'static [&'static str] {
    TYPES.iter().find(|(m, _)| *m == mime).map_or(&[], |(_, e)| e)
}

/// Endung des Namens (klein), falls vorhanden.
fn extension(name: &str) -> Option<&str> {
    name.rsplit_once('.').map(|(_, ext)| ext)
}

/// Gültiger Bildname: `^[a-z0-9]([a-z0-9._-]{0,78}[a-z0-9])?$` und eine Endung `.png`, `.jpg`,
/// `.jpeg`, `.gif`, `.webp` oder `.svg`.
pub fn valid_asset_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    let edge = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit();
    let inner = |b: u8| edge(b) || matches!(b, b'.' | b'_' | b'-');
    !bytes.is_empty()
        && bytes.len() <= 80
        && edge(bytes[0])
        && edge(bytes[bytes.len() - 1])
        && bytes.iter().all(|&b| inner(b))
        && extension(name).is_some_and(|ext| TYPES.iter().any(|(_, list)| list.contains(&ext)))
}

/// Passt die Endung des Namens zum MIME-Typ?
pub(crate) fn name_fits(name: &str, mime: &str) -> bool {
    extension(name).is_some_and(|ext| extensions(mime).contains(&ext))
}

/// Name aus einem Dateinamen: ohne Ordner, klein, Zeichen außerhalb `[a-z0-9._-]` → `-` (Folgen
/// zusammengefasst), Ränder ohne Satzzeichen, höchstens 80 Zeichen; passt die Endung nicht zum Typ,
/// kommt die Vorgabe-Endung dazu (z. B. `foto` → `foto.png`).
pub fn asset_name_from_file(file_name: &str, mime: &str) -> String {
    let base = file_name.rsplit(['/', '\\']).next().unwrap_or("");
    let mut out = String::new();
    for c in base.chars().flat_map(char::to_lowercase) {
        if c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-') {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    let ext = extensions(mime).first().copied().unwrap_or("bin");
    let (mut stem, mut ext) = match out.rsplit_once('.') {
        Some((stem, e)) if extensions(mime).contains(&e) => (stem.to_owned(), e.to_owned()),
        _ => (out.clone(), ext.to_owned()),
    };
    let trim = |s: &str| s.trim_matches(|c: char| !c.is_ascii_alphanumeric()).to_owned();
    stem = trim(&stem);
    if stem.is_empty() {
        stem = "bild".to_owned();
    }
    ext = ext.to_ascii_lowercase();
    let max_stem = 80 - ext.len() - 1;
    if stem.len() > max_stem {
        stem.truncate(max_stem);
        stem = trim(&stem);
    }
    format!("{stem}.{ext}")
}

/// `name` mit `-2`, `-3` … vor der Endung (Grenze 80 Zeichen bleibt).
fn numbered_name(name: &str, n: usize) -> String {
    let (stem, ext) = name.rsplit_once('.').unwrap_or((name, ""));
    let suffix = format!("-{n}.{ext}");
    let keep = stem.len().min(80 - suffix.len());
    let stem = stem[..keep].trim_end_matches(|c: char| !c.is_ascii_alphanumeric());
    format!("{stem}{suffix}")
}

/// Base64 (Standardalphabet, mit `=`) → Bytes; kaputt → `StoreError::Asset`.
pub fn decode_base64(text: &str) -> Result<Vec<u8>, StoreError> {
    STANDARD.decode(text).map_err(|e| StoreError::Asset(format!("Kein gültiges Base64 ({e}).")))
}

/// Bytes → Base64 (Standardalphabet, mit `=`, ohne Zeilenumbrüche).
pub fn encode_base64(bytes: &[u8]) -> String {
    STANDARD.encode(bytes)
}

/// SHA-256 als Kleinbuchstaben-Hex.
fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// Bildpfad in Normalform (Runde 7): führendes `/` und `./`-Teile entfernen, leere Teile
/// zusammenfassen (`/bilder//a.png`, `./bilder/a.png` → `bilder/a.png`); ein `..`-Teil oder ein
/// leerer Pfad → `None`. So finden Vorlage, Text, die Seite `/werke/{id}/bilder/pfad?p=`, das
/// Satzbild und `export --out` dasselbe Bild, gleich wie der Pfad im Text geschrieben ist.
/// Gespeichert wird der Pfad weiter so, wie er im Text steht (der Text bleibt byte-gleich).
pub fn normalize_asset_path(path: &str) -> Option<String> {
    let mut parts = Vec::new();
    for part in path.trim().split('/') {
        match part {
            "" | "." => {}
            ".." => return None,
            name => parts.push(name),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// Pfad, wie er in Vorlage/Text steht, prüfen: nicht leer, keine Steuerzeichen, kein `\`, keine
/// Adresse (`://`), kein `..`-Teil und mit Normalform ([`normalize_asset_path`]; `./` und leere Teile
/// sind erlaubt). Ergebnis getrimmt, sonst wie geschrieben.
fn check_path(path: &str) -> Result<String, StoreError> {
    let path = path.trim();
    let bad = |why: &str| StoreError::Asset(format!("Ungültiger Pfad „{path}“: {why}."));
    if path.is_empty() {
        return Err(bad("leer"));
    }
    if path.len() > 400 {
        return Err(bad("zu lang"));
    }
    if path.chars().any(|c| c.is_control() || c == '\\') || path.contains("://") {
        return Err(bad("nur Ordner und Dateiname mit '/' getrennt"));
    }
    if path.split('/').any(|part| part == "..") {
        return Err(bad("'..' ist nicht erlaubt"));
    }
    if normalize_asset_path(path).is_none() {
        return Err(bad("kein Dateiname"));
    }
    Ok(path.to_owned())
}

/// Prüft einen Bildpfad nach denselben Regeln wie der Store beim Speichern (siehe `check_path`,
/// Normalform über [`normalize_asset_path`]):
/// Wer Pfade aus fremdem Text übernimmt (Ordner-Import), kann so einen ungültigen überspringen,
/// statt den ganzen Import scheitern zu lassen. Ergebnis getrimmt.
pub fn check_asset_path(path: &str) -> Result<String, StoreError> {
    check_path(path)
}

/// Pfade aus einer Liste (je Eintrag auch mehrere Zeilen), geprüft, ohne Doppelte, sortiert.
fn check_paths<'a>(paths: impl IntoIterator<Item = &'a str>) -> Result<Vec<String>, StoreError> {
    let mut out = BTreeSet::new();
    for entry in paths {
        for line in entry.lines().map(str::trim).filter(|l| !l.is_empty()) {
            out.insert(check_path(line)?);
        }
    }
    Ok(out.into_iter().collect())
}

/// Geprüfter Inhalt einer neuen Fassung (auch für das Einlesen einer Austauschdatei, Runde 26: dieselbe Prüfung
/// wie beim Hochladen).
pub(crate) struct Content {
    pub(crate) mime: &'static str,
    pub(crate) encoding: AssetEncoding,
    pub(crate) content: String,
    pub(crate) bytes: i64,
    pub(crate) sha256: String,
}

/// Bildbytes prüfen wie beim Hochladen: höchstens [`MAX_ASSET_BYTES`], ein unterstützter Typ (an den ersten Bytes
/// erkannt); SVG als Text, sonst Base64.
pub(crate) fn prepare(file_name: &str, bytes: &[u8], as_base64: bool) -> Result<Content, StoreError> {
    let shown = file_name.rsplit(['/', '\\']).next().unwrap_or(file_name);
    if bytes.len() > MAX_ASSET_BYTES {
        return Err(StoreError::Asset(format!("„{shown}“ ist größer als 8 MiB.")));
    }
    let Some(mime) = detect_mime(bytes) else {
        return Err(StoreError::Asset(format!("„{shown}“ ist kein unterstütztes Bild (PNG, JPEG, GIF, WebP, SVG).")));
    };
    let (encoding, content) = if mime == "image/svg+xml" && !as_base64 {
        // detect_mime erkennt SVG nur in gültigem UTF-8.
        let text = String::from_utf8(bytes.to_vec()).map_err(|_| broken("SVG ohne UTF-8".into()))?;
        (AssetEncoding::Svg, text)
    } else {
        (AssetEncoding::Base64, encode_base64(bytes))
    };
    Ok(Content { mime, encoding, content, bytes: bytes.len() as i64, sha256: sha256_hex(bytes) })
}

/// Base64 in Zeilen zu 76 Zeichen, jede mit `\n` (Spiegel, Commit-Rumpf).
pub(crate) fn wrap_lines(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + text.len() / MIRROR_LINE + 1);
    for chunk in text.as_bytes().chunks(MIRROR_LINE) {
        // Base64 ist ASCII.
        out.push_str(std::str::from_utf8(chunk).unwrap_or(""));
        out.push('\n');
    }
    out
}

/// Größe lesbar: „512 Bytes“, „3,2 KiB“, „1,5 MiB“.
fn size_text(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} Bytes")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0).replace('.', ",")
    } else {
        format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0)).replace('.', ",")
    }
}

// ---- Lesen -------------------------------------------------------------------------------------

type InfoRow =
    (i64, Option<i64>, String, String, String, i64, String, String, String, String, String, String, Option<String>);

const INFO_COLUMNS: &str =
    "number, based_on, status, mime, encoding, bytes, sha256, caption, alt, author_name, note, created, edited";

fn parse_encoding(s: &str) -> Result<AssetEncoding, StoreError> {
    AssetEncoding::parse(s).ok_or_else(|| broken(format!("unbekannte Bildkodierung {s:?}")))
}

fn info_of(row: InfoRow) -> Result<AssetRevisionInfo, StoreError> {
    let (number, based_on, status, mime, encoding, bytes, sha256, caption, alt, author, note, created, edited) = row;
    Ok(AssetRevisionInfo {
        number: to_u32(number, "Fassungsnummer")?,
        based_on: based_on.map(|b| to_u32(b, "based_on")).transpose()?,
        current: status == "current",
        mime,
        encoding: parse_encoding(&encoding)?,
        bytes: u64::try_from(bytes).map_err(|_| broken(format!("Bildgröße außerhalb des Bereichs: {bytes}")))?,
        sha256,
        caption,
        alt,
        author,
        note,
        created,
        edited,
    })
}

async fn work_exists(conn: &mut SqliteConnection, work: WorkId) -> Result<(), StoreError> {
    let found: Option<i64> =
        sqlx::query_scalar("SELECT id FROM work WHERE id = ?").bind(work).fetch_optional(&mut *conn).await?;
    found.map(|_| ()).ok_or(StoreError::UnknownWork(work))
}

/// id des Bilds; unbekanntes Werk → `UnknownWork`, unbekannter Name → `UnknownAsset`.
async fn asset_id(conn: &mut SqliteConnection, work: WorkId, name: &str) -> Result<i64, StoreError> {
    let id: Option<i64> = sqlx::query_scalar("SELECT id FROM asset WHERE work_id = ? AND name = ?")
        .bind(work)
        .bind(name)
        .fetch_optional(&mut *conn)
        .await?;
    match id {
        Some(id) => Ok(id),
        None => {
            work_exists(conn, work).await?;
            Err(StoreError::UnknownAsset { work, name: name.to_owned() })
        }
    }
}

async fn revisions_of(conn: &mut SqliteConnection, asset: i64) -> Result<Vec<AssetRevisionInfo>, StoreError> {
    let rows: Vec<InfoRow> =
        sqlx::query_as(&format!("SELECT {INFO_COLUMNS} FROM asset_revision WHERE asset_id = ? ORDER BY number"))
            .bind(asset)
            .fetch_all(&mut *conn)
            .await?;
    rows.into_iter().map(info_of).collect()
}

/// Die aktuelle Fassung; fehlt sie, ist eine Regel verletzt.
async fn current_of(conn: &mut SqliteConnection, asset: i64, name: &str) -> Result<AssetRevisionInfo, StoreError> {
    let row: Option<InfoRow> =
        sqlx::query_as(&format!("SELECT {INFO_COLUMNS} FROM asset_revision WHERE asset_id = ? AND status = 'current'"))
            .bind(asset)
            .fetch_optional(&mut *conn)
            .await?;
    row.map(info_of).transpose()?.ok_or_else(|| broken(format!("Bild {name}: keine aktuelle Fassung")))
}

/// Fassung `rev` (Angaben); unbekannt → `Asset` mit den vorhandenen Nummern.
async fn revision_of(
    conn: &mut SqliteConnection,
    asset: i64,
    name: &str,
    rev: u32,
) -> Result<AssetRevisionInfo, StoreError> {
    let all = revisions_of(conn, asset).await?;
    let numbers: Vec<String> = all.iter().map(|r| r.number.to_string()).collect();
    all.into_iter().find(|r| r.number == rev).ok_or_else(|| {
        StoreError::Asset(format!("Bild {name}: Fassung {rev} gibt es nicht; vorhanden: {}", numbers.join(", ")))
    })
}

async fn data_of(conn: &mut SqliteConnection, asset: i64, name: &str, rev: u32) -> Result<AssetData, StoreError> {
    let row: Option<(String, String, String)> =
        sqlx::query_as("SELECT mime, encoding, content FROM asset_revision WHERE asset_id = ? AND number = ?")
            .bind(asset)
            .bind(i64::from(rev))
            .fetch_optional(&mut *conn)
            .await?;
    let Some((mime, encoding, content)) = row else {
        revision_of(conn, asset, name, rev).await?;
        return Err(broken(format!("Bild {name}: Fassung {rev} ohne Inhalt")));
    };
    Ok(AssetData { name: name.to_owned(), rev, mime, encoding: parse_encoding(&encoding)?, content })
}

async fn paths_of(conn: &mut SqliteConnection, asset: i64) -> Result<Vec<String>, StoreError> {
    Ok(sqlx::query_scalar("SELECT path FROM asset_path WHERE asset_id = ? ORDER BY path")
        .bind(asset)
        .fetch_all(&mut *conn)
        .await?)
}

// ---- Schreiben ---------------------------------------------------------------------------------

/// Freier Name: `base`, sonst `base-2`, `base-3` … (vor der Endung).
pub(crate) async fn free_name(conn: &mut SqliteConnection, work: WorkId, base: &str) -> Result<String, StoreError> {
    let mut candidate = base.to_owned();
    let mut n = 2;
    loop {
        let taken: Option<i64> = sqlx::query_scalar("SELECT 1 FROM asset WHERE work_id = ? AND name = ?")
            .bind(work)
            .bind(&candidate)
            .fetch_optional(&mut *conn)
            .await?;
        if taken.is_none() {
            return Ok(candidate);
        }
        candidate = numbered_name(base, n);
        n += 1;
    }
}

/// Neue Fassung einfügen (Nummer `asset.last_rev + 1`), Status wie angegeben. Ergebnis: Nummer.
#[allow(clippy::too_many_arguments)]
async fn insert_revision(
    conn: &mut SqliteConnection,
    asset: i64,
    based_on: Option<u32>,
    content: &Content,
    caption: &str,
    alt: &str,
    author: &str,
    note: &str,
) -> Result<u32, StoreError> {
    let last: i64 =
        sqlx::query_scalar("SELECT last_rev FROM asset WHERE id = ?").bind(asset).fetch_one(&mut *conn).await?;
    let number = last + 1;
    sqlx::query(
        "INSERT INTO asset_revision (asset_id, number, based_on, mime, encoding, content, bytes, sha256, caption, alt, \
         author_name, note, status) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'current')",
    )
    .bind(asset)
    .bind(number)
    .bind(based_on.map(i64::from))
    .bind(content.mime)
    .bind(content.encoding.as_str())
    .bind(&content.content)
    .bind(content.bytes)
    .bind(&content.sha256)
    .bind(caption)
    .bind(alt)
    .bind(author)
    .bind(note)
    .execute(&mut *conn)
    .await?;
    to_u32(number, "Fassungsnummer")
}

/// Bild, dem ein Pfad des Werks gehört (verglichen in Normalform, [`normalize_asset_path`]; ein
/// wörtlich gleicher Pfad geht vor): `(id, Name)`.
pub(crate) async fn path_owner(
    conn: &mut SqliteConnection,
    work: WorkId,
    path: &str,
) -> Result<Option<(i64, String)>, StoreError> {
    let Some(wanted) = normalize_asset_path(path) else { return Ok(None) };
    let rows: Vec<(String, i64, String)> = sqlx::query_as(
        "SELECT p.path, a.id, a.name FROM asset_path p JOIN asset a ON a.id = p.asset_id WHERE p.work_id = ? \
         ORDER BY p.path",
    )
    .bind(work)
    .fetch_all(&mut *conn)
    .await?;
    let exact = rows.iter().find(|(stored, ..)| stored.trim() == path.trim());
    let found =
        exact.or_else(|| rows.iter().find(|(stored, ..)| normalize_asset_path(stored).as_deref() == Some(&wanted)));
    Ok(found.map(|(_, id, name)| (*id, name.clone())))
}

/// Pfade an ein Bild hängen; gehört einer (in Normalform) schon einem anderen Bild des Werks →
/// `Asset`. Eine andere Schreibweise desselben Bilds kommt wie bisher dazu.
async fn insert_paths(
    conn: &mut SqliteConnection,
    work: WorkId,
    asset: i64,
    paths: &[String],
) -> Result<(), StoreError> {
    for path in paths {
        let exact: Option<i64> = sqlx::query_scalar("SELECT asset_id FROM asset_path WHERE work_id = ? AND path = ?")
            .bind(work)
            .bind(path)
            .fetch_optional(&mut *conn)
            .await?;
        if exact == Some(asset) {
            continue;
        }
        match path_owner(conn, work, path).await? {
            // Dasselbe Bild in anderer Schreibweise (`/bilder/a.png` und `bilder/a.png`): beide stehen da.
            Some((id, _)) if id == asset => {}
            Some((_, other)) => {
                return Err(StoreError::Asset(format!("Der Pfad „{path}“ gehört schon zum Bild {other}.")));
            }
            None => {}
        }
        sqlx::query("INSERT INTO asset_path (work_id, path, asset_id) VALUES (?, ?, ?)")
            .bind(work)
            .bind(path)
            .bind(asset)
            .execute(&mut *conn)
            .await?;
    }
    Ok(())
}

/// Neues Bild mit Fassung 1 (aktuell). Name: eigener (geprüft, vergeben → `Refused`) oder aus dem
/// Dateinamen (Kollision → `-2`, `-3` …).
async fn add(
    conn: &mut SqliteConnection,
    work: WorkId,
    new: &NewAsset<'_>,
    human: &str,
) -> Result<AssetChange, StoreError> {
    work_exists(conn, work).await?;
    let content = prepare(new.file_name, new.bytes, new.as_base64)?;
    let paths = check_paths(new.path)?;
    let name = match new.name.map(str::trim).filter(|n| !n.is_empty()) {
        Some(name) => {
            if !valid_asset_name(name) {
                return Err(StoreError::Asset(format!(
                    "Ungültiger Bildname „{name}“: nur a–z, 0–9, '.', '_' und '-', höchstens 80 Zeichen, mit Endung \
                     (.png, .jpg, .jpeg, .gif, .webp, .svg)."
                )));
            }
            if !name_fits(name, content.mime) {
                return Err(StoreError::Asset(format!(
                    "Die Endung von „{name}“ passt nicht zum Typ {}.",
                    content.mime
                )));
            }
            let taken: Option<i64> = sqlx::query_scalar("SELECT 1 FROM asset WHERE work_id = ? AND name = ?")
                .bind(work)
                .bind(name)
                .fetch_optional(&mut *conn)
                .await?;
            if taken.is_some() {
                return Err(StoreError::Refused("Ein Bild mit diesem Namen gibt es in diesem Werk schon."));
            }
            name.to_owned()
        }
        None => free_name(conn, work, &asset_name_from_file(new.file_name, content.mime)).await?,
    };
    let asset: i64 = sqlx::query_scalar("INSERT INTO asset (work_id, name) VALUES (?, ?) RETURNING id")
        .bind(work)
        .bind(&name)
        .fetch_one(&mut *conn)
        .await?;
    let rev = insert_revision(conn, asset, None, &content, new.caption, new.alt, human, new.note).await?;
    insert_paths(conn, work, asset, &paths).await?;
    let mut detail = format!(
        "Typ: {}\nGröße: {}\nKodierung: {}\n",
        content.mime,
        size_text(content.bytes as u64),
        content.encoding.as_str()
    );
    if !paths.is_empty() {
        detail.push_str(&format!("Pfade: {}\n", paths.join(", ")));
    }
    Ok(AssetChange { work, name, rev, kind: AssetChangeKind::Added, before: None, removed: None, detail })
}

/// Aktuelle auf `superseded`, `rev` auf `current`.
async fn switch(conn: &mut SqliteConnection, asset: i64, rev: u32) -> Result<(), StoreError> {
    sqlx::query("UPDATE asset_revision SET status = 'superseded' WHERE asset_id = ? AND status = 'current'")
        .bind(asset)
        .execute(&mut *conn)
        .await?;
    sqlx::query("UPDATE asset_revision SET status = 'current' WHERE asset_id = ? AND number = ?")
        .bind(asset)
        .bind(i64::from(rev))
        .execute(&mut *conn)
        .await?;
    Ok(())
}

/// Alle Bildzeilen eines Bilds löschen (Pfade, Fassungen, das Bild). Elternverweise werden vorher
/// gelöst und die aktuelle Fassung auf `superseded` gesetzt, damit die Trigger das Löschen erlauben.
async fn delete_rows(conn: &mut SqliteConnection, asset: i64) -> Result<(), StoreError> {
    sqlx::query("DELETE FROM asset_path WHERE asset_id = ?").bind(asset).execute(&mut *conn).await?;
    sqlx::query("UPDATE asset_revision SET based_on = NULL WHERE asset_id = ? AND based_on IS NOT NULL")
        .bind(asset)
        .execute(&mut *conn)
        .await?;
    sqlx::query("UPDATE asset_revision SET status = 'superseded' WHERE asset_id = ? AND status = 'current'")
        .bind(asset)
        .execute(&mut *conn)
        .await?;
    sqlx::query("DELETE FROM asset_revision WHERE asset_id = ?").bind(asset).execute(&mut *conn).await?;
    sqlx::query("DELETE FROM asset WHERE id = ?").bind(asset).execute(&mut *conn).await?;
    Ok(())
}

/// Alle Bilder eines Werks löschen (für `works::delete_work_rows`).
pub(crate) async fn delete_work_assets(conn: &mut SqliteConnection, work: WorkId) -> Result<(), StoreError> {
    let assets: Vec<i64> =
        sqlx::query_scalar("SELECT id FROM asset WHERE work_id = ?").bind(work).fetch_all(&mut *conn).await?;
    for asset in assets {
        delete_rows(conn, asset).await?;
    }
    // Pfade ohne Bild kann es wegen des Fremdschlüssels nicht geben; sicher ist sicher.
    sqlx::query("DELETE FROM asset_path WHERE work_id = ?").bind(work).execute(&mut *conn).await?;
    Ok(())
}

/// Wert „vorher → nachher“ in einer Zeile (leer → „–“).
fn shown(value: &str) -> String {
    let joined = value.split(['\r', '\n']).map(str::trim).filter(|p| !p.is_empty()).collect::<Vec<_>>().join(" ");
    if joined.is_empty() { "–".to_owned() } else { joined }
}

/// Spiegel-Dateien: je aktuelles Bild `bilder/<name>` (SVG-Text) bzw. `bilder/<name>.b64` (Base64 in
/// Zeilen zu 76 Zeichen, jede mit `\n`), nach Namen.
pub(crate) async fn mirror_files(conn: &mut SqliteConnection, work: WorkId) -> Result<Vec<MirrorFile>, StoreError> {
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT a.name, r.encoding, r.content FROM asset a JOIN asset_revision r ON r.asset_id = a.id \
         WHERE a.work_id = ? AND r.status = 'current' ORDER BY a.name",
    )
    .bind(work)
    .fetch_all(&mut *conn)
    .await?;
    let mut files = Vec::with_capacity(rows.len());
    for (name, encoding, content) in rows {
        let file = match parse_encoding(&encoding)? {
            AssetEncoding::Svg => MirrorFile { path: format!("bilder/{name}"), contents: content },
            AssetEncoding::Base64 => MirrorFile { path: format!("bilder/{name}.b64"), contents: wrap_lines(&content) },
        };
        files.push(file);
    }
    Ok(files)
}

impl Store {
    /// Alle Bilder des Werks nach Namen, mit den Angaben der aktuellen Fassung. Unbekanntes Werk →
    /// `UnknownWork`.
    pub async fn asset_list(&self, work: WorkId) -> Result<Vec<AssetSummary>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        work_exists(&mut conn, work).await?;
        let assets: Vec<(i64, String, String)> =
            sqlx::query_as("SELECT id, name, created FROM asset WHERE work_id = ? ORDER BY name")
                .bind(work)
                .fetch_all(&mut *conn)
                .await?;
        let mut out = Vec::with_capacity(assets.len());
        for (id, name, created) in assets {
            let revisions = revisions_of(&mut conn, id).await?;
            let current = revisions
                .iter()
                .find(|r| r.current)
                .cloned()
                .ok_or_else(|| broken(format!("Bild {name}: keine aktuelle Fassung")))?;
            let updated = revisions
                .iter()
                .flat_map(|r| std::iter::once(r.created.clone()).chain(r.edited.clone()))
                .chain(std::iter::once(created))
                .max()
                .unwrap_or_default();
            out.push(AssetSummary {
                paths: paths_of(&mut conn, id).await?,
                name,
                mime: current.mime,
                encoding: current.encoding,
                bytes: current.bytes,
                current: current.number,
                revisions: revisions.len(),
                caption: current.caption,
                alt: current.alt,
                updated,
            });
        }
        Ok(out)
    }

    /// Bilder suchen (Runde 11, Seite „Bilder“ `?q=`, RPC `asset.search`): alle Bilder, deren Name,
    /// Bildunterschrift oder Alternativtext der **aktuellen** Fassung `query` enthält – wie
    /// `LIKE '%query%'`, aber ohne Groß/Klein auch für Umlaute (SQLite faltet in `LIKE` nur ASCII) und
    /// mit `%` und `_` als gewöhnlichen Zeichen. Leere Suche (nur Leerraum) = alle Bilder. Reihenfolge
    /// wie [`Store::asset_list`] (nach Namen). Unbekanntes Werk → `UnknownWork`.
    pub async fn asset_search(&self, work: WorkId, query: &str) -> Result<Vec<AssetSummary>, StoreError> {
        let all = self.asset_list(work).await?;
        let needle = query.trim().to_lowercase();
        if needle.is_empty() {
            return Ok(all);
        }
        Ok(all
            .into_iter()
            .filter(|a| [&a.name, &a.caption, &a.alt].iter().any(|field| field.to_lowercase().contains(&needle)))
            .collect())
    }

    /// Alle Fassungen eines Bilds nach Nummer (ohne Inhalt).
    pub async fn asset_revisions(&self, work: WorkId, name: &str) -> Result<Vec<AssetRevisionInfo>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        let asset = asset_id(&mut conn, work, name).await?;
        revisions_of(&mut conn, asset).await
    }

    /// Inhalt einer Fassung (`None` = die aktuelle).
    pub async fn asset_data(&self, work: WorkId, name: &str, rev: Option<u32>) -> Result<AssetData, StoreError> {
        let mut conn = self.pool.acquire().await?;
        let asset = asset_id(&mut conn, work, name).await?;
        let rev = match rev {
            Some(rev) => rev,
            None => current_of(&mut conn, asset, name).await?.number,
        };
        data_of(&mut conn, asset, name, rev).await
    }

    /// Aktuelle Fassung des Bilds, das unter `path` in Vorlage/Text steht; verglichen in Normalform
    /// ([`normalize_asset_path`]: `/bilder/a.png`, `bilder/a.png` und `./bilder/a.png` sind derselbe
    /// Pfad). Unbekannt oder ohne Normalform (`..`) → `None`.
    pub async fn asset_by_path(&self, work: WorkId, path: &str) -> Result<Option<AssetData>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        work_exists(&mut conn, work).await?;
        let Some((asset, name)) = path_owner(&mut conn, work, path).await? else { return Ok(None) };
        let rev = current_of(&mut conn, asset, &name).await?.number;
        data_of(&mut conn, asset, &name, rev).await.map(Some)
    }

    /// Neues Bild anlegen (Fassung 1, aktuell, Autor `human`). Zu groß, kein unterstützter Typ,
    /// ungültiger Name oder Pfad → `Asset`; Name vergeben → `Refused`.
    pub async fn add_asset(&self, work: WorkId, new: &NewAsset<'_>, human: &str) -> Result<AssetChange, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let change = add(&mut tx, work, new, human).await?;
        tx.commit().await?;
        Ok(change)
    }

    /// Bild ersetzen: neue Fassung, sofort aktuell, `based_on` = die bisher aktuelle; Bildunterschrift
    /// und Alternativtext gehen mit. Gleiche Bytes (SHA-256) → `Refused("Das Bild ist unverändert.")`;
    /// anderer Typ als die Endung des Namens → `Asset` (mit Typwechsel: [`Store::replace_asset_renaming`]).
    #[allow(clippy::too_many_arguments)]
    pub async fn replace_asset(
        &self,
        work: WorkId,
        name: &str,
        file_name: &str,
        bytes: &[u8],
        as_base64: bool,
        note: &str,
        human: &str,
    ) -> Result<AssetChange, StoreError> {
        self.replace_asset_renaming(work, name, file_name, bytes, as_base64, note, human, false).await
    }

    /// Wie [`Store::replace_asset`], dazu der **Typwechsel** (Runde 21c): Ist der neue Inhalt ein anderes Format
    /// als die Endung des Namens, geht das nur mit `rename` (sonst `Asset` mit Hinweis). Dann bekommt das Bild in
    /// derselben Transaktion den Namen mit der Vorgabe-Endung des neuen Formats (`haus.svg` → `haus.png`); ist
    /// dieser Name im Werk schon vergeben, den nächsten freien (`haus-2.png`). Alle Pfade bleiben dem Bild
    /// zugeordnet (sie hängen an der id, nicht am Namen), der Text bleibt byte-gleich – Vorlage und Text finden am
    /// alten Pfad das neue Bild. Offene Anträge zum Bild ziehen den Namen mit (sie veralten beim Bestätigen ohnehin,
    /// weil sich die aktuelle Fassung geändert hat). `rename` bei gleichem Format ändert nichts am Namen.
    /// `AssetChange::name` ist der neue Name, `detail` nennt den alten.
    #[allow(clippy::too_many_arguments)]
    pub async fn replace_asset_renaming(
        &self,
        work: WorkId,
        name: &str,
        file_name: &str,
        bytes: &[u8],
        as_base64: bool,
        note: &str,
        human: &str,
        rename: bool,
    ) -> Result<AssetChange, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let asset = asset_id(&mut tx, work, name).await?;
        let before = current_of(&mut tx, asset, name).await?;
        let content = prepare(file_name, bytes, as_base64)?;
        let mut new_name = name.to_owned();
        if !name_fits(name, content.mime) {
            if !rename {
                return Err(StoreError::Asset(format!(
                    "„{file_name}“ ist {}, das passt nicht zur Endung von {name}; mit „Endung anpassen“ (rename) \
                     bekommt das Bild die neue Endung und behält seine Pfade, sonst als neues Bild hinzufügen.",
                    content.mime
                )));
            }
            let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);
            let ext = extensions(content.mime).first().copied().unwrap_or("bin");
            new_name = free_name(&mut tx, work, &format!("{stem}.{ext}")).await?;
        }
        if content.sha256 == before.sha256 && content.encoding == before.encoding {
            return Err(StoreError::Refused("Das Bild ist unverändert."));
        }
        sqlx::query("UPDATE asset_revision SET status = 'superseded' WHERE asset_id = ? AND status = 'current'")
            .bind(asset)
            .execute(&mut *tx)
            .await?;
        let rev =
            insert_revision(&mut tx, asset, Some(before.number), &content, &before.caption, &before.alt, human, note)
                .await?;
        let mut renamed = String::new();
        if new_name != name {
            sqlx::query("UPDATE asset SET name = ? WHERE id = ?").bind(&new_name).bind(asset).execute(&mut *tx).await?;
            sqlx::query(
                "UPDATE asset_request SET asset_name = ? WHERE work_id = ? AND asset_name = ? AND decided IS NULL",
            )
            .bind(&new_name)
            .bind(work)
            .bind(name)
            .execute(&mut *tx)
            .await?;
            let paths = paths_of(&mut tx, asset).await?;
            let paths = if paths.is_empty() { "keine".to_owned() } else { paths.join(", ") };
            renamed = format!("Name: {name} → {new_name} (Endung angepasst)\nPfade (bleiben beim Bild): {paths}\n");
        }
        tx.commit().await?;
        let detail = format!(
            "Fassung {rev} ersetzt Fassung {}.\nTyp: {} → {}\nGröße: {} → {}\nKodierung: {}\n{renamed}",
            before.number,
            before.mime,
            content.mime,
            size_text(before.bytes),
            size_text(content.bytes as u64),
            content.encoding.as_str()
        );
        Ok(AssetChange {
            work,
            name: new_name,
            rev,
            kind: AssetChangeKind::Replaced,
            before: Some(before),
            removed: None,
            detail,
        })
    }

    /// Altbestand (Runde 21c): Pfade, die in Normalform ([`normalize_asset_path`]) gleich sind, aber zu
    /// **verschiedenen** Bildern gehören (`/x.png` → Bild A, `x.png` → Bild B). Neu anlegen lässt der Store so
    /// etwas seit Runde 7 nicht mehr zu; ältere Datenbanken können es noch enthalten. Je Normalform eine
    /// [`PathConflict`] mit allen Schreibweisen und ihrem Bild, nach Normalform sortiert.
    pub async fn asset_path_conflicts(&self, work: WorkId) -> Result<Vec<PathConflict>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        work_exists(&mut conn, work).await?;
        path_conflicts(&mut conn, work).await
    }

    /// Kollision auflösen (nur Mensch, Runde 21c): Alle Schreibweisen der Normalform `normal` gehören danach dem
    /// Bild `asset`; die anderen Bilder verlieren diese Pfade (ihre übrigen Pfade und Fassungen bleiben). Der Text
    /// bleibt byte-gleich; so finden Vorlage, Text, Satz und Export unter jeder Schreibweise dasselbe Bild. Keine
    /// Kollision unter `normal` → `Refused`; `asset` gehört keine der Schreibweisen → `Asset`. Ergebnis für
    /// **einen** Spiegel-Commit (`Updated` am gewählten Bild, `detail` nennt die umgehängten Pfade).
    pub async fn resolve_asset_path(
        &self,
        work: WorkId,
        normal: &str,
        asset: &str,
        human: &str,
    ) -> Result<AssetChange, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        work_exists(&mut tx, work).await?;
        let wanted = normalize_asset_path(normal).unwrap_or_else(|| normal.trim().to_owned());
        let Some(conflict) = path_conflicts(&mut tx, work).await?.into_iter().find(|c| c.normal == wanted) else {
            return Err(StoreError::Refused("Unter diesem Pfad gibt es keine Kollision (mehr)."));
        };
        if !conflict.paths.iter().any(|p| p.asset == asset) {
            return Err(StoreError::Asset(format!(
                "Das Bild „{asset}“ hat keinen der Pfade von „{wanted}“; zur Wahl stehen: {}.",
                conflict
                    .paths
                    .iter()
                    .map(|p| p.asset.as_str())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
        let id = asset_id(&mut tx, work, asset).await?;
        let before = current_of(&mut tx, id, asset).await?;
        let mut moved = Vec::new();
        for owner in conflict.paths.iter().filter(|p| p.asset != asset) {
            sqlx::query("UPDATE asset_path SET asset_id = ? WHERE work_id = ? AND path = ?")
                .bind(id)
                .bind(work)
                .bind(&owner.path)
                .execute(&mut *tx)
                .await?;
            moved.push(format!("„{}“ (vorher {})", owner.path, owner.asset));
        }
        tx.commit().await?;
        let detail = format!(
            "Pfad-Kollision „{wanted}“ aufgelöst: alle Schreibweisen gehören jetzt zu {asset}.\nUmgehängt: {}\n\
             Entschieden von: {human}\n",
            moved.join(", ")
        );
        Ok(AssetChange {
            work,
            name: asset.to_owned(),
            rev: before.number,
            kind: AssetChangeKind::Updated,
            before: Some(before),
            removed: None,
            detail,
        })
    }

    /// Markierung „aktuell“ auf Fassung `rev` umschalten. Schon aktuell → `Refused`.
    pub async fn set_current_asset(&self, work: WorkId, name: &str, rev: u32) -> Result<AssetChange, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let asset = asset_id(&mut tx, work, name).await?;
        let target = revision_of(&mut tx, asset, name, rev).await?;
        if target.current {
            return Err(StoreError::Refused("Diese Fassung ist schon aktuell."));
        }
        let before = current_of(&mut tx, asset, name).await?;
        switch(&mut tx, asset, rev).await?;
        tx.commit().await?;
        let detail = format!("Vorher aktuell: Fassung {}\n", before.number);
        Ok(AssetChange {
            work,
            name: name.to_owned(),
            rev,
            kind: AssetChangeKind::Switched,
            before: Some(before),
            removed: None,
            detail,
        })
    }

    /// Bildunterschrift, Alternativtext und/oder Notiz einer Fassung ändern (nur Mensch), setzt
    /// `edited`. Der Inhalt bleibt (Trigger). Nichts geändert → `Refused("Die Fassung ist unverändert.")`.
    #[allow(clippy::too_many_arguments)]
    pub async fn update_asset_revision(
        &self,
        work: WorkId,
        name: &str,
        rev: u32,
        caption: Option<&str>,
        alt: Option<&str>,
        note: Option<&str>,
        human: &str,
    ) -> Result<AssetChange, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let asset = asset_id(&mut tx, work, name).await?;
        let before = revision_of(&mut tx, asset, name, rev).await?;
        let caption = caption.map(|c| c.replace("\r\n", "\n")).unwrap_or_else(|| before.caption.clone());
        let alt = alt.map(|a| a.replace("\r\n", "\n")).unwrap_or_else(|| before.alt.clone());
        let note = note.map(|n| n.replace("\r\n", "\n")).unwrap_or_else(|| before.note.clone());
        if caption == before.caption && alt == before.alt && note == before.note {
            return Err(StoreError::Refused("Die Fassung ist unverändert."));
        }
        sqlx::query(
            "UPDATE asset_revision SET caption = ?, alt = ?, note = ?, edited = strftime('%Y-%m-%dT%H:%M:%fZ','now') \
             WHERE asset_id = ? AND number = ?",
        )
        .bind(&caption)
        .bind(&alt)
        .bind(&note)
        .bind(asset)
        .bind(i64::from(rev))
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        let mut detail = String::new();
        for (label, old, new) in [
            ("Bildunterschrift", &before.caption, &caption),
            ("Alternativtext", &before.alt, &alt),
            ("Notiz", &before.note, &note),
        ] {
            detail.push_str(&format!("{label} vorher: {}\n{label} nachher: {}\n", shown(old), shown(new)));
        }
        detail.push_str(&format!("Geändert von: {human}\n"));
        Ok(AssetChange {
            work,
            name: name.to_owned(),
            rev,
            kind: AssetChangeKind::Updated,
            before: Some(before),
            removed: None,
            detail,
        })
    }

    /// Eine Fassung löschen (nur Mensch, nicht die aktuelle → `Refused`); Kinder hängen danach am
    /// Elternteil. `removed` trägt den gelöschten Inhalt.
    pub async fn delete_asset_revision(
        &self,
        work: WorkId,
        name: &str,
        rev: u32,
        human: &str,
    ) -> Result<AssetChange, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let asset = asset_id(&mut tx, work, name).await?;
        let before = revision_of(&mut tx, asset, name, rev).await?;
        if before.current {
            return Err(StoreError::Refused(
                "Die aktuelle Fassung lässt sich nicht löschen; erst auf eine andere umschalten.",
            ));
        }
        let removed = data_of(&mut tx, asset, name, rev).await?;
        let children: Vec<i64> =
            sqlx::query_scalar("SELECT number FROM asset_revision WHERE asset_id = ? AND based_on = ? ORDER BY number")
                .bind(asset)
                .bind(i64::from(rev))
                .fetch_all(&mut *tx)
                .await?;
        sqlx::query("DELETE FROM asset_revision WHERE asset_id = ? AND number = ?")
            .bind(asset)
            .bind(i64::from(rev))
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        let parent = before.based_on.map_or_else(|| "keins (Wurzel)".to_owned(), |b| format!("Fassung {b}"));
        let moved = if children.is_empty() {
            "keine".to_owned()
        } else {
            let list: Vec<String> = children.iter().map(i64::to_string).collect();
            format!("{} → {parent}", list.join(", "))
        };
        let detail = format!("Elternteil: {parent}\nUmgehängt: {moved}\nGelöscht von: {human}\n");
        Ok(AssetChange {
            work,
            name: name.to_owned(),
            rev,
            kind: AssetChangeKind::Deleted,
            before: Some(before),
            removed: Some(removed),
            detail,
        })
    }

    /// Das ganze Bild samt Fassungen und Pfaden löschen (nur Mensch). `removed` trägt den Inhalt der
    /// zuletzt aktuellen Fassung (alle Fassungen waren einmal aktuell und stehen in der Git-Historie).
    pub async fn delete_asset(&self, work: WorkId, name: &str, human: &str) -> Result<AssetChange, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let asset = asset_id(&mut tx, work, name).await?;
        let before = current_of(&mut tx, asset, name).await?;
        let removed = data_of(&mut tx, asset, name, before.number).await?;
        let count = revisions_of(&mut tx, asset).await?.len();
        let paths = paths_of(&mut tx, asset).await?;
        delete_rows(&mut tx, asset).await?;
        tx.commit().await?;
        let paths = if paths.is_empty() { "keine".to_owned() } else { paths.join(", ") };
        let detail = format!("Fassungen gelöscht: {count}\nPfade: {paths}\nGelöscht von: {human}\n");
        Ok(AssetChange {
            work,
            name: name.to_owned(),
            rev: before.number,
            kind: AssetChangeKind::Removed,
            before: Some(before),
            removed: Some(removed),
            detail,
        })
    }

    /// Bild in einen Abschnitt einfügen (Runde 18c): eigene Fassung des Menschen `author` auf der Basis
    /// `based_on` mit dem fertigen Text `body` speichern und – falls das Bild `asset` den Pfad `path` (in
    /// Normalform verglichen) noch nicht hat – ihm diesen Pfad geben, **beides in einer Transaktion**: Scheitert
    /// das Speichern (Überschriften passen nicht, unbekannte Fassung …) oder ist der Text unverändert, bleibt auch
    /// kein Pfad zurück. Ein Pfad, der einem anderen Bild gehört, oder ein ungültiger → `Asset`; unbekanntes Bild →
    /// `UnknownAsset`. Ergebnis: die Änderung des Abschnitts (für den einen Spiegel-Commit; `None` = Text
    /// unverändert, nichts gespeichert) und ob der Pfad neu vergeben wurde.
    #[allow(clippy::too_many_arguments)]
    pub async fn save_human_with_asset_path(
        &self,
        work: WorkId,
        node: usize,
        body: &str,
        author: &str,
        based_on: u32,
        asset: &str,
        path: &str,
    ) -> Result<(Option<crate::Change>, bool), StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let id = asset_id(&mut tx, work, asset).await?;
        let known = paths_of(&mut tx, id).await?;
        let wanted = normalize_asset_path(path);
        let added = !known.iter().any(|p| p == path || (wanted.is_some() && normalize_asset_path(p) == wanted));
        if added {
            let checked = check_paths([path])?;
            if checked.len() != 1 {
                return Err(StoreError::Asset("Ein Bild im Text braucht genau einen Pfad.".to_owned()));
            }
            insert_paths(&mut tx, work, id, &checked).await?;
        }
        let change = crate::write::save_human(&mut tx, work, node, body, author, Some(based_on)).await?;
        if change.is_none() {
            // Text unverändert: nichts speichern, auch keinen Pfad (die Transaktion rollt beim Verwerfen zurück).
            return Ok((None, false));
        }
        crate::outline::check_current_heading(&mut tx, work, node).await?;
        tx.commit().await?;
        Ok((change, added))
    }

    /// Pfade des Bilds ersetzen (je Eintrag auch mehrere Zeilen; leer = keine). Ein Pfad eines
    /// anderen Bilds oder ein ungültiger → `Asset`; unverändert → `Refused`.
    pub async fn set_asset_paths(&self, work: WorkId, name: &str, paths: &[String]) -> Result<AssetChange, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let asset = asset_id(&mut tx, work, name).await?;
        let paths = check_paths(paths.iter().map(String::as_str))?;
        let old = paths_of(&mut tx, asset).await?;
        if old == paths {
            return Err(StoreError::Refused("Die Pfade sind unverändert."));
        }
        let before = current_of(&mut tx, asset, name).await?;
        sqlx::query("DELETE FROM asset_path WHERE asset_id = ?").bind(asset).execute(&mut *tx).await?;
        insert_paths(&mut tx, work, asset, &paths).await?;
        tx.commit().await?;
        let list = |p: &[String]| if p.is_empty() { "keine".to_owned() } else { p.join(", ") };
        let detail = format!("Pfade vorher: {}\nPfade nachher: {}\n", list(&old), list(&paths));
        Ok(AssetChange {
            work,
            name: name.to_owned(),
            rev: before.number,
            kind: AssetChangeKind::Updated,
            before: Some(before),
            removed: None,
            detail,
        })
    }

    /// Dateien für den Export: je Pfad (in Normalform, [`normalize_asset_path`]: ohne führendes `/`
    /// und `./`, ohne Doppelte, sortiert) die Bytes der aktuellen Fassung.
    pub async fn export_assets(&self, work: WorkId) -> Result<Vec<ExportedFile>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        work_exists(&mut conn, work).await?;
        let rows: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT p.path, r.encoding, r.content FROM asset_path p \
             JOIN asset_revision r ON r.asset_id = p.asset_id AND r.status = 'current' \
             WHERE p.work_id = ? ORDER BY p.path",
        )
        .bind(work)
        .fetch_all(&mut *conn)
        .await?;
        let mut seen = BTreeSet::new();
        let mut files = Vec::with_capacity(rows.len());
        for (path, encoding, content) in rows {
            let path = normalize_asset_path(&path).unwrap_or_else(|| path.trim_start_matches('/').to_owned());
            if !seen.insert(path.clone()) {
                continue;
            }
            let bytes = match parse_encoding(&encoding)? {
                AssetEncoding::Base64 => decode_base64(&content)?,
                AssetEncoding::Svg => content.into_bytes(),
            };
            files.push(ExportedFile { path, bytes });
        }
        files.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(files)
    }

    /// Typst-Datei und ihre Bilder als neues Werk, alles in **einer** Transaktion: wie
    /// [`Store::import_typst_titled`], dazu je Datei ein Bild (Fassung 1 von `human` mit `note`,
    /// Name aus dem Dateinamen) und je Pfad ein Eintrag in `asset_path`. Gleiche Bytes unter
    /// gleichem Dateinamen werden ein Bild mit mehreren Pfaden. Scheitert ein Bild (Typ, Größe,
    /// Pfad), wird nichts angelegt. Zweite Rückgabe: die angelegten Bildnamen.
    pub async fn import_typst_with_assets(
        &self,
        source: &str,
        file_name: &str,
        human: &str,
        note: &str,
        title: Option<&str>,
        assets: &[ImportAsset],
    ) -> Result<(Imported, Vec<String>), StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let imported = import::import_typst(&mut tx, source, file_name, human, note, title).await?;
        let mut names = Vec::new();
        // (Dateiname, SHA-256) → Bild-id: dieselbe Datei unter zwei Schreibweisen ist ein Bild.
        let mut known: HashMap<(String, String), i64> = HashMap::new();
        for item in assets {
            let path = check_path(&item.path)?;
            let key = (item.file_name.clone(), sha256_hex(&item.bytes));
            let asset = match known.get(&key) {
                Some(&asset) => asset,
                None => {
                    let new = NewAsset {
                        file_name: &item.file_name,
                        name: None,
                        bytes: &item.bytes,
                        as_base64: false,
                        path: None,
                        caption: "",
                        alt: "",
                        note,
                    };
                    let change = add(&mut tx, imported.work, &new, human).await?;
                    let asset = asset_id(&mut tx, imported.work, &change.name).await?;
                    names.push(change.name);
                    known.insert(key, asset);
                    asset
                }
            };
            insert_paths(&mut tx, imported.work, asset, &[path]).await?;
        }
        tx.commit().await?;
        Ok((imported, names))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_bytes_tell_the_image_type() {
        assert_eq!(detect_mime(b"\x89PNG\r\n\x1a\n\0\0"), Some("image/png"), "PNG-Signatur");
        assert_eq!(detect_mime(&[0xFF, 0xD8, 0xFF, 0xE0]), Some("image/jpeg"), "JPEG-Anfang");
        assert_eq!(detect_mime(b"GIF89a...."), Some("image/gif"), "GIF89a");
        assert_eq!(detect_mime(b"RIFF\x10\0\0\0WEBPVP8 "), Some("image/webp"), "RIFF mit WEBP");
        assert_eq!(
            detect_mime(b"\xef\xbb\xbf<?xml version=\"1.0\"?>\n<!-- erfunden -->\n<!DOCTYPE svg>\n<svg xmlns=\"x\"/>"),
            Some("image/svg+xml"),
            "SVG nach BOM, Prolog, Kommentar und DOCTYPE"
        );
        assert_eq!(detect_mime(b"<svgx>"), None, "<svgx ist kein svg-Element");
        assert_eq!(detect_mime(b"<html><svg/></html>"), None, "SVG muss das erste Element sein");
        assert_eq!(detect_mime(b"%PDF-1.7"), None, "PDF ist kein Bild");
        assert_eq!(detect_mime(b"\xff\xfe<\0s\0v\0g\0"), None, "UTF-16 ist kein UTF-8-SVG");
    }

    #[test]
    fn names_follow_the_rule_and_need_a_fitting_extension() {
        for good in ["haus.svg", "a.png", "foto_2-b.jpeg", "x1.webp"] {
            assert!(valid_asset_name(good), "{good} ist gültig");
        }
        let no_extension = "a".repeat(78);
        for bad in ["", "Haus.svg", "-a.png", "a.png-", "haus", "haus.bmp", "ä.png", "a b.png", no_extension.as_str()]
        {
            assert!(!valid_asset_name(bad), "{bad:?} ist ungültig");
        }
        assert!(valid_asset_name(&format!("{}.png", "a".repeat(76))), "80 Zeichen sind erlaubt");
        assert!(!valid_asset_name(&format!("{}.png", "a".repeat(77))), "81 Zeichen nicht");
        assert!(name_fits("foto.jpg", "image/jpeg") && name_fits("foto.jpeg", "image/jpeg"), "jpg und jpeg");
        assert!(!name_fits("foto.png", "image/jpeg"), "PNG-Endung passt nicht zu JPEG");
    }

    #[test]
    fn a_file_name_becomes_a_valid_lowercase_name() {
        assert_eq!(
            asset_name_from_file("Ordner/Mein Haus.SVG", "image/svg+xml"),
            "mein-haus.svg",
            "klein, '-' statt Leerzeichen"
        );
        assert_eq!(asset_name_from_file("Grün & Blau!.png", "image/png"), "gr-n-blau.png", "unzulässige Zeichen → '-'");
        assert_eq!(asset_name_from_file("foto", "image/jpeg"), "foto.jpg", "fehlende Endung kommt dazu");
        assert_eq!(asset_name_from_file("bild.png", "image/gif"), "bild.png.gif", "falsche Endung: Vorgabe dazu");
        assert_eq!(asset_name_from_file("...", "image/png"), "bild.png", "nichts übrig → bild");
        let long = asset_name_from_file(&format!("{}.png", "x".repeat(200)), "image/png");
        assert!(valid_asset_name(&long) && long.len() == 80, "auf 80 Zeichen gekürzt: {long}");
        for name in ["mein-haus.svg", "gr-n-blau.png", "foto.jpg", "bild.png.gif", "bild.png"] {
            assert!(valid_asset_name(name), "{name} erfüllt die Regel");
        }
    }

    #[test]
    fn collisions_get_a_number_before_the_extension() {
        assert_eq!(numbered_name("haus.svg", 2), "haus-2.svg", "-2 vor der Endung");
        let long = format!("{}.png", "a".repeat(76));
        let numbered = numbered_name(&long, 12);
        assert!(numbered.len() <= 80 && numbered.ends_with("-12.png") && valid_asset_name(&numbered), "{numbered}");
    }

    #[test]
    fn base64_round_trips_and_broken_input_is_an_asset_error() {
        let bytes = [0u8, 1, 2, 250, 255];
        let text = encode_base64(&bytes);
        assert_eq!(text, "AAEC+v8=", "Standardalphabet mit '='");
        assert_eq!(decode_base64(&text).unwrap(), bytes, "hin und zurück gleich");
        assert!(matches!(decode_base64("AAEC+v8"), Err(StoreError::Asset(_))), "fehlendes '=' ist kaputt");
        assert!(matches!(decode_base64("AAEC\n+v8="), Err(StoreError::Asset(_))), "Zeilenumbruch ist kaputt");
        assert!(matches!(decode_base64("@@@@"), Err(StoreError::Asset(_))), "fremde Zeichen sind kaputt");
    }

    #[test]
    fn mirror_lines_have_76_characters_each_ending_with_a_newline() {
        let text = "A".repeat(160);
        let wrapped = wrap_lines(&text);
        let lines: Vec<&str> = wrapped.split_inclusive('\n').collect();
        assert_eq!(lines.len(), 3, "160 Zeichen → 76 + 76 + 8");
        assert!(lines.iter().all(|l| l.ends_with('\n')), "jede Zeile endet mit \\n");
        assert_eq!(lines[0].len(), 77, "76 Zeichen und \\n");
        assert_eq!(wrap_lines(""), "", "leer bleibt leer");
    }

    #[test]
    fn the_normal_form_drops_the_root_dots_and_empty_parts_and_refuses_parent_dirs() {
        assert_eq!(normalize_asset_path("/bilder/haus.svg").as_deref(), Some("bilder/haus.svg"), "ohne führendes /");
        assert_eq!(normalize_asset_path(" ./bilder//./a.png ").as_deref(), Some("bilder/a.png"), "./ und // entfallen");
        assert_eq!(normalize_asset_path("bilder/a.png").as_deref(), Some("bilder/a.png"), "Normalform bleibt");
        assert_eq!(normalize_asset_path("../a.png"), None, "hinaus geht es nicht");
        assert_eq!(normalize_asset_path("bilder/../a.png"), None, "auch nicht in der Mitte");
        assert_eq!(normalize_asset_path("./"), None, "ohne Dateiname kein Pfad");
        assert_eq!(normalize_asset_path(""), None, "leer ist kein Pfad");
    }

    #[test]
    fn paths_must_be_plain_relative_or_rooted_file_paths() {
        assert_eq!(check_path(" /bilder/haus.svg ").unwrap(), "/bilder/haus.svg", "getrimmt, führendes / erlaubt");
        assert_eq!(check_path("bilder/haus.svg").unwrap(), "bilder/haus.svg", "relativ erlaubt");
        for bad in ["", "/", "./", "../haus.svg", "a/../b.png", "a\\b.png", "https://x/y.png", "a\nb"] {
            assert!(matches!(check_path(bad), Err(StoreError::Asset(_))), "{bad:?} ist ungültig");
        }
        for (written, normal) in [("./bilder/a.png", "bilder/a.png"), ("a/./b.png", "a/b.png"), ("a//b.png", "a/b.png")]
        {
            assert_eq!(check_path(written).unwrap(), written, "{written:?} ist erlaubt und bleibt, wie er steht");
            assert_eq!(normalize_asset_path(written).as_deref(), Some(normal), "Normalform von {written:?}");
        }
    }
}
