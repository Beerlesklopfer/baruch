// SPDX-License-Identifier: AGPL-3.0-or-later
//! EPUB als ZIP packen (Besitz „epub“, Runde 20a; seit S1 echt).
//!
//! Reihenfolge nach OCF: `mimetype` zuerst, unkomprimiert (STORED) und ohne Extra-Feld, dann
//! [`CONTAINER_PATH`](crate::CONTAINER_PATH), [`OPF_PATH`](crate::OPF_PATH) und jedes Item unter `OEBPS/{href}`
//! (DEFLATE) in Manifest-Reihenfolge. Feste Zeitstempel (1980-01-01) und Rechte: gleiche Eingabe → gleiche Bytes.

use std::io::{Cursor, Write};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipWriter};

use crate::{CONTAINER_PATH, CONTENT_DIR, EpubBook, EpubError, MIMETYPE, OPF_PATH};

/// Inhalt von `META-INF/container.xml`; verweist auf [`OPF_PATH`].
pub const CONTAINER_XML: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<container version=\"1.0\" xmlns=\"urn:oasis:names:tc:opendocument:xmlns:container\">\n\
<rootfiles>\n\
<rootfile full-path=\"OEBPS/content.opf\" media-type=\"application/oebps-package+xml\"/>\n\
</rootfiles>\n\
</container>\n";

fn zip_error(e: impl std::fmt::Display) -> EpubError {
    EpubError::Zip(e.to_string())
}

/// ZIP-Bytes des EPUB.
pub fn package(book: &EpubBook) -> Result<Vec<u8>, EpubError> {
    debug_assert!(CONTAINER_XML.contains(OPF_PATH));
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let base = SimpleFileOptions::default().last_modified_time(DateTime::default()).unix_permissions(0o644);
    let stored = base.compression_method(CompressionMethod::Stored);
    let deflated = base.compression_method(CompressionMethod::Deflated);
    zip.start_file("mimetype", stored).map_err(zip_error)?;
    zip.write_all(MIMETYPE.as_bytes()).map_err(zip_error)?;
    zip.start_file(CONTAINER_PATH, deflated).map_err(zip_error)?;
    zip.write_all(CONTAINER_XML.as_bytes()).map_err(zip_error)?;
    zip.start_file(OPF_PATH, deflated).map_err(zip_error)?;
    zip.write_all(book.opf.as_bytes()).map_err(zip_error)?;
    for item in &book.items {
        if item.href.starts_with('/') || item.href.split('/').any(|part| part == ".." || part.is_empty()) {
            return Err(EpubError::Invalid(format!("unzulässiger Pfad im Manifest: {}", item.href)));
        }
        zip.start_file(format!("{CONTENT_DIR}/{}", item.href), deflated).map_err(zip_error)?;
        zip.write_all(&item.bytes).map_err(zip_error)?;
    }
    Ok(zip.finish().map_err(zip_error)?.into_inner())
}
