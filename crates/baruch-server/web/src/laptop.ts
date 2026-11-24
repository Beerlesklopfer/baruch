// SPDX-License-Identifier: AGPL-3.0-or-later
// Widget „laptop“ (Runde 33; Besitz: Aufgabe „laptop“): Laptop-Modus im Browser.
//
// Der Server zeichnet den Zustand aus Cookies und Adresse (src/laptop.rs) als Attribute an <html>; dieses Modul
// hält sie aktuell, ohne die Seite neu zu laden:
// - beim Laden und bei jeder Größenänderung die Fensterbreite messen (`window.innerWidth`), als Klasse `w` (ab
//   COMPACT_BELOW_PX), `m` oder `s` (unter NARROW_BELOW_PX) im Cookie baruch_vw merken – so zeichnet der Server
//   die nächste Seite gleich richtig – und die Attribute `data-laptop`, `data-compact`, `data-narrow`,
//   `data-rail-left`, `data-rail-right` nach derselben Formel wie der Server setzen (`effective`);
// - das Ereignis `baruch:laptop` mit `{compact, narrow}` am `document` senden (einmal beim Laden, danach bei jedem
//   Wechsel) – band.ts klappt danach seine Gruppen; dieses Modul fasst die Gruppen des Bands nie an;
// - Klicks auf `[data-laptop-set]` (Automatisch/An/Aus) und `[data-rail-toggle]` (Seitenleisten im Reiter Ansicht),
//   auf die Leisten (`[data-rail-open]`), „Einklappen“ (`[data-rail-collapse]`) und „Schließen“
//   (`[data-rail-close]`) abfangen: Cookie, Attribute und `aria-pressed` statt eines Seitenwechsels;
// - Schubladen: Im schmalen Zustand (`data-narrow`) öffnet eine Leiste ihre Spalte über dem Text
//   (`data-drawer="left|right"` an <html>, Stil in laptop.css); Esc, ein Klick daneben oder die Wahl eines Abschnitts
//   in der Gliederung schließen sie, der Fokus kehrt zum Auslöser zurück. Im breiten Fenster klappt dieselbe
//   Leiste die Spalte dauerhaft auf (Cookie baruch_lrail bzw. baruch_rrail).
//
// Ohne JavaScript bleibt alles bedienbar: Die Schalter sind Links (`?laptop=on`, `?lrail=1`), die Leisten öffnen
// die Spalte über `:target`. Die Breite kennt der Server dann nicht – „automatisch“ zeichnet breit.
//
// Die Schwellen stehen auch in src/laptop.rs (COMPACT_BELOW_PX, NARROW_BELOW_PX); ein Test hält beide gleich.
// Das Modul hängt sich selbst am `document` ein (es gibt Seiten ohne Arbeitsfläche); das Widget „laptop“ bleibt
// registriert, damit ein Element mit `data-widget="laptop"` dasselbe auslöst.

import { register, store } from "./widgets.js";

/** Unterhalb dieser Fensterbreite ist das Band kompakt (wie src/laptop.rs). */
export const COMPACT_BELOW_PX = 1600;
/** Unterhalb dieser Fensterbreite sind die Seitenleisten eingeklappt (wie src/laptop.rs). */
export const NARROW_BELOW_PX = 1280;

export type Mode = "auto" | "on" | "off";
export type WidthClass = "w" | "m" | "s";
export type Side = "left" | "right";

export interface Effective {
  compact: boolean;
  narrow: boolean;
  railLeft: boolean;
  railRight: boolean;
}

/** Breitenklasse des Fensters (Wert des Cookies baruch_vw). */
export function widthClass(px: number): WidthClass {
  if (px < NARROW_BELOW_PX) return "s";
  if (px < COMPACT_BELOW_PX) return "m";
  return "w";
}

/** Modus aus Cookie oder Attribut; alles Unbekannte ist „automatisch“. */
export function parseMode(value: string | null | undefined): Mode {
  return value === "on" || value === "off" ? value : "auto";
}

/** Wirksamer Zustand – dieselbe Formel wie `LaptopState` in src/laptop.rs. */
export function effective(mode: Mode, width: WidthClass, lrail: boolean, rrail: boolean): Effective {
  const compact = mode === "on" || (mode === "auto" && width !== "w");
  const narrow = mode === "on" || (mode === "auto" && width === "s");
  return { compact, narrow, railLeft: narrow || lrail, railRight: narrow || rrail };
}

/** Attribute an <html>: Name → Wert, `null` heißt „fehlt“ (Reihenfolge wie `html_attrs` im Server). */
export function htmlAttrs(mode: Mode, state: Effective): [string, string | null][] {
  return [
    ["data-laptop", mode],
    ["data-compact", state.compact ? "" : null],
    ["data-narrow", state.narrow ? "" : null],
    ["data-rail-left", state.railLeft ? "" : null],
    ["data-rail-right", state.railRight ? "" : null],
  ];
}

const PANES: Record<Side, string> = { left: "pane-tree", right: "pane-assistants" };
const RAIL_COOKIE: Record<Side, string> = { left: "lrail", right: "rrail" };

let mode: Mode = "auto";
const collapsed: Record<Side, boolean> = { left: false, right: false };
let current: Effective = { compact: false, narrow: false, railLeft: false, railRight: false };
let announced: string | null = null;
let drawer: { side: Side; opener: HTMLElement | null } | null = null;

function side(value: string | undefined): Side | null {
  return value === "left" || value === "right" ? value : null;
}

function railed(which: Side): boolean {
  return which === "left" ? current.railLeft : current.railRight;
}

function pane(which: Side): HTMLElement | null {
  return document.getElementById(PANES[which]);
}

/** Schalter im Band und Leisten an den Zustand angleichen (`aria-pressed`, Adresse des Rückfall-Links). */
function syncControls(): void {
  document.querySelectorAll<HTMLElement>("[data-laptop-set]").forEach((control) => {
    control.setAttribute("aria-pressed", String(control.dataset.laptopSet === mode));
  });
  document.querySelectorAll<HTMLElement>("[data-rail-toggle]").forEach((control) => {
    const which = side(control.dataset.railToggle);
    if (!which) return;
    const visible = !railed(which) || drawer?.side === which;
    control.setAttribute("aria-pressed", String(visible));
    // Der Link bleibt für „in neuem Tab öffnen“ richtig: sichtbar → einklappen (=1), eingeklappt → zeigen (=0).
    const key = RAIL_COOKIE[which];
    const href = control.getAttribute("href");
    if (href) control.setAttribute("href", href.replace(new RegExp(`${key}=[01]`), `${key}=${railed(which) ? 0 : 1}`));
  });
  document.querySelectorAll<HTMLElement>("[data-rail-open]").forEach((rail) => {
    const which = side(rail.dataset.railOpen);
    if (which) rail.setAttribute("aria-expanded", String(drawer?.side === which));
  });
}

/** Breite messen, Zustand berechnen, Attribute setzen, Wechsel melden. */
function refresh(): void {
  const root = document.documentElement;
  const width = widthClass(window.innerWidth);
  if (store.get("vw") !== width) store.set("vw", width);
  current = effective(mode, width, collapsed.left, collapsed.right);
  for (const [name, value] of htmlAttrs(mode, current)) {
    if (value === null) root.removeAttribute(name);
    else if (root.getAttribute(name) !== value) root.setAttribute(name, value);
  }
  // Eine Schublade gibt es nur über einer eingeklappten Spalte.
  if (drawer && !railed(drawer.side)) closeDrawer(false);
  syncControls();
  const signature = `${current.compact}-${current.narrow}`;
  if (signature !== announced) {
    announced = signature;
    document.dispatchEvent(
      new CustomEvent("baruch:laptop", { detail: { compact: current.compact, narrow: current.narrow } }),
    );
  }
}

function openDrawer(which: Side, opener: HTMLElement | null): void {
  const target = pane(which);
  if (!target) return;
  if (drawer) closeDrawer(false);
  drawer = { side: which, opener };
  document.documentElement.dataset.drawer = which;
  syncControls();
  // Die Spalte selbst nimmt den Fokus (Tab führt dann in ihren Inhalt), ohne in der Tab-Reihenfolge zu stehen.
  target.tabIndex = -1;
  target.focus({ preventScroll: true });
}

function closeDrawer(returnFocus: boolean): void {
  if (!drawer) return;
  const { side: which, opener } = drawer;
  drawer = null;
  delete document.documentElement.dataset.drawer;
  syncControls();
  if (!returnFocus) return;
  const rail = document.querySelector<HTMLElement>(`[data-rail-open="${which}"]`);
  // Zurück zum Auslöser; steht er nicht mehr sichtbar da (Popup des Bands zu), zur Leiste der Spalte.
  const back = opener?.isConnected && opener.getClientRects().length > 0 ? opener : rail;
  back?.focus({ preventScroll: true });
}

/** Spalte dauerhaft ein- bzw. ausklappen (Cookie; „fehlt“ heißt sichtbar). */
function setCollapsed(which: Side, value: boolean): void {
  collapsed[which] = value;
  store.set(RAIL_COOKIE[which], value ? "1" : null);
  refresh();
}

function setMode(next: Mode): void {
  mode = next;
  store.set("laptop", next === "auto" ? null : next);
  refresh();
}

function plainClick(event: MouseEvent): boolean {
  return event.button === 0 && !event.ctrlKey && !event.metaKey && !event.shiftKey && !event.altKey;
}

function onClick(event: MouseEvent): void {
  // Kein Blick auf `defaultPrevented`: Die Arbeitsfläche fängt Links der Gliederung selbst ab (workspace.ts,
  // `a[data-nav]`), und gerade dann soll die Schublade zugehen.
  if (!plainClick(event) || !(event.target instanceof Element)) return;
  const target = event.target;

  const modeControl = target.closest<HTMLElement>("[data-laptop-set]");
  if (modeControl) {
    const value = modeControl.dataset.laptopSet;
    if (value !== "auto" && value !== "on" && value !== "off") return;
    event.preventDefault();
    setMode(value);
    return;
  }

  const closer = target.closest<HTMLElement>("[data-rail-close]");
  if (closer) {
    event.preventDefault();
    closeDrawer(!closer.classList.contains("rail-scrim"));
    return;
  }

  const control = target.closest<HTMLElement>("[data-rail-toggle], [data-rail-open], [data-rail-collapse]");
  if (control) {
    const which = side(control.dataset.railToggle ?? control.dataset.railOpen ?? control.dataset.railCollapse);
    // Ohne Spalte auf dieser Seite bleibt der Link ein gewöhnlicher Link (der Server merkt das Cookie).
    if (!which || !pane(which)) return;
    event.preventDefault();
    if (control.dataset.railCollapse !== undefined) {
      setCollapsed(which, true);
      document.querySelector<HTMLElement>(`[data-rail-open="${which}"]`)?.focus({ preventScroll: true });
    } else if (current.narrow) {
      // Schmal: kein Platz für die Spalte neben dem Text – als Schublade darüber, bis sie geschlossen wird.
      if (drawer?.side === which) closeDrawer(true);
      else openDrawer(which, control);
    } else if (control.dataset.railOpen !== undefined) {
      setCollapsed(which, false);
      pane(which)?.focus({ preventScroll: true });
    } else {
      setCollapsed(which, !collapsed[which]);
    }
    return;
  }

  if (!drawer) return;
  const open = pane(drawer.side);
  if (open?.contains(target)) {
    // Ein Abschnitt in der Gliederung ist gewählt: Die Schublade hat ihren Dienst getan, der Text ist wieder frei.
    if (drawer.side === "left" && target.closest("a.tree-entry")) closeDrawer(false);
    return;
  }
  // Klick daneben schließt; der Fokus bleibt, wohin der Mensch geklickt hat. Popups (Dialoge, Menüs der
  // Gliederung) liegen außerhalb der Spalte und gehören trotzdem zu ihr.
  if (target.closest("dialog, [role=menu], .toast")) return;
  closeDrawer(false);
}

function onKeydown(event: KeyboardEvent): void {
  if (event.key !== "Escape" || !drawer || event.defaultPrevented) return;
  // Esc in einem Popup oder Eingabefeld über der Schublade gehört zuerst diesem.
  if (event.target instanceof Element && event.target.closest("dialog[open], [role=menu]")) return;
  event.preventDefault();
  closeDrawer(true);
}

let started = false;

function start(): void {
  if (started) return;
  started = true;
  const root = document.documentElement;
  // Die Cookies sind die gemerkte Wahl; sind sie nicht lesbar (Cookies gesperrt), gilt, was der Server gezeichnet hat.
  const saved = store.get("laptop");
  mode = parseMode(saved ?? (navigator.cookieEnabled ? null : root.dataset.laptop));
  collapsed.left = store.get("lrail") === "1";
  collapsed.right = store.get("rrail") === "1";
  refresh();
  // Eine Adresse mit #pane-tree (ohne JavaScript geöffnet, dann neu geladen) wird zur Schublade dieses Moduls.
  const hash = window.location.hash.slice(1);
  for (const which of ["left", "right"] as const) {
    if (hash === PANES[which] && railed(which)) {
      history.replaceState(history.state, "", window.location.pathname + window.location.search);
      openDrawer(which, null);
    }
  }
  let pending = 0;
  window.addEventListener("resize", () => {
    if (pending) return;
    pending = requestAnimationFrame(() => {
      pending = 0;
      refresh();
    });
  });
  document.addEventListener("click", onClick);
  document.addEventListener("keydown", onKeydown);
  // Nach einem Wechsel in den Einstellungen (Popup) stimmen Schalter und Attribute wieder.
  document.addEventListener("baruch:settings-changed", refresh);
}

register("laptop", start);
if (typeof document !== "undefined") start();
