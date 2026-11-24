// SPDX-License-Identifier: AGPL-3.0-or-later
// Widget „chat“ (Runde 8, Abzeichen „neu“ seit Runde 15): Abschnitts-Chat ans Ende rollen und Beiträge
// markieren, die seit dem letzten Besuch dieses Abschnitts dazukamen (Klasse `post-new` und seit Runde 18a das
// Wort „neu“ als `span.post-new-label` hinter dem Namen). Der Server liefert je Beitrag
// `data-post-created` (Zeit des Servers, vergleichbar als Text), bei Beiträgen des Menschen `data-post-own` und
// im Akkordeon-Kopf das Abzeichen `.chat-count` mit der Zahl; ohne JavaScript bleibt es bei der Zahl.
//
// Gemerkt wird in LocalStorage `baruch_chat_seen` = {"<work>-<key>": "<größtes created>" | "", "w:<work>":
// "<größtes created im Werk>"}. Regeln (Runde 15):
// - Eigene Beiträge (`data-post-own`) zählen nie als neu – auch nicht, wenn sie nach dem Neuzeichnen erscheinen.
// - Erster Besuch eines Abschnitts: neu sind fremde Beiträge seit dem letzten Besuch irgendeines Abschnitts
//   dieses Werks (`w:<work>`); gab es auch den nicht, ist nichts neu (erste Nutzung).
// - Leerer Chat: gemerkt wird „leer“ (""), keine Uhrzeit des Browsers – beim nächsten Besuch ist dann jeder
//   fremde Beitrag neu. Verglichen werden nur Zeiten des Servers.
// Jeder Zugriff auf LocalStorage steht in try/catch (gesperrt, voll, privates Fenster): Dann gibt es eben keine
// Markierung.
//
// Runde 28 (chat-auftrag), Chat bedienbar: Der Chat rollt in sich (Höhe in app.css begrenzt) und steht beim Öffnen am
// neuesten Beitrag; ist er nicht unten, zeigt „Neue Beiträge ↓“ (`[data-chat-jump]`) den Weg zurück. „Ältere
// Beiträge“ (`[data-chat-older]`, ohne JavaScript ein Link auf /beitraege) lädt über RPC `post.page` nach. Je Beitrag
// „Kopieren“ (`[data-post-copy]`, Zwischenablage; ohne JavaScript das markierbare Feld auf /beitrag) und „Zitieren“
// (`[data-post-quote]`, „> Autor: …“ ins Eingabefeld). Die Knöpfe stehen mit `hidden` im HTML und erscheinen erst
// hier – ohne JavaScript täten sie nichts. `enableCopy` nutzen auch /beitraege, /beitrag („chat-page“) und /faden.
//
// Runde 30c (chat-bedienung):
// - „Kopieren“ liefert den Klartext des Servers (`pre.post-plain`, „- “-Zeilen, ohne Auszeichnung).
// - ✕ Löschen (`a[data-post-delete]`, ohne JavaScript ein Link auf /beitrag#loeschen): Rückfrage mit `confirm`, dann RPC
//   `post.delete {confirm: true}`; der Beitrag verschwindet ohne Neuladen. Scheitert der Aufruf, führt der Link weiter.
// - Live ([`startLive`]): Solange das Chat-Akkordeon offen und die Seite sichtbar ist, fragt das Widget alle
//   [`POLL_MS`] ms `post.since {key, after}` ab (sofort nach dem Ende eines SSE-Stroms: Ereignis `baruch:chat-refresh`
//   aus assist.ts). Neue Beiträge kommen als fertiges HTML des Servers (Makro `post_entry`) ans Ende, „@x schreibt …“
//   und Fehler gescheiterter Modellaufrufe stehen in `div[data-chat-live]` (role=status). Ohne JavaScript bleibt es bei
//   der Weiterleitung auf den Chat mit der Antwort. Dieselbe Abfrage nutzt der Faden (`thread.since`, threads.ts).
// - `a[data-jump]` in Beiträgen öffnet jump.ts (suchen-verweise); dieses Widget fasst sie nicht an.

import { call } from "./rpc.js";
import { register } from "./widgets.js";

const STORAGE = "baruch_chat_seen";
/// Höchstzahl gemerkter Einträge; ältere fallen heraus.
const MAX_ENTRIES = 500;
/// Gemerkter Wert für „beim letzten Besuch war der Chat leer“.
export const EMPTY = "";

type Seen = Record<string, string>;

function readSeen(): Seen {
  try {
    const raw = window.localStorage.getItem(STORAGE);
    if (!raw) return {};
    const parsed: unknown = JSON.parse(raw);
    if (parsed === null || typeof parsed !== "object" || Array.isArray(parsed)) return {};
    const out: Seen = {};
    for (const [key, value] of Object.entries(parsed as Record<string, unknown>)) {
      if (typeof value === "string") out[key] = value;
    }
    return out;
  } catch {
    return {};
  }
}

function writeSeen(seen: Seen): void {
  try {
    const keys = Object.keys(seen);
    for (const key of keys.slice(0, Math.max(0, keys.length - MAX_ENTRIES))) delete seen[key];
    window.localStorage.setItem(STORAGE, JSON.stringify(seen));
  } catch {
    // LocalStorage gesperrt oder voll: nichts merken.
  }
}

/// Welche Beiträge sind neu? `last` = gemerkter Stand des Abschnitts (`undefined` = nie besucht, `""` = war
/// leer), `workLast` = gemerkter Stand des Werks (für den ersten Besuch). Eigene Beiträge sind nie neu.
export function newSince(created: string[], own: boolean[], last: string | undefined, workLast?: string): boolean[] {
  const since = last ?? (workLast === EMPTY ? undefined : workLast);
  return created.map((c, i) => {
    if (own[i] || c === "" || since === undefined) return false;
    return since === EMPTY || c > since;
  });
}

/// Größter Zeitpunkt (ISO-Zeiten sind als Text vergleichbar); `before`, wenn nichts größer ist.
export function latest(created: string[], before: string | undefined): string | undefined {
  const start = before === EMPTY ? undefined : before;
  return created.reduce<string | undefined>((max, c) => (c !== "" && (max === undefined || c > max) ? c : max), start);
}

/// Neu einreihen, damit die Kürzung die ältesten Einträge trifft.
function remember(seen: Seen, key: string, value: string): void {
  delete seen[key];
  seen[key] = value;
}

/// Beiträge markieren, die seit dem letzten Besuch dazukamen (siehe Kopf), und den Stand merken.
function markNew(chat: HTMLElement): void {
  const work = chat.dataset.work;
  const key = chat.dataset.key;
  if (!work || !key) return;
  const id = `${work}-${key}`;
  const workId = `w:${work}`;
  const posts = [...chat.querySelectorAll<HTMLElement>("[data-post-created]")];
  const created = posts.map((p) => p.dataset.postCreated ?? "");
  const own = posts.map((p) => p.hasAttribute("data-post-own"));
  const seen = readSeen();
  const marks = newSince(created, own, seen[id], seen[workId]);
  let fresh = 0;
  posts.forEach((post, i) => {
    if (!marks[i]) return;
    post.classList.add("post-new");
    // Runde 18a: „neu“ auch als Wort hinter dem Namen, nicht nur als farbiger Rand (Farbe allein reicht nicht).
    const author = post.querySelector<HTMLElement>(".post-author");
    if (author && !author.querySelector(".post-new-label")) {
      const label = document.createElement("span");
      label.className = "badge post-new-label";
      label.textContent = "neu";
      label.title = "Neu seit dem letzten Besuch dieses Abschnitts";
      author.append(" ", label);
    }
    fresh += 1;
  });
  const badge = chat.closest("details")?.querySelector<HTMLElement>(".chat-count");
  if (badge && fresh > 0) {
    const note = document.createElement("span");
    note.className = "chat-new";
    note.textContent = ` · ${fresh} neu`;
    badge.append(note);
    badge.title = `${badge.title} · ${fresh} neu seit dem letzten Besuch`;
  }
  const newest = latest(created, seen[id]);
  if (newest === undefined) {
    // Leerer Chat: „leer“ merken (keine Uhrzeit des Browsers).
    if (seen[id] !== EMPTY) remember(seen, id, EMPTY);
  } else if (newest !== seen[id]) {
    remember(seen, id, newest);
  }
  const workNewest = latest(created, seen[workId]);
  if (workNewest !== undefined && workNewest !== seen[workId]) remember(seen, workId, workNewest);
  writeSeen(seen);
}

// ---- Runde 28: Kopieren, Zitieren, Rollen, ältere Beiträge ----------------------------------------------------

/// Höchstens so viele Zeichen des zitierten Beitrags im (einzeiligen) Eingabefeld.
export const QUOTE_CHARS = 300;
/// Abstand zum unteren Rand (Pixel), ab dem der Chat als „unten“ gilt.
const NEAR_BOTTOM = 24;

/// Zitat für das einzeilige Eingabefeld: „> Autor: Text“, Leerraum zusammengezogen, lange Texte mit „…“ gekürzt,
/// danach ein Leerzeichen für die eigene Antwort.
export function quoteLine(author: string, body: string): string {
  const flat = body.replace(/\s+/g, " ").trim();
  const cut = flat.length > QUOTE_CHARS ? `${flat.slice(0, QUOTE_CHARS).trimEnd()} …` : flat;
  return `> ${author}: ${cut} `;
}

/// Text eines Beitrags: der Klartext des Servers (Runde 30c), das markierbare Feld der Beitragsseite oder der Text im
/// Chat.
export function postText(item: Element): string {
  const plain = item.querySelector<HTMLElement>("pre.post-plain");
  if (plain?.textContent) return plain.textContent;
  const field = item.querySelector<HTMLTextAreaElement>("textarea.post-copy-text");
  if (field) return field.value;
  return item.querySelector(".post-body")?.textContent ?? "";
}

/// In die Zwischenablage; ohne Clipboard-API (unsicherer Ursprung, alter Browser) über ein verstecktes Textfeld.
export async function copyText(text: string): Promise<boolean> {
  try {
    if (navigator.clipboard && window.isSecureContext) {
      await navigator.clipboard.writeText(text);
      return true;
    }
  } catch {
    // weiter mit dem Rückfall
  }
  const area = document.createElement("textarea");
  area.value = text;
  area.setAttribute("readonly", "");
  area.style.position = "fixed";
  area.style.opacity = "0";
  document.body.append(area);
  area.select();
  let ok = false;
  try {
    ok = document.execCommand("copy");
  } catch {
    ok = false;
  }
  area.remove();
  return ok;
}

/// „Kopieren“ je Beitrag sichtbar machen und bedienen (einmal je Wurzel).
export function enableCopy(root: ParentNode): void {
  for (const button of root.querySelectorAll<HTMLButtonElement>("[data-post-copy]")) {
    if (button.dataset.copyReady) continue;
    button.dataset.copyReady = "1";
    button.hidden = false;
    button.addEventListener("click", () => {
      const item = button.closest(".post-item");
      if (!item) return;
      void copyText(postText(item)).then((ok) => {
        const before = button.textContent;
        button.textContent = ok ? "Kopiert" : "Kopieren ging nicht – Text markieren";
        window.setTimeout(() => (button.textContent = before), 1500);
      });
    });
  }
}

/// „Zitieren“ je Beitrag: „> Autor: …“ an den Anfang des Eingabefelds des Chats.
function enableQuote(chat: HTMLElement): void {
  const input = chat.closest(".acc-body")?.querySelector<HTMLInputElement>(".chat-input input[name=message]");
  for (const button of chat.querySelectorAll<HTMLButtonElement>("[data-post-quote]")) {
    if (!input || button.dataset.quoteReady) continue;
    button.dataset.quoteReady = "1";
    button.hidden = false;
    button.addEventListener("click", () => {
      const item = button.closest<HTMLElement>(".post-item");
      if (!item) return;
      const quote = quoteLine(item.dataset.postAuthor ?? "", postText(item));
      input.value = input.value.startsWith(quote) ? input.value : quote + input.value;
      input.focus();
      input.setSelectionRange(input.value.length, input.value.length);
    });
  }
}

function nearBottom(chat: HTMLElement): boolean {
  return chat.scrollHeight - chat.scrollTop - chat.clientHeight <= NEAR_BOTTOM;
}

/// An den neuesten Beitrag rollen.
export function toNewest(chat: HTMLElement): void {
  chat.scrollTop = chat.scrollHeight;
}

/// „Neue Beiträge ↓“: sichtbar, solange der Chat nicht unten steht; ein Klick rollt hinunter.
function enableJump(chat: HTMLElement): void {
  const jump = chat.parentElement?.querySelector<HTMLButtonElement>("[data-chat-jump]");
  if (!jump) return;
  const update = () => (jump.hidden = nearBottom(chat));
  chat.addEventListener("scroll", update, { passive: true });
  jump.addEventListener("click", () => {
    toNewest(chat);
    update();
  });
  update();
}

type PagePost = { id: number; author: string; body: string; time: string; edited?: string | null; style?: string | null; url: string };

/// Einen nachgeladenen Beitrag bauen (schlicht: Autor, Zeit, Text, Kopieren/Zitieren, „Beitrag öffnen“ für alles
/// Weitere). Text nur über textContent, Farbe nur aus dem Server (geprüfte Variablen).
function olderItem(post: PagePost): HTMLElement {
  const item = document.createElement("div");
  item.className = "post-item";
  item.id = `post-${post.id}`;
  item.dataset.postId = String(post.id);
  item.dataset.postAuthor = post.author;
  const box = document.createElement("div");
  box.className = post.style ? "post pc" : "post";
  if (post.style) box.setAttribute("style", post.style);
  const author = document.createElement("div");
  author.className = "post-author";
  author.textContent = post.author;
  const time = document.createElement("span");
  time.className = "muted tiny post-time";
  time.textContent = ` ${post.time}`;
  author.append(time);
  if (post.edited) {
    const edited = document.createElement("span");
    edited.className = "muted tiny post-edited";
    edited.textContent = " bearbeitet";
    author.append(edited);
  }
  const body = document.createElement("div");
  body.className = "post-body";
  body.textContent = post.body;
  const tools = document.createElement("div");
  tools.className = "post-tools";
  for (const [attr, label] of [["data-post-copy", "Kopieren"], ["data-post-quote", "Zitieren"]]) {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "tool small";
    button.setAttribute(attr, "");
    button.hidden = true;
    button.textContent = label;
    tools.append(button);
  }
  const open = document.createElement("a");
  open.className = "tool small";
  open.href = post.url;
  open.textContent = "Beitrag öffnen";
  tools.append(open);
  box.append(author, body, tools);
  item.append(box);
  return item;
}

/// „Ältere Beiträge“: über `post.page` vor dem ersten gezeigten Beitrag nachladen, ohne die Stelle zu verlieren.
function enableOlder(chat: HTMLElement, work: number, key: number): void {
  const link = chat.querySelector<HTMLAnchorElement>("[data-chat-older]");
  if (!link) return;
  link.addEventListener("click", (event) => {
    event.preventDefault();
    const before = Number(link.dataset.before);
    const height = chat.scrollHeight;
    void call<{ posts: PagePost[]; older: number }>("post.page", { work, key, before })
      .then((page) => {
        const items = page.posts.map(olderItem);
        link.after(...items);
        enableCopy(chat);
        enableQuote(chat);
        chat.scrollTop += chat.scrollHeight - height;
        if (page.older > 0 && page.posts.length > 0) {
          link.dataset.before = String(page.posts[0].id);
          link.textContent = `Ältere Beiträge (${page.older}) …`;
        } else {
          link.remove();
        }
      })
      .catch(() => {
        // Ohne Antwort bleibt der Link: Er führt auf die ganze Chat-Historie (/beitraege).
        window.location.href = link.href;
      });
  });
}

// ---- Runde 30c: Löschen, Live-Anzeige ---------------------------------------------------------------------------

/// Abstand der Abfragen (ms).
export const POLL_MS = 3000;

/// Frage vor dem Löschen.
export function deleteQuestion(id: number, author: string): string {
  return `Beitrag ${id} von ${author} löschen? Der Spiegel hält ihn im Lösch-Commit fest.`;
}

/// ✕ je Beitrag: Rückfrage, RPC `post.delete`, Beitrag entfernen (einmal je Link).
export function enableDelete(root: ParentNode): void {
  for (const link of root.querySelectorAll<HTMLAnchorElement>("a[data-post-delete]")) {
    if (link.dataset.deleteReady) continue;
    link.dataset.deleteReady = "1";
    link.addEventListener("click", (event) => {
      if (event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey) return;
      const item = link.closest<HTMLElement>(".post-item");
      const id = Number(item?.dataset.postId);
      const work = Number(link.dataset.work);
      if (!item || !id || !work) return;
      event.preventDefault();
      if (!window.confirm(deleteQuestion(id, item.dataset.postAuthor ?? ""))) return;
      void call("post.delete", { work, id, confirm: true })
        .then(() => {
          const badge = item.closest("details")?.querySelector<HTMLElement>(".chat-count");
          item.remove();
          if (badge) {
            const count = Math.max(0, Number(badge.dataset.chatCount ?? "1") - 1);
            badge.dataset.chatCount = String(count);
            if (badge.firstChild) badge.firstChild.textContent = String(count);
          }
        })
        .catch(() => {
          // Ohne Antwort führt der Link auf die Bestätigungsseite (wie ohne JavaScript).
          window.location.href = link.href;
        });
    });
  }
}

/// Antwort von `post.since` bzw. `thread.since`.
export type SinceAnswer = {
  posts: { id: number; author: string; html: string }[];
  typing: { persona: string; since: string }[];
  failures: { persona: string; message: string; at: string }[];
};

/// Zeile „@lektor schreibt …“ (mehrere mit „und“).
export function typingLine(typing: { persona: string }[]): string {
  const names = [...new Set(typing.map((t) => t.persona))];
  if (names.length === 0) return "";
  const list = names.length === 1 ? names[0] : `${names.slice(0, -1).join(", ")} und ${names[names.length - 1]}`;
  return `${list} ${names.length === 1 ? "schreibt" : "schreiben"} …`;
}

/// Fehler, die noch nicht gezeigt wurden (Schlüssel aus Zeit und Persona), und die Schlüssel danach.
export function freshFailures<T extends { persona: string; at: string }>(failures: T[], seen: Set<string>): T[] {
  return failures.filter((f) => {
    const id = `${f.at}|${f.persona}`;
    if (seen.has(id)) return false;
    seen.add(id);
    return true;
  });
}

/// Schon gezeigte Fehler (über das Neuzeichnen hinweg, solange die Seite offen ist).
const SEEN_FAILURES = new Set<string>();

/// Größte id unter den Beiträgen der Wurzel.
function lastPostId(root: ParentNode): number {
  let max = 0;
  for (const item of root.querySelectorAll<HTMLElement>("[data-post-id]")) {
    const id = Number(item.dataset.postId);
    if (id > max) max = id;
  }
  return max;
}

export type LiveOptions = {
  /// Wurzel der Beiträge (neue kommen vor `before` bzw. ans Ende).
  list: HTMLElement;
  before: Element | null;
  /// Anzeige „schreibt …“ und Fehler.
  status: HTMLElement | null;
  /// Läuft die Abfrage gerade (Akkordeon offen, Seite sichtbar)?
  active: () => boolean;
  /// Abfrage nach `after`.
  fetch: (after: number) => Promise<SinceAnswer>;
  /// Nach dem Einfügen neuer Beiträge (Knöpfe bedienbar machen, rollen).
  inserted: (items: HTMLElement[]) => void;
};

/// Live-Abfrage starten (siehe Kopf); endet von selbst, wenn die Wurzel aus dem Dokument verschwindet.
export function startLive(options: LiveOptions): void {
  let busy = false;
  const tick = async (force: boolean) => {
    if (busy || !options.list.isConnected) return;
    if (!force && (!options.active() || document.visibilityState !== "visible")) return;
    busy = true;
    try {
      const answer = await options.fetch(lastPostId(options.list));
      const items: HTMLElement[] = [];
      for (const post of answer.posts) {
        if (options.list.querySelector(`[data-post-id="${post.id}"]`)) continue;
        const template = document.createElement("template");
        // HTML des Servers (Makro der Vorlage, alles maskiert); kein Text aus dem Browser.
        template.innerHTML = post.html.trim();
        const item = template.content.firstElementChild;
        if (item instanceof HTMLElement) {
          options.list.insertBefore(item, options.before);
          items.push(item);
        }
      }
      if (items.length > 0) options.inserted(items);
      if (options.status) {
        const lines: string[] = [];
        const typing = typingLine(answer.typing);
        if (typing) lines.push(typing);
        for (const f of freshFailures(answer.failures, SEEN_FAILURES)) {
          lines.push(`${f.persona ? `${f.persona}: ` : ""}${f.message}`);
        }
        options.status.replaceChildren(
          ...lines.map((line) => {
            const div = document.createElement("div");
            div.textContent = line;
            if (!line.endsWith("…")) div.className = "chat-live-error";
            return div;
          }),
        );
        options.status.hidden = lines.length === 0;
      }
    } catch {
      // Abfrage gescheitert (Server weg, Werk gewechselt): beim nächsten Mal wieder.
    } finally {
      busy = false;
    }
  };
  const timer = window.setInterval(() => {
    if (!options.list.isConnected) {
      window.clearInterval(timer);
      document.removeEventListener("baruch:chat-refresh", refresh);
      return;
    }
    void tick(false);
  }, POLL_MS);
  const refresh = () => void tick(true);
  document.addEventListener("baruch:chat-refresh", refresh);
}

register("chat", (chat) => {
  const work = Number(chat.dataset.work);
  const key = Number(chat.dataset.key);
  const rev = Number(chat.dataset.rev);
  enableCopy(chat);
  enableQuote(chat);
  enableJump(chat);
  enableDelete(chat);
  if (work && key) enableOlder(chat, work, key);
  markNew(chat);
  toNewest(chat);
  if (work && key) {
    const details = chat.closest("details");
    startLive({
      list: chat,
      before: chat.querySelector(".chat-end"),
      status: chat.closest(".acc-body")?.querySelector<HTMLElement>("[data-chat-live]") ?? null,
      active: () => !details || details.open,
      fetch: (after) => call<SinceAnswer>("post.since", rev ? { work, key, after, rev } : { work, key, after }),
      inserted: (items) => {
        const bottom = nearBottom(chat);
        chat.querySelector(":scope > p.muted")?.remove();
        enableCopy(chat);
        enableQuote(chat);
        enableDelete(chat);
        for (const item of items) {
          item.classList.add("post-new");
          // Formulare der neuen Beiträge (data-rpc) bedient der Arbeitsbereich über seinen Listener am Wurzelelement.
        }
        const badge = details?.querySelector<HTMLElement>(".chat-count");
        if (badge) {
          const count = Number(badge.dataset.chatCount ?? "0") + items.length;
          badge.dataset.chatCount = String(count);
          if (badge.firstChild) badge.firstChild.textContent = String(count);
        }
        if (bottom) toNewest(chat);
      },
    });
  }
});

// Seiten /beitraege und /beitrag: „Kopieren“; /beitraege steht beim Öffnen am neuesten Beitrag (wie der Anker).
register("chat-page", (page) => {
  enableCopy(page);
  enableDelete(page);
  if (!window.location.hash && page.querySelector(".posts-list")) {
    page.querySelector<HTMLElement>("#chat-neueste")?.scrollIntoView({ block: "end" });
  }
});
