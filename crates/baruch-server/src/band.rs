// SPDX-License-Identifier: AGPL-3.0-or-later
//! Ribbon (Round 33; owner: task “menueband”).
//!
//! The ribbon stands completely in `templates/base.html` (the template is embedded in many own environments,
//! a fragment would have to be registered in all of them). Pages do not know their address and `?tab=`; so the
//! layer `consent::inject` uses [`apply`] to set the active tab, the addresses of the tabs and the laptop state in
//! the finished HTML – like the badges since Round 23a (`menu::fill_badges`).
//!
//! State ([`BandState::from_request`]): `?tab=start|einfuegen|pruefen|buch|ansicht` (not remembered) and
//! `?band=open|closed` (before the cookie `baruch_band`, remembered through [`set_cookies`]). Invalid values are
//! skipped. Everything else of the query stays attached as `rest` to the addresses of tabs and switches so that a
//! click without JavaScript does not lose work, section, revision or page.
//!
//! [`apply`] replaces only literal forms of the template (order of attributes as in `base.html`); values from the
//! address (`rest`) go into attributes only escaped ([`escape_attr`]). Steps 2 to 6 act only in the
//! head (`<header class="band"` to `</header>`) so that text in the page content that happens to look like a tab
//! changes nothing; the switches (steps 7 and 8) apply on the whole page (also for the narrow rails of the
//! work page).

use crate::laptop::LaptopState;

/// Marker on `<html>` in `base.html`; [`apply`] replaces it with the attributes of the state.
pub const FILL: &str = " data-band-fill";
/// Tabs of the ribbon: (id in `?tab=`, label).
pub const TABS: [(&str, &str); 5] =
    [("start", "Start"), ("einfuegen", "Einfügen"), ("pruefen", "Prüfen"), ("buch", "Buch"), ("ansicht", "Ansicht")];
/// Keys of the address that belong to this module.
pub const QUERY_KEYS: [&str; 2] = ["tab", "band"];

/// Cookie that remembers a collapsed ribbon (`closed`; missing = open).
const COOKIE: &str = "baruch_band";
/// Start and end of the head in the finished page.
const HEAD_OPEN: &str = "<header class=\"band\"";
const HEAD_CLOSE: &str = "</header>";
/// Keys whose switches (`href="?K=V"`) take along the rest of the query and the active tab (step 7).
const SWITCH_KEYS: [&str; 4] = ["band", "laptop", "lrail", "rrail"];

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BandState {
    /// Tab from `?tab=`, only if it is in TABS.
    pub tab: Option<&'static str>,
    /// Ribbon collapsed (`?band=` before cookie `baruch_band`).
    pub closed: bool,
    /// Raw query without the keys from band::QUERY_KEYS and laptop::QUERY_KEYS (pairs with `&`, as is, not decoded).
    pub rest: String,
}

/// Pairs of the raw query as (key, value, whole pair); empty pairs (`&&`) are dropped. Nothing is decoded:
/// a key counts only in its literal spelling.
fn pairs(query: Option<&str>) -> impl Iterator<Item = (&str, &str, &str)> {
    query.unwrap_or("").split('&').filter(|pair| !pair.is_empty()).map(|pair| {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        (key, value, pair)
    })
}

/// Last valid value of `?band=` (`Some(true)` = collapsed); `None` if none is valid.
fn band_in_query(query: Option<&str>) -> Option<bool> {
    pairs(query)
        .filter_map(|(key, value, _)| match (key, value) {
            ("band", "closed") => Some(true),
            ("band", "open") => Some(false),
            _ => None,
        })
        .last()
}

/// Value of a cookie from the header `Cookie:` (`name=wert; name2=wert2`).
fn cookie<'a>(header: Option<&'a str>, name: &str) -> Option<&'a str> {
    header?.split(';').find_map(|part| part.trim().strip_prefix(name)?.strip_prefix('='))
}

impl BandState {
    /// State from cookie header and raw query of the request: `?tab=` only for ids from [`TABS`] (the last
    /// valid value counts), `?band=open|closed` before the cookie `baruch_band`, `rest` without both modules' keys.
    pub fn from_request(cookie_header: Option<&str>, query: Option<&str>) -> BandState {
        let tab = pairs(query)
            .filter(|(key, _, _)| *key == "tab")
            .filter_map(|(_, value, _)| TABS.iter().map(|(id, _)| *id).find(|id| *id == value))
            .last();
        let closed = band_in_query(query).unwrap_or_else(|| cookie(cookie_header, COOKIE) == Some("closed"));
        let rest = pairs(query)
            .filter(|(key, _, _)| !QUERY_KEYS.contains(key) && !crate::laptop::QUERY_KEYS.contains(key))
            .map(|(_, _, pair)| pair)
            .collect::<Vec<_>>()
            .join("&");
        BandState { tab, closed, rest }
    }
}

/// Finished Set-Cookie values for a valid key `band` in the address: `closed` is remembered by the cookie for a
/// year, `open` deletes it (missing = open). Without a valid value the list stays empty.
pub fn set_cookies(query: Option<&str>) -> Vec<String> {
    match band_in_query(query) {
        Some(true) => vec![format!("{COOKIE}=closed; Path=/; Max-Age=31536000; SameSite=Lax")],
        Some(false) => vec![format!("{COOKIE}=; Path=/; Max-Age=0; SameSite=Lax")],
        None => Vec::new(),
    }
}

/// Escapes a value for an HTML attribute in double or single quotes (`&`, `<`, `>`, `"`, `'`).
pub fn escape_attr(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 8);
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            other => out.push(other),
        }
    }
    out
}

/// Ids of the tabs that stand in the head (`data-band-tab="ID"`), in page order; known ones only.
fn tabs_in(head: &str) -> Vec<&'static str> {
    let mut found = Vec::new();
    let mut rest = head;
    while let Some(at) = rest.find("data-band-tab=\"") {
        rest = &rest[at + "data-band-tab=\"".len()..];
        let Some(end) = rest.find('"') else { break };
        if let Some((id, _)) = TABS.iter().find(|(id, _)| *id == &rest[..end])
            && !found.contains(id)
        {
            found.push(*id);
        }
    }
    found
}

/// Value of `data-band-default="…"` in the head.
fn default_tab(head: &str) -> Option<&str> {
    let rest = &head[head.find("data-band-default=\"")? + "data-band-default=\"".len()..];
    Some(rest[..rest.find('"')?].trim())
}

/// Step 2: active tab = `?tab=` if the page has it; else the default of the page if present; else
/// the first tab of the page (without tab “start”).
fn active_tab(head: &str, wanted: Option<&'static str>) -> &'static str {
    let tabs = tabs_in(head);
    wanted
        .filter(|tab| tabs.contains(tab))
        .or_else(|| default_tab(head).and_then(|name| tabs.iter().copied().find(|tab| *tab == name)))
        .or_else(|| tabs.first().copied())
        .unwrap_or(TABS[0].0)
}

/// Steps 3 to 6 in the head: `data-tab`, addresses and `aria-selected` of the tabs, `hidden` on the active panel,
/// groups in the compact ribbon as popups. `prefix` is the escaped rest of the query including `&amp;` (or empty).
fn fill_head(head: &str, active: &str, band: &BandState, prefix: &str, compact: bool) -> String {
    let mut out = head.replacen("data-tab=\"\"", &format!("data-tab=\"{active}\""), 1);
    for (id, _) in TABS {
        let template = format!("data-band-tab=\"{id}\" href=\"?tab={id}\" aria-selected=\"false\"");
        if !out.contains(&template) {
            continue;
        }
        let is_active = id == active;
        // Open: the active tab collapses, the others only switch. Closed: every tab opens the ribbon.
        let band_part = match (band.closed, is_active) {
            (true, _) => "&amp;band=open",
            (false, true) => "&amp;band=closed",
            (false, false) => "",
        };
        let filled =
            format!("data-band-tab=\"{id}\" href=\"?{prefix}tab={id}{band_part}\" aria-selected=\"{is_active}\"");
        out = out.replace(&template, &filled);
    }
    out = out.replace(&format!("data-band-panel=\"{active}\" hidden"), &format!("data-band-panel=\"{active}\""));
    if compact {
        out = out.replace(" data-band-group open>", " data-band-group name=\"band-group\">");
    }
    out
}

/// Step 8: pressed laptop switch, side rails (pressed = visible; collapsed, the address points to
/// “expand”) and the ribbon switch (collapsed: address to `band=open`).
fn fill_switches(page: &str, band: &BandState, laptop: &LaptopState) -> String {
    let mode = laptop.mode.name();
    let mut out = page.replace(
        &format!("data-laptop-set=\"{mode}\" aria-pressed=\"false\""),
        &format!("data-laptop-set=\"{mode}\" aria-pressed=\"true\""),
    );
    for (side, key, collapsed) in [("left", "lrail", laptop.rail_left()), ("right", "rrail", laptop.rail_right())] {
        let template = format!("data-rail-toggle=\"{side}\" aria-pressed=\"false\" href=\"?{key}=1\"");
        let filled = if collapsed {
            format!("data-rail-toggle=\"{side}\" aria-pressed=\"false\" href=\"?{key}=0\"")
        } else {
            format!("data-rail-toggle=\"{side}\" aria-pressed=\"true\" href=\"?{key}=1\"")
        };
        out = out.replace(&template, &filled);
    }
    if band.closed {
        out = out.replace(
            "data-band-toggle aria-expanded=\"true\" href=\"?band=closed\"",
            "data-band-toggle aria-expanded=\"false\" href=\"?band=open\"",
        );
    }
    out
}

/// Step 7: every `href="?K=V"` with exactly one pair and K ∈ [`SWITCH_KEYS`] gets the rest of the query and the
/// active tab before it (`href="?<rest>&tab=<active>&K=V"`). V only from `[A-Za-z0-9_-]`, else the link stays as is.
fn fill_switch_links(page: &str, active: &str, prefix: &str) -> String {
    const OPEN: &str = "href=\"?";
    let mut out = String::with_capacity(page.len() + 256);
    let mut rest = page;
    while let Some(at) = rest.find(OPEN) {
        let (before, after) = rest.split_at(at + OPEN.len());
        out.push_str(before);
        rest = after;
        let Some(end) = rest.find('"') else { break };
        let Some((key, value)) = rest[..end].split_once('=') else { continue };
        let plain = !value.is_empty() && value.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
        if SWITCH_KEYS.contains(&key) && plain {
            out.push_str(prefix);
            out.push_str("tab=");
            out.push_str(active);
            out.push_str("&amp;");
        }
    }
    out.push_str(rest);
    out
}

/// Puts the state into a finished page; a page without [`FILL`] stays unchanged. Steps (contract R33 §4):
/// 1. marker on `<html>` → attributes of the laptop state and `data-band="open|closed"`;
/// 2. active tab ([`active_tab`]); 3. `data-tab` on the head; 4. addresses of the tabs with the rest of the query,
///    `aria-selected`, `band=closed` on the active one or `band=open` on all with a collapsed ribbon;
/// 5. `hidden` is dropped on the active panel; 6. compact: groups without `open`, with `name="band-group"` (one open);
/// 7. switch addresses (`?band=`, `?laptop=`, `?lrail=`, `?rrail=`) with rest and active tab;
/// 8. `aria-pressed` of the laptop and rail switches.
pub fn apply(page: &str, band: &BandState, laptop: &LaptopState) -> String {
    if !page.contains(FILL) {
        return page.to_owned();
    }
    let state = if band.closed { "closed" } else { "open" };
    let page = page.replacen(FILL, &format!(" {} data-band=\"{state}\"", laptop.html_attrs()), 1);
    // Rest of the query: comes from the address, so only escaped into an attribute. `node=` (position in the tree, old
    // address form) is not taken along: generated links never carry `node=` (contract Round 8 §1) but `key=`; a
    // page that was still called with `?node=` loses only this position on a tab switch without JavaScript.
    let carried = pairs(Some(&band.rest))
        .filter(|(key, _, _)| *key != "node")
        .map(|(_, _, pair)| pair)
        .collect::<Vec<_>>()
        .join("&");
    let prefix = if carried.is_empty() { String::new() } else { format!("{}&amp;", escape_attr(&carried)) };
    let head = page.find(HEAD_OPEN).and_then(|start| {
        let end = start + page[start..].find(HEAD_CLOSE)? + HEAD_CLOSE.len();
        Some((start, end))
    });
    let (page, active) = match head {
        Some((start, end)) => {
            let active = active_tab(&page[start..end], band.tab);
            let filled = fill_head(&page[start..end], active, band, &prefix, laptop.compact());
            (format!("{}{}{}", &page[..start], filled, &page[end..]), active)
        }
        // Page without head (own block topbar): only the switches, with the wanted tab.
        None => (page, band.tab.unwrap_or(TABS[0].0)),
    };
    let page = fill_switches(&page, band, laptop);
    fill_switch_links(&page, active, &prefix)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::laptop::{LaptopState, Mode, Width};

    /// Head in the literal forms of `base.html`, with the tabs `tabs` and the default `default`.
    fn page(tabs: &[&str], default: &str) -> String {
        let mut out = String::from(
            "<html lang=\"de\" data-band-fill><body><header class=\"band\" data-widget=\"band\" data-tab=\"\">",
        );
        out.push_str(&format!("<nav class=\"band-tabs\" role=\"tablist\" data-band-default=\"{default}\">"));
        for id in tabs {
            out.push_str(&format!(
                "<a class=\"band-tab\" role=\"tab\" id=\"band-tab-{id}\" aria-controls=\"band-panel-{id}\" data-band-tab=\"{id}\" href=\"?tab={id}\" aria-selected=\"false\">{id}</a>"
            ));
        }
        out.push_str(
            "</nav><a class=\"btn band-toggle\" data-band-toggle aria-expanded=\"true\" href=\"?band=closed\">x</a>",
        );
        for id in tabs {
            out.push_str(&format!(
                "<div class=\"band-panel\" role=\"tabpanel\" id=\"band-panel-{id}\" aria-labelledby=\"band-tab-{id}\" data-band-panel=\"{id}\" hidden>\
                 <details class=\"band-group\" data-band-group open><summary class=\"band-group-label\">G</summary><div class=\"band-items\" role=\"group\" aria-label=\"G\"></div></details></div>"
            ));
        }
        out.push_str(
            "<a class=\"btn\" data-rail-toggle=\"left\" aria-pressed=\"false\" href=\"?lrail=1\">Gliederung</a>\
             <a class=\"btn\" data-rail-toggle=\"right\" aria-pressed=\"false\" href=\"?rrail=1\">Assistenten</a>\
             <a class=\"btn\" data-laptop-set=\"auto\" aria-pressed=\"false\" href=\"?laptop=auto\">Automatisch</a>\
             <a class=\"btn\" data-laptop-set=\"on\" aria-pressed=\"false\" href=\"?laptop=on\">An</a>\
             <a class=\"btn\" data-laptop-set=\"off\" aria-pressed=\"false\" href=\"?laptop=off\">Aus</a>\
             </header><main>Inhalt</main></body></html>",
        );
        out
    }

    const ALL: [&str; 5] = ["start", "einfuegen", "pruefen", "buch", "ansicht"];

    fn state(query: &str) -> BandState {
        BandState::from_request(None, Some(query))
    }

    #[test]
    fn page_without_marker_stays_untouched() {
        // Fragments and pages that do not extend base.html do not carry the marker.
        let plain = "<html lang=\"de\"><body><header class=\"band\" data-tab=\"\"></header>ohne Marke</body></html>";
        assert_eq!(apply(plain, &state("tab=buch"), &LaptopState::default()), plain, "ohne Marke keine Änderung");
    }

    #[test]
    fn apply_replaces_only_the_first_marker() {
        let page = "<html lang=\"de\" data-band-fill><body><p data-band-fill>x</p></body></html>";
        let out = apply(page, &BandState::default(), &LaptopState::default());
        assert_eq!(
            out,
            "<html lang=\"de\" data-laptop=\"auto\" data-band=\"open\"><body><p data-band-fill>x</p></body></html>",
            "nur die Marke an <html> wird ersetzt; ein zweites Vorkommen im Seiteninhalt bleibt, wie es ist"
        );
    }

    #[test]
    fn html_attributes_tab_selection_and_panel_follow_the_active_tab() {
        let out = apply(&page(&ALL, "start"), &state("tab=buch"), &LaptopState::default());
        assert!(
            out.starts_with("<html lang=\"de\" data-laptop=\"auto\" data-band=\"open\">"),
            "die Marke wird zu den Attributen des Zustands (Laptop-Modus und Band offen): {out}"
        );
        assert!(
            out.contains("<header class=\"band\" data-widget=\"band\" data-tab=\"buch\">"),
            "der Kopf nennt den aktiven Reiter: {out}"
        );
        assert_eq!(out.matches("aria-selected=\"true\"").count(), 1, "genau ein Reiter ist gewählt: {out}");
        assert!(
            out.contains("data-band-tab=\"buch\" href=\"?tab=buch&amp;band=closed\" aria-selected=\"true\""),
            "gewählt ist „Buch“: {out}"
        );
        assert!(
            out.contains("data-band-panel=\"buch\"><details"),
            "am aktiven Feld fällt hidden weg, damit es ohne JavaScript sichtbar ist: {out}"
        );
        assert_eq!(
            out.matches("\" hidden><details").count(),
            4,
            "die vier anderen Felder bleiben verborgen (hidden nur am aktiven entfernt): {out}"
        );
    }

    #[test]
    fn tab_from_the_address_counts_only_if_the_page_has_it() {
        // Page with a work, but not the work page: no tab „Einfügen“.
        let four = page(&["start", "pruefen", "buch", "ansicht"], "pruefen");
        let out = apply(&four, &state("tab=einfuegen"), &LaptopState::default());
        assert!(
            out.contains("data-tab=\"pruefen\""),
            "?tab=einfuegen gibt es auf dieser Seite nicht: es gilt die Vorgabe der Seite (data-band-default): {out}"
        );
        let out = apply(&four, &state("tab=ansicht"), &LaptopState::default());
        assert!(
            out.contains("data-tab=\"ansicht\""),
            "ein vorhandener Reiter aus der Adresse geht vor der Vorgabe: {out}"
        );
        // Default that does not exist on the page (page without a work with default “buch”): the first tab.
        let two = page(&["start", "ansicht"], "buch");
        let out = apply(&two, &BandState::default(), &LaptopState::default());
        assert!(
            out.contains("data-tab=\"start\""),
            "fehlt auch die Vorgabe als Reiter, gilt der erste der Seite: {out}"
        );
        assert!(
            out.contains("data-band-panel=\"start\"><details"),
            "und dessen Feld ist sichtbar – sonst stünde ohne JavaScript kein Feld da: {out}"
        );
    }

    #[test]
    fn unknown_tab_values_are_ignored_and_the_last_valid_one_wins() {
        assert_eq!(state("tab=unsinn").tab, None, "unbekannte Kennung: kein Reiter aus der Adresse");
        assert_eq!(state("tab=Buch").tab, None, "Kennungen gelten wörtlich (klein)");
        assert_eq!(state("tab=start&tab=buch").tab, Some("buch"), "mehrfach genannt: der letzte gültige Wert gilt");
        assert_eq!(
            state("tab=buch&tab=x").tab,
            Some("buch"),
            "ein ungültiger Wert dahinter verdrängt den gültigen nicht"
        );
    }

    #[test]
    fn tab_addresses_keep_the_rest_of_the_query_and_drop_the_own_keys() {
        let band = state("work=3&key=12&tab=start&rev=4&band=open&laptop=on&lrail=1&rrail=0&page=7");
        assert_eq!(
            band.rest, "work=3&key=12&rev=4&page=7",
            "Werk, Abschnitt, Fassung und Seite bleiben in ihrer Reihenfolge; tab, band, laptop, lrail, rrail fallen weg"
        );
        let out = apply(&page(&ALL, "start"), &band, &LaptopState::default());
        assert!(
            out.contains("data-band-tab=\"pruefen\" href=\"?work=3&amp;key=12&amp;rev=4&amp;page=7&amp;tab=pruefen\" aria-selected=\"false\""),
            "ein anderer Reiter führt ohne JavaScript auf dieselbe Stelle des Werks, nur mit anderem Reiter: {out}"
        );
        assert!(!out.contains("href=\"?tab="), "kein Reiter verliert den Rest der Abfrage: {out}");
        assert!(
            !out.contains("lrail=1&amp;tab") && !out.contains("laptop=on&amp;"),
            "die eigenen Schlüssel stehen nicht im Rest: {out}"
        );
    }

    #[test]
    fn node_from_an_old_address_is_not_carried_into_generated_links() {
        let band = state("title=1&work=2&node=3");
        assert_eq!(band.rest, "title=1&work=2&node=3", "der Rest selbst bleibt die rohe Abfrage (Vertrag §3.2)");
        let out = apply(&page(&ALL, "start"), &band, &LaptopState::default());
        assert!(!out.contains("node="), "kein erzeugter Link trägt node= (Vertrag Runde 8 §1): {out}");
        assert!(
            out.contains("data-band-tab=\"buch\" href=\"?title=1&amp;work=2&amp;tab=buch\""),
            "die übrigen Schlüssel bleiben: {out}"
        );
        let only = apply(&page(&ALL, "start"), &state("node=3"), &LaptopState::default());
        assert!(
            only.contains("href=\"?tab=buch\""),
            "bleibt nichts übrig, steht nur der Reiter (kein führendes &): {only}"
        );
    }

    #[test]
    fn without_a_query_the_tab_addresses_are_bare() {
        let out = apply(&page(&ALL, "start"), &BandState::default(), &LaptopState::default());
        assert!(
            out.contains("data-band-tab=\"buch\" href=\"?tab=buch\" aria-selected=\"false\""),
            "ohne Rest steht nur ?tab=ID (kein führendes &): {out}"
        );
    }

    #[test]
    fn a_hostile_rest_is_escaped_in_every_address() {
        // The query comes raw from the address: a quotation mark must not be able to end the attribute.
        let band = state("x=\"><script>alert(1)</script>&y='");
        let out = apply(&page(&ALL, "start"), &band, &LaptopState::default());
        assert!(!out.contains("<script>"), "kein Skript aus der Adresse im HTML: {out}");
        assert!(!out.contains("\"><script"), "das Attribut wird nicht beendet: {out}");
        assert!(
            out.contains("href=\"?x=&quot;&gt;&lt;script&gt;alert(1)&lt;/script&gt;&amp;y=&#39;&amp;tab=buch\""),
            "der Rest steht maskiert im href des Reiters (& < > \" '): {out}"
        );
        assert!(
            out.contains(
                "href=\"?x=&quot;&gt;&lt;script&gt;alert(1)&lt;/script&gt;&amp;y=&#39;&amp;tab=start&amp;laptop=on\""
            ),
            "auch die Schalter tragen den Rest nur maskiert: {out}"
        );
        assert_eq!(
            escape_attr("&<>\"'a"),
            "&amp;&lt;&gt;&quot;&#39;a",
            "alle fünf Zeichen werden maskiert, anderes bleibt"
        );
    }

    #[test]
    fn the_active_tab_closes_an_open_band_and_every_tab_opens_a_closed_one() {
        let open = apply(&page(&ALL, "start"), &state("work=1&tab=buch"), &LaptopState::default());
        assert!(open.contains("data-band=\"open\""), "ohne Cookie und ohne ?band= ist das Band offen: {open}");
        assert!(
            open.contains("href=\"?work=1&amp;tab=buch&amp;band=closed\" aria-selected=\"true\""),
            "offen: ein Klick auf den aktiven Reiter klappt zu (ohne JavaScript über ?band=closed): {open}"
        );
        assert!(
            open.contains("href=\"?work=1&amp;tab=start\" aria-selected=\"false\""),
            "offen: die anderen Reiter wechseln nur: {open}"
        );
        let closed = apply(&page(&ALL, "start"), &state("work=1&tab=buch&band=closed"), &LaptopState::default());
        assert!(closed.contains("data-band=\"closed\""), "?band=closed klappt zu: {closed}");
        assert!(
            closed.contains("href=\"?work=1&amp;tab=buch&amp;band=open\" aria-selected=\"true\""),
            "zu: der aktive Reiter öffnet wieder: {closed}"
        );
        assert_eq!(
            closed.matches("&amp;band=open\" aria-selected=").count(),
            5,
            "zu: jeder Reiter öffnet das Band (sonst wechselte man blind): {closed}"
        );
        assert!(
            closed.contains("data-band-toggle aria-expanded=\"false\" href=\"?work=1&amp;tab=buch&amp;band=open\""),
            "der Schalter rechts zeigt bei zugeklapptem Band auf „öffnen“ und sagt es (aria-expanded): {closed}"
        );
        assert!(
            open.contains("data-band-toggle aria-expanded=\"true\" href=\"?work=1&amp;tab=buch&amp;band=closed\""),
            "bei offenem Band klappt er zu: {open}"
        );
    }

    #[test]
    fn the_address_wins_over_the_cookie_for_the_band() {
        let cookie = Some("baruch_work=2; baruch_band=closed");
        assert!(BandState::from_request(cookie, None).closed, "Cookie baruch_band=closed: zugeklappt");
        assert!(!BandState::from_request(cookie, Some("band=open")).closed, "?band=open geht vor dem Cookie");
        assert!(
            BandState::from_request(cookie, Some("band=kaputt")).closed,
            "ein ungültiger Wert wird übergangen, das Cookie gilt"
        );
        assert!(
            !BandState::from_request(Some("baruch_bandx=closed"), None).closed,
            "ein Cookie mit ähnlichem Namen zählt nicht"
        );
        assert!(!BandState::from_request(None, None).closed, "ohne alles ist das Band offen");
    }

    #[test]
    fn compact_turns_every_group_into_a_popup() {
        let compact = LaptopState { mode: Mode::On, ..LaptopState::default() };
        let out = apply(&page(&ALL, "start"), &BandState::default(), &compact);
        assert!(!out.contains(" data-band-group open>"), "kompakt ist keine Gruppe aufgeklappt: {out}");
        assert_eq!(
            out.matches(" data-band-group name=\"band-group\">").count(),
            5,
            "jede Gruppe wird ein <details> mit gemeinsamem Namen (der Browser hält dann nur eines offen): {out}"
        );
        let wide = apply(&page(&ALL, "start"), &BandState::default(), &LaptopState::default());
        assert_eq!(
            wide.matches(" data-band-group open>").count(),
            5,
            "im vollen Band bleiben alle Gruppen offen: {wide}"
        );
        let medium = LaptopState { width: Width::Medium, ..LaptopState::default() };
        assert!(
            apply(&page(&ALL, "start"), &BandState::default(), &medium).contains("name=\"band-group\""),
            "automatisch bei mittlerer Breite (Cookie baruch_vw=m) ist das Band ebenfalls kompakt"
        );
    }

    #[test]
    fn laptop_and_rail_switches_show_their_state() {
        let on = LaptopState { mode: Mode::On, ..LaptopState::default() };
        let out = apply(&page(&ALL, "start"), &state("work=1"), &on);
        assert!(
            out.contains("data-laptop-set=\"on\" aria-pressed=\"true\""),
            "der Schalter des geltenden Modus ist gedrückt: {out}"
        );
        assert!(
            out.contains("data-laptop-set=\"auto\" aria-pressed=\"false\"")
                && out.contains("data-laptop-set=\"off\" aria-pressed=\"false\""),
            "die anderen beiden nicht: {out}"
        );
        assert!(
            out.contains(
                "data-laptop-set=\"off\" aria-pressed=\"false\" href=\"?work=1&amp;tab=start&amp;laptop=off\""
            ),
            "die Schalter behalten Werk und Reiter in der Adresse: {out}"
        );
        // “On” collapses both rails: not pressed, the address expands.
        assert!(
            out.contains("data-rail-toggle=\"left\" aria-pressed=\"false\" href=\"?work=1&amp;tab=start&amp;lrail=0\""),
            "eingeklappte Gliederung: nicht gedrückt, der Link klappt aus: {out}"
        );
        assert!(
            out.contains(
                "data-rail-toggle=\"right\" aria-pressed=\"false\" href=\"?work=1&amp;tab=start&amp;rrail=0\""
            ),
            "rechts ebenso: {out}"
        );

        let left_only = LaptopState { lrail: true, ..LaptopState::default() };
        let out = apply(&page(&ALL, "start"), &BandState::default(), &left_only);
        assert!(
            out.contains("data-laptop-set=\"auto\" aria-pressed=\"true\""),
            "Vorgabe „Automatisch“ ist gedrückt: {out}"
        );
        assert!(
            out.contains("data-rail-toggle=\"left\" aria-pressed=\"false\" href=\"?tab=start&amp;lrail=0\""),
            "nur links eingeklappt: {out}"
        );
        assert!(
            out.contains("data-rail-toggle=\"right\" aria-pressed=\"true\" href=\"?tab=start&amp;rrail=1\""),
            "die sichtbare rechte Spalte ist gedrückt, der Link klappt sie ein: {out}"
        );
    }

    #[test]
    fn switch_links_outside_the_head_are_filled_but_look_alike_tabs_are_not() {
        let body = "<html data-band-fill><body><header class=\"band\" data-tab=\"\"><nav data-band-default=\"start\">\
            <a class=\"band-tab\" role=\"tab\" id=\"band-tab-start\" aria-controls=\"band-panel-start\" data-band-tab=\"start\" href=\"?tab=start\" aria-selected=\"false\">Start</a></nav></header>\
            <main><a href=\"?lrail=0\">Gliederung zeigen</a><p>Text mit data-band-tab=\"buch\" und <a href=\"?seite=2\">weiter</a> und <a href=\"?laptop=a b\">x</a></p></main></body></html>";
        let out = apply(body, &state("work=5&tab=buch"), &LaptopState::default());
        assert!(
            out.contains("<a href=\"?work=5&amp;tab=start&amp;lrail=0\">Gliederung zeigen</a>"),
            "Schalter außerhalb des Kopfs (schmale Leisten der Arbeitsseite) bekommen Rest und Reiter: {out}"
        );
        assert!(
            out.contains("data-tab=\"start\""),
            "ein Reiter-Name im Seiteninhalt zählt nicht: ?tab=buch gilt nur, wenn der Kopf den Reiter hat: {out}"
        );
        assert!(out.contains("<a href=\"?seite=2\">weiter</a>"), "fremde Abfragen bleiben, wie sie sind: {out}");
        assert!(
            out.contains("<a href=\"?laptop=a b\">x</a>"),
            "ein Wert mit anderen Zeichen als [A-Za-z0-9_-] wird nicht angefasst: {out}"
        );
    }

    #[test]
    fn set_cookies_remembers_only_valid_values() {
        assert_eq!(
            set_cookies(Some("work=1&band=closed")),
            ["baruch_band=closed; Path=/; Max-Age=31536000; SameSite=Lax"],
            "zugeklappt wird ein Jahr gemerkt"
        );
        assert_eq!(
            set_cookies(Some("band=open")),
            ["baruch_band=; Path=/; Max-Age=0; SameSite=Lax"],
            "offen ist die Vorgabe: das Cookie wird gelöscht"
        );
        assert!(set_cookies(Some("band=zu")).is_empty(), "ungültiger Wert: nichts merken");
        assert!(set_cookies(Some("tab=buch")).is_empty(), "der Reiter wird nicht gemerkt");
        assert!(set_cookies(None).is_empty(), "ohne Abfrage nichts");
    }

    #[test]
    fn tabs_are_the_five_from_the_contract() {
        let ids: Vec<&str> = TABS.iter().map(|(id, _)| *id).collect();
        assert_eq!(
            ids,
            ["start", "einfuegen", "pruefen", "buch", "ansicht"],
            "Kennungen und Reihenfolge der Reiter (§2/§3.2)"
        );
    }

    #[test]
    fn the_template_carries_the_literal_forms_apply_relies_on() {
        // apply replaces strings: if someone changes the attribute order in base.html, the ribbon would stay mute.
        let base = include_str!("../templates/base.html");
        for (id, _) in TABS {
            assert!(
                base.contains(&format!("<a class=\"band-tab\" role=\"tab\" id=\"band-tab-{id}\" aria-controls=\"band-panel-{id}\" data-band-tab=\"{id}\" href=\"?tab={id}\" aria-selected=\"false\">")),
                "Reiter „{id}“ in der wörtlichen Form des Vertrags"
            );
            assert!(
                base.contains(&format!("<div class=\"band-panel\" role=\"tabpanel\" id=\"band-panel-{id}\" aria-labelledby=\"band-tab-{id}\" data-band-panel=\"{id}\" hidden>")),
                "Feld „{id}“ in der wörtlichen Form des Vertrags"
            );
        }
        assert!(base.contains("<header class=\"band\" data-widget=\"band\" data-tab=\"\""), "Kopf mit leerem data-tab");
        assert_eq!(
            base.matches("<details class=\"band-group\"").count(),
            base.matches(" data-band-group open><summary class=\"band-group-label\">").count(),
            "jede Gruppe endet wörtlich auf „ data-band-group open>“ (sonst bliebe sie im kompakten Band offen)"
        );
        for form in [
            "<a class=\"btn\" data-laptop-set=\"auto\" aria-pressed=\"false\" href=\"?laptop=auto\">Automatisch</a>",
            "<a class=\"btn\" data-laptop-set=\"on\" aria-pressed=\"false\" href=\"?laptop=on\">An</a>",
            "<a class=\"btn\" data-laptop-set=\"off\" aria-pressed=\"false\" href=\"?laptop=off\">Aus</a>",
            "<a class=\"btn\" data-rail-toggle=\"left\" aria-pressed=\"false\" href=\"?lrail=1\">Gliederung</a>",
            "<a class=\"btn\" data-rail-toggle=\"right\" aria-pressed=\"false\" href=\"?rrail=1\">Assistenten</a>",
            "<button class=\"btn js-only\" type=\"button\" data-zoom-reset>Zoom zurücksetzen</button>",
            "data-band-toggle aria-expanded=\"true\" href=\"?band=closed\"",
        ] {
            assert!(base.contains(form), "wörtliche Form fehlt in base.html: {form}");
        }
        for mark in ["{#- R32-PLATZ import-word -#}", "{#- R32-PLATZ beenden -#}", "{#- R32-PLATZ export-word -#}"] {
            assert_eq!(base.matches(mark).count(), 1, "Platz für Runde 32 genau einmal: {mark}");
        }
    }
}
