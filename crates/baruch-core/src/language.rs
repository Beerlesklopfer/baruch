// SPDX-License-Identifier: AGPL-3.0-or-later
//! Work language (Round 38, interface R38-S1): the language a work is written in.
//!
//! Personas answer in this language; book output follows it from Round 39 on. It is independent of the language
//! of the user interface and of the hyphenation language of the typesetting settings (`TypesetSettings.lang`).
//! Works that existed before Round 38 are German ([`WorkLanguage::De`], the default).

/// Language of a work: German (default) or English.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WorkLanguage {
    #[default]
    De,
    En,
}

impl WorkLanguage {
    /// Every work language, in the order shown in forms.
    pub const ALL: [WorkLanguage; 2] = [WorkLanguage::De, WorkLanguage::En];

    /// Code as stored in the database and sent over RPC and MCP: `"de"` or `"en"`.
    pub fn as_str(self) -> &'static str {
        match self {
            WorkLanguage::De => "de",
            WorkLanguage::En => "en",
        }
    }

    /// Reads a code: exactly `"de"` or `"en"` – no trimming, case-sensitive. Anything else is `None`.
    pub fn parse(code: &str) -> Option<WorkLanguage> {
        match code {
            "de" => Some(WorkLanguage::De),
            "en" => Some(WorkLanguage::En),
            _ => None,
        }
    }

    /// English name of the language, as used in instructions to models: `"German"` or `"English"`.
    pub fn english_name(self) -> &'static str {
        match self {
            WorkLanguage::De => "German",
            WorkLanguage::En => "English",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::WorkLanguage;

    #[test]
    fn codes_round_trip_and_nothing_else_parses() {
        for language in WorkLanguage::ALL {
            assert_eq!(
                WorkLanguage::parse(language.as_str()),
                Some(language),
                "the stored code must read back as the same language, otherwise a saved work language would be lost"
            );
        }
        for wrong in ["", "DE", " de", "en ", "fr", "english"] {
            assert_eq!(
                WorkLanguage::parse(wrong),
                None,
                "only exactly \"de\" and \"en\" are work languages (no trimming, case-sensitive): {wrong:?}"
            );
        }
    }

    #[test]
    fn german_is_the_default_and_names_are_english() {
        assert_eq!(
            WorkLanguage::default(),
            WorkLanguage::De,
            "works from before Round 38 have no language and must stay German"
        );
        assert_eq!(WorkLanguage::De.english_name(), "German", "models are told the language by its English name");
        assert_eq!(WorkLanguage::En.english_name(), "English", "models are told the language by its English name");
    }
}
