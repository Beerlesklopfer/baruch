// SPDX-License-Identifier: AGPL-3.0-or-later
//! Pack the EPUB as a ZIP (ownership "epub", Round 20a; real since S1).
//!
//! Order according to OCF: `mimetype` first, uncompressed (STORED) and without extra field, then
//! [`CONTAINER_PATH`](crate::CONTAINER_PATH), [`OPF_PATH`](crate::OPF_PATH) and every item under `OEBPS/{href}`
//! (DEFLATE) in manifest order. Fixed timestamps (1980-01-01) and permissions: same input → same bytes.

use std::io::{Cursor, Write};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, DateTime, ZipWriter};

use crate::{CONTAINER_PATH, CONTENT_DIR, EpubBook, EpubError, MIMETYPE, OPF_PATH};

/// Content of `META-INF/container.xml`; refers to [`OPF_PATH`].
pub const CONTAINER_XML: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<container version=\"1.0\" xmlns=\"urn:oasis:names:tc:opendocument:xmlns:container\">\n\
<rootfiles>\n\
<rootfile full-path=\"OEBPS/content.opf\" media-type=\"application/oebps-package+xml\"/>\n\
</rootfiles>\n\
</container>\n";

fn zip_error(e: impl std::fmt::Display) -> EpubError {
    EpubError::Zip(e.to_string())
}

/// ZIP bytes of the EPUB.
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
