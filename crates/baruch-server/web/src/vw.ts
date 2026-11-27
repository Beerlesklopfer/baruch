// SPDX-License-Identifier: AGPL-3.0-or-later
// Frühes Skript „vw“ (Runde 36): Fensterbreite schon beim ersten Aufruf nach einer Größenänderung richtig.
//
// Der Server zeichnet den Laptop-Zustand aus dem Cookie baruch_vw (src/laptop.rs). Das Cookie schreibt der Browser;
// war das Fenster seit dem letzten Besuch anders groß (neues Fenster, anderer Bildschirm, Größe geändert, während
// keine Seite von Baruch offen war), trägt die erste Anfrage noch die alte Breite. laptop.ts gleicht das aus – aber
// als Modul erst nach dem Aufbau der Seite: Sie sprang kurz um.
//
// Dieses Skript ist KEIN Modul: base.html lädt es klassisch im <head>, es läuft also, bevor der Browser die Seite
// zum ersten Mal zeichnet. Stimmt die Breitenklasse des Cookies nicht mit dem Fenster überein, schreibt es das
// Cookie neu (die nächste Anfrage zeichnet der Server richtig) und setzt die Attribute an <html> nach derselben
// Formel wie Server und laptop.ts – die Seite erscheint gleich richtig. Stimmt das Cookie, tut es nichts.
// Ohne JavaScript bleibt alles wie bisher: kein Cookie, „automatisch“ zeichnet breit.
//
// Schwellen und Formel stehen auch in src/laptop.rs und laptop.ts; Tests halten alle drei gleich. Bewusst ohne
// import/export (klassisches Skript) und in einer Funktion gekapselt (keine globalen Namen).

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
  // Der Server hat mit dieser Breite gezeichnet: nichts zu tun.
  if (cookie("vw") === width) return;
  document.cookie = `baruch_vw=${width}; Path=/; Max-Age=31536000; SameSite=Lax`;
  // Modus und Leisten wie laptop.ts sie liest: das Cookie (ein Schlüssel der Adresse steht dank Set-Cookie schon
  // darin); sind Cookies gesperrt, der vom Server gezeichnete Modus.
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
