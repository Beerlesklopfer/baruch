// SPDX-License-Identifier: AGPL-3.0-or-later
// Block „Satz“ in Einstellungen → Werk (Runde 29c, Besitz satz-oberflaeche): Vorschau ohne Neuladen über RPC
// `work.typeset_preview`, sonst wie die Formularbedienung ohne JavaScript.
//
// Widget „typeset-settings“ am Formular `form.typeset-form` (templates/settings/werk_satz.html):
// - Jede Änderung im Formular stößt nach einer Pause (entprellt, `PREVIEW_DELAY_MS`) die Vorschau an; der Knopf
//   „Vorschau“ setzt sofort, ohne das Formular abzuschicken. Eine neue Anfrage bricht die alte ab (`Newest`,
//   AbortController), eine verspätete Antwort wird verworfen.
// - Die Formularfelder liest `formToSettings` genau wie der Server (`settings::work_typeset::settings_from_form`):
//   "" = aus der Werkvorlage, Dezimalkomma, Ränder alle vier oder keiner, Bundsteg leer = 0. Grenzen prüft der Server
//   (`TypesetSettings::validate`); seine Meldung erscheint im Vorschaubereich.
// - „Speichern“ und „Zurücksetzen“ bleiben normale Formulare (im Popup schickt sie settings.ts ab).
// - Runde 31c: Gruppe „Register und Glossar“ (Glossarseite, Häkchen `index_register` je Register, Spalten, erste
//   Erwähnung) liest `formToSettings` ebenso wie der Server.

import { register } from "./widgets.js";

/// Pause nach der letzten Eingabe, bevor die Vorschau gesetzt wird.
export const PREVIEW_DELAY_MS = 800;

export interface FieldError {
  field: string;
  message: string;
}

export interface FormResult {
  settings: Record<string, unknown>;
  errors: FieldError[];
}

const LABELS: Record<string, string> = {
  chapter_start: "Kapitelanfang",
  part_page: "Teil-Seite",
  page_format: "Seitenformat",
  margins: "Ränder",
  paragraph: "Absatz",
  justify: "Blocksatz",
  hyphenate: "Silbentrennung",
  page_numbers: "Seitenzahlen",
  running_head: "Kolumnentitel",
  footnotes: "Fußnoten",
  widows_orphans: "Schusterjungen/Hurenkinder",
  xref_with_page: "Verweis mit Seitenzahl",
  glossary_page: "Glossarseite",
  glossary_first_mention: "Erste Erwähnung im Glossar auszeichnen",
};

/// Runde 31c: Spalten der Registerseiten bei leerem Feld (wie `DEFAULT_INDEX_COLUMNS` im Server).
export const DEFAULT_INDEX_COLUMNS = 2;

const CHOICES: Record<string, string[]> = {
  chapter_start: ["continuous", "new_page", "right_page"],
  part_side: ["right", "any"],
  page_format: ["a5", "120x190", "135x215", "a4"],
  paragraph: ["indent", "spacing"],
  page_number_position: ["bottom_center", "bottom_outside", "top_outside", "hidden"],
  page_number_start: ["first_page", "first_chapter"],
  running_head: ["off", "work_title", "chapter"],
  footnote_numbering: ["arabic", "symbols", "letters"],
  footnote_restart: ["continuous", "per_chapter"],
  glossary_page: ["before_indexes", "after_indexes"],
};

/// Runde 22: Lagen des Wasserzeichens (wie `DraftPlacement::ALL`).
const DRAFT_PLACEMENTS = ["diagonal", "header", "footer"];

/// Vorgabemaß je Absatzart (wie `Paragraph::ALL` im Datenmodell).
const PARAGRAPH_EM: Record<string, number> = { indent: 1.0, spacing: 0.5 };

const NUMBER = /^[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?$/;

/// Zahl aus dem Formular: leer → undefined, Dezimalkomma erlaubt, keine Zahl → NaN.
export function numberOf(raw: string): number | undefined {
  if (raw === "") return undefined;
  const text = raw.replace(/,/g, ".");
  return NUMBER.test(text) ? Number(text) : NaN;
}

/// Formularfelder → Einstellungen wie `settings_from_form` im Server (ohne die Grenzen, die prüft der Server).
/// Zahlen, die keine sind, melden sich hier als Fehler (sonst ginge `NaN` als `null` an den Server).
export function formToSettings(pairs: [string, string][]): FormResult {
  const get = (name: string): string => (pairs.find(([k]) => k === name)?.[1] ?? "").trim();
  const errors: FieldError[] = [];
  const s: Record<string, unknown> = {};
  const choice = (name: string, field: string): string | undefined => {
    const raw = get(name);
    if (raw === "") return undefined;
    if (!(CHOICES[name] ?? []).includes(raw)) {
      errors.push({ field, message: `${LABELS[field] ?? field}: unbekannte Wahl „${raw}“` });
      return undefined;
    }
    return raw;
  };
  const num = (name: string, field: string): number | undefined => {
    const value = numberOf(get(name));
    if (value !== undefined && Number.isNaN(value)) errors.push({ field, message: `${field}: keine Zahl` });
    return value;
  };
  const onOff = (name: string): boolean | undefined => {
    const raw = get(name);
    if (raw === "") return undefined;
    if (raw === "on" || raw === "off") return raw === "on";
    errors.push({ field: name, message: `${LABELS[name] ?? name}: unbekannte Wahl „${raw}“` });
    return undefined;
  };

  const chapter = choice("chapter_start", "chapter_start");
  if (chapter) s.chapter_start = chapter;
  const side = choice("part_side", "part_page");
  if (side) s.part_page = { side, blank_back: get("part_blank_back") === "1" };
  const format = get("page_format");
  if (format === "custom") {
    s.page_format = {
      kind: "custom",
      width_mm: num("page_width_mm", "page_format.width_mm") ?? NaN,
      height_mm: num("page_height_mm", "page_format.height_mm") ?? NaN,
    };
    if (get("page_width_mm") === "" || get("page_height_mm") === "") {
      errors.push({ field: "page_format", message: "Seitenformat: Breite und Höhe angeben" });
    }
  } else {
    const fixed = choice("page_format", "page_format");
    if (fixed) s.page_format = { kind: fixed };
  }
  const sides = ["margin_inside_mm", "margin_outside_mm", "margin_top_mm", "margin_bottom_mm"].map((n) =>
    num(n, `margins.${n.replace("margin_", "")}`),
  );
  const binding = num("binding_mm", "margins.binding_mm");
  const given = sides.filter((x) => x !== undefined).length;
  if (given === 4) {
    s.margins = {
      inside_mm: sides[0],
      outside_mm: sides[1],
      top_mm: sides[2],
      bottom_mm: sides[3],
      binding_mm: binding ?? 0,
    };
  } else if (given > 0 || binding !== undefined) {
    errors.push({ field: "margins", message: "Ränder: alle vier Ränder angeben oder alle leer" });
  }
  const font = get("font");
  if (font) s.font = font;
  const size = num("font_size_pt", "font_size_pt");
  if (size !== undefined) s.font_size_pt = size;
  const leading = num("leading_em", "leading_em");
  if (leading !== undefined) s.leading_em = leading;
  const paragraph = choice("paragraph", "paragraph");
  if (paragraph) s.paragraph = { kind: paragraph, em: num("paragraph_em", "paragraph.em") ?? PARAGRAPH_EM[paragraph] };
  const justify = onOff("justify");
  if (justify !== undefined) s.justify = justify;
  const hyphenate = onOff("hyphenate");
  if (hyphenate !== undefined) s.hyphenate = hyphenate;
  const lang = get("lang");
  if (lang) s.lang = lang;
  const position = choice("page_number_position", "page_numbers");
  if (position) s.page_numbers = { position, start: choice("page_number_start", "page_numbers") ?? "first_page" };
  const head = choice("running_head", "running_head");
  if (head) s.running_head = head;
  const numbering = choice("footnote_numbering", "footnotes");
  if (numbering) s.footnotes = { numbering, restart: choice("footnote_restart", "footnotes") ?? "continuous" };
  const widows = onOff("widows_orphans");
  if (widows !== undefined) s.widows_orphans = widows;
  const xref = onOff("xref_with_page");
  if (xref !== undefined) s.xref_with_page = xref;
  // Runde 31c: Glossarseite, Registerseiten (Häkchen `index_register` in Satzreihenfolge; Spalten leer = 2, keine Zahl
  // 0–255 = 0, dann meldet der Server die Grenzen) und erste Erwähnung – wie settings_from_form.
  const glossary = choice("glossary_page", "glossary_page");
  if (glossary) s.glossary_page = glossary;
  const registers = pairs.filter(([k]) => k === "index_register").map(([, v]) => v.trim()).filter((v) => v !== "");
  if (registers.length > 0) {
    const raw = get("index_columns");
    const columns = raw === "" ? DEFAULT_INDEX_COLUMNS : /^\+?\d+$/.test(raw) && Number(raw) <= 255 ? Number(raw) : 0;
    s.index_pages = { registers, columns };
  }
  const first = onOff("glossary_first_mention");
  if (first !== undefined) s.glossary_first_mention = first;
  // Runde 22: Entwurf (Häkchen `draft`) – leerer Text = „ENTWURF“, leere Deckkraft = 12, keine Zahl 0–255 = 0 (dann
  // meldet der Server die Grenzen), leere Lage = diagonal; wie settings_from_form.
  if (get("draft") === "1") {
    const raw = get("draft_opacity");
    const opacity = raw === "" ? 12 : /^\+?\d+$/.test(raw) && Number(raw) <= 255 ? Number(raw) : 0;
    const placementRaw = get("draft_placement");
    let placement = "diagonal";
    if (placementRaw !== "") {
      if (DRAFT_PLACEMENTS.includes(placementRaw)) placement = placementRaw;
      else errors.push({ field: "draft", message: `Entwurf: unbekannte Wahl „${placementRaw}“` });
    }
    const draft: Record<string, unknown> = { text: get("draft_text") || "ENTWURF" };
    const note = get("draft_note");
    if (note) draft.note = note;
    draft.date = get("draft_date") === "1";
    draft.opacity_percent = opacity;
    draft.placement = placement;
    draft.cover = get("draft_cover") === "1";
    s.draft = draft;
  }
  return { settings: s, errors };
}

/// Zeitgeber (im Test nachgebaut).
export interface Timers {
  set(run: () => void, ms: number): unknown;
  clear(handle: unknown): void;
}

const browserTimers: Timers = {
  set: (run, ms) => setTimeout(run, ms),
  clear: (handle) => clearTimeout(handle as ReturnType<typeof setTimeout>),
};

/// Entprellt `run`: `trigger` startet die Pause neu, erst nach `ms` ohne weiteren Anstoß läuft `run` einmal;
/// `now` läuft sofort (und verwirft die laufende Pause), `cancel` verwirft sie.
export function debounce(run: () => void, ms: number, timers: Timers = browserTimers) {
  let handle: unknown = null;
  const cancel = () => {
    if (handle !== null) timers.clear(handle);
    handle = null;
  };
  return {
    trigger(): void {
      cancel();
      handle = timers.set(() => {
        handle = null;
        run();
      }, ms);
    },
    now(): void {
      cancel();
      run();
    },
    cancel,
  };
}

/// Nur die neueste Anfrage zählt: `start` bricht die vorige ab.
export class Newest {
  private current: AbortController | null = null;

  start(): AbortController {
    this.current?.abort();
    this.current = new AbortController();
    return this.current;
  }

  isCurrent(controller: AbortController): boolean {
    return this.current === controller;
  }
}

interface PreviewAnswer {
  stand: string;
  pages: number;
  urls: string[];
  errors: number;
  warnings: number;
  diagnostics: { severity: string; message: string }[];
  conflicts: { field: string; label: string; line: number; text: string; message: string }[];
}

let nextId = 1;

/// JSON-RPC mit Abbruch (rpc.ts kennt kein AbortSignal).
async function rpcAbortable<T>(method: string, params: Record<string, unknown>, signal: AbortSignal): Promise<T> {
  const response = await fetch("/rpc", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ jsonrpc: "2.0", id: nextId++, method, params }),
    signal,
  });
  if (!response.ok) throw new Error(`Server antwortet ${response.status}`);
  const answer = await response.json();
  if (answer.error) throw new Error(String(answer.error.message));
  return answer.result as T;
}

function paragraph(text: string, className: string): HTMLParagraphElement {
  const p = document.createElement("p");
  p.className = className;
  p.textContent = text;
  return p;
}

function showPreview(box: HTMLElement, answer: PreviewAnswer): void {
  const parts: Node[] = [];
  let info = `Vorschau mit Mustertext: ${answer.pages} Seiten`;
  if (answer.errors) info += `, ${answer.errors} Fehler`;
  if (answer.warnings) info += `, ${answer.warnings} Warnungen`;
  const head = paragraph(`${info}.`, "small");
  head.dataset.typesetPreviewInfo = "";
  parts.push(head);
  for (const d of answer.diagnostics) parts.push(paragraph(d.message, `typeset-diagnostic small ${d.severity}`));
  for (const c of answer.conflicts) parts.push(paragraph(`${c.label}: ${c.message}`, "typeset-hint small"));
  const pages = document.createElement("div");
  pages.className = "typeset-pages";
  answer.urls.forEach((url, i) => {
    const img = document.createElement("img");
    img.src = url;
    img.alt = `Vorschau Seite ${i + 1}`;
    img.loading = "lazy";
    pages.append(img);
  });
  parts.push(pages);
  box.replaceChildren(...parts);
}

function formPairs(form: HTMLFormElement): [string, string][] {
  const pairs: [string, string][] = [];
  new FormData(form).forEach((value, key) => {
    if (typeof value === "string") pairs.push([key, value]);
  });
  return pairs;
}

if (typeof document !== "undefined") {
  register("typeset-settings", (element) => {
    if (!(element instanceof HTMLFormElement)) return;
    const form = element;
    const box = form.closest("#satz")?.querySelector<HTMLElement>("[data-typeset-preview]");
    if (!box) return;
    const work = Number(form.dataset.work);
    const newest = new Newest();
    const run = async () => {
      const { settings, errors } = formToSettings(formPairs(form));
      if (errors.length > 0) {
        newest.start().abort();
        box.replaceChildren(...errors.map((e) => paragraph(e.message, "typeset-error small")));
        return;
      }
      const controller = newest.start();
      box.setAttribute("aria-busy", "true");
      try {
        const answer = await rpcAbortable<PreviewAnswer>(
          "work.typeset_preview",
          { work: Number.isFinite(work) ? work : undefined, settings },
          controller.signal,
        );
        if (newest.isCurrent(controller)) showPreview(box, answer);
      } catch (error) {
        if (controller.signal.aborted || !newest.isCurrent(controller)) return;
        box.replaceChildren(paragraph(String((error as Error).message ?? error), "typeset-error small"));
      } finally {
        if (newest.isCurrent(controller)) box.removeAttribute("aria-busy");
      }
    };
    const preview = debounce(() => void run(), PREVIEW_DELAY_MS);
    form.addEventListener("input", () => preview.trigger());
    form.addEventListener("change", () => preview.trigger());
    form.querySelector<HTMLButtonElement>("[data-typeset-preview-button]")?.addEventListener("click", (event) => {
      event.preventDefault();
      preview.now();
    });
    form.addEventListener("submit", () => preview.cancel());
  });
}
