// SPDX-License-Identifier: AGPL-3.0-or-later
// JSON-RPC 2.0 against the Baruch core (`POST /rpc`).

import { t } from "./i18n.js";




export class RpcError extends Error {
  constructor(public code: number, message: string) {
    super(message);
  }
}

let nextId = 1;

export async function call<T>(method: string, params: Record<string, unknown> = {}): Promise<T> {
  const response = await fetch("/rpc", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ jsonrpc: "2.0", id: nextId++, method, params }),
  });
  if (!response.ok) throw new RpcError(response.status, t("widget.rpc.server_status", { status: response.status }));
  const answer = await response.json();
  if (answer.error) throw new RpcError(answer.error.code, answer.error.message);
  return answer.result as T;
}

/// Place in the work, as in the address (`/?work=1&key=12&rev=3&view=preview&pick=2:1&lq=…`, Round 8).
/// `work` missing → default work; `key` is the fixed key of the section (stays the same when the outline is
/// rearranged and takes precedence over `node`, the position, which is only a fallback of older addresses);
/// `rev` is the number of the revision (not its position); `pick` puts revisions into the preview
/// on trial (`<node>:<rev>,…`, the mark „aktuell“ stays); `lq` is the search in the
/// literature popup and is kept when navigating.
export interface Location {
  work?: number;
  key?: number;
  node?: number;
  rev?: number;
  view?: string;
  pick?: string;
  lq?: string;
  /// 0.9.0: switch "older revisions" open (`alle`), otherwise it is missing.
  fassungen?: string;
}

/// Answer of `outline.save`/`revision.save` (Round 22): `saved` „amended“ (continued, `amend_seq` = state),
/// „new“ (new revision `rev`, in the mode „auto“ with `reason`/`reason_label`) or „unchanged“.
export interface SaveAnswer {
  node: number;
  key?: number | null;
  rev?: number;
  saved?: "amended" | "new" | "unchanged";
  amend_seq?: number;
  reason?: string;
  reason_label?: string;
}

/// Feedback after saving in words: „fortgeschrieben (Stand k)“ or „neue Fassung n (Grund)“.
export function savedMessage(answer: SaveAnswer): string | null {
  switch (answer.saved) {
    case "amended":
      return t("widget.rpc.saved_amended", { seq: answer.amend_seq ?? "?" });
    case "new":
      return answer.reason_label
        ? t("widget.rpc.saved_new_reason", { rev: answer.rev ?? "?", reason: answer.reason_label })
        : t("widget.rpc.saved_new", { rev: answer.rev ?? "?" });
    case "unchanged":
      return t("widget.rpc.unchanged");
    default:
      return null;
  }
}

export interface Rendered {
  tree: string;
  editor: string;
  assistants: string;
  url: string;
  title: string;
}

export function locationFrom(url: string): Location {
  const query = new URL(url, window.location.href).searchParams;
  const number = (key: string) => {
    const value = query.get(key);
    return value === null || value === "" ? undefined : Number(value);
  };
  const lq = query.get("lq");
  const pick = query.get("pick");
  return {
    work: number("work"),
    key: number("key"),
    node: number("node"),
    rev: number("rev"),
    view: query.get("view") ?? undefined,
    pick: pick === null || pick.trim() === "" ? undefined : pick,
    lq: lq === null || lq.trim() === "" ? undefined : lq,
    fassungen: query.get("fassungen") === "alle" ? "alle" : undefined,
  };
}

/// Address for a place (counterpart of `locationFrom`): with the key `key` and without `node`
/// (key form); the position only if no key is known.
export function urlFor(at: Location): string {
  const query = new URLSearchParams();
  if (at.work !== undefined) query.set("work", String(at.work));
  if (at.key !== undefined) query.set("key", String(at.key));
  else if (at.node !== undefined) query.set("node", String(at.node));
  if (at.rev !== undefined) query.set("rev", String(at.rev));
  if (at.view === "preview") query.set("view", "preview");
  if (at.pick !== undefined && at.pick.trim() !== "") query.set("pick", at.pick);
  if (at.lq !== undefined && at.lq.trim() !== "") query.set("lq", at.lq);
  if (at.fassungen === "alle") query.set("fassungen", "alle");
  return `/?${query.toString()}`;
}
