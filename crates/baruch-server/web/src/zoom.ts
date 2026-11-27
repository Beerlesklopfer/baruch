// SPDX-License-Identifier: AGPL-3.0-or-later
// Ctrl+mouse wheel enlarges only the area under the mouse pointer (wish 2026-11-06), never the whole page.
//
// Areas (zones): every element with `data-zoom-zone="<name>"` – the outline (`tree`), the right
// column (`assistants`) and in it the accordion „Fassungen“ (`revisions`), in the editor each on its own head
// (`head`), toolbar (`toolbar`), sheet (`sheet`: typeset view, text field or preview) and status line
// (`status`) –, also an open popup (<dialog>) and on the other pages the content (<main>). The
// innermost zone under the mouse pointer wins (`closest`). Outside every zone (header, margins)
// Ctrl+mouse wheel is only intercepted so that the page does not zoom as a whole. Ctrl+0 resets the zone under the
// mouse pointer. The levels stand per name in the cookie baruch_zoom („tree_1.2-sheet_1.5“) so that they
// survive reloading. Zooming uses CSS `zoom` on the children of the zone (app.css, `[data-zoomed]`): the
// zone itself keeps its size in the layout. Zones that are created anew on redrawing (ui.render) get
// their level back at once (MutationObserver).
// Round 33: the button „Zoom zurücksetzen“ in the ribbon (tab Ansicht, `[data-zoom-reset]`) resets all zones at
// once and deletes the cookie – Ctrl+0 stays with the one zone under the mouse pointer.

import { store } from "./widgets.js";

/** Zones, the innermost found through `closest`. */
export const ZONES = "dialog[open], [data-zoom-zone], main";
const MIN = 0.5;
const MAX = 3;
const STEP = 1.1;

const levels = new Map<string, number>();

/** Name of the zone in the cookie: only letters and digits (separators are `_` and `-`). */
function zoneKey(zone: Element): string {
  const named = zone instanceof HTMLElement ? zone.dataset.zoomZone : undefined;
  const raw = named || zone.id || (zone.tagName === "MAIN" ? `main${location.pathname.split("/")[1] ?? ""}` : zone.tagName);
  return raw.replace(/[^A-Za-z0-9]/g, "").slice(0, 32) || "zone";
}

function apply(zone: HTMLElement, factor: number): void {
  if (Math.abs(factor - 1) < 0.001) {
    zone.removeAttribute("data-zoomed");
    zone.style.removeProperty("--zoom");
  } else {
    zone.setAttribute("data-zoomed", "");
    zone.style.setProperty("--zoom", factor.toFixed(2));
  }
}

/** Apply remembered levels to all zones in `root` (and `root` itself). */
function restore(root: ParentNode): void {
  const zones: HTMLElement[] = [];
  if (root instanceof HTMLElement && root.matches(`${ZONES}, dialog`)) zones.push(root);
  root.querySelectorAll<HTMLElement>(`${ZONES}, dialog`).forEach((zone) => zones.push(zone));
  for (const zone of zones) {
    const factor = levels.get(zoneKey(zone));
    if (factor) apply(zone, factor);
  }
}

function save(): void {
  const parts = [...levels].filter(([, f]) => Math.abs(f - 1) >= 0.001).map(([k, f]) => `${k}_${f.toFixed(2)}`);
  store.set("zoom", parts.length ? parts.slice(0, 20).join("-") : null);
}

function load(): void {
  for (const part of (store.get("zoom") ?? "").split("-")) {
    const [key, value] = part.split("_");
    const factor = Number(value);
    if (key && Number.isFinite(factor) && factor >= MIN && factor <= MAX) levels.set(key, factor);
  }
  restore(document);
}

function zoneAt(target: EventTarget | null): HTMLElement | null {
  return target instanceof Element ? (target.closest(ZONES) as HTMLElement | null) : null;
}

function change(zone: HTMLElement, factor: number): void {
  const key = zoneKey(zone);
  const next = Math.min(MAX, Math.max(MIN, Math.round(factor * 100) / 100));
  levels.set(key, next);
  apply(zone, next);
  save();
}

let hovered: HTMLElement | null = null;

window.addEventListener(
  "wheel",
  (event) => {
    if (!event.ctrlKey) return;
    // Always intercept: Ctrl+mouse wheel (also two-finger zoom on the touchpad) never zooms the whole page.
    event.preventDefault();
    const zone = zoneAt(event.target);
    if (!zone || event.deltaY === 0) return;
    const current = levels.get(zoneKey(zone)) ?? 1;
    change(zone, event.deltaY < 0 ? current * STEP : current / STEP);
  },
  { passive: false },
);

document.addEventListener("pointerover", (event) => {
  hovered = zoneAt(event.target);
});

document.addEventListener("keydown", (event) => {
  if ((event.ctrlKey || event.metaKey) && event.key === "0" && hovered?.isConnected) {
    event.preventDefault();
    change(hovered, 1);
  }
});

/** All zones to 100 %: forget remembered levels (cookie deleted), reset every zoomed zone of the page. */
function resetAll(): void {
  levels.clear();
  document.querySelectorAll<HTMLElement>("[data-zoomed]").forEach((zone) => apply(zone, 1));
  save();
}

// „Zoom zurücksetzen“ in the ribbon (tab Ansicht): one button for all zones, also without the mouse over a zone.
document.addEventListener("click", (event) => {
  if (!(event.target instanceof Element) || !event.target.closest("[data-zoom-reset]")) return;
  event.preventDefault();
  resetAll();
});

// A popup that opens later gets its remembered level on opening.
document.addEventListener(
  "toggle",
  (event) => {
    const zone = event.target;
    if (zone instanceof HTMLDialogElement && zone.open) {
      const factor = levels.get(zoneKey(zone));
      if (factor) apply(zone, factor);
    }
  },
  true,
);

// Redrawn areas (ui.render replaces the content of the columns) carry new zones: set the level again.
new MutationObserver((records) => {
  for (const record of records) {
    record.addedNodes.forEach((node) => {
      if (node instanceof HTMLElement) restore(node);
    });
  }
}).observe(document.body, { childList: true, subtree: true });

load();
