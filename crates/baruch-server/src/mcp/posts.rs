// SPDX-License-Identifier: AGPL-3.0-or-later
//! MCP-Werkzeug zum Bearbeiten eigener Beiträge (Runde 28, chat-auftrag).
//!
//! - `post_edit {work, id, body, thread?}`: nur eigene Beiträge des Clients (Autor `@mcp:<name>`), im Abschnitts-Chat
//!   bzw. mit `thread` im Faden; die alte Fassung bleibt als Verlauf erhalten. Beiträge von Personas und des
//!   Menschen sind über MCP nicht bearbeitbar (auch nicht mit `persona`: Personas bearbeiten nie). Im Abschnitts-Chat
//!   genau ein Commit „Beitrag <id>: bearbeitet“ (Autor der Client), im Faden keiner (Fäden stehen nicht im Spiegel).
//!   Eigentum wie bei `post_delete`: über den Handle des Clients (Runde 23: mit Token der Tokenname samt
//!   Alt-Handle; ohne Token der angegebene Name, mit `lokal-`, wenn er einem Token gehört – docs/MCP.md).

use serde::Deserialize;
use serde_json::{Value, json};

use super::session::Client;
use super::tools::{Failure, WorkRef, args, resolve, work_schema};
use crate::AppState;
use baruch_store::IdeaOrigin;

pub const NAMES: [&str; 1] = ["post_edit"];

pub const EDIT_DESCRIPTION: &str = "Eigenen Beitrag bearbeiten: nur Beiträge, deren Autor dieser Client ist \
(@mcp:<name> – mit Token der Tokenname, ohne Token der angegebene Name, als @mcp:lokal-<name>, wenn er einem Token \
gehört), im Abschnitts-Chat bzw. mit thread im Faden; die alte Fassung bleibt als Verlauf erhalten (im \
Abschnitts-Chat ein Commit mit Text vorher und nachher, im Faden keiner). Beiträge von Personas und des Menschen sind \
über MCP nicht bearbeitbar. Antwort mit id, edited, versions und url.";

pub fn edit_schema() -> Value {
    json!({ "type": "object", "required": ["work", "id", "body"], "properties": {
        "work": work_schema(),
        "id": { "type": "integer", "description": "id des Beitrags (aus chat, add_post bzw. read_node)" },
        "body": { "type": "string", "description": "neuer Text, höchstens 4000 Zeichen" },
        "thread": { "type": "integer", "description": "Faden ohne Abschnitt, in dem der Beitrag steht" } } })
}

#[derive(Deserialize)]
struct EditArgs {
    work: WorkRef,
    id: i64,
    body: String,
    thread: Option<i64>,
}

pub async fn call(state: &AppState, client: &Client, name: &str, arguments: Value) -> Option<Result<Value, Failure>> {
    if !NAMES.contains(&name) {
        return None;
    }
    Some(edit(state, client, arguments).await)
}

async fn edit(state: &AppState, client: &Client, arguments: Value) -> Result<Value, Failure> {
    let a: EditArgs = args(arguments)?;
    let (work, _, _) = resolve(&state.store, &a.work).await?;
    // Runde 23: Eigentum über den Handle des Clients (mit Token zusätzlich dessen Alt-Handle); ein Client ohne Token
    // mit dem Namen eines Tokens ist hier schon `@mcp:lokal-…` und besitzt dessen Beiträge nicht.
    let handle = client.handle();
    let also: Vec<String> = client.owned_handles().into_iter().filter(|h| h != &handle).collect();
    let mut answer = match a.thread {
        Some(thread) => crate::threads::edit_post_as(state, work, thread, a.id, &a.body, &handle, &also).await?,
        None => {
            let actor = crate::posts::Actor::own(IdeaOrigin::Mcp { handle }, also);
            crate::posts::edit(state, work, a.id, &a.body, &actor).await?
        }
    };
    answer["work"] = json!(work);
    Ok(answer)
}
