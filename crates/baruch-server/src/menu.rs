// SPDX-License-Identifier: AGPL-3.0-or-later
//! Daten und Abzeichen des Menübands (Runde 33; bis dahin Kopfzeile mit den Menüs Werk ▾, Ansicht ▾, Werkzeuge ▾
//! der Runde 9).
//!
//! Das Band wohnt in `templates/base.html` (`{% block topbar %}`): Menü **Baruch ▾**, Reiter Start · Einfügen ·
//! Prüfen · Buch · Ansicht mit Gruppen; jede Seite liefert `{% block page_title %}`, den Vorgabe-Reiter
//! (`{% block band_tab %}`) und ihre Gruppen (`page_actions`, `band_*`). Alles sind gewöhnliche Links und
//! `<details>`, also ohne JavaScript bedienbar; den aktiven Reiter und die Adressen setzt `crate::band::apply` im
//! fertigen HTML ein. `web/src/menu.ts` (Widget `menubar`) ergänzt am Menü „Baruch ▾“ die Tastatur nach dem
//! ARIA-Menü-Muster, `web/src/band.ts` die Reiter.
//!
//! Daten des Bands, in dieser Rangfolge: (1) die Variable `topbar` (nur die Arbeitsseite, gebaut
//! von [`workspace_topbar`]), (2) `work.id`/`work.title`, wenn `work` eine Abbildung ist, (3) das
//! Werk aus dem Cookie `baruch_work` (`settings.work`). Fehlt alles, gibt es nur die Reiter Start und Ansicht.
//!
//! Abzeichen: offene Vorschläge im Werk (Reiter Start), offene Befunde (Reiter Prüfen „N offen“ und Eintrag
//! „Befunde“; Zählregel Vertrag R28 §1.5, Summe von `Store::finding_open_counts`) und offene Anträge zur Gliederung
//! (Reiter Buch und Eintrag „Anträge zur Gliederung“). Die Arbeitsseite zeichnet sie im Server; `menu.ts` frischt
//! sie über die RPC-Methode `menu.badges {work?}` → `{work, outline_requests, open_proposals, open_findings}` auf.
//!
//! Runde 23a: Auch ohne JavaScript stehen die Zahlen und der Eintrag „Anträge zur Gliederung“ auf **jeder** Seite im
//! ausgelieferten HTML. `base.html` kennzeichnet außerhalb der Arbeitsseite die Abzeichen mit `data-badge-fill` und
//! die Menüleiste mit `data-badges-work="<Werk>"`; die Schicht `consent::inject` (über allen Routen) ruft
//! [`fill_page`], das die Zahlen aus [`badges`] einsetzt ([`fill_badges`]). `menu.ts` frischt sie nur noch auf.
//!
//! Runde 24: „Über Baruch“ (`/ueber`, `crate::about`) steht im Menü **Baruch ▾** – ein gewöhnlicher Link
//! ohne Werk in der Adresse (die Seite zeigt Version, Lizenz und Quellcode und braucht kein Werk).

use baruch_core::Work;
use baruch_store::WorkId;
use minijinja::{Value, context};
use serde_json::json;

use crate::AppState;
use crate::rpc::{self, INVALID_PARAMS, LibraryView, METHOD_NOT_FOUND, RpcError};
use crate::view::{self, Resolved, Shelf};

/// RPC-Methoden dieses Moduls; `rpc::call` fragt [`handles`] vor der Wahl des Vorgabewerks.
pub const METHODS: &[&str] = &["menu.badges"];

pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

/// Zahlen der Abzeichen an den Menüs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Badges {
    /// Offene (noch nicht entschiedene) Anträge zur Gliederung, Abzeichen am Reiter „Buch“.
    pub outline_requests: usize,
    /// Offene Vorschläge im ganzen Werk, Abzeichen am Reiter „Start“.
    pub open_proposals: usize,
    /// Runde 28: offene Befunde im ganzen Werk (Zählregel §1.5), Abzeichen am Reiter „Prüfen“ und am Eintrag „Befunde“.
    pub open_findings: usize,
}

/// Offene Vorschläge über alle Abschnitte des Werks.
pub fn open_proposals(work: &Work) -> usize {
    work.nodes.iter().map(|n| n.open_proposals()).sum()
}

/// Abzeichen eines Werks aus dem Speicher (dieselben Zahlen wie auf der Arbeitsseite).
pub async fn badges(state: &AppState, work: WorkId) -> Result<Badges, RpcError> {
    let outline_requests = state.store.open_outline_requests(work).await?;
    let loaded = state.store.load_work(work).await?;
    let open_findings = state.store.finding_open_counts(work).await?.values().sum();
    Ok(Badges { outline_requests, open_proposals: open_proposals(&loaded), open_findings })
}

/// Kennzeichen eines Abzeichens, das der Server im fertigen HTML füllt (`base.html`, außerhalb der Arbeitsseite).
const FILL: &str = " data-badge-fill";

/// Werk des Bands aus `data-badges-work="<Zahl>"` (`base.html`); `None` auf der Arbeitsseite (dort zeichnet die
/// Vorlage die Zahlen selbst) und auf Seiten ohne Werk.
pub fn page_work(page: &str) -> Option<WorkId> {
    let rest = &page[page.find("data-badges-work=\"")? + "data-badges-work=\"".len()..];
    rest[..rest.find('"')?].parse().ok().filter(|&work: &WorkId| work > 0)
}

/// Setzt die Zahlen in die mit `data-badge-fill` gekennzeichneten Abzeichen (`data-badge="<name>"`) und Einträge
/// (`data-badge-item="<name>"`) ein: Bei einer Zahl über null fällt `hidden` weg und die Null im Abzeichen wird die
/// Zahl („3“ bzw. „3 offen“ – der Text dahinter bleibt). Bei null bleibt das Element verborgen. Das Kennzeichen
/// selbst wird immer entfernt. Unbekannte Namen bleiben verborgen.
pub fn fill_badges(page: &str, badges: Badges) -> String {
    let mut out = String::with_capacity(page.len() + 16);
    let mut rest = page;
    while let Some(at) = rest.find(FILL) {
        let (before, after) = rest.split_at(at);
        let after = &after[FILL.len()..];
        // Das Kennzeichen steht im öffnenden Tag: Name aus `data-badge="…"` bzw. `data-badge-item="…"` davor.
        let tag = &before[before.rfind('<').unwrap_or(0)..];
        let name = ["data-badge=\"", "data-badge-item=\""].iter().find_map(|attr| {
            let value = &tag[tag.find(attr)? + attr.len()..];
            value.find('"').map(|end| &value[..end])
        });
        let count = match name {
            Some("outline_requests") => badges.outline_requests,
            Some("open_proposals") => badges.open_proposals,
            Some("open_findings") => badges.open_findings,
            _ => 0,
        };
        out.push_str(before);
        rest = after;
        if count == 0 {
            continue;
        }
        // Mit `hidden` dahinter: sichtbar machen; die Null direkt hinter dem Tag wird die Zahl.
        let visible = rest.strip_prefix(" hidden").unwrap_or(rest);
        match visible.strip_prefix(">0") {
            Some(tail) if tag.contains("data-badge=\"") => {
                out.push('>');
                out.push_str(&count.to_string());
                rest = tail;
            }
            _ => rest = visible,
        }
    }
    out.push_str(rest);
    out
}

/// Fertige Seite mit den Abzeichen ihres Werks ([`page_work`], [`badges`], [`fill_badges`]). Ohne Kennzeichen bleibt
/// die Seite unverändert; lässt sich das Werk nicht lesen (gelöscht), bleiben die Abzeichen verborgen.
pub async fn fill_page(state: &AppState, page: String) -> String {
    if !page.contains(FILL) {
        return page;
    }
    let found = match page_work(&page) {
        Some(work) => badges(state, work).await.ok(),
        None => None,
    };
    fill_badges(&page, found.unwrap_or(Badges { outline_requests: 0, open_proposals: 0, open_findings: 0 }))
}

/// `menu.badges {work?}` → `{work, outline_requests, open_proposals, open_findings}`; fehlt `work`, das Vorgabewerk.
/// Parameter, die kein Objekt sind, oder ein unbekanntes Werk → `INVALID_PARAMS`.
pub async fn call(state: &AppState, method: &str, params: serde_json::Value) -> Result<serde_json::Value, RpcError> {
    match method {
        "menu.badges" => {
            if !(params.is_null() || params.is_object()) {
                return Err(RpcError::new(INVALID_PARAMS, "menu.badges: Parameter als Objekt erwartet"));
            }
            let work = rpc::work_param(state, &params).await?;
            let badges = badges(state, work).await?;
            Ok(json!({
                "work": work,
                "outline_requests": badges.outline_requests,
                "open_proposals": badges.open_proposals,
                "open_findings": badges.open_findings,
            }))
        }
        _ => Err(RpcError::new(METHOD_NOT_FOUND, format!("unbekannte Methode {method}"))),
    }
}

/// Einträge der Werkwahl („Titel · Autor · n Abschnitte“), das gezeigte Werk gewählt.
fn work_options(shelf: &Shelf) -> Vec<Value> {
    shelf
        .works
        .iter()
        .map(|w| {
            let sections = if w.sections == 1 { "1 Abschnitt".to_owned() } else { format!("{} Abschnitte", w.sections) };
            let label = match w.author.trim() {
                "" => format!("{} · {sections}", w.title),
                author => format!("{} · {author} · {sections}", w.title),
            };
            context! { id => w.id, slug => w.slug.as_str(), title => w.title.as_str(), label => label, selected => w.id == shelf.id }
        })
        .collect()
}

/// Daten des Menübands auf der Arbeitsseite: Werk, gezeigter Abschnitt (`key`, `rev`), Ansicht,
/// Suche der Literatur (`lq_q`, fertig als `&lq=…`), Werkwahl und die Abzeichen aus dem Server.
/// `base.html` baut daraus die Adressen (Key-Form `/?work=W&key=K&rev=R…`, nie `node=`).
pub fn workspace_topbar(work: &Work, shelf: &Shelf, at: &Resolved, library: &LibraryView) -> Value {
    context! {
        work => context! { id => shelf.id, title => work.title.as_str(), author => work.author.as_str() },
        key => at.key,
        rev => at.rev,
        view => if at.preview { "preview" } else { "edit" },
        other_view => if at.preview { "edit" } else { "preview" },
        lq_q => view::lq_param(library.query.as_deref()),
        works => work_options(shelf),
        badges => context! {
            outline_requests => library.outline_requests,
            open_proposals => open_proposals(work),
            open_findings => library.extras.findings.open_by_key.values().sum::<usize>(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEAD: &str = "<nav class=\"menubar\" data-badges-work=\"7\">\
        <span class=\"badge\" data-badge=\"outline_requests\" data-badge-fill hidden>0</span> \
        <span class=\"badge warning\" data-badge=\"open_findings\" title=\"offene Befunde im Werk\" data-badge-fill hidden>0 offen</span>\
        <a href=\"/gliederung?work=7#antraege\" data-badge-item=\"outline_requests\" data-badge-fill hidden>Anträge zur Gliederung \
        <span class=\"badge\" data-badge=\"outline_requests\" data-badge-fill>0</span></a>\
        <span class=\"badge\" data-badge=\"open_proposals\" data-badge-fill hidden>0</span></nav>";

    #[test]
    fn counts_above_zero_become_visible_numbers_and_zero_stays_hidden() {
        assert_eq!(page_work(HEAD), Some(7), "das Werk steht an der Menüleiste");
        let filled = fill_badges(HEAD, Badges { outline_requests: 3, open_proposals: 0, open_findings: 12 });
        assert!(!filled.contains("data-badge-fill"), "das Kennzeichen ist überall entfernt: {filled}");
        assert!(
            filled.contains("<span class=\"badge\" data-badge=\"outline_requests\">3</span> "),
            "Abzeichen am Menü „Werk“ sichtbar mit der Zahl: {filled}"
        );
        assert!(
            filled.contains("title=\"offene Befunde im Werk\">12 offen</span>"),
            "die Befunde behalten das Wort „offen“ hinter der Zahl: {filled}"
        );
        assert!(
            filled.contains("data-badge-item=\"outline_requests\">Anträge zur Gliederung"),
            "der Eintrag „Anträge zur Gliederung“ ist nicht mehr verborgen: {filled}"
        );
        assert!(
            filled.contains("<span class=\"badge\" data-badge=\"outline_requests\">3</span></a>"),
            "auch das Abzeichen im Eintrag trägt die Zahl: {filled}"
        );
        assert!(
            filled.contains("<span class=\"badge\" data-badge=\"open_proposals\" hidden>0</span>"),
            "null offene Vorschläge: das Abzeichen bleibt verborgen: {filled}"
        );
    }

    #[test]
    fn a_page_without_marks_or_work_is_left_alone() {
        let plain = "<nav class=\"menubar\"><span class=\"badge\" data-badge=\"open_proposals\">4</span></nav>";
        let some = Badges { outline_requests: 9, open_proposals: 9, open_findings: 9 };
        assert_eq!(fill_badges(plain, some), plain, "die Arbeitsseite zeichnet selbst: nichts wird angefasst");
        assert_eq!(page_work(plain), None, "ohne Kennzeichen kein Werk");
        assert_eq!(page_work("<nav data-badges-work=\"0\">"), None, "0 ist kein Werk");
        assert_eq!(page_work("<nav data-badges-work=\"x\">"), None, "nur Zahlen");
    }
}
