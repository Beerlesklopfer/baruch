// SPDX-License-Identifier: AGPL-3.0-or-later
//! Git mirror after a change (contract B1, section 6): snapshot of the current
//! revisions from the database, author and trailers from the revision that is current now, commit in the
//! folder `<data>/works/<slug>/`. The database is the source; if the mirror fails, the
//! change stays valid, the error is logged and reported, the next commit catches up.
//! Every work has its own mirror ([`AppState::mirror`]); `record` uses the work of the change.
//!
//! Log of the revision tree: switching, changing and deleting a revision each yield one commit
//! with the subject „<Nummer> <Titel>: auf Fassung n umgeschaltet“ or „…: Fassung n geändert“ / „…:
//! Fassung n gelöscht“ and the trailer `Baruch-Action`. Changing and deleting commit even without
//! a change to the files (`allow_empty`); the body then carries before/after or the deleted
//! text, so that deleted content stays restorable from the Git history.
//!
//! Switching and accepting ([`record_switch`], contract Round 5): exactly one commit, even if the
//! revision brings another level along and sub-items are re-levelled with it (subject „… auf Fassung n
//! umgeschaltet · Ebene a → b“, body with the re-levelled items, `Baruch-Action: switched`).
//!
//! Outline (contract Round 4, section 6): every outline change is exactly one commit
//! ([`record_outline`]) with the subject „Gliederung: …“, author and committer the human, trailers
//! `Baruch-Action: outline-*` and `Baruch-Node`; the body carries the text of deleted nodes. If a
//! persona or an MCP client creates items, it is the author (`Baruch-Origin`, `Persona`); if the
//! human confirms a request, the commit names him as the confirmer and the requester as the origin
//! ([`OutlineBy`], wish 2026-11-06).
//!
//! Images (Round 11): if a persona or an MCP client creates an image (`asset_add`), it is the author of the
//! commit ([`AssetBy::Direct`]); if the human confirms an image request, the commit names him as the
//! confirmer and the requester as the origin ([`AssetBy::Confirmed`]). A confirmed cover request
//! additionally gets a log commit ([`record_asset_request`]); the cover itself is mirrored by
//! `work.set_cover`.

use baruch_core::{Author, DiffKind, RevisionStatus, Work, line_diff};
use baruch_gitmirror::{CommitRequest, MirrorFile, SideRef, Signature};
use baruch_store::{Change, ChangeKind, OutlineChange, RevisionRecord, Switch, WorkId};

use crate::AppState;

/// Name and address of the human who accepts or saves.
pub const HUMAN_NAME: &str = "Autor";
pub const HUMAN_EMAIL: &str = "autor@baruch.invalid";

fn human(name: &str) -> Signature {
    Signature { name: name.to_owned(), email: HUMAN_EMAIL.to_owned() }
}

/// Commit request without files for a change: subject, body, author, committer and trailers.
/// `work` is the state *after* the change.
pub fn describe(work: &Work, change: &Change) -> CommitRequest {
    let node = work.nodes.get(change.node);
    let title = node.map_or_else(|| format!("Abschnitt {}", change.node), |n| n.title.clone());
    let label = match node.map(|n| n.number.as_str()) {
        Some(number) if !number.is_empty() => format!("{number} {title}"),
        _ => title,
    };
    let rev = change.rev;
    let (summary, action) = match change.kind {
        ChangeKind::Accepted => (format!("{label}: Vorschlag {rev} übernommen"), None),
        ChangeKind::Saved => (format!("{label}: Revision {rev} gespeichert"), None),
        ChangeKind::Switched => (format!("{label}: auf Fassung {rev} umgeschaltet"), Some("switched")),
        ChangeKind::Updated => (format!("{label}: Fassung {rev} geändert"), Some("updated")),
        ChangeKind::Deleted => (format!("{label}: Fassung {rev} gelöscht"), Some("deleted")),
        ChangeKind::Amended => {
            let k = change.amend_seq.unwrap_or(0);
            (format!("{label}: Fassung {rev} fortgeschrieben (Stand {k})"), Some("amended"))
        }
        ChangeKind::AmendUndone => {
            let k = change.amend_seq.unwrap_or(0);
            (format!("{label}: Fortschreibung von Fassung {rev} zurückgenommen (Stand {k})"), Some("amend-undone"))
        }
    };
    let body = match (change.kind, &change.before) {
        (ChangeKind::Updated, Some(before)) => {
            let after = node.and_then(|n| n.revision(rev));
            updated_body(before, after.map_or("", |r| r.body.as_str()), after.map_or("", |r| r.note.as_str()))
        }
        (ChangeKind::Deleted, Some(before)) => deleted_body(before, &change.reparented),
        _ => String::new(),
    };
    let mut trailers = Vec::new();
    let (author, committer) = match &change.author {
        Author::Human { name } => (human(name), human(name)),
        Author::Assistant { persona, model } => {
            let name =
                work.personas.iter().find(|p| &p.handle == persona).map_or(persona.as_str(), |p| p.name.as_str());
            trailers.push(("Persona".to_owned(), persona.clone()));
            trailers.push(("Model".to_owned(), model.clone()));
            let author = Signature {
                name: format!("{name} ({persona})"),
                email: format!("{}@persona.baruch.invalid", email_local_part(persona)),
            };
            (author, human(HUMAN_NAME))
        }
    };
    trailers.push(("Baruch-Node".to_owned(), change.node.to_string()));
    trailers.push(("Baruch-Revision".to_owned(), rev.to_string()));
    if let Some(action) = action {
        trailers.push(("Baruch-Action".to_owned(), action.to_owned()));
    }
    if matches!(change.kind, ChangeKind::Amended | ChangeKind::AmendUndone) {
        trailers.push(("Baruch-Amend".to_owned(), change.amend_seq.unwrap_or(0).to_string()));
    }
    // Log commits: also without a change to the files (e.g. a revision that is not current).
    // Switching as well, so that every switch yields exactly one commit, also between revisions
    // with the same text.
    // Round 22: every amendment and undo is exactly one commit (also with the same text).
    let allow_empty = matches!(
        change.kind,
        ChangeKind::Switched
            | ChangeKind::Updated
            | ChangeKind::Deleted
            | ChangeKind::Amended
            | ChangeKind::AmendUndone
    );
    CommitRequest { files: Vec::new(), author, committer, summary, body, trailers, allow_empty }
}

fn status_word(status: RevisionStatus) -> &'static str {
    match status {
        RevisionStatus::Current => "aktuell",
        RevisionStatus::Proposed => "Vorschlag",
        RevisionStatus::Rejected => "verworfen",
        RevisionStatus::Superseded => "ersetzt",
    }
}

/// Status of a revision as a German word (aktuell, Vorschlag, verworfen, ersetzt) – also for the page „Verlauf“.
pub(crate) fn revision_status_word(status: RevisionStatus) -> &'static str {
    status_word(status)
}

fn author_text(author: &Author) -> String {
    match author {
        Author::Human { name } => name.clone(),
        Author::Assistant { persona, model } => format!("{persona} ({model})"),
    }
}

/// Body for „Fassung n geändert“: line diff before/after and the note before/after.
fn updated_body(before: &RevisionRecord, after_body: &str, after_note: &str) -> String {
    let mut out = String::from("Text (− vorher, + nachher):\n");
    if before.body == after_body {
        out.push_str("  (unverändert)\n");
    } else {
        for (kind, line) in line_diff(&before.body, after_body) {
            let mark = match kind {
                DiffKind::Same => "  ",
                DiffKind::Added => "+ ",
                DiffKind::Removed => "- ",
            };
            out.push_str(mark);
            out.push_str(&line);
            out.push('\n');
        }
    }
    out.push_str(&format!("\nNotiz vorher: {}\nNotiz nachher: {}\n", one_line(&before.note), one_line(after_note)));
    out
}

/// Body for „Fassung n gelöscht“: the complete text, then author, status, parent, note and
/// the re-parented children.
fn deleted_body(before: &RevisionRecord, reparented: &[u32]) -> String {
    let parent = before.based_on.map_or_else(|| "keins (Wurzel)".to_owned(), |b| format!("Fassung {b}"));
    let moved = if reparented.is_empty() {
        "keine".to_owned()
    } else {
        let to = before.based_on.map_or_else(|| "Wurzel".to_owned(), |b| format!("Fassung {b}"));
        let list: Vec<String> = reparented.iter().map(u32::to_string).collect();
        format!("{} → {to}", list.join(", "))
    };
    let mut text = before.body.clone();
    if !text.ends_with('\n') {
        text.push('\n');
    }
    format!(
        "Gelöschter Text:\n{text}\nAutor: {}\nStatus: {}\nElternteil: {parent}\nNotiz: {}\nUmgehängt: {moved}\n",
        author_text(&before.author),
        status_word(before.status),
        one_line(&before.note),
    )
}

/// Note in one line (empty → „–“).
fn one_line(s: &str) -> String {
    let joined = s.split(['\r', '\n']).map(str::trim).filter(|p| !p.is_empty()).collect::<Vec<_>>().join(" ");
    if joined.is_empty() { "–".to_owned() } else { joined }
}

/// Proof that the caller holds [`AppState::write_lock`].
pub type WriteGuard<'a> = tokio::sync::MutexGuard<'a, ()>;

/// Fetch the snapshot and commit. `Ok(None)`: the mirror was already up to date.
/// The caller holds the write lock since *before* the database write, otherwise the
/// snapshot could contain foreign changes.
async fn commit(
    state: &AppState,
    held: &WriteGuard<'_>,
    work: WorkId,
    request: CommitRequest,
) -> Result<Option<String>, String> {
    commit_with(state, held, work, request, None).await
}

/// Like [`commit`], then bring the side branches of the work up to date ([`side_refs`], Round 22, contract §2.12):
/// after every revision commit (`record`, `record_switch`, `record_outline_by`, exchange) and at start
/// (`sync_work`). An error of the side branches is only on stderr; the commit counts.
async fn commit_revisions(
    state: &AppState,
    held: &WriteGuard<'_>,
    work: WorkId,
    request: CommitRequest,
) -> Result<Option<String>, String> {
    let sides = match side_refs(state, work).await {
        Ok(sides) => Some(sides),
        Err(e) => {
            eprintln!("Git-Spiegel (Werk {work}): Seitenzweige nicht nachgeführt: {e}");
            None
        }
    };
    commit_with(state, held, work, request, sides).await
}

async fn commit_with(
    state: &AppState,
    _held: &WriteGuard<'_>,
    work: WorkId,
    mut request: CommitRequest,
    sides: Option<Vec<SideRef>>,
) -> Result<Option<String>, String> {
    let files = state.store.mirror_snapshot(work).await.map_err(|e| e.to_string())?;
    request.files = files.into_iter().map(|f| MirrorFile { path: f.path, contents: f.contents }).collect();
    let mirror = state.mirror(work).await?;
    let result = tokio::task::spawn_blocking(move || {
        let done = mirror.commit(&request)?;
        if let Some(sides) = sides
            && let Err(e) = mirror.sync_side_refs(SIDE_PREFIX, &request.files, &sides)
        {
            eprintln!("Git-Spiegel: Seitenzweige nicht nachgeführt: {e}");
        }
        Ok::<_, baruch_gitmirror::MirrorError>(done)
    })
    .await
    .map_err(|e| format!("Git: {e}"))?
    .map_err(|e| e.to_string());
    if result.is_err() {
        // Reopen next time: perhaps the folder is repaired by then.
        state.forget_mirror(work);
    }
    result
}

/// Bring the side branches of the work up to date without a new commit (e.g. after a new proposal that itself gets
/// no commit). The caller holds the write lock. Result: written/kept/removed; errors as text.
pub(crate) async fn sync_side_refs(
    state: &AppState,
    _held: &WriteGuard<'_>,
    work: WorkId,
) -> Result<baruch_gitmirror::SideSync, String> {
    let sides = side_refs(state, work).await?;
    let files = state.store.mirror_snapshot(work).await.map_err(|e| e.to_string())?;
    let files: Vec<MirrorFile> = files.into_iter().map(|f| MirrorFile { path: f.path, contents: f.contents }).collect();
    let mirror = state.mirror(work).await?;
    tokio::task::spawn_blocking(move || mirror.sync_side_refs(SIDE_PREFIX, &files, &sides))
        .await
        .map_err(|e| format!("Git: {e}"))?
        .map_err(|e| e.to_string())
}

/// After a new proposal (Round 22, integrator): bring the side branches up to date right away, so that the proposal
/// does not appear as `refs/revisions/<key>/<n>` only at the next revision commit or start. The caller holds the
/// write lock; an error is only on stderr (the proposal counts).
pub(crate) async fn side_refs_after_proposal(state: &AppState, held: &WriteGuard<'_>, work: WorkId) {
    if let Err(e) = sync_side_refs(state, held, work).await {
        eprintln!("Git-Spiegel: Seitenzweige nach neuem Vorschlag nicht nachgeführt: {e}");
    }
}

/// Prefix of the side branches: `refs/revisions/<key>/<n>` (key = fixed key `node.id`, n = revision number).
pub const SIDE_PREFIX: &str = "refs/revisions/";

/// Status of a revision that is not current as a word for the trailer `Baruch-Side`.
fn side_word(status: RevisionStatus) -> &'static str {
    match status {
        RevisionStatus::Current => "current",
        RevisionStatus::Proposed => "proposed",
        RevisionStatus::Rejected => "rejected",
        RevisionStatus::Superseded => "superseded",
    }
}

/// Side branches of the work from the database (Round 22, contract §2.12): per revision that is not current
/// (replaced, discarded, open proposal including variants) one reference `refs/revisions/<key>/<n>` to a commit with
/// the snapshot of the work in which the section carries this revision. Author as for the revision commits (human
/// or „Name (@handle)“), committer the human; trailers `Baruch-Node`, `Baruch-Key`, `Baruch-Revision`,
/// `Baruch-Side`.
pub(crate) async fn side_refs(state: &AppState, work: WorkId) -> Result<Vec<SideRef>, String> {
    let loaded = state.store.load_work(work).await.map_err(|e| e.to_string())?;
    let sides = state.store.side_revisions(work).await.map_err(|e| e.to_string())?;
    Ok(sides
        .into_iter()
        .map(|(key, info, file)| {
            let node = loaded.nodes.iter().find(|n| n.key == key);
            let label = match node {
                Some(n) if !n.number.is_empty() => format!("{} {}", n.number, n.title),
                Some(n) => n.title.clone(),
                None => format!("Abschnitt {key}"),
            };
            let mut trailers = Vec::new();
            let author = match &info.author {
                Author::Human { name } => human(name),
                Author::Assistant { persona, model } => {
                    let name = loaded
                        .personas
                        .iter()
                        .find(|p| &p.handle == persona)
                        .map_or(persona.as_str(), |p| p.name.as_str());
                    trailers.push(("Persona".to_owned(), persona.clone()));
                    trailers.push(("Model".to_owned(), model.clone()));
                    Signature {
                        name: format!("{name} ({persona})"),
                        email: format!("{}@persona.baruch.invalid", email_local_part(persona)),
                    }
                }
            };
            if let Some(n) = node {
                trailers.push(("Baruch-Node".to_owned(), n.id.to_string()));
            }
            trailers.push(("Baruch-Key".to_owned(), key.to_string()));
            trailers.push(("Baruch-Revision".to_owned(), info.number.to_string()));
            trailers.push(("Baruch-Side".to_owned(), side_word(info.status).to_owned()));
            let mut body = String::new();
            if let Some(variant) = &info.variant {
                body.push_str(&format!("Variante {}: {}\n", variant.label, one_line(&variant.summary)));
            }
            if !info.note.trim().is_empty() {
                body.push_str(&format!("Notiz: {}\n", one_line(&info.note)));
            }
            SideRef {
                name: format!("{SIDE_PREFIX}{key}/{}", info.number),
                path: file.path,
                contents: file.contents,
                author,
                committer: human(HUMAN_NAME),
                summary: format!("{label}: Fassung {} ({}) als Seitenzweig", info.number, status_word(info.status)),
                body,
                trailers,
            }
        })
        .collect())
}

/// Words of the fields of the setting „Fassungen“ (commit and user interface).
pub fn revision_policy_field(field: &str) -> &'static str {
    match field {
        "amend" => "Fortschreiben",
        "max_change_percent" => "größte Änderung",
        "new_after_minutes" => "neue Fassung nach",
        _ => "unbekanntes Feld",
    }
}

fn revision_policy_value(policy: &baruch_store::RevisionPolicy, field: &str) -> String {
    match field {
        "amend" => on_off(policy.amend).to_owned(),
        "max_change_percent" => format!("{} %", policy.max_change_percent),
        "new_after_minutes" if policy.new_after_minutes == 0 => "keine Zeitgrenze".to_owned(),
        "new_after_minutes" => format!("{} min", policy.new_after_minutes),
        _ => String::new(),
    }
}

/// Commit request without files for the setting „Fassungen“ (Round 22): subject „Fassungen: <Felder> geändert“,
/// body per field before → after, author and committer the human, trailer `Baruch-Action: revision-policy-set`.
/// The setting is not in the mirror, hence an empty log commit (`allow_empty`).
pub fn describe_revision_policy(change: &baruch_store::RevisionPolicyChange, human_name: &str) -> CommitRequest {
    let words: Vec<&str> = change.changed.iter().map(|f| revision_policy_field(f)).collect();
    let body = change
        .changed
        .iter()
        .map(|f| {
            format!(
                "{}: {} → {}\n",
                revision_policy_field(f),
                revision_policy_value(&change.before, f),
                revision_policy_value(&change.after, f)
            )
        })
        .collect::<String>();
    CommitRequest {
        files: Vec::new(),
        author: human(human_name),
        committer: human(human_name),
        summary: format!("Fassungen: {} geändert", words.join(", ")),
        body,
        trailers: vec![("Baruch-Action".to_owned(), "revision-policy-set".to_owned())],
        allow_empty: true,
    }
}

/// Mirror the setting „Fassungen“ (one commit, see [`describe_revision_policy`]); the caller holds the
/// write lock since before the writing. Errors are also on stderr.
pub async fn record_revision_policy(
    state: &AppState,
    held: &WriteGuard<'_>,
    change: &baruch_store::RevisionPolicyChange,
) -> Result<Option<String>, String> {
    let result = commit(state, held, change.work, describe_revision_policy(change, HUMAN_NAME)).await;
    if let Err(e) = &result {
        eprintln!("Git-Spiegel: {e}");
    }
    result
}

/// Local part of the e-mail from a persona handle: without the leading `@`, characters outside
/// `[A-Za-z0-9._-]` become `-` (`@mcp:claude-code` → `mcp-claude-code`).
fn email_local_part(handle: &str) -> String {
    let local: String = handle
        .trim_start_matches('@')
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') { c } else { '-' })
        .collect();
    if local.is_empty() { "persona".to_owned() } else { local }
}

/// Mirror a change. Errors are logged on stderr and returned as text.
///
/// `held` is the write lock that the caller took before the database write and holds up to
/// here: this way no other change lies between change and snapshot, each gets
/// its own commit with its author and its trailers.
pub async fn record(state: &AppState, held: &WriteGuard<'_>, change: &Change) -> Result<Option<String>, String> {
    crate::rag::notify(state, Some(change.work));
    let result = match state.store.load_work(change.work).await {
        Ok(work) => commit_revisions(state, held, change.work, describe(&work, change)).await,
        Err(e) => Err(e.to_string()),
    };
    if let Err(e) = &result {
        eprintln!("Git-Spiegel: {e}");
    }
    result
}

/// Commit request for a switch ([`Store::switch_to`](baruch_store::Store::switch_to)): like
/// [`describe`]; if the revision brings another level along, the subject appends „· Ebene a → b“, the
/// body names the re-levelled items with their new revisions and the renumbering, and the
/// trailer `Baruch-Action: switched` is always there (also when accepting). `work` is the state
/// *after* the change.
pub fn describe_switch(work: &Work, switch: &Switch) -> CommitRequest {
    let mut request = describe(work, &switch.change);
    let Some(outline) = &switch.outline else { return request };
    // `detail` ends with „Ebene a → b“ (store, `follow_switched`).
    if let Some(at) = outline.detail.rfind("Ebene ") {
        request.summary.push_str(&format!(" · {}", &outline.detail[at..]));
    }
    let mut body = String::new();
    if !outline.revisions.is_empty() {
        body.push_str("Mitgestuft (neue Fassungen, nur die Überschriftszeile geändert):\n");
        for &(node, rev) in &outline.revisions {
            let title = work.nodes.get(node).map_or_else(|| format!("Abschnitt {node}"), |n| n.title.clone());
            body.push_str(&format!("  {node:03} {title}: Fassung {rev}\n"));
        }
    }
    if !outline.renumbered.is_empty() {
        let list: Vec<String> = outline.renumbered.iter().map(|(old, new)| format!("{old} → {new}")).collect();
        if !body.is_empty() {
            body.push('\n');
        }
        body.push_str(&format!("Umnummeriert (seq alt → neu): {}\n", list.join(", ")));
    }
    if !body.is_empty() {
        if !request.body.is_empty() {
            request.body.push('\n');
        }
        request.body.push_str(&body);
    }
    if !request.trailers.iter().any(|(k, _)| k == "Baruch-Action") {
        request.trailers.push(("Baruch-Action".to_owned(), "switched".to_owned()));
    }
    request.allow_empty = true;
    request
}

/// Mirror a switch: exactly one commit (see [`describe_switch`]), even if the outline
/// was re-levelled with it. As with [`record`] the caller holds the write lock since before the
/// database write; errors are also on stderr.
pub async fn record_switch(state: &AppState, held: &WriteGuard<'_>, switch: &Switch) -> Result<Option<String>, String> {
    let work = switch.change.work;
    crate::rag::notify(state, Some(work));
    let result = match state.store.load_work(work).await {
        Ok(loaded) => commit_revisions(state, held, work, describe_switch(&loaded, switch)).await,
        Err(e) => Err(e.to_string()),
    };
    if let Err(e) = &result {
        eprintln!("Git-Spiegel: {e}");
    }
    result
}

/// Who caused an outline change (wish 2026-11-06).
#[derive(Debug, Clone, Copy)]
pub enum OutlineBy<'a> {
    /// The human himself (user interface, RPC).
    Human,
    /// A persona or an MCP client directly (creating only, MCP `outline_add`): it is the author of the commit,
    /// the human the committer; trailers `Baruch-Origin` and `Persona`.
    Direct(&'a baruch_store::IdeaOrigin),
    /// A request that the human confirmed: author and committer the human (confirmer), trailers
    /// `Baruch-Origin` (origin of the request), `Baruch-Request`, `Baruch-Requested-By`,
    /// `Baruch-Confirmed-By`.
    Confirmed(&'a baruch_store::OutlineRequest),
}

/// Commit request without files for an outline change of the human (see [`describe_outline_by`]).
pub fn describe_outline(work: &Work, change: &OutlineChange) -> CommitRequest {
    describe_outline_by(work, change, OutlineBy::Human)
}

/// Commit request without files for an outline change: subject „Gliederung: <detail>“, body with
/// new revisions, renumbering and the complete text of deleted nodes, trailers
/// `Baruch-Action: outline-*` and `Baruch-Node`; author and origin by `by` ([`OutlineBy`]). Always
/// one commit (`allow_empty`), even if no file changes (e.g. with the numbering). `work`
/// is the state *after* the change.
pub fn describe_outline_by(work: &Work, change: &OutlineChange, by: OutlineBy<'_>) -> CommitRequest {
    use baruch_store::IdeaOrigin;
    let mut body = String::new();
    if let OutlineBy::Confirmed(request) = by {
        body.push_str(&format!(
            "Antrag {} von {}, bestätigt von {}: {}\n",
            request.id,
            request.origin.name(),
            request.decided_by.as_deref().unwrap_or(HUMAN_NAME),
            request.summary
        ));
        if !request.note.is_empty() {
            body.push_str(&format!("Begründung: {}\n", one_line(&request.note)));
        }
    }
    if !change.revisions.is_empty() {
        if !body.is_empty() {
            body.push('\n');
        }
        body.push_str("Neue Fassungen:\n");
        for &(node, rev) in &change.revisions {
            let title = work.nodes.get(node).map_or_else(|| format!("Abschnitt {node}"), |n| n.title.clone());
            body.push_str(&format!("  {node:03} {title}: Fassung {rev}\n"));
        }
    }
    if !change.renumbered.is_empty() {
        let list: Vec<String> = change.renumbered.iter().map(|(old, new)| format!("{old} → {new}")).collect();
        if !body.is_empty() {
            body.push('\n');
        }
        body.push_str(&format!("Umnummeriert (seq alt → neu): {}\n", list.join(", ")));
    }
    for (title, text) in &change.removed {
        if !body.is_empty() {
            body.push('\n');
        }
        let mut text = text.clone();
        if !text.ends_with('\n') {
            text.push('\n');
        }
        body.push_str(&format!("Gelöschter Text „{title}“:\n{text}"));
    }
    let mut trailers = vec![
        ("Baruch-Action".to_owned(), change.action.trailer().to_owned()),
        ("Baruch-Node".to_owned(), change.node.to_string()),
    ];
    let mut author = human(HUMAN_NAME);
    let mut summary = format!("Gliederung: {}", change.detail);
    match by {
        OutlineBy::Human => {}
        OutlineBy::Direct(origin) => {
            trailers.push(("Baruch-Origin".to_owned(), origin.kind().to_owned()));
            if let IdeaOrigin::Persona { handle } | IdeaOrigin::Mcp { handle } = origin {
                trailers.push(("Persona".to_owned(), handle.clone()));
                author = Signature {
                    name: handle.clone(),
                    email: format!("{}@persona.baruch.invalid", email_local_part(handle)),
                };
            }
        }
        OutlineBy::Confirmed(request) => {
            summary.push_str(&format!(" (Antrag {})", request.id));
            let confirmer = request.decided_by.clone().unwrap_or_else(|| HUMAN_NAME.to_owned());
            author = human(&confirmer);
            trailers.push(("Baruch-Origin".to_owned(), request.origin.kind().to_owned()));
            trailers.push(("Baruch-Request".to_owned(), request.id.to_string()));
            trailers.push(("Baruch-Requested-By".to_owned(), request.origin.name().to_owned()));
            trailers.push(("Baruch-Confirmed-By".to_owned(), confirmer));
        }
    }
    CommitRequest {
        files: Vec::new(),
        author,
        committer: human(HUMAN_NAME),
        summary,
        body,
        trailers,
        allow_empty: true,
    }
}

/// Body paragraph „was außer der aktuellen Fassung mitgelöscht wurde“ (Round 36, store-5): every other revision
/// and every post of the deleted outline item verbatim, so that the mirror conceals nothing. Empty if there was
/// nothing.
pub fn describe_losses(losses: &baruch_store::OutlineLosses) -> String {
    /// Head line and wording, always with a line end.
    fn block(body: &mut String, head: &str, text: &str) {
        body.push_str(head);
        body.push_str(text);
        if !text.is_empty() && !text.ends_with('\n') {
            body.push('\n');
        }
    }
    let mut body = String::new();
    if !losses.revisions.is_empty() {
        let proposals = losses.revisions.iter().filter(|r| r.proposal).count();
        let head =
            format!("Mitgelöschte Fassungen: {} (davon offene Vorschläge: {proposals})\n", losses.revisions.len());
        block(&mut body, &head, "");
        for revision in &losses.revisions {
            let kind = if revision.proposal { "offener Vorschlag" } else { "ältere Fassung" };
            let head = format!("Fassung {} ({kind} von {}):\n", revision.number, one_line(&revision.author));
            block(&mut body, &head, &revision.body);
        }
    }
    if !losses.posts.is_empty() {
        if !body.is_empty() {
            body.push('\n');
        }
        block(&mut body, &format!("Mitgelöschte Beiträge: {}\n", losses.posts.len()), "");
        for post in &losses.posts {
            block(&mut body, &format!("Beitrag {} von {}:\n", post.id, one_line(&post.author)), &post.body);
        }
    }
    body
}

/// Like [`record_outline_by`] for deleting an outline item (Round 36, store-5): the same single commit,
/// in the body additionally the revisions and posts deleted with it ([`describe_losses`]; the caller reads them
/// before deleting with `Store::outline_delete_losses`). Plus the trailers `Baruch-Lost-Revisions` and
/// `Baruch-Lost-Posts`.
pub async fn record_outline_losses(
    state: &AppState,
    held: &WriteGuard<'_>,
    change: &OutlineChange,
    by: OutlineBy<'_>,
    losses: &baruch_store::OutlineLosses,
) -> Result<Option<String>, String> {
    if losses.revisions.is_empty() && losses.posts.is_empty() {
        return record_outline_by(state, held, change, by).await;
    }
    crate::rag::notify(state, Some(change.work));
    let result = match state.store.load_work(change.work).await {
        Ok(work) => {
            let mut request = describe_outline_by(&work, change, by);
            if !request.body.is_empty() && !request.body.ends_with("\n\n") {
                request.body.push('\n');
            }
            request.body.push_str(&describe_losses(losses));
            request.trailers.push(("Baruch-Lost-Revisions".to_owned(), losses.revisions.len().to_string()));
            request.trailers.push(("Baruch-Lost-Posts".to_owned(), losses.posts.len().to_string()));
            name_persona_author(state, &mut request).await;
            commit_revisions(state, held, change.work, request).await
        }
        Err(e) => Err(e.to_string()),
    };
    if let Err(e) = &result {
        eprintln!("Git-Spiegel: {e}");
    }
    result
}

/// Mirror an outline change of the human (see [`record_outline_by`]).
pub async fn record_outline(
    state: &AppState,
    held: &WriteGuard<'_>,
    change: &OutlineChange,
) -> Result<Option<String>, String> {
    record_outline_by(state, held, change, OutlineBy::Human).await
}

/// Mirror an outline change: exactly one commit (see [`describe_outline_by`]); for a persona
/// the author is named as with proposals „<Name> (<handle>)“, for an MCP client „MCP-Client
/// (<handle>)“. As with [`record`] the caller holds the write lock since before the database write;
/// errors are also on stderr.
pub async fn record_outline_by(
    state: &AppState,
    held: &WriteGuard<'_>,
    change: &OutlineChange,
    by: OutlineBy<'_>,
) -> Result<Option<String>, String> {
    crate::rag::notify(state, Some(change.work));
    let result = match state.store.load_work(change.work).await {
        Ok(work) => {
            let mut request = describe_outline_by(&work, change, by);
            name_persona_author(state, &mut request).await;
            commit_revisions(state, held, change.work, request).await
        }
        Err(e) => Err(e.to_string()),
    };
    if let Err(e) = &result {
        eprintln!("Git-Spiegel: {e}");
    }
    result
}

/// Commit request without files for the decision on a request that does not change the outline
/// (user question 12: rejected or outdated on consenting). Subject „Gliederung: Antrag n abgelehnt“
/// or „… veraltet“, body with requester, decider, summary, justification and reason;
/// author the decider, trailers `Baruch-Action: outline-request`, `Baruch-Decision`, `Baruch-Origin`,
/// `Baruch-Request`, `Baruch-Requested-By`, `Baruch-Decided-By`. Empty commit (`allow_empty`).
pub fn describe_request_decision(request: &baruch_store::OutlineRequest) -> CommitRequest {
    let decider = request.decided_by.clone().unwrap_or_else(|| HUMAN_NAME.to_owned());
    let label = request.decision.map_or("offen", |d| d.label());
    let mut body =
        format!("Antrag {} von {}, {label} von {decider}: {}\n", request.id, request.origin.name(), request.summary);
    if !request.note.is_empty() {
        body.push_str(&format!("Begründung: {}\n", one_line(&request.note)));
    }
    if let Some(message) = &request.message {
        body.push_str(&format!("Grund: {}\n", one_line(message)));
    }
    let decision = match request.decision {
        Some(baruch_store::Decision::Confirmed) => "confirmed",
        Some(baruch_store::Decision::Rejected) => "rejected",
        Some(baruch_store::Decision::Stale) => "stale",
        None => "open",
    };
    CommitRequest {
        files: Vec::new(),
        author: human(&decider),
        committer: human(HUMAN_NAME),
        summary: format!("Gliederung: Antrag {} {label}", request.id),
        body,
        trailers: vec![
            ("Baruch-Action".to_owned(), "outline-request".to_owned()),
            ("Baruch-Decision".to_owned(), decision.to_owned()),
            ("Baruch-Origin".to_owned(), request.origin.kind().to_owned()),
            ("Baruch-Request".to_owned(), request.id.to_string()),
            ("Baruch-Requested-By".to_owned(), request.origin.name().to_owned()),
            ("Baruch-Decided-By".to_owned(), decider),
        ],
        allow_empty: true,
    }
}

/// Mirror a decision without an outline change (see [`describe_request_decision`]); the caller
/// holds the write lock since before the decision. Errors are also on stderr.
pub async fn record_request_decision(
    state: &AppState,
    held: &WriteGuard<'_>,
    request: &baruch_store::OutlineRequest,
) -> Result<Option<String>, String> {
    let result = commit(state, held, request.work, describe_request_decision(request)).await;
    if let Err(e) = &result {
        eprintln!("Git-Spiegel: {e}");
    }
    result
}

/// Bring the mirror of a work up to the state of the database, as a commit of the human with the subject
/// `summary` (start: „Stand der Datenbank gespiegelt“, import: „Import aus <datei>“). Without a change
/// no commit (`Ok(None)`). Errors are additionally on stderr.
pub async fn sync_work(
    state: &AppState,
    held: &WriteGuard<'_>,
    work: WorkId,
    summary: &str,
) -> Result<Option<String>, String> {
    crate::rag::notify(state, Some(work));
    let request = CommitRequest {
        files: Vec::new(),
        author: human(HUMAN_NAME),
        committer: human(HUMAN_NAME),
        summary: summary.to_owned(),
        body: String::new(),
        trailers: Vec::new(),
        allow_empty: false,
    };
    let result = commit_revisions(state, held, work, request).await;
    if let Err(e) = &result {
        eprintln!("Git-Spiegel (Werk {work}): {e}");
    }
    result
}

/// Bring the mirrors of all works up to date (at start). Errors only on stderr.
pub async fn sync_all(state: &AppState) {
    let works = match state.store.works().await {
        Ok(works) => works,
        Err(e) => {
            eprintln!("Git-Spiegel beim Start nicht nachgeführt: {e}");
            return;
        }
    };
    let held = state.write_lock.lock().await;
    for (work, _, _) in works {
        let _ = sync_work(state, &held, work, "Stand der Datenbank gespiegelt").await;
    }
    let _ = crate::mirror_ideas::sync_ideas_without_work(state, &held, "Stand der Datenbank gespiegelt").await;
}

/// Convert old part texts once at start to the part marker (user question 8, decided
/// 2026-11-07; rules in `baruch_store::legacy_parts`). Runs after [`sync_all`], so that the commit per work
/// shows exactly the conversion: subject „Umwandlung Runde 14 (Nutzerentscheidung): n Teile …“, body with
/// part and revisions, trailers `Baruch-Action: parts-converted` and `Baruch-Origin`. Parts with their own
/// text stay and appear on stderr only with count and node id (no wording). Errors only on
/// stderr; the start goes on.
pub async fn convert_legacy_parts(state: &AppState) -> Option<baruch_store::PartConversion> {
    let held = state.write_lock.lock().await;
    let result = match state.store.convert_legacy_parts(HUMAN_NAME).await {
        Ok(result) => result,
        Err(e) => {
            eprintln!("Umwandlung der Teile (Runde 14) gescheitert, nichts geändert: {e}");
            return None;
        }
    };
    if !result.ran {
        return Some(result);
    }
    let mut works: Vec<WorkId> = result.converted.iter().map(|c| c.work).collect();
    works.dedup();
    for work in works {
        let parts: Vec<&baruch_store::ConvertedPart> = result.converted.iter().filter(|c| c.work == work).collect();
        crate::rag::notify(state, Some(work));
        let _ = commit_part_conversion(state, &held, work, &parts).await;
    }
    if result.converted.is_empty() && result.kept.is_empty() {
        return Some(result);
    }
    eprintln!(
        "{}: {} Teile auf die Teil-Markierung umgestellt; {} Teile mit eigenem Text unverändert{}",
        baruch_store::CONVERSION_NOTE,
        result.converted.len(),
        result.kept.len(),
        if result.kept.is_empty() {
            String::new()
        } else {
            format!(" (Knoten-ids {})", result.kept.iter().map(ToString::to_string).collect::<Vec<_>>().join(", "))
        }
    );
    Some(result)
}

/// Commit request without files for the conversion of the parts of a work.
pub fn describe_part_conversion(parts: &[&baruch_store::ConvertedPart]) -> CommitRequest {
    let count = parts.len();
    let noun = if count == 1 { "Teil" } else { "Teile" };
    let summary = format!("{}: {count} {noun} auf die Teil-Markierung umgestellt", baruch_store::CONVERSION_NOTE);
    let body = parts
        .iter()
        .map(|p| {
            let before = if p.was_empty { "leer" } else { "alte =-Zeile" };
            format!("- {} (Knoten {}): Fassung {} ({before}) → Fassung {}", p.title, p.seq, p.from, p.to)
        })
        .collect::<Vec<_>>()
        .join("\n");
    let mut trailers = vec![
        ("Baruch-Action".to_owned(), "parts-converted".to_owned()),
        ("Baruch-Origin".to_owned(), baruch_store::CONVERSION_NOTE.to_owned()),
    ];
    trailers.extend(parts.iter().map(|p| ("Baruch-Node".to_owned(), p.seq.to_string())));
    CommitRequest {
        files: Vec::new(),
        author: human(HUMAN_NAME),
        committer: human(HUMAN_NAME),
        summary,
        body,
        trailers,
        // Even if the mirror changes no file (log of the decision).
        allow_empty: true,
    }
}

async fn commit_part_conversion(
    state: &AppState,
    held: &WriteGuard<'_>,
    work: WorkId,
    parts: &[&baruch_store::ConvertedPart],
) -> Result<Option<String>, String> {
    let result = commit(state, held, work, describe_part_conversion(parts)).await;
    if let Err(e) = &result {
        eprintln!("Git-Spiegel (Werk {work}): {e}");
    }
    result
}

/// Bring the mirror of the default work up to date (as at start; without a change no commit).
pub async fn sync(state: &AppState) -> Result<Option<String>, String> {
    let held = state.write_lock.lock().await;
    let work = state.default_work().await.map_err(|e| e.to_string())?;
    sync_work(state, &held, work, "Stand der Datenbank gespiegelt").await
}

/// Commit request without files for an image change (contract Round 6, section 4f): subject
/// „Bild <name>: hinzugefügt / Fassung n ersetzt / auf Fassung n umgeschaltet / Fassung n geändert /
/// Fassung n gelöscht / gelöscht“, author and committer the human, trailers `Baruch-Asset`,
/// `Baruch-Revision` and `Baruch-Action: asset-*`. Changing carries before/after of caption,
/// alternative text and note (`after` = the revision after the change), deleting the metadata and the
/// deleted content (Base64 or SVG in lines of 76 characters), so that it stays restorable from
/// Git. Switching, changing and deleting commit even without a file change (`allow_empty`).
pub fn describe_asset(
    change: &baruch_store::AssetChange,
    after: Option<&baruch_store::AssetRevisionInfo>,
) -> CommitRequest {
    use baruch_store::AssetChangeKind as Kind;
    let (name, rev) = (&change.name, change.rev);
    let (what, action) = match change.kind {
        Kind::Added => ("hinzugefügt".to_owned(), "asset-added"),
        Kind::Replaced => (format!("Fassung {rev} ersetzt"), "asset-replaced"),
        Kind::Switched => (format!("auf Fassung {rev} umgeschaltet"), "asset-switched"),
        Kind::Updated => (format!("Fassung {rev} geändert"), "asset-updated"),
        Kind::Deleted => (format!("Fassung {rev} gelöscht"), "asset-deleted"),
        Kind::Removed => ("gelöscht".to_owned(), "asset-removed"),
    };
    let mut body = change.detail.clone();
    let push = |body: &mut String, text: &str| {
        if !body.is_empty() && !body.ends_with("\n\n") {
            if !body.ends_with('\n') {
                body.push('\n');
            }
            body.push('\n');
        }
        body.push_str(text);
    };
    if let (Kind::Updated, Some(before), Some(after)) = (change.kind, &change.before, after) {
        // If before/after is not already in the detail (paths alone), add it here.
        if !body.contains("Bildunterschrift vorher") {
            let mut text = String::new();
            for (label, old, new) in [
                ("Bildunterschrift", &before.caption, &after.caption),
                ("Alternativtext", &before.alt, &after.alt),
                ("Notiz", &before.note, &after.note),
            ] {
                text.push_str(&format!("{label} vorher: {}\n{label} nachher: {}\n", one_line(old), one_line(new)));
            }
            push(&mut body, &text);
        }
    }
    if let (Kind::Deleted | Kind::Removed, Some(before)) = (change.kind, &change.before) {
        let meta = format!(
            "Gelöschte Fassung {}: {}, {} Bytes, SHA-256 {}\nAutor: {}\nErstellt: {}\nBildunterschrift: {}\nAlternativtext: {}\nNotiz: {}\n",
            before.number,
            before.mime,
            before.bytes,
            before.sha256,
            before.author,
            before.created,
            one_line(&before.caption),
            one_line(&before.alt),
            one_line(&before.note),
        );
        push(&mut body, &meta);
    }
    if let Some(removed) = &change.removed {
        // Base64 in lines of 76 characters (ASCII); SVG verbatim, so that it can be restored unchanged.
        let (label, text) = match removed.encoding {
            baruch_store::AssetEncoding::Base64 => {
                let mut wrapped = String::new();
                for chunk in removed.content.as_bytes().chunks(76) {
                    wrapped.push_str(std::str::from_utf8(chunk).unwrap_or(""));
                    wrapped.push('\n');
                }
                ("Base64", wrapped)
            }
            baruch_store::AssetEncoding::Svg => {
                let mut text = removed.content.clone();
                if !text.ends_with('\n') {
                    text.push('\n');
                }
                ("SVG", text)
            }
        };
        push(&mut body, &format!("Gelöschter Inhalt ({label}, Fassung {}):\n{text}", removed.rev));
    }
    let trailers = vec![
        ("Baruch-Asset".to_owned(), name.clone()),
        ("Baruch-Revision".to_owned(), rev.to_string()),
        ("Baruch-Action".to_owned(), action.to_owned()),
    ];
    CommitRequest {
        files: Vec::new(),
        author: human(HUMAN_NAME),
        committer: human(HUMAN_NAME),
        summary: format!("Bild {name}: {what}"),
        body,
        trailers,
        allow_empty: matches!(change.kind, Kind::Switched | Kind::Updated | Kind::Deleted | Kind::Removed),
    }
}

/// Who caused an image change (Round 11).
#[derive(Debug, Clone, Copy)]
pub enum AssetBy<'a> {
    /// The human himself (user interface, RPC).
    Human,
    /// A persona or an MCP client directly (creating only, MCP `asset_add`): it is the author of the commit,
    /// the human the committer; trailers `Baruch-Origin` and `Persona`.
    Direct(&'a baruch_store::IdeaOrigin),
    /// An image request that the human confirmed: author and committer the human (confirmer), trailers
    /// `Baruch-Origin` (origin of the request), `Baruch-Request`, `Baruch-Requested-By`,
    /// `Baruch-Confirmed-By`; the body names request, requester, confirmer and justification.
    Confirmed(&'a baruch_store::AssetRequest),
}

/// Commit request of an image change by origin: [`describe_asset`], plus author and trailers as for
/// the outline ([`describe_outline_by`]).
pub fn describe_asset_by(
    change: &baruch_store::AssetChange,
    after: Option<&baruch_store::AssetRevisionInfo>,
    by: AssetBy<'_>,
) -> CommitRequest {
    use baruch_store::IdeaOrigin;
    let mut request = describe_asset(change, after);
    match by {
        AssetBy::Human => {}
        AssetBy::Direct(origin) => {
            request.trailers.push(("Baruch-Origin".to_owned(), origin.kind().to_owned()));
            if let IdeaOrigin::Persona { handle } | IdeaOrigin::Mcp { handle } = origin {
                request.trailers.push(("Persona".to_owned(), handle.clone()));
                request.author = Signature {
                    name: handle.clone(),
                    email: format!("{}@persona.baruch.invalid", email_local_part(handle)),
                };
            }
        }
        AssetBy::Confirmed(asked) => {
            let confirmer = asked.decided_by.clone().unwrap_or_else(|| HUMAN_NAME.to_owned());
            let mut head = format!(
                "Antrag {} von {}, bestätigt von {confirmer}: {}\n",
                asked.id,
                asked.origin.name(),
                asked.summary
            );
            if !asked.note.is_empty() {
                head.push_str(&format!("Begründung: {}\n", one_line(&asked.note)));
            }
            if !request.body.is_empty() {
                head.push('\n');
            }
            request.body = format!("{head}{}", request.body);
            request.summary.push_str(&format!(" (Antrag {})", asked.id));
            request.author = human(&confirmer);
            request.trailers.push(("Baruch-Origin".to_owned(), asked.origin.kind().to_owned()));
            request.trailers.push(("Baruch-Request".to_owned(), asked.id.to_string()));
            request.trailers.push(("Baruch-Requested-By".to_owned(), asked.origin.name().to_owned()));
            request.trailers.push(("Baruch-Confirmed-By".to_owned(), confirmer));
        }
    }
    request
}

/// Mirror an image change of the human (see [`record_asset_by`]).
pub async fn record_asset(
    state: &AppState,
    held: &WriteGuard<'_>,
    change: &baruch_store::AssetChange,
) -> Result<Option<String>, String> {
    record_asset_by(state, held, change, AssetBy::Human).await
}

/// Mirror an image change: exactly one commit (see [`describe_asset_by`]); for a persona the
/// author is named „<Name> (<handle>)“, for an MCP client „MCP-Client (<handle>)“. As with [`record`] the
/// caller holds the write lock since before the database write; errors are also on stderr.
pub async fn record_asset_by(
    state: &AppState,
    held: &WriteGuard<'_>,
    change: &baruch_store::AssetChange,
    by: AssetBy<'_>,
) -> Result<Option<String>, String> {
    let after = match change.kind {
        baruch_store::AssetChangeKind::Updated => state
            .store
            .asset_revisions(change.work, &change.name)
            .await
            .ok()
            .and_then(|list| list.into_iter().find(|r| r.number == change.rev)),
        _ => None,
    };
    let mut request = describe_asset_by(change, after.as_ref(), by);
    name_persona_author(state, &mut request).await;
    let result = commit(state, held, change.work, request).await;
    if let Err(e) = &result {
        eprintln!("Git-Spiegel: {e}");
    }
    result
}

/// Log commit for a decided image request without an image change of its own (confirmed cover:
/// the cover itself is mirrored by `work.set_cover`). Subject „Bildantrag <id> bestätigt: <Zusammenfassung>“,
/// author the confirmer, trailers as for [`AssetBy::Confirmed`] and `Baruch-Action:
/// asset-request-confirmed`; always one commit (`allow_empty`).
pub fn describe_asset_request(request: &baruch_store::AssetRequest) -> CommitRequest {
    let confirmer = request.decided_by.clone().unwrap_or_else(|| HUMAN_NAME.to_owned());
    let mut body = format!(
        "Antrag {} von {}, bestätigt von {confirmer}: {}\n",
        request.id,
        request.origin.name(),
        request.summary
    );
    if !request.note.is_empty() {
        body.push_str(&format!("Begründung: {}\n", one_line(&request.note)));
    }
    let mut trailers = vec![("Baruch-Action".to_owned(), "asset-request-confirmed".to_owned())];
    if !request.op.asset_name().is_empty() {
        trailers.push(("Baruch-Asset".to_owned(), request.op.asset_name().to_owned()));
    }
    trailers.push(("Baruch-Origin".to_owned(), request.origin.kind().to_owned()));
    trailers.push(("Baruch-Request".to_owned(), request.id.to_string()));
    trailers.push(("Baruch-Requested-By".to_owned(), request.origin.name().to_owned()));
    trailers.push(("Baruch-Confirmed-By".to_owned(), confirmer.clone()));
    CommitRequest {
        files: Vec::new(),
        author: human(&confirmer),
        committer: human(HUMAN_NAME),
        summary: format!("Bildantrag {} bestätigt: {}", request.id, request.summary),
        body,
        trailers,
        allow_empty: true,
    }
}

/// Mirror the log commit for a confirmed image request (see [`describe_asset_request`]).
pub async fn record_asset_request(
    state: &AppState,
    held: &WriteGuard<'_>,
    request: &baruch_store::AssetRequest,
) -> Result<Option<String>, String> {
    let result = commit(state, held, request.work, describe_asset_request(request)).await;
    if let Err(e) = &result {
        eprintln!("Git-Spiegel: {e}");
    }
    result
}

// ---- Exchange (Round 26) ---------------------------------------------------------------------------

/// Commit request for reading in an exchange file (Round 26, contract F6): exactly one commit with the origin
/// „Austausch aus <Instanz>“ (instance name, otherwise the first 8 characters of the identifier). Subject
/// „Austausch aus <Instanz>: Werk neu eingelesen“ or „…: abgeglichen“, body with the numbers, requests,
/// deviations and the checksum of the file; author „Austausch aus <Instanz>“, committer the human; trailers
/// `Baruch-Action: exchange-import`, `Baruch-Origin: exchange`, `Baruch-Exchange-Mode`,
/// `Baruch-Exchange-Instance`, `Baruch-Exchange-File`. Always one commit (`allow_empty`), even if no file of the
/// mirror changes (e.g. only requests).
pub fn describe_exchange(applied: &baruch_store::exchange_apply::ExchangeApplied) -> CommitRequest {
    use baruch_store::exchange_apply::{ApplyMode, DeviationAction};
    let report = &applied.report;
    // The reader already cleans the name; here once more, because Git refuses `<`, `>` and line breaks in the author.
    let name = baruch_exchange::clean_instance_name(&report.instance.name);
    let instance = if name.is_empty() { report.instance.uuid.chars().take(8).collect() } else { name };
    let origin = format!("Austausch aus {instance}");
    let what = match applied.mode {
        ApplyMode::New => format!("Werk „{}“ neu eingelesen", applied.title),
        ApplyMode::Merge => format!("„{}“ abgeglichen", applied.title),
    };
    let add = &report.add;
    let mut body = format!(
        "Hinzu: {} Punkte, {} Fassungen, {} Vorlagenfassungen, {} Einfälle, {} Beiträge, {} Fäden, {} Fadenbeiträge, \
         {} Bilder, {} Bildfassungen, {} Umschlag-Blöcke.\n",
        add.nodes,
        add.revisions,
        add.template_revisions,
        add.ideas,
        add.posts,
        add.threads,
        add.thread_posts,
        add.assets,
        add.asset_revisions,
        add.cover_blocks
    );
    if !report.requests.is_empty() {
        body.push_str(&format!("Anträge ({}):\n", report.requests.len()));
        for request in &report.requests {
            body.push_str(&format!("- {}\n", one_line(&request.label)));
        }
    }
    let count = |action| report.deviations.iter().filter(|d| d.action == action).count();
    body.push_str(&format!(
        "Abweichungen: {} weitere Fassungen, {} Anträge, {} Hinweise; hier gelöscht: {}.\n",
        count(DeviationAction::Fassung),
        count(DeviationAction::Antrag),
        count(DeviationAction::Hinweis),
        report.deleted_here.len()
    ));
    if !report.unknown.is_empty() {
        body.push_str(&format!("Unbekannt (nicht übernommen): {}\n", report.unknown.join(", ")));
    }
    if !report.refusals.is_empty() {
        body.push_str("Nicht übernommen:\n");
        for refusal in &report.refusals {
            body.push_str(&format!("- {}\n", one_line(refusal)));
        }
    }
    body.push_str(&format!("Instanz: {} ({})\nDatei: {}\n", instance, report.instance.uuid, report.sha256));
    CommitRequest {
        files: Vec::new(),
        author: human(&origin),
        committer: human(HUMAN_NAME),
        summary: format!("{origin}: {what}"),
        body,
        trailers: vec![
            ("Baruch-Action".to_owned(), "exchange-import".to_owned()),
            ("Baruch-Origin".to_owned(), "exchange".to_owned()),
            ("Baruch-Exchange-Mode".to_owned(), applied.mode.as_str().to_owned()),
            ("Baruch-Exchange-Instance".to_owned(), report.instance.uuid.clone()),
            ("Baruch-Exchange-File".to_owned(), report.sha256.clone()),
        ],
        allow_empty: true,
    }
}

/// Mirror an exchange file that was read in (see [`describe_exchange`]); the caller holds the write lock since
/// before the reading in. Reports the change to the semantic search like the Typst import
/// ([`crate::rag::notify`]). Errors are also on stderr.
pub async fn record_exchange(
    state: &AppState,
    held: &WriteGuard<'_>,
    applied: &baruch_store::exchange_apply::ExchangeApplied,
) -> Result<Option<String>, String> {
    crate::rag::notify(state, Some(applied.work));
    let result = commit_revisions(state, held, applied.work, describe_exchange(applied)).await;
    if let Err(e) = &result {
        eprintln!("Git-Spiegel (Austausch, Werk {}): {e}", applied.work);
    }
    result
}

// ---- Ideas (contract Round 7, section 6c) --------------------------------------------------------

/// Folder of the Git mirror of the ideas without a work: `<data>/einfaelle`.
pub const IDEAS_DIR: &str = "einfaelle";

/// Commit request without files for an action on an idea. `action` is the value of the trailer
/// `Baruch-Action` (`idea-create`, `idea-update`, `idea-delete`, `idea-restore`, `idea-process`),
/// `detail` an addition for the body (e.g. the changed fields or the result of the processing).
/// Subject „Einfall <id>: angelegt|geändert|gelöscht|wiederhergestellt|verarbeitet als <…>“, trailers
/// `Baruch-Idea`, `Baruch-Action`, `Baruch-Origin` and for persona/MCP `Persona: <handle>`; then
/// the author is the persona or the client (`<handle>@persona.baruch.invalid`), the committer always the
/// human. The delete commit carries the whole entry in the body (restorable). Always one commit
/// (`allow_empty`), even if no file changes.
pub fn describe_idea(idea: &baruch_store::Idea, action: &str, detail: &str) -> CommitRequest {
    use baruch_store::IdeaOrigin;
    let what = match action {
        "idea-create" => "angelegt".to_owned(),
        "idea-update" => "geändert".to_owned(),
        "idea-delete" => "gelöscht".to_owned(),
        "idea-restore" => "wiederhergestellt".to_owned(),
        "idea-process" => match idea.done_as {
            Some(used) => format!("verarbeitet als {}", used.label()),
            None => "verarbeitet".to_owned(),
        },
        other => other.to_owned(),
    };
    let mut body = detail.trim_end().to_owned();
    if action == "idea-delete" {
        if !body.is_empty() {
            body.push_str("\n\n");
        }
        body.push_str("Gelöschter Eintrag:\n");
        body.push_str(&idea.entry_markdown());
    }
    let mut trailers = vec![
        ("Baruch-Idea".to_owned(), idea.id.to_string()),
        ("Baruch-Action".to_owned(), action.to_owned()),
        ("Baruch-Origin".to_owned(), idea.origin.kind().to_owned()),
    ];
    let author = match &idea.origin {
        IdeaOrigin::Human { name } => human(name),
        IdeaOrigin::Persona { handle } | IdeaOrigin::Mcp { handle } => {
            trailers.push(("Persona".to_owned(), handle.clone()));
            Signature { name: handle.clone(), email: format!("{}@persona.baruch.invalid", email_local_part(handle)) }
        }
    };
    CommitRequest {
        files: Vec::new(),
        author,
        committer: human(HUMAN_NAME),
        summary: format!("Einfall {}: {what}", idea.id),
        body,
        trailers,
        allow_empty: true,
    }
}

/// Git mirror of the ideas without a work, `<data>/einfaelle/`; `<data>/einfaelle` must not be a
/// symbolic link.
fn ideas_dir(data: &std::path::Path) -> Result<std::path::PathBuf, String> {
    let dir = data.join(IDEAS_DIR);
    if std::fs::symlink_metadata(&dir).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(format!("{} ist eine symbolische Verknüpfung; der Git-Spiegel schreibt dort nicht", dir.display()));
    }
    Ok(dir)
}

/// Mirror an action on an idea: exactly one commit (see [`describe_idea`]) in the mirror of the work
/// `work` (snapshot with `einfaelle.md`) or, without a work, in the folder `<data>/einfaelle/` (only
/// `einfaelle.md` of the ideas without a work). For a persona the author is named as with proposals
/// „<Name> (<handle>)“. As with [`record`] the caller holds the write lock since before the
/// database write; errors are also on stderr.
pub async fn record_idea(
    state: &AppState,
    held: &WriteGuard<'_>,
    work: Option<WorkId>,
    mut request: CommitRequest,
) -> Result<Option<String>, String> {
    crate::rag::notify(state, work);
    name_persona_author(state, &mut request).await;
    let result = match work {
        Some(work) => commit(state, held, work, request).await,
        None => commit_ideas_without_work(state, request).await,
    };
    if let Err(e) = &result {
        eprintln!("Git-Spiegel (Einfälle): {e}");
    }
    result
}

/// If there is a trailer `Persona` in the request, the author is named „<Name> (<handle>)“ or, for an
/// MCP client without a persona entry, „MCP-Client (<handle>)“ (as with proposals).
async fn name_persona_author(state: &AppState, request: &mut CommitRequest) {
    if let Some(handle) = request.trailers.iter().find(|(k, _)| k == "Persona").map(|(_, v)| v.clone()) {
        let name: Option<String> = sqlx::query_scalar("SELECT name FROM persona WHERE handle = ?")
            .bind(&handle)
            .fetch_optional(state.store.pool())
            .await
            .ok()
            .flatten();
        request.author.name = match name {
            Some(name) => format!("{name} ({handle})"),
            None => format!("MCP-Client ({handle})"),
        };
    }
}

/// Action on a post (contract Round 8, 3d).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostAction {
    /// Deleted (`post-delete`).
    Delete,
    /// Moved into another section (`post-move`).
    Move,
    /// Converted into an idea (`post-to-idea`).
    ToIdea,
    /// Edited (`post-edit`, Round 28): body with the text before and after.
    Edit,
}

impl PostAction {
    /// Value of the trailer `Baruch-Action`.
    pub fn trailer(self) -> &'static str {
        match self {
            PostAction::Delete => "post-delete",
            PostAction::Move => "post-move",
            PostAction::ToIdea => "post-to-idea",
            PostAction::Edit => "post-edit",
        }
    }
}

/// „<Nummer> <Titel>“ of a section (without a number only the title).
fn node_label(work: &Work, node: usize) -> String {
    match work.nodes.get(node) {
        Some(n) if n.number.is_empty() => n.title.clone(),
        Some(n) => format!("{} {}", n.number, n.title),
        None => format!("Abschnitt {node}"),
    }
}

/// Commit request without files for an action on a post (contract Round 8, 3d):
/// subject „Beitrag <id>: gelöscht“, „…: verschoben nach <Nummer Titel>“ or „…: in Einfall <n>
/// umgewandelt“; trailers `Baruch-Post`, `Baruch-Action` (post-delete|post-move|post-to-idea),
/// `Baruch-Node` (fixed key of the section in which the post now stands or stood), for the
/// conversion `Baruch-Idea`. Deleting and converting carry the whole post
/// ([`baruch_store::PostInfo::entry_markdown`]) in the body, so that it stays restorable from the
/// history; moving names origin and target. Always one commit (`allow_empty`). The author is whoever
/// acts: the human, a persona or an MCP client (`actor`, as with ideas). `post` is the
/// post *after* the action (when moving, at the target), `from` when moving and editing (Round 28:
/// subject „Beitrag <id>: bearbeitet“, body „Vorher:“/„Nachher:“) the state before;
/// `work` is the state after the action.
pub fn describe_post(
    work: &Work,
    post: &baruch_store::PostInfo,
    action: PostAction,
    from: Option<&baruch_store::PostInfo>,
    idea: Option<i64>,
    actor: &baruch_store::IdeaOrigin,
) -> CommitRequest {
    use baruch_store::IdeaOrigin;
    let (what, body) = match action {
        PostAction::Delete => ("gelöscht".to_owned(), format!("Gelöschter Beitrag:\n{}", post.entry_markdown())),
        PostAction::Move => {
            let to = node_label(work, post.node);
            let body = match from {
                Some(from) => format!(
                    "Von Abschnitt {} (key {}) nach {} (key {}), dort an Stelle {}.\n",
                    from.node, from.key, to, post.key, post.position
                ),
                None => String::new(),
            };
            (format!("verschoben nach {to}"), body)
        }
        PostAction::Edit => {
            let before = from.map_or("", |f| f.body.as_str());
            ("bearbeitet".to_owned(), format!("Vorher:\n{before}\n\nNachher:\n{}", post.body))
        }
        PostAction::ToIdea => {
            let n = idea.unwrap_or_default();
            (
                format!("in Einfall {n} umgewandelt"),
                format!(
                    "Als Einfall {n} festgehalten (einfaelle.md); der Beitrag ist gelöscht:\n{}",
                    post.entry_markdown()
                ),
            )
        }
    };
    let mut trailers = vec![
        ("Baruch-Post".to_owned(), post.id.to_string()),
        ("Baruch-Action".to_owned(), action.trailer().to_owned()),
        ("Baruch-Node".to_owned(), post.key.to_string()),
    ];
    if let Some(idea) = idea {
        trailers.push(("Baruch-Idea".to_owned(), idea.to_string()));
    }
    let author = match actor {
        IdeaOrigin::Human { name } => human(name),
        IdeaOrigin::Persona { handle } | IdeaOrigin::Mcp { handle } => {
            trailers.push(("Persona".to_owned(), handle.clone()));
            Signature { name: handle.clone(), email: format!("{}@persona.baruch.invalid", email_local_part(handle)) }
        }
    };
    CommitRequest {
        files: Vec::new(),
        author,
        committer: human(HUMAN_NAME),
        summary: format!("Beitrag {}: {what}", post.id),
        body,
        trailers,
        allow_empty: true,
    }
}

/// Mirror an action on a post: exactly one commit in the mirror of the work (snapshot, so for the
/// conversion already with the new entry in `einfaelle.md`). As with [`record`] the
/// caller holds the write lock since before the database write; errors are also on stderr.
pub async fn record_post(
    state: &AppState,
    held: &WriteGuard<'_>,
    work: WorkId,
    mut request: CommitRequest,
) -> Result<Option<String>, String> {
    crate::rag::notify(state, Some(work));
    name_persona_author(state, &mut request).await;
    let result = commit(state, held, work, request).await;
    if let Err(e) = &result {
        eprintln!("Git-Spiegel (Beiträge): {e}");
    }
    result
}

/// Mirror a finished commit message of a module (Round 30, interface R30-S1): exactly one commit in the mirror
/// of the work, like [`record_post`] (name the persona author, errors also on stderr). This way every module
/// (replace, cross references, solutions, chat) builds its message itself without changing `mirror.rs`. The
/// caller holds the write lock since before the database write.
pub async fn record_request(
    state: &AppState,
    held: &WriteGuard<'_>,
    work: WorkId,
    mut request: CommitRequest,
) -> Result<Option<String>, String> {
    crate::rag::notify(state, Some(work));
    name_persona_author(state, &mut request).await;
    let result = commit(state, held, work, request).await;
    if let Err(e) = &result {
        eprintln!("Git-Spiegel: {e}");
    }
    result
}

/// Action on a finding (Round 28).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindingAction {
    /// The human set the status (`finding-status`).
    Status,
    /// An assistant proposed a status (`finding-propose`).
    Proposed,
    /// The human decided on a proposal (`finding-decide`).
    Decided,
}

impl FindingAction {
    /// Value of the trailer `Baruch-Action`.
    pub fn trailer(self) -> &'static str {
        match self {
            FindingAction::Status => "finding-status",
            FindingAction::Proposed => "finding-propose",
            FindingAction::Decided => "finding-decide",
        }
    }
}

/// „bestätigt“, „abgelehnt“, „veraltet“ for `finding_status.decision`.
fn decision_word(decision: Option<&str>) -> &'static str {
    match decision {
        Some("confirmed") => "bestätigt",
        Some("rejected") => "abgelehnt",
        Some("stale") => "veraltet",
        _ => "offen",
    }
}

/// Status with revision or target in words: „übernommen in Fassung 3“, „verschoben nach 1.2 Titel“.
fn finding_status_text(work: &Work, entry: &baruch_store::FindingStatusEntry) -> String {
    use baruch_store::{DeferTarget, FindingStatus};
    match entry.status {
        FindingStatus::Adopted => match entry.adopted_rev {
            Some(rev) => format!("übernommen in Fassung {rev}"),
            None => "übernommen (Fassung fehlt)".to_owned(),
        },
        FindingStatus::Deferred => match &entry.deferred {
            Some(DeferTarget::Node { key }) => match work.nodes.iter().find(|n| n.key == *key) {
                Some(n) => format!("verschoben nach {}", node_label(work, n.id)),
                None => format!("verschoben nach Abschnitt (key {key})"),
            },
            Some(DeferTarget::Idea { id }) => format!("verschoben nach Einfall {id}"),
            None => "verschoben (Ziel fehlt)".to_owned(),
        },
        other => other.label().to_owned(),
    }
}

/// Commit request without files for an action on a finding (Round 28, contract §1.2): subject „Befund <id>: …“
/// (set: the status, e.g. „verworfen“ or „übernommen in Fassung 3“; proposal: „Vorschlag „…“ von @x“;
/// decision: „Vorschlag „…“ bestätigt|abgelehnt|veraltet“); body with kind, persona, place, text of the finding,
/// justification and note; trailers `Baruch-Finding`, `Baruch-Action` (finding-status|finding-propose|finding-decide),
/// `Baruch-Node` (fixed key if the finding has a section), for a proposal `Persona` (the author is
/// then the persona or the MCP client), for a decision `Baruch-Proposer` (the author is the human who
/// decided). Always one commit (`allow_empty`). `work` is the state after the action, `entry` the affected
/// entry in the history.
pub fn describe_finding(
    work: &Work,
    finding: &baruch_store::Finding,
    entry: &baruch_store::FindingStatusEntry,
    action: FindingAction,
) -> CommitRequest {
    use baruch_store::IdeaOrigin;
    let mut trailers = vec![
        ("Baruch-Finding".to_owned(), finding.id.to_string()),
        ("Baruch-Action".to_owned(), action.trailer().to_owned()),
    ];
    if let Some(key) = finding.key {
        trailers.push(("Baruch-Node".to_owned(), key.to_string()));
    }
    let what = finding_status_text(work, entry);
    let summary = match action {
        FindingAction::Status => format!("Befund {}: {what}", finding.id),
        FindingAction::Proposed => format!("Befund {}: Vorschlag „{what}“ von {}", finding.id, entry.actor.name()),
        FindingAction::Decided => {
            format!("Befund {}: Vorschlag „{what}“ {}", finding.id, decision_word(entry.decision.as_deref()))
        }
    };
    let place = match finding.node {
        Some(seq) => node_label(work, seq),
        None if finding.thread.is_some() => "Faden ohne Abschnitt".to_owned(),
        None => "ohne Abschnitt".to_owned(),
    };
    let quoted: Vec<String> = finding.body.lines().map(|l| format!("> {l}")).collect();
    let mut body = format!(
        "[B{}] {} von {} ({place}):\n{}\n",
        finding.id,
        finding.kind.label(),
        finding.persona,
        quoted.join("\n")
    );
    if !entry.reason.is_empty() {
        body.push_str(&format!("\nBegründung: {}\n", one_line(&entry.reason)));
    }
    if !entry.note.is_empty() {
        body.push_str(&format!("\nNotiz: {}\n", one_line(&entry.note)));
    }
    let author = match (action, &entry.actor) {
        (FindingAction::Decided, actor) => {
            body.push_str(&format!(
                "\nVorschlag {} von {}: {}\n",
                entry.id,
                actor.name(),
                decision_word(entry.decision.as_deref())
            ));
            trailers.push(("Baruch-Proposer".to_owned(), actor.name().to_owned()));
            human(entry.decided_by.as_deref().unwrap_or(HUMAN_NAME))
        }
        (_, IdeaOrigin::Human { name }) => human(name),
        (_, IdeaOrigin::Persona { handle } | IdeaOrigin::Mcp { handle }) => {
            trailers.push(("Persona".to_owned(), handle.clone()));
            Signature { name: handle.clone(), email: format!("{}@persona.baruch.invalid", email_local_part(handle)) }
        }
    };
    body.push_str(&format!("\nStatus jetzt: {}\n", finding.status.label()));
    CommitRequest {
        files: Vec::new(),
        author,
        committer: human(HUMAN_NAME),
        summary,
        body,
        trailers,
        allow_empty: true,
    }
}

/// Mirror an action on a finding: exactly one commit in the mirror of the work (like [`record_post`]). The caller
/// holds the write lock since before the database write; errors are also on stderr.
pub async fn record_finding(
    state: &AppState,
    held: &WriteGuard<'_>,
    work: WorkId,
    mut request: CommitRequest,
) -> Result<Option<String>, String> {
    name_persona_author(state, &mut request).await;
    let result = commit(state, held, work, request).await;
    if let Err(e) = &result {
        eprintln!("Git-Spiegel (Befunde): {e}");
    }
    result
}

/// Commit request without files for a finding that a Lektor writes himself (Round 35, `finding.add`): subject
/// „Befund <id>: angelegt von @lektorat:<slug>“, body with kind, place and text (like [`describe_finding`]), trailers
/// `Baruch-Finding`, `Baruch-Action: finding-add`, `Baruch-Node` (if the finding has a section),
/// `Baruch-Origin: lektor` and `Baruch-Lektor` (handle). The author of the commit is the Lektor („<Anzeigename>
/// (<Handle>)“, `<slug>@lektorat.baruch.invalid`), the committer the author of the book as with all commits.
/// Deliberately without the trailer `Persona`: [`record_finding`] would otherwise take the Lektor for an MCP client.
/// Always one commit (`allow_empty`); `work` is the state after creating.
pub fn describe_finding_added(
    work: &Work,
    finding: &baruch_store::Finding,
    handle: &str,
    display: &str,
) -> CommitRequest {
    let mut trailers = vec![
        ("Baruch-Finding".to_owned(), finding.id.to_string()),
        ("Baruch-Action".to_owned(), "finding-add".to_owned()),
    ];
    if let Some(key) = finding.key {
        trailers.push(("Baruch-Node".to_owned(), key.to_string()));
    }
    trailers.push(("Baruch-Origin".to_owned(), "lektor".to_owned()));
    trailers.push(("Baruch-Lektor".to_owned(), handle.to_owned()));
    let place = match finding.node {
        Some(seq) => node_label(work, seq),
        None => "Anmerkung zum Buch, ohne Abschnitt".to_owned(),
    };
    let quoted: Vec<String> = finding.body.lines().map(|l| format!("> {l}")).collect();
    let mut body =
        format!("[B{}] {} von {handle} ({place}):\n{}\n", finding.id, finding.kind.label(), quoted.join("\n"));
    if !finding.quote.is_empty() {
        body.push_str(&format!("\nZitat: {}\n", one_line(&finding.quote)));
    }
    body.push_str(&format!("\nStatus jetzt: {}\n", finding.status.label()));
    let name = if display.trim().is_empty() { handle.to_owned() } else { format!("{} ({handle})", one_line(display)) };
    CommitRequest {
        files: Vec::new(),
        author: Signature { name, email: format!("{}@lektorat.baruch.invalid", email_local_part(handle)) },
        committer: human(HUMAN_NAME),
        summary: format!("Befund {}: angelegt von {handle}", finding.id),
        body,
        trailers,
        allow_empty: true,
    }
}

pub(crate) async fn commit_ideas_without_work(
    state: &AppState,
    mut request: CommitRequest,
) -> Result<Option<String>, String> {
    let file = state.store.idea_mirror_file(None).await.map_err(|e| e.to_string())?;
    request.files = file.into_iter().map(|f| MirrorFile { path: f.path, contents: f.contents }).collect();
    let dir = ideas_dir(&state.data)?;
    let shown = dir.display().to_string();
    tokio::task::spawn_blocking(move || {
        let mirror = baruch_gitmirror::Mirror::open_or_init(&dir).map_err(|e| format!("Git-Spiegel {shown}: {e}"))?;
        mirror.commit(&request).map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("Git: {e}"))?
}

// ---- Typeset settings (Round 29a) --------------------------------------------------------------

/// Kind of a change of the typeset settings (trailer `Baruch-Action`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypesetAction {
    /// Settings saved (`typeset-set`).
    Set,
    /// Settings reset (`typeset-reset`).
    Reset,
    /// Chapter start rule of the work template taken over; new template revision `template_rev` (`typeset-adopt`).
    Adopt { template_rev: u32 },
}

impl TypesetAction {
    /// Value of the trailer `Baruch-Action`.
    pub fn trailer(self) -> &'static str {
        match self {
            TypesetAction::Set => "typeset-set",
            TypesetAction::Reset => "typeset-reset",
            TypesetAction::Adopt { .. } => "typeset-adopt",
        }
    }
}

/// Number in German notation („10,5“).
fn de_number(x: f64) -> String {
    format!("{x}").replace('.', ",")
}

fn on_off(value: bool) -> &'static str {
    if value { "an" } else { "aus" }
}

/// Value of a field of the typeset settings in words; unset „aus der Werkvorlage“ (R31 fields: „keine“ or „aus“).
pub fn typeset_value(settings: &baruch_store::TypesetSettings, field: &str) -> String {
    use baruch_typst::typeset::PageFormat;
    let s = settings;
    let text = match field {
        "chapter_start" => s.chapter_start.map(|c| c.label().to_owned()),
        "part_page" => s
            .part_page
            .as_ref()
            .map(|p| format!("{}{}", p.side.label(), if p.blank_back { ", Rückseite leer" } else { "" })),
        "page_format" => s.page_format.map(|f| match f {
            PageFormat::Custom { width_mm, height_mm } => {
                format!("eigenes Format ({} × {} mm)", de_number(width_mm), de_number(height_mm))
            }
            fixed => fixed.label().to_owned(),
        }),
        "margins" => s.margins.as_ref().map(|m| {
            format!(
                "innen {}, außen {}, oben {}, unten {}, Bundsteg {} mm",
                de_number(m.inside_mm),
                de_number(m.outside_mm),
                de_number(m.top_mm),
                de_number(m.bottom_mm),
                de_number(m.binding_mm)
            )
        }),
        "font" => s.font.as_ref().map(|f| format!("„{f}“")),
        "font_size_pt" => s.font_size_pt.map(|x| format!("{} pt", de_number(x))),
        "leading_em" => s.leading_em.map(|x| format!("{} em", de_number(x))),
        "paragraph" => s.paragraph.map(|p| format!("{} {} em", p.label(), de_number(p.em()))),
        "justify" => s.justify.map(|v| on_off(v).to_owned()),
        "hyphenate" => s.hyphenate.map(|v| on_off(v).to_owned()),
        "lang" => s.lang.clone(),
        "page_numbers" => s.page_numbers.as_ref().map(|n| format!("{}, {}", n.position.label(), n.start.label())),
        "running_head" => s.running_head.map(|r| r.label().to_owned()),
        "footnotes" => s.footnotes.as_ref().map(|f| format!("{}, {}", f.numbering.label(), f.restart.label())),
        "widows_orphans" => s.widows_orphans.map(|v| (if v { "vermeiden" } else { "zulassen" }).to_owned()),
        "xref_with_page" => s.xref_with_page.map(|v| (if v { "mit Seitenzahl" } else { "ohne Seitenzahl" }).to_owned()),
        // Round 31: not a rule of the work template, hence „keine“ or „aus“ instead of „aus der Werkvorlage“.
        "glossary_page" => Some(s.glossary_page.map_or_else(|| "keine".to_owned(), |g| g.label().to_owned())),
        "index_pages" => Some(s.index_pages.as_ref().map_or_else(
            || "keine".to_owned(),
            |p| {
                let columns = if p.columns == 1 { "1 Spalte".to_owned() } else { format!("{} Spalten", p.columns) };
                format!("{}, {columns}", p.registers.join(", "))
            },
        )),
        "glossary_first_mention" => Some(on_off(s.glossary_first_mention == Some(true)).to_owned()),
        // Round 22: draft is not a rule of the work template, hence „an: …“ or „aus“.
        "draft" => Some(
            s.draft.as_ref().map_or_else(|| "aus".to_owned(), |d| format!("an: „{}“, {}", d.text, d.placement.label())),
        ),
        _ => None,
    };
    text.unwrap_or_else(|| "aus der Werkvorlage".to_owned())
}

/// Commit request without files for a change of the typeset settings (Round 29, contract §6.5): subject „Satz:
/// <Bezeichnungen> geändert“, „Satz: Einstellungen zurückgesetzt“ or „Satz: Kapitelanfang aus der Werkvorlage
/// übernommen (Vorlagenfassung n)“; body per changed field „Bezeichnung: vorher → nachher“; author and committer
/// the human; trailer `Baruch-Action` (typeset-set|typeset-reset|typeset-adopt). Always one commit (`allow_empty`):
/// the field „Verweis mit Seitenzahl“ changes no file, and `main.typ` carries the rule block through
/// `snapshot::head` by itself.
pub fn describe_typeset(change: &baruch_store::TypesetChange, action: TypesetAction) -> CommitRequest {
    let labels: Vec<&str> = change.changed.iter().map(|f| baruch_typst::typeset::field_label(f)).collect();
    let summary = match action {
        TypesetAction::Set => format!("Satz: {} geändert", labels.join(", ")),
        TypesetAction::Reset => "Satz: Einstellungen zurückgesetzt".to_owned(),
        TypesetAction::Adopt { template_rev } => {
            format!("Satz: Kapitelanfang aus der Werkvorlage übernommen (Vorlagenfassung {template_rev})")
        }
    };
    let mut body = String::new();
    for (field, label) in change.changed.iter().zip(&labels) {
        body.push_str(&format!(
            "{label}: {} → {}\n",
            typeset_value(&change.before, field),
            typeset_value(&change.after, field)
        ));
    }
    if let TypesetAction::Adopt { template_rev } = action {
        body.push_str(&format!(
            "\nDie Zeile mit der Regel für den Kapitelanfang steht nicht mehr in der Werkvorlage (Fassung {template_rev}).\n"
        ));
    }
    CommitRequest {
        files: Vec::new(),
        author: human(HUMAN_NAME),
        committer: human(HUMAN_NAME),
        summary,
        body,
        trailers: vec![("Baruch-Action".to_owned(), action.trailer().to_owned())],
        allow_empty: true,
    }
}

/// Mirror a change of the typeset settings: exactly one commit in the mirror of the work. The caller holds the
/// write lock since before the database write; errors are also on stderr.
pub async fn record_typeset(
    state: &AppState,
    held: &WriteGuard<'_>,
    work: WorkId,
    request: CommitRequest,
) -> Result<Option<String>, String> {
    let result = commit(state, held, work, request).await;
    if let Err(e) = &result {
        eprintln!("Git-Spiegel (Satz-Einstellungen): {e}");
    }
    result
}

// ---- Glossary and index (Round 31a) ------------------------------------------------------------

/// Author of a proposal: the persona or the MCP client (trailer `Persona`; [`name_persona_author`] sets the name).
fn proposer(handle: &str, trailers: &mut Vec<(String, String)>) -> Signature {
    trailers.push(("Persona".to_owned(), handle.to_owned()));
    Signature { name: handle.to_owned(), email: format!("{}@persona.baruch.invalid", email_local_part(handle)) }
}

/// A term as a body: term, variants, explanations, scripture references, source, status and origin.
fn term_body(term: &baruch_store::GlossaryTerm) -> String {
    let mut out = format!("Begriff: {}\n", term.term);
    if !term.variants.is_empty() {
        out.push_str(&format!("Varianten: {}\n", term.variants.join(", ")));
    }
    if !term.short.is_empty() {
        out.push_str(&format!("Kurz: {}\n", term.short));
    }
    if !term.long.is_empty() {
        out.push_str("Ausführlich:\n");
        for line in term.long.lines() {
            out.push_str(&format!("> {line}\n"));
        }
    }
    if !term.bible_refs.is_empty() {
        out.push_str(&format!("Bibelstellen: {}\n", term.bible_refs.join("; ")));
    }
    if !term.source.is_empty() {
        out.push_str(&format!("Quelle: {}\n", term.source));
    }
    let status = match term.status {
        baruch_store::GlossaryStatus::Accepted => "angenommen",
        baruch_store::GlossaryStatus::Proposed => "vorgeschlagen",
    };
    out.push_str(&format!("Status: {status}, Herkunft: {}\n", term.origin));
    out
}

/// Commit request without files for a change to the glossary (Round 31, contract §3.1): subject „Glossar: „<Begriff>“
/// angelegt|geändert|gelöscht|vorgeschlagen von <handle>|übernommen|verworfen“; body with the term (when changing
/// before and after, when deleting and discarding the whole term, so that it stays restorable from the
/// history); trailers `Baruch-Action` (glossary-add|glossary-update|glossary-delete|glossary-propose|glossary-accept|
/// glossary-reject) and `Baruch-Glossary` (uuid of the term); for the proposal the persona or the client is the
/// author (trailer `Persona`), for the decision the human with `Baruch-Proposer`. Always one commit (`allow_empty`).
pub fn describe_glossary(change: &baruch_store::GlossaryChange) -> CommitRequest {
    use baruch_store::GlossaryAction;
    let term = change.after.as_ref().or(change.before.as_ref());
    let name = term.map_or("?", |t| t.term.as_str());
    let (summary, action) = match change.action {
        GlossaryAction::Added => (format!("Glossar: „{name}“ angelegt"), "glossary-add"),
        GlossaryAction::Updated => {
            let old = change.before.as_ref().map_or(name, |t| t.term.as_str());
            if old == name {
                (format!("Glossar: „{name}“ geändert"), "glossary-update")
            } else {
                (format!("Glossar: „{old}“ geändert in „{name}“"), "glossary-update")
            }
        }
        GlossaryAction::Deleted => (format!("Glossar: „{name}“ gelöscht"), "glossary-delete"),
        GlossaryAction::Proposed => (format!("Glossar: „{name}“ vorgeschlagen von {}", change.by), "glossary-propose"),
        GlossaryAction::Accepted => (format!("Glossar: Vorschlag „{name}“ übernommen"), "glossary-accept"),
        GlossaryAction::Rejected => (format!("Glossar: Vorschlag „{name}“ verworfen"), "glossary-reject"),
    };
    let mut trailers = vec![("Baruch-Action".to_owned(), action.to_owned())];
    if let Some(term) = term {
        trailers.push(("Baruch-Glossary".to_owned(), term.uuid.clone()));
    }
    let body = match (change.action, &change.before, &change.after) {
        (GlossaryAction::Updated, Some(before), Some(after)) => {
            format!("Vorher:\n{}\nNachher:\n{}", term_body(before), term_body(after))
        }
        (_, _, Some(after)) => term_body(after),
        (_, Some(before), None) => term_body(before),
        _ => String::new(),
    };
    let author = match change.action {
        GlossaryAction::Proposed => proposer(&change.by, &mut trailers),
        GlossaryAction::Accepted | GlossaryAction::Rejected => {
            if let Some(origin) = change.before.as_ref().map(|t| t.origin.clone()) {
                trailers.push(("Baruch-Proposer".to_owned(), origin));
            }
            human(&change.by)
        }
        _ => human(&change.by),
    };
    CommitRequest {
        files: Vec::new(),
        author,
        committer: human(HUMAN_NAME),
        summary,
        body,
        trailers,
        allow_empty: true,
    }
}

/// What happened to an own index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegisterAction<'a> {
    /// Created (`register-add`).
    Added,
    /// Renamed; `old` is the title before (`register-rename`).
    Renamed { old: &'a str },
    /// Deleted (`register-delete`).
    Deleted,
}

/// Commit request for an own index: subject „Register: „<Titel>“ (<Kennung>) angelegt|umbenannt (vorher
/// „…“)|gelöscht“, author the human, trailers `Baruch-Action` and `Baruch-Register` (identifier). Always one commit.
pub fn describe_register(row: &baruch_store::IndexRegisterRow, action: RegisterAction<'_>) -> CommitRequest {
    let (summary, trailer) = match action {
        RegisterAction::Added => (format!("Register: „{}“ ({}) angelegt", row.title, row.slug), "register-add"),
        RegisterAction::Renamed { old } => {
            (format!("Register: „{}“ ({}) umbenannt (vorher „{old}“)", row.title, row.slug), "register-rename")
        }
        RegisterAction::Deleted => (format!("Register: „{}“ ({}) gelöscht", row.title, row.slug), "register-delete"),
    };
    CommitRequest {
        files: Vec::new(),
        author: human(HUMAN_NAME),
        committer: human(HUMAN_NAME),
        summary,
        body: format!("Kennung: {}\nTitel: {}\nuuid: {}\n", row.slug, row.title, row.uuid),
        trailers: vec![
            ("Baruch-Action".to_owned(), trailer.to_owned()),
            ("Baruch-Register".to_owned(), row.slug.clone()),
        ],
        allow_empty: true,
    }
}

/// What happened to an index proposal (without the accept, which [`describe_index_accept`] describes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexProposalAction {
    /// Created by a persona or a client (`index-propose`).
    Proposed,
    /// Discarded by the human (`index-reject`).
    Rejected,
}

/// Section of a proposal through its fixed key as „<Nummer> <Titel>“.
fn label_of_key(work: &Work, key: i64) -> String {
    work.nodes
        .iter()
        .find(|n| n.key == key)
        .map_or_else(|| format!("Abschnitt (key {key})"), |n| node_label(work, n.id))
}

/// An index proposal as a body: word, location, mark, justification.
fn index_body(work: &Work, proposal: &baruch_store::IndexProposal) -> String {
    let mark = &proposal.mark;
    let mut out = format!(
        "[R{}] {} in {} (Fassung {}): Wort „{}“, Fundstelle {}\nRegister: {}, Stichwort: {}\n",
        proposal.id,
        proposal.origin,
        label_of_key(work, proposal.key),
        proposal.based_on,
        proposal.word,
        proposal.occurrence.as_param(),
        mark.register,
        mark.term
    );
    for (label, value) in [("Unterstichwort", &mark.sub), ("siehe", &mark.see), ("siehe auch", &mark.also)] {
        if !value.is_empty() {
            out.push_str(&format!("{label}: {value}\n"));
        }
    }
    if !proposal.reason.is_empty() {
        out.push_str(&format!("Begründung: {}\n", one_line(&proposal.reason)));
    }
    out
}

/// Commit request for an index proposal: subject „Register: Vorschlag <id> „<Stichwort>“ in <Abschnitt> von
/// <handle>“ (author the persona or the client) or „… verworfen“ (author the human, `Baruch-Proposer`); trailers
/// `Baruch-Action` (index-propose|index-reject), `Baruch-Index-Proposal`, `Baruch-Node` (fixed key). Changes no
/// file, hence always `allow_empty`.
pub fn describe_index_proposal(
    work: &Work,
    proposal: &baruch_store::IndexProposal,
    action: IndexProposalAction,
) -> CommitRequest {
    let place = label_of_key(work, proposal.key);
    let mut trailers = vec![
        (
            "Baruch-Action".to_owned(),
            match action {
                IndexProposalAction::Proposed => "index-propose",
                IndexProposalAction::Rejected => "index-reject",
            }
            .to_owned(),
        ),
        ("Baruch-Index-Proposal".to_owned(), proposal.id.to_string()),
        ("Baruch-Node".to_owned(), proposal.key.to_string()),
    ];
    let (summary, author) = match action {
        IndexProposalAction::Proposed => (
            format!("Register: Vorschlag {} „{}“ in {place} von {}", proposal.id, proposal.mark.term, proposal.origin),
            proposer(&proposal.origin, &mut trailers),
        ),
        IndexProposalAction::Rejected => {
            trailers.push(("Baruch-Proposer".to_owned(), proposal.origin.clone()));
            (
                format!("Register: Vorschlag {} „{}“ verworfen", proposal.id, proposal.mark.term),
                human(proposal.decided_by.as_deref().unwrap_or(HUMAN_NAME)),
            )
        }
    };
    CommitRequest {
        files: Vec::new(),
        author,
        committer: human(HUMAN_NAME),
        summary,
        body: index_body(work, proposal),
        trailers,
        allow_empty: true,
    }
}

/// Commit request for accepting an index proposal: like [`describe`] for the new revision of the human
/// (trailers `Baruch-Node`, `Baruch-Revision`), subject „<Abschnitt>: Registereintrag „<Stichwort>“ übernommen
/// (Fassung n)“, plus `Baruch-Action: index-accept`, `Baruch-Index-Proposal` and `Baruch-Proposer`. `work` is the
/// state afterwards.
pub fn describe_index_accept(work: &Work, accepted: &baruch_store::IndexAccepted) -> CommitRequest {
    let mut request = describe(work, &accepted.change);
    let proposal = &accepted.proposal;
    request.summary = format!(
        "{}: Registereintrag „{}“ übernommen (Fassung {})",
        node_label(work, accepted.change.node),
        proposal.mark.term,
        accepted.change.rev
    );
    request.body = format!("{}Gesetzte Marken: {}\n", index_body(work, proposal), accepted.marks);
    request.trailers.push(("Baruch-Action".to_owned(), "index-accept".to_owned()));
    request.trailers.push(("Baruch-Index-Proposal".to_owned(), proposal.id.to_string()));
    request.trailers.push(("Baruch-Proposer".to_owned(), proposal.origin.clone()));
    request
}

/// Mirror a glossary or index change: exactly one commit in the mirror of the work (`glossar.md` and, for an
/// accept, the section). The caller holds the write lock since before the database write; errors are also
/// on stderr.
pub async fn record_glossary(
    state: &AppState,
    held: &WriteGuard<'_>,
    work: WorkId,
    mut request: CommitRequest,
) -> Result<Option<String>, String> {
    name_persona_author(state, &mut request).await;
    let result = commit(state, held, work, request).await;
    if let Err(e) = &result {
        eprintln!("Git-Spiegel (Glossar/Register): {e}");
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assistant_change_names_persona_as_author_and_human_as_committer() {
        let work = baruch_core::sample::work();
        let change = Change {
            work: 1,
            node: 2,
            rev: 3,
            author: Author::Assistant { persona: "@lektor".into(), model: "qwen3:4b".into() },
            kind: ChangeKind::Accepted,
            before: None,
            reparented: vec![],
            amend_seq: None,
        };
        let request = describe(&work, &change);
        let lektor = work.personas.iter().find(|p| p.handle == "@lektor").unwrap();
        assert_eq!(request.author.name, format!("{} (@lektor)", lektor.name));
        assert_eq!(request.author.email, "lektor@persona.baruch.invalid");
        assert_eq!(request.committer, human("Autor"), "übernommen hat der Mensch");
        let node = &work.nodes[2];
        let label = if node.number.is_empty() { node.title.clone() } else { format!("{} {}", node.number, node.title) };
        assert_eq!(request.summary, format!("{label}: Vorschlag 3 übernommen"));
        let keys: Vec<_> = request.trailers.iter().map(|(k, v)| format!("{k}: {v}")).collect();
        assert_eq!(keys, ["Persona: @lektor", "Model: qwen3:4b", "Baruch-Node: 2", "Baruch-Revision: 3"]);
    }

    #[test]
    fn human_save_has_no_persona_trailers() {
        let work = baruch_core::sample::work();
        let change = Change {
            work: 1,
            node: 4,
            rev: 5,
            author: Author::Human { name: "Autor".into() },
            kind: ChangeKind::Saved,
            before: None,
            reparented: vec![],
            amend_seq: None,
        };
        let request = describe(&work, &change);
        assert_eq!(request.author, request.committer);
        assert_eq!(request.author.email, "autor@baruch.invalid");
        assert!(request.summary.ends_with(": Revision 5 gespeichert"), "{}", request.summary);
        assert_eq!(request.trailers.len(), 2, "nur Baruch-Node und Baruch-Revision: {:?}", request.trailers);
        assert!(!request.allow_empty, "Speichern ändert immer eine Datei, kein leerer Commit nötig");
    }

    #[test]
    fn switching_names_the_author_of_the_version_and_marks_the_action() {
        let work = baruch_core::sample::work();
        let change = Change {
            work: 1,
            node: 2,
            rev: 1,
            author: Author::Human { name: "Autor".into() },
            kind: ChangeKind::Switched,
            before: None,
            reparented: vec![],
            amend_seq: None,
        };
        let request = describe(&work, &change);
        assert!(request.summary.ends_with(": auf Fassung 1 umgeschaltet"), "{}", request.summary);
        let keys: Vec<_> = request.trailers.iter().map(|(k, v)| format!("{k}: {v}")).collect();
        assert_eq!(
            keys,
            ["Baruch-Node: 2", "Baruch-Revision: 1", "Baruch-Action: switched"],
            "Trailer beim Umschalten"
        );
        assert!(request.allow_empty, "jede Umschaltung ist ein Commit, auch bei gleichem Text");
    }

    #[test]
    fn deleting_records_the_full_text_and_the_moved_children() {
        let work = baruch_core::sample::work();
        let before = RevisionRecord {
            number: 2,
            body: "= Alt\n\n<b>Wortlaut</b>\n".into(),
            note: "Notiz\nzweite Zeile".into(),
            status: RevisionStatus::Proposed,
            based_on: Some(1),
            author: Author::Assistant { persona: "@lektor".into(), model: "qwen3:4b".into() },
        };
        let change = Change {
            work: 1,
            node: 2,
            rev: 2,
            author: Author::Human { name: "Autor".into() },
            kind: ChangeKind::Deleted,
            before: Some(before),
            reparented: vec![3, 5],
            amend_seq: None,
        };
        let request = describe(&work, &change);
        assert!(request.summary.ends_with(": Fassung 2 gelöscht"), "{}", request.summary);
        assert!(
            request.body.contains("Gelöschter Text:\n= Alt\n\n<b>Wortlaut</b>\n"),
            "voller Text fehlt: {}",
            request.body
        );
        for part in [
            "Autor: @lektor (qwen3:4b)",
            "Status: Vorschlag",
            "Elternteil: Fassung 1",
            "Notiz: Notiz zweite Zeile",
            "Umgehängt: 3, 5 → Fassung 1",
        ] {
            assert!(request.body.contains(part), "{part} fehlt im Rumpf: {}", request.body);
        }
        assert_eq!(request.author, human("Autor"), "gelöscht hat der Mensch");
        assert_eq!(request.committer, human("Autor"));
        assert!(!request.trailers.iter().any(|(k, _)| k == "Persona"), "keine Persona-Trailer beim Menschen");
        assert!(request.trailers.contains(&("Baruch-Action".into(), "deleted".into())));
        assert!(request.allow_empty, "Löschen einer nicht gespiegelten Fassung braucht einen leeren Commit");
    }

    #[test]
    fn switching_with_another_level_is_one_commit_naming_the_levels_and_the_moved_points() {
        let work = baruch_core::sample::work();
        let change = Change {
            work: 1,
            node: 4,
            rev: 1,
            author: Author::Human { name: "Autor".into() },
            kind: ChangeKind::Switched,
            before: None,
            reparented: vec![],
            amend_seq: None,
        };
        let outline = OutlineChange {
            work: 1,
            action: baruch_store::OutlineAction::Level,
            node: 4,
            title: work.nodes[4].title.clone(),
            detail: format!("„{}“ Ebene 2 → 1", work.nodes[4].title),
            revisions: vec![(5, 3)],
            renumbered: vec![],
            removed: vec![],
        };
        let plain = describe_switch(&work, &Switch { change: change.clone(), outline: None });
        let before = describe(&work, &change);
        assert_eq!(
            (&plain.summary, &plain.body, &plain.trailers),
            (&before.summary, &before.body, &before.trailers),
            "ohne Ebenenwechsel wie bisher"
        );
        let request = describe_switch(&work, &Switch { change, outline: Some(outline) });
        assert!(
            request.summary.ends_with(": auf Fassung 1 umgeschaltet · Ebene 2 → 1"),
            "Betreff mit Ebene: {}",
            request.summary
        );
        assert!(
            request.body.contains(&format!("005 {}: Fassung 3", work.nodes[5].title)),
            "der Rumpf nennt den mitgestuften Punkt mit neuer Fassung: {}",
            request.body
        );
        let actions = request.trailers.iter().filter(|(k, v)| k == "Baruch-Action" && v == "switched").count();
        assert_eq!(actions, 1, "genau ein Trailer Baruch-Action: switched: {:?}", request.trailers);
        assert!(request.allow_empty, "ein Commit, auch wenn sich keine Datei änderte");
    }

    #[test]
    fn mcp_handles_become_valid_email_local_parts() {
        assert_eq!(email_local_part("@mcp:claude-code"), "mcp-claude-code", "':' ist in der Adresse nicht erlaubt");
        assert_eq!(email_local_part("@lektor"), "lektor");
        assert_eq!(email_local_part("@Zoë_1.x"), "Zo-_1.x", "Umlaute und Sonderzeichen werden '-'");
    }

    fn post_info(node: usize, key: i64) -> baruch_store::PostInfo {
        baruch_store::PostInfo {
            id: 12,
            work: 1,
            node,
            key,
            position: 0,
            author: "@mcp:probe".into(),
            body: "Erste Zeile\nZweite Zeile".into(),
            sources: vec![],
            refs: vec![],
            created: "2026-11-06T10:00:00.000Z".into(),
            edited: None,
        }
    }

    #[test]
    fn post_commits_name_action_post_node_and_idea_and_keep_the_text() {
        let work = baruch_core::sample::work();
        let actor = baruch_store::IdeaOrigin::Mcp { handle: "@mcp:probe".into() };
        let trailer = |r: &CommitRequest, k: &str| r.trailers.iter().find(|(n, _)| n == k).map(|(_, v)| v.clone());

        let deleted = describe_post(&work, &post_info(2, 33), PostAction::Delete, None, None, &actor);
        assert_eq!(deleted.summary, "Beitrag 12: gelöscht", "Betreff");
        assert_eq!(trailer(&deleted, "Baruch-Post").as_deref(), Some("12"), "Baruch-Post");
        assert_eq!(trailer(&deleted, "Baruch-Action").as_deref(), Some("post-delete"), "Baruch-Action");
        assert_eq!(trailer(&deleted, "Baruch-Node").as_deref(), Some("33"), "Baruch-Node ist der Schlüssel");
        assert!(deleted.body.contains("Erste Zeile\nZweite Zeile"), "ganzer Text im Rumpf: {}", deleted.body);
        assert!(deleted.allow_empty, "auch ohne Dateiänderung ein Commit (Beiträge stehen nicht im Spiegel)");
        assert_eq!(deleted.author.email, "mcp-probe@persona.baruch.invalid", "Autor ist der Client");
        assert_eq!(deleted.committer.name, HUMAN_NAME, "Committer bleibt der Mensch");

        let from = post_info(2, 33);
        let moved = describe_post(&work, &post_info(4, 55), PostAction::Move, Some(&from), None, &actor);
        let target = &work.nodes[4];
        assert_eq!(moved.summary, format!("Beitrag 12: verschoben nach {} {}", target.number, target.title));
        assert!(moved.body.contains("Von Abschnitt 2 (key 33)"), "Herkunft im Rumpf: {}", moved.body);
        assert_eq!(trailer(&moved, "Baruch-Idea"), None, "nur die Umwandlung nennt einen Einfall");

        let human = baruch_store::IdeaOrigin::Human { name: HUMAN_NAME.into() };
        let converted = describe_post(&work, &post_info(2, 33), PostAction::ToIdea, None, Some(7), &human);
        assert_eq!(converted.summary, "Beitrag 12: in Einfall 7 umgewandelt");
        assert_eq!(trailer(&converted, "Baruch-Idea").as_deref(), Some("7"), "Baruch-Idea");
        assert_eq!(trailer(&converted, "Persona"), None, "der Mensch ist keine Persona");
        assert!(converted.body.contains("Zweite Zeile"), "Beitrag im Rumpf (wiederherstellbar)");
    }

    #[test]
    fn typeset_commits_name_fields_both_values_and_the_action() {
        use baruch_typst::typeset::{ChapterStart, TypesetSettings};
        // Contract §6.5: subject with labels, body before → after per field, trailers, human as author.
        let before = TypesetSettings { font_size_pt: Some(10.5), ..Default::default() };
        let after = TypesetSettings {
            chapter_start: Some(ChapterStart::RightPage),
            font_size_pt: Some(11.0),
            xref_with_page: Some(true),
            ..Default::default()
        };
        let change =
            baruch_store::TypesetChange { changed: before.changed_fields(&after), before, after: after.clone() };
        let request = describe_typeset(&change, TypesetAction::Set);
        assert_eq!(request.summary, "Satz: Kapitelanfang, Schriftgröße, Verweis mit Seitenzahl geändert");
        assert!(
            request.body.contains("Kapitelanfang: aus der Werkvorlage → neue rechte Seite\n"),
            "ungesetzt heißt „aus der Werkvorlage“: {}",
            request.body
        );
        assert!(request.body.contains("Schriftgröße: 10,5 pt → 11 pt\n"), "Zahl deutsch mit Einheit: {}", request.body);
        assert!(request.allow_empty, "xref_with_page ändert keine Datei; trotzdem ein Commit");
        assert_eq!(request.author, human(HUMAN_NAME), "nur der Mensch ändert Satz-Einstellungen");
        assert_eq!(request.trailers, vec![("Baruch-Action".to_owned(), "typeset-set".to_owned())]);
        let reset = baruch_store::TypesetChange {
            changed: after.changed_fields(&TypesetSettings::default()),
            before: after,
            after: TypesetSettings::default(),
        };
        let request = describe_typeset(&reset, TypesetAction::Reset);
        assert_eq!(request.summary, "Satz: Einstellungen zurückgesetzt");
        assert_eq!(request.trailers[0].1, "typeset-reset");
        let request = describe_typeset(&reset, TypesetAction::Adopt { template_rev: 3 });
        assert_eq!(request.summary, "Satz: Kapitelanfang aus der Werkvorlage übernommen (Vorlagenfassung 3)");
        assert_eq!(request.trailers[0].1, "typeset-adopt");
    }

    #[test]
    fn every_typeset_field_has_words() {
        // Every field from FIELDS gets a value in the body; a forgotten field would silently say „aus der Werkvorlage“.
        let full = baruch_exchange::typeset::full_sample();
        for (field, label) in baruch_typst::typeset::FIELDS {
            assert_ne!(typeset_value(&full, field), "aus der Werkvorlage", "{label} ({field}) hat Worte");
        }
    }
}
