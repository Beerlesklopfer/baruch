// SPDX-License-Identifier: AGPL-3.0-or-later
//! Glossary and index: RPC `glossary.*` and `index.registers/register_*/proposals/decide` (Round 31, contract §3.1;
//! interface R31-S1, signatures frozen, bodies by the task glossar-daten).
//!
//! - Assistants and MCP only propose terms and index entries ([`propose`], [`index_propose`]); adopting,
//!   changing and deleting only by the human (RPC from the UI, forms).
//! - Writing functions take the write lock before the DB write, exactly one mirror commit per change
//!   (`mirror::describe_glossary` etc., trailer `Baruch-Action: glossary-add|glossary-update|glossary-delete|
//!   glossary-propose|glossary-accept|glossary-reject|register-add|register-rename|register-delete|index-propose|
//!   index-accept|index-reject`); the author is the human, for proposals the persona or the MCP client. Return value =
//!   the same JSON answer as the RPC method (`commit` or `mirror` on an error of the mirror).
//! - Bible references of a term are checked and normalised by [`normalize_bible_refs`] with
//!   `baruch_scripture::parse` („Röm 3,24“); unreadable ones → `INVALID_PARAMS` with message, nothing stored.
//! - `glossary.suggest` asks the persona by the lector rule ([`crate::assist::glossary`]); the result is created only
//!   as proposals.

use baruch_store::{
    GlossaryEdit, GlossaryStatus, GlossaryTermId, IndexProposalId, IndexProposalStatus, NewIndexProposal, WorkId,
};
use baruch_typst::index::{Register, builtin_registers};
use serde_json::{Value, json};

use crate::AppState;
use crate::i18n::tc;
use crate::mirror::{self, HUMAN_NAME};
use crate::rpc::{INVALID_PARAMS, METHOD_NOT_FOUND, RpcError};

/// RPC methods of this module.
pub const METHODS: &[&str] = &[
    "glossary.list",
    "glossary.get",
    "glossary.lookup",
    "glossary.add",
    "glossary.update",
    "glossary.delete",
    "glossary.decide",
    "glossary.suggest",
    "index.registers",
    "index.register_add",
    "index.register_rename",
    "index.register_delete",
    "index.proposals",
    "index.decide",
];

pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

fn invalid(message: impl Into<String>) -> RpcError {
    RpcError::new(INVALID_PARAMS, message)
}

/// Integer `name` from the parameters; missing or not a number → `INVALID_PARAMS`.
fn int_param(params: &Value, name: &str) -> Result<i64, RpcError> {
    params
        .get(name)
        .and_then(Value::as_i64)
        .ok_or_else(|| invalid(tc("reference_msg.param.number", &[("name", name.into())])))
}

/// Text `name`; missing → `None`, not a text → `INVALID_PARAMS`.
fn str_param<'a>(params: &'a Value, name: &str) -> Result<Option<&'a str>, RpcError> {
    match params.get(name) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.as_str())),
        Some(_) => Err(invalid(tc("reference_msg.param.text", &[("name", name.into())]))),
    }
}

fn required_str<'a>(params: &'a Value, name: &str) -> Result<&'a str, RpcError> {
    str_param(params, name)?.ok_or_else(|| invalid(tc("reference_msg.param.missing", &[("name", name.into())])))
}

fn bool_param(params: &Value, name: &str) -> Result<bool, RpcError> {
    params
        .get(name)
        .and_then(Value::as_bool)
        .ok_or_else(|| invalid(tc("reference_msg.param.flag", &[("name", name.into())])))
}

/// List of texts `name`; missing → empty. A single text counts as a list of lines (one entry per line).
fn list_param(params: &Value, name: &str) -> Result<Vec<String>, RpcError> {
    match params.get(name) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::String(s)) => Ok(s.lines().map(str::to_owned).collect()),
        Some(Value::Array(items)) => items
            .iter()
            .map(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| invalid(tc("reference_msg.param.text_list", &[("name", name.into())])))
            })
            .collect(),
        Some(_) => Err(invalid(tc("reference_msg.param.text_list", &[("name", name.into())]))),
    }
}

/// Confirmation `confirm: true` for deleting; otherwise `INVALID_PARAMS`.
fn confirmed(params: &Value) -> Result<(), RpcError> {
    if params.get("confirm") == Some(&Value::Bool(true)) {
        Ok(())
    } else {
        Err(invalid(tc("reference_msg.param.confirm_delete", &[])))
    }
}

/// Input of a term from the parameters (`term` mandatory; `short`, `long`, `source`, `variants`, `bible_refs`).
pub fn edit_from_params(params: &Value) -> Result<GlossaryEdit, RpcError> {
    Ok(GlossaryEdit {
        term: required_str(params, "term")?.to_owned(),
        short: str_param(params, "short")?.unwrap_or_default().to_owned(),
        long: str_param(params, "long")?.unwrap_or_default().to_owned(),
        variants: list_param(params, "variants")?,
        source: str_param(params, "source")?.unwrap_or_default().to_owned(),
        bible_refs: list_param(params, "bible_refs")?,
    })
}

/// Checks and normalises Bible references with `baruch_scripture::parse` („röm 3, 24“ → „Röm 3,24“; several with `;`
/// become single entries). Unknown book, non-existent chapters/verses or unreadable input → `INVALID_PARAMS` with
/// the message of baruch_scripture.
pub fn normalize_bible_refs(refs: &[String]) -> Result<Vec<String>, RpcError> {
    let mut out: Vec<String> = Vec::new();
    for reference in refs {
        let reference = reference.trim();
        if reference.is_empty() {
            continue;
        }
        let found = baruch_scripture::parse(reference);
        if found.is_empty() {
            return Err(invalid(tc("reference_msg.glossary.ref_unreadable", &[("reference", reference.into())])));
        }
        for item in found {
            match (&item.status, &item.normalized) {
                (baruch_scripture::RefStatus::Valid, Some(normalized)) => {
                    if !out.contains(normalized) {
                        out.push(normalized.clone());
                    }
                }
                _ => {
                    return Err(invalid(tc(
                        "reference_msg.glossary.ref_problem",
                        &[("reference", reference.into()), ("problem", item.message().into())],
                    )));
                }
            }
        }
    }
    Ok(out)
}

/// Answer RPC `glossary.*` and the index methods.
pub async fn call(state: &AppState, work: WorkId, method: &str, params: Value) -> Result<Value, RpcError> {
    let params = if params.is_null() { json!({}) } else { params };
    match method {
        "glossary.list" => {
            let status = match str_param(&params, "status")?.unwrap_or("all") {
                "all" => None,
                "accepted" => Some(GlossaryStatus::Accepted),
                "proposed" => Some(GlossaryStatus::Proposed),
                other => return Err(invalid(tc("reference_msg.glossary.status", &[("other", other.into())]))),
            };
            let terms = state.store.glossary(work, status).await?;
            Ok(json!({ "work": work, "terms": terms.iter().map(term_json).collect::<Vec<_>>() }))
        }
        "glossary.get" => {
            let term = state.store.glossary_term(work, int_param(&params, "id")?).await?;
            Ok(json!({ "work": work, "term": term_json(&term) }))
        }
        "glossary.lookup" => {
            let word = required_str(&params, "word")?;
            let terms = state.store.glossary_lookup(work, word).await?;
            Ok(json!({ "work": work, "word": word, "terms": terms.iter().map(term_json).collect::<Vec<_>>() }))
        }
        "glossary.add" => add(state, work, edit_from_params(&params)?).await,
        "glossary.update" => update(state, work, int_param(&params, "id")?, edit_from_params(&params)?).await,
        "glossary.delete" => {
            confirmed(&params)?;
            delete(state, work, int_param(&params, "id")?).await
        }
        "glossary.decide" => decide(state, work, int_param(&params, "id")?, bool_param(&params, "accept")?).await,
        "glossary.suggest" => {
            let key = match params.get("key") {
                None | Some(Value::Null) => None,
                Some(_) => Some(int_param(&params, "key")?),
            };
            crate::assist::glossary::suggest(state, work, key, str_param(&params, "persona")?).await
        }
        "index.registers" => {
            let builtin: Vec<String> = builtin_registers().into_iter().map(|r| r.slug).collect();
            let list: Vec<Value> = registers(state, work)
                .await?
                .into_iter()
                .map(|r| {
                    json!({ "slug": r.slug, "title": r.title, "kind": r.kind, "builtin": builtin.contains(&r.slug) })
                })
                .collect();
            Ok(json!({ "work": work, "registers": list }))
        }
        "index.register_add" => {
            register_add(state, work, required_str(&params, "slug")?, required_str(&params, "title")?).await
        }
        "index.register_rename" => {
            register_rename(state, work, required_str(&params, "slug")?, required_str(&params, "title")?).await
        }
        "index.register_delete" => {
            confirmed(&params)?;
            register_delete(state, work, required_str(&params, "slug")?).await
        }
        "index.proposals" => {
            let status = match str_param(&params, "status")?.unwrap_or("open") {
                "open" => Some(IndexProposalStatus::Open),
                "all" => None,
                other => return Err(invalid(tc("reference_msg.index.status", &[("other", other.into())]))),
            };
            let list = state.store.index_proposals(work, status).await?;
            Ok(json!({ "work": work, "proposals": list.iter().map(proposal_json).collect::<Vec<_>>() }))
        }
        "index.decide" => index_decide(state, work, int_param(&params, "id")?, bool_param(&params, "accept")?).await,
        _ => Err(RpcError::new(METHOD_NOT_FOUND, format!("unknown method {method}"))),
    }
}

/// JSON of a term: `{id, uuid, term, short, long, variants, source, bible_refs, status, origin, created, updated,
/// updated_by, anchor}`.
pub fn term_json(term: &baruch_store::GlossaryTerm) -> Value {
    json!({
        "id": term.id, "uuid": term.uuid, "term": term.term, "short": term.short, "long": term.long,
        "variants": term.variants, "source": term.source, "bible_refs": term.bible_refs, "status": term.status,
        "origin": term.origin, "created": term.created, "updated": term.updated, "updated_by": term.updated_by,
        "anchor": baruch_typst::index::glossary_anchor(&term.uuid),
    })
}

/// JSON of an index proposal: `{id, key, based_on, word, occurrence, mark, reason, origin, status, created, decided,
/// decided_by, url}`.
pub fn proposal_json(proposal: &baruch_store::IndexProposal) -> Value {
    json!({
        "id": proposal.id, "key": proposal.key, "based_on": proposal.based_on, "word": proposal.word,
        "occurrence": proposal.occurrence, "mark": proposal.mark, "reason": proposal.reason,
        "origin": proposal.origin, "status": proposal.status, "created": proposal.created,
        "decided": proposal.decided, "decided_by": proposal.decided_by,
        "url": format!("/werke/{}/register#vorschlag-{}", proposal.work, proposal.id),
    })
}

/// Append `commit` or `mirror` to the answer.
fn with_mirror(mut answer: Value, result: Result<Option<String>, String>) -> Value {
    match result {
        Ok(Some(commit)) => answer["commit"] = json!(commit),
        Ok(None) => {}
        Err(e) => answer["mirror"] = json!(e),
    }
    answer
}

/// Built-in and own indexes of the work (built-in first, own ones in their order).
pub async fn registers(state: &AppState, work: WorkId) -> Result<Vec<Register>, RpcError> {
    let mut list = builtin_registers();
    list.extend(state.store.index_registers(work).await?.into_iter().map(|r| Register {
        slug: r.slug,
        title: r.title,
        kind: baruch_typst::index::RegisterKind::Custom,
    }));
    Ok(list)
}

/// Normalise the Bible references of the input (see [`normalize_bible_refs`]).
fn normalized(mut edit: GlossaryEdit) -> Result<GlossaryEdit, RpcError> {
    edit.bible_refs = normalize_bible_refs(&edit.bible_refs)?;
    Ok(edit)
}

/// Mirror a change to the glossary and build `{work, term|deleted, commit?|mirror?}`.
async fn glossary_answer(
    state: &AppState,
    held: &mirror::WriteGuard<'_>,
    change: &baruch_store::GlossaryChange,
) -> Value {
    let request = mirror::describe_glossary(change);
    let result = mirror::record_glossary(state, held, change.work, request).await;
    let answer = match (&change.after, &change.before) {
        (Some(after), _) => json!({ "work": change.work, "term": term_json(after) }),
        (None, Some(before)) => json!({ "work": change.work, "deleted": before.id }),
        (None, None) => json!({ "work": change.work }),
    };
    with_mirror(answer, result)
}

/// Create a term (human).
pub async fn add(state: &AppState, work: WorkId, edit: GlossaryEdit) -> Result<Value, RpcError> {
    let edit = normalized(edit)?;
    let held = state.write_lock.lock().await;
    let change = state.store.glossary_add(work, &edit, HUMAN_NAME).await?;
    Ok(glossary_answer(state, &held, &change).await)
}

/// Change a term (human).
pub async fn update(state: &AppState, work: WorkId, id: GlossaryTermId, edit: GlossaryEdit) -> Result<Value, RpcError> {
    let edit = normalized(edit)?;
    let held = state.write_lock.lock().await;
    let change = state.store.glossary_update(work, id, &edit, HUMAN_NAME).await?;
    Ok(glossary_answer(state, &held, &change).await)
}

/// Delete a term (human).
pub async fn delete(state: &AppState, work: WorkId, id: GlossaryTermId) -> Result<Value, RpcError> {
    let held = state.write_lock.lock().await;
    let change = state.store.glossary_delete(work, id, HUMAN_NAME).await?;
    Ok(glossary_answer(state, &held, &change).await)
}

/// Decide a proposal (human).
pub async fn decide(state: &AppState, work: WorkId, id: GlossaryTermId, accept: bool) -> Result<Value, RpcError> {
    let held = state.write_lock.lock().await;
    let change = state.store.glossary_decide(work, id, accept, HUMAN_NAME).await?;
    Ok(glossary_answer(state, &held, &change).await)
}

/// Propose a term (persona or client as `origin`).
pub async fn propose(state: &AppState, work: WorkId, edit: GlossaryEdit, origin: &str) -> Result<Value, RpcError> {
    let edit = normalized(edit)?;
    let held = state.write_lock.lock().await;
    let change = state.store.glossary_propose(work, &edit, origin).await?;
    Ok(glossary_answer(state, &held, &change).await)
}

/// Mirror a change to an own index: `{work, register, commit?|mirror?}`.
async fn register_answer(
    state: &AppState,
    held: &mirror::WriteGuard<'_>,
    work: WorkId,
    row: &baruch_store::IndexRegisterRow,
    action: mirror::RegisterAction<'_>,
) -> Value {
    let request = mirror::describe_register(row, action);
    let result = mirror::record_glossary(state, held, work, request).await;
    let answer = json!({ "work": work, "register": {
        "slug": row.slug, "title": row.title, "uuid": row.uuid, "position": row.position, "created": row.created,
    } });
    with_mirror(answer, result)
}

/// Create an own index (human).
pub async fn register_add(state: &AppState, work: WorkId, slug: &str, title: &str) -> Result<Value, RpcError> {
    let held = state.write_lock.lock().await;
    let row = state.store.index_register_add(work, slug, title).await?;
    Ok(register_answer(state, &held, work, &row, mirror::RegisterAction::Added).await)
}

/// Rename an own index (human).
pub async fn register_rename(state: &AppState, work: WorkId, slug: &str, title: &str) -> Result<Value, RpcError> {
    let held = state.write_lock.lock().await;
    let before = state.store.index_registers(work).await?.into_iter().find(|r| r.slug == slug.trim());
    let row = state.store.index_register_rename(work, slug, title).await?;
    let old = before.map(|r| r.title).unwrap_or_default();
    Ok(register_answer(state, &held, work, &row, mirror::RegisterAction::Renamed { old: &old }).await)
}

/// Delete an own index (human).
pub async fn register_delete(state: &AppState, work: WorkId, slug: &str) -> Result<Value, RpcError> {
    let held = state.write_lock.lock().await;
    let row = state.store.index_register_delete(work, slug).await?;
    let mut answer = register_answer(state, &held, work, &row, mirror::RegisterAction::Deleted).await;
    answer["deleted"] = json!(row.slug);
    Ok(answer)
}

/// Create an index proposal (persona or client, `proposal.origin`): `{work, proposal, commit?|mirror?}`. Never a
/// text change.
pub async fn index_propose(state: &AppState, work: WorkId, proposal: NewIndexProposal) -> Result<Value, RpcError> {
    let held = state.write_lock.lock().await;
    let created = state.store.index_propose(work, &proposal).await?;
    let loaded = state.store.load_work(work).await?;
    let request = mirror::describe_index_proposal(&loaded, &created, mirror::IndexProposalAction::Proposed);
    let result = mirror::record_glossary(state, &held, work, request).await;
    Ok(with_mirror(json!({ "work": work, "proposal": proposal_json(&created) }), result))
}

/// Decide an index proposal (human); accepting = new revision of the human with the mark.
pub async fn index_decide(
    state: &AppState,
    work: WorkId,
    id: IndexProposalId,
    accept: bool,
) -> Result<Value, RpcError> {
    let held = state.write_lock.lock().await;
    if !accept {
        let proposal = state.store.index_proposal_reject(work, id, HUMAN_NAME).await?;
        let loaded = state.store.load_work(work).await?;
        let request = mirror::describe_index_proposal(&loaded, &proposal, mirror::IndexProposalAction::Rejected);
        let result = mirror::record_glossary(state, &held, work, request).await;
        return Ok(with_mirror(json!({ "work": work, "proposal": proposal_json(&proposal) }), result));
    }
    let accepted = state.store.index_proposal_accept(work, id, HUMAN_NAME).await?;
    let loaded = state.store.load_work(work).await?;
    let request = mirror::describe_index_accept(&loaded, &accepted);
    let result = mirror::record_glossary(state, &held, work, request).await;
    crate::rag::notify(state, Some(work));
    let answer = json!({
        "work": work,
        "proposal": proposal_json(&accepted.proposal),
        "key": accepted.proposal.key,
        "rev": accepted.change.rev,
        "marks": accepted.marks,
    });
    Ok(with_mirror(answer, result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bible_references_are_checked_and_normalized() {
        // Contract §3.1: bible_refs checked and normalised with baruch_scripture; several references with „;“ become
        // single entries, duplicates are dropped.
        let refs = vec!["röm 3,24".to_owned(), "Röm 3,24".to_owned(), "Joh 3,16; 4,1".to_owned(), " ".to_owned()];
        let normalized = normalize_bible_refs(&refs).expect("lesbar");
        assert_eq!(normalized, ["Röm 3,24", "Joh 3,16", "Joh 4,1"], "einheitliche Schreibweise: {normalized:?}");
        for bad in ["Hesekiel 99,1", "Xyz 1,1", "Röm 3,999"] {
            let error = normalize_bible_refs(&[bad.to_owned()]).unwrap_err();
            assert_eq!(error.code, INVALID_PARAMS, "{bad}: INVALID_PARAMS");
            assert!(error.message.contains(bad), "Meldung nennt die Stelle: {}", error.message);
        }
    }
}
