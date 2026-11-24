// SPDX-License-Identifier: AGPL-3.0-or-later
//! Grenzen beim Lesen einer Austauschdatei (Vertrag R26 F7, S1 (6)). Den Größen im ZIP-Kopf wird nicht
//! getraut: Entpackt wird mit `take(Grenze + 1)`.

const MIB: u64 = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Größe der ganzen Datei (ZIP oder XML).
    pub max_file_bytes: u64,
    /// Summe aller entpackten Einträge.
    pub max_unpacked_bytes: u64,
    /// Ein entpackter Eintrag.
    pub max_entry_bytes: u64,
    /// Zahl der Einträge im ZIP.
    pub max_entries: usize,
    /// Größe von `werk.xml` bzw. der `.baruch.xml`.
    pub max_xml_bytes: u64,
    /// Verschachtelungstiefe der Elemente.
    pub max_depth: usize,
    /// Zahl der Elemente.
    pub max_elements: usize,
    /// Attribute je Element.
    pub max_attributes: usize,
    /// Bytes je Textinhalt.
    pub max_text_bytes: usize,
}

impl Limits {
    pub const DEFAULT: Limits = Limits {
        max_file_bytes: 128 * MIB,
        max_unpacked_bytes: 256 * MIB,
        max_entry_bytes: 64 * MIB,
        max_entries: 4096,
        max_xml_bytes: 64 * MIB,
        max_depth: 32,
        max_elements: 500_000,
        max_attributes: 32,
        max_text_bytes: 16 * MIB as usize,
    };
}

impl Default for Limits {
    fn default() -> Self {
        Limits::DEFAULT
    }
}
