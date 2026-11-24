// SPDX-License-Identifier: AGPL-3.0-or-later
// Einstellungen als Popup (Runde 9). Ohne JavaScript führt jeder Link auf /einstellungen/<seite>; dort
// gehen alle Formulare per POST an den Server und kommen mit 303 zurück.
//
// Widget „settings“ am <dialog id="settings"> (Hülle in base.html):
// - Klick auf ein Element mit [data-settings-open] (Link) oder Alt+, öffnet den Dialog (showModal)
//   und lädt den Rahmen der Seite (`?fragment=1`) in [data-settings-content]. Trägt der Link
//   data-settings-page="<slug>", gilt /einstellungen/<slug>?work=W.
// - Links mit data-settings-page im Dialog wechseln die Seite im Dialog; alle anderen Links
//   navigieren normal.
// - Formulare mit method=post im Dialog: fetch POST (application/x-www-form-urlencoded, Weiterleitung
//   folgen), aus der Antwortseite wird [data-settings-frame] übernommen (auch bei 422 – dort steht die
//   Meldung). Nach jedem Erfolg – nicht nach 422 und nicht nach einer Weiterleitung auf einen Fehlercode
//   (`?fehler=…`, Runde 23a, `saved`) –: Ereignis `baruch:settings-changed` mit detail {page}; die Darstellung
//   setzt das Thema sofort (dataset.theme). GET-Formulare (z. B. „Vorschau“ des Gliederungsschemas)
//   auf /einstellungen/… laden ebenfalls im Dialog.

import { mount, register } from "./widgets.js";

/// Werk der Seite: aus der Adresse, sonst aus dem Cookie baruch_work.
function currentWork(): string | null {
  const fromUrl = new URL(window.location.href).searchParams.get("work");
  if (fromUrl && /^\d+$/.test(fromUrl)) return fromUrl;
  const cookie = document.cookie.split(";").map((p) => p.trim()).find((p) => p.startsWith("baruch_work="));
  const value = cookie?.slice("baruch_work=".length) ?? "";
  return /^\d+$/.test(value) ? value : null;
}

/// Adresse einer Seite im Dialog: /einstellungen/<slug>?work=W.
function pageUrl(slug: string): string {
  const url = new URL(`/einstellungen/${encodeURIComponent(slug)}`, window.location.href);
  const work = currentWork();
  if (work) url.searchParams.set("work", work);
  return url.toString();
}

/// Ziel eines Öffnen-Links: data-settings-page geht vor href.
function targetOf(link: HTMLElement): string {
  const slug = link.dataset.settingsPage;
  if (slug) return pageUrl(slug);
  const href = link.getAttribute("href");
  return href ? new URL(href, window.location.href).toString() : pageUrl("darstellung");
}

/// Gehört die Adresse zu den Einstellungen dieses Servers?
function isSettings(url: URL): boolean {
  return url.origin === window.location.origin && url.pathname.startsWith("/einstellungen");
}

/// Runde 23a: Ist das Formular wirklich gespeichert? Nur dann meldet der Dialog `baruch:settings-changed`. Abgelehnt
/// heißt: Status außerhalb 2xx (422 mit der Meldung auf der Seite) oder – nach gefolgter Weiterleitung (303) – eine
/// Adresse mit einem Fehlercode (`?fehler=<code>` der Verbindungen und Quellen, `?zeitfehler=1`, `?satzbildfehler=1` …:
/// jeder Parametername, der auf „fehler“ endet).
export function saved(response: { ok: boolean; url: string }): boolean {
  if (!response.ok) return false;
  let url: URL;
  try {
    url = new URL(response.url, window.location.href);
  } catch {
    return true;
  }
  for (const key of url.searchParams.keys()) {
    if (key.toLowerCase().endsWith("fehler")) return false;
  }
  return true;
}

function errorText(error: unknown): string {
  return String((error as Error)?.message ?? error);
}

register("settings", (element) => {
  if (!(element instanceof HTMLDialogElement)) return;
  const dialog = element;
  const content = dialog.querySelector<HTMLElement>("[data-settings-content]");
  if (!content) return;
  let opener: HTMLElement | null = null;

  const show = (message: string) => {
    const p = document.createElement("p");
    p.className = "notice error";
    p.setAttribute("role", "alert");
    p.textContent = message;
    content.replaceChildren(p);
  };

  /// Rahmen aus einer HTML-Antwort übernehmen; `false`, wenn keiner darin steht.
  const take = (html: string): boolean => {
    const doc = new DOMParser().parseFromString(html, "text/html");
    const frame = doc.querySelector<HTMLElement>("[data-settings-frame]");
    if (!frame) {
      const alert = doc.querySelector("[role=alert]")?.textContent?.trim();
      show(alert || doc.body?.textContent?.trim().slice(0, 300) || "Unerwartete Antwort des Servers.");
      return false;
    }
    content.replaceChildren(document.importNode(frame, true));
    mount(content);
    content.querySelector<HTMLElement>("[aria-current=page]")?.focus({ preventScroll: true });
    return true;
  };

  const load = async (target: string) => {
    const url = new URL(target, window.location.href);
    url.searchParams.set("fragment", "1");
    content.setAttribute("aria-busy", "true");
    try {
      const response = await fetch(url, { headers: { Accept: "text/html" } });
      take(await response.text());
    } catch (error) {
      show(`Einstellungen ließen sich nicht laden: ${errorText(error)}`);
    } finally {
      content.removeAttribute("aria-busy");
    }
  };

  const open = (target: string, from: HTMLElement | null) => {
    opener = from ?? (document.activeElement instanceof HTMLElement ? document.activeElement : null);
    if (!dialog.open) dialog.showModal();
    void load(target);
  };

  dialog.addEventListener("close", () => {
    opener?.focus();
    opener = null;
  });

  // Öffnen: Links mit data-settings-open (Kopfzeile, „Farben …“ im Akkordeon) und Alt+,.
  document.addEventListener("click", (event) => {
    if (event.defaultPrevented || event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey) return;
    const link = (event.target as Element | null)?.closest<HTMLElement>("[data-settings-open]");
    if (!link || dialog.contains(link)) return;
    event.preventDefault();
    open(targetOf(link), link);
  });
  document.addEventListener("keydown", (event) => {
    if (!event.altKey || event.ctrlKey || event.metaKey || event.key !== ",") return;
    event.preventDefault();
    const link = document.querySelector<HTMLElement>("[data-settings-open]");
    open(link ? targetOf(link) : pageUrl("darstellung"), null);
  });

  // Seitenwechsel im Dialog.
  content.addEventListener("click", (event) => {
    if (event.defaultPrevented || event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey) return;
    const link = (event.target as Element | null)?.closest<HTMLAnchorElement>("a[data-settings-page]");
    if (!link) return;
    event.preventDefault();
    void load(link.href);
  });

  // Formulare im Dialog.
  content.addEventListener("submit", (event) => {
    const form = event.target;
    if (!(form instanceof HTMLFormElement)) return;
    const submitter = event.submitter instanceof HTMLElement ? event.submitter : null;
    const button = submitter instanceof HTMLButtonElement || submitter instanceof HTMLInputElement ? submitter : null;
    const method = (button?.getAttribute("formmethod") ?? form.getAttribute("method") ?? "get").toLowerCase();
    const action = new URL(button?.getAttribute("formaction") ?? form.getAttribute("action") ?? "", window.location.href);
    const data = new FormData(form, submitter);
    const fields = new URLSearchParams();
    data.forEach((value, key) => {
      if (typeof value === "string") fields.append(key, value);
    });
    if (method === "get") {
      if (!isSettings(action)) return;
      event.preventDefault();
      fields.forEach((value, key) => action.searchParams.append(key, value));
      void load(action.toString());
      return;
    }
    if (method !== "post") return;
    event.preventDefault();
    const page = content.querySelector<HTMLElement>("[data-settings-frame]")?.dataset.page ?? "";
    void (async () => {
      content.setAttribute("aria-busy", "true");
      try {
        const response = await fetch(action, {
          method: "POST",
          headers: { "Content-Type": "application/x-www-form-urlencoded" },
          body: fields.toString(),
          redirect: "follow",
        });
        const taken = take(await response.text());
        if (saved(response) && taken) {
          const theme = fields.get("theme");
          if (theme === "system") delete document.documentElement.dataset.theme;
          else if (theme === "light" || theme === "dark") document.documentElement.dataset.theme = theme;
          document.dispatchEvent(new CustomEvent("baruch:settings-changed", { detail: { page } }));
        }
      } catch (error) {
        show(`Speichern fehlgeschlagen: ${errorText(error)}`);
      } finally {
        content.removeAttribute("aria-busy");
      }
    })();
  });
});
