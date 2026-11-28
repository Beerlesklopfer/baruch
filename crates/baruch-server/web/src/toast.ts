// SPDX-License-Identifier: AGPL-3.0-or-later
// Toast (Round 15, contract §4): short notices, e.g. „Komprimiert: …“, when a persona saw the context only
// in pieces and summarised.
//
// - Widget „toast“: a notice rendered by the server (templates/fragments/toast.html, without JavaScript a
//   box at its place) moves into the stack at the bottom right and gets „Schließen“.
// - Event „baruch:toast“ on document: detail = {message: string, level: "warn" | "info", summarized?:
//   string[]} creates the same notice (sender: assist.ts/chat.ts on the SSE event „compressed“).
// - „warn“ stays until the human closes it (no timeout); „info“ disappears by itself after
//   INFO_MS. Escape closes the youngest notice if the focus lies in it.
// - Texts only through textContent (never as HTML).

import { t } from "./i18n.js";
import { mount, register } from "./widgets.js";

type Level = "warn" | "info";

interface ToastDetail {
  message?: unknown;
  level?: unknown;
  summarized?: unknown;
}

/// A notice of the level „info“ stays this long.
const INFO_MS = 8000;

function stack(): HTMLElement {
  let element = document.querySelector<HTMLElement>(".toast-stack");
  if (!element) {
    element = document.createElement("div");
    element.className = "toast-stack";
    element.setAttribute("aria-live", "polite");
    document.body.append(element);
  }
  return element;
}

function level(value: unknown): Level {
  return value === "info" ? "info" : "warn";
}

/// Build a notice with the same structure as fragments/toast.html (without innerHTML).
function build(message: string, lvl: Level, summarized: string[]): HTMLElement {
  const toast = document.createElement("div");
  toast.className = `toast toast-${lvl}`;
  toast.dataset.widget = "toast";
  toast.dataset.level = lvl;
  toast.setAttribute("role", "status");
  const text = document.createElement("p");
  text.className = "toast-message";
  text.textContent = message;
  toast.append(text);
  if (summarized.length > 0) {
    const details = document.createElement("details");
    details.className = "toast-details";
    const summary = document.createElement("summary");
    summary.textContent = t("widget.toast.summarized", { count: summarized.length });
    const list = document.createElement("ul");
    for (const part of summarized) {
      const item = document.createElement("li");
      item.textContent = part;
      list.append(item);
    }
    details.append(summary, list);
    toast.append(details);
  }
  return toast;
}

register("toast", (element) => {
  const lvl = level(element.dataset.level);
  const close = document.createElement("button");
  close.type = "button";
  close.className = "toast-close";
  close.setAttribute("aria-label", t("widget.toast.close_label"));
  close.title = t("widget.toast.close");
  close.textContent = "×";
  const remove = () => element.remove();
  close.addEventListener("click", remove);
  element.addEventListener("keydown", (event) => {
    if (event.key === "Escape") remove();
  });
  element.prepend(close);
  if (!element.closest(".toast-stack")) stack().append(element);
  if (lvl === "info") window.setTimeout(remove, INFO_MS);
});

document.addEventListener("baruch:toast", (event) => {
  const detail = ((event as CustomEvent<ToastDetail>).detail ?? {}) as ToastDetail;
  if (typeof detail.message !== "string" || detail.message.trim() === "") return;
  const summarized = Array.isArray(detail.summarized)
    ? detail.summarized.filter((part): part is string => typeof part === "string")
    : [];
  const toast = build(detail.message, level(detail.level), summarized);
  stack().append(toast);
  mount(toast);
});
