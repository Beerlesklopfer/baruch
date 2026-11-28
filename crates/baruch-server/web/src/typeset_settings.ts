// SPDX-License-Identifier: AGPL-3.0-or-later
// Block „Satz“ in Einstellungen → Werk (Round 29c, ownership satz-oberflaeche): preview without reloading through RPC
// `work.typeset_preview`, otherwise like operating the form without JavaScript.
//
// Widget „typeset-settings“ on the form `form.typeset-form` (templates/settings/werk_satz.html):
// - Every change in the form starts the preview after a pause (debounced, `PREVIEW_DELAY_MS`); the button
//   „Vorschau“ typesets at once without submitting the form. A new request aborts the old one (`Newest`,
//   AbortController), a late answer is discarded.
// - `formToSettings` reads the form fields exactly like the server (`settings::work_typeset::settings_from_form`):
//   "" = from the work template, decimal comma, margins all four or none, gutter empty = 0. The server checks the
//   limits (`TypesetSettings::validate`); its message appears in the preview area.
// - „Speichern“ and „Zurücksetzen“ stay normal forms (in the popup settings.ts submits them).
// - Round 31c: group „Register und Glossar“ (glossary page, checkboxes `index_register` per index, columns, first
//   mention) is read by `formToSettings` just like the server.

import { t } from "./i18n.js";
import { register } from "./widgets.js";

/// Pause after the last input before the preview is typeset.
export const PREVIEW_DELAY_MS = 800;

export interface FieldError {
  field: string;
  message: string;
}

export interface FormResult {
  settings: Record<string, unknown>;
  errors: FieldError[];
}

/// Catalog keys of the field names in messages.
const LABELS: Record<string, string> = {
  chapter_start: "widget.typeset_settings.label_chapter_start",
  part_page: "widget.typeset_settings.label_part_page",
  page_format: "widget.typeset_settings.label_page_format",
  margins: "widget.typeset_settings.label_margins",
  paragraph: "widget.typeset_settings.label_paragraph",
  justify: "widget.typeset_settings.label_justify",
  hyphenate: "widget.typeset_settings.label_hyphenate",
  page_numbers: "widget.typeset_settings.label_page_numbers",
  running_head: "widget.typeset_settings.label_running_head",
  footnotes: "widget.typeset_settings.label_footnotes",
  widows_orphans: "widget.typeset_settings.label_widows_orphans",
  xref_with_page: "widget.typeset_settings.label_xref_with_page",
  glossary_page: "widget.typeset_settings.label_glossary_page",
  glossary_first_mention: "widget.typeset_settings.label_glossary_first_mention",
};

/// Name of a field for messages; unknown fields by their name.
function labelOf(field: string): string {
  return field in LABELS ? t(LABELS[field]) : field;
}

/// Round 31c: columns of the index pages with an empty field (like `DEFAULT_INDEX_COLUMNS` in the server).
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

/// Round 22: placements of the watermark (like `DraftPlacement::ALL`).
const DRAFT_PLACEMENTS = ["diagonal", "header", "footer"];

/// Default measure per paragraph kind (like `Paragraph::ALL` in the data model).
const PARAGRAPH_EM: Record<string, number> = { indent: 1.0, spacing: 0.5 };

const NUMBER = /^[+-]?(\d+\.?\d*|\.\d+)([eE][+-]?\d+)?$/;

/// Number from the form: empty → undefined, decimal comma allowed, not a number → NaN.
export function numberOf(raw: string): number | undefined {
  if (raw === "") return undefined;
  const text = raw.replace(/,/g, ".");
  return NUMBER.test(text) ? Number(text) : NaN;
}

/// Form fields → settings like `settings_from_form` in the server (without the limits, the server checks those).
/// Numbers that are none report here as errors (otherwise `NaN` would go to the server as `null`).
export function formToSettings(pairs: [string, string][]): FormResult {
  const get = (name: string): string => (pairs.find(([k]) => k === name)?.[1] ?? "").trim();
  const errors: FieldError[] = [];
  const s: Record<string, unknown> = {};
  const choice = (name: string, field: string): string | undefined => {
    const raw = get(name);
    if (raw === "") return undefined;
    if (!(CHOICES[name] ?? []).includes(raw)) {
      errors.push({ field, message: t("widget.typeset_settings.unknown_choice", { label: labelOf(field), value: raw }) });
      return undefined;
    }
    return raw;
  };
  const num = (name: string, field: string): number | undefined => {
    const value = numberOf(get(name));
    if (value !== undefined && Number.isNaN(value)) errors.push({ field, message: t("widget.typeset_settings.not_a_number", { field }) });
    return value;
  };
  const onOff = (name: string): boolean | undefined => {
    const raw = get(name);
    if (raw === "") return undefined;
    if (raw === "on" || raw === "off") return raw === "on";
    errors.push({ field: name, message: t("widget.typeset_settings.unknown_choice", { label: labelOf(name), value: raw }) });
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
      errors.push({ field: "page_format", message: t("widget.typeset_settings.page_format_both") });
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
    errors.push({ field: "margins", message: t("widget.typeset_settings.margins_all") });
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
  // Round 31c: glossary page, index pages (checkboxes `index_register` in typesetting order; columns empty = 2, not a
  // number 0–255 = 0, then the server reports the limits) and first mention – like settings_from_form.
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
  // Round 22: draft (checkbox `draft`) – empty text = „ENTWURF“, empty opacity = 12, not a number 0–255 = 0 (then
  // the server reports the limits), empty placement = diagonal; like settings_from_form.
  if (get("draft") === "1") {
    const raw = get("draft_opacity");
    const opacity = raw === "" ? 12 : /^\+?\d+$/.test(raw) && Number(raw) <= 255 ? Number(raw) : 0;
    const placementRaw = get("draft_placement");
    let placement = "diagonal";
    if (placementRaw !== "") {
      if (DRAFT_PLACEMENTS.includes(placementRaw)) placement = placementRaw;
      else errors.push({ field: "draft", message: t("widget.typeset_settings.draft_unknown", { value: placementRaw }) });
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

/// Timer (rebuilt in the test).
export interface Timers {
  set(run: () => void, ms: number): unknown;
  clear(handle: unknown): void;
}

const browserTimers: Timers = {
  set: (run, ms) => setTimeout(run, ms),
  clear: (handle) => clearTimeout(handle as ReturnType<typeof setTimeout>),
};

/// Debounces `run`: `trigger` restarts the pause, only after `ms` without a further trigger `run` runs once;
/// `now` runs at once (and discards the running pause), `cancel` discards it.
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

/// Only the newest request counts: `start` aborts the previous one.
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

/// JSON-RPC with abort (rpc.ts knows no AbortSignal).
async function rpcAbortable<T>(method: string, params: Record<string, unknown>, signal: AbortSignal): Promise<T> {
  const response = await fetch("/rpc", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ jsonrpc: "2.0", id: nextId++, method, params }),
    signal,
  });
  if (!response.ok) throw new Error(t("widget.typeset_settings.server_status", { status: response.status }));
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
  // Singular and plural are two keys each (widget texts have no selectors).
  const pagesKey =
    answer.pages === 1 ? "widget.typeset_settings.preview_pages_one" : "widget.typeset_settings.preview_pages_other";
  const errorsKey =
    answer.errors === 1 ? "widget.typeset_settings.preview_errors_one" : "widget.typeset_settings.preview_errors_other";
  const warningsKey =
    answer.warnings === 1 ? "widget.typeset_settings.preview_warnings_one" : "widget.typeset_settings.preview_warnings_other";
  let info = t(pagesKey, { count: answer.pages });
  if (answer.errors) info += `, ${t(errorsKey, { count: answer.errors })}`;
  if (answer.warnings) info += `, ${t(warningsKey, { count: answer.warnings })}`;
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
    img.alt = t("widget.typeset_settings.preview_page", { page: i + 1 });
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
