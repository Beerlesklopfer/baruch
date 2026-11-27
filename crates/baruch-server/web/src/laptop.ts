// SPDX-License-Identifier: AGPL-3.0-or-later
// Widget „laptop“ (Round 33; ownership: task „laptop“): laptop mode in the browser.
//
// The server draws the state from cookies and address (src/laptop.rs) as attributes on <html>; this module
// keeps them current without reloading the page:
// - on loading and on every resize measure the window width (`window.innerWidth`), remember it as class `w` (from
//   COMPACT_BELOW_PX), `m` or `s` (below NARROW_BELOW_PX) in the cookie baruch_vw – this way the server draws
//   the next page right at once – and set the attributes `data-laptop`, `data-compact`, `data-narrow`,
//   `data-rail-left`, `data-rail-right` by the same formula as the server (`effective`);
// - send the event `baruch:laptop` with `{compact, narrow}` on the `document` (once on loading, afterwards on every
//   change) – band.ts then folds its groups; this module never touches the groups of the ribbon;
// - intercept clicks on `[data-laptop-set]` (Automatisch/An/Aus) and `[data-rail-toggle]` (sidebars in the tab
//   Ansicht), on the rails (`[data-rail-open]`), „Einklappen“ (`[data-rail-collapse]`) and „Schließen“
//   (`[data-rail-close]`): cookie, attributes and `aria-pressed` instead of a page change;
// - drawers: in the narrow state (`data-narrow`) a rail opens its column above the text
//   (`data-drawer="left|right"` on <html>, style in laptop.css); Esc, a click next to it or choosing a section
//   in the outline close it, the focus returns to the trigger. In the wide window the same
//   rail unfolds the column permanently (cookie baruch_lrail or baruch_rrail).
//
// Without JavaScript everything stays usable: the switches are links (`?laptop=on`, `?lrail=1`), the rails open
// the column through `:target`. The server then does not know the width – „automatisch“ draws wide.
//
// The thresholds also stand in src/laptop.rs (COMPACT_BELOW_PX, NARROW_BELOW_PX); a test keeps both equal.
// The module attaches itself to the `document` (there are pages without a workspace); the widget „laptop“ stays
// registered so that an element with `data-widget="laptop"` triggers the same.

import { register, store } from "./widgets.js";

/** Below this window width the ribbon is compact (as in src/laptop.rs). */
export const COMPACT_BELOW_PX = 1600;
/** Below this window width the sidebars are collapsed (as in src/laptop.rs). */
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

/** Width class of the window (value of the cookie baruch_vw). */
export function widthClass(px: number): WidthClass {
  if (px < NARROW_BELOW_PX) return "s";
  if (px < COMPACT_BELOW_PX) return "m";
  return "w";
}

/** Mode from cookie or attribute; everything unknown is „automatisch“. */
export function parseMode(value: string | null | undefined): Mode {
  return value === "on" || value === "off" ? value : "auto";
}

/** Effective state – the same formula as `LaptopState` in src/laptop.rs. */
export function effective(mode: Mode, width: WidthClass, lrail: boolean, rrail: boolean): Effective {
  const compact = mode === "on" || (mode === "auto" && width !== "w");
  const narrow = mode === "on" || (mode === "auto" && width === "s");
  return { compact, narrow, railLeft: narrow || lrail, railRight: narrow || rrail };
}

/** Attributes on <html>: name → value, `null` means "missing" (order as `html_attrs` in the server). */
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

/** Align switches in the ribbon and rails with the state (`aria-pressed`, address of the fallback link). */
function syncControls(): void {
  document.querySelectorAll<HTMLElement>("[data-laptop-set]").forEach((control) => {
    control.setAttribute("aria-pressed", String(control.dataset.laptopSet === mode));
  });
  document.querySelectorAll<HTMLElement>("[data-rail-toggle]").forEach((control) => {
    const which = side(control.dataset.railToggle);
    if (!which) return;
    const visible = !railed(which) || drawer?.side === which;
    control.setAttribute("aria-pressed", String(visible));
    // The link stays right for "open in a new tab": visible → collapse (=1), collapsed → show (=0).
    const key = RAIL_COOKIE[which];
    const href = control.getAttribute("href");
    if (href) control.setAttribute("href", href.replace(new RegExp(`${key}=[01]`), `${key}=${railed(which) ? 0 : 1}`));
  });
  document.querySelectorAll<HTMLElement>("[data-rail-open]").forEach((rail) => {
    const which = side(rail.dataset.railOpen);
    if (which) rail.setAttribute("aria-expanded", String(drawer?.side === which));
  });
}

/** Measure the width, compute the state, set the attributes, report a change. */
function refresh(): void {
  const root = document.documentElement;
  const width = widthClass(window.innerWidth);
  if (store.get("vw") !== width) store.set("vw", width);
  current = effective(mode, width, collapsed.left, collapsed.right);
  for (const [name, value] of htmlAttrs(mode, current)) {
    if (value === null) root.removeAttribute(name);
    else if (root.getAttribute(name) !== value) root.setAttribute(name, value);
  }
  // A drawer exists only above a collapsed column.
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
  // The column itself takes the focus (Tab then leads into its content) without being in the tab order.
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
  // Back to the trigger; if it is no longer visible (popup of the ribbon closed), to the rail of the column.
  const back = opener?.isConnected && opener.getClientRects().length > 0 ? opener : rail;
  back?.focus({ preventScroll: true });
}

/** Collapse or unfold the column permanently (cookie; "missing" means visible). */
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
  // No look at `defaultPrevented`: the workspace intercepts links of the outline itself (workspace.ts,
  // `a[data-nav]`), and exactly then the drawer is to close.
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
    // Without a column on this page the link stays an ordinary link (the server remembers the cookie).
    if (!which || !pane(which)) return;
    event.preventDefault();
    if (control.dataset.railCollapse !== undefined) {
      setCollapsed(which, true);
      document.querySelector<HTMLElement>(`[data-rail-open="${which}"]`)?.focus({ preventScroll: true });
    } else if (current.narrow) {
      // Narrow: no room for the column next to the text – as a drawer above it until it is closed.
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
    // A section in the outline is chosen: the drawer has done its service, the text is free again.
    if (drawer.side === "left" && target.closest("a.tree-entry")) closeDrawer(false);
    return;
  }
  // A click next to it closes; the focus stays where the human clicked. Popups (dialogs, menus of the
  // outline) lie outside the column and still belong to it.
  if (target.closest("dialog, [role=menu], .toast")) return;
  closeDrawer(false);
}

function onKeydown(event: KeyboardEvent): void {
  if (event.key !== "Escape" || !drawer || event.defaultPrevented) return;
  // Esc in a popup or input field above the drawer belongs to that first.
  if (event.target instanceof Element && event.target.closest("dialog[open], [role=menu]")) return;
  event.preventDefault();
  closeDrawer(true);
}

let started = false;

function start(): void {
  if (started) return;
  started = true;
  const root = document.documentElement;
  // The cookies are the remembered choice; if they cannot be read (cookies blocked), what the server drew applies.
  const saved = store.get("laptop");
  mode = parseMode(saved ?? (navigator.cookieEnabled ? null : root.dataset.laptop));
  collapsed.left = store.get("lrail") === "1";
  collapsed.right = store.get("rrail") === "1";
  refresh();
  // An address with #pane-tree (opened without JavaScript, then reloaded) becomes the drawer of this module.
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
  // After a change in the settings (popup) switches and attributes are right again.
  document.addEventListener("baruch:settings-changed", refresh);
}

register("laptop", start);
if (typeof document !== "undefined") start();
