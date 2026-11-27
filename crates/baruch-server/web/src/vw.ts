// SPDX-License-Identifier: AGPL-3.0-or-later
// Early script „vw“ (Round 36): window width right already on the first request after a resize.
//
// The server draws the laptop state from the cookie baruch_vw (src/laptop.rs). The browser writes the cookie;
// if the window had a different size since the last visit (new window, another screen, size changed while
// no page of Baruch was open), the first request still carries the old width. laptop.ts compensates for that – but
// as a module only after the page is built: it jumped briefly.
//
// This script is NOT a module: base.html loads it classically in the <head>, so it runs before the browser draws
// the page for the first time. If the width class of the cookie does not match the window, it rewrites the
// cookie (the server draws the next request right) and sets the attributes on <html> by the same
// formula as server and laptop.ts – the page appears right at once. If the cookie is right, it does nothing.
// Without JavaScript everything stays as before: no cookie, „automatisch“ draws wide.
//
// Thresholds and formula also stand in src/laptop.rs and laptop.ts; tests keep all three equal. Deliberately without
// import/export (classic script) and encapsulated in a function (no global names).

(() => {
  const COMPACT_BELOW_PX = 1600;
  const NARROW_BELOW_PX = 1280;
  const root = document.documentElement;
  const px = window.innerWidth;
  if (!(px > 0)) return;
  const width = px < NARROW_BELOW_PX ? "s" : px < COMPACT_BELOW_PX ? "m" : "w";
  const cookie = (name: string): string | null => {
    const prefix = `baruch_${name}=`;
    for (const part of document.cookie.split(";")) {
      const pair = part.trim();
      if (pair.startsWith(prefix)) return pair.slice(prefix.length);
    }
    return null;
  };
  // The server has drawn with this width: nothing to do.
  if (cookie("vw") === width) return;
  document.cookie = `baruch_vw=${width}; Path=/; Max-Age=31536000; SameSite=Lax`;
  // Mode and rails as laptop.ts reads them: the cookie (a key of the address already stands in it thanks to
  // Set-Cookie); if cookies are blocked, the mode drawn by the server.
  const saved = cookie("laptop") ?? (navigator.cookieEnabled ? null : root.getAttribute("data-laptop"));
  const mode = saved === "on" || saved === "off" ? saved : "auto";
  const compact = mode === "on" || (mode === "auto" && width !== "w");
  const narrow = mode === "on" || (mode === "auto" && width === "s");
  const attrs: [string, boolean][] = [
    ["data-compact", compact],
    ["data-narrow", narrow],
    ["data-rail-left", narrow || cookie("lrail") === "1"],
    ["data-rail-right", narrow || cookie("rrail") === "1"],
  ];
  root.setAttribute("data-laptop", mode);
  for (const [name, on] of attrs) {
    if (on) root.setAttribute(name, "");
    else root.removeAttribute(name);
  }
})();
