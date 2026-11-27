// SPDX-License-Identifier: AGPL-3.0-or-later
//! Limits when reading a Word file (interface R32-S1, frozen).
//!
//! The sizes in the header of the ZIP archive are not trusted: the reader reads every entry through
//! `take(limit + 1)` and aborts with [`crate::DocxError::Limit`] as soon as more comes (protection against
//! decompression bombs).

/// Upper limits of the reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Largest file (packed), bytes.
    pub max_file_bytes: u64,
    /// Sum of all unpacked entries, bytes.
    pub max_unpacked_bytes: u64,
    /// Largest single entry (unpacked), bytes.
    pub max_entry_bytes: u64,
    /// Number of entries in the archive.
    pub max_entries: usize,
    /// Largest XML part (unpacked), bytes.
    pub max_xml_bytes: u64,
    /// Deepest nesting of the XML elements.
    pub max_depth: usize,
    /// Number of XML elements per part.
    pub max_elements: usize,
    /// Text of the whole document, bytes.
    pub max_text_bytes: usize,
    /// Number of images.
    pub max_images: usize,
}

impl Limits {
    /// Default: 64 MiB file, 256 MiB unpacked, 64 MiB per entry, 4096 entries, 64 MiB per XML part, depth 128,
    /// 2 000 000 elements, 16 MiB text, 2000 images.
    pub const DEFAULT: Limits = Limits {
        max_file_bytes: 64 * 1024 * 1024,
        max_unpacked_bytes: 256 * 1024 * 1024,
        max_entry_bytes: 64 * 1024 * 1024,
        max_entries: 4096,
        max_xml_bytes: 64 * 1024 * 1024,
        max_depth: 128,
        max_elements: 2_000_000,
        max_text_bytes: 16 * 1024 * 1024,
        max_images: 2000,
    };
}

impl Default for Limits {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[cfg(test)]
mod tests {
    use super::Limits;

    #[test]
    fn default_limits_are_the_contract_values() {
        let limits = Limits::default();
        assert_eq!(limits, Limits::DEFAULT, "Default und DEFAULT sind dieselbe Vorgabe (Vertrag R32 §1.2)");
        assert_eq!(limits.max_file_bytes, 67_108_864, "64 MiB je Datei – dieselbe Grenze wie MAX_DOCX_BYTES im Server");
        assert_eq!(limits.max_unpacked_bytes, 268_435_456, "256 MiB entpackt insgesamt");
        assert_eq!(limits.max_entry_bytes, 67_108_864, "64 MiB je Eintrag");
        assert_eq!(limits.max_entries, 4096, "höchstens 4096 Einträge im Archiv");
        assert_eq!(limits.max_xml_bytes, 67_108_864, "64 MiB je XML-Teil");
        assert_eq!(limits.max_depth, 128, "XML höchstens 128 Ebenen tief");
        assert_eq!(limits.max_elements, 2_000_000, "höchstens zwei Millionen Elemente je Teil");
        assert_eq!(limits.max_text_bytes, 16_777_216, "16 MiB Text");
        assert_eq!(limits.max_images, 2000, "höchstens 2000 Bilder");
    }
}
