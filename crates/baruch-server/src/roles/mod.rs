// SPDX-License-Identifier: AGPL-3.0-or-later
//! Roles (Round 35, contract §S1.4 and §2): the ONLY place of the rights check.
//!
//! The account kind is global, the role holds per work ([`role_in`]): an author account is author in every work, a
//! Lektor account is Lektor only in released works (`work_member`), otherwise it has no role (work invisible).
//!
//! For Lektors everything is forbidden that is not explicitly allowed: [`check_request`] checks every route against
//! [`LEKTOR_ROUTES`] (called from [`crate::auth::gate`]), [`check_rpc`] every RPC method against [`LEKTOR_RPC`]
//! (called from [`crate::rpc::call`], that is for `/rpc`, `/act` and `/lektorat/act`). New routes and methods
//! of future rounds are thereby locked for Lektors without any further action. Form handlers of the modules
//! (`/beitraege/act` etc.) call their module directly and do not go through `rpc::call` – that is why they are not
//! in [`LEKTOR_ROUTES`].
//!
//! Work binding: an allowed route or method holds only for released works. The work stands in the path (`{id}`),
//! in the address (`?work=`) or in the RPC parameter `work`; a work that is not released is rejected with the same
//! message as an unknown one (a Lektor does not learn which works exist). If the work is missing, never the
//! default work of the instance applies, but the smallest released one: inserted for RPC, as a redirect
//! (303) to the same address with `work=` for GET pages. A `work` in the address or in the parameters is checked for
//! EVERY allowed route and method, even if it needs no work itself.
//!
//! Roles are never remembered beyond one request: every check reads the releases anew
//! (`Store::member_works`, which delivers something only for active Lektor accounts) – withdrawal and lock take
//! effect with the next request.

pub mod lektor;
pub mod routes;
pub mod rpc;

use axum::http::Method;
use baruch_store::{StoreError, WorkId, WorkSummary};
use serde::Serialize;
use serde_json::{Value, json};

use crate::AppState;
use crate::auth::Who;
use crate::rpc::{FORBIDDEN, INVALID_PARAMS, RpcError, UNAUTHENTICATED};

/// Role in a work.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Author,
    Lektor,
}

/// Verdict on a request: allowed, redirect (303) or rejected (403 with a reason).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Allow,
    Redirect(String),
    Deny(String),
}

/// Where an allowed route or method takes its work from: not at all, from the path (`{id}`), from the address
/// (`?work=`) or from the RPC parameter `work`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkFrom {
    None,
    Path,
    Query,
    Param,
}

/// Routes a Lektor may call: method (`GET` | `POST`), pattern as in the router, origin of the work
/// (contract §2.1; reasons in `docs/Architektur.md`, „Runde 35“). No upload, no `/act`, no form of a module
/// (`/beitraege/act` etc. call their module directly, without `rpc::call`).
pub const LEKTOR_ROUTES: &[(&str, &str, WorkFrom)] = &[
    ("GET", "/", WorkFrom::Query),
    ("GET", "/werke", WorkFrom::None),
    ("GET", "/beitraege", WorkFrom::Query),
    ("GET", "/beitrag", WorkFrom::Query),
    ("GET", "/faeden", WorkFrom::Query),
    ("GET", "/faden", WorkFrom::Query),
    ("GET", "/suche", WorkFrom::Query),
    ("GET", "/gliederung", WorkFrom::Query),
    ("GET", "/werke/{id}/befunde", WorkFrom::Path),
    ("GET", "/werke/{id}/fassungen/verlauf", WorkFrom::Path),
    ("GET", "/werke/{id}/glossar", WorkFrom::Path),
    ("GET", "/werke/{id}/register", WorkFrom::Path),
    ("GET", "/werke/{id}/bilder/{name}/datei", WorkFrom::Path),
    ("GET", "/werke/{id}/bilder/{name}/vorschau", WorkFrom::Path),
    ("GET", "/ueber", WorkFrom::None),
    ("GET", "/ueber/lizenz", WorkFrom::None),
    ("GET", "/konto", WorkFrom::None),
    ("GET", "/anmelden", WorkFrom::None),
    // Without a work (Round 36, before `WorkFrom::Query`): the page takes its work from `settings::resolve_work`,
    // which knows only released works for a Lektor (`AppState::default_work`, cookie `baruch_work` only if
    // released) – this way a Lektor without a release reaches his display settings as well. A `work` in the address
    // is still checked (see `check_request_preferring`).
    ("GET", "/einstellungen/darstellung", WorkFrom::None),
    ("GET", "/static/app.css", WorkFrom::None),
    ("GET", "/static/favicon.svg", WorkFrom::None),
    ("GET", "/static/js/{name}", WorkFrom::None),
    ("POST", "/rpc", WorkFrom::None),
    ("POST", "/lektorat/act", WorkFrom::None),
    ("POST", "/abmelden", WorkFrom::None),
    ("POST", "/konto/passwort", WorkFrom::None),
    ("POST", "/einstellungen/darstellung/act", WorkFrom::None),
];

/// RPC methods a Lektor may call, with the origin of the work (contract §2.2; reasons in
/// `docs/Architektur.md`, „Runde 35“). [`WorkFrom::Param`]: if `work` is missing, the smallest released work is
/// inserted.
///
/// Deviations from the start of the contract, purely reading:
/// - `wysiwyg.*` and `ribbon.formats` are "without a work" again since Round 36 ([`WorkFrom::None`]; in Round 35
///   [`WorkFrom::Param`]): without `work`, `rpc::work_param` takes [`AppState::default_work`], which gives a Lektor
///   only the smallest released work and without a release `UnknownWork(0)` – never the default work of the instance
///   (`round36_lektor`). A named `work` is checked as for every method.
/// - added (purely reading, per work): `work.sheet` (typeset view for the editor) and `search.find` (search in the
///   work, page `/suche`).
pub const LEKTOR_RPC: &[(&str, WorkFrom)] = &[
    // reading, per work
    ("work.get", WorkFrom::Param),
    ("node.get", WorkFrom::Param),
    ("revision.tree", WorkFrom::Param),
    ("revision.amends", WorkFrom::Param),
    ("revision.fold_get", WorkFrom::Param),
    ("ui.render", WorkFrom::Param),
    ("menu.badges", WorkFrom::Param),
    ("post.page", WorkFrom::Param),
    ("post.since", WorkFrom::Param),
    ("post.versions", WorkFrom::Param),
    ("finding.list", WorkFrom::Param),
    ("finding.get", WorkFrom::Param),
    ("finding.matrix", WorkFrom::Param),
    ("thread.list", WorkFrom::Param),
    ("thread.get", WorkFrom::Param),
    ("thread.since", WorkFrom::Param),
    ("thread.post_versions", WorkFrom::Param),
    ("glossary.list", WorkFrom::Param),
    ("glossary.lookup", WorkFrom::Param),
    ("index.entries", WorkFrom::Param),
    ("index.overview", WorkFrom::Param),
    ("work.sheet", WorkFrom::Param),
    ("search.find", WorkFrom::Param),
    // without a work
    ("work.list", WorkFrom::None),
    ("account.me", WorkFrom::None),
    ("account.set_password", WorkFrom::None),
    ("account.logout_all", WorkFrom::None),
    // without work binding (Round 36): if `work` is missing, `AppState::default_work` applies (for Lektors only
    // released works)
    ("wysiwyg.parse", WorkFrom::None),
    ("wysiwyg.serialize", WorkFrom::None),
    ("wysiwyg.diff", WorkFrom::None),
    ("wysiwyg.style", WorkFrom::None),
    ("ribbon.formats", WorkFrom::None),
    // writing, per work
    ("revision.propose", WorkFrom::Param),
    ("post.add", WorkFrom::Param),
    ("post.edit", WorkFrom::Param),
    ("post.delete", WorkFrom::Param),
    ("finding.add", WorkFrom::Param),
];

/// Message when a Lektor calls something outside his tables.
const AUTHOR_ONLY: &str = "Das darf nur der Autor";

/// Message for a Lektor without a released work.
pub const NO_WORK: &str = "Noch kein Werk freigegeben.";

/// Message for a work that is not released: the same as for an unknown one (`StoreError::UnknownWork`).
pub fn unknown_work(work: WorkId) -> String {
    StoreError::UnknownWork(work).to_string()
}

/// Message and code when a Lektor is not allowed the method `method` ([`FORBIDDEN`]).
pub(crate) fn author_only(method: &str) -> RpcError {
    let shown: String = method.chars().take(80).collect();
    RpcError::new(FORBIDDEN, format!("Only the author may do this: {shown}"))
}

/// The works released for `who`, ascending; only a Lektor has any (full rights need no release).
/// Read anew every time.
pub async fn granted_works(state: &AppState, who: &Who) -> Result<Vec<WorkId>, StoreError> {
    match who.lektor() {
        Some(identity) => state.store.member_works(identity.id).await,
        None => Ok(Vec::new()),
    }
}

/// Role of `who` in the work; `None` = none (work invisible, 403).
pub async fn role_in(state: &AppState, who: &Who, work: WorkId) -> Result<Option<Role>, StoreError> {
    if who.full_rights() {
        return Ok(Some(Role::Author));
    }
    Ok(granted_works(state, who).await?.contains(&work).then_some(Role::Lektor))
}

/// The works `who` may see (for overview and work choice): with full rights all, as a Lektor the
/// released ones, otherwise none. System purposes (backup, mirror) keep using `Store::work_list`.
pub async fn visible_works(state: &AppState, who: &Who) -> Result<Vec<WorkSummary>, StoreError> {
    if who.full_rights() {
        return state.store.work_list().await;
    }
    let granted = granted_works(state, who).await?;
    if granted.is_empty() {
        return Ok(Vec::new());
    }
    let mut works = state.store.work_list().await?;
    works.retain(|work| granted.contains(&work.id));
    Ok(works)
}

/// Does `path` match the pattern (as in the router: `{name}` stands for exactly one non-empty segment)? Delivers
/// the value of the placeholder `{id}` (work) if the pattern has one. The raw path is compared: a percent-encoded
/// path matches no fixed segment, and an encoded work id is not a number – both are rejected.
fn match_route<'p>(pattern: &str, path: &'p str) -> Option<Option<&'p str>> {
    let mut id = None;
    let mut wanted = pattern.split('/');
    let mut given = path.split('/');
    loop {
        match (wanted.next(), given.next()) {
            (None, None) => return Some(id),
            (Some(w), Some(g)) if w.starts_with('{') && w.ends_with('}') => {
                if g.is_empty() {
                    return None;
                }
                if w == "{id}" {
                    id = Some(g);
                }
            }
            (Some(w), Some(g)) if w == g => {}
            _ => return None,
        }
    }
}

/// All values of `work` in the address, read the way the pages read them (axum `Query`, that is percent-decoded –
/// an encoded name such as `wor%6B` counts as well). `Err` = the address cannot be read.
fn query_works(query: Option<&str>) -> Result<Vec<String>, ()> {
    let Some(query) = query.filter(|q| !q.is_empty()) else { return Ok(Vec::new()) };
    let uri: axum::http::Uri = format!("/?{query}").parse().map_err(|_| ())?;
    let pairs = axum::extract::Query::<Vec<(String, String)>>::try_from_uri(&uri).map_err(|_| ())?;
    Ok(pairs.0.into_iter().filter(|(name, _)| name == "work").map(|(_, value)| value).collect())
}

/// The only check of a route. Full rights → allowed; Lektor → only matches in [`LEKTOR_ROUTES`] with
/// a released work (see the module header, "Work binding").
pub async fn check_request(state: &AppState, who: &Who, method: &Method, path: &str, query: Option<&str>) -> Verdict {
    check_request_preferring(state, who, method, path, query, None).await
}

/// [`check_request`] with the work the browser opened last (`preferred`, cookie `baruch_work`; this is how
/// [`crate::auth::gate`] calls it): if `work` is missing in the address of a GET page, the redirect leads to this
/// work if it is released – otherwise to the smallest released one (contract §2.1). The cookie never grants a
/// right: a `preferred` that is not released or unreadable simply does not count.
pub async fn check_request_preferring(
    state: &AppState,
    who: &Who,
    method: &Method,
    path: &str,
    query: Option<&str>,
    preferred: Option<WorkId>,
) -> Verdict {
    if who.full_rights() {
        return Verdict::Allow;
    }
    if who.lektor().is_none() {
        return Verdict::Deny("Anmeldung erforderlich.".to_owned());
    }
    // axum answers HEAD with the GET handler; anything other than GET, HEAD and POST does not exist for Lektors.
    let verb = if method == Method::GET || method == Method::HEAD {
        "GET"
    } else if method == Method::POST {
        "POST"
    } else {
        return Verdict::Deny(format!("{AUTHOR_ONLY}."));
    };
    let hit = LEKTOR_ROUTES
        .iter()
        .filter(|(v, _, _)| *v == verb)
        .find_map(|(_, pattern, from)| match_route(pattern, path).map(|id| (id, *from)));
    let Some((id, from)) = hit else {
        return Verdict::Deny(format!("{AUTHOR_ONLY}."));
    };
    let granted = match granted_works(state, who).await {
        Ok(granted) => granted,
        Err(e) => {
            eprintln!("Rollen: Freigaben nicht lesbar: {e}");
            return Verdict::Deny("Freigaben nicht lesbar.".to_owned());
        }
    };
    // Every `work` of the address must be released – for every route, even if it needs no work itself.
    let Ok(in_query) = query_works(query) else {
        return Verdict::Deny("Adresse nicht lesbar.".to_owned());
    };
    for value in &in_query {
        match value.parse::<WorkId>() {
            Ok(work) if granted.contains(&work) => {}
            Ok(work) => return Verdict::Deny(unknown_work(work)),
            Err(_) => return Verdict::Deny("work: work id (number) expected".to_owned()),
        }
    }
    match from {
        WorkFrom::None | WorkFrom::Param => Verdict::Allow,
        WorkFrom::Path => match id.map(str::parse::<WorkId>) {
            Some(Ok(work)) if granted.contains(&work) => Verdict::Allow,
            Some(Ok(work)) => Verdict::Deny(unknown_work(work)),
            _ => Verdict::Deny("Werk-id (Zahl) erwartet".to_owned()),
        },
        WorkFrom::Query if !in_query.is_empty() => Verdict::Allow,
        WorkFrom::Query if verb != "GET" => Verdict::Deny(NO_WORK.to_owned()),
        WorkFrom::Query => match preferred.filter(|work| granted.contains(work)).or(granted.first().copied()) {
            // Never the default work of the instance: the same address with the work opened last if it is
            // released, otherwise with the smallest released one.
            Some(work) => Verdict::Redirect(match query.filter(|q| !q.is_empty()) {
                Some(query) => format!("{path}?{query}&work={work}"),
                None => format!("{path}?work={work}"),
            }),
            None => Verdict::Redirect("/werke".to_owned()),
        },
    }
}

/// `work` from RPC parameters, read the way the modules read it (`rpc::work_param`: number; `findings::rpc`: also
/// digits as text): `Ok(None)` = missing, `Err` = neither a number nor digits.
fn param_work(params: &Value) -> Result<Option<WorkId>, RpcError> {
    let wrong = || RpcError::new(INVALID_PARAMS, "work: Werk-id (Zahl) erwartet");
    match params.get("work") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => n.as_i64().map(Some).ok_or_else(wrong),
        Some(Value::String(s)) => s.trim().parse().map(Some).map_err(|_| wrong()),
        Some(_) => Err(wrong()),
    }
}

/// The only check of an RPC method, before every dispatch (also for unknown names). Full rights → Ok;
/// without a login [`UNAUTHENTICATED`]; Lektor → only methods from [`LEKTOR_RPC`], otherwise [`FORBIDDEN`]. A named
/// `work` must be released ([`FORBIDDEN`] with the message of an unknown work) and afterwards stands as a number
/// in `params`; if it is missing with [`WorkFrom::Param`], the smallest released work is inserted.
pub async fn check_rpc(state: &AppState, who: &Who, method: &str, params: &mut Value) -> Result<(), RpcError> {
    if who.full_rights() {
        return Ok(());
    }
    if who.lektor().is_none() {
        return Err(RpcError::new(UNAUTHENTICATED, "Anmeldung erforderlich."));
    }
    let Some((_, from)) = LEKTOR_RPC.iter().find(|(name, _)| *name == method) else {
        return Err(author_only(method));
    };
    if !(params.is_null() || params.is_object()) {
        return Err(RpcError::new(INVALID_PARAMS, "Parameter als Objekt erwartet"));
    }
    let granted = granted_works(state, who).await?;
    match param_work(params)? {
        Some(work) if granted.contains(&work) => params["work"] = json!(work),
        Some(work) => return Err(RpcError::new(FORBIDDEN, unknown_work(work))),
        None if *from == WorkFrom::None => {}
        None => {
            let Some(work) = granted.first() else {
                return Err(RpcError::new(FORBIDDEN, NO_WORK));
            };
            if params.is_null() {
                *params = json!({});
            }
            params["work"] = json!(work);
        }
    }
    Ok(())
}

/// Name under which `who` writes: a Lektor under his handle (`@lektorat:<slug>`), all others as
/// [`crate::mirror::HUMAN_NAME`].
pub fn author_name(who: &Who) -> String {
    match who.lektor() {
        Some(identity) => identity.handle.clone(),
        None => crate::mirror::HUMAN_NAME.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{AccountKind, Identity};

    fn account(kind: AccountKind) -> Who {
        Who::Account(Identity {
            id: 7,
            name: "Erika".into(),
            display: "Erika Beispiel".into(),
            kind,
            handle: crate::auth::lektor_handle("Erika"),
        })
    }

    #[test]
    fn a_pattern_matches_whole_paths_only_and_yields_the_work_segment() {
        assert_eq!(match_route("/werke", "/werke"), Some(None), "festes Muster ohne Werk");
        assert_eq!(match_route("/werke/{id}/befunde", "/werke/7/befunde"), Some(Some("7")), "Werk aus dem Pfad");
        assert_eq!(
            match_route("/werke/{id}/bilder/{name}/datei", "/werke/7/bilder/rose.png/datei"),
            Some(Some("7")),
            "nur der Platzhalter id ist das Werk, name ein beliebiges Segment"
        );
        assert_eq!(match_route("/static/js/{name}", "/static/js/app.js"), Some(None), "Platzhalter ohne Werk");
        for (pattern, path, why) in [
            ("/werke", "/werke/", "ein Schrägstrich am Ende ist ein anderer Pfad"),
            ("/werke/{id}/befunde", "/werke/7/befunde/act", "ein längerer Pfad passt nicht"),
            ("/werke/{id}/befunde", "/werke//befunde", "ein leeres Segment füllt keinen Platzhalter"),
            ("/werke/{id}/befunde", "/werke/7", "ein kürzerer Pfad passt nicht"),
            ("/", "/act", "die Startseite ist nur „/“"),
            ("/werke", "/%77erke", "kodierte feste Segmente passen nicht"),
        ] {
            assert_eq!(match_route(pattern, path), None, "{why}");
        }
    }

    #[test]
    fn work_in_the_query_is_read_decoded_like_the_pages_read_it() {
        assert_eq!(query_works(None), Ok(vec![]), "ohne Adresse kein Werk");
        assert_eq!(query_works(Some("key=4&work=3")), Ok(vec!["3".to_owned()]), "das schlichte work");
        assert_eq!(
            query_works(Some("work=3&wor%6B=5")),
            Ok(vec!["3".to_owned(), "5".to_owned()]),
            "ein kodierter Name zählt mit – sonst ließe sich ein zweites, fremdes Werk an der Prüfung vorbeireichen"
        );
        assert_eq!(query_works(Some("work=%33")), Ok(vec!["3".to_owned()]), "kodierte Werte werden dekodiert");
        assert_eq!(query_works(Some("works=3&q=work")), Ok(vec![]), "andere Namen sind kein Werk");
    }

    #[test]
    fn the_tables_name_every_entry_once_and_only_the_agreed_writing_ways() {
        for (i, (verb, pattern, _)) in LEKTOR_ROUTES.iter().enumerate() {
            assert!(matches!(*verb, "GET" | "POST"), "{pattern}: nur GET und POST gibt es im Router");
            assert!(
                !LEKTOR_ROUTES[..i].iter().any(|(v, p, _)| v == verb && p == pattern),
                "{verb} {pattern} steht doppelt in LEKTOR_ROUTES"
            );
        }
        let posts: Vec<&str> = LEKTOR_ROUTES.iter().filter(|(v, _, _)| *v == "POST").map(|(_, p, _)| *p).collect();
        assert_eq!(
            posts,
            ["/rpc", "/lektorat/act", "/abmelden", "/konto/passwort", "/einstellungen/darstellung/act"],
            "schreibend nur die Wege durch rpc::call, das eigene Konto und die Darstellung (Cookie)"
        );
        for (i, (method, _)) in LEKTOR_RPC.iter().enumerate() {
            assert!(!LEKTOR_RPC[..i].iter().any(|(m, _)| m == method), "{method} steht doppelt in LEKTOR_RPC");
        }
        let writing: Vec<&str> = LEKTOR_RPC.iter().map(|(m, _)| *m).skip_while(|m| *m != "revision.propose").collect();
        assert_eq!(
            writing,
            ["revision.propose", "post.add", "post.edit", "post.delete", "finding.add"],
            "am Buch schreibt ein Lektor nur Vorschlag, Beitrag und Befund"
        );
    }

    #[test]
    fn a_lektor_writes_under_the_handle_everybody_else_as_the_author() {
        assert_eq!(author_name(&account(AccountKind::Lektor)), "@lektorat:erika", "Herkunft eines Lektors");
        for who in [Who::Open, Who::LocalAuthor, account(AccountKind::Author), Who::Anonymous] {
            assert_eq!(author_name(&who), crate::mirror::HUMAN_NAME, "{who:?} schreibt als Autor wie bisher");
        }
    }
}
