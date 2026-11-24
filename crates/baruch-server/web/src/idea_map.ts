// SPDX-License-Identifier: AGPL-3.0-or-later
// Widget der Gedankenkarte (Runde 25, Vertrag §4.4/§4.5, `data-widget="idea-map"` auf `.im-stage`). Ohne
// JavaScript bleibt die Karte über Links und Formulare bedienbar; das Widget liest nur den DOM-Vertrag, den der
// Server erzeugt (karte-bild), und ruft nur die RPC `idea.get`, `idea.create`, `idea.update`, `idea.link`,
// `idea.map_pos_set` und `idea.map_pos_reset`.
//
// - Zoom: Mausrad ohne Strg (um den Mauszeiger), Knöpfe + / − / Einpassen in der Werkzeugleiste, Tasten + − 0.
//   Strg+Mausrad bleibt bei zoom.ts. Verschieben: Ziehen auf leerer Fläche. Beides setzt nur `transform` auf
//   `[data-im-viewport]`; der Ausschnitt überlebt das Neuladen in sessionStorage (nur Bequemlichkeit, try/catch).
// - Einfall ziehen: Ablage auf Teil/Kapitel/Abschnitt (Abstand ≤ `data-r`) → `idea.update {id, key}`, auf „Ohne
//   Kapitel“ → `idea.update {id, key: null}`, je mit Neuladen; auf einen Einfall → Dialog mit Art und Notiz →
//   `idea.link`; sonst → `idea.map_pos_set {work, idea, x, y}` (Koordinaten über `getScreenCTM()` des Viewports),
//   die Kanten wandern beim Ziehen mit, danach `im-pinned`. „Anordnung zurücksetzen“ → `idea.map_pos_reset {work}`.
// - Tastatur: Pfeile wandern zum nächsten Knoten in der Richtung (wandernder tabindex), Enter öffnet die
//   Seitenleiste eines Einfalls (`idea.get`, speichern mit `idea.update`), V verbindet vom fokussierten Einfall
//   (Ziel mit Pfeilen, Enter, Escape bricht ab), N schaltet die Nähe (Klasse `im-hide-near` und `near` in der
//   Adresse per `history.replaceState`), Alt+E legt einen Einfall mit dem Bezug des fokussierten Knotens an.
//   Ein Klick auf einen Einfall öffnet ebenfalls die Seitenleiste (mit Strg/Umschalt/Mitteltaste der Link).
// - Texte nur über textContent (nie als HTML); Meldungen in einer Live-Region (`.im-status`).

import { call } from "./rpc.js";
import { register } from "./widgets.js";

const MIN_SCALE = 0.2;
/// Rand für Beschriftungen beim Einpassen (viewBox-Einheiten): seitlich bzw. unter dem untersten Kreis.
const LABEL_SIDE = 40;
const LABEL_BELOW = 30;
const MAX_SCALE = 6;
const STEP = 1.2;
/// Ab so vielen Bildschirmpunkten wird aus einem Klick ein Ziehen.
const DRAG_THRESHOLD = 4;
/// Grenze der gespeicherten Koordinaten (wie MAP_COORD_LIMIT im Store).
const COORD_LIMIT = 1_000_000;
const MAX_NOTE = 500;

const KINDS: ReadonlyArray<readonly [string, string]> = [
  ["leads_to", "führt weiter"],
  ["belongs_to", "gehört zu"],
  ["example_of", "Beispiel für"],
  ["contradicts", "widerspricht"],
];

interface IdeaJson {
  id: number;
  words: string[];
  note: string;
  quote: string;
  source: string;
  bible: string;
  status: string;
  node_title?: string | null;
}

interface View {
  s: number;
  tx: number;
  ty: number;
}

type Direction = "left" | "right" | "up" | "down";

function num(value: string | undefined, fallback = 0): number {
  const n = Number(value);
  return value === undefined || value === "" || !Number.isFinite(n) ? fallback : n;
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

/// Kennung eines Knotens (`data-node`): "w", "n:<key>", "none", "i:<id>".
function nodeId(el: Element): string {
  return (el as SVGElement).dataset.node ?? "";
}

function isIdea(el: Element | null): boolean {
  return !!el && el.classList.contains("im-idea");
}

function isOutlineNode(el: Element): boolean {
  return ["im-part", "im-chapter", "im-section"].some((c) => el.classList.contains(c));
}

/// Kurzer Name eines Knotens für Meldungen: aria-label, sonst <title>, sonst die Kennung.
function nameOf(el: Element): string {
  const label = el.getAttribute("aria-label");
  if (label) return label;
  const title = el.querySelector("title")?.textContent?.trim();
  return title || nodeId(el);
}

function element<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  props: Partial<Record<string, string>> = {},
  text?: string,
): HTMLElementTagNameMap[K] {
  const el = document.createElement(tag);
  for (const [key, value] of Object.entries(props)) if (value !== undefined) el.setAttribute(key, value);
  if (text !== undefined) el.textContent = text;
  return el;
}

register("idea-map", (stage) => {
  const work = num(stage.dataset.work, NaN);
  const svg = stage.querySelector<SVGSVGElement>("svg[data-im-svg]");
  const viewport = svg?.querySelector<SVGGElement>("g[data-im-viewport]");
  const side = stage.querySelector<HTMLElement>("[data-im-side]");
  const toolbar = stage.querySelector<HTMLElement>("[data-im-toolbar]");
  if (!svg || !viewport || !Number.isFinite(work)) return;
  const map = svg;
  const layer = viewport;

  // ---- Live-Region und Hilfe -------------------------------------------------------------------------------
  const status = element("p", { class: "im-status", role: "status", "aria-live": "polite" });
  stage.append(status);
  const announce = (message: string): void => {
    status.textContent = "";
    // Neu setzen, damit Bildschirmleser auch dieselbe Meldung zweimal vorlesen.
    window.setTimeout(() => (status.textContent = message), 30);
  };
  const helpId = `karte-help-${work}`;
  const help = element(
    "p",
    { class: "im-help", id: helpId },
    "Tastatur: Pfeile wandern, Enter öffnet einen Einfall, V verbindet, N schaltet die Nähe, + − 0 zoomen, " +
      "Alt+E legt einen Einfall an. Ziehen: auf ein Kapitel (Bezug), auf einen Einfall (Verbindung) oder frei (Lage).",
  );
  (toolbar ?? stage).append(help);
  map.setAttribute("aria-describedby", helpId);

  // Runde 18a: Die Seitenleiste beginnt am oberen Rand des Bilds (unter Werkzeugleiste, Hilfe und Nähe-Hinweis),
  // statt rechts einen Teil der Werkzeugleiste zu überdecken (karte_widget.css liest --im-top).
  const keepSideBelowToolbar = (): void => {
    const top = map.getBoundingClientRect().top - stage.getBoundingClientRect().top;
    stage.style.setProperty("--im-top", `${Math.max(0, Math.round(top))}px`);
  };
  keepSideBelowToolbar();
  if (typeof ResizeObserver !== "undefined") new ResizeObserver(keepSideBelowToolbar).observe(toolbar ?? stage);
  window.addEventListener("resize", keepSideBelowToolbar);

  // ---- Ausschnitt (Zoom und Verschieben) ------------------------------------------------------------------
  const viewKey = `baruch_im_view_${work}`;
  let view: View = { s: 1, tx: 0, ty: 0 };
  const applyView = (): void => {
    layer.setAttribute("transform", `translate(${view.tx.toFixed(2)} ${view.ty.toFixed(2)}) scale(${view.s.toFixed(4)})`);
  };
  const saveView = (): void => {
    try {
      sessionStorage.setItem(viewKey, JSON.stringify(view));
    } catch {
      /* ohne Speicher kein Gedächtnis des Ausschnitts */
    }
  };
  try {
    const saved = JSON.parse(sessionStorage.getItem(viewKey) ?? "null") as View | null;
    if (saved && [saved.s, saved.tx, saved.ty].every(Number.isFinite) && saved.s >= MIN_SCALE && saved.s <= MAX_SCALE) {
      view = saved;
      applyView();
    }
  } catch {
    /* kein gespeicherter Ausschnitt */
  }

  /// Bildschirmpunkt → Koordinaten der viewBox (ohne Transform des Viewports).
  const rootPoint = (clientX: number, clientY: number): DOMPoint | null => {
    const m = map.getScreenCTM();
    return m ? new DOMPoint(clientX, clientY).matrixTransform(m.inverse()) : null;
  };
  /// Bildschirmpunkt → Koordinaten der Knoten (`data-x`/`data-y`), über die CTM des Viewports.
  const contentPoint = (clientX: number, clientY: number): DOMPoint | null => {
    const m = layer.getScreenCTM();
    return m ? new DOMPoint(clientX, clientY).matrixTransform(m.inverse()) : null;
  };
  const viewBox = (): { x: number; y: number; width: number; height: number } => {
    const box = map.viewBox.baseVal;
    if (box && box.width > 0 && box.height > 0) return { x: box.x, y: box.y, width: box.width, height: box.height };
    const rect = map.getBoundingClientRect();
    return { x: 0, y: 0, width: rect.width || 1, height: rect.height || 1 };
  };
  const zoomAt = (factor: number, px: number, py: number): void => {
    const next = clamp(view.s * factor, MIN_SCALE, MAX_SCALE);
    const k = next / view.s;
    view = { s: next, tx: px - (px - view.tx) * k, ty: py - (py - view.ty) * k };
    applyView();
    saveView();
  };
  const zoomCenter = (factor: number): void => {
    const box = viewBox();
    zoomAt(factor, box.x + box.width / 2, box.y + box.height / 2);
  };
  /// Alle Knoten in den Ausschnitt einpassen.
  const fit = (): void => {
    const nodes = allNodes();
    if (nodes.length === 0) {
      view = { s: 1, tx: 0, ty: 0 };
    } else {
      let [minX, minY, maxX, maxY] = [Infinity, Infinity, -Infinity, -Infinity];
      for (const el of nodes) {
        const { x, y, r } = geometry(el);
        minX = Math.min(minX, x - r);
        minY = Math.min(minY, y - r);
        maxX = Math.max(maxX, x + r);
        maxY = Math.max(maxY, y + r);
      }
      // Platz für die Beschriftungen unter und neben den Kreisen (sonst schneidet das Einpassen sie ab).
      minX -= LABEL_SIDE;
      maxX += LABEL_SIDE;
      minY -= LABEL_SIDE / 2;
      maxY += LABEL_BELOW;
      const box = viewBox();
      const width = Math.max(maxX - minX, 1);
      const height = Math.max(maxY - minY, 1);
      const s = clamp(Math.min(box.width / width, box.height / height) * 0.95, MIN_SCALE, MAX_SCALE);
      view = {
        s,
        tx: box.x + (box.width - width * s) / 2 - minX * s,
        ty: box.y + (box.height - height * s) / 2 - minY * s,
      };
    }
    applyView();
    saveView();
    announce(`Eingepasst (${Math.round(view.s * 100)} %).`);
  };

  map.addEventListener(
    "wheel",
    (event) => {
      if (event.ctrlKey || event.deltaY === 0) return; // Strg+Mausrad gehört zoom.ts
      event.preventDefault();
      const p = rootPoint(event.clientX, event.clientY);
      if (p) zoomAt(event.deltaY < 0 ? STEP : 1 / STEP, p.x, p.y);
    },
    { passive: false },
  );

  if (toolbar) {
    const group = element("div", { class: "im-zoom", role: "group", "aria-label": "Zoom der Karte" });
    const button = (action: string, label: string, text: string): HTMLButtonElement => {
      const b = element("button", { type: "button", class: "btn", "data-im-zoom": action, "aria-label": label, title: label }, text);
      group.append(b);
      return b;
    };
    button("in", "Vergrößern (+)", "+").addEventListener("click", () => zoomCenter(STEP));
    button("out", "Verkleinern (−)", "−").addEventListener("click", () => zoomCenter(1 / STEP));
    button("fit", "Einpassen (0)", "Einpassen").addEventListener("click", fit);
    toolbar.prepend(group);
  }

  // ---- Knoten und Kanten ----------------------------------------------------------------------------------
  const allNodes = (): SVGElement[] => Array.from(map.querySelectorAll<SVGElement>(".im-node[data-node]"));
  const geometry = (el: Element): { x: number; y: number; r: number } => {
    const d = (el as SVGElement).dataset;
    return { x: num(d.x), y: num(d.y), r: num(d.r, 10) };
  };
  const nodeById = (id: string | undefined): SVGElement | null =>
    id === undefined ? null : map.querySelector<SVGElement>(`.im-node[data-node="${CSS.escape(id)}"]`);
  /// Kante neu zeichnen (Runde 18a): Mittelpunkt zu Mittelpunkt; mit Pfeilspitze (`data-arrow`) endet sie am Rand
  /// des Zielkreises wie in svg.rs, damit die Spitze nicht unter dem Kreis liegt.
  const redraw = (line: SVGElement): void => {
    const a = nodeById(line.dataset.from);
    const b = nodeById(line.dataset.to);
    if (!a || !b) return;
    const ga = geometry(a);
    const gb = geometry(b);
    let x2 = gb.x;
    let y2 = gb.y;
    if (line.hasAttribute("data-arrow")) {
      const d = Math.hypot(gb.x - ga.x, gb.y - ga.y);
      if (d > gb.r + 2) {
        x2 = gb.x - ((gb.x - ga.x) / d) * (gb.r + 2);
        y2 = gb.y - ((gb.y - ga.y) / d) * (gb.r + 2);
      }
    }
    const round = (v: number): string => String(Math.round(v * 10) / 10);
    line.setAttribute("x1", round(ga.x));
    line.setAttribute("y1", round(ga.y));
    line.setAttribute("x2", round(x2));
    line.setAttribute("y2", round(y2));
  };
  /// Beschriftung einer Kante in die Mitte zwischen beide Knoten (wie svg.rs: 4 Einheiten darüber).
  const relabel = (label: SVGElement): void => {
    const a = nodeById(label.dataset.from);
    const b = nodeById(label.dataset.to);
    if (!a || !b) return;
    const ga = geometry(a);
    const gb = geometry(b);
    label.setAttribute("x", String(Math.round(((ga.x + gb.x) / 2) * 10) / 10));
    label.setAttribute("y", String(Math.round(((ga.y + gb.y) / 2 - 4) * 10) / 10));
  };
  /// Knoten an (x, y) setzen und jede Kante, die an ihm hängt, samt Beschriftung live nachziehen.
  const place = (el: SVGElement, x: number, y: number): void => {
    el.dataset.x = String(Math.round(x * 100) / 100);
    el.dataset.y = String(Math.round(y * 100) / 100);
    el.setAttribute("transform", `translate(${el.dataset.x} ${el.dataset.y})`);
    const id = CSS.escape(nodeId(el));
    map
      .querySelectorAll<SVGLineElement>(`.im-edge[data-from="${id}"], .im-edge[data-to="${id}"]`)
      .forEach(redraw);
    map
      .querySelectorAll<SVGTextElement>(`.im-edge-label[data-from="${id}"], .im-edge-label[data-to="${id}"]`)
      .forEach(relabel);
  };
  /// Knoten unter (x, y), außer `except` und dem Werk: Einfälle vor Gliederungsknoten, dann der nächste.
  const targetAt = (except: Element, x: number, y: number): SVGElement | null => {
    let best: { el: SVGElement; idea: boolean; d: number } | null = null;
    for (const el of allNodes()) {
      if (el === except || nodeId(el) === "w") continue;
      const g = geometry(el);
      const d = Math.hypot(g.x - x, g.y - y);
      if (d > g.r) continue;
      const idea = isIdea(el);
      if (!best || (idea && !best.idea) || (idea === best.idea && d < best.d)) best = { el, idea, d };
    }
    return best?.el ?? null;
  };

  // Wandernder tabindex: genau ein Knoten ist per Tab erreichbar (zuerst das Werk), die übrigen über die Pfeile.
  const nodes = allNodes();
  for (const el of nodes) {
    el.setAttribute("tabindex", "-1");
    if (!el.getAttribute("aria-label")) el.setAttribute("aria-label", nameOf(el));
    if (el.tagName.toLowerCase() === "g") el.setAttribute("role", "button");
  }
  const entry = nodes.find((el) => nodeId(el) === "w") ?? nodes[0];
  entry?.setAttribute("tabindex", "0");
  let current: SVGElement | undefined = entry;
  map.addEventListener("focusin", (event) => {
    const el = (event.target as Element | null)?.closest<SVGElement>(".im-node[data-node]");
    if (!el || el === current) return;
    current?.setAttribute("tabindex", "-1");
    el.setAttribute("tabindex", "0");
    current = el;
  });
  const focusNode = (el: SVGElement): void => {
    el.focus();
    // Liegt der Knoten außerhalb des Ausschnitts, wird er in die Mitte geholt.
    const rect = el.getBoundingClientRect();
    const frame = map.getBoundingClientRect();
    if (rect.right < frame.left || rect.left > frame.right || rect.bottom < frame.top || rect.top > frame.bottom) {
      const { x, y } = geometry(el);
      const box = viewBox();
      view = { ...view, tx: box.x + box.width / 2 - x * view.s, ty: box.y + box.height / 2 - y * view.s };
      applyView();
      saveView();
    }
  };
  /// Nächster Knoten in der Richtung (y wächst nach unten): Abstand in der Richtung plus doppelter Querversatz.
  const neighbour = (from: SVGElement, direction: Direction, only?: (el: SVGElement) => boolean): SVGElement | null => {
    const f = geometry(from);
    let best: { el: SVGElement; score: number } | null = null;
    for (const el of allNodes()) {
      if (el === from || (only && !only(el))) continue;
      const g = geometry(el);
      const dx = g.x - f.x;
      const dy = g.y - f.y;
      const [main, cross] =
        direction === "right" ? [dx, dy] : direction === "left" ? [-dx, dy] : direction === "down" ? [dy, dx] : [-dy, dx];
      if (main <= 0) continue;
      const score = main + 2 * Math.abs(cross);
      if (!best || score < best.score) best = { el, score };
    }
    return best?.el ?? null;
  };
  const focused = (): SVGElement | null => {
    const active = document.activeElement;
    return active instanceof Element ? active.closest<SVGElement>(".im-node[data-node]") : null;
  };

  // ---- Neu laden --------------------------------------------------------------------------------------------
  const reload = (): void => {
    saveView();
    window.location.reload();
  };
  const failed = (error: unknown): void => {
    announce(`Fehler: ${String((error as Error)?.message ?? error)}`);
  };

  // ---- Verbinden (Dialog) -------------------------------------------------------------------------------
  let connecting: SVGElement | null = null;
  const stopConnecting = (): void => {
    connecting?.classList.remove("im-source");
    map.querySelectorAll(".im-target").forEach((el) => el.classList.remove("im-target"));
    stage.classList.remove("im-connecting");
    connecting = null;
  };
  const linkDialog = (from: SVGElement, to: SVGElement): void => {
    const fromId = num(from.dataset.idea, NaN);
    const toId = num(to.dataset.idea, NaN);
    if (!Number.isFinite(fromId) || !Number.isFinite(toId) || fromId === toId) return;
    const dialog = element("dialog", { class: "im-dialog", "aria-labelledby": "karte-link-title" });
    const form = element("form", { method: "dialog" });
    form.append(element("h2", { id: "karte-link-title" }, "Einfälle verbinden"));
    form.append(element("p", { class: "muted" }, `${nameOf(from)} → ${nameOf(to)}`));
    const set = element("fieldset");
    set.append(element("legend", {}, "Art der Verbindung"));
    KINDS.forEach(([value, label], index) => {
      const row = element("label", { class: "im-kind" });
      const radio = element("input", { type: "radio", name: "kind", value });
      if (index === 0) radio.checked = true;
      row.append(radio, document.createTextNode(` ${label}`));
      set.append(row);
    });
    form.append(set);
    const noteLabel = element("label", { class: "im-note" }, "Notiz (höchstens 500 Zeichen, optional)");
    const note = element("textarea", { name: "note", rows: "3", maxlength: String(MAX_NOTE) });
    noteLabel.append(note);
    form.append(noteLabel);
    const line = element("p", { class: "im-dialog-status", role: "alert" });
    const actions = element("div", { class: "im-dialog-actions" });
    const ok = element("button", { type: "submit", class: "btn primary" }, "Verbinden");
    const cancel = element("button", { type: "button", class: "btn" }, "Abbrechen");
    actions.append(ok, cancel);
    form.append(line, actions);
    dialog.append(form);
    document.body.append(dialog);
    let busy = false;
    cancel.addEventListener("click", () => dialog.close());
    dialog.addEventListener("close", () => {
      dialog.remove();
      if (!busy) {
        announce("Verbinden abgebrochen.");
        from.focus();
      }
    });
    form.addEventListener("submit", (event) => {
      event.preventDefault();
      if (busy) return;
      const kind = (form.querySelector<HTMLInputElement>("input[name=kind]:checked")?.value ?? "leads_to").trim();
      busy = true;
      call("idea.link", { from: fromId, to: toId, kind, note: note.value })
        .then(reload)
        .catch((error: unknown) => {
          busy = false;
          line.textContent = String((error as Error)?.message ?? error);
        });
    });
    dialog.showModal();
    form.querySelector<HTMLInputElement>("input[name=kind]")?.focus();
  };

  // ---- Seitenleiste ---------------------------------------------------------------------------------------
  let sideFor: SVGElement | null = null;
  const closeSide = (): void => {
    if (!side) return;
    side.hidden = true;
    side.replaceChildren();
    sideFor?.focus();
    sideFor = null;
  };
  const openSide = (el: SVGElement): void => {
    const id = num(el.dataset.idea, NaN);
    if (!side || !Number.isFinite(id)) return;
    sideFor = el;
    side.hidden = false;
    side.replaceChildren(element("p", { class: "muted" }, `Einfall ${id} wird geladen …`));
    call<{ idea: IdeaJson }>("idea.get", { id })
      .then(({ idea }) => {
        if (sideFor !== el) return;
        const form = element("form", { class: "im-side-form", "aria-labelledby": "karte-side-title" });
        form.append(element("h2", { id: "karte-side-title" }, `Einfall ${idea.id}`));
        const field = (label: string, input: HTMLElement): void => {
          const wrap = element("label", {}, label);
          wrap.append(input);
          form.append(wrap);
        };
        const words = element("input", { type: "text", name: "words", required: "" });
        words.value = idea.words.join(", ");
        field("Schlagwörter", words);
        const note = element("textarea", { name: "note", rows: "5" });
        note.value = idea.note;
        field("Gedanke", note);
        const state = element("select", { name: "status" });
        for (const [value, label] of [["open", "offen"], ["done", "verarbeitet"]]) {
          const option = element("option", { value }, label);
          option.selected = idea.status === value;
          state.append(option);
        }
        field("Status", state);
        if (idea.node_title) form.append(element("p", { class: "muted small" }, `Bezug: ${idea.node_title}`));
        if (idea.quote) form.append(element("blockquote", { class: "im-side-quote" }, idea.quote));
        const line = element("p", { class: "im-side-status", role: "alert" });
        const actions = element("div", { class: "im-side-actions" });
        const save = element("button", { type: "submit", class: "btn primary" }, "Speichern");
        const close = element("button", { type: "button", class: "btn" }, "Schließen");
        const page = element("a", { href: `/einfaelle/${idea.id}`, class: "btn" }, "Seite des Einfalls");
        actions.append(save, close, page);
        form.append(line, actions);
        close.addEventListener("click", closeSide);
        form.addEventListener("submit", (event) => {
          event.preventDefault();
          call("idea.update", { id: idea.id, words: words.value, note: note.value, status: state.value })
            .then(reload)
            .catch((error: unknown) => (line.textContent = String((error as Error)?.message ?? error)));
        });
        side.replaceChildren(form);
        words.focus();
      })
      .catch((error: unknown) => {
        side.replaceChildren(element("p", { role: "alert" }, String((error as Error)?.message ?? error)));
      });
  };
  side?.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      closeSide();
    }
  });

  // ---- Schnellerfassung (Alt+E) ---------------------------------------------------------------------------
  /// Bezug für einen neuen Einfall: Schlüssel eines Gliederungsknotens bzw. der Bezug des fokussierten Einfalls
  /// (sonst dessen Anker); Werk und „Ohne Kapitel“ → ohne Bezug.
  const anchorKey = (el: SVGElement | null): number | null => {
    if (!el) return null;
    if (isIdea(el)) {
      const own = num(el.dataset.key, NaN);
      if (Number.isFinite(own)) return own;
      const anchor = el.dataset.anchor ?? "";
      return anchor.startsWith("n:") ? num(anchor.slice(2), NaN) || null : null;
    }
    const key = num(el.dataset.key, NaN);
    return Number.isFinite(key) ? key : null;
  };
  const capture = (): void => {
    if (document.querySelector("dialog.im-dialog[open]")) return;
    const at = focused() ?? current ?? null;
    const key = anchorKey(at);
    const dialog = element("dialog", { class: "im-dialog", "aria-labelledby": "karte-new-title" });
    const form = element("form", { method: "dialog" });
    form.append(element("h2", { id: "karte-new-title" }, "Neuer Einfall"));
    form.append(element("p", { class: "muted" }, key === null ? "Ohne Bezug" : `Bezug: ${at ? nameOf(at) : key}`));
    const wordsLabel = element("label", {}, "Schlagwörter (mit Komma getrennt)");
    const words = element("input", { type: "text", name: "words", required: "" });
    wordsLabel.append(words);
    const noteLabel = element("label", {}, "Gedanke");
    const note = element("textarea", { name: "note", rows: "3" });
    noteLabel.append(note);
    const line = element("p", { class: "im-dialog-status", role: "alert" });
    const actions = element("div", { class: "im-dialog-actions" });
    const ok = element("button", { type: "submit", class: "btn primary" }, "Festhalten");
    const cancel = element("button", { type: "button", class: "btn" }, "Abbrechen");
    actions.append(ok, cancel);
    form.append(wordsLabel, noteLabel, line, actions);
    dialog.append(form);
    document.body.append(dialog);
    let busy = false;
    cancel.addEventListener("click", () => dialog.close());
    dialog.addEventListener("close", () => {
      dialog.remove();
      if (!busy) at?.focus();
    });
    form.addEventListener("submit", (event) => {
      event.preventDefault();
      if (busy) return;
      if (words.value.trim() === "") {
        line.textContent = "Bitte mindestens ein Schlagwort.";
        words.focus();
        return;
      }
      busy = true;
      call("idea.create", { work, key, words: words.value, note: note.value })
        .then(reload)
        .catch((error: unknown) => {
          busy = false;
          line.textContent = String((error as Error)?.message ?? error);
        });
    });
    dialog.showModal();
    words.focus();
  };
  document.addEventListener("keydown", (event) => {
    if (!event.altKey || event.ctrlKey || event.metaKey) return;
    // `code` dazu: Auf manchen Tastaturen ergibt Alt+E ein anderes Zeichen (z. B. „€“).
    if (event.key.toLowerCase() !== "e" && event.code !== "KeyE") return;
    if (!stage.isConnected) return;
    event.preventDefault();
    capture();
  });

  // ---- Nähe ein/aus ---------------------------------------------------------------------------------------
  const toggleNear = (): void => {
    const shown = stage.dataset.near !== "0" && !stage.classList.contains("im-hide-near");
    const next = !shown;
    const url = new URL(window.location.href);
    url.searchParams.set("near", next ? "1" : "0");
    if (next && stage.dataset.near === "0" && !map.querySelector(".im-near")) {
      // Ohne Nähe gezeichnet: die Kanten fehlen im Bild, also mit near=1 neu holen.
      saveView();
      window.location.replace(url.toString());
      return;
    }
    stage.classList.toggle("im-hide-near", !next);
    history.replaceState(history.state, "", url.toString());
    const box = document.querySelector<HTMLInputElement>("form.im-filter input[name=near]");
    if (box) box.checked = next;
    announce(next ? "Nähe eingeblendet." : "Nähe ausgeblendet.");
  };

  // ---- Ziehen und Klicken ---------------------------------------------------------------------------------
  interface Drag {
    el: SVGElement;
    pointer: number;
    startClientX: number;
    startClientY: number;
    startX: number;
    startY: number;
    offsetX: number;
    offsetY: number;
    moving: boolean;
    target: SVGElement | null;
  }
  interface Pan {
    pointer: number;
    start: DOMPoint;
    view: View;
  }
  let drag: Drag | null = null;
  let pan: Pan | null = null;
  let swallowClick = false;

  const setTarget = (el: SVGElement | null): void => {
    if (drag?.target === el) return;
    drag?.target?.classList.remove("im-drop-target");
    el?.classList.add("im-drop-target");
    if (drag) drag.target = el;
  };

  map.addEventListener("pointerdown", (event) => {
    if (event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey) return;
    const node = (event.target as Element | null)?.closest<SVGElement>(".im-node[data-node]");
    if (node) {
      if (!isIdea(node)) return; // Gliederungsknoten bleiben Links
      const c = contentPoint(event.clientX, event.clientY);
      if (!c) return;
      const { x, y } = geometry(node);
      drag = {
        el: node,
        pointer: event.pointerId,
        startClientX: event.clientX,
        startClientY: event.clientY,
        startX: x,
        startY: y,
        offsetX: c.x - x,
        offsetY: c.y - y,
        moving: false,
        target: null,
      };
      return;
    }
    const p = rootPoint(event.clientX, event.clientY);
    if (!p) return;
    pan = { pointer: event.pointerId, start: p, view: { ...view } };
    map.setPointerCapture(event.pointerId);
    stage.classList.add("im-panning");
  });

  map.addEventListener("pointermove", (event) => {
    if (pan && event.pointerId === pan.pointer) {
      const p = rootPoint(event.clientX, event.clientY);
      if (!p) return;
      view = { s: pan.view.s, tx: pan.view.tx + (p.x - pan.start.x), ty: pan.view.ty + (p.y - pan.start.y) };
      applyView();
      return;
    }
    if (!drag || event.pointerId !== drag.pointer) return;
    if (!drag.moving) {
      if (Math.hypot(event.clientX - drag.startClientX, event.clientY - drag.startClientY) < DRAG_THRESHOLD) return;
      drag.moving = true;
      drag.el.classList.add("im-dragging");
      stage.classList.add("im-drag-active");
      map.setPointerCapture(event.pointerId);
    }
    const c = contentPoint(event.clientX, event.clientY);
    if (!c) return;
    const x = clamp(c.x - drag.offsetX, -COORD_LIMIT, COORD_LIMIT);
    const y = clamp(c.y - drag.offsetY, -COORD_LIMIT, COORD_LIMIT);
    place(drag.el, x, y);
    setTarget(targetAt(drag.el, x, y));
  });

  const endPan = (): void => {
    if (!pan) return;
    pan = null;
    stage.classList.remove("im-panning");
    saveView();
  };

  /// Ablage eines gezogenen Einfalls (Vertrag §4.5).
  const drop = (d: Drag): void => {
    const el = d.el;
    const id = num(el.dataset.idea, NaN);
    const target = d.target;
    const back = (): void => place(el, d.startX, d.startY);
    if (target && isIdea(target)) {
      back();
      linkDialog(el, target);
      return;
    }
    if (target && (isOutlineNode(target) || nodeId(target) === "none")) {
      const key = nodeId(target) === "none" ? null : num(target.dataset.key, NaN);
      if (key !== null && !Number.isFinite(key)) return back();
      back();
      announce(key === null ? "Bezug wird entfernt …" : `Bezug wird gesetzt: ${nameOf(target)} …`);
      call("idea.update", { id, key }).then(reload).catch((error: unknown) => failed(error));
      return;
    }
    const x = clamp(num(el.dataset.x), -COORD_LIMIT, COORD_LIMIT);
    const y = clamp(num(el.dataset.y), -COORD_LIMIT, COORD_LIMIT);
    call("idea.map_pos_set", { work, idea: id, x, y })
      .then(() => {
        el.classList.add("im-pinned");
        announce(`Lage von ${nameOf(el)} gemerkt.`);
      })
      .catch((error: unknown) => {
        back();
        failed(error);
      });
  };

  const endDrag = (event: PointerEvent, cancelled: boolean): void => {
    if (!drag || event.pointerId !== drag.pointer) return;
    const d = drag;
    drag = null;
    d.el.classList.remove("im-dragging");
    stage.classList.remove("im-drag-active");
    d.target?.classList.remove("im-drop-target");
    if (!d.moving) return; // ein Klick: der click-Handler entscheidet
    swallowClick = true;
    if (cancelled) place(d.el, d.startX, d.startY);
    else drop(d);
  };

  map.addEventListener("pointerup", (event) => {
    endPan();
    endDrag(event, false);
  });
  map.addEventListener("pointercancel", (event) => {
    endPan();
    endDrag(event, true);
  });

  map.addEventListener("click", (event) => {
    const node = (event.target as Element | null)?.closest<SVGElement>(".im-node[data-node]");
    if (swallowClick) {
      swallowClick = false;
      event.preventDefault();
      return;
    }
    if (!node || !isIdea(node) || event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey) return;
    event.preventDefault();
    if (connecting) {
      if (node !== connecting) {
        const from = connecting;
        stopConnecting();
        linkDialog(from, node);
      }
      return;
    }
    openSide(node);
  });

  // ---- Tastatur -----------------------------------------------------------------------------------------
  const ARROWS: Record<string, Direction> = { ArrowLeft: "left", ArrowRight: "right", ArrowUp: "up", ArrowDown: "down" };
  stage.addEventListener("keydown", (event) => {
    const target = event.target as Element | null;
    if (target?.closest("input, textarea, select, [data-im-side], dialog, form")) return;
    if (event.ctrlKey || event.metaKey || event.altKey) return;
    const node = focused();
    const direction = ARROWS[event.key];
    if (direction) {
      const from = node ?? current;
      if (!from) return;
      event.preventDefault();
      const next = connecting
        ? neighbour(from, direction, (el) => isIdea(el) && el !== connecting)
        : neighbour(from, direction);
      if (!next) return;
      if (connecting) {
        map.querySelectorAll(".im-target").forEach((el) => el.classList.remove("im-target"));
        next.classList.add("im-target");
      }
      focusNode(next);
      return;
    }
    switch (event.key) {
      case "+":
      case "=":
        event.preventDefault();
        zoomCenter(STEP);
        return;
      case "-":
      case "−":
        event.preventDefault();
        zoomCenter(1 / STEP);
        return;
      case "0":
        event.preventDefault();
        fit();
        return;
      case "n":
      case "N":
        event.preventDefault();
        toggleNear();
        return;
      case "v":
      case "V":
        if (!node || !isIdea(node)) {
          announce("Verbinden geht von einem Einfall aus: erst einen Einfall wählen.");
          return;
        }
        event.preventDefault();
        stopConnecting();
        connecting = node;
        node.classList.add("im-source");
        stage.classList.add("im-connecting");
        announce(`Verbinden von ${nameOf(node)}: Ziel mit den Pfeilen wählen, Enter verbindet, Escape bricht ab.`);
        return;
      case "Enter":
        if (!node) return;
        if (connecting) {
          event.preventDefault();
          if (isIdea(node) && node !== connecting) {
            const from = connecting;
            stopConnecting();
            linkDialog(from, node);
          } else {
            announce("Ziel muss ein anderer Einfall sein.");
          }
          return;
        }
        if (isIdea(node)) {
          event.preventDefault();
          openSide(node);
        }
        return; // Gliederungsknoten: Enter folgt dem Link
      case "Escape":
        if (connecting) {
          event.preventDefault();
          const from = connecting;
          stopConnecting();
          from.focus();
          announce("Verbinden abgebrochen.");
        } else if (side && !side.hidden) {
          event.preventDefault();
          closeSide();
        }
        return;
    }
  });

  // „Anordnung zurücksetzen“ (Formular der Werkzeugleiste, ohne JavaScript an /werke/W/karte/act).
  toolbar?.querySelectorAll<HTMLFormElement>("form").forEach((form) => {
    const method = form.querySelector<HTMLInputElement>("input[name=method]")?.value;
    if (method !== "idea.map_pos_reset") return;
    form.addEventListener("submit", (event) => {
      event.preventDefault();
      call("idea.map_pos_reset", { work }).then(reload).catch((error: unknown) => failed(error));
    });
  });

  stage.classList.add("im-ready");
});
