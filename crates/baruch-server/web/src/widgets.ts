// SPDX-License-Identifier: AGPL-3.0-or-later
// Small widget framework: an element with `data-widget="name"` gets the registered
// behaviour when mounting. The server supplies finished HTML (also usable without JavaScript);
// after every exchange of an area `mount` mounts the widgets in it anew.

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

/// Button for submitting a form – also outside the form through `form="<id>"` (the editor has
/// „Speichern“ in the status line, not in the form).
/// Round 33: the buttons „Speichern“ in the ribbon (docked bar, `[data-edit-toolbar]`) also belong to the form
/// through `form="editor-form"` and would stand before it in the document; authoritative (label
/// „Gespeichert“/„Speichern“) remains the button at the form or in the status line.
export function submitButton(form: HTMLFormElement): HTMLButtonElement | null {
  let docked: HTMLButtonElement | null = null;
  for (const element of Array.from(form.elements)) {
    if (!(element instanceof HTMLButtonElement) || element.type !== "submit") continue;
    if (!element.closest("[data-edit-toolbar]")) return element;
    docked ??= element;
  }
  return docked;
}

/// Round 33: bar of the editor (`[data-edit-toolbar]`, fragments/edit_toolbar.html). It stands in the fragment of the
/// editor (`div.wysiwyg`) until band.ts hooks it into the docking place of the ribbon (`[data-band-dock]`); afterwards
/// `root.querySelector` and `closest("div.wysiwyg")` no longer find it. A page has at most one editor,
/// so the docked bar belongs to it.
export function editToolbar(root: ParentNode | null = null): HTMLElement | null {
  return (
    root?.querySelector<HTMLElement>("[data-edit-toolbar]") ??
    document.querySelector<HTMLElement>("[data-band-dock] [data-edit-toolbar]")
  );
}

/// Unsaved change in the editor? Then ask before everything that redraws the areas
/// (links, change of revision, rearranging the outline, other forms, work selection). `true` = go on.
export function leaveEditor(): boolean {
  const editor = document.querySelector<HTMLElement>("[data-widget=editor]");
  return editor?.dataset.dirty !== "1" || confirm("Ungespeicherte Änderung verwerfen?");
}

/// Round 23a (code review web-1): while a save of the editor is running (reconcile the typeset view, `outline.check`,
/// `outline.save`, redraw), the editor area is locked (`inert`, `aria-busy`). Otherwise characters typed
/// after the form was captured would be lost on redrawing without a confirmation, and a second Ctrl+S
/// would send the same basis once more. Several locks count; the last release lifts the lock and gives
/// the focus back if the element is still there (after an error; after success the area is redrawn).
/// Wish 2026-11-27 („Strg+S als normales Speichern“): on the first lock the editor remembers the place (cursor,
/// selection, scroll position); after the redraw the last release sets it again in the new editor (place.ts).
let editorLocks = 0;
let editorPlace: Place | null = null;

export function editorLocked(): boolean {
  return editorLocks > 0;
}

export function lockEditor(): () => void {
  const pane = document.getElementById("pane-editor");
  const focused = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  // Round 33: the buttons of the editor stand docked in the ribbon – also locked during saving.
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
    // The docking place itself stays during the redraw (only its content changes): always release.
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

/// Settings of the user interface as cookies `baruch_<name>` (one year, only this server). The server
/// reads them and draws theme, column widths, accordion and outline right at once.
/// Values only from [A-Za-z0-9._%-], otherwise nothing is stored.
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
