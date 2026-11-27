// SPDX-License-Identifier: AGPL-3.0-or-later
//! Limits when reading an exchange file (contract R26 F7, S1 (6)). The sizes in the ZIP header are not
//! trusted: unpacking uses `take(limit + 1)`.

const MIB: u64 = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Size of the whole file (ZIP or XML).
    pub max_file_bytes: u64,
    /// Sum of all unpacked entries.
    pub max_unpacked_bytes: u64,
    /// One unpacked entry.
    pub max_entry_bytes: u64,
    /// Number of entries in the ZIP.
    pub max_entries: usize,
    /// Size of `werk.xml` or of the `.baruch.xml`.
    pub max_xml_bytes: u64,
    /// Nesting depth of the elements.
    pub max_depth: usize,
    /// Number of elements.
    pub max_elements: usize,
    /// Attributes per element.
    pub max_attributes: usize,
    /// Bytes per text content.
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
