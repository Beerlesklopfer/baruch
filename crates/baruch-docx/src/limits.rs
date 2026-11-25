// SPDX-License-Identifier: AGPL-3.0-or-later
//! Grenzen beim Lesen einer Word-Datei (Schnittstelle R32-S1, eingefroren).
//!
//! Den Größen im Kopf des ZIP-Archivs wird nicht getraut: Der Leser liest jeden Eintrag über `take(Grenze + 1)`
//! und bricht mit [`crate::DocxError::Limit`] ab, sobald mehr kommt (Schutz vor Dekompressionsbomben).

/// Obergrenzen des Lesers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Größte Datei (gepackt), Bytes.
    pub max_file_bytes: u64,
    /// Summe aller entpackten Einträge, Bytes.
    pub max_unpacked_bytes: u64,
    /// Größter einzelner Eintrag (entpackt), Bytes.
    pub max_entry_bytes: u64,
    /// Zahl der Einträge im Archiv.
    pub max_entries: usize,
    /// Größter XML-Teil (entpackt), Bytes.
    pub max_xml_bytes: u64,
    /// Tiefste Verschachtelung der XML-Elemente.
    pub max_depth: usize,
    /// Zahl der XML-Elemente je Teil.
    pub max_elements: usize,
    /// Text des ganzen Dokuments, Bytes.
    pub max_text_bytes: usize,
    /// Zahl der Bilder.
    pub max_images: usize,
}

impl Limits {
    /// Vorgabe: 64 MiB Datei, 256 MiB entpackt, 64 MiB je Eintrag, 4096 Einträge, 64 MiB je XML-Teil, Tiefe 128,
    /// 2 000 000 Elemente, 16 MiB Text, 2000 Bilder.
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
