// SPDX-License-Identifier: AGPL-3.0-or-later
// Texts and formats of the user interface for the widgets (Round 39a, interface R39a-S1, contract §2.6).
//
// The server puts the catalog of the page language into the page head (`<script type="application/json"
// id="baruch-i18n">`, `i18n::widget_catalog_json`): `{"lang": "de", "messages": {"widget.…": "…"}}`. This module
// reads it once. The language never comes from the browser (`navigator`): the server has decided it (cookie, own
// setting, Accept-Language). Without the element (node tests, pages without a catalog) `t` returns the key and the
// formats are English.
//
// Widgets import `{ t } from "./i18n.js"`; keys are `widget.<module>.<name>`, placeables only `{ $name }`.

type Lang = "en" | "de";

interface Catalog {
  lang: Lang;
  messages: Record<string, string>;
}

let catalog: Catalog | null | undefined;

function load(): Catalog | null {
  if (catalog !== undefined) return catalog;
  catalog = null;
  try {
    const element = typeof document === "undefined" ? null : document.getElementById("baruch-i18n");
    const parsed: unknown = element?.textContent ? JSON.parse(element.textContent) : null;
    if (parsed && typeof parsed === "object") {
      const { lang, messages } = parsed as { lang?: unknown; messages?: unknown };
      if ((lang === "en" || lang === "de") && messages && typeof messages === "object") {
        catalog = { lang, messages: messages as Record<string, string> };
      }
    }
  } catch {
    catalog = null;
  }
  return catalog;
}

/** Text of `key` with `{ $name }` replaced by `args[name]`; an unknown key returns the key itself. */
export function t(key: string, args?: Record<string, string | number>): string {
  const text = load()?.messages[key];
  if (typeof text !== "string") return key;
  if (!args) return text;
  return text.replace(/\{ \$([A-Za-z0-9_-]+) \}/g, (whole: string, name: string) =>
    Object.prototype.hasOwnProperty.call(args, name) ? String(args[name]) : whole,
  );
}

/** Language of the page as the server decided it. */
export function uiLang(): Lang {
  return load()?.lang ?? "en";
}

/** Number with `decimals` places and thousands separators, as `i18n::number`: de `1.234,5`, en `1,234.5`. */
export function fmtNumber(value: number, decimals = 0): string {
  if (!Number.isFinite(value)) return String(value);
  const [group, point] = uiLang() === "de" ? [".", ","] : [",", "."];
  const plain = Math.abs(value).toFixed(decimals);
  const [whole, fraction = ""] = plain.split(".");
  let out = "";
  for (let i = 0; i < whole.length; i++) {
    if (i > 0 && (whole.length - i) % 3 === 0) out += group;
    out += whole[i];
  }
  if (fraction) out += point + fraction;
  const zero = /^[0.]*$/.test(plain);
  return value < 0 && !zero ? `-${out}` : out;
}

/** Stored UTC stamp (`2026-11-07T17:53:42.451Z`, also without `Z` or with a space) → date in local time. */
function parseStamp(iso: string): Date | null {
  const text = iso.trim().replace(" ", "T");
  if (!/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}/.test(text)) return null;
  const zoned = /(Z|[+-]\d{2}:?\d{2})$/.test(text) ? text : `${text}Z`;
  const date = new Date(zoned);
  return Number.isNaN(date.getTime()) ? null : date;
}

function pad(value: number): string {
  return String(value).padStart(2, "0");
}

function day(date: Date): string {
  const [y, m, d] = [String(date.getFullYear()), pad(date.getMonth() + 1), pad(date.getDate())];
  return uiLang() === "de" ? `${d}.${m}.${y}` : `${y}-${m}-${d}`;
}

/** Point in time to the minute, as `localtime::ui_minutes`: de `07.11.2026 18:53`, en `2026-11-07 18:53`. */
export function fmtMinutes(iso: string): string {
  const date = parseStamp(iso);
  return date ? `${day(date)} ${pad(date.getHours())}:${pad(date.getMinutes())}` : iso;
}

/** Day, as `localtime::ui_day`: de `07.11.2026`, en `2026-11-07`. */
export function fmtDay(iso: string): string {
  const date = parseStamp(iso);
  return date ? day(date) : iso;
}
