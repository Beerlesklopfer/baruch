// SPDX-License-Identifier: AGPL-3.0-or-later
//! Rollen (Runde 35, Vertrag §S1.4 und §2): die EINZIGE Stelle der Rechteprüfung.
//!
//! Die Kontoart ist global, die Rolle gilt je Werk ([`role_in`]): Ein Autor-Konto ist Autor in jedem Werk, ein
//! Lektor-Konto ist Lektor nur in freigegebenen Werken (`work_member`), sonst hat es keine Rolle (Werk unsichtbar).
//!
//! Für Lektoren ist alles verboten, was nicht ausdrücklich erlaubt ist: [`check_request`] prüft jede Route gegen
//! [`LEKTOR_ROUTES`] (aufgerufen aus [`crate::auth::gate`]), [`check_rpc`] jede RPC-Methode gegen [`LEKTOR_RPC`]
//! (aufgerufen aus [`crate::rpc::call`], also für `/rpc`, `/act` und `/lektorat/act`). Neue Routen und Methoden
//! künftiger Runden sind damit ohne Zutun für Lektoren gesperrt. Formular-Handler der Module (`/beitraege/act`
//! usw.) rufen ihr Modul direkt und gehen nicht durch `rpc::call` – sie stehen darum nicht in [`LEKTOR_ROUTES`].
//!
//! Werk-Bindung: Eine erlaubte Route bzw. Methode gilt nur für freigegebene Werke. Das Werk steht im Pfad (`{id}`),
//! in der Adresse (`?work=`) oder im RPC-Parameter `work`; ein nicht freigegebenes Werk wird mit derselben Meldung
//! abgelehnt wie ein unbekanntes (ein Lektor erfährt nicht, welche Werke es gibt). Fehlt das Werk, gilt nie das
//! Vorgabewerk der Instanz, sondern das kleinste freigegebene: bei RPC eingesetzt, bei GET-Seiten als Umleitung
//! (303) auf dieselbe Adresse mit `work=`. Ein `work` in der Adresse oder in den Parametern wird bei JEDER erlaubten
//! Route und Methode geprüft, auch wenn sie selbst kein Werk braucht.
//!
//! Rollen werden nie über eine Anfrage hinaus gemerkt: Jede Prüfung liest die Freigaben neu
//! (`Store::member_works`, das nur für aktive Lektor-Konten etwas liefert) – Entzug und Sperre wirken bei der
//! nächsten Anfrage.

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

/// Rolle in einem Werk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Author,
    Lektor,
}

/// Urteil über eine Anfrage: erlaubt, Umleitung (303) oder abgelehnt (403 mit Grund).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    Allow,
    Redirect(String),
    Deny(String),
}

/// Woher eine erlaubte Route bzw. Methode ihr Werk nimmt: gar nicht, aus dem Pfad (`{id}`), aus der Adresse
/// (`?work=`) oder aus dem RPC-Parameter `work`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkFrom {
    None,
    Path,
    Query,
    Param,
}

/// Routen, die ein Lektor aufrufen darf: Methode (`GET` | `POST`), Muster wie im Router, Herkunft des Werks
/// (Vertrag §2.1; Begründungen in `docs/Architektur.md`, „Runde 35“). Kein Upload, kein `/act`, kein Formular eines Moduls
/// (`/beitraege/act` usw. rufen ihr Modul direkt, ohne `rpc::call`).
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
    // Ohne Werk (Runde 36, vorher `WorkFrom::Query`): Die Seite nimmt ihr Werk aus `settings::resolve_work`, das für
    // einen Lektor nur freigegebene Werke kennt (`AppState::default_work`, Cookie `baruch_work` nur wenn
    // freigegeben) – so erreicht auch ein Lektor ohne Freigabe seine Darstellung. Ein `work` in der Adresse wird
    // weiter geprüft (siehe `check_request_preferring`).
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

/// RPC-Methoden, die ein Lektor aufrufen darf, mit der Herkunft des Werks (Vertrag §2.2; Begründungen in
/// `docs/Architektur.md`, „Runde 35“). [`WorkFrom::Param`]: fehlt `work`, wird das kleinste freigegebene Werk eingesetzt.
///
/// Abweichungen vom Start des Vertrags, rein lesend:
/// - `wysiwyg.*` und `ribbon.formats` stehen seit Runde 36 wieder „ohne Werk“ ([`WorkFrom::None`]; in Runde 35
///   [`WorkFrom::Param`]): Ohne `work` nimmt `rpc::work_param` [`AppState::default_work`], das einem Lektor nur das
///   kleinste freigegebene Werk gibt und ohne Freigabe `UnknownWork(0)` – nie das Vorgabewerk der Instanz
///   (`round36_lektor`). Ein genanntes `work` wird wie bei jeder Methode geprüft.
/// - ergänzt (rein lesend, je Werk): `work.sheet` (Satzbild für den Editor) und `search.find` (Suche im Werk, Seite
///   `/suche`).
pub const LEKTOR_RPC: &[(&str, WorkFrom)] = &[
    // lesend, je Werk
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
    // ohne Werk
    ("work.list", WorkFrom::None),
    ("account.me", WorkFrom::None),
    ("account.set_password", WorkFrom::None),
    ("account.logout_all", WorkFrom::None),
    // ohne Werk-Bindung (Runde 36): fehlt `work`, gilt `AppState::default_work` (für Lektoren nur Freigegebenes)
    ("wysiwyg.parse", WorkFrom::None),
    ("wysiwyg.serialize", WorkFrom::None),
    ("wysiwyg.diff", WorkFrom::None),
    ("wysiwyg.style", WorkFrom::None),
    ("ribbon.formats", WorkFrom::None),
    // schreibend, je Werk
    ("revision.propose", WorkFrom::Param),
    ("post.add", WorkFrom::Param),
    ("post.edit", WorkFrom::Param),
    ("post.delete", WorkFrom::Param),
    ("finding.add", WorkFrom::Param),
];

/// Meldung, wenn ein Lektor etwas außerhalb seiner Tabellen aufruft.
const AUTHOR_ONLY: &str = "Das darf nur der Autor";

/// Meldung für einen Lektor ohne freigegebenes Werk.
pub const NO_WORK: &str = "Noch kein Werk freigegeben.";

/// Meldung für ein nicht freigegebenes Werk: dieselbe wie für ein unbekanntes (`StoreError::UnknownWork`).
pub fn unknown_work(work: WorkId) -> String {
    StoreError::UnknownWork(work).to_string()
}

/// Meldung und Code, wenn ein Lektor die Methode `method` nicht darf ([`FORBIDDEN`]).
pub(crate) fn author_only(method: &str) -> RpcError {
    let shown: String = method.chars().take(80).collect();
    RpcError::new(FORBIDDEN, format!("{AUTHOR_ONLY}: {shown}"))
}

/// Die für `who` freigegebenen Werke, aufsteigend; nur ein Lektor hat welche (volle Rechte brauchen keine Freigabe).
/// Jedes Mal neu gelesen.
pub async fn granted_works(state: &AppState, who: &Who) -> Result<Vec<WorkId>, StoreError> {
    match who.lektor() {
        Some(identity) => state.store.member_works(identity.id).await,
        None => Ok(Vec::new()),
    }
}

/// Rolle von `who` im Werk; `None` = keine (Werk unsichtbar, 403).
pub async fn role_in(state: &AppState, who: &Who, work: WorkId) -> Result<Option<Role>, StoreError> {
    if who.full_rights() {
        return Ok(Some(Role::Author));
    }
    Ok(granted_works(state, who).await?.contains(&work).then_some(Role::Lektor))
}

/// Die Werke, die `who` sehen darf (für Übersicht und Werkwahl): mit vollen Rechten alle, als Lektor die
/// freigegebenen, sonst keines. Systemzwecke (Sicherung, Spiegel) nehmen weiter `Store::work_list`.
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

/// Passt `path` auf das Muster (wie im Router: `{name}` steht für genau ein nicht leeres Segment)? Liefert den Wert
/// des Platzhalters `{id}` (Werk), falls das Muster einen hat. Verglichen wird der rohe Pfad: Ein prozentkodierter
/// Pfad passt auf kein festes Segment, und eine kodierte Werk-id ist keine Zahl – beides wird abgelehnt.
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

/// Alle Werte von `work` in der Adresse, so gelesen wie die Seiten sie lesen (axum `Query`, also prozentdekodiert –
/// auch ein kodierter Name wie `wor%6B` zählt). `Err` = die Adresse lässt sich nicht lesen.
fn query_works(query: Option<&str>) -> Result<Vec<String>, ()> {
    let Some(query) = query.filter(|q| !q.is_empty()) else { return Ok(Vec::new()) };
    let uri: axum::http::Uri = format!("/?{query}").parse().map_err(|_| ())?;
    let pairs = axum::extract::Query::<Vec<(String, String)>>::try_from_uri(&uri).map_err(|_| ())?;
    Ok(pairs.0.into_iter().filter(|(name, _)| name == "work").map(|(_, value)| value).collect())
}

/// Die einzige Prüfung einer Route. Volle Rechte → erlaubt; Lektor → nur Treffer in [`LEKTOR_ROUTES`] mit
/// freigegebenem Werk (siehe Modulkopf „Werk-Bindung“).
pub async fn check_request(state: &AppState, who: &Who, method: &Method, path: &str, query: Option<&str>) -> Verdict {
    check_request_preferring(state, who, method, path, query, None).await
}

/// [`check_request`] mit dem zuletzt geöffneten Werk des Browsers (`preferred`, Cookie `baruch_work`; so ruft
/// [`crate::auth::gate`] auf): Fehlt `work` in der Adresse einer GET-Seite, führt die Umleitung auf dieses Werk, wenn
/// es freigegeben ist – sonst auf das kleinste freigegebene (Vertrag §2.1). Das Cookie gibt nie ein Recht: Ein nicht
/// freigegebenes oder unlesbares `preferred` zählt schlicht nicht.
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
    // HEAD beantwortet axum mit dem GET-Handler; alles andere als GET, HEAD und POST gibt es für Lektoren nicht.
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
    // Jedes `work` der Adresse muss freigegeben sein – bei jeder Route, auch wenn sie selbst kein Werk braucht.
    let Ok(in_query) = query_works(query) else {
        return Verdict::Deny("Adresse nicht lesbar.".to_owned());
    };
    for value in &in_query {
        match value.parse::<WorkId>() {
            Ok(work) if granted.contains(&work) => {}
            Ok(work) => return Verdict::Deny(unknown_work(work)),
            Err(_) => return Verdict::Deny("work: Werk-id (Zahl) erwartet".to_owned()),
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
            // Nie das Vorgabewerk der Instanz: dieselbe Adresse mit dem zuletzt geöffneten Werk, wenn es freigegeben
            // ist, sonst mit dem kleinsten freigegebenen.
            Some(work) => Verdict::Redirect(match query.filter(|q| !q.is_empty()) {
                Some(query) => format!("{path}?{query}&work={work}"),
                None => format!("{path}?work={work}"),
            }),
            None => Verdict::Redirect("/werke".to_owned()),
        },
    }
}

/// `work` aus RPC-Parametern, gelesen wie die Module es lesen (`rpc::work_param`: Zahl; `findings::rpc`: auch
/// Ziffern als Text): `Ok(None)` = fehlt, `Err` = weder Zahl noch Ziffern.
fn param_work(params: &Value) -> Result<Option<WorkId>, RpcError> {
    let wrong = || RpcError::new(INVALID_PARAMS, "work: Werk-id (Zahl) erwartet");
    match params.get("work") {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => n.as_i64().map(Some).ok_or_else(wrong),
        Some(Value::String(s)) => s.trim().parse().map(Some).map_err(|_| wrong()),
        Some(_) => Err(wrong()),
    }
}

/// Die einzige Prüfung einer RPC-Methode, vor jedem Dispatch (auch für unbekannte Namen). Volle Rechte → Ok;
/// ohne Anmeldung [`UNAUTHENTICATED`]; Lektor → nur Methoden aus [`LEKTOR_RPC`], sonst [`FORBIDDEN`]. Ein genanntes
/// `work` muss freigegeben sein ([`FORBIDDEN`] mit der Meldung eines unbekannten Werks) und steht danach als Zahl
/// in `params`; fehlt es bei [`WorkFrom::Param`], wird das kleinste freigegebene Werk eingesetzt.
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

/// Name, unter dem `who` schreibt: ein Lektor unter seinem Handle (`@lektorat:<slug>`), alle anderen als
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
