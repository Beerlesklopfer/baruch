// SPDX-License-Identifier: AGPL-3.0-or-later
//! User-facing text of a store error in the language of the user interface (Round 39a, contract §2.4). The store
//! (`baruch-store`) keeps its German messages; this module maps them to `store_msg.*` keys of the catalogs.
//!
//! Used by `rpc.rs` (`impl From<StoreError> for RpcError`, language of the request) and `mcp/tools.rs`
//! (`impl From<StoreError> for Failure`, always English). Interface R39a-S1 passes the text through unchanged;
//! the task messages_2 fills in the mapping.

use baruch_store::StoreError;

use crate::i18n::Lang;

/// Text of `e` in `lang`.
pub fn store_error_text(lang: Lang, e: &StoreError) -> String {
    let _ = lang;
    e.to_string()
}
