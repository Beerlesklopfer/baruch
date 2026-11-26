// SPDX-License-Identifier: AGPL-3.0-or-later
// Widget „wysiwyg“: der Abschnitt im Satzbild der Werkvorlage (contenteditable) mit Umschalter
// „Satzbild | Quelltext“. Der Server zerlegt den Text in Blöcke (RPC wysiwyg.parse) und liefert das
// HTML; Inseln (nicht abbildbares Typst) stehen mit contenteditable=false da. Vor dem Speichern
// (Ereignis `baruch:before-save` am Editor-Formular, ausgelöst von outline.ts) baut wysiwyg.serialize
// den Text neu: Ein Block ohne Eingabe wird als `keep` geschickt und bleibt Byte für Byte erhalten,
// nur Blöcke mit Eingabe werden neu erzeugt. Abgeschickt wird immer das Textfeld.
//
// Den Satzbild-Diff der Vorschläge rendert seit Runde 7 der Server (fragments/proposals.html,
// `pv-satz`); das frühere Widget „wysiwyg-diff“ ist entfallen, die RPC-Methode wysiwyg.diff bleibt.
//
// Vorschläge an ihrer Stelle (Runde 8): Der Server stellt jedes Stück (Hunk) eines offenen Vorschlags
// als <details class="pv" data-pv-gap="g"> in den Lesefluss `section.pv-flow` unter dem Formular. Bei
// offenem Satzbild setzt das Widget jedes Stück als nicht bearbeitbares Geschwister (contenteditable
// = false) vor den Block `[data-b=g]` des Satzbilds (am Ende: dahinter) und verbirgt den Lesefluss; im
// Quelltext kommen die Stücke an ihren Platz im Lesefluss zurück. editsOf, blocksIn und markSelection
// übergehen `.pv`: Speichern ändert dadurch nichts. Die Knöpfe „Übernehmen“/„Verwerfen“ zeigen über das
// Attribut `form` auf Formulare im Lesefluss (keine Formulare im Editor-Formular).
//
// Szenen und Arbeitsaufträge (`#szene[…]`, `#arbeit[…]`): Ihr Rumpf besteht aus eigenen Blöcken
// (`data-n` = Index im Rumpf, beim Öffnen vergeben). Ist nur ein Absatz darin bearbeitet, geht eine
// Rumpf-Anweisung `{replaces, body: [...]}` an den Server; die übrigen Blöcke bleiben byte-gleich.
//
// Überschriften (Runde 7, „Absatz wird Titel“): Eine Überschrift ist immer eine Zeile. Ein einfacher
// Zeilenumbruch in ihr (Umschalt+Enter, `<br>` des Browsers, ein Zeilenende im Text) beendet sie; was
// dahinter steht, geht als eigener Absatz an den Server (der Server teilt ebenso). Umschalt+Enter und
// Einfügen mehrerer Zeilen in einer Überschrift beginnen einen neuen Absatz; Rückschritt am Anfang
// eines Absatzes bzw. Entf am Ende einer Überschrift ziehen keinen Absatz in die Überschrift. Vor dem
// Speichern gelten die Titelregeln des Servers (leer, Zeilenumbruch, über 200 Zeichen) mit derselben
// Meldung; maßgeblich bleibt der Server.
//
// Teile und „ohne Nummer“ (Runde 14): Die Markierungszeilen `#heading(level: 1, numbering: none)[T]
// <baruch-part>` bzw. `… <baruch-unnumbered>` kommen als `div.ws-block.ws-part[data-kind=part]` bzw.
// `div.ws-block.ws-unnumbered[data-kind=unnumbered][data-level]` mit dem Titel als Klartext. Sie
// verhalten sich wie Überschriften (eine Zeile, Enter beginnt einen Absatz dahinter, gleiche
// Titelregeln); geändert geht `{kind: "part", title}` bzw. `{kind: "unnumbered", level, title}` an den
// Server, der die Markierung baut (Kommentar der Zeile bleibt). Das Absatzformat lässt sich für sie
// nicht wählen: Teil und „ohne Nummer“ ändert die Gliederung bzw. der Quelltext.
//
// Bilder (Runde 7): Eine Insel mit Bild kommt als `div.ws-block.ws-figure[data-kind=island]` bzw.
// `span.ws-island.ws-inline-image`; sie ist eine Insel wie jede andere (nicht bearbeitbar, `keep`,
// als Ganzes löschbar, Rohtext in `data-raw`).
//
// Das Widget sitzt auf `div.wysiwyg`, das Werkzeugleiste (`[data-edit-toolbar]`), Blatt
// (`div.editor-body` mit dem Formular und dem Satzbild `[data-ws-wrap]`) und Statuszeile (Hinweis
// `[data-ws-hint]`, „Speichern“ mit `form="editor-form"`) umfasst (Wunsch 2026-11-06). Ohne JavaScript
// bleiben Werkzeugleiste, Satzbild und Hinweis `hidden`, es gibt nur das Textfeld. Runde 33: Nach dem Einhängen
// hängt band.ts die Werkzeugleiste in das Menüband um (Andockplatz); was danach noch in die Leiste greift, nimmt
// `editToolbar` (widgets.ts) statt `root.querySelector`. Vorgabe ist das
// Satzbild (auch nach jedem Neuzeichnen über ui.render), außer das Cookie `baruch_editor` sagt
// ausdrücklich `quelltext`. Mit `#fundstelle` in der Adresse beim Laden der Seite beginnt der erste Editor
// im Quelltext (die Markierung der Fundstelle liegt im Textfeld). Bei offenem Satzbild trägt der Bereich
// `data-ws-open`; das Blatt füllt die Höhe des Editors und scrollt allein (app.css, wysiwyg.css).
//
// Satzbild als Vorgabe (Runde 8, Ursache in docs/WYSIWYG.md): Früher schaltete das Widget beim
// Einhängen erst auf Quelltext (`show` mit `open = false`: Umschalter „Quelltext“ gedrückt, Textfeld
// sichtbar) und öffnete das Satzbild erst nach zwei RPC-Antworten (wysiwyg.parse, wysiwyg.style);
// ein Bildschirmfoto nach dem Laden zeigte darum den Quelltext. Jetzt liefert der Server das Satzbild
// der gezeigten Fassung fertig mit (`[data-ws-sheet][data-ws-ready]`) und `aria-pressed` passend zum
// Cookie; `adoptSheet` öffnet es beim Einhängen sofort, ohne Zwischenzustand. Fehlt es (Text, den das
// Textfeld anders liefert), öffnet `openSheet` wie bisher über RPC, ohne vorher umzuschalten.

// Runde 18b („konstrukte“): Aufzählungen `1.` (kind `enum`, `<ol start>`), Begriffslisten (kind `terms`,
// `<dl>`), geschachtelte Listen (Liste im `<li>`, `depths`), erzwungener Umbruch (`span.ws-linebreak`, t
// `linebreak`), Links (`a.ws-link`, Text bearbeitbar) und Verweise (`button.ws-ref`, nicht bearbeitbar) sind
// Satzbild statt Insel. Eine Insel öffnet per Doppelklick ein Quelltextfeld an Ort und Stelle; geändert geht sie als
// `{replaces, raw}` (Block-Insel) bzw. im neu erzeugten Absatz (Insel im Satz) an den Server. Das Menüband
// (ribbon.ts) schickt Befehle nur über `baruch:ws-command`, die Bild-Werkzeuge fragen die Cursorstelle über
// `baruch:ws-caret` (ws_command.ts). Ein vom Server mitgeliefertes `<style id="ws-style" data-work="W">` wird
// ohne RPC übernommen.
//
// Runde 31c („kontextmenue“): Registermarken `#metadata((…))<baruch-idx>` kommen als leeres, nicht tippbares
// `span.ws-index-mark` (Felder in data-*, Beschreibung in title/aria-label, Zeichen aus dem Stil) und gehen als
// `{t: "index_mark", mark}` zurück; der Server schreibt sie kanonisch. Setzen, Ändern und Entfernen kommen als Befehl
// `index_mark` (ws_command.ts) vom Kontextmenü bzw. Menüband (context_menu.ts); kein Import zwischen den beiden.

import { markdownHeading } from "./markdown.js";
import { call } from "./rpc.js";
import { editToolbar, register, store, submitButton } from "./widgets.js";
import {
  citeKey,
  cleanMark,
  type IndexMark,
  type IndexMarkCommand,
  markLabel,
  markSource,
  scriptureSource,
  validateMark,
  WS_CARET,
  WS_COMMAND,
  type WsCaretDetail,
  type WsCommandDetail,
  type WsCommandName,
} from "./ws_command.js";

type Inline =
  | { t: "text"; v: string }
  | { t: "soft" }
  | { t: "linebreak" }
  | { t: "emph" | "strong" | "footnote" | "quote"; c: Inline[] }
  | { t: "label"; v: string }
  | { t: "island"; raw: string }
  | { t: "link"; url: string; c?: Inline[] }
  | { t: "xref"; key: number; c: Inline[]; page?: boolean }
  | { t: "ref"; target: string; supplement?: string }
  | { t: "index_mark"; mark: IndexMark };

type Term = { term: Inline[]; desc: Inline[] };

type BlockKind =
  | { kind: "heading"; level: number; inlines: Inline[]; label?: string }
  | { kind: "paragraph"; inlines: Inline[] }
  | { kind: "quote"; block: boolean; attribution: Inline[] | null; body: Block[] }
  | { kind: "list"; ordered: boolean; items: Inline[][]; depths?: number[] }
  | { kind: "enum"; start: number; items: Inline[][]; depths?: number[] }
  | { kind: "terms"; items: Term[] }
  | { kind: "scene" | "work"; body: Block[]; args?: string }
  | { kind: "part"; title: string }
  | { kind: "unnumbered"; level: number; title: string }
  | { kind: "island" };

type Block = BlockKind & { raw?: string };

type Edit =
  | { keep: number }
  | { replaces: number | null; block: BlockKind }
  | { replaces: number; body: Edit[] }
  | { replaces: number; raw: string };

interface Parsed {
  html: string;
}

interface Style {
  css: string;
}

const COOKIE = "editor";

/// `#fundstelle` gilt nur für den Editor, der beim Laden der Seite entsteht; nach einem Neuzeichnen
/// (Klick in der Gliederung, ui.render) steht noch die alte Adresse da, bis workspace.ts sie ersetzt.
let fundstelle = window.location.hash === "#fundstelle";

function notify(message: string): void {
  const toast = document.createElement("div");
  toast.className = "toast";
  toast.setAttribute("role", "alert");
  toast.textContent = message;
  document.body.append(toast);
  setTimeout(() => toast.remove(), 5000);
}

// ---- Darstellung nach der Werkvorlage (einmal je Werk) ------------------------------------
/// Laufende Anfragen `wysiwyg.style` je Werk (nur solange sie laufen; danach steht das Element im Dokument).
const styling = new Map<number, Promise<void>>();

/// Nur `keep` bleibt: alle anderen `<style id="ws-style">` (anderer Werke oder veraltet) entfallen.
function dropOtherStyles(keep: HTMLStyleElement | null): void {
  document.querySelectorAll<HTMLStyleElement>("style#ws-style").forEach((element) => {
    if (element !== keep) element.remove();
  });
}

/// Stil der Werkvorlage für `work`. Runde 18b: Liefert der Server ihn schon im HTML (`<style id="ws-style"
/// data-work="W">`, Satzbild je Werk), wird er ohne RPC übernommen; sonst wie bisher über `wysiwyg.style`.
function applyStyle(work: number): Promise<void> {
  const own = document.querySelector<HTMLStyleElement>(`style#ws-style[data-work="${work}"]`);
  if (own) {
    dropOtherStyles(own);
    return Promise.resolve();
  }
  const running = styling.get(work);
  if (running) return running;
  const pending = call<Style>("wysiwyg.style", { work })
    .then((style) => {
      const element = document.createElement("style");
      element.id = "ws-style";
      element.dataset.work = String(work);
      element.textContent = style.css;
      dropOtherStyles(null);
      document.head.append(element);
    })
    .catch(() => {})
    .finally(() => styling.delete(work));
  styling.set(work, pending);
  return pending;
}

// ---- DOM → Blöcke ---------------------------------------------------------------------------
function pushText(out: Inline[], text: string): void {
  if (text === "") return;
  const last = out[out.length - 1];
  if (last && last.t === "text") last.v += text;
  else out.push({ t: "text", v: text });
}

/// Zeilenenden im Sinn von Typst (wie `baruch_typst::is_newline`); der Server ersetzt sie ebenfalls.
const NEWLINES = /[\n\r\v\f\u0085\u2028\u2029]/g;

// ---- Titelregeln (wie `baruch_store::check_title`, maßgeblich bleibt der Server) -------------
const MAX_TITLE_CHARS = 200;

/// Meldung, wenn ein Titel gegen die Regeln des Servers verstößt, sonst `null`.
function titleProblem(title: string): string | null {
  const trimmed = title.trim();
  if (trimmed === "") return "Ein Gliederungspunkt braucht einen Titel.";
  if (/[\n\r\v\f\u0085\u2028\u2029]/.test(trimmed)) return "Der Titel darf keinen Zeilenumbruch enthalten.";
  const count = [...trimmed].length;
  if (count > MAX_TITLE_CHARS) return `Der Titel ist zu lang (höchstens ${MAX_TITLE_CHARS} Zeichen, hier ${count}).`;
  return null;
}

/// Sichtbarer Text einer Folge (für die Titelregeln; Inseln mit ihrem Rohtext).
function plainText(inlines: Inline[]): string {
  return inlines
    .map((inline) => {
      switch (inline.t) {
        case "text":
          return inline.v;
        case "soft":
        case "linebreak":
          return " ";
        case "label":
        case "index_mark":
          return "";
        case "island":
          return inline.raw;
        case "link":
          return inline.c ? plainText(inline.c) : inline.url;
        case "xref":
          return plainText(inline.c);
        case "ref":
          return `@${inline.target}`;
        default:
          return plainText(inline.c);
      }
    })
    .join("");
}

/// Überschrift am ersten einfachen Umbruch teilen, auf oberster Ebene wie in emph/strong (beliebig
/// tief): davor der Titel, dahinter (falls etwas übrig bleibt) ein Absatz; eine geteilte
/// Hervorhebung steht mit ihrem vorderen Teil im Titel und mit dem hinteren im Absatz. Fußnoten und
/// Zitate bleiben ganz. Wie `split_heading` im Server; Umbrüche am Anfang zählen nicht.
function splitHeading(inlines: Inline[]): { head: Inline[]; rest: Inline[] | null } {
  const { head, rest } = cutAtBreak(trimFront(inlines));
  if (rest === null) return { head, rest: null };
  const tail = trimBack(trimFront(rest));
  return { head: trimBack(head), rest: tail.length > 0 ? tail : null };
}

/// Am ersten Umbruch teilen, der in der Überschriftszeile stünde (oberste Ebene oder in emph/strong).
function cutAtBreak(inlines: Inline[]): { head: Inline[]; rest: Inline[] | null } {
  const head: Inline[] = [];
  for (let k = 0; k < inlines.length; k++) {
    const inline = inlines[k];
    if (inline.t === "soft") return { head, rest: inlines.slice(k + 1) };
    if (inline.t !== "emph" && inline.t !== "strong") {
      head.push(inline);
      continue;
    }
    const inner = cutAtBreak(inline.c);
    if (inner.rest === null) {
      head.push({ t: inline.t, c: inner.head });
      continue;
    }
    if (inner.head.length > 0) head.push({ t: inline.t, c: inner.head });
    const rest: Inline[] = inner.rest.length > 0 ? [{ t: inline.t, c: inner.rest }] : [];
    return { head, rest: rest.concat(inlines.slice(k + 1)) };
  }
  return { head, rest: null };
}

/// Umbrüche, leere Texte, leere Hervorhebungen und Leerraum am Anfang entfernen (auch in emph/strong).
function trimFront(inlines: Inline[]): Inline[] {
  const list = inlines.slice();
  while (list.length > 0) {
    const first = list[0];
    if (first.t === "soft") list.shift();
    else if (first.t === "text") {
      const v = first.v.replace(/^[ \t]+/, "");
      if (v === "") list.shift();
      else {
        list[0] = { t: "text", v };
        break;
      }
    } else if (first.t === "emph" || first.t === "strong") {
      const c = trimFront(first.c);
      if (c.length === 0) list.shift();
      else {
        list[0] = { t: first.t, c };
        break;
      }
    } else break;
  }
  return list;
}

/// Wie `trimFront`, aber am Ende.
function trimBack(inlines: Inline[]): Inline[] {
  const list = inlines.slice();
  while (list.length > 0) {
    const k = list.length - 1;
    const last = list[k];
    if (last.t === "soft") list.pop();
    else if (last.t === "text") {
      const v = last.v.replace(/[ \t]+$/, "");
      if (v === "") list.pop();
      else {
        list[k] = { t: "text", v };
        break;
      }
    } else if (last.t === "emph" || last.t === "strong") {
      const c = trimBack(last.c);
      if (c.length === 0) list.pop();
      else {
        list[k] = { t: last.t, c };
        break;
      }
    } else break;
  }
  return list;
}

/// Runde 31c: Marke aus einem `span.ws-index-mark` (Felder in `data-register`, `data-term`, `data-sub`, `data-see`,
/// `data-also`, `data-sort`, wie baruch-wysiwyg sie schreibt); ungültig → `null` (die Marke entfällt nicht still: das
/// Kontextmenü prüft vorher mit `validateMark`).
function markOf(element: HTMLElement): IndexMark | null {
  const d = element.dataset;
  const mark = cleanMark({
    register: d.register ?? "",
    term: d.term ?? "",
    sub: d.sub,
    see: d.see,
    also: d.also,
    sort: d.sort,
  });
  return validateMark(mark) === null ? mark : null;
}

/// Runde 31c: Felder, Beschreibung und `data-raw` einer Marke an ein (neues oder vorhandenes) Element schreiben –
/// dieselbe Form wie `render_index_mark` in baruch-wysiwyg.
function fillMark(element: HTMLElement, mark: IndexMark): void {
  const clean = cleanMark(mark);
  element.className = "ws-index-mark";
  element.contentEditable = "false";
  element.setAttribute("role", "img");
  element.tabIndex = -1;
  const label = markLabel(clean);
  element.title = label;
  element.setAttribute("aria-label", label);
  for (const key of ["register", "term", "sub", "see", "also", "sort"] as const) {
    const value = clean[key];
    if (value) element.dataset[key] = value;
    else delete element.dataset[key];
  }
  element.dataset.raw = markSource(clean);
  element.textContent = "";
}

/// Alle Blöcke um `node` (oberster und geschachtelte) als bearbeitet markieren.
function dirtyAround(node: Node): void {
  for (let el = node instanceof HTMLElement ? node : node.parentElement; el; el = el.parentElement) {
    if (el.classList.contains("ws-block")) el.dataset.dirty = "1";
    if (el.hasAttribute("data-ws-sheet")) break;
  }
}

/// Fließtext eines Elements; unbekannte Elemente (span, font, div des Browsers) zählen nur mit ihrem Inhalt.
/// `heading`: das Label hinter der Überschrift (`data-heading-label`) gehört nicht zum Titel.
function inlinesOf(element: Node, heading = false): Inline[] {
  const out: Inline[] = [];
  const walk = (node: Node, into: Inline[]) => {
    for (const child of Array.from(node.childNodes)) {
      if (child.nodeType === Node.TEXT_NODE) {
        // Geschützte Leerzeichen setzt der Browser auch selbst; echte stehen in span.ws-nbsp.
        const raw = (child.textContent ?? "").replace(/\u00a0/g, " ");
        if (heading) {
          // In einer Überschrift beendet ein Zeilenende sie (Runde 7), statt Text anzuhängen.
          raw.split(NEWLINES).forEach((piece, k) => {
            if (k > 0) into.push({ t: "soft" });
            pushText(into, piece);
          });
        } else pushText(into, raw.replace(NEWLINES, " "));
        continue;
      }
      if (!(child instanceof HTMLElement)) continue;
      const tag = child.tagName;
      const list = (t: "emph" | "strong" | "footnote" | "quote") => {
        const c: Inline[] = [];
        walk(child, c);
        if (c.length > 0) into.push({ t, c });
      };
      if (heading && child.dataset.headingLabel !== undefined) continue;
      // Das Quelltextfeld einer Insel (Runde 18b) ist Bedienung, kein Text.
      if (child.classList.contains("ws-island-edit")) continue;
      // Runde 31c: Registermarke (leeres Element, Felder in data-*); vor der Insel geprüft.
      if (child.classList.contains("ws-index-mark")) {
        const mark = markOf(child);
        if (mark) into.push({ t: "index_mark", mark });
      } else if (child.classList.contains("ws-island")) into.push({ t: "island", raw: child.dataset.raw ?? "" });
      else if (child.classList.contains("ws-linebreak")) into.push({ t: "linebreak" });
      else if (child.classList.contains("ws-ref")) {
        const ref: Extract<Inline, { t: "ref" }> = { t: "ref", target: child.dataset.target ?? "" };
        if (child.dataset.supplement !== undefined) ref.supplement = child.dataset.supplement;
        into.push(ref);
      } else if (child.classList.contains("ws-xref")) {
        // Runde 30a: Querverweis; der Seitenzahl-Zusatz (span.ws-xref-page) ist Anzeige, kein Text.
        const key = Number(child.dataset.key);
        const c: Inline[] = [];
        const shown = child.cloneNode(true) as HTMLElement;
        shown.querySelectorAll(".ws-xref-page").forEach((p) => p.remove());
        walk(shown, c);
        if (c.length > 0 && Number.isFinite(key)) into.push({ t: "xref", key, c, page: child.dataset.page === "1" });
      } else if (child.classList.contains("ws-link")) {
        const url = child.dataset.url ?? "";
        if (child.dataset.bare !== undefined) into.push({ t: "link", url });
        else {
          // Ein Link ohne Text (alles gelöscht) entfällt mit seiner Adresse.
          const c: Inline[] = [];
          walk(child, c);
          if (c.length > 0) into.push({ t: "link", url, c });
        }
      }
      else if (child.classList.contains("ws-label")) into.push({ t: "label", v: child.dataset.v ?? "" });
      else if (child.classList.contains("ws-soft")) {
        into.push({ t: "soft" });
        pushText(into, (child.textContent ?? "").replace(/^ /, "").replace(/ /g, " "));
      } else if (child.classList.contains("ws-nbsp")) {
        const text = child.textContent ?? "";
        pushText(into, text === " " ? " " : text.replace(/ /g, " "));
      } else if (child.classList.contains("ws-footnote")) list("footnote");
      else if (child.classList.contains("ws-quote") || tag === "Q") list("quote");
      else if (tag === "EM" || tag === "I" || child.style.fontStyle === "italic") list("emph");
      else if (tag === "STRONG" || tag === "B" || child.style.fontWeight === "bold" || child.style.fontWeight === "700") {
        list("strong");
      } else if (tag === "BR") into.push({ t: "soft" });
      else walk(child, into);
    }
  };
  walk(element, out);
  // Umbrüche am Rand und doppelte entfallen, Leerraum am Rand auch.
  const clean: Inline[] = [];
  for (const inline of out) {
    if (inline.t === "soft" && (clean.length === 0 || clean[clean.length - 1].t === "soft")) continue;
    clean.push(inline);
  }
  while (clean.length > 0 && clean[clean.length - 1].t === "soft") clean.pop();
  const first = clean[0];
  if (first && first.t === "text") first.v = first.v.replace(/^[ \t]+/, "");
  const last = clean[clean.length - 1];
  if (last && last.t === "text") last.v = last.v.replace(/[ \t]+$/, "");
  return clean.filter((inline) => inline.t !== "text" || inline.v !== "");
}

/// Blöcke, die eine Zeile mit Titel sind: Überschrift, Teil, „ohne Nummer“ (Runde 14).
function isTitleKind(kind: string | undefined): boolean {
  return kind === "heading" || kind === "part" || kind === "unnumbered";
}

/// Titel eines Teils bzw. einer Markierung „ohne Nummer“: Klartext, Zeilenenden und geschützte
/// Leerzeichen werden Leerzeichen.
function markerTitle(element: HTMLElement): string {
  return (element.textContent ?? "").replace(/\u00a0/g, " ").replace(NEWLINES, " ").trim();
}

function isBlock(node: Node): node is HTMLElement {
  return node instanceof HTMLElement && node.classList.contains("ws-block");
}

/// Ein Stück eines Vorschlags (details.pv), das im Satzbild steht: kein Teil des Texts.
function isProposal(node: Node): boolean {
  return node instanceof HTMLElement && node.classList.contains("pv");
}

/// Art eines Blocks aus seinem Element (auch geschachtelt; Inseln mit ihrem Rohtext).
function kindOf(element: HTMLElement): Block | null {
  const kind = element.dataset.kind;
  const body = (): Block[] => {
    const holder = element.querySelector<HTMLElement>(":scope > .ws-body");
    return holder ? blocksIn(holder) : [];
  };
  switch (kind) {
    case "heading": {
      const label = element.querySelector<HTMLElement>(":scope > [data-heading-label]")?.dataset.v;
      const heading: Block = { kind: "heading", level: Number(element.dataset.level ?? "1"), inlines: inlinesOf(element, true) };
      if (label) heading.label = label;
      return heading;
    }
    case "list":
    case "enum": {
      // Runde 18b: geschachtelte Punkte (Liste im `<li>`) mit Tiefe; ohne Schachtelung kein `depths`.
      const list = element.querySelector<HTMLElement>(":scope > ul, :scope > ol");
      const items: Inline[][] = [];
      const depths: number[] = [];
      if (list) listItems(list, 0, items, depths);
      if (items.length === 0) return null;
      const nested = depths.some((d) => d > 0);
      if (kind === "enum") {
        const start = Number(element.dataset.start ?? list?.getAttribute("start") ?? "1");
        const block: Extract<BlockKind, { kind: "enum" }> = { kind: "enum", start: Number.isFinite(start) ? start : 1, items };
        if (nested) block.depths = depths;
        return block;
      }
      const block: Extract<BlockKind, { kind: "list" }> = { kind: "list", ordered: element.dataset.ordered === "true", items };
      if (nested) block.depths = depths;
      return block;
    }
    case "terms": {
      // Runde 18b: Begriffsliste als `<dl>`; jedes `<dt>` beginnt einen Eintrag, `<dd>` ist seine Erklärung.
      const items: Term[] = [];
      for (const child of Array.from(element.querySelector(":scope > dl")?.children ?? [])) {
        if (child.tagName === "DT") items.push({ term: inlinesOf(child), desc: [] });
        else if (child.tagName === "DD" && items.length > 0) items[items.length - 1].desc.push(...inlinesOf(child));
      }
      const kept = items.filter((item) => item.term.length > 0);
      return kept.length > 0 ? { kind: "terms", items: kept } : null;
    }
    case "quote": {
      const attribution = element.querySelector<HTMLElement>(":scope > .ws-attribution");
      return {
        kind: "quote",
        block: element.dataset.block === "true",
        attribution: attribution ? inlinesOf(attribution) : null,
        body: body(),
      };
    }
    case "scene":
    case "work": {
      const args = element.querySelector<HTMLElement>(":scope > .ws-args")?.dataset.raw;
      const block: Block = { kind, body: body() };
      if (args !== undefined) block.args = args;
      return block;
    }
    case "part":
      return { kind: "part", title: markerTitle(element) };
    case "unnumbered":
      return { kind: "unnumbered", level: Number(element.dataset.level ?? "1"), title: markerTitle(element) };
    case "island":
      return { kind: "island", raw: element.dataset.raw ?? "" };
    default: {
      const inlines = inlinesOf(element);
      return inlines.length > 0 ? { kind: "paragraph", inlines } : null;
    }
  }
}

/// Punkte einer Liste in Reihenfolge mit ihrer Tiefe (Runde 18b): je `<li>` der Text ohne geschachtelte Listen, dann
/// die geschachtelten Listen eine Ebene tiefer (auch eine Liste direkt in der Liste, wie Browser einrücken).
function listItems(list: Element, depth: number, items: Inline[][], depths: number[]): void {
  for (const child of Array.from(list.children)) {
    if (child.tagName === "UL" || child.tagName === "OL") {
      listItems(child, depth + 1, items, depths);
      continue;
    }
    if (child.tagName !== "LI") continue;
    const own = child.cloneNode(true) as HTMLElement;
    own.querySelectorAll(":scope > ul, :scope > ol").forEach((nested) => nested.remove());
    const inlines = inlinesOf(own);
    if (inlines.length > 0) {
      items.push(inlines);
      depths.push(depth);
    }
    for (const nested of Array.from(child.children)) {
      if (nested.tagName === "UL" || nested.tagName === "OL") listItems(nested, depth + 1, items, depths);
    }
  }
}

/// Geschachtelte Blöcke eines Rumpfs; lose Texte (vom Browser eingefügt) werden Absätze.
function blocksIn(holder: HTMLElement): Block[] {
  const out: Block[] = [];
  for (const child of Array.from(holder.childNodes)) {
    if (isProposal(child)) continue;
    const block = isBlock(child) ? kindOf(child) : looseParagraph(child);
    if (block) out.push(block);
  }
  return out;
}

function looseParagraph(node: Node): Block | null {
  const holder = document.createElement("div");
  holder.append(node.cloneNode(true));
  const inlines = inlinesOf(holder);
  return inlines.length > 0 ? { kind: "paragraph", inlines } : null;
}

/// Rumpf-Blöcke einer Szene/Arbeit durchnummerieren (`data-n`, Index im Rumpf wie beim Server).
function numberBodies(sheet: HTMLElement): void {
  sheet.querySelectorAll<HTMLElement>(".ws-body").forEach((body) => {
    let n = 0;
    for (const child of Array.from(body.children)) {
      if (isBlock(child)) child.dataset.n = String(n++);
    }
  });
}

/// Anweisungen für wysiwyg.serialize: `keep` für jeden Block ohne Eingabe, sonst der neue Block; für
/// eine bearbeitete Szene/Arbeit Anweisungen für ihren Rumpf (nur der bearbeitete Absatz wird neu).
/// Ein zweiter Block mit demselben Index (vom Browser kopiert) gilt als neu. `holder`: das Satzbild
/// (Index `data-b`) oder ein Rumpf (`data-n`).
function editsOf(holder: HTMLElement, nested = false, upTo: Node | null = null): Edit[] {
  const edits: Edit[] = [];
  const seen = new Set<number>();
  for (const child of Array.from(holder.childNodes)) {
    // `upTo` (Runde 18b, Cursorstelle): nur die Blöcke bis einschließlich dieses einen.
    if (upTo && child !== upTo && upTo.compareDocumentPosition(child) & Node.DOCUMENT_POSITION_FOLLOWING) break;
    // Vorschläge im Satzbild gehören nicht zum Text.
    if (isProposal(child)) continue;
    if (!isBlock(child)) {
      const block = looseParagraph(child);
      if (block) edits.push({ replaces: null, block });
      continue;
    }
    const mark = nested ? child.dataset.n : child.dataset.b;
    const index = mark === undefined ? null : Number(mark);
    const fresh = index === null || seen.has(index);
    if (index !== null) seen.add(index);
    // Runde 18b: eine an Ort und Stelle geänderte Insel geht wörtlich als `{replaces, raw}`.
    if (!fresh && child.dataset.kind === "island" && child.dataset.edited === "1") {
      edits.push({ replaces: index as number, raw: child.dataset.raw ?? "" });
      continue;
    }
    if (!fresh && (child.dataset.dirty !== "1" || child.dataset.kind === "island")) {
      edits.push({ keep: index as number });
      continue;
    }
    const inner = child.querySelector<HTMLElement>(":scope > .ws-body");
    if (!fresh && inner && (child.dataset.kind === "scene" || child.dataset.kind === "work")) {
      edits.push({ replaces: index as number, body: editsOf(inner, true) });
      continue;
    }
    const block = kindOf(child);
    if (!block || block.kind === "island") continue;
    if (block.kind === "heading") {
      edits.push(...headingEdits(block, fresh ? null : index));
      continue;
    }
    const kind: Block = { ...block };
    delete kind.raw;
    edits.push({ replaces: fresh ? null : index, block: kind });
  }
  return edits;
}

/// Ein Überschriftsblock: die Überschrift bis zum ersten Umbruch, dahinter (falls vorhanden) ein Absatz.
function headingEdits(block: Block & { kind: "heading" }, replaces: number | null): Edit[] {
  const { head, rest } = splitHeading(block.inlines);
  const heading: Extract<BlockKind, { kind: "heading" }> = { kind: "heading", level: block.level, inlines: head };
  if (block.label) heading.label = block.label;
  const edits: Edit[] = [{ replaces, block: heading }];
  if (rest) edits.push({ replaces: null, block: { kind: "paragraph", inlines: rest } });
  return edits;
}

/// Titelregeln für alle Überschriften im Satzbild (Ebene 1–3 sind Gliederungspunkte); `null` = gut.
function headingProblem(sheet: HTMLElement): string | null {
  for (const child of Array.from(sheet.children)) {
    if (!(child instanceof HTMLElement) || !isBlock(child)) continue;
    if (child.dataset.kind === "part" || child.dataset.kind === "unnumbered") {
      const problem = titleProblem(markerTitle(child));
      if (problem) return problem;
      continue;
    }
    if (child.dataset.kind !== "heading") continue;
    if (Number(child.dataset.level ?? "1") > 3) continue;
    const { head } = splitHeading(inlinesOf(child, true));
    const problem = titleProblem(plainText(head));
    if (problem) return problem;
  }
  return null;
}

/// Eingefügter Text → Absätze (an Leerzeilen getrennt, einfache Zeilenenden werden Leerzeichen).
function pastedParagraphs(text: string): string[] {
  return text
    .split(/\n[ \t]*\n\s*/)
    .map((part) => part.replace(NEWLINES, " ").trim())
    .filter((part) => part !== "");
}

// ---- Widget -----------------------------------------------------------------------------------
interface State {
  form: HTMLFormElement;
  area: HTMLTextAreaElement;
  sheet: HTMLElement;
  work: number;
  /// Text, aus dem das Satzbild gebaut ist (die Indizes `data-b` beziehen sich darauf).
  parsed: string;
  /// Satzbild offen?
  open: boolean;
  /// Eingabe im Satzbild seit dem letzten Übertragen ins Textfeld?
  touched: boolean;
  /// Ins Satzbild gesetzte Vorschläge mit ihrem Platz (Kommentar) im Lesefluss.
  placed: { item: HTMLElement; home: Comment }[];
}

const states = new WeakMap<HTMLFormElement, State>();

/// Zuletzt eingehängtes Satzbild mit seiner Formatwahl (für `selectionchange`, einmal je Seite).
let current: { state: State; format: HTMLSelectElement | null } | null = null;

document.addEventListener("selectionchange", () => {
  if (!current || !current.format || !current.state.open || !current.state.sheet.isConnected) return;
  const block = topBlock(current.state.sheet, document.getSelection()?.anchorNode ?? null);
  if (!block) return;
  current.format.value = isTitleKind(block.dataset.kind) && block.dataset.kind !== "part" ? (block.dataset.level ?? "1") : "paragraph";
});

/// Markdown-Überschrift im Satzbild (Nutzerentscheidung 2026-11-07): Ein Absatz, dessen Text mit `# `
/// bis `###### ` beginnt, wird eine Überschrift dieser Ebene (4–6 → 3, Regel in `markdown.ts`); die
/// Rauten und eine schließende Folge ` ##` entfallen, der Cursor bleibt an seiner Stelle im Titel.
/// `true`, wenn umgewandelt.
function convertMarkdownHeading(state: State, block: HTMLElement): boolean {
  if (block.dataset.kind !== "paragraph" || block.closest("li")) return false;
  const found = markdownHeading((block.textContent ?? "").replace(/\n/g, " "));
  if (!found) return false;
  const walker = document.createTreeWalker(block, NodeFilter.SHOW_TEXT);
  const first = walker.nextNode() as Text | null;
  const prefix = first ? /^#{1,6} /.exec(first.data) : null;
  if (!first || !prefix) return false;
  const selection = document.getSelection();
  const caret = selection && selection.rangeCount > 0 ? selection.getRangeAt(0) : null;
  const caretOffset = caret && caret.startContainer === first ? caret.startOffset : null;
  first.data = first.data.slice(prefix[0].length);
  let last: Text = first;
  for (let node = walker.nextNode(); node; node = walker.nextNode()) last = node as Text;
  const closing = /[ \t]+#+\s*$/.exec(last.data);
  if (closing) last.data = last.data.slice(0, closing.index);
  if (caretOffset !== null && selection) {
    const place = document.createRange();
    place.setStart(first, Math.max(0, Math.min(first.data.length, caretOffset - prefix[0].length)));
    place.collapse(true);
    selection.removeAllRanges();
    selection.addRange(place);
  }
  block.dataset.kind = "heading";
  block.dataset.level = String(found.level);
  block.dataset.dirty = "1";
  markTouched(state);
  return true;
}

/// Satzbild ins Textfeld übertragen (wysiwyg.serialize), danach `input` am Textfeld. Vorher werden
/// Absätze mit Markdown-Überschrift umgewandelt (wer ohne Enter speichert).
async function sync(state: State): Promise<void> {
  if (!state.open || !state.touched) return;
  for (const child of Array.from(state.sheet.children)) {
    if (child instanceof HTMLElement && isBlock(child)) convertMarkdownHeading(state, child);
  }
  const problem = headingProblem(state.sheet);
  if (problem) throw new Error(problem);
  const result = await call<{ body: string }>("wysiwyg.serialize", {
    work: state.work,
    body: state.parsed,
    edits: editsOf(state.sheet),
  });
  state.touched = false;
  state.area.value = result.body;
  state.area.dispatchEvent(new Event("input", { bubbles: true }));
}

/// Quelltext des offenen Satzbilds wie `sync`, aber ohne Folgen (Vertrag Runde 17 §2.6): kein Umwandeln von
/// Markdown-Überschriften im Satzbild, Textfeld und `touched` bleiben, wie sie sind. Ohne Eingabe seit dem
/// letzten Übertragen steht der Quelltext schon im Textfeld.
async function serializeText(state: State): Promise<string> {
  if (!state.touched) return state.area.value;
  const result = await call<{ body: string }>("wysiwyg.serialize", {
    work: state.work,
    body: state.parsed,
    edits: editsOf(state.sheet),
  });
  return result.body;
}

/// Oberster Block, in dem ein Knoten liegt.
function topBlock(sheet: HTMLElement, node: Node | null): HTMLElement | null {
  let current: Node | null = node;
  while (current && current.parentNode !== sheet) current = current.parentNode;
  return current && isBlock(current) ? current : null;
}

/// Alle Blöcke, die die Auswahl berührt, als bearbeitet markieren: oberste und geschachtelte (im
/// Rumpf einer Szene), damit nur diese neu erzeugt werden.
function markSelection(state: State): void {
  const selection = document.getSelection();
  if (!selection || selection.rangeCount === 0) return;
  const range = selection.getRangeAt(0);
  state.sheet.querySelectorAll<HTMLElement>(".ws-block").forEach((block) => {
    // Blöcke im Satzbild eines Vorschlags (details.pv) sind nicht der Text.
    if (block.closest(".pv")) return;
    if (range.intersectsNode(block)) block.dataset.dirty = "1";
  });
}

/// Steht der (leere) Cursor am Anfang bzw. Ende des Blocks?
function caretAt(block: HTMLElement, range: Range, edge: "start" | "end"): boolean {
  if (!range.collapsed) return false;
  const probe = document.createRange();
  probe.selectNodeContents(block);
  if (edge === "start") probe.setEnd(range.startContainer, range.startOffset);
  else probe.setStart(range.endContainer, range.endOffset);
  return probe.toString().replace(NEWLINES, "") === "";
}

/// Nachbarblock davor bzw. dahinter; Stücke der Vorschläge (`details.pv`) dazwischen zählen nicht (Runde 18b: sonst
/// griff der Schutz unten nicht, wenn ein Vorschlag zwischen Überschrift und Absatz stand).
function blockNeighbour(block: HTMLElement, backward: boolean): HTMLElement | null {
  let node = backward ? block.previousElementSibling : block.nextElementSibling;
  while (node && isProposal(node)) node = backward ? node.previousElementSibling : node.nextElementSibling;
  return node instanceof HTMLElement ? node : null;
}

/// Rückschritt am Anfang eines Absatzes hinter einer Überschrift bzw. Entf am Ende einer Überschrift
/// vor einem Absatz: Der Browser zöge den Absatz in die Überschrift (Runde 7). Stattdessen entfällt nur
/// ein leerer Absatz; sonst bleibt alles, wie es ist. `true` = abgefangen.
function guardHeadingMerge(state: State, event: InputEvent): boolean {
  if (event.inputType !== "deleteContentBackward" && event.inputType !== "deleteContentForward") return false;
  const selection = document.getSelection();
  if (!selection || selection.rangeCount === 0) return false;
  const range = selection.getRangeAt(0);
  const block = topBlock(state.sheet, range.startContainer);
  if (!block) return false;
  const backward = event.inputType === "deleteContentBackward";
  const neighbour = blockNeighbour(block, backward);
  if (!neighbour || !isBlock(neighbour)) return false;
  const [heading, other] = backward ? [neighbour, block] : [block, neighbour];
  if (!isTitleKind(heading.dataset.kind) || isTitleKind(other.dataset.kind) || other.dataset.kind === "island") {
    return false;
  }
  if (!caretAt(block, range, backward ? "start" : "end")) return false;
  event.preventDefault();
  if ((other.textContent ?? "").trim() === "") {
    other.remove();
    markTouched(state);
    const caret = document.createRange();
    caret.selectNodeContents(heading);
    caret.collapse(false);
    selection.removeAllRanges();
    selection.addRange(caret);
  }
  return true;
}

function markTouched(state: State): void {
  state.touched = true;
  state.form.dataset.dirty = "1";
  const save = submitButton(state.form);
  if (save) {
    save.disabled = false;
    save.textContent = save.dataset.dirty ?? save.textContent;
  }
}

/// Enter: Block am Cursor teilen (neuer Absatz ohne Index). In Listen macht der Browser einen
/// neuen Punkt.
function splitBlock(state: State): void {
  const selection = document.getSelection();
  if (!selection || selection.rangeCount === 0) return;
  const range = selection.getRangeAt(0);
  const start = range.startContainer instanceof Element ? range.startContainer : range.startContainer.parentElement;
  const block = start?.closest<HTMLElement>(".ws-block");
  if (!block || !state.sheet.contains(block)) return;
  range.deleteContents();
  const tail = document.createRange();
  tail.setStart(range.startContainer, range.startOffset);
  // Das Label hinter einer Überschrift bleibt bei ihr.
  const label = block.querySelector<HTMLElement>(":scope > [data-heading-label]");
  if (label && range.comparePoint(label, 0) > 0) tail.setEndBefore(label);
  else tail.setEnd(block, block.childNodes.length);
  const rest = tail.extractContents();
  const fresh = document.createElement("div");
  fresh.className = "ws-block";
  fresh.dataset.kind = "paragraph";
  fresh.dataset.dirty = "1";
  fresh.append(rest);
  // Ein leerer Block braucht einen Platzhalter, sonst setzt der Browser den Cursor nicht hinein.
  if (!fresh.textContent) fresh.append(document.createElement("br"));
  block.after(fresh);
  const top = topBlock(state.sheet, block);
  if (top) top.dataset.dirty = "1";
  block.dataset.dirty = "1";
  const caret = document.createRange();
  caret.setStart(fresh, 0);
  caret.collapse(true);
  selection.removeAllRanges();
  selection.addRange(caret);
}

/// Umschalt+Enter: einfacher Zeilenumbruch (im Satz ein Leerzeichen, im Quelltext eine neue Zeile).
function insertSoft(): void {
  const selection = document.getSelection();
  if (!selection || selection.rangeCount === 0) return;
  const range = selection.getRangeAt(0);
  range.deleteContents();
  const soft = document.createElement("span");
  soft.className = "ws-soft";
  soft.textContent = " ";
  range.insertNode(soft);
  range.setStartAfter(soft);
  range.collapse(true);
  selection.removeAllRanges();
  selection.addRange(range);
}

/// Absatzformat des Blocks am Cursor (nur Absatz und Überschrift).
function setFormat(state: State, value: string): void {
  const selection = document.getSelection();
  const block = topBlock(state.sheet, selection?.anchorNode ?? null);
  if (block && (block.dataset.kind === "part" || block.dataset.kind === "unnumbered")) {
    notify("Teile und Punkte ohne Nummer ändert die Gliederung oder der Quelltext, nicht das Absatzformat.");
    return;
  }
  if (!block || (block.dataset.kind !== "paragraph" && block.dataset.kind !== "heading")) {
    notify("Das Absatzformat lässt sich nur für Absätze und Überschriften wählen.");
    return;
  }
  if (value === "paragraph") {
    block.dataset.kind = "paragraph";
    delete block.dataset.level;
  } else {
    block.dataset.kind = "heading";
    block.dataset.level = value;
  }
  block.dataset.dirty = "1";
  markTouched(state);
}

/// Lesefluss mit den Vorschlägen dieses Editors (section.pv-flow neben dem Formular), falls da.
function flowOf(state: State): HTMLElement | null {
  return state.form.parentElement?.querySelector<HTMLElement>(":scope > section.pv-flow") ?? null;
}

/// Erster Block des Satzbilds mit `data-b` ≥ `gap` (neue Blöcke ohne Index zählen nicht).
function blockFrom(sheet: HTMLElement, gap: number): HTMLElement | null {
  for (const child of Array.from(sheet.children)) {
    if (child instanceof HTMLElement && isBlock(child) && child.dataset.b !== undefined && Number(child.dataset.b) >= gap) {
      return child;
    }
  }
  return null;
}

/// Stücke der Vorschläge aus dem Lesefluss an ihre Stelle im Satzbild setzen: vor den Block
/// `[data-b=gap]` (am Ende dahinter), in der Reihenfolge des Servers (an derselben Stelle untereinander
/// nach Persona, darin nach Nummer), nicht bearbeitbar. Der Lesefluss wird verborgen.
function placeProposals(state: State): void {
  const flow = flowOf(state);
  if (!flow) return;
  const items = Array.from(flow.querySelectorAll<HTMLElement>("details.pv[data-pv-gap]"));
  for (const item of items) {
    const home = document.createComment("pv");
    item.before(home);
    state.placed.push({ item, home });
    item.contentEditable = "false";
    const target = blockFrom(state.sheet, Number(item.dataset.pvGap));
    if (target) target.before(item);
    else state.sheet.append(item);
  }
  flow.hidden = true;
}

/// Stücke zurück an ihren Platz im Lesefluss (vor neuem Satzbild und im Quelltext); Lesefluss zeigen.
function returnProposals(state: State): void {
  for (const { item, home } of state.placed) {
    item.removeAttribute("contenteditable");
    home.replaceWith(item);
  }
  state.placed = [];
  const flow = flowOf(state);
  if (flow) flow.hidden = false;
}

/// Satzbild öffnen, dessen HTML schon im Blatt steht (vom Server oder aus wysiwyg.parse).
function sheetReady(state: State, root: HTMLElement): void {
  numberBodies(state.sheet);
  state.parsed = state.area.value;
  state.touched = false;
  state.open = true;
  show(state, root);
  placeProposals(state);
}

async function openSheet(state: State, root: HTMLElement): Promise<boolean> {
  try {
    const [parsed] = await Promise.all([
      call<Parsed>("wysiwyg.parse", { work: state.work, body: state.area.value }),
      applyStyle(state.work),
    ]);
    returnProposals(state);
    state.sheet.innerHTML = parsed.html;
    sheetReady(state, root);
    return true;
  } catch (error) {
    notify(String((error as Error).message ?? error));
    return false;
  }
}

/// Beim Einhängen: das fertige Satzbild des Servers (`data-ws-ready`, Runde 8) sofort öffnen, ohne
/// erst den Quelltext zu zeigen; der Stil der Werkvorlage kommt nach. `false`, wenn es fehlt.
function adoptSheet(state: State, root: HTMLElement): boolean {
  if (!state.sheet.hasAttribute("data-ws-ready")) return false;
  state.sheet.removeAttribute("data-ws-ready");
  sheetReady(state, root);
  void applyStyle(state.work);
  return true;
}

function show(state: State, root: HTMLElement): void {
  // Runde 33: Die Leiste steht nach dem Andocken im Menüband (band.ts), nicht mehr unter `root`.
  const bar = editToolbar(root);
  bar?.querySelectorAll<HTMLButtonElement>("[data-ws-mode]").forEach((button) => {
    button.setAttribute("aria-pressed", String((button.dataset.wsMode === "satz") === state.open));
  });
  const tools = bar?.querySelector<HTMLElement>("[data-ws-tools]");
  if (tools) tools.hidden = !state.open;
  const toolsOff = bar?.querySelector<HTMLElement>("[data-ws-source-note]");
  if (toolsOff) toolsOff.hidden = state.open;
  const hint = root.querySelector<HTMLElement>("[data-ws-hint]");
  if (hint) hint.hidden = !state.open;
  const wrap = root.querySelector<HTMLElement>("[data-ws-wrap]");
  if (wrap) wrap.hidden = !state.open;
  // Nur das offene Satzbild wächst (wysiwyg.css); im Quelltext nimmt das Textfeld den Platz.
  root.toggleAttribute("data-ws-open", state.open);
  state.area.hidden = state.open;
}

// ---- Runde 18b: Befehle des Menübands, Cursorstelle, Inseln an Ort und Stelle ------------------

/// Zuletzt gesehene Auswahl im Satzbild je Formular (ein Klick ins Menüband nimmt den Fokus).
const lastRanges = new WeakMap<State, Range>();

/// Liegt der Knoten im Text des Satzbilds (nicht in einem Vorschlag, nicht im Quelltextfeld einer Insel)?
function inSheetText(state: State, node: Node | null): boolean {
  const element = node instanceof Element ? node : (node?.parentElement ?? null);
  return !!element && state.sheet.contains(element) && !element.closest(".pv, .ws-island-edit");
}

/// Auswahl im Satzbild: die aktuelle, sonst die zuletzt gesehene (wiederhergestellt), sonst `null`.
function sheetRange(state: State): Range | null {
  const selection = document.getSelection();
  if (selection && selection.rangeCount > 0 && inSheetText(state, selection.getRangeAt(0).startContainer)) {
    return selection.getRangeAt(0);
  }
  const last = lastRanges.get(state);
  if (!last || !inSheetText(state, last.startContainer)) return null;
  state.sheet.focus();
  selection?.removeAllRanges();
  selection?.addRange(last);
  return last;
}

/// Neues Element ans Ende eines Blocks bzw. an die Auswahl setzen und den Cursor hinein stellen.
function placeCaret(node: Node, select: boolean): void {
  const selection = document.getSelection();
  if (!selection) return;
  const range = document.createRange();
  if (select) range.selectNodeContents(node);
  else {
    range.setStartAfter(node);
    range.collapse(true);
  }
  selection.removeAllRanges();
  selection.addRange(range);
}

/// Inline-Element an der Auswahl: die Auswahl kommt hinein, sonst `placeholder` (markiert zum Überschreiben).
function wrapInline(range: Range, element: HTMLElement, placeholder: string): void {
  if (range.collapsed) element.textContent = placeholder;
  else element.append(range.extractContents());
  range.insertNode(element);
  placeCaret(element, range.collapsed || element.textContent === placeholder);
}

/// Neuer Block (ohne Index) mit Art `kind`.
function newBlock(kind: string): HTMLElement {
  const block = document.createElement("div");
  block.className = "ws-block";
  block.dataset.kind = kind;
  block.dataset.dirty = "1";
  return block;
}

/// Nachbesserung R31: Steht dieselbe Marke schon in der Kette der Marken direkt an `at` (wie `mark_word` im Server)?
export function sameMarkBehind(at: Range, mark: IndexMark): boolean {
  const node = at.startContainer;
  const offset = at.startOffset;
  let next: Node | null =
    node.nodeType === Node.TEXT_NODE
      ? offset >= (node as Text).data.length
        ? node.nextSibling
        : null
      : (node.childNodes[offset] ?? null);
  const raw = markSource(cleanMark(mark));
  while (next) {
    if (next.nodeType === Node.TEXT_NODE && (next as Text).data === "") {
      next = next.nextSibling;
      continue;
    }
    if (!(next instanceof HTMLElement) || !next.classList.contains("ws-index-mark")) return false;
    if (next.dataset.raw === raw) return true;
    next = next.nextSibling;
  }
  return false;
}

/// Runde 31c: Ende des markierten Worts als leere Stelle – eine Auswahl ohne Leerraum am Ende, ohne Auswahl das Ende
/// des Worts am Cursor. Dort steht die Marke („direkt hinter dem markierten Wort“).
function wordEnd(range: Range): Range {
  const end = document.createRange();
  let node = range.endContainer;
  let offset = range.endOffset;
  if (node.nodeType === Node.TEXT_NODE) {
    const data = (node as Text).data;
    if (range.collapsed) {
      while (offset < data.length && /[\p{L}\p{N}\p{M}_'’-]/u.test(data[offset])) offset++;
    } else {
      while (offset > 0 && /\s/.test(data[offset - 1])) offset--;
    }
  }
  end.setStart(node, offset);
  end.collapse(true);
  return end;
}

/// Runde 31c: Befehl `index_mark` (`arg` = JSON {op, mark}, ws_command.ts). `add` setzt eine neue Marke hinter das
/// markierte Wort (nicht in Überschrift, Teil, „ohne Nummer“, Insel, Verweis, Label oder einer anderen Marke), `edit`
/// und `remove` wirken auf die Marke mit `data-ctx-target` (vom Kontextmenü gesetzt). Der Block wird neu erzeugt, der
/// Editor ist danach ungespeichert; gespeichert wird mit „Speichern“ als neue Fassung.
function runIndexMark(state: State, arg?: string): void {
  let command: IndexMarkCommand;
  try {
    command = JSON.parse(arg ?? "{}") as IndexMarkCommand;
  } catch {
    notify("Registermarke: unlesbarer Auftrag.");
    return;
  }
  const mark = cleanMark(command.mark ?? { register: "", term: "" });
  if (command.op === "edit" || command.op === "remove") {
    const target = state.sheet.querySelector<HTMLElement>(".ws-index-mark[data-ctx-target]");
    state.sheet.querySelectorAll("[data-ctx-target]").forEach((el) => el.removeAttribute("data-ctx-target"));
    if (!target || target.closest(".pv")) {
      notify("Die Registermarke steht nicht mehr im Satzbild.");
      return;
    }
    if (command.op === "edit") {
      const problem = validateMark(mark);
      if (problem) {
        notify(problem);
        return;
      }
      fillMark(target, mark);
      dirtyAround(target);
    } else {
      dirtyAround(target);
      target.remove();
    }
    markTouched(state);
    return;
  }
  const problem = validateMark(mark);
  if (problem) {
    notify(problem);
    return;
  }
  const range = sheetRange(state);
  const top = range ? topBlock(state.sheet, range.startContainer) : null;
  if (!range || !top) {
    notify("Bitte zuerst ein Wort im Satzbild markieren.");
    return;
  }
  const at = range.endContainer instanceof Element ? range.endContainer : range.endContainer.parentElement;
  if (isTitleKind(top.dataset.kind) || top.dataset.kind === "island") {
    notify("In einer Überschriftszeile oder Typst-Insel steht keine Registermarke; bitte ein Wort im Fließtext wählen.");
    return;
  }
  if (at?.closest(".ws-island, .ws-index-mark, .ws-ref, .ws-label, .ws-linebreak")) {
    notify("Hier lässt sich keine Registermarke setzen; bitte ein Wort im Fließtext wählen.");
    return;
  }
  const end = wordEnd(range);
  if (sameMarkBehind(end, mark)) {
    notify("Dieses Wort trägt diese Registermarke schon; nichts geändert.");
    return;
  }
  const element = document.createElement("span");
  fillMark(element, mark);
  end.insertNode(element);
  placeCaret(element, false);
  dirtyAround(element);
  markTouched(state);
}

/// Befehl des Menübands im offenen Satzbild ausführen (Vertrag R18-S1, `baruch:ws-command`). Jeder Befehl gilt als
/// erledigt, solange das Satzbild offen ist – auch wenn er an der Stelle nicht geht (dann mit Meldung), damit der
/// Absender nicht ins verborgene Textfeld schreibt.
function runCommand(state: State, cmd: WsCommandName, arg?: string): void {
  if (cmd === "index_mark") {
    runIndexMark(state, arg);
    return;
  }
  const range = sheetRange(state);
  const top = range ? topBlock(state.sheet, range.startContainer) : null;
  const textual = (block: HTMLElement | null) =>
    !!block && ["paragraph", "heading", "list", "enum", "terms", "quote"].includes(block.dataset.kind ?? "");
  const inlineTarget = range && top && (textual(top) || top.dataset.kind === "scene" || top.dataset.kind === "work");
  const done = () => {
    if (top) top.dataset.dirty = "1";
    markSelection(state);
    markTouched(state);
  };
  switch (cmd) {
    case "footnote":
    case "quote":
    case "break":
    case "xref":
    case "scripture":
    case "cite":
    case "link": {
      if (!range || !inlineTarget) {
        notify("Bitte zuerst den Cursor in einen Absatz setzen.");
        return;
      }
      if (cmd === "scripture") {
        // Runde 19b: Bibelstelle nach der Werkvorlage als Insel `#fn[Stelle]` (nur als Ganzes änderbar, wie im Satz).
        let choice: { fn?: unknown; text?: unknown } = {};
        try {
          choice = JSON.parse(arg ?? "{}");
        } catch {
          choice = {};
        }
        const fn = typeof choice.fn === "string" ? choice.fn : "";
        const text = typeof choice.text === "string" ? choice.text : (window.prompt("Bibelstelle (z. B. Joh 3,16):") ?? "");
        if (text.trim() === "") return;
        const raw = scriptureSource(fn, text);
        if (!raw) {
          notify("Diese Bibelstelle lässt sich so nicht einfügen (höchstens 200 Zeichen, Funktion der Vorlage nötig).");
          return;
        }
        range.deleteContents();
        const island = document.createElement("span");
        island.className = "ws-island";
        island.contentEditable = "false";
        island.dataset.raw = raw;
        island.title = "Typst-Insel";
        island.textContent = raw;
        range.insertNode(island);
        placeCaret(island, false);
      } else if (cmd === "cite") {
        // Runde 19b: Literaturverweis `@schlüssel` wie ein Verweis aus dem Quelltext (button.ws-ref).
        const key = citeKey(arg ?? window.prompt("Schlüssel des Literaturverweises (z. B. mueller2020):") ?? "");
        if (!key) {
          if (arg !== undefined) notify("Ungültiger Schlüssel (Buchstaben, Ziffern, _ - : .).");
          return;
        }
        range.deleteContents();
        const ref = document.createElement("button");
        ref.type = "button";
        ref.className = "ws-ref";
        ref.contentEditable = "false";
        ref.tabIndex = -1;
        ref.dataset.target = key;
        ref.title = `Verweis auf <${key}> (nur im Quelltext änderbar)`;
        ref.textContent = `@${key}`;
        range.insertNode(ref);
        placeCaret(ref, false);
      } else if (cmd === "break") {
        range.deleteContents();
        const brk = document.createElement("span");
        brk.className = "ws-linebreak";
        brk.contentEditable = "false";
        brk.dataset.linebreak = "1";
        brk.title = "erzwungener Zeilenumbruch (\\)";
        brk.append("↵", document.createElement("br"));
        range.insertNode(brk);
        placeCaret(brk, false);
      } else if (cmd === "xref") {
        // Runde 30a: Querverweis aus dem Dialog (xref.ts), `arg` = JSON {key, text, page}.
        let choice: { key?: unknown; text?: unknown; page?: unknown } = {};
        try {
          choice = JSON.parse(arg ?? "{}");
        } catch {
          choice = {};
        }
        if (typeof choice.key !== "number" || typeof choice.text !== "string") return;
        const xref = document.createElement("span");
        xref.className = "ws-xref";
        xref.dataset.key = String(choice.key);
        xref.dataset.page = choice.page === true ? "1" : "0";
        xref.title = `Querverweis auf k${choice.key} (Strg+Klick: springen)`;
        wrapInline(range, xref, choice.text);
        if (choice.page === true) {
          const suffix = document.createElement("span");
          suffix.className = "ws-xref-page";
          suffix.contentEditable = "false";
          suffix.textContent = ", S.\u00a0…";
          xref.append(suffix);
        }
      } else if (cmd === "link") {
        const url = (arg ?? window.prompt("Adresse des Links (z. B. https://…):") ?? "").trim();
        if (!url) return;
        const link = document.createElement("a");
        link.className = "ws-link";
        link.dataset.url = url;
        link.title = `Link: ${url}`;
        wrapInline(range, link, url);
      } else if (cmd === "quote" && range.collapsed && top) {
        // Runde 23a (Befund des Nutzers): Ohne Auswahl entsteht ein Zitatblock als eigener Block hinter dem Block am
        // Cursor, darunter ein Absatz „– “ für die Stellenangabe (beide neu, also mit Leerzeilen im Quelltext).
        // Mit Auswahl bleibt es beim Zitat im Fließtext (`#quote[…]`).
        const box = newBlock("quote");
        box.dataset.block = "true";
        const body = document.createElement("div");
        body.className = "ws-body";
        const paragraph = newBlock("paragraph");
        paragraph.textContent = arg ?? "Zitat";
        body.append(paragraph);
        box.append(body);
        const reference = newBlock("paragraph");
        reference.textContent = "–\u00a0";
        top.after(box, reference);
        placeCaret(paragraph, true);
        markTouched(state);
        return;
      } else {
        const element = document.createElement(cmd === "footnote" ? "span" : "q");
        element.className = cmd === "footnote" ? "ws-footnote" : "ws-quote";
        wrapInline(range, element, arg ?? (cmd === "footnote" ? "Fußnote" : "Zitat"));
      }
      done();
      return;
    }
    case "list":
    case "enum":
    case "terms": {
      if (!top || (top.dataset.kind !== "paragraph" && top.dataset.kind !== "list" && top.dataset.kind !== "enum")) {
        notify("Liste, Aufzählung und Begriffsliste entstehen aus einem Absatz (Cursor hineinsetzen).");
        return;
      }
      if (top.dataset.kind !== "paragraph") {
        // Liste ↔ Aufzählung: nur die Art und das Listenelement wechseln, die Punkte bleiben.
        if (cmd === "terms") {
          notify("Eine Liste wird keine Begriffsliste; bitte einen Absatz wählen.");
          return;
        }
        const old = top.querySelector<HTMLElement>(":scope > ul, :scope > ol");
        const fresh = document.createElement(cmd === "enum" ? "ol" : "ul");
        if (old) {
          fresh.append(...Array.from(old.childNodes));
          old.replaceWith(fresh);
        }
        top.dataset.kind = cmd;
        if (cmd === "enum") top.dataset.start = "1";
        else top.dataset.ordered = "false";
        done();
        return;
      }
      const content = Array.from(top.childNodes);
      top.textContent = "";
      top.dataset.kind = cmd;
      if (cmd === "terms") {
        const dl = document.createElement("dl");
        const dt = document.createElement("dt");
        dt.textContent = arg ?? "Begriff";
        const dd = document.createElement("dd");
        dd.append(...content);
        dl.append(dt, dd);
        top.append(dl);
        placeCaret(dt, true);
      } else {
        const list = document.createElement(cmd === "enum" ? "ol" : "ul");
        const li = document.createElement("li");
        li.append(...content);
        list.append(li);
        top.append(list);
        if (cmd === "enum") top.dataset.start = "1";
        else top.dataset.ordered = "false";
        placeCaret(li, false);
      }
      done();
      return;
    }
    case "scene":
    case "work": {
      if (top && (top.dataset.kind === "scene" || top.dataset.kind === "work")) {
        notify("Hier steht schon eine Szene bzw. ein Arbeitsauftrag.");
        return;
      }
      const box = newBlock(cmd);
      const body = document.createElement("div");
      body.className = "ws-body";
      box.append(body);
      if (top && textual(top) && top.dataset.kind !== "heading") {
        // Der Block am Cursor wandert in den Rumpf (dort neu, ohne Index); der alte Block entfällt.
        top.before(box);
        delete top.dataset.b;
        top.dataset.dirty = "1";
        body.append(top);
      } else {
        const paragraph = newBlock("paragraph");
        paragraph.append(document.createElement("br"));
        body.append(paragraph);
        if (top) top.after(box);
        else state.sheet.append(box);
        placeCaret(paragraph, true);
      }
      markTouched(state);
      return;
    }
  }
}

/// Quelltext des offenen Satzbilds (wie `baruch:serialize-text`) und die UTF-16-Stelle des Zeilenanfangs hinter dem
/// obersten Block am Cursor (Vertrag R18-S1, `baruch:ws-caret`). Die Stelle ist die Länge des Texts, den dieselben
/// Anweisungen bis einschließlich dieses Blocks ergeben (samt seiner Leerzeilen); ohne Cursor oder hinter dem
/// letzten Block das Ende.
async function caretPlace(state: State): Promise<{ body: string; offset: number }> {
  const body = await serializeText(state);
  const range = sheetRange(state);
  const block = range ? topBlock(state.sheet, range.startContainer) : null;
  if (!block) return { body, offset: body.length };
  const prefix = await call<{ body: string }>("wysiwyg.serialize", {
    work: state.work,
    body: state.parsed,
    edits: editsOf(state.sheet, false, block),
  });
  const head = prefix.body;
  const fits = body.startsWith(head) && (head.endsWith("\n") || head.length === body.length);
  return { body, offset: fits ? head.length : body.length };
}

/// Quelltextfeld einer Insel an Ort und Stelle (Doppelklick): Übernehmen setzt `data-raw` und markiert die Insel
/// (`data-edited`), Abbrechen bzw. Escape stellt sie wieder her. Tasten im Feld gehen nicht an das Satzbild.
function editIsland(state: State, island: HTMLElement): void {
  if (island.querySelector(".ws-island-edit") || island.closest(".pv")) return;
  const saved = Array.from(island.childNodes);
  const raw = (island.dataset.raw ?? "").replace(/\s+$/, "");
  const editor = document.createElement(island.tagName === "SPAN" ? "span" : "div");
  editor.className = "ws-island-edit";
  editor.contentEditable = "false";
  const area = document.createElement("textarea");
  area.value = raw;
  area.setAttribute("aria-label", "Quelltext der Typst-Insel");
  area.rows = Math.min(12, Math.max(2, raw.split("\n").length + 1));
  const buttons = document.createElement("span");
  buttons.className = "ws-island-buttons";
  const ok = document.createElement("button");
  ok.type = "button";
  ok.className = "btn small";
  ok.textContent = "Übernehmen";
  const cancel = document.createElement("button");
  cancel.type = "button";
  cancel.className = "btn small";
  cancel.textContent = "Abbrechen";
  buttons.append(ok, cancel);
  editor.append(area, buttons);
  island.replaceChildren(editor);
  const close = (apply: boolean) => {
    if (!island.contains(editor)) return;
    const value = area.value.replace(/\s+$/, "");
    if (apply && value !== raw) {
      island.dataset.raw = island.tagName === "SPAN" ? value : `${value}\n`;
      island.dataset.edited = "1";
      island.dataset.dirty = "1";
      // Die Insel zeigt ihren neuen Rohtext (ein Bild erst nach dem Speichern wieder als Bild).
      const code = document.createElement("code");
      code.className = "ws-raw";
      code.textContent = value;
      island.replaceChildren(island.tagName === "SPAN" ? document.createTextNode(value) : code);
      island.classList.remove("ws-inline-image", "ws-figure");
      // Inseln im Satz gehören zu ihrem Absatz; der wird mit der neuen Insel neu erzeugt.
      if (island.tagName === "SPAN") {
        let holder = island.parentElement?.closest<HTMLElement>(".ws-block") ?? null;
        while (holder) {
          holder.dataset.dirty = "1";
          holder = holder.parentElement?.closest<HTMLElement>(".ws-block") ?? null;
        }
      }
      const top = topBlock(state.sheet, island);
      if (top) top.dataset.dirty = "1";
      markTouched(state);
    } else island.replaceChildren(...saved);
  };
  for (const type of ["keydown", "keyup", "keypress", "beforeinput", "input", "paste"]) {
    area.addEventListener(type, (event) => event.stopPropagation());
  }
  area.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      event.preventDefault();
      close(false);
    } else if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
      event.preventDefault();
      close(true);
    }
  });
  // Wer woanders hinklickt oder speichert, übernimmt (blur kommt vor dem Absenden).
  area.addEventListener("change", () => close(true));
  ok.addEventListener("click", () => close(true));
  cancel.addEventListener("mousedown", (event) => event.preventDefault());
  cancel.addEventListener("click", () => close(false));
  area.focus();
}

/// Enter in einer Begriffsliste: ein neuer Eintrag (Begriff und Erklärung) hinter dem aktuellen; `true` = erledigt.
function newTerm(state: State): boolean {
  const anchor = document.getSelection()?.anchorNode ?? null;
  const cell = (anchor instanceof Element ? anchor : anchor?.parentElement)?.closest<HTMLElement>("dt, dd");
  if (!cell || !state.sheet.contains(cell)) return false;
  const dd = cell.tagName === "DT" ? (cell.nextElementSibling?.tagName === "DD" ? cell.nextElementSibling : cell) : cell;
  const dt = document.createElement("dt");
  dt.append(document.createElement("br"));
  const desc = document.createElement("dd");
  dd.after(dt, desc);
  const top = topBlock(state.sheet, cell);
  if (top) top.dataset.dirty = "1";
  placeCaret(dt, true);
  markTouched(state);
  return true;
}

/// Runde 30a: Text `find` im Satzbild markieren (Highlight „baruch-find“) und hinscrollen; Typst-Maskierungen und
/// Leerraumfolgen zählen nicht. Ohne Highlight-API nur hinscrollen.
function markFound(sheet: HTMLElement, find: string): void {
  const plain = find.replace(/\\(.)/g, "$1").replace(/[_*]/g, "").trim();
  if (!plain) return;
  const walker = document.createTreeWalker(sheet, NodeFilter.SHOW_TEXT);
  const nodes: { node: Text; start: number }[] = [];
  let all = "";
  for (let n = walker.nextNode(); n; n = walker.nextNode()) {
    nodes.push({ node: n as Text, start: all.length });
    all += (n as Text).data;
  }
  const words = plain.split(/\s+/).map((w) => w.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
  const match = new RegExp(words.join("\\s+")).exec(all.replace(/\u00a0/g, " "));
  if (!match) return;
  const point = (pos: number) => {
    for (let i = nodes.length - 1; i >= 0; i--) {
      if (nodes[i].start <= pos) return { node: nodes[i].node, offset: Math.min(pos - nodes[i].start, nodes[i].node.data.length) };
    }
    return null;
  };
  const a = point(match.index);
  const b = point(match.index + match[0].length);
  if (!a || !b) return;
  const range = document.createRange();
  range.setStart(a.node, a.offset);
  range.setEnd(b.node, b.offset);
  a.node.parentElement?.scrollIntoView({ block: "center" });
  const registry = typeof CSS !== "undefined" && "highlights" in CSS ? (CSS as unknown as { highlights: Map<string, unknown> }).highlights : null;
  const Ctor = (window as unknown as { Highlight?: new (...r: Range[]) => unknown }).Highlight;
  if (registry && Ctor) registry.set("baruch-find", new Ctor(range));
}

register("wysiwyg", (root) => {
  // Das Widget umfasst Werkzeugleiste, Blatt (mit dem Formular) und Statuszeile (editor.html).
  const form = root.querySelector<HTMLFormElement>("form[data-widget=editor]");
  const area = form?.querySelector<HTMLTextAreaElement>("textarea[name=body]");
  const sheet = root.querySelector<HTMLElement>("[data-ws-sheet]");
  if (!form || !area || !sheet) return;
  const state: State = {
    form,
    area,
    sheet,
    work: Number(root.dataset.work),
    parsed: area.value,
    open: false,
    touched: false,
    placed: [],
  };
  states.set(form, state);
  // Vorgabe Satzbild; Quelltext nur nach ausdrücklicher Wahl (Cookie) oder für die Fundstelle beim Laden.
  const source = fundstelle || store.get(COOKIE) === "quelltext";
  fundstelle = false;
  // Die Werkzeugleiste (fragments/edit_toolbar.html) ist ohne JavaScript verborgen.
  const toolbar = root.querySelector<HTMLElement>("[data-edit-toolbar]");
  if (toolbar) toolbar.hidden = false;
  // Satzbild vom Server: gleich offen. Sonst Quelltext zeigen, nur wenn er gewünscht ist; das Satzbild
  // über RPC öffnet unten, ohne vorher auf Quelltext umzuschalten.
  if (source || !adoptSheet(state, root)) {
    if (source) show(state, root);
    else state.area.hidden = true;
  }

  root.querySelectorAll<HTMLButtonElement>("[data-ws-mode]").forEach((button) => {
    button.addEventListener("click", async () => {
      const satz = button.dataset.wsMode === "satz";
      if (satz === state.open) return;
      // Ausdrückliche Wahl: nur sie setzt das Cookie.
      store.set(COOKIE, satz ? "satz" : "quelltext");
      if (satz) {
        await openSheet(state, root);
        state.sheet.focus();
        return;
      }
      try {
        await sync(state);
      } catch (error) {
        notify(String((error as Error).message ?? error));
        return;
      }
      state.open = false;
      returnProposals(state);
      show(state, root);
      state.area.focus();
    });
  });

  root.querySelectorAll<HTMLButtonElement>("[data-ws-cmd]").forEach((button) => {
    // Die Auswahl im Satzbild bleibt beim Klick erhalten.
    button.addEventListener("mousedown", (event) => event.preventDefault());
    button.addEventListener("click", () => {
      markSelection(state);
      document.execCommand(button.dataset.wsCmd ?? "", false);
      markTouched(state);
    });
  });
  const format = root.querySelector<HTMLSelectElement>("[data-ws-format]");
  format?.addEventListener("change", () => {
    setFormat(state, format.value);
    state.sheet.focus();
  });
  current = { state, format };

  // Runde 18b: Befehle des Menübands (ws_command.ts). Ohne bubbles am Bereich; nur bei offenem Satzbild erledigt.
  root.addEventListener(WS_COMMAND, (event) => {
    const detail = (event as CustomEvent<WsCommandDetail>).detail;
    if (!detail || !state.open || !state.sheet.isConnected) return;
    detail.handled = true;
    runCommand(state, detail.cmd, detail.arg);
  });
  // Cursorstelle für Bilder (ws_command.ts): genau ein Promise, auch wenn das Ereignis am Formular ankommt.
  const answered = new WeakSet<Event>();
  const onCaret = (event: Event) => {
    const detail = (event as CustomEvent<WsCaretDetail>).detail;
    if (answered.has(event) || !state.open || !Array.isArray(detail?.wait)) return;
    answered.add(event);
    detail.wait.push(caretPlace(state));
  };
  root.addEventListener(WS_CARET, onCaret);
  form.addEventListener(WS_CARET, onCaret);
  // Runde 30a: Fundstelle eines Sprungs (jump.ts, `baruch:find {text, at}`) im Satzbild markieren – über die CSS
  // Custom Highlight API, ohne das DOM zu ändern; den Quelltext übernimmt search.ts. Ist das Satzbild noch nicht
  // offen (es lädt über RPC), wird kurz gewartet.
  const onFind = (event: Event) => {
    const detail = (event as CustomEvent<{ text?: unknown }>).detail;
    if (typeof detail?.text !== "string" || !root.isConnected) return;
    const text = detail.text;
    let tries = 0;
    const attempt = () => {
      if (!root.isConnected) return;
      if (!state.open) {
        if (++tries < 30 && state.area.hidden) window.setTimeout(attempt, 100);
        return;
      }
      markFound(state.sheet, text);
    };
    attempt();
  };
  document.addEventListener("baruch:find", onFind);
  document.addEventListener("selectionchange", () => {
    const selection = document.getSelection();
    if (state.open && selection && selection.rangeCount > 0 && inSheetText(state, selection.getRangeAt(0).startContainer)) {
      lastRanges.set(state, selection.getRangeAt(0).cloneRange());
    }
  });
  // Inseln an Ort und Stelle: Doppelklick öffnet das Quelltextfeld (auch Bilder; Inseln in Vorschlägen nicht).
  sheet.addEventListener("dblclick", (event) => {
    const target = event.target instanceof Element ? event.target : null;
    const island = target?.closest<HTMLElement>(".ws-island, .ws-block[data-kind=island]");
    if (!island || !sheet.contains(island) || island.closest(".pv") || target?.closest(".ws-island-edit")) return;
    event.preventDefault();
    editIsland(state, island);
  });
  // Ein Verweis ist eine Schaltfläche: der Klick zeigt das Ziel (Label im Satzbild), ändert aber nichts.
  sheet.addEventListener("click", (event) => {
    const ref = event.target instanceof Element ? event.target.closest<HTMLElement>("button.ws-ref") : null;
    if (!ref || !sheet.contains(ref)) return;
    event.preventDefault();
    const target = ref.dataset.target ?? "";
    const label = Array.from(sheet.querySelectorAll<HTMLElement>(".ws-label")).find((l) => l.dataset.v === target);
    if (label) label.scrollIntoView({ block: "center" });
    else notify(`Verweis auf <${target}> – das Ziel steht nicht in diesem Abschnitt; ändern nur im Quelltext.`);
  });

  sheet.addEventListener("beforeinput", (event) => {
    if (guardHeadingMerge(state, event as InputEvent)) return;
    markSelection(state);
  });
  sheet.addEventListener("input", () => {
    markSelection(state);
    markTouched(state);
  });
  sheet.addEventListener("paste", (event) => {
    // Nur Text einfügen; fremdes HTML wäre kein Typst. Leerzeilen trennen Absätze: Jedes weitere
    // Stück beginnt wie mit Enter einen neuen Absatz (in Listen bleibt es ein Punkt).
    event.preventDefault();
    const text = (event.clipboardData?.getData("text/plain") ?? "").replace(/\r\n?/g, "\n");
    markSelection(state);
    const parts = pastedParagraphs(text);
    const anchor = document.getSelection()?.anchorNode ?? null;
    const inList = (anchor?.parentElement ?? null)?.closest("li");
    const inHeading = isTitleKind(topBlock(state.sheet, anchor)?.dataset.kind);
    if (inHeading && NEWLINES.test(text.trim())) {
      // Mehrere Zeilen in einer Überschrift: Sie gehören darunter, nie in die Überschriftszeile.
      NEWLINES.lastIndex = 0;
      for (const part of text.split(/\n\s*\n|\n/).map((p) => p.replace(NEWLINES, " ").trim()).filter((p) => p !== "")) {
        splitBlock(state);
        document.execCommand("insertText", false, part);
      }
      markTouched(state);
      return;
    }
    NEWLINES.lastIndex = 0;
    if (parts.length < 2 || inList) {
      document.execCommand("insertText", false, text.replace(NEWLINES, " "));
      return;
    }
    document.execCommand("insertText", false, parts[0]);
    for (const part of parts.slice(1)) {
      splitBlock(state);
      document.execCommand("insertText", false, part);
    }
    markTouched(state);
  });
  sheet.addEventListener("keydown", (event) => {
    // Strg+S speichert über den Hörer am Dokument (panes.ts), der Fokus darf überall stehen.
    if ((event.ctrlKey || event.metaKey) && (event.key.toLowerCase() === "i" || event.key.toLowerCase() === "b")) {
      markSelection(state);
      markTouched(state);
      return;
    }
    if (event.key !== "Enter" || event.isComposing) return;
    const inList = (document.getSelection()?.anchorNode?.parentElement ?? null)?.closest("li");
    markSelection(state);
    markTouched(state);
    if (inList && !event.shiftKey) return;
    event.preventDefault();
    // Runde 18b: In einer Begriffsliste beginnt Enter einen neuen Eintrag.
    if (!event.shiftKey && newTerm(state)) return;
    // Enter schließt eine Markdown-Überschrift ab (`# Titel` → Überschrift), danach wie immer ein Absatz.
    const current = topBlock(state.sheet, document.getSelection()?.anchorNode ?? null);
    if (!event.shiftKey && current) convertMarkdownHeading(state, current);
    // In einer Überschrift beginnt auch Umschalt+Enter einen neuen Absatz (Runde 7).
    const inHeading = isTitleKind(topBlock(state.sheet, document.getSelection()?.anchorNode ?? null)?.dataset.kind);
    if (event.shiftKey && !inHeading) insertSoft();
    else splitBlock(state);
  });

  // Vor dem Speichern: Satzbild ins Textfeld (Vertrag Runde 5, 3b; ausgelöst von outline.ts).
  form.addEventListener("baruch:before-save", (event) => {
    const detail = (event as CustomEvent<{ wait?: Promise<unknown>[] }>).detail;
    if (state.open && state.touched && Array.isArray(detail?.wait)) detail.wait.push(sync(state));
  });

  // Vollständiger Quelltext, ohne zu speichern (Vertrag Runde 17 §2.6, z. B. für die Live-Zählung in
  // timing.ts): Nur bei offenem Satzbild legt das Widget ein Promise in `wait`; Textfeld und `touched`
  // bleiben unverändert. Ist niemand zuständig, bleibt `wait` leer (Rückfall beim Auslöser).
  form.addEventListener("baruch:serialize-text", (event) => {
    const detail = (event as CustomEvent<{ wait?: Promise<string>[] }>).detail;
    if (state.open && Array.isArray(detail?.wait)) detail.wait.push(serializeText(state));
  });

  if (!source && !state.open) {
    void openSheet(state, root).then((opened) => {
      // Scheitert das Öffnen, bleibt der Quelltext (Textfeld wieder zeigen).
      if (!opened) show(state, root);
    });
  }
});

// Rückfall, falls niemand `baruch:before-save` auslöst: Vor jedem anderen Hörer (Fenster, Einfang-
// Phase) das Satzbild übertragen und danach erneut absenden.
window.addEventListener(
  "submit",
  (event) => {
    const form = event.target instanceof HTMLFormElement ? event.target : null;
    const state = form ? states.get(form) : undefined;
    if (!form || !state || !state.open || !state.touched) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    // Runde 22: Der auslösende Knopf („Als neue Fassung speichern“) geht beim erneuten Absenden mit.
    const submitter = (event as SubmitEvent).submitter;
    sync(state)
      .then(() => {
        if (submitter instanceof HTMLButtonElement && submitter.form === form) form.requestSubmit(submitter);
        else form.requestSubmit();
      })
      .catch((error) => notify(String((error as Error).message ?? error)));
  },
  true,
);
