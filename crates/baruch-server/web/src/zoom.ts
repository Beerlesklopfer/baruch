// SPDX-License-Identifier: AGPL-3.0-or-later
// Strg+Mausrad vergrößert nur den Bereich unter dem Mauszeiger (Wunsch 2026-11-06), nie die ganze Seite.
//
// Bereiche (Zonen): jedes Element mit `data-zoom-zone="<name>"` – die Gliederung (`tree`), die rechte
// Spalte (`assistants`) und darin das Akkordeon „Fassungen“ (`revisions`), im Editor je für sich Kopf
// (`head`), Werkzeugleiste (`toolbar`), Blatt (`sheet`: Satzbild, Textfeld oder Vorschau) und Statuszeile
// (`status`) –, außerdem ein offenes Popup (<dialog>) und auf den übrigen Seiten der Inhalt (<main>). Die
// innerste Zone unter dem Mauszeiger gewinnt (`closest`). Außerhalb jeder Zone (Kopfzeile, Ränder) wird
// Strg+Mausrad nur abgefangen, damit die Seite nicht als Ganzes zoomt. Strg+0 setzt die Zone unter dem
// Mauszeiger zurück. Die Stufen stehen je Name im Cookie baruch_zoom („tree_1.2-sheet_1.5“), damit sie das
// Neuladen überstehen. Gezoomt wird mit CSS `zoom` auf den Kindern der Zone (app.css, `[data-zoomed]`): Die
// Zone selbst behält ihre Größe im Layout. Zonen, die beim Neuzeichnen (ui.render) neu entstehen, bekommen
// ihre Stufe sofort wieder (MutationObserver).

import { store } from "./widgets.js";

/** Zonen, die innerste gefunden über `closest`. */
export const ZONES = "dialog[open], [data-zoom-zone], main";
const MIN = 0.5;
const MAX = 3;
const STEP = 1.1;

const levels = new Map<string, number>();

/** Name der Zone im Cookie: nur Buchstaben und Ziffern (Trennzeichen sind `_` und `-`). */
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

/** Gemerkte Stufen auf alle Zonen in `root` (und `root` selbst) anwenden. */
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
    // Immer abfangen: Strg+Mausrad (auch Zwei-Finger-Zoom am Touchpad) zoomt nie die ganze Seite.
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

// Ein Popup, das später aufgeht, bekommt seine gemerkte Stufe beim Öffnen.
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

// Neu gezeichnete Bereiche (ui.render ersetzt den Inhalt der Spalten) tragen neue Zonen: Stufe wieder setzen.
new MutationObserver((records) => {
  for (const record of records) {
    record.addedNodes.forEach((node) => {
      if (node instanceof HTMLElement) restore(node);
    });
  }
}).observe(document.body, { childList: true, subtree: true });

load();
