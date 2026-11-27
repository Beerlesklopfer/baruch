// SPDX-License-Identifier: AGPL-3.0-or-later
//! MCP-Werkzeuge für Bilder (Vertrag Runde 11, Abschnitt 7).
//!
//! - `asset_add` legt ein Bild **sofort** an (Fassung 1, Autor die Persona bzw. der Client als
//!   `@mcp:<name>`, ein Spiegel-Commit mit Herkunft, [`mirror::AssetBy::Direct`]); höchstens 8 MiB
//!   entschlüsselt wie über RPC. Mit `role: "cover"` legt es zusätzlich einen **Antrag** an, das Bild
//!   als Umschlag zu setzen.
//! - `asset_replace`, `asset_set_cover`, `asset_delete` ändern **nichts**: Sie legen einen Antrag an
//!   ([`baruch_store::Store::asset_request`], Migration 0018), über den der Mensch auf der Seite
//!   „Bilder“ (`#antraege`) entscheidet ([`crate::asset_requests`]).
//! - `asset_requests` liest die Anträge (offen bzw. alle, wahlweise nur eigene).
//!
//! `persona` nur ohne Token vom eigenen Rechner (wie bei `outline_*`). Alles Schreibende unter
//! [`AppState::write_lock`].
//!
//! **Kontingent (Runde 21c)** je Antragsteller (Persona bzw. Client `@mcp:<name>`) und Werk in rollenden 24 Stunden,
//! gezählt aus den vorhandenen Zeilen ([`baruch_store::Store::asset_mcp_usage`]: Bildfassungen mit diesem Autor und
//! Anträge zum Ersetzen): höchstens [`MAX_MCP_ASSET_WRITES_PER_DAY`] schreibende Aufrufe (`asset_add` und
//! `asset_replace`), höchstens [`MAX_MCP_ASSET_BYTES_PER_DAY`] entschlüsselt, höchstens
//! [`MAX_MCP_OPEN_REPLACE_REQUESTS`] offene Anträge zum Ersetzen. Darüber antwortet das Werkzeug mit `isError` und
//! einer ehrlichen Meldung, gespeichert wird nichts. Geprüft wird unter der Schreibsperre, direkt vor dem Schreiben.
//! **Runde 23:** Mit Token gilt das Kontingent je Token; **alle Clients ohne Token** – gleich welcher
//! `clientInfo.name`, auch mit `persona` – teilen sich je Werk **ein** Kontingent
//! ([`baruch_store::Store::asset_mcp_usage_without_token`]). Vorher öffnete jeder neue Name ein neues.
//! `asset_replace` nimmt `rename` (Typwechsel, das Bild bekommt beim Bestätigen die neue Endung).
//! `asset_requests` entfernt vor dem Lesen den Inhalt entschiedener Anträge, die älter als
//! [`baruch_store::ASSET_REQUEST_PURGE_DAYS`] Tage sind.

use baruch_store::{AssetEncoding, AssetOp, IdeaOrigin, MAX_ASSET_BYTES, NewAsset, WorkId};
use serde::Deserialize;
use serde_json::{Value, json};

use super::session::Client;
use super::tools::{Failure, WorkRef, args, origin_of, tool_error, work_or_default, work_schema};
use crate::{AppState, asset_requests, mirror};

/// Höchstens so viele schreibende Aufrufe (`asset_add` + `asset_replace`) je Antragsteller und Werk in 24 Stunden.
pub const MAX_MCP_ASSET_WRITES_PER_DAY: usize = 30;
/// Höchstens so viele Bytes (entschlüsselt) je Antragsteller und Werk in 24 Stunden: 64 MiB.
pub const MAX_MCP_ASSET_BYTES_PER_DAY: u64 = 64 * 1024 * 1024;
/// Höchstens so viele offene Anträge zum Ersetzen je Antragsteller und Werk.
pub const MAX_MCP_OPEN_REPLACE_REQUESTS: usize = 10;

/// Namen der Werkzeuge dieses Moduls (in der Reihenfolge von `tools/list`).
pub const NAMES: &[&str] = &["asset_add", "asset_replace", "asset_set_cover", "asset_delete", "asset_requests"];

pub const ADD_DESCRIPTION: &str = "Add an image immediately (revision 1): content as Base64 (encoding \"base64\", PNG, \
JPEG, GIF, WebP, SVG) or SVG text (encoding \"svg\"), at most 8 MiB decoded. name missing → from file_name; path (one \
per line) is the path under which template or text name the image with image(\"…\"). The author of revision and commit \
is `persona` or the client as @mcp:<name> (origin in the Git mirror). role: \"cover\" additionally files a request to \
set the image as the cover (the human confirms). Answer: name, rev, mime, bytes, url, commit?, request?. Replacing, \
cover and deleting work only as a request (asset_replace, asset_set_cover, asset_delete). Quota per work in 24 hours \
– per token, all clients without a token together (whatever name or persona): at most 30 writing calls (asset_add \
and asset_replace) and 64 MiB; beyond that an error, nothing stored.";

pub const REPLACE_DESCRIPTION: &str = "Request: replace an image with new content (content as for asset_add; \
caption/alt optional for the new revision; another format only with rename: true – then the image gets the new \
extension on confirming, all paths stay). Changes nothing immediately: files a request that the human confirms or \
rejects on the \"Bilder\" page. If the image has another current revision on confirming than at the request, the \
request is closed as stale. Quota as for asset_add (per token, without a token together), plus at most 10 open \
requests to replace. Answer: request, status \"open\".";

pub const SET_COVER_DESCRIPTION: &str = "Request: set an image as the cover of the work (name) or remove the cover \
(name: null). Changes nothing immediately: the human confirms or rejects; if the cover on confirming is another one \
than at the request, the request is closed as stale. Answer: request, status \"open\".";

pub const DELETE_DESCRIPTION: &str = "Request: delete an image with all revisions (without rev) or one revision that \
is not current (rev). Changes nothing immediately: the human confirms or rejects. Answer: request, status \"open\".";

pub const REQUESTS_DESCRIPTION: &str = "Read the image requests: open ones (default) or with open: false also decided \
ones (confirmed, rejected, stale with reason); mine: true only own ones. The content of a request to replace is not \
in the answer (only bytes and content_url); 30 days after the decision it is removed (purged).";

fn persona_schema() -> Value {
    json!({ "type": "string", "description": "persona handle (e.g. @lektor) as author or requester (only without a token from the own machine); otherwise the client as @mcp:<name>" })
}

fn note_schema() -> Value {
    json!({ "type": "string", "description": "note or reason for the human" })
}

fn name_schema() -> Value {
    json!({ "type": "string", "description": "image name from list_assets, e.g. umschlag.png" })
}

fn content_schema() -> Value {
    json!({ "type": "string", "description": "content: Base64 (standard alphabet) or SVG text, see encoding" })
}

fn encoding_schema() -> Value {
    json!({ "enum": ["base64", "svg"], "description": "base64 for all formats, svg for SVG text" })
}

pub fn add_schema() -> Value {
    json!({ "type": "object", "required": ["file_name", "content", "encoding"], "properties": {
        "work": work_schema(),
        "file_name": { "type": "string", "description": "file name, e.g. umschlag.png (the name derives from it if name is missing)" },
        "content": content_schema(),
        "encoding": encoding_schema(),
        "name": { "type": "string", "description": "own image name (lower-case letters, digits, - _ . with extension)" },
        "path": { "type": "string", "description": "path(s) in template/text, one per line, e.g. bilder/umschlag.png" },
        "caption": { "type": "string", "description": "Bildunterschrift" },
        "alt": { "type": "string", "description": "Alternativtext (Barrierefreiheit)" },
        "note": note_schema(),
        "role": { "enum": ["cover"], "description": "cover: additionally a request to set the image as the cover" },
        "persona": persona_schema() } })
}

pub fn replace_schema() -> Value {
    json!({ "type": "object", "required": ["name", "content", "encoding"], "properties": {
        "work": work_schema(), "name": name_schema(), "content": content_schema(), "encoding": encoding_schema(),
        "caption": { "type": "string", "description": "caption of the new revision (missing: the previous one)" },
        "alt": { "type": "string", "description": "alternative text of the new revision (missing: the previous one)" },
        "rename": { "type": "boolean", "description": "true: another format is allowed; the image gets the new extension on confirming (default false)" },
        "note": note_schema(), "persona": persona_schema() } })
}

pub fn set_cover_schema() -> Value {
    json!({ "type": "object", "required": ["name"], "properties": {
        "work": work_schema(),
        "name": { "type": ["string", "null"], "description": "Bildname; null = Umschlag entfernen" },
        "note": note_schema(), "persona": persona_schema() } })
}

pub fn delete_schema() -> Value {
    json!({ "type": "object", "required": ["name"], "properties": {
        "work": work_schema(), "name": name_schema(),
        "rev": { "type": "integer", "minimum": 1, "description": "only this revision (not the current one); missing: the whole image" },
        "note": note_schema(), "persona": persona_schema() } })
}

pub fn requests_schema() -> Value {
    json!({ "type": "object", "properties": {
        "work": work_schema(),
        "open": { "type": "boolean", "description": "true (default): only open ones; false: also decided ones" },
        "mine": { "type": "boolean", "description": "only own ones (requester = persona or @mcp:<name>); default false" },
        "persona": persona_schema() } })
}

#[derive(Deserialize)]
struct AddArgs {
    work: Option<WorkRef>,
    file_name: String,
    content: String,
    encoding: String,
    name: Option<String>,
    path: Option<String>,
    caption: Option<String>,
    alt: Option<String>,
    #[serde(default)]
    note: String,
    role: Option<String>,
    persona: Option<String>,
}

#[derive(Deserialize)]
struct ReplaceArgs {
    work: Option<WorkRef>,
    name: String,
    content: String,
    encoding: String,
    caption: Option<String>,
    alt: Option<String>,
    #[serde(default)]
    note: String,
    #[serde(default)]
    rename: bool,
    persona: Option<String>,
}

#[derive(Deserialize)]
struct CoverArgs {
    work: Option<WorkRef>,
    name: Option<String>,
    #[serde(default)]
    note: String,
    persona: Option<String>,
}

#[derive(Deserialize)]
struct DeleteArgs {
    work: Option<WorkRef>,
    name: String,
    rev: Option<u32>,
    #[serde(default)]
    note: String,
    persona: Option<String>,
}

#[derive(Deserialize)]
struct ListArgs {
    work: Option<WorkRef>,
    open: Option<bool>,
    #[serde(default)]
    mine: bool,
    persona: Option<String>,
}

pub async fn call(state: &AppState, client: &Client, name: &str, arguments: Value) -> Option<Result<Value, Failure>> {
    Some(match name {
        "asset_add" => add(state, client, arguments).await,
        "asset_replace" => replace(state, client, arguments).await,
        "asset_set_cover" => set_cover(state, client, arguments).await,
        "asset_delete" => delete(state, client, arguments).await,
        "asset_requests" => requests(state, client, arguments).await,
        _ => return None,
    })
}

/// Adresse der Anträge auf der Seite „Bilder“.
fn requests_url(work: WorkId) -> String {
    format!("/werke/{work}/bilder#antraege")
}

fn encoding(text: &str) -> Result<AssetEncoding, Failure> {
    AssetEncoding::parse(text.trim())
        .ok_or_else(|| Failure::Args(format!("arguments: encoding \"{text}\" – base64 or svg expected")))
}

/// Bytes aus `content` (Base64 entschlüsselt bzw. SVG-Text); über 8 MiB → Werkzeugfehler.
fn decoded(content: &str, encoding: AssetEncoding) -> Result<Vec<u8>, Failure> {
    let bytes = match encoding {
        AssetEncoding::Base64 => baruch_store::decode_base64(content)?,
        AssetEncoding::Svg => content.as_bytes().to_vec(),
    };
    if bytes.len() > MAX_ASSET_BYTES {
        return Err(tool_error(format!(
            "The image has {} bytes; at most 8 MiB ({MAX_ASSET_BYTES} bytes) are allowed.",
            bytes.len()
        )));
    }
    Ok(bytes)
}

/// Kontingent prüfen (siehe Moduldoku): `bytes` will der Aufruf schreiben, `replace` = ein Antrag zum Ersetzen.
/// Darüber → Werkzeugfehler mit ehrlicher Meldung. Der Aufrufer hält die Schreibsperre.
async fn check_quota(
    state: &AppState,
    client: &Client,
    work: WorkId,
    origin: &IdeaOrigin,
    bytes: u64,
    replace: bool,
) -> Result<(), Failure> {
    // Runde 23: je Token ein Kontingent; alle Clients ohne Token zusammen eines (sonst öffnete ein neuer Name oder
    // ein anderes `persona` ein neues). Die Autoren der Tokens – Handle und Alt-Handle, auch widerrufener – zählen
    // nicht zur Gruppe ohne Token.
    let (usage, who) = if client.token {
        (state.store.asset_mcp_usage(work, origin.name()).await?, origin.name().to_owned())
    } else {
        let token_handles = super::tools::token_handles(&state.store).await?;
        let usage = state.store.asset_mcp_usage_without_token(work, &token_handles).await?;
        (usage, "The clients without a token (shared quota)".to_owned())
    };
    let who = who.as_str();
    let have = if client.token { "has" } else { "have" };
    if usage.writes >= MAX_MCP_ASSET_WRITES_PER_DAY {
        return Err(tool_error(format!(
            "Quota exhausted: {who} {have} already made {} writing image calls (asset_add, asset_replace) in this work in \
             the last 24 hours; allowed are {MAX_MCP_ASSET_WRITES_PER_DAY}. Nothing stored; try again later or ask the \
             human to upload the image.",
            usage.writes
        )));
    }
    if usage.bytes.saturating_add(bytes) > MAX_MCP_ASSET_BYTES_PER_DAY {
        return Err(tool_error(format!(
            "Quota exhausted: {who} {have} already written {} bytes of images in this work in the last 24 hours; with \
             these {bytes} bytes it would be more than 64 MiB ({MAX_MCP_ASSET_BYTES_PER_DAY} bytes). Nothing stored.",
            usage.bytes
        )));
    }
    if replace && usage.open_replace >= MAX_MCP_OPEN_REPLACE_REQUESTS {
        return Err(tool_error(format!(
            "Too many open requests: {who} {have} already {} open requests to replace in this work; allowed are \
             {MAX_MCP_OPEN_REPLACE_REQUESTS}. Nothing stored; first the human decides on the open ones \
             (asset_requests).",
            usage.open_replace
        )));
    }
    Ok(())
}

/// Antrag anlegen und die Antwort `{work, request, status: "open", message, url}` bauen.
async fn open_request(
    state: &AppState,
    work: WorkId,
    op: &AssetOp,
    origin: &IdeaOrigin,
    note: &str,
) -> Result<Value, Failure> {
    let id = {
        let _held = state.write_lock.lock().await;
        state.store.asset_request(work, op, origin, note).await?
    };
    request_answer(state, work, id).await
}

/// Antwort `{work, request, status: "open", message, url}` zum Antrag `id`.
async fn request_answer(state: &AppState, work: WorkId, id: i64) -> Result<Value, Failure> {
    let request = state.store.asset_request_get(work, id).await?;
    Ok(json!({
        "work": work,
        "request": asset_requests::request_json(&request),
        "status": "open",
        "message": format!(
            "Request {} filed ({}); the request itself changes nothing yet. The human confirms or rejects.",
            request.id, request.summary
        ),
        "url": requests_url(work),
    }))
}

async fn add(state: &AppState, client: &Client, arguments: Value) -> Result<Value, Failure> {
    let a: AddArgs = args(arguments)?;
    let cover = match a.role.as_deref().map(str::trim) {
        None | Some("") => false,
        Some("cover") => true,
        Some(other) => return Err(Failure::Args(format!("arguments: role \"{other}\" – only \"cover\" expected"))),
    };
    let encoding = encoding(&a.encoding)?;
    let bytes = decoded(&a.content, encoding)?;
    let work = work_or_default(state, a.work.as_ref()).await?;
    let origin = origin_of(&state.store, client, a.persona.as_deref()).await?;
    let new = NewAsset {
        file_name: &a.file_name,
        name: a.name.as_deref().filter(|n| !n.trim().is_empty()),
        bytes: &bytes,
        as_base64: encoding == AssetEncoding::Base64,
        path: a.path.as_deref(),
        caption: a.caption.as_deref().unwrap_or(""),
        alt: a.alt.as_deref().unwrap_or(""),
        note: a.note.trim(),
    };
    let (change, commit) = {
        let held = state.write_lock.lock().await;
        check_quota(state, client, work, &origin, bytes.len() as u64, false).await?;
        let change = state.store.add_asset(work, &new, origin.name()).await?;
        let commit = mirror::record_asset_by(state, &held, &change, mirror::AssetBy::Direct(&origin)).await;
        (change, commit)
    };
    let summary = state.store.asset_list(work).await?.into_iter().find(|s| s.name == change.name);
    let mut answer = json!({
        "work": work,
        "name": change.name,
        "rev": change.rev,
        "mime": summary.as_ref().map(|s| s.mime.clone()),
        "bytes": bytes.len(),
        "url": format!("/werke/{work}/bilder/{}", change.name),
        "author": origin.name(),
    });
    match commit {
        Ok(Some(commit)) => answer["commit"] = json!(commit),
        Ok(None) => {}
        Err(e) => answer["mirror"] = json!(e),
    }
    if cover {
        let expected_cover = asset_requests::cover_name(state, work).await?;
        let op = AssetOp::SetCover { name: Some(change.name.clone()), expected_cover };
        let note = if a.note.trim().is_empty() { "Umschlag (asset_add mit role cover)" } else { a.note.trim() };
        match open_request(state, work, &op, &origin, note).await {
            Ok(request) => {
                answer["request"] = request["request"].clone();
                answer["message"] = json!(format!(
                    "Image \"{}\" added; {}",
                    change.name,
                    request["message"].as_str().unwrap_or("Request for the cover filed.")
                ));
            }
            // Das Bild ist angelegt; der Antrag scheiterte (z. B. schon offen) – ehrlich melden, nicht verschweigen.
            Err(Failure::Tool(message)) => answer["request_error"] = json!(message),
            Err(other) => return Err(other),
        }
    }
    Ok(answer)
}

async fn replace(state: &AppState, client: &Client, arguments: Value) -> Result<Value, Failure> {
    let a: ReplaceArgs = args(arguments)?;
    let encoding = encoding(&a.encoding)?;
    let size = decoded(&a.content, encoding)?.len() as u64;
    let work = work_or_default(state, a.work.as_ref()).await?;
    let origin = origin_of(&state.store, client, a.persona.as_deref()).await?;
    let op = AssetOp::Replace {
        name: a.name,
        mime: String::new(),
        encoding,
        content: a.content,
        bytes: 0,
        caption: a.caption,
        alt: a.alt,
        expected_sha256: String::new(),
    };
    let id = {
        let _held = state.write_lock.lock().await;
        check_quota(state, client, work, &origin, size, true).await?;
        state.store.asset_request_with(work, &op, &origin, &a.note, a.rename).await?
    };
    request_answer(state, work, id).await
}

async fn set_cover(state: &AppState, client: &Client, arguments: Value) -> Result<Value, Failure> {
    if arguments.get("name").is_none() {
        return Err(Failure::Args("arguments: name is missing (image name or null to remove)".into()));
    }
    let a: CoverArgs = args(arguments)?;
    let work = work_or_default(state, a.work.as_ref()).await?;
    let origin = origin_of(&state.store, client, a.persona.as_deref()).await?;
    let name = a.name.map(|n| n.trim().to_owned()).filter(|n| !n.is_empty());
    let expected_cover = asset_requests::cover_name(state, work).await?;
    if name == expected_cover {
        return Err(tool_error(match &name {
            Some(name) => format!("\"{name}\" is already the cover; nothing to request."),
            None => "The work has no cover; nothing to remove.".to_owned(),
        }));
    }
    let op = AssetOp::SetCover { name, expected_cover };
    open_request(state, work, &op, &origin, &a.note).await
}

async fn delete(state: &AppState, client: &Client, arguments: Value) -> Result<Value, Failure> {
    let a: DeleteArgs = args(arguments)?;
    let work = work_or_default(state, a.work.as_ref()).await?;
    let origin = origin_of(&state.store, client, a.persona.as_deref()).await?;
    let op = AssetOp::Delete { name: a.name, rev: a.rev, expected_sha256: String::new() };
    open_request(state, work, &op, &origin, &a.note).await
}

async fn requests(state: &AppState, client: &Client, arguments: Value) -> Result<Value, Failure> {
    let a: ListArgs = args(arguments)?;
    let work = work_or_default(state, a.work.as_ref()).await?;
    let own = if a.mine { Some(origin_of(&state.store, client, a.persona.as_deref()).await?) } else { None };
    asset_requests::purge(state, work).await?;
    let list = state.store.asset_requests(work, a.open.unwrap_or(true)).await?;
    let list: Vec<Value> = list
        .iter()
        .filter(|r| own.as_ref().is_none_or(|o| r.origin.name() == o.name()))
        .map(asset_requests::request_json)
        .collect();
    Ok(json!({ "work": work, "requests": list, "url": requests_url(work) }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn more_than_8_mib_decoded_is_refused_and_8_mib_pass() {
        let exact = baruch_store::encode_base64(&vec![1u8; MAX_ASSET_BYTES]);
        assert_eq!(decoded(&exact, AssetEncoding::Base64).unwrap().len(), MAX_ASSET_BYTES, "genau 8 MiB gehen");
        let over = baruch_store::encode_base64(&vec![1u8; MAX_ASSET_BYTES + 1]);
        match decoded(&over, AssetEncoding::Base64) {
            Err(Failure::Tool(text)) => assert!(text.contains("8 MiB"), "Meldung nennt die Grenze: {text}"),
            other => panic!("über 8 MiB entschlüsselt ist ein Werkzeugfehler, nicht {other:?}"),
        }
        assert!(matches!(encoding("zip"), Err(Failure::Args(_))), "unbekannte Kodierung ist ein Argumentfehler");
    }
}
