// SPDX-License-Identifier: AGPL-3.0-or-later
// JSON-RPC 2.0 gegen den Baruch-Kern (`POST /rpc`).

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
  if (!response.ok) throw new RpcError(response.status, `Server antwortet ${response.status}`);
  const answer = await response.json();
  if (answer.error) throw new RpcError(answer.error.code, answer.error.message);
  return answer.result as T;
}

/// Stelle im Werk, wie in der Adresse (`/?work=1&key=12&rev=3&view=preview&pick=2:1&lq=…`, Runde 8).
/// `work` fehlt → Vorgabewerk; `key` ist der feste Schlüssel des Abschnitts (bleibt beim Umstellen der
/// Gliederung gleich und geht vor `node`, der Stelle, die nur noch Rückfall älterer Adressen ist);
/// `rev` ist die Nummer der Fassung (nicht ihre Stelle); `pick` setzt in der Vorschau Fassungen
/// probeweise ein (`<node>:<rev>,…`, die Markierung „aktuell“ bleibt); `lq` ist die Suche im
/// Literatur-Popup und bleibt beim Navigieren erhalten.
export interface Location {
  work?: number;
  key?: number;
  node?: number;
  rev?: number;
  view?: string;
  pick?: string;
  lq?: string;
}

/// Antwort von `outline.save`/`revision.save` (Runde 22): `saved` „amended“ (fortgeschrieben, `amend_seq` = Stand),
/// „new“ (neue Fassung `rev`, im Modus „auto“ mit `reason`/`reason_label`) oder „unchanged“.
export interface SaveAnswer {
  node: number;
  key?: number | null;
  rev?: number;
  saved?: "amended" | "new" | "unchanged";
  amend_seq?: number;
  reason?: string;
  reason_label?: string;
}

/// Rückmeldung nach dem Speichern in Worten: „fortgeschrieben (Stand k)“ bzw. „neue Fassung n (Grund)“.
export function savedMessage(answer: SaveAnswer): string | null {
  switch (answer.saved) {
    case "amended":
      return `Gespeichert: fortgeschrieben (Stand ${answer.amend_seq ?? "?"}).`;
    case "new":
      return answer.reason_label
        ? `Gespeichert: neue Fassung ${answer.rev ?? "?"} (${answer.reason_label}).`
        : `Gespeichert: neue Fassung ${answer.rev ?? "?"}.`;
    case "unchanged":
      return "Nichts zu speichern: Der Text ist unverändert.";
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
  };
}

/// Adresse zu einer Stelle (Gegenstück zu `locationFrom`): mit Schlüssel `key` und ohne `node`
/// (Key-Form); die Stelle nur, wenn kein Schlüssel bekannt ist.
export function urlFor(at: Location): string {
  const query = new URLSearchParams();
  if (at.work !== undefined) query.set("work", String(at.work));
  if (at.key !== undefined) query.set("key", String(at.key));
  else if (at.node !== undefined) query.set("node", String(at.node));
  if (at.rev !== undefined) query.set("rev", String(at.rev));
  if (at.view === "preview") query.set("view", "preview");
  if (at.pick !== undefined && at.pick.trim() !== "") query.set("pick", at.pick);
  if (at.lq !== undefined && at.lq.trim() !== "") query.set("lq", at.lq);
  return `/?${query.toString()}`;
}
