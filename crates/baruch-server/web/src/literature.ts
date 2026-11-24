// SPDX-License-Identifier: AGPL-3.0-or-later
// Literatur mit JavaScript. Ohne JavaScript geht alles über die Seiten /literatur und /quelle und
// das Formular an POST /literatur/act.
//
// - Widget „literature“ (<dialog> in der Kopfzeile): Die Schaltfläche „Literatur“ (Link auf
//   /literatur) und Alt+L öffnen das Popup. Inhalt über `literature.render`; die Suche bleibt über
//   Schließen und Öffnen erhalten und steht als `lq` in der Adresse. Ein Werk-Treffer schließt das
//   Popup und öffnet den Abschnitt im Arbeitsbereich. Der Umschalter „Suche nach“ (Runde 12, Radio
//   `lmode` = fts | rag) geht mit an `literature.render` und steht als `lmode=rag` in der Adresse;
//   „Dieselbe Suche im Wortlaut (FTS)“ lädt im Popup die Wortlaut-Suche.
// - Fundstelle: Steht `#fundstelle` in der Adresse (Werk-Treffer), wird das erste Suchwort im
//   Textfeld des Editors markiert und angesprungen. Runde 18c: Ist das Satzbild offen (Werk-Treffer aus dem
//   Popup), markiert die CSS Custom Highlight API die Fundstelle im Satzbild (ohne DOM-Änderung); ohne die API
//   oder ohne Treffer im Satzbild bleibt die Markierung im (verborgenen) Textfeld wie bisher.
// - Widget „source“ (Dokumentansicht /quelle): Kontextmenü auf markiertem Text mit Hervorheben,
//   Zitieren, An Persona geben (Runde 15: RPC `literature.to_persona` mit der Persona und Frage aus dem Formular,
//   danach zum Abschnitt) und Ähnliches suchen. Text und Fundstelle (Zitat, Stelle, Zeichenbereich) gehen an
//   dieselben Wege wie das Formular. Runde 15: auch im Volltext einer verknüpften Quelle – Anfang und Ende
//   zählen dort ab `data-offset` des Teils (wie die Zahlenfelder ohne JavaScript).
// - Abschnitt: Zitieren nennt ihn über den festen Schlüssel `key` (`data-key` der Dokumentansicht),
//   nie über die Stelle; die Stelle `node` in Adressen ist nur Anzeige.

import { call } from "./rpc.js";
import { mount, register } from "./widgets.js";

/// Kleinbuchstaben ohne diakritische Zeichen (wie die Suche: „langmutig“ trifft „langmütig“).
function fold(text: string): string {
  return text.normalize("NFD").replace(/\p{Diacritic}/gu, "").toLowerCase();
}

function toast(message: string): void {
  const element = document.createElement("div");
  element.className = "toast";
  element.setAttribute("role", "alert");
  element.textContent = message;
  document.body.append(element);
  setTimeout(() => element.remove(), 6000);
}

function errorText(error: unknown): string {
  return String((error as Error)?.message ?? error);
}

/// Feld aus der aktuellen Adresse als Zahl.
function numberParam(name: string): number | undefined {
  const value = new URL(window.location.href).searchParams.get(name);
  return value === null || value === "" || Number.isNaN(Number(value)) ? undefined : Number(value);
}

/// Fester Schlüssel des offenen Abschnitts: aus der Adresse, sonst aus dem Editor-Formular.
function keyParam(): number | undefined {
  const fromUrl = numberParam("key");
  if (fromUrl !== undefined) return fromUrl;
  const field = document.querySelector<HTMLInputElement>("form.editor-form input[name=key]");
  return field && field.value.trim() !== "" && !Number.isNaN(Number(field.value)) ? Number(field.value) : undefined;
}

function lqParam(): string | undefined {
  const lq = new URL(window.location.href).searchParams.get("lq");
  return lq === null || lq.trim() === "" ? undefined : lq;
}

/// Art der Suche (Runde 12): `rag` = Bedeutung, sonst Wortlaut (`fts`, Vorgabe, nicht in der Adresse).
type SearchMode = "fts" | "rag";

function modeOf(value: string | null | undefined): SearchMode {
  return value === "rag" ? "rag" : "fts";
}

function lmodeParam(): SearchMode {
  return modeOf(new URL(window.location.href).searchParams.get("lmode"));
}

// ---- Fundstelle im Editor ----------------------------------------------------------------

/// Erstes Vorkommen von `word` in `text`, ohne Rücksicht auf Groß-/Kleinschreibung und Akzente.
/// Ergebnis als [Anfang, Ende) in UTF-16-Einheiten von `text` (wie `setSelectionRange`).
export function findFolded(text: string, word: string): [number, number] | null {
  const needle = fold(word);
  if (needle === "") return null;
  let folded = "";
  const origin: number[] = [];
  let i = 0;
  for (const ch of text) {
    const f = fold(ch);
    for (let k = 0; k < f.length; k++) origin.push(i);
    folded += f;
    i += ch.length;
  }
  origin.push(i);
  const at = folded.indexOf(needle);
  if (at < 0) return null;
  const last = origin[at + needle.length - 1];
  const end = last + (text.codePointAt(last)! > 0xffff ? 2 : 1);
  return [origin[at], end];
}

/// Kann der Browser Bereiche hervorheben (CSS Custom Highlight API)?
function highlightApi(): boolean {
  return typeof CSS !== "undefined" && "highlights" in CSS && typeof (window as { Highlight?: unknown }).Highlight === "function";
}

const HIGHLIGHT = "literatur-fundstelle";

/// Fundstelle im offenen Satzbild markieren (Runde 18c): über die CSS Custom Highlight API (`::highlight(
/// literatur-fundstelle)`, static/sheet.css) – nur ein Range-Objekt, das DOM des Satzbilds bleibt unverändert (es
/// wird gespeichert). Gesucht wird im Text der Blöcke, ohne die eingesetzten Vorschläge (`.pv`). `true`, wenn eins
/// der Suchwörter gefunden und markiert ist.
function markInSheet(sheet: HTMLElement, words: string[]): boolean {
  if (!highlightApi()) return false;
  const nodes: { node: Text; start: number }[] = [];
  let whole = "";
  const walker = document.createTreeWalker(sheet, NodeFilter.SHOW_TEXT, {
    acceptNode: (node) => (node.parentElement?.closest(".pv") ? NodeFilter.FILTER_REJECT : NodeFilter.FILTER_ACCEPT),
  });
  for (let node = walker.nextNode(); node; node = walker.nextNode()) {
    const text = node as Text;
    nodes.push({ node: text, start: whole.length });
    whole += text.data;
  }
  const locate = (offset: number, end: boolean): [Text, number] | null => {
    for (const { node, start } of nodes) {
      const stop = start + node.data.length;
      if (end ? offset <= stop : offset < stop) return [node, offset - start];
    }
    return null;
  };
  const registry = (CSS as unknown as { highlights: Map<string, unknown> }).highlights;
  for (const word of words) {
    const found = findFolded(whole, word.replace(/[^\p{L}\p{N}]/gu, ""));
    if (!found) continue;
    const a = locate(found[0], false);
    const b = locate(found[1], true);
    if (!a || !b) continue;
    const range = document.createRange();
    range.setStart(a[0], a[1]);
    range.setEnd(b[0], b[1]);
    const Ctor = (window as unknown as { Highlight: new (...ranges: Range[]) => unknown }).Highlight;
    registry.set(HIGHLIGHT, new Ctor(range));
    a[0].parentElement?.scrollIntoView({ block: "center" });
    // Tippen im Satzbild nimmt die Markierung weg (die Stelle verschiebt sich sonst unter ihr).
    sheet.addEventListener("input", () => registry.delete(HIGHLIGHT), { once: true });
    return true;
  }
  return false;
}

/// Markiert die Fundstelle der Suche `lq` im Editor und rollt dorthin: bei offenem Satzbild im Satzbild (Runde
/// 18c), sonst im Textfeld. Öffnet das Satzbild erst noch (über RPC), wird nach dem Öffnen markiert.
function markFundstelle(lq: string | undefined): boolean {
  if (!lq) return false;
  const area = document.querySelector<HTMLTextAreaElement>("#pane-editor textarea[name=body]");
  if (!area) return false;
  const words = lq.split(/\s+/).filter((w) => /[\p{L}\p{N}]/u.test(w));
  const root = area.closest<HTMLElement>("div.wysiwyg[data-widget=wysiwyg]");
  const sheet = root?.querySelector<HTMLElement>("[data-ws-sheet]");
  if (root && sheet && root.hasAttribute("data-ws-open")) {
    if (markInSheet(sheet, words)) return true;
  } else if (root && sheet && area.hidden) {
    // Das Satzbild öffnet gerade (wysiwyg.ts über RPC): erst danach markieren, höchstens 10 s warten.
    const observer = new MutationObserver(() => {
      if (!root.hasAttribute("data-ws-open")) return;
      observer.disconnect();
      markInSheet(sheet, words);
    });
    observer.observe(root, { attributes: true, attributeFilter: ["data-ws-open"] });
    setTimeout(() => observer.disconnect(), 10000);
    return true;
  }
  for (const word of words) {
    const found = findFolded(area.value, word.replace(/[^\p{L}\p{N}]/gu, ""));
    if (!found) continue;
    area.focus({ preventScroll: true });
    area.setSelectionRange(found[0], found[1]);
    const lines = area.value.slice(0, found[0]).split("\n").length - 1;
    const height = parseFloat(getComputedStyle(area).lineHeight) || 20;
    area.scrollTop = Math.max(0, lines * height - area.clientHeight / 3);
    area.scrollIntoView({ block: "nearest" });
    area.classList.add("fundstelle");
    return true;
  }
  return false;
}

/// Nach dem Neuzeichnen des Editors (ui.render) einmal die Fundstelle markieren.
function markAfterRender(lq: string | undefined): void {
  const pane = document.getElementById("pane-editor");
  if (!pane) return;
  const observer = new MutationObserver(() => {
    observer.disconnect();
    markFundstelle(lq);
  });
  observer.observe(pane, { childList: true });
  setTimeout(() => observer.disconnect(), 10000);
}

/// Werk-Treffer im Arbeitsbereich öffnen: über einen `data-nav`-Link im Widget „workspace“
/// (ohne Neuladen); fehlt es (andere Seite), gewöhnlich navigieren.
function openWorkHit(href: string): void {
  const url = new URL(href, window.location.href);
  const lq = url.searchParams.get("lq") ?? undefined;
  const workspace = document.querySelector<HTMLElement>("[data-widget=workspace]");
  if (!workspace || url.pathname !== "/") {
    window.location.assign(url.toString());
    return;
  }
  url.hash = "";
  const link = document.createElement("a");
  link.href = url.toString();
  link.dataset.nav = "";
  link.hidden = true;
  workspace.append(link);
  markAfterRender(lq);
  link.click();
  link.remove();
}

// ---- Popup -------------------------------------------------------------------------------

interface RenderedLiterature {
  html: string;
  lq: string | null;
  lmode?: string;
}

register("literature", (element) => {
  const dialog = element as HTMLDialogElement;
  const content = dialog.querySelector<HTMLElement>(".literature-content");
  const opener = document.querySelector<HTMLAnchorElement>("[data-literature-open]");
  if (!content || typeof dialog.showModal !== "function") return;
  // Suche des Popups; Vorgabe ist die der Adresse. Bleibt über Schließen und Öffnen erhalten.
  let query: string | undefined = lqParam();
  let mode: SearchMode = lmodeParam();
  let loaded = false;

  const load = async (lq: string | undefined, lmode: SearchMode): Promise<void> => {
    try {
      const result = await call<RenderedLiterature>("literature.render", {
        work: numberParam("work"),
        node: numberParam("node"),
        key: keyParam(),
        rev: numberParam("rev"),
        lq: lq ?? "",
        lmode,
      });
      content.innerHTML = result.html;
      mount(content);
      query = result.lq ?? undefined;
      mode = modeOf(result.lmode);
      loaded = true;
      // Suche und Art der Suche stehen in der Adresse, damit Navigation und Neuladen sie behalten.
      const url = new URL(window.location.href);
      if (query) url.searchParams.set("lq", query);
      else url.searchParams.delete("lq");
      if (mode === "rag") url.searchParams.set("lmode", "rag");
      else url.searchParams.delete("lmode");
      history.replaceState(history.state, "", url.toString());
    } catch (error) {
      toast(errorText(error));
    }
  };

  const open = (): void => {
    if (!dialog.open) dialog.showModal();
    if (!loaded) void load(query, mode);
    content.querySelector<HTMLInputElement>('input[name="lq"]')?.focus();
  };

  opener?.addEventListener("click", (event) => {
    if (event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey) return;
    event.preventDefault();
    open();
  });
  document.addEventListener("keydown", (event) => {
    if (!event.altKey || event.ctrlKey || event.metaKey || event.key.toLowerCase() !== "l") return;
    event.preventDefault();
    if (dialog.open) dialog.close();
    else open();
  });
  // Suchformular im Popup: über literature.render statt der Seite /literatur.
  content.addEventListener("submit", (event) => {
    const form = (event.target as Element).closest<HTMLFormElement>("form.library-search");
    if (!form) return;
    event.preventDefault();
    const data = new FormData(form);
    const lq = String(data.get("lq") ?? "");
    void load(lq.trim() === "" ? undefined : lq, modeOf(String(data.get("lmode") ?? "")));
  });
  // „Dieselbe Suche im Wortlaut (FTS)“ ist ein Link auf /literatur; im Popup über literature.render.
  content.addEventListener("click", (event) => {
    const link = (event.target as Element).closest<HTMLAnchorElement>("a[data-fts-link]");
    if (!link || event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey) return;
    event.preventDefault();
    void load(query, "fts");
  });
  // Werk-Treffer: Popup zu, Abschnitt im Arbeitsbereich; Quellen öffnen die Dokumentansicht.
  content.addEventListener("click", (event) => {
    const link = (event.target as Element).closest<HTMLAnchorElement>("a[data-hit=work]");
    if (!link || event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey) return;
    event.preventDefault();
    dialog.close();
    openWorkHit(link.href);
  });
});

// ---- Kontextmenü in der Dokumentansicht --------------------------------------------------

/// Markierung innerhalb eines Zitats: Zitat-id, Stelle und Zeichenbereich (Unicode-Zeichen ab 0).
interface Picked {
  text: string;
  quote?: number;
  locator: string;
  start?: number;
  end?: number;
}

function codePoints(text: string): number {
  return Array.from(text).length;
}

function picked(root: HTMLElement): Picked | null {
  const selection = window.getSelection();
  if (!selection || selection.isCollapsed || selection.rangeCount === 0) return null;
  const range = selection.getRangeAt(0);
  const text = selection.toString();
  if (text.trim() === "" || !root.contains(range.commonAncestorContainer)) return null;
  const node = range.commonAncestorContainer;
  const element = node instanceof Element ? node : node.parentElement;
  // Volltext (Runde 15): Bereich ab dem Anfang des Teils plus `data-offset` (Zeichen ab Anfang des Dokuments).
  const full = element?.closest<HTMLElement>(".fulltext-text");
  if (full && full.contains(range.startContainer) && full.contains(range.endContainer)) {
    const base = Number(full.dataset.offset ?? "0") || 0;
    const at = (container: Node, offset: number): number => {
      const upto = document.createRange();
      upto.selectNodeContents(full);
      upto.setEnd(container, offset);
      return base + codePoints(upto.toString());
    };
    const start = at(range.startContainer, range.startOffset);
    const end = at(range.endContainer, range.endOffset);
    return { text: range.toString(), locator: "", start, end: Math.max(start, end) };
  }
  const quote = element?.closest<HTMLElement>("[data-quote]");
  const body = quote?.querySelector<HTMLElement>(".source-text");
  if (!quote || !body || !body.contains(range.startContainer) || !body.contains(range.endContainer)) {
    return { text, locator: "" };
  }
  // Stellen aus dem DOM-Bereich, nicht aus `selection.toString()`: Die Auswahl fasst Umbrüche und
  // Leerraumfolgen nach CSS zusammen, `Range.toString()` liefert den Text der Textknoten wörtlich.
  // Anfang und Ende zählen darum beide vom Anfang des Zitats aus, und `text` ist der wörtliche
  // Ausschnitt; der Server gleicht zusätzlich leerraum-robust ab und speichert den Zitatausschnitt.
  const offset = (container: Node, at: number): number => {
    const upto = document.createRange();
    upto.selectNodeContents(body);
    upto.setEnd(container, at);
    return codePoints(upto.toString());
  };
  const start = offset(range.startContainer, range.startOffset);
  const end = offset(range.endContainer, range.endOffset);
  return {
    text: range.toString(),
    quote: Number(quote.dataset.quote),
    locator: quote.dataset.locator ?? "",
    start,
    end: Math.max(start, end),
  };
}

register("source", (root) => {
  const work = root.dataset.work ? Number(root.dataset.work) : undefined;
  const node = root.dataset.node ? Number(root.dataset.node) : undefined;
  const key = root.dataset.key ? Number(root.dataset.key) : undefined;
  const title = Number(root.dataset.title);
  const lq = root.dataset.lq;
  const lmode = modeOf(root.dataset.lmode);
  const form = document.querySelector<HTMLFormElement>("form[data-source-form]");
  const place = (): string => {
    const query = new URLSearchParams();
    if (work !== undefined) query.set("work", String(work));
    // Key-Form (Runde 8): die Stelle nur, wenn es keinen Schlüssel gibt.
    if (key !== undefined) query.set("key", String(key));
    else if (node !== undefined) query.set("node", String(node));
    return query.toString();
  };
  const withLq = (url: string): string =>
    (lq ? `${url}&lq=${encodeURIComponent(lq)}` : url) + (lmode === "rag" ? "&lmode=rag" : "");

  const menu = document.createElement("div");
  menu.className = "context-menu";
  menu.setAttribute("role", "menu");
  menu.hidden = true;
  const kinds = form?.querySelector<HTMLSelectElement>("select[name=kind]");
  const kind = document.createElement("select");
  kind.setAttribute("aria-label", "Art der Hervorhebung");
  kind.innerHTML = kinds?.innerHTML ?? '<option value="core">Kernaussage</option>';
  kind.value = "core";
  menu.append(kind);
  const entries: Array<[string, string]> = [
    ["highlight", "Hervorheben"],
    ["cite", "Zitieren"],
    ["persona", "An Persona geben"],
    ["similar", "Ähnliches suchen"],
  ];
  for (const [action, label] of entries) {
    const button = document.createElement("button");
    button.type = "button";
    button.className = "btn";
    button.dataset.action = action;
    button.setAttribute("role", "menuitem");
    button.textContent = label;
    menu.append(button);
  }
  document.body.append(menu);
  let current: Picked | null = null;

  const close = (): void => {
    menu.hidden = true;
    current = null;
  };

  const add = async (picked: Picked, as: string): Promise<number> => {
    const result = await call<{ highlight: { id: number } }>("highlight.add", {
      work,
      title,
      quote: picked.quote,
      locator: picked.locator,
      start: picked.start,
      end: picked.end,
      text: picked.text,
      kind: as,
    });
    return result.highlight.id;
  };

  const run = async (action: string, picked: Picked): Promise<void> => {
    switch (action) {
      case "highlight": {
        const id = await add(picked, kind.value);
        const quote = picked.quote !== undefined ? `&quote=${picked.quote}` : "";
        window.location.assign(withLq(`/quelle?title=${title}&${place()}${quote}&highlight=${id}`) + `#hervorhebung-${id}`);
        break;
      }
      case "cite": {
        // Ohne Abschnitt kein Zitat (früher Abschnitt 0); vor dem Anlegen der Hervorhebung prüfen.
        if (key === undefined) {
          toast("Abschnitt fehlt: Die Dokumentansicht aus einem Abschnitt heraus öffnen.");
          break;
        }
        const id = await add(picked, "quote");
        const result = await call<{ node: number; key: number; rev: number }>("highlight.cite", { work, highlight: id, key });
        window.location.assign(withLq(`/?work=${work ?? ""}&key=${result.key}&rev=${result.rev}`));
        break;
      }
      case "persona": {
        // Ohne Abschnitt keine Antwort im Abschnitts-Chat; der Server sagt dasselbe auf der Hinweisseite.
        if (key === undefined) {
          toast("Abschnitt wählen: Die Literatur aus einem Abschnitt heraus öffnen, dann den Text an die Persona geben.");
          break;
        }
        const field = (name: string): string => {
          const element = form?.elements.namedItem(name);
          return element instanceof HTMLInputElement || element instanceof HTMLSelectElement ? element.value : "";
        };
        const label = root.dataset.sourceLabel ?? "Literatur";
        const source = picked.locator ? `${label}, ${picked.locator}` : label;
        toast("Die Persona antwortet …");
        await call("literature.to_persona", {
          work,
          key,
          persona: field("persona"),
          question: field("question"),
          text: picked.text,
          source,
          title,
        });
        window.location.assign(withLq(`/?work=${work ?? ""}&key=${key}`));
        break;
      }
      case "similar": {
        const words = Array.from(picked.text.matchAll(/[\p{L}\p{N}]{3,}/gu), (m) => m[0]);
        const longest = [...new Set(words)].sort((a, b) => b.length - a.length).slice(0, 3);
        const chosen = words.filter((w, i) => longest.includes(w) && words.indexOf(w) === i);
        const tail = lmode === "rag" ? "&lmode=rag" : "";
        window.location.assign(`/literatur?${place()}&lq=${encodeURIComponent(chosen.join(" "))}${tail}`);
        break;
      }
    }
  };

  root.addEventListener("contextmenu", (event) => {
    const found = picked(root);
    if (!found) return;
    event.preventDefault();
    current = found;
    // Formular unten gleich mitfüllen (auch für die Arbeit ohne Menü).
    if (form) {
      const set = (name: string, value: string) => {
        const field = form.elements.namedItem(name);
        if (field instanceof HTMLInputElement || field instanceof HTMLTextAreaElement || field instanceof HTMLSelectElement) {
          field.value = value;
        }
      };
      set("text", found.text);
      set("quote", found.quote !== undefined ? String(found.quote) : "");
      set("locator", found.locator);
      set("start", found.start !== undefined ? String(found.start) : "");
      set("end", found.end !== undefined ? String(found.end) : "");
    }
    menu.hidden = false;
    const x = Math.min(event.clientX, window.innerWidth - menu.offsetWidth - 8);
    const y = Math.min(event.clientY, window.innerHeight - menu.offsetHeight - 8);
    menu.style.left = `${Math.max(0, x) + window.scrollX}px`;
    menu.style.top = `${Math.max(0, y) + window.scrollY}px`;
    menu.querySelector<HTMLButtonElement>("button")?.focus();
  });
  menu.addEventListener("click", (event) => {
    const button = (event.target as Element).closest<HTMLButtonElement>("button[data-action]");
    if (!button || !current) return;
    const chosen = current;
    close();
    run(button.dataset.action ?? "", chosen).catch((error) => toast(errorText(error)));
  });
  document.addEventListener("pointerdown", (event) => {
    if (!menu.hidden && !menu.contains(event.target as Node)) close();
  });
  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && !menu.hidden) close();
  });
});

// Werk-Treffer aus /literatur oder dem Popup: nach dem Laden die Fundstelle markieren.
if (window.location.hash === "#fundstelle") {
  markFundstelle(lqParam());
}
