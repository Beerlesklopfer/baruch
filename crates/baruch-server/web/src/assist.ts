// SPDX-License-Identifier: AGPL-3.0-or-later
// Widget „assist“ (Round 10): forms „Prüfen“, „Vorschlag“ and the chat input „@persona fragen …“.
// Without JavaScript they go to POST /assistenten/act (page reloaded afterwards). With JavaScript the widget sends
// the same fields as JSON to POST /assistenten/stream and shows the answer word by word in a
// provisional post in the colour of the persona, with „Abbrechen“ (AbortController: the server aborts the
// model call). On `done`, `baruch:navigate {url}` redraws the areas.
// Events: start {persona, style}, delta {text}, posted {post}, retry {persona, reason},
// compressed {message, summarized, chunks}, piece {persona, index, count} (Round 18b), done {kind, url, compressed, …},
// error {message}.
// Round 15: `posted` shows the own post in the chat at once; `retry` discards what was shown of the persona so far
// and says why; `compressed` reports as a toast (`baruch:toast`, level „warn“, receiver toast.ts).
// Aborting reports correctly depending on the kind: in the chat the own post and finished answers are saved (the
// page is redrawn), for checking/asking/proposing nothing is saved.
// Round 30c: at the end of every stream `baruch:chat-refresh` tells the chat to ask for new posts at once; the field
// `basis` (review basis) goes along like every form field.

import { t } from "./i18n.js";
import { register } from "./widgets.js";

export type StreamEvent = { event: string; data: Record<string, unknown> };

/// Split SSE text into events; returns the finished events and the incomplete rest.
export function parseEvents(buffer: string): { events: StreamEvent[]; rest: string } {
  const events: StreamEvent[] = [];
  const normalized = buffer.replace(/\r\n/g, "\n");
  const blocks = normalized.split("\n\n");
  const rest = blocks.pop() ?? "";
  for (const block of blocks) {
    let event = "message";
    const data: string[] = [];
    for (const line of block.split("\n")) {
      if (line.startsWith("event:")) event = line.slice(6).trim();
      else if (line.startsWith("data:")) data.push(line.slice(5).replace(/^ /, ""));
    }
    if (data.length === 0) continue;
    try {
      const parsed: unknown = JSON.parse(data.join("\n"));
      if (parsed !== null && typeof parsed === "object") events.push({ event, data: parsed as Record<string, unknown> });
    } catch {
      // Skip an unreadable event.
    }
  }
  return { events, rest };
}

const NUMERIC = new Set(["work", "key", "rev"]);

/// Message on aborting: in the chat something is already saved, for single jobs nothing.
export function cancelMessage(method: string): string {
  return method === "persona.chat"
    ? t("widget.assist.cancelled_chat")
    : t("widget.assist.cancelled_nothing");
}

/// Notice through the toast (contract Round 15, §4); toast.ts shows it.
export function toast(message: string, level: "warn" | "info" = "warn"): void {
  document.dispatchEvent(new CustomEvent("baruch:toast", { detail: { message, level } }));
}

/// Put the finished own post (SSE `posted`) into the chat at once – before the provisional post.
function showPosted(before: HTMLElement, post: Record<string, unknown>): void {
  const item = document.createElement("div");
  item.className = "post-item";
  item.dataset.postId = String(post.id ?? "");
  item.setAttribute("data-post-own", "");
  const box = document.createElement("div");
  box.className = "post";
  const author = document.createElement("div");
  author.className = "post-author";
  author.textContent = String(post.author ?? "");
  const body = document.createElement("div");
  body.className = "post-body";
  body.textContent = String(post.body ?? "");
  box.append(author, body);
  item.append(box);
  before.before(item);
}

function formParams(form: HTMLFormElement, submitter: HTMLElement | null): Record<string, unknown> {
  const data = new FormData(form);
  if (submitter instanceof HTMLButtonElement && submitter.name) data.set(submitter.name, submitter.value);
  const params: Record<string, unknown> = {};
  for (const [name, value] of data) {
    if (name === "view" || name === "lq") continue;
    const text = String(value);
    if (NUMERIC.has(name)) {
      if (text.trim() !== "") params[name] = Number(text);
    } else {
      params[name] = text;
    }
  }
  return params;
}

/// Provisional post at the end of the section chat (the accordion „Abschnitts-Chat“ is opened).
function pendingPost(form: HTMLFormElement): { box: HTMLElement; body: HTMLElement; status: HTMLElement; cancel: HTMLButtonElement } {
  const root = form.closest(".accordion") ?? document;
  const chat = root.querySelector<HTMLElement>(".chat[data-widget=chat]");
  const details = chat?.closest("details");
  if (details) details.open = true;
  const box = document.createElement("div");
  box.className = "post pc pending";
  box.setAttribute("aria-live", "polite");
  const author = document.createElement("div");
  author.className = "post-author";
  const status = document.createElement("span");
  status.className = "assist-status";
  status.textContent = t("widget.assist.waiting");
  author.append(status);
  const body = document.createElement("div");
  body.className = "post-body";
  const cancel = document.createElement("button");
  cancel.type = "button";
  cancel.className = "btn small assist-cancel";
  cancel.textContent = t("widget.assist.cancel");
  box.append(author, body, cancel);
  (chat ?? form.parentElement ?? document.body).append(box);
  return { box, body, status, cancel };
}

async function stream(form: HTMLFormElement, submitter: HTMLElement | null): Promise<void> {
  const params = formParams(form, submitter);
  const method = String(params.method ?? "");
  const view = new FormData(form).get("view");
  const ui = pendingPost(form);
  const controller = new AbortController();
  ui.cancel.addEventListener("click", () => controller.abort());
  const buttons = [...form.querySelectorAll<HTMLButtonElement>("button")];
  buttons.forEach((b) => (b.disabled = true));
  let done: Record<string, unknown> | null = null;
  const errors: string[] = [];
  // Part of the provisional post per persona or per piece (emptied on `retry`, new on `piece`).
  let segment: HTMLElement = ui.body;
  let toasted = false;
  const location = (): string => {
    const url = new URL(window.location.href);
    return url.pathname + url.search;
  };
  try {
    const response = await fetch("/assistenten/stream", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(params),
      signal: controller.signal,
    });
    if (!response.ok || !response.body) throw new Error(await response.text());
    const reader = response.body.getReader();
    const decoder = new TextDecoder();
    let buffer = "";
    for (;;) {
      const { value, done: end } = await reader.read();
      if (end) break;
      buffer += decoder.decode(value, { stream: true });
      const parsed = parseEvents(buffer);
      buffer = parsed.rest;
      for (const { event, data } of parsed.events) {
        if (event === "start") {
          ui.status.textContent = t("widget.assist.writing", { persona: String(data.persona ?? "") });
          if (typeof data.style === "string") ui.box.setAttribute("style", data.style);
          if (ui.body.textContent) ui.body.append(document.createElement("hr"));
          segment = document.createElement("span");
          ui.body.append(segment);
        } else if (event === "delta") {
          segment.append(document.createTextNode(String(data.text ?? "")));
        } else if (event === "posted") {
          const post = data.post;
          if (post !== null && typeof post === "object") showPosted(ui.box, post as Record<string, unknown>);
        } else if (event === "piece") {
          // Round 18b: the next piece of a job split into pieces begins; earlier pieces stay.
          segment = document.createElement("span");
          ui.body.append(segment);
        } else if (event === "retry") {
          // Requested anew: discard what was shown of this persona so far – since Round 18b only the running piece
          // (earlier pieces are completely checked) – and name the reason.
          segment.textContent = "";
          ui.status.textContent = t("widget.assist.retry", { persona: String(data.persona ?? ""), reason: String(data.reason ?? "") });
        } else if (event === "compressed") {
          toast(String(data.message ?? t("widget.assist.compressed")));
          toasted = true;
        } else if (event === "error") {
          errors.push(String(data.message ?? t("widget.assist.error")));
        } else if (event === "done") {
          done = data;
        }
      }
    }
  } catch (error) {
    if (controller.signal.aborted) errors.push(cancelMessage(method));
    else errors.push(String((error as Error).message ?? error));
  } finally {
    buttons.forEach((b) => (b.disabled = false));
    // Round 30c: end of the stream – the chat asks for new posts at once (chat.ts, `post.since`).
    document.dispatchEvent(new CustomEvent("baruch:chat-refresh"));
  }
  const summary = done?.compressed;
  if (!toasted && summary !== null && typeof summary === "object") {
    toast(String((summary as Record<string, unknown>).message ?? t("widget.assist.compressed")));
  }
  if (!done && controller.signal.aborted && method === "persona.chat") {
    // In the chat the own post (and every finished answer) is saved: redraw so that it stays.
    document.dispatchEvent(new CustomEvent("baruch:navigate", { detail: { url: location() } }));
  }
  if (done && typeof done.url === "string") {
    let url = done.url;
    if (view === "preview" && !url.includes("view=preview")) url += "&view=preview";
    if (errors.length === 0) {
      ui.box.remove();
      document.dispatchEvent(new CustomEvent("baruch:navigate", { detail: { url } }));
      return;
    }
    document.dispatchEvent(new CustomEvent("baruch:navigate", { detail: { url } }));
  }
  ui.cancel.remove();
  ui.status.textContent = controller.signal.aborted && method === "persona.chat" ? t("widget.assist.status_cancelled") : t("widget.assist.status_not_saved");
  const message = document.createElement("div");
  message.className = "assist-error";
  message.setAttribute("role", "alert");
  message.textContent = errors.join("\n");
  ui.box.append(message);
}

register("assist", (form) => {
  if (!(form instanceof HTMLFormElement)) return;
  form.addEventListener("submit", (event) => {
    event.preventDefault();
    void stream(form, (event as SubmitEvent).submitter);
  });
});
