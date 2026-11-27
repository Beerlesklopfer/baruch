// SPDX-License-Identifier: AGPL-3.0-or-later
//! The archive of a Word file: ZIP within the [`Limits`] and the relationships (`*.rels`).
//!
//! The sizes in the header are not trusted: they only serve the early rejection (before unpacking); every entry
//! is read through `take(limit + 1)`, and the sum of what was really unpacked is counted. Names with `..`,
//! leading `/`, backslash or drive are [`DocxError::Unsafe`], as are relationship targets that leave the archive.

use std::collections::HashMap;
use std::io::{Cursor, Read};

use zip::ZipArchive;

use super::xml::{self, Element};
use crate::{DocxError, Limits, ooxml};

/// Beginning of an OLE compound file: old `.doc` or an Office file encrypted with a password.
const OLE_MAGIC: &[u8] = &[0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1];

pub(crate) struct Package<'a> {
    zip: ZipArchive<Cursor<&'a [u8]>>,
    limits: Limits,
    unpacked: u64,
}

fn unsafe_name(name: &str) -> bool {
    name.is_empty()
        || name.starts_with('/')
        || name.contains('\\')
        || name.contains('\0')
        || name.as_bytes().get(1) == Some(&b':')
        || name.split('/').any(|part| part == "..")
}

impl<'a> Package<'a> {
    /// Open and check the archive without unpacking anything.
    pub fn open(bytes: &'a [u8], limits: &Limits) -> Result<Self, DocxError> {
        if bytes.len() as u64 > limits.max_file_bytes {
            return Err(DocxError::Limit(format!("Datei größer als {} Bytes", limits.max_file_bytes)));
        }
        if bytes.starts_with(OLE_MAGIC) {
            return Err(DocxError::NotDocx(
                "altes Word-Format (.doc) oder mit Kennwort verschlüsselt – bitte in Word als .docx ohne Kennwort \
                 speichern"
                    .into(),
            ));
        }
        if !bytes.starts_with(b"PK\x03\x04") {
            return Err(DocxError::NotDocx("kein ZIP-Archiv (eine .docx-Datei beginnt mit „PK“)".into()));
        }
        let mut zip =
            ZipArchive::new(Cursor::new(bytes)).map_err(|e| DocxError::Archive(format!("kein lesbares ZIP ({e})")))?;
        if zip.len() > limits.max_entries {
            return Err(DocxError::Limit(format!("mehr als {} Einträge im Archiv", limits.max_entries)));
        }
        let mut declared: u64 = 0;
        for index in 0..zip.len() {
            let file = zip
                .by_index_raw(index)
                .map_err(|e| DocxError::Archive(format!("Eintrag {index} nicht lesbar ({e})")))?;
            let name = std::str::from_utf8(file.name_raw())
                .map_err(|_| DocxError::Archive(format!("Name des Eintrags {index} ist kein UTF-8")))?
                .to_owned();
            if unsafe_name(&name) {
                return Err(DocxError::Unsafe(format!("Eintrag „{name}“ verlässt den Ordner des Archivs")));
            }
            if file.encrypted() {
                return Err(DocxError::NotDocx(format!("„{name}“ ist verschlüsselt")));
            }
            if file.size() > limits.max_entry_bytes {
                return Err(DocxError::Limit(format!(
                    "Eintrag „{name}“ ist entpackt größer als {} Bytes",
                    limits.max_entry_bytes
                )));
            }
            declared = declared.saturating_add(file.size());
            if declared > limits.max_unpacked_bytes {
                return Err(DocxError::Limit(format!(
                    "das Archiv ist entpackt größer als {} Bytes",
                    limits.max_unpacked_bytes
                )));
            }
        }
        Ok(Package { zip, limits: *limits, unpacked: 0 })
    }

    pub fn has(&self, name: &str) -> bool {
        self.zip.index_for_name(name).is_some()
    }

    /// Unpack one entry (at most `max` bytes); `None` if it does not exist.
    pub fn read(&mut self, name: &str, max: u64) -> Result<Option<Vec<u8>>, DocxError> {
        let Some(index) = self.zip.index_for_name(name) else { return Ok(None) };
        let max = max.min(self.limits.max_entry_bytes);
        let left = self.limits.max_unpacked_bytes.saturating_sub(self.unpacked);
        let file = self.zip.by_index(index).map_err(|e| DocxError::Archive(format!("„{name}“ nicht lesbar ({e})")))?;
        let mut bytes = Vec::new();
        file.take(max.min(left).saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|e| DocxError::Archive(format!("„{name}“ lässt sich nicht entpacken ({e})")))?;
        if bytes.len() as u64 > max {
            return Err(DocxError::Limit(format!("Eintrag „{name}“ ist entpackt größer als {max} Bytes")));
        }
        if bytes.len() as u64 > left {
            return Err(DocxError::Limit(format!(
                "das Archiv ist entpackt größer als {} Bytes",
                self.limits.max_unpacked_bytes
            )));
        }
        self.unpacked += bytes.len() as u64;
        Ok(Some(bytes))
    }

    /// Read one XML part; `None` if it does not exist.
    pub fn xml(&mut self, name: &str) -> Result<Option<Element>, DocxError> {
        let limits = self.limits;
        match self.read(name, limits.max_xml_bytes)? {
            Some(bytes) => xml::parse(name, &bytes, &limits).map(Some),
            None => Ok(None),
        }
    }
}

/// A relationship: type, target (part path in the archive or, with `external`, the address) and whether the
/// target lies outside.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Rel {
    pub kind: String,
    pub target: String,
    pub external: bool,
}

/// Path of the relationship file of a part (`word/document.xml` → `word/_rels/document.xml.rels`).
pub(crate) fn rels_path(part: &str) -> String {
    match part.rsplit_once('/') {
        Some((dir, file)) => format!("{dir}/_rels/{file}.rels"),
        None => format!("_rels/{part}.rels"),
    }
}

/// Resolve the target of a relationship relative to the part `base`; everything that would leave the archive is
/// `Unsafe`.
pub(crate) fn resolve_target(base: &str, target: &str) -> Result<String, DocxError> {
    let leaves = || DocxError::Unsafe(format!("Beziehungsziel „{target}“ liegt außerhalb der Datei"));
    if target.contains("://")
        || target.contains('\\')
        || target.contains('\0')
        || target.to_ascii_lowercase().starts_with("file:")
        || target.as_bytes().get(1) == Some(&b':')
    {
        return Err(leaves());
    }
    let mut parts: Vec<&str> = match target.strip_prefix('/') {
        Some(_) => Vec::new(),
        None => base.rsplit_once('/').map(|(dir, _)| dir.split('/').collect()).unwrap_or_default(),
    };
    for part in target.trim_start_matches('/').split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.pop().is_none() {
                    return Err(leaves());
                }
            }
            p => parts.push(p),
        }
    }
    if parts.is_empty() {
        return Err(leaves());
    }
    Ok(parts.join("/"))
}

/// Relationships of the part `part` (empty if there is no file for it). Targets inside the file are resolved.
pub(crate) fn rels(package: &mut Package<'_>, part: &str) -> Result<HashMap<String, Rel>, DocxError> {
    let mut out = HashMap::new();
    let path = if part.is_empty() { ooxml::PART_ROOT_RELS.to_owned() } else { rels_path(part) };
    let Some(root) = package.xml(&path)? else { return Ok(out) };
    for rel in root.elements().filter(|e| e.name == "Relationship") {
        let (Some(id), Some(target)) = (rel.attr("Id"), rel.attr("Target")) else { continue };
        let kind = rel.attr("Type").unwrap_or("").to_owned();
        let external = rel.attr("TargetMode").is_some_and(|m| m.eq_ignore_ascii_case(ooxml::TARGET_MODE_EXTERNAL));
        let target = if external { target.to_owned() } else { resolve_target(part, target)? };
        out.insert(id.to_owned(), Rel { kind, target, external });
    }
    Ok(out)
}
