// SPDX-License-Identifier: AGPL-3.0-or-later
//! Data and badges of the ribbon (Round 33; until then a header with the menus Werk ▾, Ansicht ▾, Werkzeuge ▾
//! of Round 9).
//!
//! The ribbon lives in `templates/base.html` (`{% block topbar %}`): menu **Baruch ▾**, tabs Start · Einfügen ·
//! Prüfen · Buch · Ansicht with groups; every page supplies `{% block page_title %}`, the default tab
//! (`{% block band_tab %}`) and its groups (`page_actions`, `band_*`). All of them are ordinary links and
//! `<details>`, so usable without JavaScript; the active tab and the addresses are inserted into the finished HTML
//! by `crate::band::apply`. `web/src/menu.ts` (widget `menubar`) adds the keyboard to the menu „Baruch ▾“ following
//! the ARIA menu pattern, `web/src/band.ts` the tabs.
//!
//! Data of the ribbon, in this order of precedence: (1) the variable `topbar` (only the work page, built
//! by [`workspace_topbar`]), (2) `work.id`/`work.title` if `work` is a mapping, (3) the
//! work from the cookie `baruch_work` (`settings.work`). If everything is missing, there are only the tabs Start and
//! Ansicht.
//!
//! Badges: open proposals in the work (tab Start), open findings (tab Prüfen „N offen“ and entry
//! „Befunde“; counting rule contract R28 §1.5, sum of `Store::finding_open_counts`) and open requests for the
//! outline (tab Buch and entry „Anträge zur Gliederung“). The work page draws them in the server; `menu.ts`
//! refreshes them through the RPC method `menu.badges {work?}` → `{work, outline_requests, open_proposals,
//! open_findings}`.
//!
//! Round 23a: also without JavaScript the numbers and the entry „Anträge zur Gliederung“ are in the served HTML on
//! **every** page. Outside of the work page `base.html` labels the badges with `data-badge-fill` and
//! the menu bar with `data-badges-work="<Werk>"`; the layer `consent::inject` (above all routes) calls
//! [`fill_page`], which inserts the numbers from [`badges`] ([`fill_badges`]). `menu.ts` only refreshes them.
//!
//! Round 24: „Über Baruch“ (`/ueber`, `crate::about`) is in the menu **Baruch ▾** – an ordinary link
//! without a work in the address (the page shows version, licence and source code and needs no work).

use baruch_core::Work;
use baruch_store::WorkId;
use minijinja::{Value, context};
use serde_json::json;

use crate::AppState;
use crate::rpc::{self, INVALID_PARAMS, LibraryView, METHOD_NOT_FOUND, RpcError};
use crate::view::{self, Resolved, Shelf};

/// RPC methods of this module; `rpc::call` asks [`handles`] before choosing the default work.
pub const METHODS: &[&str] = &["menu.badges"];

pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

/// Numbers of the badges at the menus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Badges {
    /// Open (not yet decided) requests for the outline, badge at the tab „Buch“.
    pub outline_requests: usize,
    /// Open proposals in the whole work, badge at the tab „Start“.
    pub open_proposals: usize,
    /// Round 28: open findings in the whole work (counting rule §1.5), badge at the tab „Prüfen“ and at the entry
    /// „Befunde“.
    pub open_findings: usize,
}

/// Open proposals across all sections of the work.
pub fn open_proposals(work: &Work) -> usize {
    work.nodes.iter().map(|n| n.open_proposals()).sum()
}

/// Badges of a work from the store (the same numbers as on the work page).
pub async fn badges(state: &AppState, work: WorkId) -> Result<Badges, RpcError> {
    let outline_requests = state.store.open_outline_requests(work).await?;
    let loaded = state.store.load_work(work).await?;
    let open_findings = state.store.finding_open_counts(work).await?.values().sum();
    Ok(Badges { outline_requests, open_proposals: open_proposals(&loaded), open_findings })
}

/// Marker of a badge that the server fills in the finished HTML (`base.html`, outside of the work page).
const FILL: &str = " data-badge-fill";

/// Work of the ribbon from `data-badges-work="<Zahl>"` (`base.html`); `None` on the work page (there the
/// template draws the numbers itself) and on pages without a work.
pub fn page_work(page: &str) -> Option<WorkId> {
    let rest = &page[page.find("data-badges-work=\"")? + "data-badges-work=\"".len()..];
    rest[..rest.find('"')?].parse().ok().filter(|&work: &WorkId| work > 0)
}

/// Inserts the numbers into the badges (`data-badge="<name>"`) and entries (`data-badge-item="<name>"`) labelled
/// with `data-badge-fill`: for a number above zero `hidden` is dropped and the zero in the badge becomes the
/// number („3“ or „3 offen“ – the text after it stays). At zero the element stays hidden. The marker
/// itself is always removed. Unknown names stay hidden.
pub fn fill_badges(page: &str, badges: Badges) -> String {
    let mut out = String::with_capacity(page.len() + 16);
    let mut rest = page;
    while let Some(at) = rest.find(FILL) {
        let (before, after) = rest.split_at(at);
        let after = &after[FILL.len()..];
        // The marker is in the opening tag: name from `data-badge="…"` or `data-badge-item="…"` before it.
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
        // With `hidden` after it: make visible; the zero directly after the tag becomes the number.
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

/// Finished page with the badges of its work ([`page_work`], [`badges`], [`fill_badges`]). Without a marker the
/// page stays unchanged; if the work cannot be read (deleted), the badges stay hidden.
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

/// `menu.badges {work?}` → `{work, outline_requests, open_proposals, open_findings}`; if `work` is missing, the
/// default work. Parameters that are not an object, or an unknown work → `INVALID_PARAMS`.
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
        _ => Err(RpcError::new(METHOD_NOT_FOUND, format!("unknown method {method}"))),
    }
}

/// Entries of the work selection („Titel · Autor · n Abschnitte“), the work shown selected.
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

/// Data of the ribbon on the work page: work, section shown (`key`, `rev`), view,
/// search of the literature (`lq_q`, finished as `&lq=…`), work selection and the badges from the server.
/// `base.html` builds the addresses from it (key form `/?work=W&key=K&rev=R…`, never `node=`).
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
