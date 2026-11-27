// SPDX-License-Identifier: AGPL-3.0-or-later
// Widget „threads“ (Round 15, threads without a section). Without JavaScript the pages /faeden and /faden work with
// the form to POST /faden/act. With JavaScript: the thread scrolls to the last post, Ctrl+Enter in the answer field
// submits the form, and while the personas answer, the buttons are disabled and say so.
// Round 28 (chat-auftrag): the posts scroll in `.thread-scroll`; on opening it stands at the newest post
// (`#chat-neueste`, without JavaScript through the anchor of the redirect). „Kopieren“ per post (`enableCopy` from
// chat.ts); „Zitieren“ is a link (`?zitat=<id>#antwort`) that prefills the answer with „> …“ also without JavaScript.
// Round 30c (chat-bedienung):
// - ✕ (`summary[data-thread-delete]`, without JavaScript it unfolds a form with a checkbox): confirmation with
//   `confirm`, RPC `thread.post_delete {confirm: true}`, the post disappears; if the call fails, the form stays.
// - ↻ (`form[data-thread-rpc]`, without JavaScript POST /faden/act): RPC `thread.repeat`, then a query at once.
// - Live: `thread.since` every 3 s while the page is visible (`startLive` from chat.ts) – new posts, „@x schreibt …“,
//   errors in `div[data-chat-live]`.

import { call } from "./rpc.js";
import { deleteQuestion, enableCopy, startLive, type SinceAnswer } from "./chat.js";
import { register } from "./widgets.js";

/// Make ✕ and ↻ of the thread posts operable (once per element).
function enableThreadTools(root: ParentNode, work: number, thread: number, status: HTMLElement | null): void {
  for (const summary of root.querySelectorAll<HTMLElement>("summary[data-thread-delete]")) {
    if (summary.dataset.deleteReady) continue;
    summary.dataset.deleteReady = "1";
    summary.addEventListener("click", (event) => {
      const item = summary.closest<HTMLElement>(".post-item");
      const id = Number(item?.dataset.postId);
      if (!item || !id) return;
      event.preventDefault();
      if (!window.confirm(deleteQuestion(id, item.dataset.postAuthor ?? ""))) return;
      void call("thread.post_delete", { work, thread, id, confirm: true })
        .then(() => item.remove())
        .catch(() => {
          // Fallback: the form with the checkbox (as without JavaScript).
          summary.closest("details")?.setAttribute("open", "");
        });
    });
  }
  for (const form of root.querySelectorAll<HTMLFormElement>("form[data-thread-rpc]")) {
    if (form.dataset.rpcReady) continue;
    form.dataset.rpcReady = "1";
    form.addEventListener("submit", (event) => {
      event.preventDefault();
      const id = Number(new FormData(form).get("id"));
      const button = form.querySelector<HTMLButtonElement>("button");
      if (button) button.disabled = true;
      document.dispatchEvent(new CustomEvent("baruch:chat-refresh"));
      void call("thread.repeat", { work, thread, id })
        .catch((error: unknown) => {
          if (status) {
            const div = document.createElement("div");
            div.className = "chat-live-error";
            div.textContent = String((error as Error).message ?? error);
            status.replaceChildren(div);
            status.hidden = false;
          }
        })
        .finally(() => {
          if (button) button.disabled = false;
          document.dispatchEvent(new CustomEvent("baruch:chat-refresh"));
        });
    });
  }
}

register("threads", (root) => {
  enableCopy(root);
  const scroll = root.querySelector<HTMLElement>(".thread-scroll");
  if (scroll && window.location.hash !== "#antwort" && !window.location.hash.startsWith("#beitrag-")) {
    scroll.scrollTop = scroll.scrollHeight;
  } else if (!scroll) {
    const last = root.querySelector<HTMLElement>(".thread-post:last-of-type");
    last?.scrollIntoView({ block: "nearest" });
  }
  for (const form of root.querySelectorAll<HTMLFormElement>("form.thread-form")) {
    const area = form.querySelector<HTMLTextAreaElement>("textarea[name=message]");
    area?.addEventListener("keydown", (event) => {
      if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
        event.preventDefault();
        form.requestSubmit();
      }
    });
    form.addEventListener("submit", () => {
      if (!area) return;
      for (const button of form.querySelectorAll<HTMLButtonElement>("button")) {
        button.disabled = true;
        button.textContent = "Personas antworten …";
      }
    });
  }
  const work = Number(root.dataset.work);
  const thread = Number(root.dataset.thread);
  const list = root.querySelector<HTMLElement>("ol.thread-posts");
  if (!work || !thread || !list) return;
  const status = root.querySelector<HTMLElement>("[data-chat-live]");
  enableThreadTools(root, work, thread, status);
  startLive({
    list,
    before: null,
    status,
    active: () => true,
    fetch: (after) => call<SinceAnswer>("thread.since", { work, thread, after }),
    inserted: (items) => {
      enableCopy(list);
      enableThreadTools(list, work, thread, status);
      for (const item of items) item.classList.add("post-new");
      if (scroll) scroll.scrollTop = scroll.scrollHeight;
    },
  });
});
