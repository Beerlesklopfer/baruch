// SPDX-License-Identifier: AGPL-3.0-or-later
// Widget der Mindmap (Runde 34, Vertrag §7, `data-widget="mindmap"` auf `.mm-stage`). Ohne JavaScript bleibt die
// Seite ein Bild mit Links (Einklappen, Abschnitt) plus Gliederungsansicht; das Widget liest nur den DOM-Vertrag
// des Bildes (src/idea_map/bild/mindmap.rs) und ruft nur die RPC `idea.create`, `idea.update`, `idea.move`,
// `idea.delete`, `idea.restore`, `idea.link`, `idea.unlink` und `idea.link_restore`. Es ordnet nichts selbst an:
// Nach jeder Aktion holt es das Bild über `data-bild` neu (gleicher Filter aus `data-back`, eingeklappte Knoten
// als `zu=`) und behält Auswahl und Ausschnitt.
//
// - Klick wählt; Doppelklick oder F2 benennt um (nur `data-editable="1"`; Enter übernimmt, Esc verwirft).
// - Tab legt einen Unterknoten an, Enter ein Geschwister (vor dem nächsten Geschwister); beides steht sofort im
//   Umbenennen. Am Werk landet der neue Einfall in der Wolke, ebenso mit Alt+E.
// - Entf/Rücktaste löscht nach Rückfrage (samt Unterknoten, wiederherstellbar); an einer gewählten Beziehung löst
//   Entf die Beziehung.
// - Ziehen eines Einfalls: auf einen Einfall → Unterknoten (`parent`); auf dessen oberen/unteren Rand → davor bzw.
//   dahinter einordnen (`before`); auf Teil/Kapitel → dorthin (`parent: null, key`); aufs Werk oder in die Wolke →
//   in die Wolke (`parent: null, key: null`, die Unterknoten wandern mit). Aus der Wolke auf einen Knoten ziehen
//   hängt den Einfall dort ein.
// - Pfeile wandern (links/rechts = Eltern/Kind je nach Seite, hoch/runter = Geschwister), Leertaste klappt,
//   Strg+Mausrad, `+`, `-`, `0` zoomen, Ziehen der Fläche verschiebt, Strg+Z nimmt die letzte Aktion zurück
//   (eine Stufe, Umkehr-RPC), Alt+L bzw. „Verbinden“ zieht eine Beziehung zum nächsten gewählten Einfall.
// - Eingeklappt ist reine Anzeige: je Werk in localStorage (try/catch), nie auf dem Server.
// - Texte nur über textContent/value (nie als HTML); das Bild kommt fertig maskiert vom Server.

import { call } from "./rpc.js";
import { register } from "./widgets.js";

const MIN_SCALE = 0.2;
const MAX_SCALE = 6;
const STEP = 1.2;
/// Ab so vielen Bildschirmpunkten wird aus einem Klick ein Ziehen.
const DRAG_THRESHOLD = 5;
const MAX_TITLE = 200;
const MAX_NOTE = 500;
const NEW_TITLE = "Neuer Einfall";

const KINDS: ReadonlyArray<readonly [string, string]> = [
  ["leads_to", "führt weiter"],
  ["belongs_to", "gehört zu"],
  ["example_of", "Beispiel für"],
  ["contradicts", "widerspricht"],
];

type Kind = "work" | "part" | "chapter" | "idea";
type Direction = "left" | "right" | "up" | "down";

interface Info {
  el: SVGGElement;
  id: string;
  kind: Kind;
  idea: number | null;
  key: number | null;
  parent: string | null;
  side: string;
  editable: boolean;
  title: string;
  own: string;
  cloud: boolean;
  children: number;
}

interface Undo {
  label: string;
  run: () => Promise<unknown>;
}

interface Outcome {
  message: string;
  select?: string | null;
  after?: () => void;
}

interface MoveParams {
  parent?: number | null;
  key?: number | null;
  before?: number | null;
}

interface Drop {
  target: Info | null;
  cloud: boolean;
  zone: "into" | "before" | "after";
}

function numberOf(text: string | undefined): number | null {
  if (text === undefined || text === "") return null;
  const value = Number(text);
  return Number.isFinite(value) ? value : null;
}

function inField(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return false;
  return target.closest("input, textarea, select, dialog, [contenteditable=true]") !== null;
}

register("mindmap", (stage) => {
  const work = numberOf(stage.dataset.work);
  const bild = stage.dataset.bild ?? "";
  const back = stage.dataset.back ?? "";
  const canvas = stage.querySelector<HTMLElement>("[data-mm-canvas]");
  const status = stage.querySelector<HTMLElement>("[data-mm-status]");
  if (work === null || bild === "" || !canvas) return;

  let closed: string[] = (stage.dataset.zu ?? "").split(",").filter(Boolean);
  let selected: string | null = stage.dataset.wahl || null;
  let selectedLink: number | null = null;
  let undo: Undo | null = null;
  let linkFrom: string | null = null;
  let busy = false;
  let view = { s: 1, tx: 0, ty: 0 };
  let suppressClick = false;

  const closedKey = `baruch.mindmap.zu.${work}`;
  const viewKey = `baruch.mindmap.ausschnitt.${work}`;

  // ---- Bild und DOM-Vertrag ----------------------------------------------------------------------------------

  const svg = (): SVGSVGElement | null => canvas.querySelector<SVGSVGElement>("svg.mm-svg");
  const viewport = (): SVGGElement | null => canvas.querySelector<SVGGElement>(".mm-viewport");

  function info(el: SVGGElement): Info {
    const d = el.dataset;
    const kind: Kind = el.classList.contains("mm-idea")
      ? "idea"
      : el.classList.contains("mm-chapter")
        ? "chapter"
        : el.classList.contains("mm-part")
          ? "part"
          : "work";
    return {
      el,
      id: d.node ?? "",
      kind,
      idea: numberOf(d.idea),
      key: numberOf(d.key),
      parent: d.parent ?? null,
      side: d.side ?? "",
      editable: d.editable === "1",
      title: d.title ?? "",
      own: d.own ?? "",
      cloud: d.cloud === "1",
      children: numberOf(d.children) ?? 0,
    };
  }

  const all = (): Info[] => Array.from(canvas.querySelectorAll<SVGGElement>(".mm-node")).map(info);
  const byId = (id: string | null): Info | null => (id === null ? null : (all().find((n) => n.id === id) ?? null));
  const current = (): Info | null => byId(selected);
  const siblings = (n: Info): Info[] =>
    all().filter((m) => m.parent === n.parent && m.cloud === n.cloud && m.side === n.side);
  const parentIdea = (n: Info): number | null => (n.parent?.startsWith("i:") ? numberOf(n.parent.slice(2)) : null);

  function say(text: string, error = false, withUndo = false): void {
    if (!status) return;
    status.textContent = text;
    status.classList.toggle("mm-error", error);
    if (withUndo && undo) {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "btn small";
      button.textContent = "Rückgängig";
      button.addEventListener("click", () => void undoLast());
      status.append(button);
    }
  }

  function applyView(): void {
    viewport()?.setAttribute("transform", `translate(${view.tx} ${view.ty}) scale(${view.s})`);
    try {
      sessionStorage.setItem(viewKey, JSON.stringify(view));
    } catch {
      // Nur Bequemlichkeit.
    }
  }

  function remember(): void {
    try {
      localStorage.setItem(closedKey, closed.join(","));
    } catch {
      // Nur Bequemlichkeit: ohne Speicher gilt das Einklappen bis zum Neuladen.
    }
    const parts = [back, closed.length ? `zu=${closed.join(",")}` : "", selected ? `wahl=${selected}` : ""];
    const query = parts.filter(Boolean).join("&");
    try {
      history.replaceState(history.state, "", query ? `${location.pathname}?${query}` : location.pathname);
    } catch {
      // Adresse bleibt, wie sie ist.
    }
  }

  function select(id: string | null, focus = true): void {
    selectedLink = null;
    canvas?.querySelectorAll(".mm-selected").forEach((e) => e.classList.remove("mm-selected"));
    const nodes = all();
    const node = nodes.find((n) => n.id === id) ?? null;
    selected = node ? node.id : null;
    for (const n of nodes) n.el.setAttribute("tabindex", n === node || (!node && n.kind === "work") ? "0" : "-1");
    if (!node) return;
    node.el.classList.add("mm-selected");
    if (focus) node.el.focus({ preventScroll: true });
  }

  /// Holt das Bild neu (gleicher Filter, eingeklappte Knoten) und setzt es an die Stelle des alten.
  async function refresh(selectId?: string | null): Promise<void> {
    const query = [back, closed.length ? `zu=${closed.join(",")}` : ""].filter(Boolean).join("&");
    const response = await fetch(query ? `${bild}?${query}` : bild, { headers: { Accept: "image/svg+xml" } });
    if (!response.ok) throw new Error(`Das Bild ließ sich nicht neu holen (Server antwortet ${response.status}).`);
    const holder = document.createElement("template");
    holder.innerHTML = await response.text();
    const fresh = holder.content.querySelector("svg.mm-svg");
    if (!fresh || !canvas) throw new Error("Das Bild ließ sich nicht neu holen (keine Mindmap in der Antwort).");
    canvas.replaceChildren(fresh);
    closed = (fresh.getAttribute("data-zu") ?? "").split(",").filter(Boolean);
    const counter = document.querySelector<HTMLElement>("[data-mm-cloud-count]");
    const count = fresh.getAttribute("data-cloud-count") ?? "0";
    if (counter) {
      counter.dataset.mmCloudCount = count;
      counter.textContent = `${count} in der Wolke`;
    }
    applyView();
    select(selectId === undefined ? selected : selectId);
    remember();
  }

  /// Eine Aktion: RPC, Bild neu, Meldung (mit „Rückgängig“). Fehler stehen als Meldung da; das Bild wird dann
  /// trotzdem neu geholt, damit es den Stand des Servers zeigt.
  async function act(run: () => Promise<Outcome>): Promise<void> {
    if (busy) return;
    busy = true;
    try {
      const outcome = await run();
      await refresh(outcome.select);
      say(outcome.message, false, true);
      outcome.after?.();
    } catch (error) {
      say(error instanceof Error ? error.message : String(error), true);
      try {
        await refresh();
      } catch {
        // Die Meldung oben nennt den Fehler.
      }
    } finally {
      busy = false;
    }
  }

  async function undoLast(): Promise<void> {
    const step = undo;
    if (!step) {
      say("Nichts zurückzunehmen.");
      return;
    }
    await act(async () => {
      await step.run();
      undo = null;
      return { message: `Zurückgenommen: ${step.label}.` };
    });
  }

  // ---- Einklappen --------------------------------------------------------------------------------------------

  function toggle(id: string): void {
    closed = closed.includes(id) ? closed.filter((c) => c !== id) : [...closed, id];
    void refresh(id).catch((error: unknown) => say(error instanceof Error ? error.message : String(error), true));
  }

  function closedFromHref(href: string): void {
    const zu = new URL(href, location.href).searchParams.get("zu") ?? "";
    closed = zu.split(",").filter(Boolean);
    void refresh().catch((error: unknown) => say(error instanceof Error ? error.message : String(error), true));
  }

  // ---- Umbenennen, Anlegen, Löschen --------------------------------------------------------------------------

  function rename(n: Info): void {
    if (!n.editable || n.idea === null) {
      say("Umbenennen lassen sich nur Einfälle; Werk, Teile und Kapitel heißen wie in der Gliederung.");
      return;
    }
    const box = n.el.querySelector(".mm-box")?.getBoundingClientRect();
    const frame = stage.getBoundingClientRect();
    if (!box) return;
    const input = document.createElement("input");
    input.type = "text";
    input.className = "mm-edit";
    input.maxLength = MAX_TITLE;
    input.value = n.title;
    input.setAttribute("aria-label", "Titel des Einfalls (Enter übernimmt, Esc verwirft)");
    input.style.left = `${Math.max(4, box.left - frame.left)}px`;
    input.style.top = `${box.top - frame.top - 3}px`;
    input.style.width = `${Math.max(170, box.width + 30)}px`;
    stage.append(input);
    input.focus();
    input.select();
    let done = false;
    const idea = n.idea;
    const finish = (commit: boolean): void => {
      if (done) return;
      done = true;
      const title = input.value.trim().slice(0, MAX_TITLE);
      input.remove();
      if (!commit || title === "" || title === n.title) {
        select(n.id);
        return;
      }
      void act(async () => {
        await call("idea.update", { id: idea, title });
        undo = { label: "Umbenennen", run: () => call("idea.update", { id: idea, title: n.own }) };
        return { message: `Umbenannt in „${title}“.`, select: n.id };
      });
    };
    input.addEventListener("keydown", (event) => {
      event.stopPropagation();
      if (event.key === "Enter") {
        event.preventDefault();
        finish(true);
      } else if (event.key === "Escape") {
        event.preventDefault();
        finish(false);
      }
    });
    input.addEventListener("blur", () => finish(true));
  }

  /// Wohin ein neuer Einfall unter `p` gehört: Einfall → `parent`, Teil/Kapitel → `key`, Werk → nichts (Wolke).
  function placeUnder(params: Record<string, unknown>, p: Info | null): void {
    if (p?.kind === "idea" && p.idea !== null) params.parent = p.idea;
    else if (p && p.kind !== "work" && p.key !== null) params.key = p.key;
  }

  function create(mode: "child" | "sibling" | "cloud"): void {
    const n = current();
    const params: Record<string, unknown> = { work, title: NEW_TITLE };
    let open: string | null = null;
    if (mode === "sibling" && n && n.kind === "idea" && !n.cloud) {
      const row = siblings(n);
      const next = row[row.findIndex((m) => m.id === n.id) + 1];
      placeUnder(params, byId(n.parent));
      if (next && next.kind === "idea" && next.idea !== null) params.before = next.idea;
    } else if (mode !== "cloud" && n && !n.cloud) {
      placeUnder(params, n);
      open = n.id;
    } else if (mode !== "cloud" && n && n.cloud && mode === "child") {
      placeUnder(params, n);
    }
    void act(async () => {
      const answer = await call<{ idea?: { id?: number } }>("idea.create", params);
      const id = answer.idea?.id;
      if (typeof id !== "number") throw new Error("Der Server hat keinen neuen Einfall gemeldet.");
      if (open !== null) closed = closed.filter((c) => c !== open);
      undo = { label: "Anlegen", run: () => call("idea.delete", { id }) };
      const where = params.parent === undefined && params.key === undefined ? " in der Wolke" : "";
      return {
        message: `Einfall angelegt${where}. Titel eingeben, Enter übernimmt.`,
        select: `i:${id}`,
        after: () => {
          const fresh = byId(`i:${id}`);
          if (fresh) rename(fresh);
        },
      };
    });
  }

  function unlinkSelected(): void {
    const id = selectedLink;
    if (id === null) return;
    void act(async () => {
      await call("idea.unlink", { id });
      undo = { label: "Beziehung lösen", run: () => call("idea.link_restore", { id }) };
      return { message: "Beziehung gelöst." };
    });
  }

  function remove(): void {
    if (selectedLink !== null) {
      unlinkSelected();
      return;
    }
    const n = current();
    if (!n) return;
    if (!n.editable || n.idea === null) {
      say("Löschen lassen sich nur Einfälle; Teile und Kapitel ändert die Gliederung (Antrag).");
      return;
    }
    const below = n.children > 0 ? ` samt ${n.children} Unterknoten` : "";
    if (!window.confirm(`Einfall „${n.title}“${below} löschen? (wiederherstellbar)`)) return;
    const idea = n.idea;
    void act(async () => {
      await call("idea.delete", { id: idea });
      undo = { label: "Löschen", run: () => call("idea.restore", { id: idea }) };
      return { message: `Einfall „${n.title}“${below} gelöscht.`, select: n.cloud ? "w" : n.parent };
    });
  }

  // ---- Umhängen und Ordnen (idea.move) -----------------------------------------------------------------------

  function move(n: Info, params: MoveParams, message: string): void {
    if (n.idea === null) return;
    const idea = n.idea;
    const row = siblings(n);
    const next = row[row.findIndex((m) => m.id === n.id) + 1];
    const backTo: MoveParams = {
      parent: parentIdea(n),
      key: n.key,
      before: next && next.kind === "idea" ? next.idea : null,
    };
    void act(async () => {
      await call("idea.move", { id: idea, ...params });
      undo = { label: "Umhängen", run: () => call("idea.move", { id: idea, ...backTo }) };
      return { message, select: n.id };
    });
  }

  function toCloud(n: Info | null): void {
    if (!n || !n.editable || n.idea === null) {
      say("In die Wolke legen lassen sich nur Einfälle.");
      return;
    }
    if (n.cloud) {
      say("Dieser Einfall liegt schon in der Wolke.");
      return;
    }
    const below = n.children > 0 ? ` samt ${n.children} Unterknoten` : "";
    move(n, { parent: null, key: null }, `„${n.title}“${below} liegt jetzt in der Wolke.`);
  }

  function isInside(target: Info, n: Info): boolean {
    let at: Info | null = target;
    for (let guard = 0; at && guard < 64; guard++) {
      if (at.id === n.id) return true;
      at = byId(at.parent);
    }
    return false;
  }

  function dropAt(x: number, y: number): Drop | null {
    const under = document.elementFromPoint(x, y);
    const el = under?.closest<SVGGElement>(".mm-node");
    if (el && canvas?.contains(el)) {
      const target = info(el);
      const box = el.querySelector(".mm-box")?.getBoundingClientRect();
      let zone: Drop["zone"] = "into";
      if (box && target.kind === "idea") {
        const at = (y - box.top) / Math.max(1, box.height);
        zone = at < 0.25 ? "before" : at > 0.75 ? "after" : "into";
      }
      return { target, cloud: false, zone };
    }
    if (under?.closest(".mm-cloud")) return { target: null, cloud: true, zone: "into" };
    return null;
  }

  function drop(n: Info, where: Drop | null): void {
    if (!where) return;
    if (where.cloud || where.target?.kind === "work") {
      toCloud(n);
      return;
    }
    const target = where.target;
    if (!target || isInside(target, n)) {
      if (target && target.id !== n.id) say("Ein Einfall lässt sich nicht unter seinen eigenen Unterknoten hängen.", true);
      return;
    }
    if (target.kind !== "idea") {
      move(n, { parent: null, key: target.key }, `„${n.title}“ hängt jetzt an „${target.title}“.`);
      return;
    }
    if (where.zone === "into") {
      if (target.idea !== null) move(n, { parent: target.idea }, `„${n.title}“ ist jetzt Unterknoten von „${target.title}“.`);
      return;
    }
    // Davor/dahinter: gleicher Elternknoten wie das Ziel, `before` = Ziel bzw. dessen nächstes Geschwister.
    const row = siblings(target).filter((m) => m.id !== n.id);
    const after = row[row.findIndex((m) => m.id === target.id) + 1];
    const before = where.zone === "before" ? target.idea : after && after.kind === "idea" ? after.idea : null;
    const parent = parentIdea(target);
    const params: MoveParams = parent !== null ? { parent, before } : { parent: null, key: target.key, before };
    move(n, params, `„${n.title}“ steht jetzt ${where.zone === "before" ? "vor" : "hinter"} „${target.title}“.`);
  }

  // ---- Verbinden (idea.link) ---------------------------------------------------------------------------------

  function stopLinking(): void {
    linkFrom = null;
    stage.classList.remove("mm-linking");
    canvas?.querySelectorAll(".mm-source").forEach((e) => e.classList.remove("mm-source"));
  }

  function startLinking(): void {
    const n = current();
    if (!n || n.idea === null) {
      say("Verbinden: zuerst einen Einfall wählen.");
      return;
    }
    linkFrom = n.id;
    stage.classList.add("mm-linking");
    n.el.classList.add("mm-source");
    say(`Verbinden von „${n.title}“: jetzt den zweiten Einfall wählen (Klick oder Pfeile und Enter; Esc bricht ab).`);
  }

  function finishLinking(target: Info): void {
    const from = byId(linkFrom);
    stopLinking();
    if (!from || from.idea === null || target.idea === null || from.id === target.id) {
      say("Verbinden geht nur zwischen zwei verschiedenen Einfällen.", true);
      return;
    }
    const dialog = document.createElement("dialog");
    dialog.className = "mm-dialog";
    const form = document.createElement("form");
    form.method = "dialog";
    const head = document.createElement("h2");
    head.textContent = `„${from.title}“ verbinden mit „${target.title}“`;
    const kindLabel = document.createElement("label");
    kindLabel.textContent = "Art";
    const kind = document.createElement("select");
    for (const [value, label] of KINDS) {
      const option = document.createElement("option");
      option.value = value;
      option.textContent = label;
      kind.append(option);
    }
    kindLabel.append(kind);
    const noteLabel = document.createElement("label");
    noteLabel.textContent = "Notiz (Beschriftung der Linie)";
    const note = document.createElement("input");
    note.type = "text";
    note.maxLength = MAX_NOTE;
    noteLabel.append(note);
    const actions = document.createElement("div");
    actions.className = "mm-dialog-actions";
    const ok = document.createElement("button");
    ok.type = "submit";
    ok.className = "btn primary";
    ok.value = "ok";
    ok.textContent = "Verbinden";
    const cancel = document.createElement("button");
    cancel.type = "submit";
    cancel.className = "btn";
    cancel.value = "cancel";
    cancel.textContent = "Abbrechen";
    actions.append(ok, cancel);
    form.append(head, kindLabel, noteLabel, actions);
    dialog.append(form);
    stage.append(dialog);
    dialog.addEventListener("close", () => {
      const go = dialog.returnValue === "ok";
      const params = { from: from.idea, to: target.idea, kind: kind.value, note: note.value.trim() };
      dialog.remove();
      if (!go) {
        select(from.id);
        return;
      }
      void act(async () => {
        const answer = await call<{ link?: { id?: number }; changed?: boolean }>("idea.link", params);
        const id = answer.link?.id;
        undo = typeof id === "number" && answer.changed !== false
          ? { label: "Verbinden", run: () => call("idea.unlink", { id }) }
          : null;
        return {
          message: answer.changed === false ? "Diese Beziehung gibt es schon; nichts geändert." : "Verbunden.",
          select: from.id,
        };
      });
    });
    dialog.showModal();
    kind.focus();
  }

  // ---- Wandern, Zoom -----------------------------------------------------------------------------------------

  function step(direction: Direction): void {
    const n = current();
    if (!n) {
      select("w");
      return;
    }
    const nodes = all();
    const neighbour = (offset: number): Info | undefined => {
      const row = siblings(n);
      return row[row.findIndex((m) => m.id === n.id) + offset];
    };
    if (n.kind === "work") {
      const first =
        direction === "down"
          ? nodes.find((m) => m.cloud)
          : nodes.find((m) => m.parent === n.id && !m.cloud && m.side === direction);
      if (first) select(first.id);
      return;
    }
    if (n.cloud) {
      const other = neighbour(direction === "up" || direction === "left" ? -1 : 1);
      if (other) select(other.id);
      else if (direction === "up") select("w");
      return;
    }
    if (direction === "up" || direction === "down") {
      const other = neighbour(direction === "up" ? -1 : 1);
      if (other) select(other.id);
      return;
    }
    if (n.side === direction) {
      const child = nodes.find((m) => m.parent === n.id);
      if (child) select(child.id);
      else if (closed.includes(n.id)) toggle(n.id);
    } else if (n.parent) {
      select(n.parent);
    }
  }

  function zoom(factor: number, clientX?: number, clientY?: number): void {
    const image = svg();
    const matrix = image?.getScreenCTM();
    const s = Math.min(MAX_SCALE, Math.max(MIN_SCALE, view.s * factor));
    if (!image || !matrix || matrix.a === 0) {
      view = { ...view, s };
    } else {
      const rect = image.getBoundingClientRect();
      const px = ((clientX ?? rect.left + rect.width / 2) - matrix.e) / matrix.a;
      const py = ((clientY ?? rect.top + rect.height / 2) - matrix.f) / matrix.d;
      view = { s, tx: px - ((px - view.tx) * s) / view.s, ty: py - ((py - view.ty) * s) / view.s };
    }
    applyView();
  }

  function fit(): void {
    view = { s: 1, tx: 0, ty: 0 };
    applyView();
  }

  // ---- Maus --------------------------------------------------------------------------------------------------

  canvas.addEventListener("pointerdown", (event) => {
    if (event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey) return;
    const target = event.target instanceof Element ? event.target : null;
    if (!target || target.closest(".mm-toggle, .mm-section, .mm-findings")) return;
    const el = target.closest<SVGGElement>(".mm-node");
    const start = { x: event.clientX, y: event.clientY };
    const origin = { ...view };
    const node = el ? info(el) : null;
    const scale = svg()?.getScreenCTM()?.a || 1;
    let dragging = false;
    let ghost: HTMLElement | null = null;
    let bar: HTMLElement | null = null;
    let where: Drop | null = null;
    const clearMarks = (): void => {
      canvas.querySelectorAll(".mm-drop, .mm-drop-before, .mm-drop-after").forEach((e) =>
        e.classList.remove("mm-drop", "mm-drop-before", "mm-drop-after"),
      );
      bar?.remove();
      bar = null;
    };
    const onMove = (moved: PointerEvent): void => {
      const dx = moved.clientX - start.x;
      const dy = moved.clientY - start.y;
      if (!dragging && Math.hypot(dx, dy) < DRAG_THRESHOLD) return;
      if (!node) {
        // Fläche verschieben.
        dragging = true;
        stage.classList.add("mm-panning");
        view = { s: origin.s, tx: origin.tx + dx / scale, ty: origin.ty + dy / scale };
        applyView();
        return;
      }
      if (!node.editable || node.idea === null) return;
      if (!dragging) {
        dragging = true;
        stage.classList.add("mm-dragging");
        node.el.classList.add("mm-moving");
        ghost = document.createElement("div");
        ghost.className = "mm-drag-ghost";
        ghost.textContent = node.title;
        document.body.append(ghost);
      }
      if (ghost) {
        ghost.style.left = `${moved.clientX + 12}px`;
        ghost.style.top = `${moved.clientY + 10}px`;
      }
      clearMarks();
      where = dropAt(moved.clientX, moved.clientY);
      if (where?.cloud) {
        canvas.querySelector(".mm-cloud")?.classList.add("mm-drop");
      } else if (where?.target && !isInside(where.target, node)) {
        const mark = where.zone === "into" ? "mm-drop" : where.zone === "before" ? "mm-drop-before" : "mm-drop-after";
        where.target.el.classList.add(mark);
        const box = where.target.el.querySelector(".mm-box")?.getBoundingClientRect();
        if (box && where.zone !== "into") {
          bar = document.createElement("div");
          bar.className = "mm-drop-bar";
          bar.style.left = `${box.left}px`;
          bar.style.width = `${box.width}px`;
          bar.style.top = `${(where.zone === "before" ? box.top : box.bottom) - 1}px`;
          document.body.append(bar);
        }
      }
    };
    const onUp = (): void => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onUp);
      stage.classList.remove("mm-panning", "mm-dragging");
      node?.el.classList.remove("mm-moving");
      ghost?.remove();
      clearMarks();
      if (!dragging) return;
      // Der Klick nach dem Loslassen gehört zum Ziehen, nicht zur Auswahl.
      suppressClick = true;
      window.setTimeout(() => (suppressClick = false), 0);
      if (node) drop(node, where);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
  });

  canvas.addEventListener("click", (event) => {
    const target = event.target instanceof Element ? event.target : null;
    if (!target) return;
    if (suppressClick) {
      event.preventDefault();
      return;
    }
    // Mit Strg/Umschalt/Mitteltaste bleibt ein Link ein Link (neuer Tab).
    if (event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey) return;
    const el = target.closest<SVGGElement>(".mm-node");
    if (target.closest(".mm-toggle") && el) {
      event.preventDefault();
      toggle(info(el).id);
      return;
    }
    if (target.closest(".mm-section, .mm-findings")) return;
    const link = target.closest<SVGPathElement>(".mm-link");
    if (link) {
      select(null);
      selectedLink = numberOf(link.dataset.link);
      link.classList.add("mm-selected");
      say("Beziehung gewählt: Entf löst sie (wiederherstellbar mit Strg+Z).");
      return;
    }
    if (!el) return;
    event.preventDefault();
    const n = info(el);
    if (linkFrom !== null) finishLinking(n);
    else {
      select(n.id);
      remember();
    }
  });

  canvas.addEventListener("dblclick", (event) => {
    const el = event.target instanceof Element ? event.target.closest<SVGGElement>(".mm-node") : null;
    if (!el) return;
    event.preventDefault();
    select(info(el).id);
    rename(info(el));
  });

  canvas.addEventListener(
    "wheel",
    (event) => {
      if (!event.ctrlKey && !event.metaKey) return;
      event.preventDefault();
      event.stopPropagation();
      zoom(event.deltaY < 0 ? STEP : 1 / STEP, event.clientX, event.clientY);
    },
    { passive: false },
  );

  // ---- Tasten ------------------------------------------------------------------------------------------------

  document.addEventListener("keydown", (event) => {
    if (!stage.isConnected || inField(event.target)) return;
    const key = event.key;
    const plain = !event.ctrlKey && !event.metaKey && !event.altKey;
    if ((event.ctrlKey || event.metaKey) && !event.altKey && !event.shiftKey && key.toLowerCase() === "z") {
      event.preventDefault();
      void undoLast();
      return;
    }
    // `code` dazu: Auf manchen Tastaturen ergibt Alt+E bzw. Alt+L ein anderes Zeichen.
    if (event.altKey && !event.ctrlKey && !event.metaKey && (key.toLowerCase() === "e" || event.code === "KeyE")) {
      event.preventDefault();
      create("cloud");
      return;
    }
    if (event.altKey && !event.ctrlKey && !event.metaKey && (key.toLowerCase() === "l" || event.code === "KeyL")) {
      event.preventDefault();
      startLinking();
      return;
    }
    // Alles Weitere nur, wenn die Mindmap den Fokus hat.
    if (!plain || !(event.target instanceof Node) || !canvas.contains(event.target)) return;
    const n = current();
    switch (key) {
      case "Tab":
        // Umschalt+Tab und Tab ohne Auswahl bleiben beim Browser (kein Tastaturkäfig; Esc hebt die Auswahl auf).
        if (event.shiftKey || !n) return;
        event.preventDefault();
        create("child");
        return;
      case "Enter":
        if (!n) return;
        event.preventDefault();
        if (linkFrom !== null) finishLinking(n);
        else create(n.kind === "idea" ? "sibling" : "child");
        return;
      case "F2":
        event.preventDefault();
        if (n) rename(n);
        return;
      case "Delete":
      case "Backspace":
        event.preventDefault();
        remove();
        return;
      case " ":
        event.preventDefault();
        if (n && n.children > 0 && !n.cloud && n.kind !== "work") toggle(n.id);
        return;
      case "ArrowLeft":
      case "ArrowRight":
      case "ArrowUp":
      case "ArrowDown":
        event.preventDefault();
        step(key.slice(5).toLowerCase() as Direction);
        remember();
        return;
      case "+":
      case "=":
        event.preventDefault();
        zoom(STEP);
        return;
      case "-":
        event.preventDefault();
        zoom(1 / STEP);
        return;
      case "0":
        event.preventDefault();
        fit();
        return;
      case "Escape":
        if (linkFrom !== null) {
          stopLinking();
          say("Verbinden abgebrochen.");
        } else {
          select(null);
        }
        return;
      default:
    }
  });

  // ---- Menüband und Werkzeugleiste ---------------------------------------------------------------------------

  document.addEventListener("click", (event) => {
    if (!stage.isConnected || event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey) return;
    const target = event.target instanceof Element ? event.target : null;
    const fold = target?.closest<HTMLAnchorElement>("a[data-mm-zu]");
    if (fold) {
      event.preventDefault();
      closedFromHref(fold.href);
      return;
    }
    const button = target?.closest<HTMLElement>("[data-mm-action]");
    if (!button) return;
    event.preventDefault();
    const n = current();
    switch (button.dataset.mmAction) {
      case "child":
        create("child");
        break;
      case "sibling":
        create(n && n.kind === "idea" ? "sibling" : "child");
        break;
      case "rename":
        if (n) rename(n);
        else say("Umbenennen: zuerst einen Einfall wählen.");
        break;
      case "delete":
        if (n || selectedLink !== null) remove();
        else say("Löschen: zuerst einen Einfall wählen.");
        break;
      case "link":
        startLinking();
        break;
      case "cloud":
        toCloud(n);
        break;
      case "undo":
        void undoLast();
        break;
      case "zoom-in":
        zoom(STEP);
        break;
      case "zoom-out":
        zoom(1 / STEP);
        break;
      case "fit":
        fit();
        break;
      default:
    }
  });

  const toolbar = stage.querySelector<HTMLElement>("[data-mm-toolbar]");
  if (toolbar) {
    const group = document.createElement("span");
    group.className = "mm-zoom";
    for (const [action, label, title] of [
      ["zoom-out", "−", "Verkleinern (-)"],
      ["zoom-in", "+", "Vergrößern (+)"],
      ["fit", "Einpassen", "Ganze Mindmap zeigen (0)"],
    ] as const) {
      const button = document.createElement("button");
      button.type = "button";
      button.className = "btn small";
      button.dataset.mmAction = action;
      button.textContent = label;
      button.title = title;
      group.append(button);
    }
    toolbar.prepend(group);
    const help = document.createElement("p");
    help.className = "mm-help small";
    help.textContent =
      "Klick wählt, Doppelklick oder F2 benennt um. Tab: Unterknoten, Enter: Geschwister, Entf: löschen, " +
      "Leertaste: klappen, Pfeile: wandern, Strg+Z: rückgängig, Alt+L: verbinden, Alt+E: neuer Einfall in der Wolke. " +
      "Ziehen hängt um: auf einen Knoten, zwischen Geschwister oder in die Wolke. Strg+Mausrad, +, − und 0 zoomen.";
    toolbar.after(help);
  }

  // ---- Start -------------------------------------------------------------------------------------------------

  stage.classList.add("mm-ready");
  try {
    const stored = JSON.parse(sessionStorage.getItem(viewKey) ?? "null") as Partial<typeof view> | null;
    if (stored && [stored.s, stored.tx, stored.ty].every((v) => typeof v === "number" && Number.isFinite(v))) {
      view = { s: Math.min(MAX_SCALE, Math.max(MIN_SCALE, stored.s as number)), tx: stored.tx as number, ty: stored.ty as number };
    }
  } catch {
    // Ohne gespeicherten Ausschnitt: eingepasst.
  }
  applyView();
  select(selected, false);
  // Eingeklappt je Werk gemerkt: gilt, wenn die Adresse selbst kein `zu` trägt.
  let stored = "";
  try {
    stored = localStorage.getItem(closedKey) ?? "";
  } catch {
    stored = "";
  }
  if (!new URLSearchParams(location.search).has("zu") && stored !== "") {
    closed = stored.split(",").filter(Boolean);
    void refresh().catch(() => undefined);
  }
});
