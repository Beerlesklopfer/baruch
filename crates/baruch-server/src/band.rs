// SPDX-License-Identifier: AGPL-3.0-or-later
//! Menüband (Runde 33; Besitz: Aufgabe „menueband“).
//!
//! Das Band steht vollständig in `templates/base.html` (die Vorlage ist in vielen eigenen Umgebungen eingebettet,
//! ein Fragment müsste in allen registriert werden). Seiten kennen ihre Adresse und `?tab=` nicht; darum setzt die
//! Schicht `consent::inject` über [`apply`] den aktiven Reiter, die Adressen der Reiter und den Laptop-Zustand im
//! fertigen HTML ein – wie die Abzeichen seit Runde 23a (`menu::fill_badges`).
//!
//! Zustand ([`BandState::from_request`]): `?tab=start|einfuegen|pruefen|buch|ansicht` (nicht gemerkt) und
//! `?band=open|closed` (vor dem Cookie `baruch_band`, gemerkt über [`set_cookies`]). Ungültige Werte werden
//! übergangen. Alles Übrige der Abfrage bleibt als `rest` an den Adressen der Reiter und Schalter hängen, damit ein
//! Klick ohne JavaScript Werk, Abschnitt, Fassung oder Seite nicht verliert.
//!
//! [`apply`] ersetzt nur wörtliche Formen der Vorlage (Reihenfolge der Attribute wie in `base.html`); Werte aus der
//! Adresse (`rest`) gehen ausschließlich maskiert in Attribute ([`escape_attr`]). Die Schritte 2 bis 6 wirken nur im
//! Kopf (`<header class="band"` bis `</header>`), damit Text im Seiteninhalt, der zufällig wie ein Reiter aussieht,
//! nichts verstellt; die Schalter (Schritte 7 und 8) gelten auf der ganzen Seite (auch für die schmalen Leisten der
//! Arbeitsseite).

use crate::laptop::LaptopState;

/// Marke an `<html>` in `base.html`; [`apply`] ersetzt sie durch die Attribute des Zustands.
pub const FILL: &str = " data-band-fill";
/// Reiter des Bands: (Kennung in `?tab=`, Beschriftung).
pub const TABS: [(&str, &str); 5] =
    [("start", "Start"), ("einfuegen", "Einfügen"), ("pruefen", "Prüfen"), ("buch", "Buch"), ("ansicht", "Ansicht")];
/// Schlüssel der Adresse, die diesem Modul gehören.
pub const QUERY_KEYS: [&str; 2] = ["tab", "band"];

/// Cookie, das ein zugeklapptes Band merkt (`closed`; fehlt = offen).
const COOKIE: &str = "baruch_band";
/// Anfang und Ende des Kopfs in der fertigen Seite.
const HEAD_OPEN: &str = "<header class=\"band\"";
const HEAD_CLOSE: &str = "</header>";
/// Schlüssel, deren Schalter (`href="?K=V"`) den Rest der Abfrage und den aktiven Reiter mitnehmen (Schritt 7).
const SWITCH_KEYS: [&str; 4] = ["band", "laptop", "lrail", "rrail"];

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BandState {
    /// Reiter aus `?tab=`, nur wenn er in TABS steht.
    pub tab: Option<&'static str>,
    /// Band zugeklappt (`?band=` vor Cookie `baruch_band`).
    pub closed: bool,
    /// Rohe Abfrage der Anfrage ohne die Schlüssel aus band::QUERY_KEYS und laptop::QUERY_KEYS (Paare mit `&`, unverändert, nicht dekodiert).
    pub rest: String,
}

/// Paare der rohen Abfrage als (Schlüssel, Wert, ganzes Paar); leere Paare (`&&`) fallen weg. Nichts wird dekodiert:
/// Ein Schlüssel gilt nur in seiner wörtlichen Schreibweise.
fn pairs(query: Option<&str>) -> impl Iterator<Item = (&str, &str, &str)> {
    query.unwrap_or("").split('&').filter(|pair| !pair.is_empty()).map(|pair| {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        (key, value, pair)
    })
}

/// Letzter gültiger Wert von `?band=` (`Some(true)` = zugeklappt); `None`, wenn keiner gültig ist.
fn band_in_query(query: Option<&str>) -> Option<bool> {
    pairs(query)
        .filter_map(|(key, value, _)| match (key, value) {
            ("band", "closed") => Some(true),
            ("band", "open") => Some(false),
            _ => None,
        })
        .last()
}

/// Wert eines Cookies aus dem Kopf `Cookie:` (`name=wert; name2=wert2`).
fn cookie<'a>(header: Option<&'a str>, name: &str) -> Option<&'a str> {
    header?.split(';').find_map(|part| part.trim().strip_prefix(name)?.strip_prefix('='))
}

impl BandState {
    /// Zustand aus Cookie-Kopf und roher Abfrage der Anfrage: `?tab=` nur für Kennungen aus [`TABS`] (der letzte
    /// gültige Wert gilt), `?band=open|closed` vor dem Cookie `baruch_band`, `rest` ohne die Schlüssel beider Module.
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

/// Fertige Set-Cookie-Werte für einen gültigen Schlüssel `band` in der Adresse: `closed` merkt das Cookie ein Jahr,
/// `open` löscht es (fehlt = offen). Ohne gültigen Wert bleibt die Liste leer.
pub fn set_cookies(query: Option<&str>) -> Vec<String> {
    match band_in_query(query) {
        Some(true) => vec![format!("{COOKIE}=closed; Path=/; Max-Age=31536000; SameSite=Lax")],
        Some(false) => vec![format!("{COOKIE}=; Path=/; Max-Age=0; SameSite=Lax")],
        None => Vec::new(),
    }
}

/// Maskiert einen Wert für ein HTML-Attribut in doppelten oder einfachen Anführungszeichen (`&`, `<`, `>`, `"`, `'`).
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

/// Kennungen der Reiter, die im Kopf stehen (`data-band-tab="ID"`), in der Reihenfolge der Seite; nur bekannte.
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

/// Wert von `data-band-default="…"` im Kopf.
fn default_tab(head: &str) -> Option<&str> {
    let rest = &head[head.find("data-band-default=\"")? + "data-band-default=\"".len()..];
    Some(rest[..rest.find('"')?].trim())
}

/// Schritt 2: aktiver Reiter = `?tab=`, falls die Seite ihn hat; sonst die Vorgabe der Seite, falls vorhanden; sonst
/// der erste Reiter der Seite (ohne Reiter „start“).
fn active_tab(head: &str, wanted: Option<&'static str>) -> &'static str {
    let tabs = tabs_in(head);
    wanted
        .filter(|tab| tabs.contains(tab))
        .or_else(|| default_tab(head).and_then(|name| tabs.iter().copied().find(|tab| *tab == name)))
        .or_else(|| tabs.first().copied())
        .unwrap_or(TABS[0].0)
}

/// Schritte 3 bis 6 im Kopf: `data-tab`, Adressen und `aria-selected` der Reiter, `hidden` am aktiven Feld, Gruppen
/// im kompakten Band als Popups. `prefix` ist der maskierte Rest der Abfrage samt `&amp;` (oder leer).
fn fill_head(head: &str, active: &str, band: &BandState, prefix: &str, compact: bool) -> String {
    let mut out = head.replacen("data-tab=\"\"", &format!("data-tab=\"{active}\""), 1);
    for (id, _) in TABS {
        let template = format!("data-band-tab=\"{id}\" href=\"?tab={id}\" aria-selected=\"false\"");
        if !out.contains(&template) {
            continue;
        }
        let is_active = id == active;
        // Offen: der aktive Reiter klappt zu, die anderen wechseln nur. Zu: jeder Reiter öffnet das Band.
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

/// Schritt 8: gedrückter Laptop-Schalter, Seitenleisten (gedrückt = sichtbar; eingeklappt zeigt die Adresse auf
/// „ausklappen“) und der Band-Schalter (zugeklappt: Adresse auf `band=open`).
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

/// Schritt 7: jedes `href="?K=V"` mit genau einem Paar und K ∈ [`SWITCH_KEYS`] bekommt den Rest der Abfrage und den
/// aktiven Reiter davor (`href="?<rest>&tab=<aktiv>&K=V"`). V nur aus `[A-Za-z0-9_-]`, sonst bleibt der Link, wie er ist.
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

/// Setzt den Zustand in eine fertige Seite ein; eine Seite ohne [`FILL`] bleibt unverändert. Schritte (Vertrag R33 §4):
/// 1. Marke an `<html>` → Attribute des Laptop-Zustands und `data-band="open|closed"`;
/// 2. aktiver Reiter ([`active_tab`]); 3. `data-tab` am Kopf; 4. Adressen der Reiter mit dem Rest der Abfrage,
///    `aria-selected`, `band=closed` am aktiven bzw. `band=open` an allen bei zugeklapptem Band;
/// 5. `hidden` fällt am aktiven Feld weg; 6. kompakt: Gruppen ohne `open`, mit `name="band-group"` (nur eine offen);
/// 7. Schalter-Adressen (`?band=`, `?laptop=`, `?lrail=`, `?rrail=`) mit Rest und aktivem Reiter;
/// 8. `aria-pressed` der Laptop- und Leisten-Schalter.
pub fn apply(page: &str, band: &BandState, laptop: &LaptopState) -> String {
    if !page.contains(FILL) {
        return page.to_owned();
    }
    let state = if band.closed { "closed" } else { "open" };
    let page = page.replacen(FILL, &format!(" {} data-band=\"{state}\"", laptop.html_attrs()), 1);
    // Rest der Abfrage: kommt aus der Adresse, also nur maskiert in ein Attribut. `node=` (Stelle im Baum, alte
    // Adressform) geht nicht mit: Erzeugte Links tragen nie `node=` (Vertrag Runde 8 §1), sondern `key=`; eine
    // Seite, die noch mit `?node=` aufgerufen wurde, verliert beim Reiterwechsel ohne JavaScript nur diese Stelle.
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
        // Seite ohne Kopf (eigener Block topbar): nur die Schalter, mit dem gewünschten Reiter.
        None => (page, band.tab.unwrap_or(TABS[0].0)),
    };
    let page = fill_switches(&page, band, laptop);
    fill_switch_links(&page, active, &prefix)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::laptop::{LaptopState, Mode, Width};

    /// Kopf in den wörtlichen Formen von `base.html`, mit den Reitern `tabs` und der Vorgabe `default`.
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
        // Fragmente und Seiten, die base.html nicht erweitern, tragen die Marke nicht.
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
        // Seite mit Werk, aber nicht die Arbeitsseite: kein Reiter „Einfügen“.
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
        // Vorgabe, die es auf der Seite nicht gibt (Seite ohne Werk mit Vorgabe „buch“): der erste Reiter.
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
        // Die Abfrage kommt roh aus der Adresse: Ein Anführungszeichen dürfte das Attribut nicht beenden.
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
        // „An“ klappt beide Leisten ein: nicht gedrückt, die Adresse klappt aus.
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
        // apply ersetzt Zeichenketten: Ändert jemand die Reihenfolge der Attribute in base.html, bliebe das Band stumm.
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
