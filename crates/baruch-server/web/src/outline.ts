// SPDX-License-Identifier: AGPL-3.0-or-later
// Widget „outline“ (linke Spalte): die einzige Gliederung der Oberfläche (Vertrag Runde 5).
//
// - Ziehen und Ablegen: oberes Drittel der Zeile = davor, Mitte = darunter, unteres Drittel = danach.
//   Verschieben behält die Ebene (davor/danach nur neben einen Punkt derselben Ebene, darunter nur
//   unter einen Punkt genau eine Ebene höher); Ziel und Ebene stehen beim Ziehen unter der
//   Gliederung, ungültige Ziele werden abgewiesen.
// - Alt+Umschalt+←/→ stuft höher bzw. tiefer, Alt+Umschalt+↑/↓ verschiebt (überall außer in
//   Eingabefeldern).
// - Kontextmenü (Rechtsklick, Umschalt+F10, Kontextmenü-Taste, Knopf „⋯“): Umbenennen (F2), neuer
//   Punkt davor/danach/darunter, neuer Teil davor/danach (Runde 14, `level: 0`, nur an Teilen und
//   Kapiteln), höher/tiefer stufen, Löschen… (Entf, mit confirm() und `confirm: true`). F2, Entf und die Kontextmenü-Taste wirken nur bei Fokus in der Gliederung.
// - Umbenennen an Ort und Stelle: Doppelklick auf den gewählten Eintrag oder F2; Enter bestätigt
//   (`outline.rename`), Esc bricht ab. Anlegen mit einem Eingabefeld für den Titel (`outline.add`).
//   Ein leerer, mehrzeiliger oder über 200 Zeichen langer Titel wird mit der Meldung des Servers
//   vorab abgewiesen (Runde 7); das Feld bleibt offen.
// - Nummerierung im Kopf (`form[data-outline-numbering]`): `vorlage` → null, `keine` → "".
//
// Jede schreibende Aktion fragt vorher `leaveEditor()` und meldet nach Erfolg `baruch:navigate` mit
// `{url, asked: true}` (der Arbeitsbereich zeichnet neu). Ohne JavaScript tun dasselbe die Formulare
// je Eintrag (↑ ↓ ← →) und die Seiten /gliederung… an /gliederung/act.
//
// Außerdem fragt es vor dem Speichern im Editor (`form[data-outline-save]`) `outline.check` und
// bestätigt Teilen oder Zusammenführen per confirm(); dann schickt es das Formular mit split/merge = 1.
// Vorher löst es am Formular `baruch:before-save` aus (`detail.wait`: Promises, auf die es wartet –
// z. B. das Satzbild, das seinen Text ins Textfeld schreibt). Der Server bleibt maßgeblich
// (Gültigkeit, Ebenen); hier wird nur vorab abgewiesen.
// Runde 22: Der auslösende Knopf (`submitter`, z. B. „Als neue Fassung speichern“ mit `as_new=1`) geht beim
// erneuten Absenden mit (`requestSubmit(submitter)`), damit der Arbeitsbereich den Modus kennt.

import { call } from "./rpc.js";
import { editorLocked, leaveEditor, lockEditor, register } from "./widgets.js";

type Place = "before" | "after" | "into";

interface Answer {
  node: number;
  /// Fester Schlüssel des danach zu zeigenden Punkts (Runde 8); fehlt er, die Stelle.
  key?: number | null;
}

interface HeadingCheck {
  title: string | null;
  level: number | null;
  split: string[];
  merge: boolean;
  valid: boolean;
  message: string | null;
}

const levelNames = ["Teil", "Kapitel", "Abschnitt", "Unterabschnitt"];
const placeWords: Record<Place, string> = { before: "vor", after: "hinter", into: "unter" };
const keepsLevel = "Verschieben behält die Ebene (Kapitel bleibt Kapitel); erst umstufen (← / →).";

/// Titelregeln wie `baruch_store::check_title` (Runde 7; maßgeblich bleibt der Server): Meldung oder `null`.
function titleProblem(title: string): string | null {
  const trimmed = title.trim();
  if (trimmed === "") return "Ein Gliederungspunkt braucht einen Titel.";
  if (/[\n\r\v\f\u0085\u2028\u2029]/.test(trimmed)) return "Der Titel darf keinen Zeilenumbruch enthalten.";
  const count = [...trimmed].length;
  if (count > 200) return `Der Titel ist zu lang (höchstens 200 Zeichen, hier ${count}).`;
  return null;
}

/// Nach dem nächsten Einhängen den gewählten Eintrag fokussieren (Tastatur bleibt in der Gliederung).
let focusAfterRender = false;

function notify(message: string): void {
  const toast = document.createElement("div");
  toast.className = "toast";
  toast.setAttribute("role", "alert");
  toast.textContent = message;
  document.body.append(toast);
  setTimeout(() => toast.remove(), 5000);
}

/// Neu zeichnen lassen; gefragt (ungespeicherter Text) hat der Aufrufer schon.
function navigate(url: string, asked: boolean): void {
  document.dispatchEvent(new CustomEvent("baruch:navigate", { detail: { url, asked } }));
}

function nodeOf(item: HTMLElement): number {
  return Number(item.dataset.outlineNode);
}

/// Fester Schlüssel (`data-key`): Die Stelle kann veralten, wenn ein anderer Tab umstellt.
function keyOf(item: HTMLElement): number {
  return Number(item.dataset.key);
}

function levelOf(item: HTMLElement): number {
  return Number(item.dataset.level);
}

/// Gliederungsänderung über RPC, danach neu zeichnen lassen. Ungespeicherter Text im Editor ginge
/// beim Neuzeichnen verloren: vorher fragen, bei „Abbrechen“ nichts ändern.
async function act(work: number, method: string, params: Record<string, unknown>): Promise<boolean> {
  if (!leaveEditor()) return false;
  try {
    const result = await call<Answer>(method, { work, ...params });
    focusAfterRender = true;
    // Key-Form (Runde 8); nur ohne Schlüssel in der Antwort die Stelle als Rückfall.
    const place = typeof result.key === "number" ? `key=${result.key}` : `node=${result.node}`;
    navigate(`/?work=${work}&${place}`, true);
    return true;
  } catch (error) {
    notify(String((error as Error).message ?? error));
    return false;
  }
}

interface Verdict {
  ok: boolean;
  text: string;
}

/// Ziel prüfen wie der Server: Die Ebene bleibt (davor/danach nur neben denselben Ebene, darunter nur
/// unter eine Ebene höher), nicht in den eigenen Teilbaum.
function judge(dragged: HTMLElement, target: HTMLElement, place: Place): Verdict {
  if (dragged === target || dragged.contains(target)) {
    return { ok: false, text: "Nicht in den eigenen Teilbaum." };
  }
  const level = levelOf(dragged);
  const targetLevel = levelOf(target);
  const keeps = place === "into" ? targetLevel + 1 === level : targetLevel === level;
  if (!keeps) return { ok: false, text: keepsLevel };
  const title = target.dataset.title ?? "";
  return { ok: true, text: `${placeWords[place]} „${title}“ · Ebene ${level} (${levelNames[level] ?? ""})` };
}

const marks = ["drop-before", "drop-after", "drop-into", "drop-invalid"];

/// Der Eintrag, auf den sich Tasten beziehen: der mit dem Fokus, sonst der gewählte.
function focusedItem(root: HTMLElement): HTMLElement | null {
  const active = document.activeElement as HTMLElement | null;
  const fromFocus = active && root.contains(active) ? active.closest<HTMLElement>("li[data-outline-node]") : null;
  return fromFocus ?? root.querySelector(".tree-entry.selected")?.closest<HTMLElement>("li[data-outline-node]") ?? null;
}

function entryOf(item: HTMLElement): HTMLAnchorElement | null {
  return item.querySelector<HTMLAnchorElement>(":scope > details > summary .tree-entry, :scope > .leaf .tree-entry");
}

function rowOf(item: HTMLElement): HTMLElement | null {
  return item.querySelector<HTMLElement>(":scope > details > summary .outline-row, :scope > .leaf .outline-row");
}

function typing(element: Element | null): boolean {
  const el = element as HTMLElement | null;
  return !!el && (el.isContentEditable || el.matches("textarea, input, select"));
}

/// Eingabefeld, das Klicks und Tasten nicht an Zusammenfassung, Link oder Arbeitsbereich weitergibt.
function field(value: string, placeholder: string, className: string): HTMLInputElement {
  const input = document.createElement("input");
  input.type = "text";
  input.value = value;
  input.placeholder = placeholder;
  input.className = className;
  input.setAttribute("aria-label", placeholder);
  for (const type of ["click", "mousedown", "dblclick"]) {
    input.addEventListener(type, (event) => {
      event.stopPropagation();
      if (type === "click") event.preventDefault();
    });
  }
  return input;
}

/// Umbenennen an Ort und Stelle: Enter → outline.rename, Esc oder Verlassen → abbrechen.
function startRename(root: HTMLElement, item: HTMLElement): void {
  const entry = entryOf(item);
  const row = rowOf(item);
  if (!entry || !row || row.querySelector(".outline-rename")) return;
  const title = item.dataset.title ?? "";
  const input = field(title, "Neuer Titel", "outline-rename");
  entry.hidden = true;
  row.draggable = false;
  entry.after(input);
  let done = false;
  const finish = () => {
    if (done) return;
    done = true;
    input.remove();
    entry.hidden = false;
    row.draggable = true;
  };
  input.addEventListener("keydown", (event) => {
    event.stopPropagation();
    if (event.key === "Escape") {
      event.preventDefault();
      finish();
      entry.focus();
    } else if (event.key === "Enter") {
      event.preventDefault();
      const next = input.value.trim();
      if (next === title) {
        finish();
        entry.focus();
        return;
      }
      const problem = titleProblem(input.value);
      if (problem) {
        // Das Feld bleibt offen; die Meldung ist dieselbe wie die des Servers.
        notify(problem);
        return;
      }
      done = true;
      void act(Number(root.dataset.work), "outline.rename", { node: nodeOf(item), key: keyOf(item), title: next }).then(
        (ok) => {
          if (!ok) {
            done = false;
            finish();
          }
        },
      );
    }
  });
  input.addEventListener("blur", () => finish());
  input.focus();
  input.select();
}

/// Neuer Punkt mit Eingabefeld für den Titel, davor/danach/darunter: Enter → outline.add. Mit
/// `part` ein neuer Teil (`level: 0`, Runde 14) davor bzw. danach.
function startAdd(root: HTMLElement, item: HTMLElement, place: Place, part = false): void {
  root.querySelector(".outline-new")?.remove();
  const title = item.dataset.title ?? "";
  const holder = document.createElement("li");
  holder.className = "outline-new";
  const what = part ? "neuen Teils" : "neuen Punkts";
  const input = field("", `Titel des ${what} ${placeWords[place]} „${title}“`, "");
  holder.append(input);
  if (place === "before") item.before(holder);
  else if (place === "after") item.after(holder);
  else {
    const list = item.querySelector<HTMLElement>(":scope > details > ul");
    if (list) list.append(holder);
    else item.after(holder);
  }
  let done = false;
  const finish = () => {
    if (done) return;
    done = true;
    holder.remove();
  };
  input.addEventListener("keydown", (event) => {
    event.stopPropagation();
    if (event.key === "Escape") {
      event.preventDefault();
      finish();
      entryOf(item)?.focus();
    } else if (event.key === "Enter") {
      event.preventDefault();
      const name = input.value.trim();
      const problem = titleProblem(input.value);
      if (problem) {
        notify(problem);
        return;
      }
      done = true;
      const params: Record<string, unknown> = { target: nodeOf(item), target_key: keyOf(item), place, title: name };
      if (part) params.level = 0;
      void act(Number(root.dataset.work), "outline.add", params).then((ok) => {
        if (!ok) {
          done = false;
          finish();
        }
      });
    }
  });
  input.addEventListener("blur", () => finish());
  input.focus();
}

/// Löschen mit Rückfrage; der Server verlangt `confirm: true`.
function askDelete(root: HTMLElement, item: HTMLElement): void {
  if (item.dataset.children !== undefined) {
    notify("Der Gliederungspunkt hat Unterpunkte; erst verschieben oder löschen.");
    return;
  }
  const title = item.dataset.title ?? "";
  if (!confirm(`„${title}“ mit allen Fassungen und Beiträgen löschen?\nDer Text bleibt im Git-Spiegel lesbar.`)) return;
  void act(Number(root.dataset.work), "outline.delete", { node: nodeOf(item), key: keyOf(item), confirm: true });
}

function setLevel(root: HTMLElement, item: HTMLElement, delta: number): void {
  const level = levelOf(item);
  if (delta < 0 && item.dataset.promote === undefined) return;
  if (delta > 0 && item.dataset.demote === undefined) return;
  void act(Number(root.dataset.work), "outline.set_level", { node: nodeOf(item), key: keyOf(item), level: level + delta });
}

// ---- Kontextmenü ----------------------------------------------------------------------------
function openMenu(root: HTMLElement, item: HTMLElement, x: number | null, y: number | null): void {
  const menu = root.querySelector<HTMLElement>("[data-outline-menu]");
  if (!menu) return;
  const level = levelOf(item);
  const enable: Record<string, boolean> = {
    rename: true,
    "add-before": true,
    "add-after": true,
    "add-into": level < 3,
    "part-before": level <= 1,
    "part-after": level <= 1,
    promote: item.dataset.promote !== undefined,
    demote: item.dataset.demote !== undefined,
    delete: item.dataset.children === undefined,
  };
  menu.querySelectorAll<HTMLButtonElement>("button[data-action]").forEach((button) => {
    button.disabled = !enable[button.dataset.action ?? ""];
  });
  const box = root.getBoundingClientRect();
  const anchor = (rowOf(item) ?? item).getBoundingClientRect();
  const left = (x ?? anchor.left + 24) - box.left;
  const top = (y ?? anchor.bottom) - box.top;
  menu.style.left = `${Math.max(0, left)}px`;
  menu.style.top = `${Math.max(0, top)}px`;
  menu.dataset.for = String(keyOf(item));
  menu.hidden = false;
  menu.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus();
}

function closeMenu(root: HTMLElement, refocus: boolean): void {
  const menu = root.querySelector<HTMLElement>("[data-outline-menu]");
  if (!menu || menu.hidden) return;
  menu.hidden = true;
  if (refocus) {
    const item = root.querySelector<HTMLElement>(`li[data-key="${menu.dataset.for ?? ""}"]`);
    if (item) entryOf(item)?.focus();
  }
}

function runAction(root: HTMLElement, item: HTMLElement, action: string): void {
  switch (action) {
    case "rename":
      startRename(root, item);
      break;
    case "add-before":
      startAdd(root, item, "before");
      break;
    case "add-after":
      startAdd(root, item, "after");
      break;
    case "add-into":
      startAdd(root, item, "into");
      break;
    case "part-before":
      startAdd(root, item, "before", true);
      break;
    case "part-after":
      startAdd(root, item, "after", true);
      break;
    case "promote":
      setLevel(root, item, -1);
      break;
    case "demote":
      setLevel(root, item, 1);
      break;
    case "delete":
      askDelete(root, item);
      break;
  }
}

register("outline", (root) => {
  let dragged: HTMLElement | null = null;
  const hint = root.querySelector<HTMLElement>("[data-outline-hint]");
  const menu = root.querySelector<HTMLElement>("[data-outline-menu]");
  if (focusAfterRender) {
    focusAfterRender = false;
    root.querySelector<HTMLElement>(".tree-entry.selected")?.focus();
  }
  const clear = () => {
    root.querySelectorAll<HTMLElement>(".outline-row").forEach((row) => row.classList.remove(...marks));
  };
  const show = (text: string | null) => {
    if (!hint) return;
    hint.hidden = text === null;
    hint.textContent = text ?? "";
  };
  const spot = (event: DragEvent) => {
    const row = (event.target as Element | null)?.closest<HTMLElement>(".outline-row");
    const item = row?.closest<HTMLElement>("li[data-outline-node]");
    if (!row || !item || !dragged) return null;
    const box = row.getBoundingClientRect();
    const y = (event.clientY - box.top) / Math.max(1, box.height);
    const place: Place = y < 1 / 3 ? "before" : y > 2 / 3 ? "after" : "into";
    return { row, item, place };
  };

  root.addEventListener("dragstart", (event) => {
    const row = (event.target as Element | null)?.closest<HTMLElement>(".outline-row");
    const item = row?.closest<HTMLElement>("li[data-outline-node]");
    if (!row || !item) return;
    dragged = item;
    item.classList.add("dragging");
    if (event.dataTransfer) {
      event.dataTransfer.setData("text/plain", item.dataset.title ?? "");
      event.dataTransfer.effectAllowed = "move";
    }
  });
  root.addEventListener("dragend", () => {
    dragged?.classList.remove("dragging");
    dragged = null;
    clear();
    show(null);
  });
  root.addEventListener("dragover", (event) => {
    const found = spot(event);
    if (!found || !dragged) return;
    const verdict = judge(dragged, found.item, found.place);
    clear();
    found.row.classList.add(verdict.ok ? `drop-${found.place}` : "drop-invalid");
    show(verdict.text);
    if (verdict.ok) {
      event.preventDefault();
      if (event.dataTransfer) event.dataTransfer.dropEffect = "move";
    }
  });
  root.addEventListener("drop", (event) => {
    const found = spot(event);
    const source = dragged;
    if (!found || !source) return;
    event.preventDefault();
    clear();
    show(null);
    if (!judge(source, found.item, found.place).ok) return;
    void act(Number(root.dataset.work), "outline.move", {
      node: nodeOf(source),
      key: keyOf(source),
      target: nodeOf(found.item),
      target_key: keyOf(found.item),
      place: found.place,
    });
  });

  // Kontextmenü: Rechtsklick auf einen Eintrag bzw. der Knopf „⋯“.
  root.addEventListener("contextmenu", (event) => {
    const item = (event.target as Element | null)?.closest<HTMLElement>("li[data-outline-node]");
    if (!item || typing(event.target as Element)) return;
    event.preventDefault();
    openMenu(root, item, event.clientX, event.clientY);
  });
  root.addEventListener("click", (event) => {
    const opener = (event.target as Element | null)?.closest<HTMLElement>("[data-outline-menu-open]");
    if (!opener) return;
    event.preventDefault();
    event.stopPropagation();
    const item = opener.closest<HTMLElement>("li[data-outline-node]");
    if (item) openMenu(root, item, null, null);
  });
  menu?.addEventListener("click", (event) => {
    const button = (event.target as Element | null)?.closest<HTMLButtonElement>("button[data-action]");
    if (!button || button.disabled) return;
    event.stopPropagation();
    const item = root.querySelector<HTMLElement>(`li[data-key="${menu.dataset.for ?? ""}"]`);
    closeMenu(root, false);
    if (item) runAction(root, item, button.dataset.action ?? "");
  });
  menu?.addEventListener("keydown", (event) => {
    const buttons = [...menu.querySelectorAll<HTMLButtonElement>("button[data-action]:not(:disabled)")];
    const at = buttons.indexOf(document.activeElement as HTMLButtonElement);
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      closeMenu(root, true);
    } else if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      event.preventDefault();
      event.stopPropagation();
      const step = event.key === "ArrowDown" ? 1 : -1;
      buttons[(at + step + buttons.length) % buttons.length]?.focus();
    } else if (event.key === "Tab") {
      closeMenu(root, false);
    }
  });
  menu?.addEventListener("focusout", (event) => {
    const next = event.relatedTarget as Node | null;
    if (!next || !menu.contains(next)) closeMenu(root, false);
  });

  // Doppelklick auf den gewählten Eintrag benennt um; ein einfacher Klick darauf öffnet ihn (nach
  // kurzer Wartezeit, damit der Doppelklick nicht vom Neuzeichnen verschluckt wird).
  let pending: number | undefined;
  root.addEventListener(
    "click",
    (event) => {
      const link = (event.target as Element | null)?.closest<HTMLAnchorElement>("a.tree-entry.selected");
      if (!link || event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey || event.altKey) return;
      event.preventDefault();
      event.stopPropagation();
      if (event.detail > 1) return;
      window.clearTimeout(pending);
      pending = window.setTimeout(() => navigate(link.href, false), 300);
    },
    true,
  );
  root.addEventListener("dblclick", (event) => {
    const link = (event.target as Element | null)?.closest<HTMLAnchorElement>("a.tree-entry.selected");
    const item = link?.closest<HTMLElement>("li[data-outline-node]");
    if (!item) return;
    event.preventDefault();
    window.clearTimeout(pending);
    startRename(root, item);
  });

  // F2, Entf, Kontextmenü-Taste und Umschalt+F10 nur bei Fokus in der Gliederung.
  root.addEventListener("keydown", (event) => {
    if (typing(event.target as Element) || event.altKey || event.ctrlKey || event.metaKey) return;
    const menuKey = event.key === "ContextMenu" || (event.key === "F10" && event.shiftKey);
    if (event.key !== "F2" && event.key !== "Delete" && !menuKey) return;
    if (menu && menu.contains(event.target as Node)) return;
    const item = focusedItem(root);
    if (!item) return;
    event.preventDefault();
    if (event.key === "F2") startRename(root, item);
    else if (event.key === "Delete") askDelete(root, item);
    else openMenu(root, item, null, null);
  });
});

// Ein Klick außerhalb schließt das Kontextmenü (einmal für die ganze Seite; der Baum wird neu gezeichnet).
document.addEventListener("pointerdown", (event) => {
  const root = document.querySelector<HTMLElement>("[data-widget=outline]");
  const menu = root?.querySelector<HTMLElement>("[data-outline-menu]");
  if (root && menu && !menu.hidden && !menu.contains(event.target as Node)) closeMenu(root, false);
});

// ---- Tastatur: Alt+Umschalt+Pfeil (einmal für die ganze Seite) -----------------------------
document.addEventListener("keydown", (event) => {
  if (!event.altKey || !event.shiftKey || event.ctrlKey || event.metaKey) return;
  if (!["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(event.key)) return;
  if (typing(document.activeElement)) return;
  const root = document.querySelector<HTMLElement>("[data-widget=outline]");
  const item = root?.querySelector(".tree-entry.selected")?.closest<HTMLElement>("li[data-outline-node]");
  if (!root || !item) return;
  event.preventDefault();
  const work = Number(root.dataset.work);
  const node = nodeOf(item);
  const key = keyOf(item);
  switch (event.key) {
    case "ArrowLeft":
      setLevel(root, item, -1);
      break;
    case "ArrowRight":
      setLevel(root, item, 1);
      break;
    case "ArrowUp":
      if (item.dataset.up !== undefined) {
        const target = { target: Number(item.dataset.up), target_key: Number(item.dataset.upKey) };
        void act(work, "outline.move", { node, key, ...target, place: "before" });
      }
      break;
    case "ArrowDown":
      if (item.dataset.down !== undefined) {
        const target = { target: Number(item.dataset.down), target_key: Number(item.dataset.downKey) };
        void act(work, "outline.move", { node, key, ...target, place: "after" });
      }
      break;
  }
});

// ---- Nummerierung im Kopf der Gliederung ----------------------------------------------------
document.addEventListener(
  "submit",
  (event) => {
    const form = (event.target as Element | null)?.closest<HTMLFormElement>("form[data-outline-numbering]");
    if (!form) return;
    event.preventDefault();
    event.stopImmediatePropagation();
    const data = new FormData(form);
    const raw = String(data.get("numbering") ?? "").trim();
    const numbering = raw === "" || raw === "vorlage" ? null : raw === "keine" ? "" : raw;
    void act(Number(data.get("work")), "outline.set_numbering", {
      node: Number(data.get("node")),
      key: Number(data.get("key")),
      numbering,
    });
  },
  true,
);

// ---- Editor: vor dem Speichern Überschriften prüfen und Teilen/Zusammenführen bestätigen -----
async function checkThenSubmit(form: HTMLFormElement, submitter: HTMLElement | null): Promise<void> {
  // Runde 23a (Codereview web-1): Ein Speichern zur Zeit; bis es fertig ist, bleibt der Editor gesperrt.
  if (editorLocked()) return;
  const release = lockEditor();
  try {
    await checkedSubmit(form, submitter);
  } finally {
    release();
  }
}

async function checkedSubmit(form: HTMLFormElement, submitter: HTMLElement | null): Promise<void> {
  // Andere Widgets (Satzbild) schreiben ihren Stand vorher ins Textfeld; die Quelle der Wahrheit
  // beim Absenden bleibt das Textfeld.
  const wait: Promise<unknown>[] = [];
  form.dispatchEvent(new CustomEvent("baruch:before-save", { detail: { wait } }));
  try {
    await Promise.all(wait);
  } catch (e) {
    notify(String((e as Error).message ?? e));
    return;
  }
  const data = new FormData(form);
  const params = {
    work: Number(data.get("work")),
    node: Number(data.get("node")),
    key: Number(data.get("key")),
    body: String(data.get("body") ?? ""),
  };
  let check: HeadingCheck;
  try {
    check = await call<HeadingCheck>("outline.check", params);
  } catch (error) {
    notify(String((error as Error).message ?? error));
    return;
  }
  if (!check.valid) {
    notify(check.message ?? "Diese Überschriften ergeben keine gültige Gliederung.");
    return;
  }
  const extra: [string, string][] = [];
  if (check.split.length > 0) {
    if (!confirm(`Der Text enthält neue Überschriften: ${check.split.join(", ")}.\nAbschnitt teilen und speichern?`)) return;
    extra.push(["split", "1"]);
  }
  if (check.merge) {
    if (!confirm(`${check.message ?? "Der Text hat keine Überschrift mehr."}\nZusammenführen und speichern?`)) return;
    extra.push(["merge", "1"]);
  }
  const added = extra.map(([name, value]) => {
    const input = document.createElement("input");
    input.type = "hidden";
    input.name = name;
    input.value = value;
    form.append(input);
    return input;
  });
  // Das zweite Absenden geht an den Arbeitsbereich (RPC outline.save) durch.
  form.dataset.checked = "1";
  try {
    if (submitter instanceof HTMLButtonElement && submitter.form === form) form.requestSubmit(submitter);
    else form.requestSubmit();
  } finally {
    delete form.dataset.checked;
    added.forEach((input) => input.remove());
  }
}

document.addEventListener(
  "submit",
  (event) => {
    const form = (event.target as Element | null)?.closest<HTMLFormElement>("form[data-outline-save]");
    if (!form || form.dataset.checked === "1") return;
    event.preventDefault();
    event.stopImmediatePropagation();
    void checkThenSubmit(form, (event as SubmitEvent).submitter ?? null);
  },
  true,
);
