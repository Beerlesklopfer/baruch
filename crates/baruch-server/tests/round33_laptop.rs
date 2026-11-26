// SPDX-License-Identifier: AGPL-3.0-or-later
//! Runde 33 – Laptop-Modus (Aufgabe „laptop“, Vertrag §2/§6): Zustand aus Adresse und Cookies, Attribute an <html>, Seitenleisten.
//!
//! Echter Datenordner im Temp-Ordner mit dem erfundenen Gartenbuch der Server-Tests; kein Netz, kein Eliezer, kein
//! Browser. Geprüft wird, was der Server ausliefert (Attribute an `<html>`, `Set-Cookie`, Markup der Arbeitsseite,
//! CSS, Skript), und das Verhalten von laptop.js in Node auf einem kleinen DOM-Nachbau (fehlt `node`: ÜBERSPRUNGEN).
//! Ehrlich: Ob Leisten und Schubladen im Fenster gut aussehen und ob `:target` im Browser greift, zeigt erst die
//! Sichtprüfung des Integrators (Firefox, mit und ohne JavaScript) – hier stehen Markup und Regeln, keine Pixel.

use crate::common;

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use baruch_server::laptop::{COMPACT_BELOW_PX, LaptopState, Mode, NARROW_BELOW_PX, Width};
use serde_json::{Value, json};

fn script(name: &str) -> &'static str {
    baruch_server::SCRIPTS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, body)| *body)
        .unwrap_or_else(|| panic!("{name} fehlt"))
}

/// Seite holen, auf Wunsch mit Cookie-Kopf; Kopfzeilen und Text der Antwort.
async fn page(server: &common::Server, uri: &str, cookies: Option<&str>) -> (HeaderMap, String) {
    let mut request = Request::get(uri);
    if let Some(cookies) = cookies {
        request = request.header(header::COOKIE, cookies);
    }
    let (status, headers, body) = server.send(request.body(Body::empty()).unwrap()).await;
    assert_eq!(status, StatusCode::OK, "{uri} wird ausgeliefert: {body}");
    (headers, body)
}

/// Das öffnende `<html …>` der Seite.
fn html_tag(page: &str) -> &str {
    let start = page.find("<html").expect("Seite beginnt mit <html>");
    &page[start..start + page[start..].find('>').unwrap() + 1]
}

/// Trägt das Tag das Attribut als ganzes Wort (nicht bloß als Anfang eines anderen)?
fn has_attr(tag: &str, attr: &str) -> bool {
    tag.trim_end_matches('>').split(' ').any(|part| part == attr)
}

fn set_cookies(headers: &HeaderMap) -> Vec<String> {
    headers.get_all(header::SET_COOKIE).iter().map(|v| v.to_str().unwrap().to_owned()).collect()
}

const FLAGS: [&str; 4] = ["data-compact", "data-narrow", "data-rail-left", "data-rail-right"];

#[tokio::test]
async fn the_page_carries_the_state_from_the_cookies_at_the_html_element() {
    let server = common::open().await;
    let work = server.import_garden().await.work;
    let uri = format!("/?work={work}");

    let (_, plain) = page(&server, &uri, None).await;
    let tag = html_tag(&plain);
    assert!(tag.contains("data-laptop=\"auto\""), "ohne Cookies: automatisch – {tag}");
    for flag in FLAGS {
        assert!(
            !has_attr(tag, flag),
            "ohne Cookies kennt der Server die Fensterbreite nicht und zeichnet breit, also ohne {flag}: {tag}"
        );
    }

    let (_, on) = page(&server, &uri, Some("baruch_laptop=on")).await;
    let tag = html_tag(&on);
    assert!(
        tag.contains("data-laptop=\"on\" data-compact data-narrow data-rail-left data-rail-right"),
        "„an“ gilt unabhängig von der Breite: Band kompakt, schmal, beide Leisten – in der festen Reihenfolge: {tag}"
    );

    let (_, medium) = page(&server, &uri, Some("baruch_vw=m")).await;
    let tag = html_tag(&medium);
    assert!(has_attr(tag, "data-compact"), "mittlere Breite (1280–1599 px) und automatisch: Band kompakt – {tag}");
    for flag in &FLAGS[1..] {
        assert!(!has_attr(tag, flag), "mittlere Breite lässt die Seitenspalten offen, kein {flag}: {tag}");
    }

    let (_, off) = page(&server, &uri, Some("baruch_vw=s; baruch_laptop=off")).await;
    let tag = html_tag(&off);
    assert!(tag.contains("data-laptop=\"off\""), "der gemerkte Modus steht am Element: {tag}");
    for flag in FLAGS {
        assert!(!has_attr(tag, flag), "„aus“ hält auch ein schmales Fenster im vollen Zustand, kein {flag}: {tag}");
    }

    let (_, small) = page(&server, &uri, Some("baruch_vw=s")).await;
    let tag = html_tag(&small);
    assert!(
        tag.contains("data-laptop=\"auto\" data-compact data-narrow data-rail-left data-rail-right"),
        "schmales Fenster (unter 1280 px) und automatisch: wie „an“ – {tag}"
    );

    let (_, left) = page(&server, &uri, Some("baruch_lrail=1")).await;
    let tag = html_tag(&left);
    assert!(has_attr(tag, "data-rail-left"), "baruch_lrail=1 klappt die Gliederung ein: {tag}");
    for flag in ["data-compact", "data-narrow", "data-rail-right"] {
        assert!(!has_attr(tag, flag), "eine einzeln eingeklappte Leiste ändert sonst nichts, kein {flag}: {tag}");
    }

    let (_, junk) = page(&server, &uri, Some("baruch_laptop=\"><b>; baruch_vw=x; baruch_rrail=ja")).await;
    let tag = html_tag(&junk);
    assert!(tag.contains("data-laptop=\"auto\""), "ungültige Cookies gelten als nicht vorhanden: {tag}");
    assert!(
        !tag.contains("<b") && !FLAGS.iter().any(|f| has_attr(tag, f)),
        "nichts aus dem Cookie steht im Tag: {tag}"
    );
}

#[tokio::test]
async fn a_key_in_the_address_works_in_the_same_answer_and_is_remembered() {
    let server = common::open().await;

    // Arbeitsseite ohne Werk in der Adresse: Sie merkt schon das Werk (baruch_work) – das Cookie des Laptop-Modus
    // muss daneben stehen, nicht an seiner Stelle.
    let (headers, body) = page(&server, "/?laptop=on", Some("baruch_laptop=off")).await;
    let tag = html_tag(&body);
    assert!(
        tag.contains("data-laptop=\"on\" data-compact data-narrow data-rail-left data-rail-right"),
        "?laptop=on gilt schon für diese Antwort, vor dem Cookie „off“: {tag}"
    );
    let cookies = set_cookies(&headers);
    assert!(
        cookies.iter().any(|c| c == "baruch_laptop=on; Path=/; Max-Age=31536000; SameSite=Lax"),
        "die Wahl wird ein Jahr gemerkt: {cookies:?}"
    );
    assert!(
        cookies.iter().any(|c| c.starts_with("baruch_work=")),
        "das Cookie der Arbeitsseite bleibt daneben erhalten (anhängen, nicht ersetzen): {cookies:?}"
    );

    let work = server.import_garden().await.work;
    let (headers, body) = page(&server, &format!("/?work={work}&lrail=1"), None).await;
    assert!(has_attr(html_tag(&body), "data-rail-left"), "?lrail=1 klappt die Gliederung in derselben Antwort ein");
    assert!(
        set_cookies(&headers).iter().any(|c| c == "baruch_lrail=1; Path=/; Max-Age=31536000; SameSite=Lax"),
        "und wird gemerkt: {:?}",
        set_cookies(&headers)
    );

    let (headers, body) =
        page(&server, &format!("/?work={work}&laptop=auto&rrail=0"), Some("baruch_laptop=on; baruch_rrail=1")).await;
    let tag = html_tag(&body);
    assert!(
        tag.contains("data-laptop=\"auto\"") && !FLAGS.iter().any(|f| has_attr(tag, f)),
        "?laptop=auto und ?rrail=0 heben die gemerkte Wahl in derselben Antwort auf: {tag}"
    );
    let cookies = set_cookies(&headers);
    for name in ["baruch_laptop", "baruch_rrail"] {
        assert!(
            cookies.iter().any(|c| c == &format!("{name}=; Path=/; Max-Age=0; SameSite=Lax")),
            "die Vorgabe ist das fehlende Cookie – {name} wird gelöscht: {cookies:?}"
        );
    }

    // Ungültiges aus der Adresse: wirkt nicht, wird nicht gemerkt und steht nicht am Element.
    let (headers, body) = page(&server, &format!("/?work={work}&laptop=%22%3E%3Cscript%3E&lrail=7"), None).await;
    let tag = html_tag(&body);
    assert!(
        tag.contains("data-laptop=\"auto\"") && !has_attr(tag, "data-rail-left"),
        "ungültige Werte übergangen: {tag}"
    );
    assert!(
        !set_cookies(&headers).iter().any(|c| c.starts_with("baruch_laptop") || c.starts_with("baruch_lrail")),
        "für ungültige Werte gibt es kein Set-Cookie: {:?}",
        set_cookies(&headers)
    );

    // Auch Seiten ohne Arbeitsfläche tragen den Zustand (das Band ist überall dasselbe).
    let (headers, body) = page(&server, "/werke?laptop=off", Some("baruch_vw=s")).await;
    assert!(html_tag(&body).contains("data-laptop=\"off\""), "die Werkliste trägt den Modus: {}", html_tag(&body));
    assert!(
        set_cookies(&headers).iter().any(|c| c.starts_with("baruch_laptop=off;")),
        "und merkt ihn wie die Arbeitsseite: {:?}",
        set_cookies(&headers)
    );
}

#[tokio::test]
async fn the_workspace_keeps_its_hooks_and_gets_a_rail_per_side_usable_without_javascript() {
    let server = common::open().await;
    let work = server.import_garden().await.work;
    let (_, html) = page(&server, &format!("/?work={work}"), None).await;

    // Haken, an denen Widgets, Tests und fremde Seiten hängen (Vertrag §6: wörtlich erhalten).
    for hook in [
        "<main class=\"workspace\" data-widget=\"workspace\">",
        "id=\"pane-tree\" aria-label=\"Gliederung\" data-zoom-zone=\"tree\"",
        "<section class=\"pane\" id=\"pane-editor\" aria-label=\"Editor\">",
        "id=\"pane-assistants\" aria-label=\"Assistenten\" data-zoom-zone=\"assistants\"",
        "data-widget=\"splitter\" data-side=\"left\"",
        "data-widget=\"splitter\" data-side=\"right\"",
    ] {
        assert_eq!(html.matches(hook).count(), 1, "{hook} steht genau einmal auf der Arbeitsseite");
    }

    let main = &html[html.find("<main class=\"workspace\"").unwrap()..];
    let main = &main[..main.find("</main>").expect("Ende der Arbeitsfläche")];
    let inner = main[main.find('>').unwrap() + 1..].trim_start();
    assert!(
        inner.starts_with("<nav class=\"pane\" id=\"pane-tree\""),
        "die Gliederung bleibt erstes Kind des Rasters – das Widget „splitter“ misst die linke Spalte über \
         firstElementChild: {}",
        &inner[..inner.len().min(120)]
    );
    assert!(
        main.trim_end().ends_with("</aside>"),
        "die rechte Spalte bleibt letztes Kind des Rasters (lastElementChild im Widget „splitter“)"
    );

    // Je Seite eine Leiste: ein gewöhnlicher Link auf die Spalte – ohne JavaScript öffnet er sie über :target.
    for (side, target, label) in [("left", "pane-tree", "Gliederung"), ("right", "pane-assistants", "Assistenten")] {
        let rail = format!("<a class=\"rail\" href=\"#{target}\" data-rail-open=\"{side}\" aria-controls=\"{target}\"");
        assert_eq!(main.matches(&rail).count(), 1, "genau eine Leiste für {label} mit Link auf #{target}");
        let at = main.find(&rail).unwrap();
        let link = &main[at..at + main[at..].find("</a>").unwrap()];
        assert!(
            link.contains(&format!("<span class=\"rail-label\">{label}</span>")),
            "die Leiste nennt ihre Spalte beim Namen (nicht nur ein Pfeil): {link}"
        );
        // Schließen-Link in der Spalte selbst: Er wechselt das Sprungziel auf den Editor, damit fällt :target weg.
        let pane = &main[main.find(&format!("id=\"{target}\"")).unwrap()..];
        let bar = &pane[pane.find("<div class=\"rail-bar\">").unwrap_or_else(|| panic!("Bedienzeile in {label}"))..];
        let bar = &bar[..bar.find("</div>").unwrap()];
        assert!(
            bar.contains("class=\"tool tiny rail-close\" href=\"#pane-editor\" data-rail-close"),
            "{label}: „Schließen“ ist ein Link auf #pane-editor (ohne JavaScript schließt das die Schublade): {bar}"
        );
        assert!(bar.contains(">Schließen</a>"), "{label}: der Link heißt „Schließen“: {bar}");
        let key = if side == "left" { "lrail" } else { "rrail" };
        assert!(
            bar.contains(&format!("{key}=1\" data-rail-collapse=\"{side}\"")),
            "{label}: „Einklappen“ ist ein Link mit ?{key}=1 – der Server merkt es als Cookie, auch ohne JavaScript: {bar}"
        );
    }
    assert_eq!(
        main.matches(
            "<a class=\"rail-scrim\" href=\"#pane-editor\" data-rail-close tabindex=\"-1\" aria-hidden=\"true\">"
        )
        .count(),
        1,
        "die Fläche neben der Schublade ist ein Link auf #pane-editor, aber kein Halt für Tab und Vorleser"
    );

    // Die Bedienzeile steckt in den Fragmenten: ui.render ersetzt den Inhalt der Spalten, sie muss das überstehen.
    let ui = server.rpc("ui.render", json!({ "work": work })).await;
    assert!(ui["error"].is_null(), "ui.render antwortet: {ui}");
    for (pane, side) in [("tree", "left"), ("assistants", "right")] {
        let fragment = ui["result"][pane].as_str().unwrap();
        assert!(
            fragment.contains(&format!("data-rail-collapse=\"{side}\"")) && fragment.contains("data-rail-close"),
            "nach dem Neuzeichnen der Spalte „{pane}“ sind „Einklappen“ und „Schließen“ noch da"
        );
    }
}

/// Alle `@media`-Köpfe eines Stylesheets.
fn media_queries(css: &str) -> Vec<&str> {
    css.match_indices("@media").map(|(at, _)| &css[at..at + css[at..].find('{').unwrap()]).collect()
}

#[tokio::test]
async fn the_style_hangs_on_the_attributes_and_not_on_the_window_width() {
    let server = common::open().await;
    let (status, served) = server.get("/static/app.css").await;
    assert_eq!(status, StatusCode::OK, "Stil wird ausgeliefert");
    for selector in ["html[data-rail-left]", "html[data-rail-right]", "html[data-narrow]", "html[data-compact]"] {
        assert!(served.contains(selector), "das ausgelieferte CSS kennt den Zustand {selector}");
    }
    assert!(
        served.contains("html[data-rail-left] #pane-tree,\nhtml[data-rail-right] #pane-assistants { display: none; }"),
        "eine eingeklappte Spalte ist verborgen, an ihrer Stelle steht die Leiste"
    );
    assert!(
        served.contains("html:not(.js)[data-rail-left] #pane-tree:target")
            && served.contains("html:not(.js)[data-rail-right] #pane-assistants:target"),
        "ohne JavaScript öffnet :target die eingeklappte Spalte als Schublade"
    );
    let laptop = include_str!("../static/laptop.css");
    // Runde 35: Hinter laptop.css folgt nur noch konten.css (Konten und Rollen: eigene Klassen, keine Regel für die
    // Zustände des Laptop-Modus – das prüft round35_oberflaeche).
    let konten = include_str!("../static/konten.css");
    let before_konten = served.trim_end().strip_suffix(konten.trim_end()).unwrap_or(&served);
    assert!(
        before_konten.trim_end().ends_with(laptop.trim_end()),
        "laptop.css steht hinter dem Grundgerüst und überschreibt es damit (gleiche Spezifität, spätere Regel gewinnt)"
    );

    // Eigene Dateien der Aufgabe: Grundgerüst (app.css) und Laptop-Modus (laptop.css).
    let base = include_str!("../static/app.css");
    for (name, css) in [("app.css", base), ("laptop.css", laptop)] {
        assert!(
            !css.contains(".topbar"),
            "{name}: Die Kopfzeile ist das Menüband (band.css) – tote .topbar-Regeln würden nur verwirren"
        );
        for media in media_queries(css) {
            assert!(
                !media.contains("width"),
                "{name}: keine Media-Query auf die Fensterbreite ({media}) – die Breite misst laptop.ts und schreibt \
                 sie als Attribut an <html>; zwei Quellen (Attribut und Media-Query) liefen auseinander, sobald der \
                 Mensch „an“ oder „aus“ wählt"
            );
        }
    }
    assert!(
        base.contains("grid-template-columns: var(--left) 6px 1fr 6px var(--right);"),
        "das Grundraster bleibt drei Spalten mit zwei Grenzen (Widget „splitter“ setzt --left/--right)"
    );
    assert!(
        base.contains(".rail, .rail-scrim { display: none; }"),
        "im Ruhezustand (beide Spalten offen) sind Leisten und Fläche verborgen"
    );
    assert!(
        laptop.contains("html[data-rail-left] .rail[data-rail-open=\"left\"],\nhtml[data-rail-right] .rail[data-rail-open=\"right\"] { display: flex; }"),
        "sichtbar wird nur die Leiste der eingeklappten Seite"
    );
}

/// Skript in Node ausführen: alle ausgelieferten Skripte als Module in einem Temp-Ordner, `probe.js` gibt JSON aus.
/// Fehlt `node`, `None` (der Test meldet „ÜBERSPRUNGEN“).
fn node(probe: &str) -> Option<Value> {
    let dir = tempfile::tempdir().unwrap();
    for (name, body) in baruch_server::SCRIPTS {
        std::fs::write(dir.path().join(name), body).unwrap();
    }
    std::fs::write(dir.path().join("package.json"), "{\"type\": \"module\"}").unwrap();
    std::fs::write(dir.path().join("probe.js"), probe).unwrap();
    let out = match std::process::Command::new("node").arg("probe.js").current_dir(dir.path()).output() {
        Ok(out) => out,
        Err(_) => {
            eprintln!("ÜBERSPRUNGEN: node fehlt");
            return None;
        }
    };
    assert!(out.status.success(), "node: {}", String::from_utf8_lossy(&out.stderr));
    Some(
        serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&out.stdout))),
    )
}

#[test]
fn the_widget_shares_thresholds_and_formula_with_the_server() {
    let main = script("main.js");
    assert!(main.contains("./laptop.js"), "main.js lädt laptop.js, sonst misst niemand die Fensterbreite");
    let laptop = script("laptop.js");
    assert!(
        laptop.contains(&format!("COMPACT_BELOW_PX = {COMPACT_BELOW_PX};")),
        "die Schwelle „kompakt“ ({COMPACT_BELOW_PX} px) steht im Widget wie im Server – sonst zeichnete der Server \
         nach dem Neuladen anders, als das Widget es gerade zeigt"
    );
    assert!(
        laptop.contains(&format!("NARROW_BELOW_PX = {NARROW_BELOW_PX};")),
        "ebenso die Schwelle „schmal“ ({NARROW_BELOW_PX} px)"
    );
    assert!(laptop.contains("window.innerWidth"), "gemessen wird window.innerWidth (Vertrag §2)");
    assert!(
        laptop.contains("new CustomEvent(\"baruch:laptop\""),
        "Wechsel meldet das Widget als Ereignis baruch:laptop (band.ts klappt danach seine Gruppen)"
    );
    assert!(!laptop.contains("band-group"), "laptop.ts fasst die Gruppen des Bands nie an (Vertrag §6)");

    // Reine Funktionen gegen die Rust-Seite: Breitenklasse an den Grenzen und alle Kombinationen der Formel.
    let pixels = [0u32, 1023, 1279, 1280, 1366, 1599, 1600, 1920, 3840];
    let Some(result) = node(&format!(
        "import {{ widthClass, effective, htmlAttrs, parseMode }} from './laptop.js';\n\
         const text = (mode, w, l, r) => htmlAttrs(mode, effective(mode, w, l, r)).filter(([, v]) => v !== null)\n\
           .map(([n, v]) => n === 'data-laptop' ? `${{n}}=\"${{v}}\"` : n).join(' ');\n\
         const all = [];\n\
         for (const mode of ['auto', 'on', 'off']) for (const w of ['w', 'm', 's'])\n\
           for (const [l, r] of [[false, false], [true, false], [false, true], [true, true]]) all.push(text(mode, w, l, r));\n\
         console.log(JSON.stringify({{ classes: {pixels:?}.map(widthClass), all,\n\
           modes: ['on', 'off', 'auto', 'quer', '\"><b>', null, undefined].map(parseMode) }}));"
    )) else {
        return;
    };
    let classes: Vec<&str> = pixels.iter().map(|px| Width::from_px(*px).cookie()).collect();
    assert_eq!(
        result["classes"],
        json!(classes),
        "widthClass im Widget liefert für {pixels:?} dieselbe Klasse wie Width::from_px (Grenzen 1280 und 1600 gehören \
         zur breiteren Klasse)"
    );
    let mut all = Vec::new();
    for mode in [Mode::Auto, Mode::On, Mode::Off] {
        for width in [Width::Wide, Width::Medium, Width::Small] {
            for (lrail, rrail) in [(false, false), (true, false), (false, true), (true, true)] {
                all.push(LaptopState { mode, width, lrail, rrail }.html_attrs());
            }
        }
    }
    assert_eq!(
        result["all"],
        json!(all),
        "für alle 36 Kombinationen aus Modus, Breite und Leisten setzt das Widget dieselben Attribute wie der Server \
         (html_attrs) – eine Quelle der Wahrheit an <html>"
    );
    assert_eq!(
        result["modes"],
        json!(["on", "off", "auto", "auto", "auto", "auto", "auto"]),
        "Unbekanntes im Cookie ist „automatisch“, wie im Server"
    );
}

/// DOM-Nachbau für laptop.js: <html> mit Attributen, Cookies, zwei Spalten, die Schalter des Bands. Das Skript führt
/// die Schritte aus und gibt nach jedem den Zustand aus.
const HARNESS: &str = r#"
const camel = (s) => s.replace(/-([a-z])/g, (_, c) => c.toUpperCase());
const focusLog = [];
class Element {
  constructor(name, data = {}, attrs = {}) {
    this.name = name; this.dataset = data; this.attrs = new Map(Object.entries(attrs));
    this.classList = { contains: () => false }; this.isConnected = true; this.tabIndex = 0;
  }
  setAttribute(n, v) { this.attrs.set(n, String(v)); }
  getAttribute(n) { return this.attrs.has(n) ? this.attrs.get(n) : null; }
  removeAttribute(n) { this.attrs.delete(n); }
  matches(sel) {
    return sel.split(",").some((s) => { const m = /^\s*\[data-([a-z-]+)\]\s*$/.exec(s); return !!m && camel(m[1]) in this.dataset; });
  }
  closest(sel) { return this.matches(sel) ? this : null; }
  contains(other) { return other === this; }
  focus() { focusLog.push(this.name); }
  getClientRects() { return [1]; }
}
globalThis.Element = Element;
const root = new Element("html");
const jar = new Map();
const panes = { "pane-tree": new Element("pane-tree"), "pane-assistants": new Element("pane-assistants") };
const controls = {
  auto: new Element("auto", { laptopSet: "auto" }), on: new Element("on", { laptopSet: "on" }),
  off: new Element("off", { laptopSet: "off" }),
  left: new Element("toggle-left", { railToggle: "left" }, { href: "?work=3&tab=ansicht&lrail=1" }),
  right: new Element("toggle-right", { railToggle: "right" }, { href: "?work=3&tab=ansicht&rrail=1" }),
  railLeft: new Element("rail-left", { railOpen: "left" }),
  collapseRight: new Element("collapse-right", { railCollapse: "right" }),
  text: new Element("text"),
};
const handlers = {}; const windowHandlers = {}; const events = [];
globalThis.document = {
  documentElement: root,
  get cookie() { return [...jar].map(([k, v]) => `${k}=${v}`).join("; "); },
  set cookie(text) {
    const [pair, ...rest] = text.split(";").map((p) => p.trim());
    const [name, value] = pair.split("=");
    if (rest.includes("Max-Age=0")) jar.delete(name); else jar.set(name, value);
  },
  querySelectorAll: (sel) => Object.values(controls).filter((c) => c.matches(sel)),
  querySelector: () => null,
  getElementById: (id) => panes[id] ?? null,
  addEventListener: (type, fn) => { handlers[type] = fn; },
  dispatchEvent: (event) => { events.push([event.type, event.detail]); return true; },
};
globalThis.window = {
  innerWidth: 1366, location: { hash: "", pathname: "/", search: "" },
  addEventListener: (type, fn) => { windowHandlers[type] = fn; },
};
Object.defineProperty(globalThis, "navigator", { value: { cookieEnabled: true }, configurable: true });
globalThis.history = { state: null, replaceState() {} };
globalThis.requestAnimationFrame = (fn) => { fn(); return 0; };

const FLAGS = ["data-compact", "data-narrow", "data-rail-left", "data-rail-right"];
const steps = [];
const snap = (step) => steps.push({
  step,
  attrs: [`data-laptop="${root.getAttribute("data-laptop")}"`, ...FLAGS.filter((f) => root.getAttribute(f) !== null)].join(" "),
  cookies: Object.fromEntries([...jar].sort()),
  drawer: root.dataset.drawer ?? null,
  pressed: Object.fromEntries(["auto", "on", "off", "left", "right"].map((k) => [k, controls[k].getAttribute("aria-pressed")])),
  hrefs: [controls.left.getAttribute("href"), controls.right.getAttribute("href")],
  events: events.length,
  focus: [...focusLog],
});
let prevented = 0;
const click = (target) => handlers.click({ target, button: 0, preventDefault() { prevented += 1; } });
const resize = (px) => { window.innerWidth = px; windowHandlers.resize(); };

jar.set("baruch_rrail", "1");
await import("./laptop.js");
snap("laden bei 1366 px, rechte Spalte gemerkt eingeklappt");
resize(1024); snap("verkleinert auf 1024 px");
click(controls.left); snap("schmal: Seitenleiste Gliederung im Band");
handlers.keydown({ key: "Escape", target: controls.text, defaultPrevented: false, preventDefault() {} });
snap("Esc");
click(controls.railLeft); snap("schmal: Leiste Gliederung");
click(controls.text); snap("Klick daneben");
click(controls.off); snap("Laptop-Modus aus");
click(controls.left); snap("breit: Gliederung einklappen");
click(controls.railLeft); snap("breit: Leiste klappt dauerhaft auf");
click(controls.right); snap("breit: Assistenten zeigen");
click(controls.collapseRight); snap("Einklappen in der rechten Spalte");
resize(1920); click(controls.auto); snap("1920 px, automatisch");
click(controls.on); snap("an");
console.log(JSON.stringify({ steps, events, focusLog, prevented }));
"#;

#[test]
fn the_widget_measures_remembers_and_opens_drawers() {
    let Some(result) = node(HARNESS) else {
        return;
    };
    let step = |name: &str| -> Value {
        result["steps"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["step"] == name)
            .unwrap_or_else(|| panic!("Schritt {name} fehlt: {result}"))
            .clone()
    };
    let attrs = |mode, width, lrail, rrail| json!(LaptopState { mode, width, lrail, rrail }.html_attrs());

    let s = step("laden bei 1366 px, rechte Spalte gemerkt eingeklappt");
    assert_eq!(
        s["attrs"],
        attrs(Mode::Auto, Width::Medium, false, true),
        "beim Laden misst das Widget 1366 px (mittel): Band kompakt, dazu die gemerkte rechte Leiste – dieselben \
         Attribute, die der Server für diese Cookies schriebe"
    );
    assert_eq!(s["cookies"]["baruch_vw"], "m", "die Breitenklasse steht im Cookie baruch_vw für die nächste Seite");
    assert_eq!(
        result["events"][0],
        json!(["baruch:laptop", { "compact": true, "narrow": false }]),
        "beim Laden meldet das Widget den Zustand einmal als baruch:laptop"
    );
    assert_eq!(
        s["pressed"],
        json!({ "auto": "true", "on": "false", "off": "false", "left": "true", "right": "false" }),
        "gedrückt sind „Automatisch“ und die sichtbare Gliederung; die eingeklappte rechte Spalte nicht"
    );
    assert_eq!(
        s["hrefs"],
        json!(["?work=3&tab=ansicht&lrail=1", "?work=3&tab=ansicht&rrail=0"]),
        "der Rückfall-Link führt jeweils zum anderen Zustand (sichtbar → einklappen, eingeklappt → zeigen); der Rest \
         der Adresse bleibt"
    );

    let s = step("verkleinert auf 1024 px");
    assert_eq!(
        s["attrs"],
        attrs(Mode::Auto, Width::Small, false, true),
        "unter 1280 px klappen beide Seitenspalten ein (data-narrow), ohne Neuladen"
    );
    assert_eq!(s["cookies"]["baruch_vw"], "s", "das Cookie folgt der Größenänderung");
    assert_eq!(s["events"], 2, "der Wechsel wird gemeldet");
    assert_eq!(
        result["events"][1],
        json!(["baruch:laptop", { "compact": true, "narrow": true }]),
        "mit dem neuen Zustand"
    );

    let s = step("schmal: Seitenleiste Gliederung im Band");
    assert_eq!(
        s["drawer"], "left",
        "im schmalen Zustand öffnet der Schalter die Gliederung als Schublade über dem Text"
    );
    assert_eq!(
        s["attrs"],
        attrs(Mode::Auto, Width::Small, false, true),
        "eingeklappt bleibt sie dabei (keine Spalte neben dem Text)"
    );
    assert!(s["cookies"].get("baruch_lrail").is_none(), "eine Schublade ist vorübergehend und wird nicht gemerkt: {s}");
    assert_eq!(s["pressed"]["left"], "true", "solange die Schublade offen ist, gilt die Gliederung als sichtbar");

    let s = step("Esc");
    assert_eq!(s["drawer"], Value::Null, "Esc schließt die Schublade");
    assert_eq!(
        s["focus"],
        json!(["pane-tree", "toggle-left"]),
        "beim Öffnen geht der Fokus in die Spalte, beim Schließen zurück zum Auslöser"
    );

    assert_eq!(step("schmal: Leiste Gliederung")["drawer"], "left", "auch die Leiste selbst öffnet die Schublade");
    let s = step("Klick daneben");
    assert_eq!(s["drawer"], Value::Null, "ein Klick neben die Schublade schließt sie");
    assert_eq!(
        s["focus"],
        json!(["pane-tree", "toggle-left", "pane-tree"]),
        "dabei bleibt der Fokus, wohin der Mensch geklickt hat (nur das zweite Öffnen kam dazu)"
    );

    let s = step("Laptop-Modus aus");
    assert_eq!(s["attrs"], attrs(Mode::Off, Width::Small, false, true), "„aus“ zeichnet auch bei 1024 px voll");
    assert_eq!(s["cookies"]["baruch_laptop"], "off", "die Wahl steht im Cookie baruch_laptop");
    assert_eq!(s["pressed"]["off"], "true", "der gewählte Schalter ist gedrückt");
    assert_eq!(s["pressed"]["auto"], "false", "die anderen nicht");
    assert_eq!(
        result["events"][2],
        json!(["baruch:laptop", { "compact": false, "narrow": false }]),
        "Wechsel gemeldet"
    );

    let s = step("breit: Gliederung einklappen");
    assert_eq!(
        s["attrs"],
        attrs(Mode::Off, Width::Small, true, true),
        "nicht schmal: der Schalter klappt die Spalte dauerhaft ein"
    );
    assert_eq!(s["cookies"]["baruch_lrail"], "1", "gemerkt im Cookie baruch_lrail");
    assert_eq!(s["drawer"], Value::Null, "ohne Schublade");
    assert_eq!(s["events"], 3, "compact/narrow sind gleich geblieben: kein weiteres Ereignis");

    let s = step("breit: Leiste klappt dauerhaft auf");
    assert_eq!(s["attrs"], attrs(Mode::Off, Width::Small, false, true), "die Leiste zeigt die Spalte wieder");
    assert!(s["cookies"].get("baruch_lrail").is_none(), "sichtbar ist das fehlende Cookie: {s}");

    let s = step("breit: Assistenten zeigen");
    assert_eq!(s["attrs"], attrs(Mode::Off, Width::Small, false, false), "die gemerkte rechte Leiste klappt auf");
    assert!(s["cookies"].get("baruch_rrail").is_none(), "Cookie baruch_rrail gelöscht: {s}");
    let s = step("Einklappen in der rechten Spalte");
    assert_eq!(
        s["attrs"],
        attrs(Mode::Off, Width::Small, false, true),
        "„Einklappen“ in der Spalte wirkt wie der Schalter"
    );
    assert_eq!(s["cookies"]["baruch_rrail"], "1", "und wird gemerkt");

    let s = step("1920 px, automatisch");
    assert_eq!(s["attrs"], attrs(Mode::Auto, Width::Wide, false, true), "breit und automatisch: volles Band");
    assert!(s["cookies"].get("baruch_laptop").is_none(), "„automatisch“ löscht das Cookie: {s}");
    assert_eq!(s["cookies"]["baruch_vw"], "w", "Breitenklasse breit");

    let s = step("an");
    assert_eq!(s["attrs"], attrs(Mode::On, Width::Wide, false, true), "„an“ gilt auch im breiten Fenster");
    assert_eq!(s["cookies"]["baruch_laptop"], "on", "gemerkt");
    assert_eq!(
        result["prevented"], 9,
        "jeder abgefangene Klick auf einen Schalter ersetzt den Seitenwechsel (preventDefault); der Klick daneben nicht"
    );
}

#[test]
fn zoom_reset_and_ctrl_zero_set_the_levels_back() {
    // Ehrlich: zoom.js hängt beim Laden an window und document und braucht für echtes Verhalten einen Browser; hier
    // steht nur, dass die Wege im ausgelieferten Skript verdrahtet sind. Die Sichtprüfung macht der Integrator.
    let zoom = script("zoom.js");
    assert!(
        zoom.contains("closest(\"[data-zoom-reset]\")"),
        "ein Klick auf den Haken [data-zoom-reset] (Knopf „Zoom zurücksetzen“ im Reiter Ansicht) wird abgefangen"
    );
    let reset = &zoom[zoom.find("function resetAll()").expect("resetAll setzt alle Zonen zurück")..];
    let reset = &reset[..reset.find("\n}").unwrap()];
    assert!(
        reset.contains("levels.clear()") && reset.contains("[data-zoomed]") && reset.contains("save()"),
        "Zurücksetzen vergisst alle Stufen, nimmt jeder gezoomten Zone die Stufe und löscht das Cookie: {reset}"
    );
    assert!(
        zoom.contains("event.key === \"0\"") && zoom.contains("change(hovered, 1)"),
        "Strg+0 setzt weiterhin die Zone unter dem Mauszeiger zurück"
    );
    let panes = script("panes.js");
    assert!(
        panes.contains("grid.firstElementChild") && panes.contains("grid.lastElementChild"),
        "das Widget „splitter“ misst die Spalten weiter als erstes und letztes Kind – darum stehen die Leisten \
         zwischen den Spalten (Test the_workspace_keeps_its_hooks…)"
    );
    assert!(
        panes.contains("register(\"theme\"") && panes.contains("store.set(\"theme\""),
        "der Thema-Umschalter (Widget theme) bleibt, wie er war"
    );
}
