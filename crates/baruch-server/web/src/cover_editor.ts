// SPDX-License-Identifier: AGPL-3.0-or-later
// Cover editor (Round 27, task umschlag-widget). Without JavaScript everything goes through the form
// `POST /werke/umschlag/anordnung` on the page `/werke/{id}/umschlag` (fields `b.<kind>.<feld>`, preview by GET).
//
// - Widget „cover-editor“ on the editor container (`data-widget="cover-editor"`); data: JSON from `cover.layout`
//   in the attribute `data-layout` (rendered by the server). The frames of the blocks (`[data-cover-block]`) and the
//   fields (`[name="b.<kind>.<feld>"]`) are supplied by the server; the widget keeps both in the same state.
// - Dragging moves a frame, the handles (`[data-cover-handle=start|end]`) change the width in reading direction
//   (vertically for the spine). While dragging, the frame snaps to the guide lines (trim edge, safety margin,
//   centre; SNAP_MM).
// - Keyboard on a frame: arrow keys STEP_MM (with Shift BIG_STEP_MM), Ctrl+←/→ width, Ctrl+Z undo,
//   Ctrl+S save; „Hell/Dunkel“ toggles the colour of the chosen block.
// - Saving through RPC `cover.layout_set` with all changed blocks (whole blocks), quick choice and resetting
//   through `cover.layout_reset` (afterwards the page reloads).
// - Coordinates as in the server (contract 1.2): mm from the trim edge of the own face; faces in the whole cover
//   back (b, b), spine (b + W, b), front (b + W + S, b).

import { call, RpcError } from "./rpc.js";
import { register } from "./widgets.js";

type Face = "front" | "spine" | "back";

interface Block {
  kind: string;
  x_mm: number;
  y_mm: number;
  width_mm: number;
  align: string;
  size_pt: number;
  leading_em: number;
  color: string;
  font: string | null;
  style: string;
  backdrop: string;
  visible: boolean;
  direction: string | null;
  text: string | null;
  stored?: boolean;
  height_mm?: number;
}

interface Geometry {
  page_width_mm: number;
  page_height_mm: number;
  spine_mm: number;
  bleed_mm: number;
  safety_mm: number;
}

interface Layout {
  work: number;
  stored: boolean;
  blocks: Block[];
  geometry: Geometry;
  warnings: { code: string; block: string | null; message: string }[];
}

/// Step of the arrow keys (mm) and with Shift.
export const STEP_MM = 0.5;
export const BIG_STEP_MM = 5;
/// Snapping to guide lines up to this distance (mm).
export const SNAP_MM = 1.5;
/// Limits as in the store (contract section 2).
const POS_MM: [number, number] = [-50, 1000];
const WIDTH_MM: [number, number] = [1, 1000];

const NUMBER_FIELDS = ["x_mm", "y_mm", "width_mm", "size_pt", "leading_em"] as const;
const SENT_FIELDS = [
  "kind", "x_mm", "y_mm", "width_mm", "align", "size_pt", "leading_em", "color", "font", "style", "backdrop", "visible",
  "direction", "text",
] as const;

function faceOf(kind: string): Face {
  if (kind.startsWith("front_")) return "front";
  if (kind.startsWith("spine_")) return "spine";
  return "back";
}

function origin(g: Geometry, face: Face): [number, number] {
  const b = g.bleed_mm;
  if (face === "back") return [b, b];
  if (face === "spine") return [b + g.page_width_mm, b];
  return [b + g.page_width_mm + g.spine_mm, b];
}

function faceSize(g: Geometry, face: Face): [number, number] {
  return face === "spine" ? [g.spine_mm, g.page_height_mm] : [g.page_width_mm, g.page_height_mm];
}

function wrapSize(g: Geometry): [number, number] {
  return [2 * g.page_width_mm + g.spine_mm + 2 * g.bleed_mm, g.page_height_mm + 2 * g.bleed_mm];
}

/// Round to 1/100 mm (against floating-point remainders while dragging).
function round(value: number): number {
  return Math.round(value * 100) / 100;
}

function clamp(value: number, [low, high]: [number, number]): number {
  return Math.min(high, Math.max(low, value));
}

/// „12,5“ for labels.
function mm(value: number): string {
  return value.toFixed(1).replace(".", ",");
}

/// Extent of the frame on the face (mm): on the spine the text runs rotated, `width_mm` is vertical.
function extent(block: Block): [number, number] {
  const height = block.height_mm ?? 0;
  return faceOf(block.kind) === "spine" ? [height, block.width_mm] : [block.width_mm, height];
}

/// The block as `cover.layout_set` expects it (without `stored` and `height_mm`).
function payload(block: Block): Record<string, unknown> {
  const out: Record<string, unknown> = {};
  for (const field of SENT_FIELDS) out[field] = block[field];
  return out;
}

function copy(block: Block): Block {
  return { ...block };
}

function same(a: Block, b: Block): boolean {
  return JSON.stringify(payload(a)) === JSON.stringify(payload(b));
}

/// Nearest guide line to one of the edges (`edges`), at most SNAP_MM away; shift or `null`.
function snapDelta(edges: number[], targets: number[]): number | null {
  let best: number | null = null;
  for (const edge of edges) {
    for (const target of targets) {
      const delta = target - edge;
      if (Math.abs(delta) <= SNAP_MM && (best === null || Math.abs(delta) < Math.abs(best))) best = delta;
    }
  }
  return best;
}

register("cover-editor", (root) => {
  let layout: Layout;
  try {
    layout = JSON.parse(root.dataset.layout ?? "") as Layout;
  } catch {
    return; // without data the form stays (works without JavaScript)
  }
  if (!layout || !Array.isArray(layout.blocks) || !layout.geometry) return;
  const work = Number(root.dataset.work ?? layout.work);
  const geometry = layout.geometry;
  const [wrapW, wrapH] = wrapSize(geometry);
  const stage = root.querySelector<HTMLElement>("[data-cover-stage]");
  const statusLine = root.querySelector<HTMLElement>("[data-cover-status]");
  const undoButton = root.querySelector<HTMLButtonElement>("[data-cover-undo]");
  const saveButton = root.querySelector<HTMLButtonElement>("[data-cover-save]");
  const toneButton = root.querySelector<HTMLButtonElement>("[data-cover-tone]");
  const image = root.querySelector<HTMLImageElement>("[data-cover-image]");

  const blocks = new Map<string, Block>();
  const saved = new Map<string, Block>();
  for (const block of layout.blocks) {
    blocks.set(block.kind, copy(block));
    saved.set(block.kind, copy(block));
  }
  const frames = new Map<string, HTMLElement>();
  root.querySelectorAll<HTMLElement>("[data-cover-block]").forEach((frame) => {
    const kind = frame.dataset.coverBlock ?? "";
    if (blocks.has(kind)) frames.set(kind, frame);
  });
  const undo: Block[] = [];
  let selected: string | null = null;
  let previewTimer: ReturnType<typeof setTimeout> | null = null;

  const isDirty = (kind: string) => {
    const now = blocks.get(kind);
    const before = saved.get(kind);
    return !!now && !!before && !same(now, before);
  };
  const dirtyKinds = () => [...blocks.keys()].filter(isDirty);

  function say(message: string, error = false): void {
    if (!statusLine) return;
    statusLine.textContent = message;
    statusLine.classList.toggle("error", error);
  }

  function labelOf(kind: string): string {
    return frames.get(kind)?.querySelector(".cover-block-name")?.textContent?.trim() || kind;
  }

  function field(kind: string, name: string): HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement | null {
    return root.querySelector<HTMLInputElement>(`[name="b.${kind}.${name}"]`);
  }

  /// Bring frame and fields of a block up to date.
  function place(kind: string): void {
    const block = blocks.get(kind);
    if (!block) return;
    const frame = frames.get(kind);
    if (frame) {
      const [ox, oy] = origin(geometry, faceOf(kind));
      const [w, h] = extent(block);
      frame.style.left = `${((ox + block.x_mm) / wrapW) * 100}%`;
      frame.style.top = `${((oy + block.y_mm) / wrapH) * 100}%`;
      frame.style.width = `${(w / wrapW) * 100}%`;
      frame.style.height = `${(h / wrapH) * 100}%`;
      frame.dataset.x = String(block.x_mm);
      frame.dataset.y = String(block.y_mm);
      frame.dataset.w = String(block.width_mm);
      frame.dataset.color = block.color;
      frame.classList.toggle("is-hidden", !block.visible);
      frame.classList.toggle("is-dirty", isDirty(kind));
      const hidden = block.visible ? "" : ", verborgen";
      frame.setAttribute(
        "aria-label",
        `${labelOf(kind)}: x ${mm(block.x_mm)} mm, y ${mm(block.y_mm)} mm, Breite ${mm(block.width_mm)} mm${hidden}`,
      );
    }
    for (const name of NUMBER_FIELDS) {
      const input = field(kind, name);
      if (input && Number(input.value.replace(",", ".")) !== block[name]) input.value = String(block[name]);
    }
    const color = field(kind, "color");
    if (color && color.value !== block.color) color.value = block.color;
  }

  function refreshButtons(): void {
    const dirty = dirtyKinds().length;
    if (undoButton) undoButton.disabled = undo.length === 0;
    if (saveButton) saveButton.disabled = dirty === 0;
    root.dataset.dirty = dirty > 0 ? "1" : "";
    if (toneButton) {
      const block = selected ? blocks.get(selected) : undefined;
      toneButton.setAttribute("aria-pressed", block?.color === "dark" ? "true" : "false");
      toneButton.disabled = !block;
    }
  }

  /// Preview of the typesetting (Typst SVG) for the changed blocks, delayed.
  function schedulePreview(): void {
    if (!image) return;
    if (previewTimer !== null) clearTimeout(previewTimer);
    previewTimer = setTimeout(() => {
      previewTimer = null;
      const query = new URLSearchParams();
      for (const kind of dirtyKinds()) {
        const block = blocks.get(kind)!;
        for (const name of SENT_FIELDS) {
          if (name === "kind") continue;
          const value = block[name];
          if (value === null) continue;
          query.append(`b.${kind}.${name}`, typeof value === "boolean" ? (value ? "1" : "0") : String(value));
        }
      }
      query.set("t", String(Date.now()));
      image.src = `/werke/${work}/umschlag.svg?${query.toString()}`;
    }, 700);
  }

  /// A change to a block: remember the state before (undo), apply, show.
  function change(kind: string, apply: (block: Block) => void, remember = true): void {
    const block = blocks.get(kind);
    if (!block) return;
    const before = copy(block);
    apply(block);
    block.x_mm = round(clamp(block.x_mm, POS_MM));
    block.y_mm = round(clamp(block.y_mm, POS_MM));
    block.width_mm = round(clamp(block.width_mm, WIDTH_MM));
    if (same(before, block)) return;
    if (remember) undo.push(before);
    place(kind);
    refreshButtons();
    schedulePreview();
  }

  function undoLast(): void {
    const before = undo.pop();
    if (!before) {
      say("Nichts rückgängig zu machen.");
      return;
    }
    blocks.set(before.kind, copy(before));
    place(before.kind);
    refreshButtons();
    schedulePreview();
    say(`Rückgängig: ${labelOf(before.kind)}.`);
  }

  function select(kind: string | null): void {
    selected = kind;
    frames.forEach((frame, k) => frame.classList.toggle("is-selected", k === kind));
    if (kind) {
      const details = root.querySelector<HTMLDetailsElement>(`[data-cover-fields="${kind}"]`);
      if (details) details.open = true;
    }
    refreshButtons();
  }

  async function save(): Promise<void> {
    const changed = dirtyKinds();
    if (changed.length === 0) {
      say("Nichts zu speichern.");
      return;
    }
    if (saveButton) saveButton.disabled = true;
    say("Speichert …");
    try {
      const answer = await call<{ layout?: Layout; mirror?: string }>("cover.layout_set", {
        work,
        blocks: changed.map((kind) => payload(blocks.get(kind)!)),
      });
      const fresh = answer?.layout?.blocks ?? [];
      for (const kind of changed) {
        const current = blocks.get(kind)!;
        const stored = fresh.find((b) => b.kind === kind);
        if (stored) {
          current.height_mm = stored.height_mm ?? current.height_mm;
          current.stored = true;
        }
        saved.set(kind, copy(current));
        place(kind);
      }
      const warnings = answer?.layout?.warnings?.length ?? 0;
      say(
        `Gespeichert (${changed.length} ${changed.length === 1 ? "Block" : "Blöcke"}).` +
          (warnings > 0 ? ` Druckprüfung: ${warnings} Hinweis${warnings === 1 ? "" : "e"} – Seite neu laden zeigt sie.` : "") +
          (answer?.mirror ? ` Spiegel: ${answer.mirror}` : ""),
      );
      if (image) image.src = `/werke/${work}/umschlag.svg?t=${Date.now()}`;
    } catch (error) {
      say(error instanceof RpcError || error instanceof Error ? error.message : String(error), true);
    }
    refreshButtons();
  }

  async function reset(params: Record<string, unknown>, what: string): Promise<void> {
    if (dirtyKinds().length > 0 && !confirm("Ungespeicherte Änderungen der Anordnung verwerfen?")) return;
    say(`${what} …`);
    try {
      await call("cover.layout_reset", { work, ...params });
      saved.clear();
      blocks.forEach((block, kind) => saved.set(kind, copy(block)));
      window.location.assign(`/werke/${work}/umschlag?ok=anordnung#anordnung`);
    } catch (error) {
      say(error instanceof Error ? error.message : String(error), true);
    }
  }

  // ---- Dragging and changing the width ----------------------------------------------------------

  interface Drag {
    kind: string;
    mode: "move" | "start" | "end";
    x: number;
    y: number;
    start: Block;
    scale: number;
    pointer: number;
  }
  let drag: Drag | null = null;

  /// Targets on the face (mm): trim edge, safety margin, centre.
  function targets(face: Face): [number[], number[]] {
    const [fw, fh] = faceSize(geometry, face);
    const s = geometry.safety_mm;
    return [[0, s, fw / 2, fw - s, fw], [0, s, fh / 2, fh - s, fh]];
  }

  function dragTo(event: PointerEvent): void {
    if (!drag) return;
    const d = drag;
    const dx = (event.clientX - d.x) * d.scale;
    const dy = (event.clientY - d.y) * d.scale;
    const face = faceOf(d.kind);
    const spine = face === "spine";
    const [tx, ty] = targets(face);
    let snapped = "";
    change(d.kind, (block) => {
      Object.assign(block, copy(d.start));
      const [w, h] = extent(d.start);
      if (d.mode === "move") {
        block.x_mm = d.start.x_mm + dx;
        block.y_mm = d.start.y_mm + dy;
        const sx = snapDelta([block.x_mm, block.x_mm + w / 2, block.x_mm + w], tx);
        const sy = snapDelta([block.y_mm, block.y_mm + h / 2, block.y_mm + h], ty);
        if (sx !== null) {
          block.x_mm += sx;
          snapped += "x";
        }
        if (sy !== null) {
          block.y_mm += sy;
          snapped += "y";
        }
      } else {
        const along = spine ? dy : dx;
        const lines = spine ? ty : tx;
        if (d.mode === "end") {
          const startPos = spine ? d.start.y_mm : d.start.x_mm;
          let edge = startPos + d.start.width_mm + along;
          const s = snapDelta([edge], lines);
          if (s !== null) {
            edge += s;
            snapped += spine ? "y" : "x";
          }
          block.width_mm = Math.max(WIDTH_MM[0], edge - startPos);
        } else {
          const end = (spine ? d.start.y_mm : d.start.x_mm) + d.start.width_mm;
          let edge = (spine ? d.start.y_mm : d.start.x_mm) + along;
          const s = snapDelta([edge], lines);
          if (s !== null) {
            edge += s;
            snapped += spine ? "y" : "x";
          }
          edge = Math.min(edge, end - WIDTH_MM[0]);
          if (spine) block.y_mm = edge;
          else block.x_mm = edge;
          block.width_mm = end - edge;
        }
      }
    }, false);
    const frame = frames.get(d.kind);
    if (frame) frame.dataset.snapped = snapped;
  }

  frames.forEach((frame, kind) => {
    frame.addEventListener("pointerdown", (event: PointerEvent) => {
      if (event.button !== undefined && event.button !== 0) return;
      const handle = (event.target as HTMLElement | null)?.closest?.("[data-cover-handle]") as HTMLElement | null;
      const width = stage?.getBoundingClientRect().width ?? 0;
      select(kind);
      frame.focus();
      if (width <= 0) return;
      event.preventDefault();
      const mode = handle?.dataset.coverHandle === "start" ? "start" : handle ? "end" : "move";
      drag = { kind, mode, x: event.clientX, y: event.clientY, start: copy(blocks.get(kind)!), scale: wrapW / width,
        pointer: event.pointerId };
      try {
        frame.setPointerCapture?.(event.pointerId);
      } catch {
        // it also works without pointer capture (events then only arrive above the frame)
      }
    });
    frame.addEventListener("pointermove", (event: PointerEvent) => {
      if (drag && drag.kind === kind) dragTo(event);
    });
    const finish = (event: PointerEvent) => {
      if (!drag || drag.kind !== kind) return;
      dragTo(event);
      const start = drag.start;
      drag = null;
      if (!same(start, blocks.get(kind)!)) {
        undo.push(start);
        refreshButtons();
        const block = blocks.get(kind)!;
        say(`${labelOf(kind)}: x ${mm(block.x_mm)} mm, y ${mm(block.y_mm)} mm, Breite ${mm(block.width_mm)} mm.`);
      }
    };
    frame.addEventListener("pointerup", finish);
    frame.addEventListener("pointercancel", finish);
    frame.addEventListener("focus", () => select(kind));
  });

  // ---- Keyboard -----------------------------------------------------------------------------------

  root.addEventListener("keydown", (event: KeyboardEvent) => {
    const target = event.target as HTMLElement | null;
    const typing = !!target?.closest?.("input, textarea, select");
    const ctrl = event.ctrlKey || event.metaKey;
    if (ctrl && !typing && (event.key === "z" || event.key === "Z") && !event.shiftKey) {
      event.preventDefault();
      undoLast();
      return;
    }
    if (ctrl && (event.key === "s" || event.key === "S")) {
      event.preventDefault();
      void save();
      return;
    }
    const frame = target?.closest?.("[data-cover-block]") as HTMLElement | null;
    const kind = frame?.dataset.coverBlock;
    if (!kind || !blocks.has(kind)) return;
    const step = event.shiftKey ? BIG_STEP_MM : STEP_MM;
    const spine = faceOf(kind) === "spine";
    let apply: ((block: Block) => void) | null = null;
    switch (event.key) {
      case "ArrowLeft":
        apply = ctrl ? (b) => (b.width_mm -= step) : (b) => (b.x_mm -= step);
        break;
      case "ArrowRight":
        apply = ctrl ? (b) => (b.width_mm += step) : (b) => (b.x_mm += step);
        break;
      case "ArrowUp":
        apply = ctrl ? (spine ? (b) => (b.width_mm -= step) : null) : (b) => (b.y_mm -= step);
        break;
      case "ArrowDown":
        apply = ctrl ? (spine ? (b) => (b.width_mm += step) : null) : (b) => (b.y_mm += step);
        break;
      case "Escape":
        frame?.blur();
        select(null);
        return;
    }
    if (!apply) return;
    event.preventDefault();
    change(kind, apply);
    const block = blocks.get(kind)!;
    say(`${labelOf(kind)}: x ${mm(block.x_mm)} mm, y ${mm(block.y_mm)} mm, Breite ${mm(block.width_mm)} mm.`);
  });

  // ---- Fields of the sidebar --------------------------------------------------------------------

  const onField = (event: Event) => {
    const input = event.target as HTMLInputElement | null;
    const name = input?.getAttribute?.("name") ?? "";
    const match = /^b\.([a-z_]+)\.([a-z_]+)$/.exec(name);
    if (!input || !match || !blocks.has(match[1])) return;
    const [, kind, key] = match;
    const raw = input.value.trim();
    change(kind, (block) => {
      if ((NUMBER_FIELDS as readonly string[]).includes(key)) {
        const value = Number(raw.replace(",", "."));
        if (raw === "" || !Number.isFinite(value)) return;
        if (key === "size_pt" || key === "leading_em") {
          // Carry the height along roughly until the typesetting measures it (saving or preview).
          const old = block.size_pt * (1 + block.leading_em);
          (block as unknown as Record<string, number>)[key] = value;
          const now = block.size_pt * (1 + block.leading_em);
          if (old > 0 && block.height_mm !== undefined) block.height_mm = (block.height_mm * now) / old;
        } else {
          (block as unknown as Record<string, number>)[key] = value;
        }
      } else if (key === "visible") {
        block.visible = raw === "1";
      } else if (key === "font") {
        block.font = raw === "" ? null : raw;
      } else if (key === "text") {
        block.text = input.value;
      } else if (key === "direction") {
        block.direction = raw === "" ? null : raw;
      } else if (key === "align" || key === "style" || key === "backdrop" || key === "color") {
        (block as unknown as Record<string, string>)[key] = key === "color" ? raw.toLowerCase() : raw;
      }
    });
  };
  root.addEventListener("change", onField);

  // ---- Buttons -----------------------------------------------------------------------------------

  undoButton?.addEventListener("click", (event) => {
    event.preventDefault();
    undoLast();
  });
  saveButton?.addEventListener("click", (event) => {
    event.preventDefault();
    void save();
  });
  toneButton?.addEventListener("click", (event) => {
    event.preventDefault();
    if (!selected) {
      say("Erst einen Block wählen.");
      return;
    }
    const kind = selected;
    change(kind, (block) => (block.color = block.color === "light" ? "dark" : "light"));
    say(`${labelOf(kind)}: Farbe ${blocks.get(kind)!.color === "dark" ? "dunkel" : "hell"}.`);
  });
  root.querySelectorAll<HTMLButtonElement>("[data-cover-quick]").forEach((button) => {
    button.addEventListener("click", (event) => {
      event.preventDefault();
      void reset({ faces: ["front"], area: button.dataset.coverQuick }, `Schnellwahl ${button.textContent?.trim() ?? ""}`);
    });
  });
  root.querySelector<HTMLButtonElement>("[data-cover-reset]")?.addEventListener("click", (event) => {
    event.preventDefault();
    const faces = Array.from(root.querySelectorAll<HTMLInputElement>("[data-cover-reset-form] input[name=faces]"))
      .filter((box) => box.checked)
      .map((box) => box.value);
    const what = faces.length === 0 ? "die ganze Anordnung" : "die gewählten Flächen";
    if (!confirm(`Anordnung zurücksetzen (${what})? Die Vorgaben gelten dann wieder.`)) return;
    void reset(faces.length === 0 ? {} : { faces }, "Zurücksetzen");
  });
  window.addEventListener("beforeunload", (event: BeforeUnloadEvent) => {
    if (dirtyKinds().length === 0) return;
    event.preventDefault();
    event.returnValue = "";
  });

  root.classList.add("cover-editor-ready");
  blocks.forEach((_, kind) => place(kind));
  refreshButtons();
});
