// SPDX-License-Identifier: AGPL-3.0-or-later
// Toast (Runde 15, Vertrag §4): kurze Hinweise, z. B. „Komprimiert: …“, wenn eine Persona den Kontext nur
// gestückelt und zusammengefasst sah.
//
// - Widget „toast“: ein vom Server gerenderter Hinweis (templates/fragments/toast.html, ohne JavaScript ein
//   Kasten an seiner Stelle) wandert in den Stapel unten rechts und bekommt „Schließen“.
// - Ereignis „baruch:toast“ auf document: detail = {message: string, level: "warn" | "info", summarized?:
//   string[]} legt denselben Hinweis an (Sender: assist.ts/chat.ts beim SSE-Ereignis „compressed“).
// - „warn“ bleibt stehen, bis der Mensch ihn schließt (kein Zeitablauf); „info“ verschwindet nach
//   INFO_MS von selbst. Escape schließt den jüngsten Hinweis, wenn der Fokus in ihm liegt.
// - Texte nur über textContent (nie als HTML).

import { mount, register } from "./widgets.js";

type Level = "warn" | "info";

interface ToastDetail {
  message?: unknown;
  level?: unknown;
  summarized?: unknown;
}

/// So lange bleibt ein Hinweis der Stufe „info“ stehen.
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

/// Hinweis im selben Aufbau wie fragments/toast.html bauen (ohne innerHTML).
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
    summary.textContent = `Nur als Zusammenfassung gesehen (${summarized.length})`;
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
  close.setAttribute("aria-label", "Hinweis schließen");
  close.title = "Schließen";
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
