// SPDX-License-Identifier: AGPL-3.0-or-later
//! Verbindungen zwischen Einfällen schreiben (Vertrag Runde 25 §4.1/§4.2), gemeinsam für RPC und MCP: Anlegen
//! bzw. Wiederherstellen ([`link`]), Lösen ([`unlink`]) und Wiederherstellen ([`restore`]).
//!
//! Jede Änderung läuft unter [`AppState::write_lock`] und ist genau ein Commit im Spiegel des Werks der beiden
//! Einfälle (ohne Werk im Ordner `<data>/einfaelle/`) über [`mirror::record_idea`]: Betreff „Verbindung <id>:
//! Einfall <a> <Art> Einfall <b> (angelegt|gelöscht|wiederhergestellt)“, Trailer `Baruch-Action:
//! idea-link|idea-unlink|idea-link-restore`, `Baruch-Idea-Link`, `Baruch-Origin` und bei Persona/MCP `Persona`
//! (dann ist sie Autor, Committer immer der Mensch). Ein `idea.link` auf ein schon aktives Paar ändert nichts
//! und committet nicht; auf ein gelöschtes stellt es wieder her (`idea-link-restore`). Kein Buchtext ändert sich.

use baruch_gitmirror::{CommitRequest, Signature};
use baruch_store::{IdeaLink, IdeaLinkId, IdeaOrigin, NewIdeaLink, WorkId};
use serde_json::{Value as Json, json};

use crate::rpc::RpcError;
use crate::{AppState, mirror};

/// Was mit einer Verbindung geschah.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkAction {
    Create,
    Delete,
    Restore,
}

impl LinkAction {
    /// Wert des Trailers `Baruch-Action`.
    pub fn trailer(self) -> &'static str {
        match self {
            LinkAction::Create => "idea-link",
            LinkAction::Delete => "idea-unlink",
            LinkAction::Restore => "idea-link-restore",
        }
    }

    fn word(self) -> &'static str {
        match self {
            LinkAction::Create => "angelegt",
            LinkAction::Delete => "gelöscht",
            LinkAction::Restore => "wiederhergestellt",
        }
    }
}

/// Lokaler Teil einer Adresse aus einem Handle (wie im Spiegel bei Einfällen).
fn email_local_part(handle: &str) -> String {
    let local: String = handle
        .trim_start_matches('@')
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') { c } else { '-' })
        .collect();
    if local.is_empty() { "persona".to_owned() } else { local }
}

/// Commit-Auftrag ohne Dateien für eine Aktion von `actor` an `link` (siehe Modulkopf). `labels` = Schlagwörter
/// der beiden Einfälle für den Rumpf.
pub fn describe(link: &IdeaLink, action: LinkAction, actor: &IdeaOrigin, labels: (&str, &str)) -> CommitRequest {
    let mut trailers = vec![
        ("Baruch-Action".to_owned(), action.trailer().to_owned()),
        ("Baruch-Idea-Link".to_owned(), link.id.to_string()),
        ("Baruch-Origin".to_owned(), actor.kind().to_owned()),
    ];
    let author = match actor {
        IdeaOrigin::Human { name } => Signature { name: name.clone(), email: mirror::HUMAN_EMAIL.to_owned() },
        IdeaOrigin::Persona { handle } | IdeaOrigin::Mcp { handle } => {
            trailers.push(("Persona".to_owned(), handle.clone()));
            Signature { name: handle.clone(), email: format!("{}@persona.baruch.invalid", email_local_part(handle)) }
        }
    };
    let mut body = format!("Von: Einfall {} ({})\nNach: Einfall {} ({})", link.from, labels.0, link.to, labels.1);
    if link.kind.symmetric() {
        body.push_str("\n„widerspricht“ ist ungerichtet.");
    }
    if !link.note.is_empty() {
        body.push_str("\nNotiz: ");
        body.push_str(&link.note.split('\n').collect::<Vec<_>>().join("\n  "));
    }
    CommitRequest {
        files: Vec::new(),
        author,
        committer: Signature { name: mirror::HUMAN_NAME.to_owned(), email: mirror::HUMAN_EMAIL.to_owned() },
        summary: format!(
            "Verbindung {}: Einfall {} {} Einfall {} ({})",
            link.id,
            link.from,
            link.kind.label(),
            link.to,
            action.word()
        ),
        body,
        trailers,
        allow_empty: true,
    }
}

/// Ergebnis des Spiegels in die Antwort: `commit` bei neuem Commit, `mirror` bei Fehler.
fn add_mirror(answer: &mut Json, result: Result<Option<String>, String>) {
    match result {
        Ok(Some(commit)) => answer["commit"] = json!(commit),
        Ok(None) => {}
        Err(e) => answer["mirror"] = json!(e),
    }
}

/// Werk der Verbindung (das der beiden Einfälle) und die Schlagwörter beider für den Rumpf.
async fn context(state: &AppState, link: &IdeaLink) -> Result<(Option<WorkId>, String, String), RpcError> {
    let from = state.store.idea_get(link.from).await?;
    let to = state.store.idea_get(link.to).await?;
    Ok((from.work, from.words.join(", "), to.words.join(", ")))
}

async fn record(
    state: &AppState,
    held: &mirror::WriteGuard<'_>,
    link: &IdeaLink,
    action: LinkAction,
    actor: &IdeaOrigin,
    answer: &mut Json,
) -> Result<(), RpcError> {
    let (work, a, b) = context(state, link).await?;
    let request = describe(link, action, actor, (&a, &b));
    add_mirror(answer, mirror::record_idea(state, held, work, request).await);
    Ok(())
}

/// Verbinden (bzw. eine gelöschte gleiche Verbindung wiederherstellen). Antwort `{link, changed, commit?|mirror?}`.
pub(crate) async fn link(state: &AppState, new: &NewIdeaLink<'_>) -> Result<Json, RpcError> {
    let held = state.write_lock.lock().await;
    // Unter der Sperre nachsehen, ob es Paar und Art schon gelöscht gibt: dann ist es ein Wiederherstellen.
    let (from, to) = if new.kind.symmetric() && new.from > new.to { (new.to, new.from) } else { (new.from, new.to) };
    let was_deleted = match state.store.idea_links_of_idea(from, true).await {
        Ok(links) => links.iter().any(|l| l.to == to && l.from == from && l.kind == new.kind && l.deleted.is_some()),
        Err(_) => false,
    };
    let (link, changed) = state.store.idea_link_create(new).await?;
    let mut answer = json!({ "link": link, "changed": changed });
    if changed {
        let action = if was_deleted { LinkAction::Restore } else { LinkAction::Create };
        record(state, &held, &link, action, &new.origin, &mut answer).await?;
    }
    Ok(answer)
}

/// Verbindung lösen (weich). Antwort `{link, commit?|mirror?}`.
pub(crate) async fn unlink(state: &AppState, id: IdeaLinkId, actor: &IdeaOrigin) -> Result<Json, RpcError> {
    let held = state.write_lock.lock().await;
    let link = state.store.idea_link_delete(id).await?;
    let mut answer = json!({ "link": link });
    record(state, &held, &link, LinkAction::Delete, actor, &mut answer).await?;
    Ok(answer)
}

/// Gelöste Verbindung wiederherstellen. Antwort `{link, commit?|mirror?}`.
pub(crate) async fn restore(state: &AppState, id: IdeaLinkId, actor: &IdeaOrigin) -> Result<Json, RpcError> {
    let held = state.write_lock.lock().await;
    let link = state.store.idea_link_restore(id).await?;
    let mut answer = json!({ "link": link });
    record(state, &held, &link, LinkAction::Restore, actor, &mut answer).await?;
    Ok(answer)
}

#[cfg(test)]
mod tests {
    use super::*;
    use baruch_store::LinkKind;

    fn sample(kind: LinkKind, note: &str) -> IdeaLink {
        IdeaLink {
            id: 7,
            uuid: String::new(),
            from: 48,
            to: 79,
            kind,
            note: note.into(),
            origin: IdeaOrigin::Human { name: "Autor".into() },
            created: "t".into(),
            updated: "t".into(),
            deleted: None,
        }
    }

    #[test]
    fn the_commit_names_link_ideas_kind_and_origin() {
        let actor = IdeaOrigin::Mcp { handle: "@mcp:probe".into() };
        let request =
            describe(&sample(LinkKind::Contradicts, "a\nb"), LinkAction::Create, &actor, ("Laube", "Rose, Dorn"));
        // Vertrag §4.1: Zusammenfassung „Verbindung <id>: Einfall <a> <Art> Einfall <b> (angelegt)“.
        assert_eq!(
            request.summary, "Verbindung 7: Einfall 48 widerspricht Einfall 79 (angelegt)",
            "Betreff nach Vertrag"
        );
        let trailer = |k: &str| request.trailers.iter().find(|(key, _)| key == k).map(|(_, v)| v.as_str());
        assert_eq!(trailer("Baruch-Action"), Some("idea-link"), "Aktion");
        assert_eq!(trailer("Baruch-Idea-Link"), Some("7"), "id der Verbindung");
        assert_eq!(trailer("Baruch-Origin"), Some("mcp"), "Herkunft des Handelnden");
        assert_eq!(trailer("Persona"), Some("@mcp:probe"), "Persona-Trailer benennt den Autor (record_idea)");
        assert_eq!(request.author.email, "mcp-probe@persona.baruch.invalid", "Autor ist der Client");
        assert_eq!(request.committer.name, mirror::HUMAN_NAME, "Committer bleibt der Mensch");
        assert!(request.body.contains("Notiz: a\n  b"), "Notiz mit eingerückter Folgezeile: {}", request.body);
        assert!(request.allow_empty, "immer ein Commit");
    }
}
