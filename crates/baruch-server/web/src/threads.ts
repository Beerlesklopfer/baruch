// SPDX-License-Identifier: AGPL-3.0-or-later
// Widget „threads“ (Runde 15, Fäden ohne Abschnitt). Ohne JavaScript gehen die Seiten /faeden und /faden mit dem
// Formular an POST /faden/act. Mit JavaScript: der Faden rollt zum letzten Beitrag, Strg+Enter im Antwortfeld
// schickt das Formular ab, und während die Personas antworten, sind die Knöpfe gesperrt und sagen das.
// Runde 28 (chat-auftrag): Die Beiträge rollen in `.thread-scroll`; beim Öffnen steht er am neuesten Beitrag
// (`#chat-neueste`, ohne JavaScript über den Anker der Weiterleitung). „Kopieren“ je Beitrag (`enableCopy` aus chat.ts);
// „Zitieren“ ist ein Link (`?zitat=<id>#antwort`), der auch ohne JavaScript die Antwort mit „> …“ vorbelegt.
// Runde 30c (chat-bedienung):
// - ✕ (`summary[data-thread-delete]`, ohne JavaScript klappt es ein Formular mit Kästchen auf): Rückfrage mit `confirm`,
//   RPC `thread.post_delete {confirm: true}`, der Beitrag verschwindet; scheitert der Aufruf, bleibt das Formular.
// - ↻ (`form[data-thread-rpc]`, ohne JavaScript POST /faden/act): RPC `thread.repeat`, danach sofort eine Abfrage.
// - Live: `thread.since` alle 3 s bei sichtbarer Seite (`startLive` aus chat.ts) – neue Beiträge, „@x schreibt …“,
//   Fehler in `div[data-chat-live]`.

import { call } from "./rpc.js";
import { deleteQuestion, enableCopy, startLive, type SinceAnswer } from "./chat.js";
import { register } from "./widgets.js";

/// ✕ und ↻ der Fadenbeiträge bedienbar machen (einmal je Element).
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
          // Rückfall: das Formular mit Kästchen (wie ohne JavaScript).
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
