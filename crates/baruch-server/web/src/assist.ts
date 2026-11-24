// SPDX-License-Identifier: AGPL-3.0-or-later
// Widget „assist“ (Runde 10): Formulare „Prüfen“, „Vorschlag“ und die Chat-Eingabe „@persona fragen …“.
// Ohne JavaScript gehen sie an POST /assistenten/act (Seite danach neu). Mit JavaScript schickt das Widget
// dieselben Felder als JSON an POST /assistenten/stream und zeigt die Antwort Wort für Wort in einem
// vorläufigen Beitrag in der Farbe der Persona, mit „Abbrechen“ (AbortController: der Server bricht den
// Modellaufruf ab). Bei `done` zeichnet `baruch:navigate {url}` die Bereiche neu.
// Ereignisse: start {persona, style}, delta {text}, posted {post}, retry {persona, reason},
// compressed {message, summarized, chunks}, piece {persona, index, count} (Runde 18b), done {kind, url, compressed, …},
// error {message}.
// Runde 15: `posted` zeigt den eigenen Beitrag sofort im Chat; `retry` verwirft das bisher Gezeigte der Persona
// und sagt warum; `compressed` meldet sich als Toast (`baruch:toast`, Stufe „warn“, Empfänger toast.ts).
// Abbrechen meldet je nach Art richtig: im Chat sind der eigene Beitrag und fertige Antworten gespeichert (die
// Seite wird neu gezeichnet), beim Prüfen/Fragen/Vorschlag ist nichts gespeichert.
// Runde 30c: Am Ende jedes Stroms meldet `baruch:chat-refresh` dem Chat, sofort nach neuen Beiträgen zu fragen; das Feld
// `basis` (Prüfgrundlage) geht wie jedes Formularfeld mit.

import { register } from "./widgets.js";

export type StreamEvent = { event: string; data: Record<string, unknown> };

/// SSE-Text in Ereignisse zerlegen; gibt die fertigen Ereignisse und den unvollständigen Rest zurück.
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
      // Unlesbares Ereignis übergehen.
    }
  }
  return { events, rest };
}

const NUMERIC = new Set(["work", "key", "rev"]);

/// Meldung beim Abbrechen: im Chat ist schon etwas gespeichert, bei Einzelaufträgen nichts.
export function cancelMessage(method: string): string {
  return method === "persona.chat"
    ? "Abgebrochen. Dein Beitrag und schon fertige Antworten sind gespeichert; die laufende Antwort nicht."
    : "Abgebrochen; es wurde nichts gespeichert.";
}

/// Hinweis über den Toast (Vertrag Runde 15, §4); toast.ts zeigt ihn.
export function toast(message: string, level: "warn" | "info" = "warn"): void {
  document.dispatchEvent(new CustomEvent("baruch:toast", { detail: { message, level } }));
}

/// Fertigen eigenen Beitrag (SSE `posted`) sofort in den Chat setzen – vor den vorläufigen Beitrag.
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

/// Vorläufiger Beitrag am Ende des Abschnitts-Chats (Akkordeon „Abschnitts-Chat“ wird geöffnet).
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
  status.textContent = "wartet auf das Modell …";
  author.append(status);
  const body = document.createElement("div");
  body.className = "post-body";
  const cancel = document.createElement("button");
  cancel.type = "button";
  cancel.className = "btn small assist-cancel";
  cancel.textContent = "Abbrechen";
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
  // Teil des vorläufigen Beitrags je Persona bzw. je Stück (bei `retry` geleert, bei `piece` neu).
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
          ui.status.textContent = `${String(data.persona ?? "")} schreibt …`;
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
          // Runde 18b: Das nächste Stück eines gestückelten Auftrags beginnt; frühere Stücke bleiben stehen.
          segment = document.createElement("span");
          ui.body.append(segment);
        } else if (event === "retry") {
          // Neu angefordert: das bisher Gezeigte dieser Persona verwerfen – seit Runde 18b nur das laufende Stück
          // (frühere Stücke sind fertig geprüft) – und den Grund nennen.
          segment.textContent = "";
          ui.status.textContent = `${String(data.persona ?? "")}: Antwort wird neu angefordert (${String(data.reason ?? "")}) …`;
        } else if (event === "compressed") {
          toast(String(data.message ?? "Komprimiert."));
          toasted = true;
        } else if (event === "error") {
          errors.push(String(data.message ?? "Fehler"));
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
    // Runde 30c: Ende des Stroms – der Chat fragt sofort nach neuen Beiträgen (chat.ts, `post.since`).
    document.dispatchEvent(new CustomEvent("baruch:chat-refresh"));
  }
  const summary = done?.compressed;
  if (!toasted && summary !== null && typeof summary === "object") {
    toast(String((summary as Record<string, unknown>).message ?? "Komprimiert."));
  }
  if (!done && controller.signal.aborted && method === "persona.chat") {
    // Im Chat ist der eigene Beitrag (und jede fertige Antwort) gespeichert: neu zeichnen, damit er stehen bleibt.
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
  ui.status.textContent = controller.signal.aborted && method === "persona.chat" ? "abgebrochen" : "nicht gespeichert";
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
