// SPDX-License-Identifier: AGPL-3.0-or-later
//! User-facing text of a store error in the language of the user interface (Round 39a, contract §2.4). The store
//! (`baruch-store`) keeps its German messages; this module maps them to `store_msg.*` keys of the catalogs.
//!
//! Used by `rpc.rs` (`impl From<StoreError> for RpcError`, language of the request) and `mcp/tools.rs`
//! (`impl From<StoreError> for Failure`, always English).
//!
//! Variants with a fixed wording are formatted from their fields. Variants that carry a text (`Refused`, `Heading`,
//! `Asset`, …) are matched against the German texts of [`PATTERN_KEYS`] in `locales/de/store_msg.ftl`: a `{ $name }`
//! there matches any text, the captured parts become the arguments of the key, and an argument named `detail` is
//! translated again (nested messages such as "Veraltet: …"). The German catalog holds the store's wording verbatim,
//! so German output is unchanged. A text that matches no pattern stays as the store wrote it (German).

use std::sync::LazyLock;

use baruch_store::StoreError;
use regex::Regex;

use crate::i18n::{self, Arg, Lang};

/// Keys whose German text is a pattern for the texts of the store (longest literal part tried first).
pub const PATTERN_KEYS: &[&str] = &[
    "store_msg.accounts.taken",
    "store_msg.accounts.not_first",
    "store_msg.accounts.needs_author",
    "store_msg.accounts.last_author",
    "store_msg.accounts.share_role",
    "store_msg.accounts.share_unknown",
    "store_msg.accounts.share_lektor_only",
    "store_msg.accounts.ui_language",
    "store_msg.revision.no_current",
    "store_msg.revision.undo_current_only",
    "store_msg.revision.undo_human_only",
    "store_msg.revision.no_such_step",
    "store_msg.revision.already_at_step",
    "store_msg.revision.step_other_heading",
    "store_msg.revision.policy_percent",
    "store_msg.revision.policy_minutes",
    "store_msg.revision.already_current",
    "store_msg.revision.unchanged",
    "store_msg.revision.current_not_deletable",
    "store_msg.revision.needs_text",
    "store_msg.revision.only_proposals_accept",
    "store_msg.revision.only_proposals_reject",
    "store_msg.revision.proposals_need_current",
    "store_msg.revision.fold_above_current",
    "store_msg.revision.stale_amend",
    "store_msg.revision.stale",
    "store_msg.asset.unchanged",
    "store_msg.asset.request_current_rev",
    "store_msg.asset.layout_unchanged",
    "store_msg.asset.human_direct",
    "store_msg.asset.request_open",
    "store_msg.asset.request_decided",
    "store_msg.asset.name_taken",
    "store_msg.asset.no_collision",
    "store_msg.asset.paths_unchanged",
    "store_msg.asset.changed_meanwhile",
    "store_msg.asset.new_too_big",
    "store_msg.asset.new_unsupported",
    "store_msg.asset.svg_needs_text",
    "store_msg.asset.mime_mismatch",
    "store_msg.asset.format_change",
    "store_msg.asset.unknown_rev_named",
    "store_msg.asset.unknown_rev",
    "store_msg.asset.blocks_expected",
    "store_msg.asset.layout_changed",
    "store_msg.asset.bad_base64",
    "store_msg.asset.path_empty",
    "store_msg.asset.path_too_long",
    "store_msg.asset.path_separator",
    "store_msg.asset.path_dotdot",
    "store_msg.asset.path_no_file",
    "store_msg.asset.too_big",
    "store_msg.asset.unsupported",
    "store_msg.asset.path_taken",
    "store_msg.asset.bad_name",
    "store_msg.asset.extension_mismatch",
    "store_msg.asset.replace_mismatch",
    "store_msg.asset.collision_paths",
    "store_msg.asset.one_path",
    "store_msg.book.year",
    "store_msg.book.paper",
    "store_msg.book.bleed",
    "store_msg.book.field_too_long",
    "store_msg.book.field_one_line",
    "store_msg.connections.unknown",
    "store_msg.connections.unknown_way",
    "store_msg.connections.source_title",
    "store_msg.connections.source_address",
    "store_msg.connections.source_gone",
    "store_msg.connections.config_object",
    "store_msg.connections.model_gone",
    "store_msg.connections.model_name",
    "store_msg.connections.model_kind",
    "store_msg.connections.model_address",
    "store_msg.connections.model_taken",
    "store_msg.connections.eliezer_kind",
    "store_msg.connections.eliezer_delete",
    "store_msg.connections.lancedb_collections",
    "store_msg.connections.collection_name",
    "store_msg.connections.collections_max",
    "store_msg.connections.fetch_max",
    "store_msg.connections.sword_name",
    "store_msg.connections.sword_max",
    "store_msg.connections.persona_unknown",
    "store_msg.exchange.work_exists",
    "store_msg.exchange.work_missing",
    "store_msg.exchange.request_part",
    "store_msg.exchange.request_current",
    "store_msg.exchange.glossary_max",
    "store_msg.exchange.register_title",
    "store_msg.exchange.uuid_elsewhere",
    "store_msg.exchange.unknown_scheme",
    "store_msg.exchange.db_mismatch",
    "store_msg.exchange.instance_name_long",
    "store_msg.exchange.instance_name_control",
    "store_msg.exchange.instance_uuid",
    "store_msg.glossary.no_text",
    "store_msg.tokens.name_needed",
    "store_msg.tokens.name_long",
    "store_msg.tokens.name_control",
    "store_msg.tokens.no_random",
    "store_msg.tokens.name_valid",
    "store_msg.tokens.unknown",
    "store_msg.tokens.handle_reserved",
    "store_msg.tokens.handle_taken",
    "store_msg.tokens.handle_revoked",
    "store_msg.outline.into_own_subtree",
    "store_msg.outline.already_there",
    "store_msg.outline.parts_level",
    "store_msg.outline.too_deep",
    "store_msg.outline.same_level",
    "store_msg.outline.children_too_deep",
    "store_msg.outline.same_title",
    "store_msg.outline.no_items",
    "store_msg.outline.max_items",
    "store_msg.outline.key_missing",
    "store_msg.outline.first_level",
    "store_msg.outline.none_higher",
    "store_msg.outline.parts_counted",
    "store_msg.outline.has_number",
    "store_msg.outline.has_no_number",
    "store_msg.outline.has_children",
    "store_msg.outline.last_item",
    "store_msg.outline.confirm_merge",
    "store_msg.outline.confirm_split",
    "store_msg.outline.numbering_same",
    "store_msg.outline.part_not_into",
    "store_msg.outline.part_place",
    "store_msg.outline.lead_text",
    "store_msg.outline.merge_first",
    "store_msg.outline.merge_part",
    "store_msg.outline.to_part",
    "store_msg.outline.part_hint",
    "store_msg.outline.switch_hint",
    "store_msg.outline.with_switch_hint",
    "store_msg.outline.switch_no_heading",
    "store_msg.outline.switch_more_headings",
    "store_msg.outline.proposal_refused",
    "store_msg.outline.not_executable",
    "store_msg.outline.title_empty",
    "store_msg.outline.title_newline",
    "store_msg.outline.title_long",
    "store_msg.outline.item_n",
    "store_msg.outline.unknown_scheme",
    "store_msg.outline.idea_other_work",
    "store_msg.personas.prompt_empty",
    "store_msg.personas.prompt_long",
    "store_msg.personas.color",
    "store_msg.posts.already_here",
    "store_msg.posts.unknown_thread",
    "store_msg.posts.edit_empty",
    "store_msg.posts.edit_long",
    "store_msg.posts.edit_unchanged",
    "store_msg.posts.basis_start",
    "store_msg.posts.basis_long",
    "store_msg.posts.relay_from",
    "store_msg.findings.too_long_body",
    "store_msg.findings.too_long_quote",
    "store_msg.findings.too_long_check",
    "store_msg.findings.too_long_solution_quote",
    "store_msg.findings.too_long_solution",
    "store_msg.findings.too_long_note",
    "store_msg.findings.too_long_reason",
    "store_msg.findings.needs_text",
    "store_msg.findings.persona_length",
    "store_msg.findings.one_origin",
    "store_msg.findings.unknown_section",
    "store_msg.findings.unknown_thread_post",
    "store_msg.findings.solution_empty",
    "store_msg.findings.setter_name",
    "store_msg.findings.status_same",
    "store_msg.findings.human_direct",
    "store_msg.findings.proposal_origin",
    "store_msg.findings.human_only",
    "store_msg.findings.status_same_proposal",
    "store_msg.findings.proposal_open",
    "store_msg.findings.decider_name",
    "store_msg.findings.not_proposal",
    "store_msg.findings.decided",
    "store_msg.findings.reject_reason",
    "store_msg.findings.no_section",
    "store_msg.findings.unknown_rev",
    "store_msg.findings.defer_own",
    "store_msg.findings.defer_section",
    "store_msg.findings.defer_idea",
    "store_msg.findings.unknown_proposal",
    "store_msg.replace.query_length",
    "store_msg.replace.replacement_length",
    "store_msg.replace.sections_max",
    "store_msg.replace.twice",
    "store_msg.replace.undone",
    "store_msg.replace.unknown",
    "store_msg.timing.words",
    "store_msg.timing.target",
    "store_msg.works.template_heading",
    "store_msg.works.template_unchanged",
    "store_msg.works.bible_module",
    "store_msg.works.title_mismatch",
    "store_msg.works.not_archived",
    "store_msg.works.restore_title",
    "store_msg.works.restore_source",
    "store_msg.works.no_sections",
    "store_msg.exchange.part_exists",
    "store_msg.outline.move_keeps_level",
];

struct Pattern {
    key: &'static str,
    regex: Regex,
    names: Vec<String>,
    literal: usize,
}

/// Regex of a raw catalog value: literal text escaped, every `{ $name }` a lazy group; `None` without literal text.
fn compile(key: &'static str, raw: &str) -> Option<Pattern> {
    let placeable = Regex::new(r#"\{\s*\$([a-z0-9_]+)\s*\}|\{\s*"([^"]*)"\s*\}"#).expect("valid regex");
    let mut source = String::from("(?s)^");
    let mut names = Vec::new();
    let mut literal = 0;
    let mut last = 0;
    for found in placeable.captures_iter(raw) {
        let whole = found.get(0).expect("group 0");
        let text = &raw[last..whole.start()];
        literal += text.chars().count();
        source.push_str(&regex::escape(text));
        if let Some(name) = found.get(1) {
            names.push(name.as_str().to_owned());
            source.push_str("(.+?)");
        } else if let Some(text) = found.get(2) {
            literal += text.as_str().chars().count();
            source.push_str(&regex::escape(text.as_str()));
        }
        last = whole.end();
    }
    let tail = &raw[last..];
    literal += tail.chars().count();
    source.push_str(&regex::escape(tail));
    source.push('$');
    (literal > 0).then(|| Pattern { key, regex: Regex::new(&source).expect("escaped pattern"), names, literal })
}

static PATTERNS: LazyLock<Vec<Pattern>> = LazyLock::new(|| {
    let german: Vec<(String, String)> = i18n::CATALOGS
        .iter()
        .filter(|(code, stem, _)| *code == Lang::De.code() && *stem == "store_msg")
        .flat_map(|(_, _, source)| i18n::catalog_entries(source))
        .collect();
    let mut patterns: Vec<Pattern> = PATTERN_KEYS
        .iter()
        .filter_map(|key| german.iter().find(|(k, _)| k == key).and_then(|(_, raw)| compile(key, raw)))
        .collect();
    patterns.sort_by(|a, b| b.literal.cmp(&a.literal).then(a.key.cmp(b.key)));
    patterns
});

/// Key of the pattern that matches `text`, if any (for the checks).
pub fn pattern_key(text: &str) -> Option<&'static str> {
    PATTERNS.iter().find(|p| p.regex.is_match(text)).map(|p| p.key)
}

/// A German text of the store in `lang`; a text that matches no pattern stays as it is.
pub fn translate(lang: Lang, text: &str) -> String {
    for pattern in PATTERNS.iter() {
        let Some(found) = pattern.regex.captures(text) else { continue };
        let values: Vec<String> = pattern
            .names
            .iter()
            .enumerate()
            .map(|(i, name)| {
                let value = found.get(i + 1).map_or("", |m| m.as_str());
                if name == "detail" { translate(lang, value) } else { value.to_owned() }
            })
            .collect();
        let args: Vec<(&str, Arg<'_>)> =
            pattern.names.iter().zip(&values).map(|(name, value)| (name.as_str(), Arg::from(value))).collect();
        return i18n::t(lang, pattern.key, &args);
    }
    text.to_owned()
}

/// Text of `e` in `lang`.
pub fn store_error_text(lang: Lang, e: &StoreError) -> String {
    match e {
        StoreError::Db(err) => i18n::t(lang, "store_msg.error.db", &[("detail", err.to_string().into())]),
        StoreError::Migrate(err) => i18n::t(lang, "store_msg.error.migrate", &[("detail", err.to_string().into())]),
        StoreError::Io { path, source } => i18n::t(
            lang,
            "store_msg.error.io",
            &[("path", path.display().to_string().into()), ("detail", source.to_string().into())],
        ),
        StoreError::UnknownWork(work) => {
            i18n::t(lang, "store_msg.error.unknown_work", &[("work", work.to_string().into())])
        }
        StoreError::UnknownNode { node, .. } => {
            i18n::t(lang, "store_msg.error.unknown_node", &[("node", node.to_string().into())])
        }
        StoreError::UnknownRevision { rev, existing, .. } if existing.is_empty() => {
            i18n::t(lang, "store_msg.error.unknown_revision_none", &[("rev", rev.to_string().into())])
        }
        StoreError::UnknownRevision { rev, existing, .. } => {
            let list = existing.iter().map(u32::to_string).collect::<Vec<_>>().join(", ");
            i18n::t(lang, "store_msg.error.unknown_revision", &[("rev", rev.to_string().into()), ("list", list.into())])
        }
        StoreError::EmptyPost => i18n::t(lang, "store_msg.error.empty_post", &[]),
        StoreError::AlreadyImported { slug, title, .. } => {
            i18n::t(lang, "store_msg.error.already_imported", &[("title", title.into()), ("slug", slug.into())])
        }
        StoreError::Import(detail) => {
            i18n::t(lang, "store_msg.error.import", &[("detail", translate(lang, detail).into())])
        }
        StoreError::DuplicateTitle { slug, title, .. } => {
            i18n::t(lang, "store_msg.error.duplicate_title", &[("title", title.into()), ("slug", slug.into())])
        }
        StoreError::EmptyTitle => i18n::t(lang, "store_msg.error.empty_title", &[]),
        StoreError::GoneNode { .. } => i18n::t(lang, "store_msg.error.gone_node", &[]),
        StoreError::Numbering(detail) => {
            i18n::t(lang, "store_msg.error.numbering", &[("detail", translate(lang, detail).into())])
        }
        StoreError::UnknownAsset { name, .. } => {
            i18n::t(lang, "store_msg.error.unknown_asset", &[("name", name.into())])
        }
        StoreError::UnknownPersona(handle) => {
            i18n::t(lang, "store_msg.error.unknown_persona", &[("handle", handle.into())])
        }
        StoreError::UnknownIdea(id) => i18n::t(lang, "store_msg.error.unknown_idea", &[("id", id.to_string().into())]),
        StoreError::UnknownPost { work, post } => i18n::t(
            lang,
            "store_msg.error.unknown_post",
            &[("post", post.to_string().into()), ("work", work.to_string().into())],
        ),
        StoreError::UnknownRequest { work, id } => i18n::t(
            lang,
            "store_msg.error.unknown_request",
            &[("id", id.to_string().into()), ("work", work.to_string().into())],
        ),
        StoreError::Exchange(detail) => {
            i18n::t(lang, "store_msg.error.exchange", &[("detail", translate(lang, detail).into())])
        }
        StoreError::UnknownFinding { work, id } => i18n::t(
            lang,
            "store_msg.error.unknown_finding",
            &[("id", id.to_string().into()), ("work", work.to_string().into())],
        ),
        StoreError::Typeset(detail) => {
            // The check joins all its messages with "; ".
            let detail = detail.split("; ").map(|part| translate(lang, part)).collect::<Vec<_>>().join("; ");
            i18n::t(lang, "store_msg.error.typeset", &[("detail", detail.into())])
        }
        StoreError::Glossary(detail) => {
            i18n::t(lang, "store_msg.error.glossary", &[("detail", translate(lang, detail).into())])
        }
        StoreError::UnknownGlossaryTerm { work, id } => i18n::t(
            lang,
            "store_msg.error.unknown_glossary_term",
            &[("id", id.to_string().into()), ("work", work.to_string().into())],
        ),
        StoreError::UnknownIndexProposal { work, id } => i18n::t(
            lang,
            "store_msg.error.unknown_index_proposal",
            &[("id", id.to_string().into()), ("work", work.to_string().into())],
        ),
        StoreError::Refused(text) => translate(lang, text),
        StoreError::Heading(text)
        | StoreError::Asset(text)
        | StoreError::Title(text)
        | StoreError::Color(text)
        | StoreError::Idea(text)
        | StoreError::Finding(text)
        | StoreError::PostEdit(text)
        | StoreError::Replace(text)
        | StoreError::CoverBlock(text)
        | StoreError::Stale(text) => translate(lang, text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pattern_key_has_a_german_pattern_and_an_english_text() {
        for key in PATTERN_KEYS {
            assert!(i18n::exists(key), "{key} is listed as a pattern and must be in the en catalog");
            assert!(PATTERNS.iter().any(|p| p.key == *key), "{key} needs a German text with literal parts");
        }
    }

    #[test]
    fn german_stays_verbatim_and_english_is_translated() {
        let text = "Ungültiger Pfad „a/../b.png“: '..' ist nicht erlaubt.";
        assert_eq!(translate(Lang::De, text), text, "German output is the store's wording");
        assert_eq!(
            translate(Lang::En, text),
            "Invalid path “a/../b.png”: '..' is not allowed.",
            "the argument is carried into the English text"
        );
        assert_eq!(translate(Lang::En, "etwas ganz anderes"), "etwas ganz anderes", "an unknown text stays");
    }

    #[test]
    fn nested_details_are_translated_too() {
        let text = "Veraltet: Der Gliederungspunkt heißt schon so.";
        assert_eq!(
            translate(Lang::En, text),
            "Outdated: The outline item already has this name.",
            "the detail after the prefix is a store message of its own"
        );
        let e = StoreError::UnknownRevision { node: 1, rev: 7, existing: vec![] };
        assert_eq!(store_error_text(Lang::De, &e), e.to_string(), "German equals the Display text");
        assert_eq!(store_error_text(Lang::En, &e), "Revision 7 does not exist; available: none", "no German 'keine'");
    }
}
