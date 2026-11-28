// SPDX-License-Identifier: AGPL-3.0-or-later
// Widget „chat“ (Round 8, badge „neu“ since Round 15): scroll the section chat to the end and mark posts
// that were added since the last visit of this section (class `post-new` and since Round 18a the
// word „neu“ as `span.post-new-label` behind the name). The server supplies per post
// `data-post-created` (time of the server, comparable as text), for posts of the human `data-post-own` and
// in the accordion head the badge `.chat-count` with the number; without JavaScript only the number remains.
//
// Remembered in LocalStorage `baruch_chat_seen` = {"<work>-<key>": "<largest created>" | "", "w:<work>":
// "<largest created in the work>"}. Rules (Round 15):
// - Own posts (`data-post-own`) never count as new – not even when they appear after the redraw.
// - First visit of a section: new are foreign posts since the last visit of any section
//   of this work (`w:<work>`); if there was none either, nothing is new (first use).
// - Empty chat: „leer“ ("") is remembered, not a clock time of the browser – on the next visit every
//   foreign post is then new. Only times of the server are compared.
// Every access to LocalStorage stands in try/catch (blocked, full, private window): then there simply is no
// marking.
//
// Round 28 (chat-auftrag), chat usable: the chat scrolls within itself (height limited in app.css) and stands at the
// newest post on opening; if it is not at the bottom, „Neue Beiträge ↓“ (`[data-chat-jump]`) shows the way back.
// „Ältere Beiträge“ (`[data-chat-older]`, without JavaScript a link to /beitraege) loads more through RPC `post.page`.
// Per post „Kopieren“ (`[data-post-copy]`, clipboard; without JavaScript the selectable field on /beitrag) and
// „Zitieren“ (`[data-post-quote]`, „> Autor: …“ into the input field). The buttons stand in the HTML with `hidden` and
// appear only here – without JavaScript they would do nothing. `enableCopy` is also used by /beitraege, /beitrag
// („chat-page“) and /faden.
//
// Round 30c (chat-bedienung):
// - „Kopieren“ supplies the plain text of the server (`pre.post-plain`, „- “ lines, without markup).
// - ✕ Löschen (`a[data-post-delete]`, without JavaScript a link to /beitrag#loeschen): confirmation with `confirm`,
//   then RPC `post.delete {confirm: true}`; the post disappears without reloading. If the call fails, the link leads
//   on.
// - Live ([`startLive`]): as long as the chat accordion is open and the page visible, the widget queries every
//   [`POLL_MS`] ms `post.since {key, after}` (at once after the end of an SSE stream: event `baruch:chat-refresh`
//   from assist.ts). New posts come as finished HTML of the server (macro `post_entry`) to the end, „@x schreibt …“
//   and errors of failed model calls stand in `div[data-chat-live]` (role=status). Without JavaScript the
//   redirect to the chat with the answer remains. The thread uses the same query (`thread.since`, threads.ts).
// - `a[data-jump]` in posts opens jump.ts (suchen-verweise); this widget does not touch them.

import { t } from "./i18n.js";
import { call } from "./rpc.js";
import { register } from "./widgets.js";

const STORAGE = "baruch_chat_seen";
/// Maximum number of remembered entries; older ones drop out.
const MAX_ENTRIES = 500;
/// Remembered value for "the chat was empty on the last visit".
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
    // LocalStorage blocked or full: remember nothing.
  }
}

/// Which posts are new? `last` = remembered state of the section (`undefined` = never visited, `""` = was
/// empty), `workLast` = remembered state of the work (for the first visit). Own posts are never new.
export function newSince(created: string[], own: boolean[], last: string | undefined, workLast?: string): boolean[] {
  const since = last ?? (workLast === EMPTY ? undefined : workLast);
  return created.map((c, i) => {
    if (own[i] || c === "" || since === undefined) return false;
    return since === EMPTY || c > since;
  });
}

/// Largest point in time (ISO times are comparable as text); `before` if nothing is larger.
export function latest(created: string[], before: string | undefined): string | undefined {
  const start = before === EMPTY ? undefined : before;
  return created.reduce<string | undefined>((max, c) => (c !== "" && (max === undefined || c > max) ? c : max), start);
}

/// Re-insert so that the trimming hits the oldest entries.
function remember(seen: Seen, key: string, value: string): void {
  delete seen[key];
  seen[key] = value;
}

/// Mark posts that were added since the last visit (see the header) and remember the state.
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
    // Round 18a: „neu“ also as a word behind the name, not only as a coloured border (colour alone is not enough).
    const author = post.querySelector<HTMLElement>(".post-author");
    if (author && !author.querySelector(".post-new-label")) {
      const label = document.createElement("span");
      label.className = "badge post-new-label";
      label.textContent = t("widget.chat.new_badge");
      label.title = t("widget.chat.new_title");
      author.append(" ", label);
    }
    fresh += 1;
  });
  const badge = chat.closest("details")?.querySelector<HTMLElement>(".chat-count");
  if (badge && fresh > 0) {
    const note = document.createElement("span");
    note.className = "chat-new";
    note.textContent = ` · ${t("widget.chat.new_count", { count: fresh })}`;
    badge.append(note);
    badge.title = `${badge.title} · ${t("widget.chat.new_since", { count: fresh })}`;
  }
  const newest = latest(created, seen[id]);
  if (newest === undefined) {
    // Empty chat: remember „leer“ (no clock time of the browser).
    if (seen[id] !== EMPTY) remember(seen, id, EMPTY);
  } else if (newest !== seen[id]) {
    remember(seen, id, newest);
  }
  const workNewest = latest(created, seen[workId]);
  if (workNewest !== undefined && workNewest !== seen[workId]) remember(seen, workId, workNewest);
  writeSeen(seen);
}

// ---- Round 28: copying, quoting, scrolling, older posts -------------------------------------------------------

/// At most this many characters of the quoted post in the (single-line) input field.
export const QUOTE_CHARS = 300;
/// Distance to the bottom edge (pixels) from which the chat counts as "at the bottom".
const NEAR_BOTTOM = 24;

/// Quotation for the single-line input field: „> Autor: Text“, whitespace collapsed, long texts shortened with „…“,
/// then a space for the own answer.
export function quoteLine(author: string, body: string): string {
  const flat = body.replace(/\s+/g, " ").trim();
  const cut = flat.length > QUOTE_CHARS ? `${flat.slice(0, QUOTE_CHARS).trimEnd()} …` : flat;
  return `> ${author}: ${cut} `;
}

/// Text of a post: the plain text of the server (Round 30c), the selectable field of the post page or the text in the
/// chat.
export function postText(item: Element): string {
  const plain = item.querySelector<HTMLElement>("pre.post-plain");
  if (plain?.textContent) return plain.textContent;
  const field = item.querySelector<HTMLTextAreaElement>("textarea.post-copy-text");
  if (field) return field.value;
  return item.querySelector(".post-body")?.textContent ?? "";
}

/// Into the clipboard; without the Clipboard API (insecure origin, old browser) through a hidden text field.
export async function copyText(text: string): Promise<boolean> {
  try {
    if (navigator.clipboard && window.isSecureContext) {
      await navigator.clipboard.writeText(text);
      return true;
    }
  } catch {
    // continue with the fallback
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

/// Make „Kopieren“ per post visible and operable (once per root).
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
        button.textContent = ok ? t("widget.chat.copied") : t("widget.chat.copy_failed");
        window.setTimeout(() => (button.textContent = before), 1500);
      });
    });
  }
}

/// „Zitieren“ per post: „> Autor: …“ at the beginning of the input field of the chat.
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

/// Scroll to the newest post.
export function toNewest(chat: HTMLElement): void {
  chat.scrollTop = chat.scrollHeight;
}

/// „Neue Beiträge ↓“: visible as long as the chat is not at the bottom; a click scrolls down.
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

/// Build a post loaded later (plain: author, time, text, Kopieren/Zitieren, „Beitrag öffnen“ for everything
/// else). Text only through textContent, colour only from the server (checked variables).
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
    edited.textContent = ` ${t("widget.chat.edited")}`;
    author.append(edited);
  }
  const body = document.createElement("div");
  body.className = "post-body";
  body.textContent = post.body;
  const tools = document.createElement("div");
  tools.className = "post-tools";
  for (const [attr, label] of [["data-post-copy", t("widget.chat.copy")], ["data-post-quote", t("widget.chat.quote")]]) {
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
  open.textContent = t("widget.chat.open_post");
  tools.append(open);
  box.append(author, body, tools);
  item.append(box);
  return item;
}

/// „Ältere Beiträge“: load more through `post.page` before the first shown post without losing the place.
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
          link.textContent = t("widget.chat.older", { count: page.older });
        } else {
          link.remove();
        }
      })
      .catch(() => {
        // Without an answer the link stays: it leads to the whole chat history (/beitraege).
        window.location.href = link.href;
      });
  });
}

// ---- Round 30c: deleting, live display --------------------------------------------------------------------------

/// Interval of the queries (ms).
export const POLL_MS = 3000;

/// Question before deleting.
export function deleteQuestion(id: number, author: string): string {
  return t("widget.chat.delete_question", { id, author });
}

/// ✕ per post: confirmation, RPC `post.delete`, remove the post (once per link).
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
          // Without an answer the link leads to the confirmation page (as without JavaScript).
          window.location.href = link.href;
        });
    });
  }
}

/// Answer of `post.since` or `thread.since`.
export type SinceAnswer = {
  posts: { id: number; author: string; html: string }[];
  typing: { persona: string; since: string }[];
  failures: { persona: string; message: string; at: string }[];
};

/// Line „@lektor schreibt …“ (several with „und“).
export function typingLine(typing: { persona: string }[]): string {
  const names = [...new Set(typing.map((p) => p.persona))];
  if (names.length === 0) return "";
  if (names.length === 1) return t("widget.chat.typing_one", { name: names[0] });
  const list = `${names.slice(0, -1).join(", ")} ${t("widget.chat.and")} ${names[names.length - 1]}`;
  return t("widget.chat.typing_many", { names: list });
}

/// Errors that have not been shown yet (key from time and persona), and the keys afterwards.
export function freshFailures<T extends { persona: string; at: string }>(failures: T[], seen: Set<string>): T[] {
  return failures.filter((f) => {
    const id = `${f.at}|${f.persona}`;
    if (seen.has(id)) return false;
    seen.add(id);
    return true;
  });
}

/// Errors already shown (across the redraw, as long as the page is open).
const SEEN_FAILURES = new Set<string>();

/// Largest id among the posts of the root.
function lastPostId(root: ParentNode): number {
  let max = 0;
  for (const item of root.querySelectorAll<HTMLElement>("[data-post-id]")) {
    const id = Number(item.dataset.postId);
    if (id > max) max = id;
  }
  return max;
}

export type LiveOptions = {
  /// Root of the posts (new ones come before `before` or to the end).
  list: HTMLElement;
  before: Element | null;
  /// Display „schreibt …“ and errors.
  status: HTMLElement | null;
  /// Is the query running right now (accordion open, page visible)?
  active: () => boolean;
  /// Query after `after`.
  fetch: (after: number) => Promise<SinceAnswer>;
  /// After inserting new posts (make buttons operable, scroll).
  inserted: (items: HTMLElement[]) => void;
};

/// Start the live query (see the header); ends by itself when the root disappears from the document.
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
        // HTML of the server (macro of the template, everything escaped); no text from the browser.
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
      // Query failed (server gone, work changed): again next time.
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
          // Forms of the new posts (data-rpc) are served by the workspace through its listener on the root element.
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

// Pages /beitraege and /beitrag: „Kopieren“; /beitraege stands at the newest post on opening (like the anchor).
register("chat-page", (page) => {
  enableCopy(page);
  enableDelete(page);
  if (!window.location.hash && page.querySelector(".posts-list")) {
    page.querySelector<HTMLElement>("#chat-neueste")?.scrollIntoView({ block: "end" });
  }
});
