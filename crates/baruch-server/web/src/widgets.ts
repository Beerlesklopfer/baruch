// SPDX-License-Identifier: AGPL-3.0-or-later
// Kleiner Widget-Rahmen: Ein Element mit `data-widget="name"` bekommt beim Einhängen das
// registrierte Verhalten. Der Server liefert fertiges HTML (auch ohne JavaScript bedienbar);
// nach jedem Austausch eines Bereichs hängt `mount` die Widgets darin neu ein.

import { capturePlace, restorePlace, type Place } from "./place.js";

export type Mount = (element: HTMLElement) => void;

const registry = new Map<string, Mount>();
const mounted = new WeakSet<HTMLElement>();

export function register(name: string, mount: Mount): void {
  registry.set(name, mount);
}

export function mount(root: ParentNode = document): void {
  const elements: HTMLElement[] = [];
  if (root instanceof HTMLElement && root.dataset.widget) elements.push(root);
  root.querySelectorAll<HTMLElement>("[data-widget]").forEach((e) => elements.push(e));
  for (const element of elements) {
    if (mounted.has(element)) continue;
    const behaviour = registry.get(element.dataset.widget ?? "");
    if (!behaviour) continue;
    mounted.add(element);
    try {
      behaviour(element);
    } catch (error) {
      console.error(`Widget ${element.dataset.widget}:`, error);
    }
  }
}

/// Knopf zum Absenden eines Formulars – auch außerhalb des Formulars über `form="<id>"` (der Editor hat
/// „Speichern“ in der Statuszeile, nicht im Formular).
/// Runde 33: Die Knöpfe „Speichern“ im Menüband (angedockte Leiste, `[data-edit-toolbar]`) gehören über
/// `form="editor-form"` auch zum Formular und stünden im Dokument davor; maßgeblich (Beschriftung
/// „Gespeichert“/„Speichern“) bleibt der Knopf am Formular bzw. in der Statuszeile.
export function submitButton(form: HTMLFormElement): HTMLButtonElement | null {
  let docked: HTMLButtonElement | null = null;
  for (const element of Array.from(form.elements)) {
    if (!(element instanceof HTMLButtonElement) || element.type !== "submit") continue;
    if (!element.closest("[data-edit-toolbar]")) return element;
    docked ??= element;
  }
  return docked;
}

/// Runde 33: Leiste des Editors (`[data-edit-toolbar]`, fragments/edit_toolbar.html). Sie steht im Fragment des
/// Editors (`div.wysiwyg`), bis band.ts sie in den Andockplatz des Menübands hängt (`[data-band-dock]`); danach
/// finden `root.querySelector` und `closest("div.wysiwyg")` sie nicht mehr. Eine Seite hat höchstens einen Editor,
/// die angedockte Leiste gehört also zu ihm.
export function editToolbar(root: ParentNode | null = null): HTMLElement | null {
  return (
    root?.querySelector<HTMLElement>("[data-edit-toolbar]") ??
    document.querySelector<HTMLElement>("[data-band-dock] [data-edit-toolbar]")
  );
}

/// Ungespeicherte Änderung im Editor? Dann vor allem fragen, was die Bereiche neu zeichnet
/// (Links, Fassungswechsel, Gliederung umstellen, andere Formulare, Werkwahl). `true` = weiter.
export function leaveEditor(): boolean {
  const editor = document.querySelector<HTMLElement>("[data-widget=editor]");
  return editor?.dataset.dirty !== "1" || confirm("Ungespeicherte Änderung verwerfen?");
}

/// Runde 23a (Codereview web-1): Während ein Speichern des Editors läuft (Satzbild abgleichen, `outline.check`,
/// `outline.save`, Neuzeichnen), ist der Editor-Bereich gesperrt (`inert`, `aria-busy`). Sonst gingen Zeichen, die
/// nach dem Abgreifen des Formulars getippt werden, beim Neuzeichnen ohne Rückfrage verloren, und ein zweites Strg+S
/// schickte dieselbe Grundlage noch einmal. Mehrere Sperren zählen; die letzte Freigabe hebt die Sperre auf und gibt
/// den Fokus zurück, wenn das Element noch da ist (nach einem Fehler; nach Erfolg ist der Bereich neu gezeichnet).
/// Wunsch 2026-11-27 („Strg+S als normales Speichern“): Beim ersten Sperren merkt sich der Editor die Stelle (Cursor,
/// Auswahl, Scrollstand); nach dem Neuzeichnen setzt die letzte Freigabe sie im neuen Editor wieder (place.ts).
let editorLocks = 0;
let editorPlace: Place | null = null;

export function editorLocked(): boolean {
  return editorLocks > 0;
}

export function lockEditor(): () => void {
  const pane = document.getElementById("pane-editor");
  const focused = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  // Runde 33: Die Knöpfe des Editors stehen angedockt im Menüband – während des Speicherns ebenfalls gesperrt.
  const dock = document.querySelector<HTMLElement>("[data-band-dock]");
  if (editorLocks === 0) editorPlace = capturePlace();
  editorLocks += 1;
  if (pane) {
    pane.inert = true;
    pane.setAttribute("aria-busy", "true");
  }
  if (dock) dock.inert = true;
  let released = false;
  return () => {
    if (released) return;
    released = true;
    editorLocks -= 1;
    if (editorLocks > 0) return;
    // Der Andockplatz selbst bleibt beim Neuzeichnen stehen (nur sein Inhalt wechselt): immer freigeben.
    const docks = document.querySelectorAll<HTMLElement>("[data-band-dock]");
    docks.forEach((d) => (d.inert = false));
    if (!pane) return;
    pane.inert = false;
    pane.removeAttribute("aria-busy");
    const place = editorPlace;
    editorPlace = null;
    if (focused?.isConnected && pane.contains(focused)) focused.focus({ preventScroll: true });
    else if (place) restorePlace(place);
  };
}

/// Einstellungen der Oberfläche als Cookies `baruch_<name>` (ein Jahr, nur dieser Server). Der Server
/// liest sie und zeichnet Thema, Spaltenbreiten, Akkordeon und Gliederung gleich richtig.
/// Werte nur aus [A-Za-z0-9._%-], sonst wird nichts gespeichert.
export const store = {
  get(name: string): string | null {
    const prefix = `baruch_${name}=`;
    for (const part of document.cookie.split(";")) {
      const pair = part.trim();
      if (pair.startsWith(prefix)) return pair.slice(prefix.length);
    }
    return null;
  },
  set(name: string, value: string | null): void {
    if (value !== null && !/^[A-Za-z0-9._%-]*$/.test(value)) return;
    const age = value === null ? 0 : 31536000;
    document.cookie = `baruch_${name}=${value ?? ""}; Path=/; Max-Age=${age}; SameSite=Lax`;
  },
};
