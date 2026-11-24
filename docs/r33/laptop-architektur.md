<!-- Runde 33, Aufgabe „laptop“: Abschnitt für docs/Architektur.md. -->

### Laptop-Modus (Runde 33)

Eine Quelle der Wahrheit: **Attribute an `<html>`**. Der Server schreibt sie beim Ausliefern, das Widget hält sie
im Browser aktuell, CSS und andere Widgets lesen nur sie. Es gibt für die Modi **keine Media-Query auf die
Fensterbreite** – sonst gäbe es zwei Wahrheiten, sobald der Mensch „An“ oder „Aus“ wählt.

| Attribut | Bedeutung | gesetzt, wenn |
|---|---|---|
| `data-laptop="auto\|on\|off"` | Wahl des Menschen | immer |
| `data-compact` | Band kompakt | `on`, oder `auto` und Breite `m`/`s` |
| `data-narrow` | schmal: beide Seitenspalten sind Leisten | `on`, oder `auto` und Breite `s` |
| `data-rail-left` | Gliederung eingeklappt | schmal, oder von Hand (`lrail`) |
| `data-rail-right` | rechte Spalte eingeklappt | schmal, oder von Hand (`rrail`) |
| `data-drawer="left\|right"` | Schublade offen (nur laptop.ts, nie vom Server) | solange die Schublade offen ist |

#### Server: `src/laptop.rs`

- `LaptopState::from_request(cookie_header, query)` liest den Zustand: Schlüssel der Adresse `laptop`
  (`auto|on|off`), `lrail`, `rrail` (`0|1`) gelten **vor** den Cookies `baruch_laptop` (`on|off`, fehlt =
  automatisch), `baruch_lrail`, `baruch_rrail` (`1`, fehlt = sichtbar). Die Breitenklasse kommt nur aus dem Cookie
  `baruch_vw` (`w|m|s`, fehlt = breit). Ungültige Werte werden übergangen; steht ein Schlüssel mehrfach in der
  Adresse, gilt der letzte gültige (das Band hängt seine Schlüssel hinten an).
- `compact()`, `narrow()`, `rail_left()`, `rail_right()` sind die Formel aus der Tabelle; `html_attrs()` schreibt
  sie in fester Reihenfolge und **nur aus festen Zeichenketten** – nichts aus Adresse oder Cookie gelangt in die
  Seite.
- `set_cookies(query)` liefert fertige `Set-Cookie`-Werte für gültige Schlüssel der Adresse
  (`…; Path=/; Max-Age=31536000; SameSite=Lax`); die Vorgaben (`laptop=auto`, `lrail=0`, `rrail=0`) löschen das
  Cookie (`Max-Age=0`).
- Schwellen: `COMPACT_BELOW_PX = 1600`, `NARROW_BELOW_PX = 1280`; `Width::from_px` ordnet zu.
- Verdrahtung: `consent::inject` (Schicht über allen HTML-Antworten) bildet den Zustand vor `next.run`,
  `band::apply` setzt `html_attrs()` an der Marke ` data-band-fill` ein, die Cookies werden an die Antwort
  **angehängt** (`/` setzt schon `baruch_work`). Keine neue Route, keine RPC-Methode, keine Migration.

#### Browser: `web/src/laptop.ts`

- Hängt sich beim Laden selbst am `document` ein (Seiten ohne Arbeitsfläche haben kein Element dafür); das Widget
  `laptop` bleibt registriert und löst dasselbe aus.
- Misst `window.innerWidth` beim Laden und bei `resize` (gebündelt über `requestAnimationFrame`), schreibt die
  Klasse ins Cookie `baruch_vw` und setzt die Attribute nach derselben Formel (`effective`, `htmlAttrs`). Die
  Schwellen stehen als `COMPACT_BELOW_PX`/`NARROW_BELOW_PX` auch im Skript; `tests/round33_laptop.rs` vergleicht
  sie und alle 36 Kombinationen der Formel mit der Rust-Seite.
- **Ereignis `baruch:laptop`** am `document`, `detail: {compact, narrow}`: einmal beim Laden, danach bei jedem
  Wechsel von `compact` oder `narrow`. band.ts klappt danach seine Gruppen. Das Ereignis beim Laden geht hinaus,
  bevor Widgets eingehängt sind – wer es braucht, liest beim Einhängen zusätzlich die Attribute an `<html>`.
  laptop.ts fasst die Gruppen des Bands nie an; band.ts misst nie Breiten.
- Klicks (ein Hörer am `document`): `[data-laptop-set]` (Modus, Cookie `baruch_laptop`), `[data-rail-toggle]`
  (Schalter im Band), `[data-rail-open]` (Leiste), `[data-rail-collapse]` („Einklappen“ in der Spalte),
  `[data-rail-close]` („Schließen“, Fläche neben der Schublade). Das Widget pflegt `aria-pressed` der Schalter
  (Leiste sichtbar = gedrückt) und die Adresse des Rückfall-Links (`lrail=0|1`). Gibt es die Spalte auf der Seite
  nicht, bleibt der Link ein gewöhnlicher Link.
- Schublade: im schmalen Zustand `data-drawer` an `<html>`; Fokus in die Spalte, Esc und „Schließen“ geben ihn
  dem Auslöser zurück, ein Klick daneben schließt ohne Fokuswechsel, die Wahl eines Abschnitts (`a.tree-entry`)
  schließt die Gliederung. Nicht schmal klappen Leiste und Schalter die Spalte dauerhaft (Cookies `baruch_lrail`,
  `baruch_rrail`).
- `web/src/zoom.ts`: `[data-zoom-reset]` setzt alle Zonen zurück und löscht `baruch_zoom`.

#### Markup und Stil

- `templates/workspace.html`: zwischen den Spalten je eine Leiste `a.rail[data-rail-open]` (`href="#pane-tree"`
  bzw. `#pane-assistants`) und die Fläche `a.rail-scrim` (`href="#pane-editor"`). Die Spalten bleiben erstes und
  letztes Kind des Rasters, weil das Widget `splitter` sie so misst. `id="pane-tree"`, `id="pane-editor"`,
  `id="pane-assistants"`, die Splitter und alle `data-zoom-zone` sind unverändert.
- `templates/fragments/tree.html`, `assistants.html`: oben die Zeile `div.rail-bar` mit „Einklappen“
  (`?lrail=1` bzw. `?rrail=1`) und „Schließen“ (`#pane-editor`). Sie steht in den Fragmenten, weil `ui.render`
  den Inhalt der Spalten ersetzt.
- `static/app.css`: Grundgerüst (Raster, Leisten im Ruhezustand verborgen). Die Regeln der alten Kopfzeile
  (`.topbar`) und die beiden Breiten-Media-Queries (Kopfzeile unter 1000 px, Spalten untereinander unter 900 px)
  sind entfallen.
- `static/laptop.css` (in `STYLE` zuletzt): alles an `data-compact`, `data-narrow`, `data-rail-*` außerhalb des
  Bands. Ohne JavaScript öffnet `:target` die Schublade (`html:not(.js)…:target`); mit JavaScript gelten diese
  Regeln nicht, damit eine Adresse mit `#pane-tree` die Schublade nicht offen hält. Die Fläche daneben braucht
  ohne JavaScript `:has()`; fehlt das im Browser, bleibt „Schließen“.

#### Grenzen

- Ohne JavaScript fehlt `baruch_vw`: „Automatisch“ zeichnet breit, der Mensch stellt „An“.
- Beim allerersten Aufruf mit JavaScript kennt der Server die Breite ebenfalls noch nicht; die Seite kommt breit
  und wird vom Widget sofort umgestellt (ein kurzer Sprung, danach stimmt das Cookie).
- Das Cookie `baruch_vw` gilt je Browser, nicht je Fenster: Zwei verschieden breite Fenster überschreiben es
  gegenseitig; das Widget stellt jedes Fenster nach dem Laden richtig.
