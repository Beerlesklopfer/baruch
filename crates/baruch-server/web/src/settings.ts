// SPDX-License-Identifier: AGPL-3.0-or-later
// Settings as a popup (Round 9). Without JavaScript every link leads to /einstellungen/<seite>; there
// all forms go by POST to the server and come back with 303.
//
// Widget „settings“ on the <dialog id="settings"> (shell in base.html):
// - A click on an element with [data-settings-open] (link) or Alt+, opens the dialog (showModal)
//   and loads the frame of the page (`?fragment=1`) into [data-settings-content]. If the link carries
//   data-settings-page="<slug>", /einstellungen/<slug>?work=W applies.
// - Links with data-settings-page in the dialog switch the page in the dialog; all other links
//   navigate normally.
// - Forms with method=post in the dialog: fetch POST (application/x-www-form-urlencoded, follow the
//   redirect), from the answer page [data-settings-frame] is taken over (also on 422 – the message stands
//   there). After every success – not after 422 and not after a redirect to an error code
//   (`?fehler=…`, Round 23a, `saved`) –: event `baruch:settings-changed` with detail {page}; the appearance page
//   sets the theme at once (dataset.theme). GET forms (e.g. „Vorschau“ of the outline scheme)
//   on /einstellungen/… also load in the dialog.

import { t } from "./i18n.js";
import { mount, register } from "./widgets.js";

/// Work of the page: from the address, otherwise from the cookie baruch_work.
function currentWork(): string | null {
  const fromUrl = new URL(window.location.href).searchParams.get("work");
  if (fromUrl && /^\d+$/.test(fromUrl)) return fromUrl;
  const cookie = document.cookie.split(";").map((p) => p.trim()).find((p) => p.startsWith("baruch_work="));
  const value = cookie?.slice("baruch_work=".length) ?? "";
  return /^\d+$/.test(value) ? value : null;
}

/// Address of a page in the dialog: /einstellungen/<slug>?work=W.
function pageUrl(slug: string): string {
  const url = new URL(`/einstellungen/${encodeURIComponent(slug)}`, window.location.href);
  const work = currentWork();
  if (work) url.searchParams.set("work", work);
  return url.toString();
}

/// Target of an open link: data-settings-page takes precedence over href.
function targetOf(link: HTMLElement): string {
  const slug = link.dataset.settingsPage;
  if (slug) return pageUrl(slug);
  const href = link.getAttribute("href");
  return href ? new URL(href, window.location.href).toString() : pageUrl("darstellung");
}

/// Does the address belong to the settings of this server?
function isSettings(url: URL): boolean {
  return url.origin === window.location.origin && url.pathname.startsWith("/einstellungen");
}

/// Round 23a: is the form really saved? Only then does the dialog report `baruch:settings-changed`. Rejected
/// means: status outside 2xx (422 with the message on the page) or – after a followed redirect (303) – an
/// address with an error code (`?fehler=<code>` of the connections and sources, `?zeitfehler=1`,
/// `?satzbildfehler=1` …: every parameter name that ends in „fehler“).
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

  /// Take over the frame from an HTML answer; `false` if there is none in it.
  const take = (html: string): boolean => {
    const doc = new DOMParser().parseFromString(html, "text/html");
    const frame = doc.querySelector<HTMLElement>("[data-settings-frame]");
    if (!frame) {
      const alert = doc.querySelector("[role=alert]")?.textContent?.trim();
      show(alert || doc.body?.textContent?.trim().slice(0, 300) || t("widget.settings.unexpected"));
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
      show(t("widget.settings.load_failed", { error: errorText(error) }));
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

  // Round 35: a Lektor (data-rolle on the <body>, konten_ui::apply) must not call `settings.render` – for a Lektor
  // „Einstellungen“ stays an ordinary link to the page Darstellung (cookies in the own browser).
  const lektor = () => document.body.dataset.rolle === "lektor";

  // Opening: links with data-settings-open (header, „Farben …“ in the accordion) and Alt+,.
  document.addEventListener("click", (event) => {
    if (lektor()) return;
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
    if (lektor()) {
      window.location.assign(link ? targetOf(link) : pageUrl("darstellung"));
      return;
    }
    open(link ? targetOf(link) : pageUrl("darstellung"), null);
  });

  // Page change in the dialog.
  content.addEventListener("click", (event) => {
    if (event.defaultPrevented || event.button !== 0 || event.ctrlKey || event.metaKey || event.shiftKey) return;
    const link = (event.target as Element | null)?.closest<HTMLAnchorElement>("a[data-settings-page]");
    if (!link) return;
    event.preventDefault();
    void load(link.href);
  });

  // Forms in the dialog.
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
        show(t("widget.settings.save_failed", { error: errorText(error) }));
      } finally {
        content.removeAttribute("aria-busy");
      }
    })();
  });
});
