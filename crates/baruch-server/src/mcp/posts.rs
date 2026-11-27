// SPDX-License-Identifier: AGPL-3.0-or-later
//! MCP tool for editing one's own posts (Round 28, chat-auftrag).
//!
//! - `post_edit {work, id, body, thread?}`: only the client's own posts (author `@mcp:<name>`), in the section chat
//!   or with `thread` in the thread; the old revision is kept as history. Posts of personas and of the
//!   human cannot be edited through MCP (not with `persona` either: personas never edit). In the section chat
//!   exactly one commit „Beitrag <id>: bearbeitet“ (author the client), in the thread none (threads are not in the
//!   mirror). Ownership as with `post_delete`: through the handle of the client (Round 23: with a token the token
//!   name including the legacy handle; without a token the given name, with `lokal-` if it belongs to a token –
//!   docs/MCP.md).

use serde::Deserialize;
use serde_json::{Value, json};

use super::session::Client;
use super::tools::{Failure, WorkRef, args, resolve, work_schema};
use crate::AppState;
use baruch_store::IdeaOrigin;

pub const NAMES: [&str; 1] = ["post_edit"];

pub const EDIT_DESCRIPTION: &str = "Edit an own post: only posts whose author is this client (@mcp:<name> – with a \
token the token name, without a token the given name, as @mcp:lokal-<name> if it belongs to a token), in the section \
chat or with thread in the thread; the old version stays as history (in the section chat one commit with the text \
before and after, in the thread none). Posts of personas and of the human cannot be edited over MCP. Answer with \
id, edited, versions and url.";

pub fn edit_schema() -> Value {
    json!({ "type": "object", "required": ["work", "id", "body"], "properties": {
        "work": work_schema(),
        "id": { "type": "integer", "description": "id of the post (from chat, add_post or read_node)" },
        "body": { "type": "string", "description": "new text, at most 4000 characters" },
        "thread": { "type": "integer", "description": "thread without a section that holds the post" } } })
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
    // Round 23: ownership through the handle of the client (with a token additionally its legacy handle); a client
    // without a token with the name of a token is already `@mcp:lokal-…` here and does not own its posts.
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
