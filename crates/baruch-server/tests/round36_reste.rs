// SPDX-License-Identifier: AGPL-3.0-or-later
//! Round 36 „Reste“, owner reste: 0.9.0 notes and items from „offen nach R24“.
//!
//! Works and texts are freely invented; no model, no network. Scripts run in Node on a small DOM mock (if `node` is
//! missing: ÜBERSPRUNGEN). The tests for the personas (plain question, proposal for the chapter) are with their mocks
//! in `round22_befunde` and `kritik_zyklus`.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{Request, StatusCode, header};
use baruch_server::laptop::{COMPACT_BELOW_PX, LaptopState, Mode, NARROW_BELOW_PX, Width};
use baruch_server::xref::outline_mentions;
use serde_json::{Value, json};

use crate::common;

fn result(answer: &Value) -> &Value {
    assert!(answer["error"].is_null(), "RPC sollte gelingen: {answer}");
    &answer["result"]
}

/// Undo the escaping of the templates (only for reading addresses in the assertions).
fn readable(page: &str) -> String {
    page.replace("&#x2f;", "/").replace("&amp;", "&").replace("&quot;", "\"")
}

fn part<'a>(page: &'a str, start: &str, end: &str) -> &'a str {
    let from = page.find(start).unwrap_or_else(|| panic!("„{start}“ fehlt in der Seite"));
    let to = page[from..].find(end).unwrap_or_else(|| panic!("„{end}“ fehlt hinter „{start}“"));
    &page[from..from + to]
}

// ---- Counter of the revisions in the band -------------------------------------------------------------------

const FERRY: &str = "#set document(title: \"Die Seilfähre\", author: \"Ole Strom\")\n\n\
= Am Anleger\n\nDas Boot liegt fest. Der Fährmann wartet auf den Wind.\n\n\
= Drüben\n\nEin Licht brennt im Zollhaus.\n";

/// Note from the cross-check 0.9.0: the counter in the editor header showed „(+ n ausgeblendet)“, the counter in the
/// band did not – there „2/2“ looked as if there were only two revisions.
#[tokio::test]
async fn the_revision_counter_in_the_band_shows_the_hidden_ones_like_the_editor_head() {
    let server = common::open().await;
    let work = server.import(FERRY, "seilfaehre.typ").await.work;
    let store = &server.state.store;
    for word in ["kleine", "alte", "neue"] {
        let loaded = store.load_work(work).await.unwrap();
        let node = &loaded.nodes[0];
        let body = node.revisions[node.current_index().unwrap()].body.replacen("Boot", &format!("{word} Boot"), 1);
        store.save_human(work, 0, &body, "Ole").await.unwrap().unwrap();
    }
    let key = result(&server.rpc("node.get", json!({ "work": work, "node": 0 })).await)["key"].as_i64().unwrap();
    let uri = format!("/?work={work}&key={key}");
    let band_of = |page: &str| part(page, "aria-label=\"Fassung wechseln\"", "form=\"editor-form\"").to_owned();
    let head_of = |page: &str| part(page, "<nav class=\"switcher\"", "</nav>").to_owned();

    let (_, before) = server.get(&uri).await;
    let before = readable(&before);
    assert!(band_of(&before).contains(">4/4</span>"), "Vorbedingung: ohne Marke zählt das Band alle vier Fassungen");
    assert!(!band_of(&before).contains("ausgeblendet"), "ohne Marke kein Zusatz im Band");

    let form = format!("method=revision.fold_set&work={work}&key={key}&rev=4&view=edit");
    let (status, _) = server.post_form("/act", &form).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "Marke „bis hier ausblenden“ an der aktuellen Fassung 4");

    let (_, closed) = server.get(&uri).await;
    let closed = readable(&closed);
    let (band, head) = (band_of(&closed), head_of(&closed));
    assert!(head.contains("(+ 3 ausgeblendet) ▸"), "Vorbedingung: der Editor-Kopf nennt die drei Ausgeblendeten");
    assert!(band.contains(">1/1</span>"), "das Band zählt wie der Kopf nur die sichtbaren Fassungen: {band}");
    assert!(band.contains("(+ 3 ausgeblendet) ▸"), "und nennt jetzt wie der Kopf die Ausgeblendeten: {band}");
    let show = format!("href=\"/?work={work}&key={key}&rev=4&fassungen=alle\"");
    assert!(
        band.contains(&show) && head.contains(&show),
        "der Zusatz im Band ist derselbe Link wie im Kopf (öffnet den Schalter ohne JavaScript): {band}"
    );
    assert_eq!(
        closed.matches("data-rev-fold=\"closed\"").count(),
        1,
        "der Schalter des Kopfs bleibt der eine mit data-rev-fold (das Band trägt data-band-rev-fold)"
    );

    let (_, open) = server.get(&format!("{uri}&rev=4&fassungen=alle")).await;
    let open = readable(&open);
    let band = band_of(&open);
    assert!(band.contains(">4/4</span>"), "Schalter offen: das Band zählt wieder alle: {band}");
    assert!(band.contains("3 ältere verbergen ▾"), "und bietet wie der Kopf das Verbergen an: {band}");
    assert!(
        band.contains(&format!("href=\"/?work={work}&key={key}&rev=3&fassungen=alle\"")),
        "‹ im Band trägt den offenen Schalter weiter (vorher fiel er beim Blättern über das Band zu): {band}"
    );
}

// ---- Cookie baruch_vw on the first call --------------------------------------------------------------------

fn script(name: &str) -> &'static str {
    baruch_server::SCRIPTS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, body)| *body)
        .unwrap_or_else(|| panic!("{name} fehlt in den ausgelieferten Skripten"))
}

/// Run a script in Node (all delivered scripts in a temp directory); `None` if `node` is missing.
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

#[tokio::test]
async fn the_early_script_is_loaded_before_the_modules_and_pages_without_scripts_stay_without() {
    let server = common::open().await;
    let work = server.import(FERRY, "seilfaehre.typ").await.work;
    let (status, page) = server.get(&format!("/?work={work}")).await;
    assert_eq!(status, StatusCode::OK, "die Arbeitsseite lädt");
    let head = part(&page, "<head>", "</head>");
    let early = head.find("<script src=\"/static/js/vw.js\"></script>").expect("das frühe Skript steht im <head>");
    let modules = head.find("<script type=\"module\" src=\"/static/js/main.js\"></script>").expect("main.js");
    assert!(early < modules, "klassisch und vor den Modulen: Es läuft, bevor die Seite zum ersten Mal gezeichnet wird");
    let (status, js) = server.get("/static/js/vw.js").await;
    assert_eq!(status, StatusCode::OK, "vw.js wird ausgeliefert");
    assert!(
        !js.lines().any(|line| line.starts_with("import ") || line.starts_with("export ")),
        "vw.js ist ein klassisches Skript ohne import/export (ein Modul liefe erst nach dem Aufbau der Seite)"
    );
    assert!(
        js.contains(&format!("COMPACT_BELOW_PX = {COMPACT_BELOW_PX};"))
            && js.contains(&format!("NARROW_BELOW_PX = {NARROW_BELOW_PX};")),
        "vw.js rechnet mit denselben Schwellen wie der Server"
    );
    // Unchanged without JavaScript: without the cookie the server draws wide, with the cookie according to the cookie.
    let tag = part(&page, "<html", ">");
    assert!(
        tag.contains("data-laptop=\"auto\"") && !tag.contains("data-compact") && !tag.contains("data-narrow"),
        "ohne Cookie baruch_vw (ohne JavaScript) bleibt „automatisch“ breit: {tag}"
    );
    let request =
        Request::get(format!("/?work={work}")).header(header::COOKIE, "baruch_vw=s").body(Body::empty()).unwrap();
    let (_, _, narrow) = server.send(request).await;
    assert!(part(&narrow, "<html", ">").contains("data-narrow"), "mit Cookie zeichnet der Server wie bisher schmal");
    // The typeset view works without scripts – also without the early one.
    let (status, typeset) = server.get(&format!("/werke/{work}/satz")).await;
    if status == StatusCode::OK {
        assert!(!typeset.contains("/static/js/vw.js"), "eine Seite ohne Skripte bekommt auch dieses nicht");
    }
}

/// DOM mock for vw.js: <html> with the attributes that the server has drawn for the old cookies, cookies and window
/// width. Every case loads the script anew and reports attributes and cookies afterwards.
const VW_HARNESS: &str = r#"
const FLAGS = ["data-compact", "data-narrow", "data-rail-left", "data-rail-right"];
const cases = CASES;
const out = [];
let n = 0;
for (const c of cases) {
  const attrs = new Map(c.drawn);
  const jar = new Map(Object.entries(c.cookies));
  let writes = 0;
  globalThis.document = {
    documentElement: {
      setAttribute: (k, v) => attrs.set(k, String(v)),
      getAttribute: (k) => (attrs.has(k) ? attrs.get(k) : null),
      removeAttribute: (k) => attrs.delete(k),
    },
    get cookie() { return [...jar].map(([k, v]) => `${k}=${v}`).join("; "); },
    set cookie(text) {
      writes += 1;
      const [pair, ...rest] = text.split(";").map((p) => p.trim());
      const [name, value] = pair.split("=");
      if (rest.includes("Max-Age=0")) jar.delete(name); else jar.set(name, value);
    },
  };
  globalThis.window = { innerWidth: c.px };
  Object.defineProperty(globalThis, "navigator", { value: { cookieEnabled: true }, configurable: true });
  n += 1;
  await import(`./vw.js?fall=${n}`);
  out.push({
    name: c.name,
    attrs: [`data-laptop="${attrs.get("data-laptop")}"`, ...FLAGS.filter((f) => attrs.has(f))].join(" "),
    vw: jar.get("baruch_vw") ?? null,
    writes,
  });
}
console.log(JSON.stringify(out));
"#;

/// Note of the review R33: after a change of size (new window, other screen) the first request still carried the old
/// width; the page jumped as soon as laptop.ts ran as a module. The early script corrects cookie and attributes before
/// the first paint – by the same formula as the server.
#[test]
fn the_early_script_corrects_cookie_and_attributes_on_the_first_call_after_a_resize() {
    let drawn = |state: &LaptopState| -> Value {
        // What the server has written to <html> for the old cookies, as pairs [name, value].
        let mut pairs = vec![json!(["data-laptop", state.mode.name()])];
        for (on, name) in [
            (state.compact(), "data-compact"),
            (state.narrow(), "data-narrow"),
            (state.rail_left(), "data-rail-left"),
            (state.rail_right(), "data-rail-right"),
        ] {
            if on {
                pairs.push(json!([name, ""]));
            }
        }
        json!(pairs)
    };
    let wide = LaptopState { mode: Mode::Auto, width: Width::Wide, lrail: false, rrail: false };
    let small = LaptopState { mode: Mode::Auto, width: Width::Small, lrail: false, rrail: false };
    let cases = json!([
        { "name": "breit gemerkt, Fenster jetzt 1024 px", "px": 1024, "cookies": { "baruch_vw": "w" }, "drawn": drawn(&wide) },
        { "name": "schmal gemerkt, Fenster jetzt 1920 px", "px": 1920, "cookies": { "baruch_vw": "s" }, "drawn": drawn(&small) },
        { "name": "noch nie gemessen, Fenster 1366 px", "px": 1366, "cookies": {}, "drawn": drawn(&wide) },
        { "name": "breit gemerkt, Fenster jetzt 1500 px, rechte Spalte gemerkt eingeklappt", "px": 1500,
          "cookies": { "baruch_vw": "w", "baruch_rrail": "1" },
          "drawn": drawn(&LaptopState { rrail: true, ..wide }) },
        { "name": "Laptop-Modus aus, Fenster jetzt 1024 px", "px": 1024,
          "cookies": { "baruch_vw": "w", "baruch_laptop": "off" },
          "drawn": drawn(&LaptopState { mode: Mode::Off, ..wide }) },
        { "name": "Cookie stimmt schon", "px": 1024, "cookies": { "baruch_vw": "s" }, "drawn": drawn(&small) },
        { "name": "genau an der Schwelle 1280 px", "px": 1280, "cookies": { "baruch_vw": "s" }, "drawn": drawn(&small) },
    ]);
    let Some(seen) = node(&VW_HARNESS.replace("CASES", &cases.to_string())) else {
        return;
    };
    let attrs = |mode, width, lrail, rrail| json!(LaptopState { mode, width, lrail, rrail }.html_attrs());
    let expected = [
        ("s", attrs(Mode::Auto, Width::Small, false, false), 1, "verkleinert: sofort schmal, Cookie neu"),
        ("w", attrs(Mode::Auto, Width::Wide, false, false), 1, "vergrößert: sofort breit, Cookie neu"),
        ("m", attrs(Mode::Auto, Width::Medium, false, false), 1, "erster Besuch mit JavaScript: Cookie entsteht"),
        ("m", attrs(Mode::Auto, Width::Medium, false, true), 1, "die gemerkte rechte Leiste bleibt eingeklappt"),
        (
            "s",
            attrs(Mode::Off, Width::Small, false, false),
            1,
            "„aus“ bleibt breit gezeichnet, nur das Cookie ändert sich",
        ),
        ("s", attrs(Mode::Auto, Width::Small, false, false), 0, "stimmt das Cookie, schreibt das Skript nichts"),
        (
            "m",
            attrs(Mode::Auto, Width::Medium, false, false),
            1,
            "1280 px ist schon die mittlere Klasse (wie im Server)",
        ),
    ];
    let seen = seen.as_array().expect("ein Ergebnis je Fall");
    assert_eq!(seen.len(), expected.len(), "jeder Fall lief");
    for (got, (vw, attrs, writes, why)) in seen.iter().zip(expected) {
        let name = got["name"].as_str().unwrap();
        assert_eq!(got["vw"], vw, "{name}: Cookie baruch_vw – {why}");
        assert_eq!(
            got["attrs"], attrs,
            "{name}: Attribute an <html> wie der Server sie für diese Breite schriebe – {why}"
        );
        assert_eq!(got["writes"], writes, "{name}: Zahl der Schreibzugriffe auf das Cookie – {why}");
    }
    let source = script("vw.js");
    assert!(
        source.contains("Max-Age=31536000") && source.contains("SameSite=Lax"),
        "Cookie wie die übrigen der Oberfläche"
    );
}

// ---- „Sicherung angelegt“ only once -------------------------------------------------------------------------

async fn open_in_subdir() -> common::Server {
    let root = tempfile::tempdir().unwrap();
    let data = root.path().join("daten");
    std::fs::create_dir(&data).unwrap();
    let state = Arc::new(baruch_server::AppState::open(&data).await.expect("Datenordner lässt sich öffnen"));
    common::Server { app: common::router(state.clone()), state, data: root }
}

#[tokio::test]
async fn the_backup_notice_shows_once_after_a_real_backup_and_never_for_a_mere_link() {
    let server = open_in_subdir().await;
    let other = open_in_subdir().await;
    let notice = "Sicherung angelegt";
    let (status, page) = server.get("/einstellungen/daten?ok=sicherung").await;
    assert_eq!(status, StatusCode::OK, "die Seite lädt");
    assert!(!page.contains(notice), "ein bloßer Link ohne Sicherung zeigt keine Erfolgsmeldung");

    let (status, location) = server.post_form("/einstellungen/daten/sicherung", "confirm=1").await;
    assert_eq!(status, StatusCode::SEE_OTHER, "die Sicherung wird angelegt");
    assert_eq!(location.as_deref(), Some("/einstellungen/daten?ok=sicherung"), "Weiterleitung mit festem Code");
    let made: Vec<String> = std::fs::read_dir(server.path())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("daten.sicherung-"))
        .collect();
    assert_eq!(made.len(), 1, "Vorbedingung: genau eine Sicherung liegt neben dem Datenordner: {made:?}");

    // The note belongs to this data directory: another instance in the same process shows nothing.
    let (_, foreign) = other.get("/einstellungen/daten?ok=sicherung").await;
    assert!(!foreign.contains(notice), "der Vermerk einer anderen Instanz gilt hier nicht");

    let (_, without_code) = server.get("/einstellungen/daten").await;
    assert!(!without_code.contains(notice), "ohne den Meldecode erscheint die Meldung nicht (und bleibt aufgehoben)");
    let (_, first) = server.get("/einstellungen/daten?ok=sicherung").await;
    assert!(
        first.contains(&format!("Sicherung angelegt: <code>{}</code>", made[0])),
        "nach der wirklich angelegten Sicherung nennt die Seite sie – mit dem Ordner, den der Server angelegt hat"
    );
    let (_, second) = server.get("/einstellungen/daten?ok=sicherung").await;
    assert!(!second.contains(notice), "beim Neuladen bzw. über den Link steht die Meldung nicht noch einmal da");

    // A rejected backup (without confirmation) leaves no note behind.
    let (status, _) = server.post_form("/einstellungen/daten/sicherung", "").await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "ohne Bestätigung keine Sicherung");
    let (_, after_refusal) = server.get("/einstellungen/daten?ok=sicherung").await;
    assert!(!after_refusal.contains(notice), "ohne angelegte Sicherung keine Meldung");
}

// ---- Permissions of the data directory ----------------------------------------------------------------------

#[cfg(unix)]
#[tokio::test]
async fn open_permissions_of_an_existing_data_folder_are_reported_and_never_changed() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let data = root.path().join("daten");
    std::fs::create_dir(&data).unwrap();
    std::fs::set_permissions(&data, std::fs::Permissions::from_mode(0o755)).unwrap();
    let mode = || std::fs::metadata(&data).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(), 0o755, "Vorbedingung: der vorhandene Datenordner ist für andere lesbar");

    let state = Arc::new(baruch_server::AppState::open(&data).await.expect("Datenordner lässt sich öffnen"));
    let server = common::Server { app: common::router(state.clone()), state, data: root };
    assert_eq!(mode(), 0o755, "das Öffnen ändert die Rechte eines vorhandenen Ordners nicht");
    assert_eq!(baruch_server::settings::open_permissions(&data), Some(0o755), "die Prüfung nennt die Rechte");

    let (status, page) = server.get("/einstellungen/daten").await;
    assert_eq!(status, StatusCode::OK, "die Seite Daten lädt");
    let warning = part(&page, "data-open-permissions", "</p>");
    assert!(
        warning.contains("Rechte 0755") && warning.contains("ändert") && warning.contains("nicht"),
        "die Seite meldet die offenen Rechte und sagt, dass Baruch sie nicht ändert: {warning}"
    );
    assert_eq!(mode(), 0o755, "auch das Anzeigen ändert nichts – melden, nie ändern");

    for (open, why) in [(0o750, "Gruppe darf lesen"), (0o701, "andere dürfen hinein")] {
        std::fs::set_permissions(&data, std::fs::Permissions::from_mode(open)).unwrap();
        assert_eq!(baruch_server::settings::open_permissions(&data), Some(open), "{why}: gemeldet");
    }
    std::fs::set_permissions(&data, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(baruch_server::settings::open_permissions(&data), None, "nur der Besitzer: nichts zu melden");
    let (_, page) = server.get("/einstellungen/daten").await;
    assert!(!page.contains("data-open-permissions"), "mit 0700 steht kein Hinweis auf der Seite");
    assert_eq!(
        baruch_server::settings::open_permissions(&data.join("gibt-es-nicht")),
        None,
        "ein Ordner, den es nicht gibt, ist kein Befund (der Start meldet ihn an anderer Stelle)"
    );
}

// ---- Token list and button „Zitat“ --------------------------------------------------------------------------

#[test]
fn the_former_name_in_the_token_list_does_not_break_inside_the_name() {
    let template = include_str!("../templates/settings/mcp.html");
    assert!(
        template.contains("<span class=\"muted tiny token-former\">(früher <code>{{ t.legacy }}</code>)</span>"),
        "die Klammer „(früher @mcp:…)“ trägt die Klasse token-former"
    );
    let css = include_str!("../static/settings.css");
    assert!(css.contains(".token-former { white-space: nowrap; }"), "die Klammer bricht nicht um");
    let general =
        css.find(".settings-table td code { word-break: break-all; }").expect("die allgemeine Regel der Tabelle");
    let special = css
        .find(".settings-table td .token-former code { white-space: nowrap; overflow-wrap: normal; word-break: keep-all; }")
        .expect("die Regel für den früheren Namen");
    assert!(
        special > general,
        "die Regel für den früheren Namen steht hinter der allgemeinen und ist genauer (eine Klasse mehr): Sie hebt \
         `word-break: break-all` für diesen Namen auf – kein Umbruch mitten in „@mcp:…“"
    );
}

#[tokio::test]
async fn the_quote_button_leaves_no_lonely_dash() {
    let snippet = baruch_server::ribbon::QUOTE_SNIPPET;
    assert_eq!(snippet, "#quote(block: true)[]\n", "der Knopf fügt nur den Zitatblock ein");
    assert!(
        !snippet.contains('–') && !snippet.contains('—') && !snippet.contains("attribution"),
        "kein Gedankenstrich und keine leere Stellenangabe, die ungefüllt im Buch stünden"
    );
    let server = common::open().await;
    let formats = server.rpc("ribbon.formats", json!({})).await;
    let quote = result(&formats)["formats"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["cmd"] == "quote")
        .expect("Knopf Zitat")
        .clone();
    assert_eq!(quote["snippet"], snippet, "die RPC liefert denselben Text");
    assert!(
        quote["title"].as_str().unwrap_or_default().contains("Stellenangabe")
            || quote.to_string().contains("Stellenangabe"),
        "der Hinweis des Knopfs sagt, wohin die Stellenangabe gehört: {quote}"
    );
    // What the button inserts is typeset by the typeset view as a quote block – without a paragraph that consists of a
    // dash only.
    let inserted = format!("Der Satz davor.\n\n{snippet}\nDer Satz danach.\n");
    let doc = baruch_wysiwyg::parse(&inserted);
    assert!(
        doc.blocks.iter().any(|b| matches!(b.kind, baruch_wysiwyg::BlockKind::Quote { block: true, .. })),
        "das Eingefügte ist ein Zitatblock: {:#?}",
        doc.blocks
    );
    assert!(!inserted.lines().any(|line| line.trim() == "–"), "keine Zeile, die nur aus dem Gedankenstrich besteht");
}

// ---- Outline mentions ---------------------------------------------------------------------------------------

#[tokio::test]
async fn outline_mentions_know_ordinals_beyond_twelve_and_roman_numbers_beyond_xxxix() {
    let server = common::open().await;
    let mut source = String::from("#set document(title: \"Das lange Jahr\")\n#set heading(numbering: \"1.\")\n\n");
    for n in 1..=45 {
        source.push_str(&format!("= Woche {n}\n\nNotiz zur Woche {n}.\n\n"));
    }
    let work = server.import(&source, "jahr.typ").await.work;
    let work = server.state.store.load_work(work).await.unwrap();
    let key = |n: usize| work.nodes.iter().find(|node| node.title == format!("Woche {n}")).unwrap().key;
    let text = "Im dreizehnten Kapitel beginnt der Frost, das einundvierzigste Kapitel taut auf. Siehe Kapitel XLII \
                und Kapitel XIV, auch Kapitel 45. Vom zwanzigsten Kapitel an wird geheizt.";
    let found: Vec<(String, i64)> = outline_mentions(&work, text).into_iter().map(|m| (m.text, m.key)).collect();
    assert_eq!(
        found,
        vec![
            ("dreizehnten Kapitel".to_owned(), key(13)),
            ("einundvierzigste Kapitel".to_owned(), key(41)),
            ("Kapitel XLII".to_owned(), key(42)),
            ("Kapitel XIV".to_owned(), key(14)),
            ("Kapitel 45".to_owned(), key(45)),
            ("zwanzigsten Kapitel".to_owned(), key(20)),
        ],
        "Ordnungszahlen über „zwölfte“ und römische Zahlen über XXXIX hinaus führen zum gezählten Kapitel"
    );
    // Still only when unambiguous: no target, no mention – and never a wrong one.
    let none = "Im sechsundvierzigsten Kapitel, in Kapitel XLVI, in Kapitel L und im hundertsten Kapitel steht nichts. \
                Kapitel IIII und Kapitel VX sind keine Zahlen; das achtzehn Kapitel lange Buch auch nicht.";
    let found: Vec<String> = outline_mentions(&work, none).into_iter().map(|m| m.text).collect();
    assert!(found.is_empty(), "ohne genau ein Ziel wird nichts genannt: {found:?}");
    // „achtzehnten“ is the eighteenth, not the eighth chapter (the whole stem counts, not its beginning).
    let eighteen = outline_mentions(&work, "Das steht im achtzehnten Kapitel und im achten Kapitel.");
    assert_eq!(
        eighteen.iter().map(|m| m.key).collect::<Vec<_>>(),
        vec![key(18), key(8)],
        "achtzehnten → 18, achten → 8"
    );
}

// ---- store-5: deleting an outline item logs proposals and posts ----------------------------------------------

const MILL: &str = "#set document(title: \"Die Windmühle\", author: \"Ole Strom\")\n\n\
= Der Mahlgang\n\nDer Stein dreht sich langsam.\n\n\
= Der Sackboden\n\nHier lagert das Korn.\n\n\
= Die Kappe\n\nSie dreht sich nach dem Wind.\n";

fn last_commit(server: &common::Server, slug: &str) -> String {
    common::git(&server.mirror_dir_of(slug), &["log", "-1", "--format=%an%n%s%n%b"]).unwrap_or_default()
}

fn commits(server: &common::Server, slug: &str) -> usize {
    common::git(&server.mirror_dir_of(slug), &["rev-list", "--count", "HEAD"]).map_or(0, |c| c.trim().parse().unwrap())
}

/// Section `node` gets one open proposal, one discarded one and two posts.
async fn fill(server: &common::Server, work: i64, node: usize, title: &str) {
    let store = &server.state.store;
    let proposal = format!("= {title}\n\nVorschlag: Hier lagert das Korn in Leinensäcken.\n");
    store.add_proposal(work, node, &proposal, "@lektor", "probe", "Säcke").await.unwrap();
    let rejected = format!("= {title}\n\nVerworfen: Hier lagert gar nichts.\n");
    let number = store.add_proposal(work, node, &rejected, "@nathan", "probe", "leer").await.unwrap();
    store.reject(work, node, number).await.unwrap();
    for (author, body) in
        [("@nathan", "Wie viele Säcke passen auf den Boden?"), ("Autor", "Etwa vierzig, bis unters Dach.")]
    {
        let post = baruch_core::Post {
            id: 0,
            created: String::new(),
            author: author.into(),
            body: body.into(),
            sources: vec![],
            refs: vec![],
        };
        store.add_post(work, node, &post).await.unwrap();
    }
}

#[tokio::test]
async fn deleting_an_outline_point_records_the_lost_proposals_and_posts_in_the_mirror_commit() {
    let server = common::open().await;
    let report = server.import(MILL, "windmuehle.typ").await;
    let (work, slug) = (report.work, report.slug);
    fill(&server, work, 1, "Der Sackboden").await;
    // One commit in advance so that the mirror knows the state before the deletion.
    let before = commits(&server, &slug);

    let answer = server.rpc("outline.delete", json!({ "work": work, "node": 1, "confirm": true })).await;
    result(&answer);
    assert_eq!(commits(&server, &slug), before + 1, "das Löschen ist genau ein Commit");
    let commit = last_commit(&server, &slug);
    assert!(commit.contains("Gliederung: „Der Sackboden“ gelöscht"), "Betreff nennt das Löschen: {commit}");
    assert!(
        commit.contains("Gelöschter Text „Der Sackboden“:") && commit.contains("Hier lagert das Korn."),
        "wie bisher steht der Text der aktuellen Fassung im Commit: {commit}"
    );
    assert!(
        commit.contains("Mitgelöschte Fassungen: 2 (davon offene Vorschläge: 1)"),
        "der Commit zählt die mitgelöschten Fassungen: {commit}"
    );
    assert!(
        commit.contains("Fassung 2 (offener Vorschlag von @lektor):")
            && commit.contains("Vorschlag: Hier lagert das Korn in Leinensäcken."),
        "der offene Vorschlag steht mit Herkunft und Wortlaut im Commit: {commit}"
    );
    assert!(
        commit.contains("Fassung 3 (ältere Fassung von @nathan):")
            && commit.contains("Verworfen: Hier lagert gar nichts."),
        "auch die verworfene Fassung geht nicht stillschweigend verloren: {commit}"
    );
    assert!(commit.contains("Mitgelöschte Beiträge: 2"), "der Commit zählt die mitgelöschten Beiträge: {commit}");
    assert!(
        commit.contains("von @nathan:\nWie viele Säcke passen auf den Boden?")
            && commit.contains("von Autor:\nEtwa vierzig, bis unters Dach."),
        "jeder Beitrag steht mit Autor und Wortlaut im Commit: {commit}"
    );
    assert!(
        commit.contains("Baruch-Lost-Revisions: 2") && commit.contains("Baruch-Lost-Posts: 2"),
        "die Zahlen stehen auch als Trailer da: {commit}"
    );
    assert!(commit.starts_with("Autor\n"), "gelöscht hat der Mensch (Autor des Commits): {commit}");

    // An item without proposals and posts: the commit stays as it was.
    let answer = server.rpc("outline.delete", json!({ "work": work, "node": 1, "confirm": true })).await;
    result(&answer);
    let plain = last_commit(&server, &slug);
    assert!(plain.contains("Gliederung: „Die Kappe“ gelöscht"), "der nächste Punkt ist gelöscht: {plain}");
    assert!(
        !plain.contains("Mitgelöscht") && !plain.contains("Baruch-Lost"),
        "ohne Verluste nennt der Commit keine (unverändert gegenüber vorher): {plain}"
    );
}

async fn mcp_post(server: &common::Server, session: Option<&str>, message: Value) -> (axum::http::HeaderMap, Value) {
    let mut request = Request::post("/mcp").header(header::CONTENT_TYPE, "application/json");
    if let Some(id) = session {
        request = request.header("mcp-session-id", id);
    }
    let mut request = request.body(Body::from(message.to_string())).unwrap();
    request.extensions_mut().insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40000))));
    let (status, headers, body) = server.send(request).await;
    assert_eq!(status, StatusCode::OK, "MCP antwortet: {body}");
    (headers, serde_json::from_str(&body).unwrap())
}

/// The human stays the only one who deletes: a client only files a request over MCP. If the human confirms it, the
/// commit names the same losses; the request alone deletes and logs nothing.
#[tokio::test]
async fn a_confirmed_delete_request_records_the_losses_too_and_the_request_alone_deletes_nothing() {
    let server = common::open().await;
    let report = server.import(MILL, "windmuehle.typ").await;
    let (work, slug) = (report.work, report.slug);
    fill(&server, work, 1, "Der Sackboden").await;
    let key = server.state.store.load_work(work).await.unwrap().nodes[1].key;

    let (headers, answer) = mcp_post(
        &server,
        None,
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize",
                "params": { "protocolVersion": "2025-06-18", "capabilities": {},
                            "clientInfo": { "name": "Mahlwerk", "version": "1.0" } } }),
    )
    .await;
    assert!(answer["result"].is_object(), "initialize: {answer}");
    let session = headers.get("mcp-session-id").unwrap().to_str().unwrap().to_owned();
    let call = json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/call",
                       "params": { "name": "outline_delete", "arguments": { "work": work, "key": key } } });
    let (_, answer) = mcp_post(&server, Some(&session), call).await;
    assert_eq!(answer["result"]["isError"], false, "der Antrag wird angenommen: {answer}");
    let loaded = server.state.store.load_work(work).await.unwrap();
    assert!(
        loaded.nodes.iter().any(|n| n.key == key) && loaded.nodes[1].posts.len() == 2,
        "der Antrag allein löscht nichts: Punkt und Beiträge sind noch da"
    );
    let requests = server.rpc("outline.requests", json!({ "work": work })).await;
    let id = result(&requests)["requests"][0]["id"].as_i64().expect("ein offener Antrag");

    let before = commits(&server, &slug);
    let decided = server.rpc("outline.request_decide", json!({ "work": work, "id": id, "decision": "confirm" })).await;
    result(&decided);
    assert_eq!(commits(&server, &slug), before + 1, "das Bestätigen ist genau ein Commit");
    let commit = last_commit(&server, &slug);
    assert!(commit.contains("gelöscht") && commit.contains(&format!("(Antrag {id})")), "Betreff mit Antrag: {commit}");
    assert!(
        commit.contains("Fassung 2 (offener Vorschlag von @lektor):")
            && commit.contains("Vorschlag: Hier lagert das Korn in Leinensäcken.")
            && commit.contains("Mitgelöschte Beiträge: 2")
            && commit.contains("Wie viele Säcke passen auf den Boden?"),
        "auch beim bestätigten Antrag stehen Vorschlag und Beiträge im Commit: {commit}"
    );
    assert!(
        commit.contains("Baruch-Confirmed-By: Autor") && commit.starts_with("Autor\n"),
        "bestätigt und damit gelöscht hat der Mensch, nicht der Client: {commit}"
    );
    assert!(
        !server.state.store.load_work(work).await.unwrap().nodes.iter().any(|n| n.key == key),
        "nach dem Bestätigen ist der Punkt gelöscht"
    );
}
