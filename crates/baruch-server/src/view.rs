// SPDX-License-Identifier: AGPL-3.0-or-later
//! Ansichtsmodelle für die Vorlagen und das Zeichnen der Seite bzw. ihrer drei Bereiche.
//!
//! Revisionen werden überall über ihre **Nummer** angesprochen (Adresse `rev`, Blättern, Formulare),
//! nicht über ihre Stelle: Nach dem Löschen einer Fassung hat die Nummernfolge Lücken. „‹ n/m ›“
//! zeigt die Stelle n unter m Fassungen, die aktuelle Fassung ist hervorgehoben (`is-current`).
//!
//! Runde 35 (Konten und Rollen, Vertrag §3.3): [`page`] und [`panes`] fragen [`crate::auth::current`]. Ist ein
//! Lektor angemeldet, kommen die drei Bereiche aus eigenen Vorlagen (`fragments/lektor_tree.html`,
//! `lektor_editor.html`, `lektor_assistants.html`): lesen, „Vorschlagen“ (`revision.propose`), Beitrag (`post.add`),
//! eigene Beiträge ändern oder entfernen und Befund (`finding.add`) – alles über `POST /lektorat/act` bzw. `/rpc`.
//! Die Knöpfe des Autors (Speichern, Übernehmen, Verwerfen, Umschalten, Löschen, Ausblenden, Personas, Literatur)
//! stehen dann gar nicht im HTML. Für alle anderen bleibt die Seite Byte für Byte, wie sie war. Beide Funktionen
//! laufen in der Aufgabe der Anfrage (nie abgespalten), sonst wäre die Antwort „ohne Konten“.

use std::collections::HashMap;

use baruch_core::{Author, DiffKind, Node, NodeKind, RevisionStatus, Work, WorkKind, line_diff, revision_fold};
use minijinja::value::Serde;
use minijinja::{Environment, Value, context};
use serde::{Deserialize, Serialize};

use baruch_store::{FALLBACK_COLOR, NodeRevisions, PersonaColor, RevisionInfo, WorkId, WorkSummary};

use crate::rpc::LibraryView;

/// Was gerade gezeigt wird; steht auch in der Adresse (`/?work=1&key=12&rev=3&view=preview`, Runde 8).
/// Kanonisch ist der feste Schlüssel `key` (`Node::key`); `node` (Stelle) gilt nur noch als Rückfall
/// beim Lesen älterer Adressen, erzeugte Links tragen es nicht mehr.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Location {
    /// Werk; fehlt es, das Vorgabewerk.
    pub work: Option<WorkId>,
    /// Fester Schlüssel des Abschnitts; geht vor `node` und bleibt beim Umstellen der Gliederung gleich.
    pub key: Option<i64>,
    /// Stelle (seq) des Abschnitts: nur Rückfall, wenn `key` fehlt oder unbekannt ist.
    pub node: Option<usize>,
    /// Revisionsnummer, ab 1; fehlt sie (oder gibt es sie nicht), die aktuelle.
    pub rev: Option<usize>,
    pub view: Option<String>,
    /// Probeweise gesetzte Fassungen für die Vorschau, `"<node>:<rev>[,<node>:<rev>…]"`. Ungültige
    /// Einträge werden übergangen; die Markierung „aktuell“ ändert sich dadurch nicht.
    pub pick: Option<String>,
    /// Suche der Literatur (Popup der Kopfzeile bzw. Seite `/literatur`; Werk und Bibliothek), `?lq=…`.
    pub lq: Option<String>,
    /// Runde 22: Rückmeldung nach dem Speichern ohne JavaScript (`/gliederung/act`): `amended-<k>` (fortgeschrieben,
    /// Stand k) oder `new-<n>-<grund>` (neue Fassung n, Grund nach `NewReason::id`) bzw. `new-<n>`. Steht nicht in
    /// erzeugten Adressen; Ungültiges wird übergangen.
    pub saved: Option<String>,
    /// 0.9.0 (Nutzer 2026-11-24): `alle` öffnet den Schalter „ältere Fassungen“ – Blättern ‹ › zeigt dann auch die
    /// unter der Marke ausgeblendeten Fassungen ([`baruch_core::revision_fold`]); sonst überspringt es sie.
    pub fassungen: Option<String>,
}

/// Einstellungen der Oberfläche aus Cookies (`baruch_*`, gesetzt von den Widgets). Der Server
/// zeichnet damit gleich richtig: Thema, Spaltenbreiten, offener Abschnitt rechts, zugeklappte Kapitel,
/// Ansicht des Editors.
#[derive(Debug, Clone, Serialize)]
pub struct Settings {
    /// `light` oder `dark`; fehlt es, folgt die Seite dem System.
    pub theme: Option<&'static str>,
    /// Breite der linken bzw. rechten Spalte in Prozent (12–45).
    pub left: Option<f32>,
    pub right: Option<f32>,
    /// Offener Abschnitt des Akkordeons rechts: `personas`, `revisions` (Fassungen) oder `chat` (Vorgabe).
    pub accordion: &'static str,
    /// Zugeklappte Teile und Kapitel.
    pub closed: Vec<usize>,
    /// Zuletzt geöffnetes Werk (Cookie `baruch_work`); `/` ohne `?work=` öffnet es, wenn es noch da ist.
    pub work: Option<WorkId>,
    /// Eingeklappte Vorschläge im Lesefluss als `(key, rev)` (Cookie `baruch_folded` =
    /// '.'-getrennte `<key>-<rev>`, höchstens [`MAX_FOLDED`] Einträge, nur `[0-9.-]`).
    pub folded: Vec<(i64, u32)>,
    /// Ansicht des Editors (Cookie `baruch_editor`, gesetzt nur vom Umschalter): `satz` (Vorgabe) oder
    /// `quelltext`. Der Server zeichnet den Umschalter (`aria-pressed`) und das Satzbild damit gleich
    /// richtig, statt dass das Widget nach dem Laden erst umschaltet (Runde 8, 2d).
    pub editor: &'static str,
    /// Vorschläge im Lesefluss anfangs (Cookie `baruch_pv`, Einstellungen „Darstellung“, Runde 9):
    /// `open` (Vorgabe) oder `closed` – dann zeichnet der Server alle eingeklappt außer dem gezeigten.
    /// Bei `closed` trägt [`Settings::folded`] zusätzlich den Merker [`FOLD_ALL`].
    pub pv: &'static str,
}

/// Merker in [`Settings::folded`] für `baruch_pv=closed`: „alle Vorschläge eingeklappt“. Schlüssel 0
/// gibt es nicht (`parse_folded` nimmt nur Schlüssel und Nummern ab 1), der Merker trifft also nie
/// einen echten Vorschlag; so wirkt die Einstellung über denselben Weg wie das Cookie `baruch_folded`.
pub const FOLD_ALL: (i64, u32) = (0, 0);

impl Default for Settings {
    fn default() -> Self {
        Settings {
            theme: None,
            left: None,
            right: None,
            accordion: "chat",
            closed: Vec::new(),
            work: None,
            folded: Vec::new(),
            editor: "satz",
            pv: "open",
        }
    }
}

/// Höchstzahl der Einträge im Cookie `baruch_folded`; weitere werden übergangen.
pub const MAX_FOLDED: usize = 200;

/// Wert von `baruch_folded` → `(key, rev)`; enthält er etwas anderes als `[0-9.-]`, gilt er nicht.
fn parse_folded(value: &str) -> Vec<(i64, u32)> {
    if !value.bytes().all(|b| b.is_ascii_digit() || b == b'.' || b == b'-') {
        return Vec::new();
    }
    let mut out: Vec<(i64, u32)> = Vec::new();
    for entry in value.split('.').filter(|e| !e.is_empty()).take(MAX_FOLDED) {
        let Some((key, rev)) = entry.split_once('-') else { continue };
        let (Ok(key), Ok(rev)) = (key.parse::<i64>(), rev.parse::<u32>()) else { continue };
        if key > 0 && rev > 0 && !out.contains(&(key, rev)) {
            out.push((key, rev));
        }
    }
    out
}

impl Settings {
    /// Aus dem `Cookie`-Kopf lesen; Unbekanntes und Ungültiges wird übergangen.
    pub fn from_cookie_header(header: Option<&str>) -> Self {
        let mut settings = Settings::default();
        let width = |v: &str| v.trim_end_matches('%').parse::<f32>().ok().filter(|w| (12.0..=45.0).contains(w));
        for pair in header.unwrap_or("").split(';') {
            let Some((key, value)) = pair.trim().split_once('=') else { continue };
            match key {
                "baruch_theme" => {
                    settings.theme = match value {
                        "light" => Some("light"),
                        "dark" => Some("dark"),
                        _ => None,
                    }
                }
                "baruch_left" => settings.left = width(value),
                "baruch_right" => settings.right = width(value),
                "baruch_accordion" if value == "personas" => settings.accordion = "personas",
                "baruch_accordion" if value == "revisions" => settings.accordion = "revisions",
                // Runde 28: Akkordeon „Befunde“ (fragments/findings_panel.html).
                "baruch_accordion" if value == "befunde" => settings.accordion = "befunde",
                "baruch_closed" => settings.closed = value.split('.').filter_map(|id| id.parse().ok()).collect(),
                "baruch_work" => settings.work = value.parse().ok().filter(|&id: &WorkId| id > 0),
                "baruch_folded" => settings.folded = parse_folded(value),
                "baruch_editor" if value == "quelltext" => settings.editor = "quelltext",
                "baruch_pv" if value == "closed" => settings.pv = "closed",
                _ => {}
            }
        }
        if settings.pv == "closed" {
            settings.folded.push(FOLD_ALL);
        }
        settings
    }

    /// Inline-Stil für `<html>` mit den Spaltenbreiten.
    fn style(&self) -> String {
        let mut style = String::new();
        if let Some(left) = self.left {
            style.push_str(&format!("--left:{left:.1}%;"));
        }
        if let Some(right) = self.right {
            style.push_str(&format!("--right:{right:.1}%;"));
        }
        style
    }
}

/// Aufgelöste Stelle: gültiger Abschnitt, Stelle (`index`, ab 0) und Nummer (`rev`) der gezeigten
/// Revision, Ansicht und die gültigen probeweise gesetzten Fassungen (`pick`).
#[derive(Debug, Clone)]
pub struct Resolved {
    pub node: usize,
    /// Fester Schlüssel des gezeigten Abschnitts (`Node::key`); steht in jeder erzeugten Adresse.
    pub key: i64,
    pub index: usize,
    pub rev: u32,
    pub preview: bool,
    pub pick: Vec<(usize, u32)>,
    /// Die Adresse nannte einen Schlüssel, den es im Werk nicht (mehr) gibt; gezeigt wird dann der
    /// Rückfall (`node` bzw. der Vorgabe-Abschnitt), der Editor-Kopf nennt den Schlüssel.
    pub missing_key: Option<i64>,
    /// Runde 22: Rückmeldung nach dem Speichern ([`Location::saved`], geprüft in [`saved_notice`]).
    pub saved: Option<String>,
    /// 0.9.0: Schalter „ältere Fassungen“ offen (`?fassungen=alle`).
    pub show_all: bool,
}

impl Location {
    /// Abschnitt bestimmen (Vertrag Runde 8, §1): `key` gefunden → dieser Abschnitt (geht vor `node`);
    /// `key` unbekannt → `node`, sonst der Vorgabe-Abschnitt, mit `missing_key`; ohne `key` wie bisher
    /// `node` bzw. die Vorgabe.
    pub fn resolve(&self, work: &Work) -> Resolved {
        let by_key = self.key.and_then(|key| work.nodes.iter().find(|n| n.key == key)).map(|n| n.id);
        let missing_key = self.key.filter(|_| by_key.is_none());
        let fallback = || self.node.filter(|&id| id < work.nodes.len()).unwrap_or(default_node(work));
        let node = by_key.unwrap_or_else(fallback);
        let n = &work.nodes[node];
        let index = self
            .rev
            .and_then(|rev| u32::try_from(rev).ok())
            .and_then(|rev| n.position_of(rev))
            .or_else(|| n.current_index())
            .unwrap_or(0);
        let rev = n.revisions.get(index).map_or(0, |r| r.number);
        let pick = parse_pick(work, self.pick.as_deref());
        let preview = self.view.as_deref() == Some("preview");
        let show_all = self.fassungen.as_deref() == Some("alle");
        Resolved { node, key: n.key, index, rev, preview, pick, missing_key, saved: self.saved.clone(), show_all }
    }
}

/// Kanonische Adresse eines Abschnitts (Vertrag Runde 8, §1):
/// `/?work=W&key=K[&rev=R][&view=preview][&pick=…][&lq=…]`, in dieser Reihenfolge. Nur Ziffern, feste
/// Namen und prozentkodierte Werte; `pick` und `lq` kommen fertig als `&pick=…` bzw. `&lq=…` (oder leer).
pub fn node_url(work: WorkId, key: i64, rev: Option<u32>, preview: bool, pick_q: &str, lq_q: &str) -> String {
    let mut url = format!("/?work={work}&key={key}");
    if let Some(rev) = rev {
        url.push_str(&format!("&rev={rev}"));
    }
    if preview {
        url.push_str("&view=preview");
    }
    url.push_str(pick_q);
    url.push_str(lq_q);
    url
}

/// `"<node>:<rev>,…"` → gültige Paare (Abschnitt und Fassung gibt es); je Abschnitt zählt der letzte
/// Eintrag, Ungültiges wird übergangen. Ergebnis nach Abschnitt sortiert.
pub fn parse_pick(work: &Work, pick: Option<&str>) -> Vec<(usize, u32)> {
    let mut chosen: Vec<(usize, u32)> = Vec::new();
    for entry in pick.unwrap_or("").split(',') {
        let Some((node, rev)) = entry.trim().split_once(':') else { continue };
        let (Ok(node), Ok(rev)) = (node.trim().parse::<usize>(), rev.trim().parse::<u32>()) else { continue };
        if work.nodes.get(node).and_then(|n| n.revision(rev)).is_none() {
            continue;
        }
        chosen.retain(|&(n, _)| n != node);
        chosen.push((node, rev));
    }
    chosen.sort_unstable();
    chosen
}

/// `&pick=2:3,4:1` für eine nicht leere Auswahl, sonst leer (nur Ziffern, `:` und `,`).
pub fn pick_param(pick: &[(usize, u32)]) -> String {
    if pick.is_empty() {
        return String::new();
    }
    let list: Vec<String> = pick.iter().map(|(node, rev)| format!("{node}:{rev}")).collect();
    format!("&pick={}", list.join(","))
}

/// Erster Abschnitt mit offenem Vorschlag, sonst der erste überhaupt.
fn default_node(work: &Work) -> usize {
    work.nodes.iter().find(|n| n.open_proposals() > 0).map_or(0, |n| n.id)
}

impl Resolved {
    /// Adresse der Stelle: `/?work=<id>&key=…&rev=…[&view=preview][&pick=…][&lq=…]` ([`node_url`]).
    pub fn url(&self, work: WorkId, lq: Option<&str>) -> String {
        let url = node_url(work, self.key, Some(self.rev), self.preview, &pick_param(&self.pick), &lq_param(lq));
        if self.show_all { format!("{url}{FOLD_OPEN_Q}") } else { url }
    }
}

/// 0.9.0: Zusatz der Adresse für den offenen Schalter „ältere Fassungen“ ([`Location::fassungen`]).
pub const FOLD_OPEN_Q: &str = "&fassungen=alle";

/// `&lq=<kodiert>` für eine nicht leere Suche, sonst leer.
pub fn lq_param(lq: Option<&str>) -> String {
    match lq.map(str::trim).filter(|q| !q.is_empty()) {
        Some(q) => format!("&lq={}", query_escape(q)),
        None => String::new(),
    }
}

/// Prozentkodierung für einen Wert in der Adresse: alles außer `A–Z a–z 0–9 - . _ ~` als `%XX`.
pub fn query_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// Das gezeigte Werk (id, Ordnername) und alle Werke für die Werkwahl („Titel · Autor · n Abschnitte“).
#[derive(Debug, Clone)]
pub struct Shelf {
    pub id: WorkId,
    pub slug: String,
    pub works: Vec<WorkSummary>,
}

#[derive(Serialize)]
struct TreeItem {
    id: usize,
    /// Fester Schlüssel (`Node::key`): die Formulare schicken ihn mit, die Stelle `id` kann veralten.
    key: i64,
    number: String,
    title: String,
    part: bool,
    section: bool,
    proposals: usize,
    /// Runde 28: offene Befunde samt Unterpunkten (Zählregel §1.5, [`crate::findings::view::subtree_open`]);
    /// gesetzt von [`attach_findings`], vorher 0.
    findings: usize,
    /// Davon am Punkt selbst.
    findings_own: usize,
    selected: bool,
    /// Runde 22 (R22a): weitere nicht aktuelle Fassungen außer offenen Vorschlägen (ersetzt, verworfen).
    alternatives: usize,
    /// Aufgeklappt: nicht zugeklappt, oder der gewählte Abschnitt liegt darin.
    open: bool,
    /// Ebene in der Gliederung (Teil 0, Kapitel 1, Abschnitt 2, Unterabschnitt 3) und ihr Name.
    level: u8,
    level_name: &'static str,
    /// Nachbarn auf derselben Ebene unter demselben Elternteil (Ziele für ↑ und ↓).
    up: Option<usize>,
    down: Option<usize>,
    up_key: Option<i64>,
    down_key: Option<i64>,
    /// ← (höher stufen) bzw. → (tiefer stufen) möglich; Teile nie.
    promote: bool,
    demote: bool,
    children: Vec<TreeItem>,
    /// Runde 16b: Sprechdauer (bei Punkten mit Unterpunkten samt Nachfahren) als „m:ss“ und Zeitmarke;
    /// `None`, wenn das Werk keine Zeiten zeigt ([`crate::timing::WorkTiming::shown`]).
    duration: Option<String>,
    start: Option<String>,
}

/// Ebene je Knoten aus Art und Elternteil: Teil 0, Kapitel 1, Abschnitt 2, Abschnitt unter Abschnitt 3.
/// Knoten stehen in Baumreihenfolge, das Elternteil also vorher.
pub fn levels(work: &Work) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::with_capacity(work.nodes.len());
    for node in &work.nodes {
        let parent = node.parent.filter(|&p| p < out.len()).map(|p| out[p]);
        out.push(match node.kind {
            NodeKind::Part => 0,
            NodeKind::Chapter => 1,
            NodeKind::Section => parent.map_or(2, |p| (p + 1).clamp(2, 3)),
        });
    }
    out
}

/// Name einer Ebene; bei Predigten heißt Ebene 1 „Predigtteil“.
pub fn level_name(kind: WorkKind, level: u8) -> &'static str {
    match (level, kind) {
        (0, _) => "Teil",
        (1, WorkKind::Sermon) => "Predigtteil",
        (1, WorkKind::Book) => "Kapitel",
        (2, _) => "Abschnitt",
        _ => "Unterabschnitt",
    }
}

/// Voriger und nächster Knoten derselben Ebene unter demselben Elternteil.
pub fn neighbours(work: &Work, levels: &[u8], node: usize) -> (Option<usize>, Option<usize>) {
    let n = &work.nodes[node];
    let same: Vec<usize> = work.children(n.parent).filter(|s| levels[s.id] == levels[node]).map(|s| s.id).collect();
    let at = same.iter().position(|&s| s == node);
    let up = at.and_then(|i| i.checked_sub(1)).map(|i| same[i]);
    let down = at.and_then(|i| same.get(i + 1).copied());
    (up, down)
}

fn tree(work: &Work, parent: Option<usize>, selected: usize, closed: &[usize]) -> Vec<TreeItem> {
    tree_with(work, &levels(work), parent, selected, closed)
}

fn tree_with(work: &Work, levels: &[u8], parent: Option<usize>, selected: usize, closed: &[usize]) -> Vec<TreeItem> {
    work.children(parent)
        .map(|n| {
            let children = tree_with(work, levels, Some(n.id), selected, closed);
            let holds_selected = children.iter().any(|c| c.selected || c.holds_selected());
            let level = levels[n.id];
            let (up, down) = neighbours(work, levels, n.id);
            TreeItem {
                id: n.id,
                key: n.key,
                up_key: up.map(|u| work.nodes[u].key),
                down_key: down.map(|d| work.nodes[d].key),
                number: n.number.clone(),
                title: n.title.clone(),
                part: n.kind == NodeKind::Part,
                section: n.kind == NodeKind::Section,
                proposals: n.open_proposals(),
                findings: 0,
                findings_own: 0,
                selected: n.id == selected,
                alternatives: alternatives_of(n).len(),
                open: !closed.contains(&n.id) || holds_selected,
                level,
                level_name: level_name(work.kind, level),
                up,
                down,
                promote: level >= 2,
                demote: (1..=2).contains(&level) && n.id > 0,
                children,
                duration: None,
                start: None,
            }
        })
        .collect()
}

impl TreeItem {
    fn holds_selected(&self) -> bool {
        self.children.iter().any(|c| c.selected || c.holds_selected())
    }
}

// ---- Runde 16b: Zeitwächter und Freigaben der Personas -------------------------------------------------

/// Freigabe einer Persona aus `persona.list` (Runde 15b): Verbindung lokal/extern und ob ein Auftrag hinaus darf.
#[derive(Debug, Clone, Default, Serialize)]
pub struct PersonaAccess {
    pub handle: String,
    /// „lokal“ oder „extern“.
    pub locality: String,
    pub external_allowed: bool,
    /// Name der Verbindung (leer, wenn keine).
    pub connection: String,
    /// Wirksame Kontextlänge in Tokens ohne Netzabfrage (`persona.list` → `context.tokens`, Integrator Runde 17);
    /// `None` = nicht bekannt, dann zeigt die Seite die Einstellung der Persona.
    pub context_tokens: Option<u32>,
}

impl PersonaAccess {
    /// Extern ohne Freigabe: Aufträge gingen nicht hinaus, die Persona ist auf der Arbeitsseite gesperrt.
    pub fn blocked(&self) -> bool {
        self.locality == "extern" && !self.external_allowed
    }
}

/// Was die Arbeitsseite über [`LibraryView`] hinaus braucht (Runde 16b): Zeiten des Werks und Freigaben der
/// Personas. Beides nur lesend.
#[derive(Debug, Clone, Default)]
pub struct ViewExtras {
    pub timing: Option<crate::timing::WorkTiming>,
    pub personas: Vec<PersonaAccess>,
    /// Lektor nach der Einstellung mit der wirksamen Kontextlänge ohne Netzabfrage (`persona.list` → `lector`,
    /// Integrator Runde 17); `None` = keine Liste geladen, dann gilt `Work::lector`.
    pub lector: Option<String>,
    /// Knöpfe des Menübands nach der Werkvorlage (Runde 18a, [`crate::ribbon::ribbon`]).
    pub ribbon: crate::ribbon::Ribbon,
    /// CSS des Satzbilds nach Vorlage und Einstellungen des Werks (Runde 18c, [`crate::sheet::sheet_css_for`]);
    /// `None`, wenn es sich nicht berechnen ließ (dann holt das Widget es per RPC).
    pub sheet_css: Option<String>,
    /// Runde 28: offene Befunde je Abschnitt und Befunde für das Akkordeon ([`crate::findings::view::load`]).
    pub findings: crate::findings::view::FindingsView,
    /// Runde 28: bearbeitete Beiträge des Werks, `post.id` → Zeit der letzten Änderung ([`crate::posts::edited_map`]).
    pub post_edits: HashMap<i64, String>,
    /// Runde 30: Metadaten der Beiträge des Werks (nur nicht leere, [`baruch_store::Store::work_post_meta`]).
    pub post_meta: HashMap<i64, baruch_store::PostMeta>,
    /// Runde 19c: Kasten „Bibeln“ der Literatur für die Arbeitsseite ohne JavaScript
    /// ([`crate::literature::bibles::workspace_shelf`]); `None` = nicht geladen (dann kein Kasten).
    pub bibles: Option<crate::literature::bibles::Shelf>,
    /// 0.9.0: Marken „ältere Fassungen ausblenden“ des Werks, `Node::key` → Nummer
    /// ([`baruch_store::Store::work_revision_folds`]).
    pub revision_folds: HashMap<i64, u32>,
}

/// Zeiten ([`crate::timing::work_timing`]) und Freigaben der Personas (nur über `persona.list`) laden.
pub async fn extras(state: &crate::AppState, work: WorkId) -> Result<ViewExtras, crate::rpc::RpcError> {
    let timing = Some(crate::timing::work_timing(state, work).await?);
    let list = crate::personas::call(state, "persona.list", serde_json::json!({})).await?;
    let personas = list["personas"]
        .as_array()
        .map(|rows| {
            rows.iter()
                .map(|p| PersonaAccess {
                    handle: p["handle"].as_str().unwrap_or_default().to_owned(),
                    locality: p["locality"].as_str().unwrap_or("lokal").to_owned(),
                    external_allowed: p["external_allowed"].as_bool().unwrap_or(false),
                    connection: p["connection"]["name"].as_str().unwrap_or_default().to_owned(),
                    context_tokens: p["context"]["tokens"].as_u64().and_then(|t| u32::try_from(t).ok()),
                })
                .collect()
        })
        .unwrap_or_default();
    let lector = list["lector"].as_str().map(str::to_owned);
    let ribbon = crate::ribbon::ribbon(state, work).await?;
    let sheet_css = crate::sheet::sheet_css_for(state, work).await.ok();
    let findings = crate::findings::view::load(state, work).await.unwrap_or_default();
    let post_edits = crate::posts::edited_map(state, work).await.unwrap_or_default();
    let post_meta = state.store.work_post_meta(work).await.unwrap_or_default();
    let bibles = crate::literature::bibles::workspace_shelf(state, work).await;
    let revision_folds = state.store.work_revision_folds(work).await.unwrap_or_default();
    Ok(ViewExtras {
        timing,
        personas,
        lector,
        ribbon,
        sheet_css,
        findings,
        post_edits,
        post_meta,
        bibles,
        revision_folds,
    })
}

/// Offene Befunde (samt Unterpunkten bzw. am Punkt selbst) an die Einträge der Gliederung hängen (Runde 28).
fn attach_findings(items: &mut [TreeItem], sums: &[usize], own: &HashMap<i64, usize>) {
    for item in items {
        item.findings = sums.get(item.id).copied().unwrap_or(0);
        item.findings_own = own.get(&item.key).copied().unwrap_or(0);
        attach_findings(&mut item.children, sums, own);
    }
}

/// Dauer und Zeitmarke an die Einträge der Gliederung hängen (nur, wenn das Werk Zeiten zeigt). Punkte mit
/// Unterpunkten zeigen die Summe samt Nachfahren, Blätter ihren eigenen Text.
fn attach_timing(items: &mut [TreeItem], timing: &crate::timing::WorkTiming) {
    use baruch_typst::timing::clock;
    for item in items {
        if let Some(section) = timing.sections.get(item.id) {
            let seconds = if item.children.is_empty() { section.seconds } else { section.subtree_seconds };
            item.duration = Some(clock(seconds));
            item.start = Some(clock(section.start_seconds));
        }
        attach_timing(&mut item.children, timing);
    }
}

/// Kontext `timing` für Baum und Editorkopf: `None`, wenn das Werk keine Zeiten zeigt (Buch ohne Zielzeit).
fn timing_context(timing: Option<&crate::timing::WorkTiming>, node: &Node, shown_body: &str) -> Option<Value> {
    use baruch_typst::timing::{clock, speaking_seconds, spoken_words};
    let timing = timing.filter(|t| t.shown())?;
    let current = timing.sections.get(node.id).map_or(0, |s| s.seconds);
    let words = spoken_words(shown_body);
    let section = speaking_seconds(words, timing.words_per_minute);
    Some(context! {
        total => timing.seconds, total_clock => clock(timing.seconds),
        target => timing.target_seconds, target_clock => timing.target_seconds.map(clock),
        over => timing.over_seconds.is_some(), over_clock => timing.over_seconds.map(clock),
        words_per_minute => timing.words_per_minute, sermon => timing.kind == WorkKind::Sermon,
        // Gezeigte Fassung des Abschnitts (im Editor das, was im Textfeld steht) und die Summe ohne die
        // aktuelle Fassung dieses Abschnitts (für die Live-Rechnung des Widgets).
        section => section, section_clock => clock(section), section_words => words,
        rest => timing.seconds.saturating_sub(current),
        start_clock => timing.sections.get(node.id).map(|s| clock(s.start_seconds)),
    })
}

#[derive(Serialize)]
struct DiffLine {
    kind: &'static str,
    text: String,
}

#[derive(Serialize)]
struct PreviewLine {
    level: u8,
    text: String,
}

fn status_label(status: RevisionStatus) -> &'static str {
    match status {
        RevisionStatus::Current => "aktuell",
        RevisionStatus::Proposed => "Vorschlag",
        RevisionStatus::Rejected => "verworfen",
        RevisionStatus::Superseded => "ersetzt",
    }
}

fn author_label(author: &Author) -> String {
    match author {
        Author::Human { name } => name.clone(),
        // Runde 35: Ein Lektor schreibt ohne Modell – gezeigt wird nur die Herkunft, kein leeres „()“.
        Author::Assistant { persona, model } if model.is_empty() && is_lektorat(persona) => persona.clone(),
        Author::Assistant { persona, model } => format!("{persona} ({model})"),
    }
}

/// Herkunft eines Lektor-Kontos (`@lektorat:<slug>`, [`crate::auth::lektor_handle`]) – ein Mensch, keine Persona.
fn is_lektorat(handle: &str) -> bool {
    handle.starts_with("@lektorat:")
}

/// Kapitel (bzw. Teil) eines Knotens: der Knoten selbst, wenn er kein Abschnitt ist, sonst der
/// nächste Vorfahr, der kein Abschnitt ist (Abschnitte ohne solchen Vorfahr stehen für sich).
fn chapter_of(work: &Work, node: usize) -> usize {
    let mut at = node;
    let mut steps = 0;
    while let Some(n) = work.nodes.get(at) {
        match (n.kind, n.parent) {
            (NodeKind::Section, Some(parent)) if steps < work.nodes.len() => {
                at = parent;
                steps += 1;
            }
            _ => break,
        }
    }
    at
}

/// Das Kapitel und alle Knoten darunter in Baumreihenfolge.
fn chapter_nodes(work: &Work, node: usize) -> Vec<&Node> {
    fn walk<'a>(work: &'a Work, id: usize, out: &mut Vec<&'a Node>, depth: usize) {
        let Some(n) = work.nodes.get(id) else { return };
        if depth > work.nodes.len() || out.iter().any(|o| o.id == id) {
            return; // Schleife in parent
        }
        out.push(n);
        for child in work.children(Some(id)) {
            walk(work, child.id, out, depth + 1);
        }
    }
    let mut out = Vec::new();
    walk(work, chapter_of(work, node), &mut out, 0);
    out
}

/// Ein Abschnitt der Vorschau: seine Blöcke (Zeilen) mit den offenen Vorschlägen an ihrer Stelle.
#[derive(Serialize)]
struct PreviewSection {
    node: usize,
    flow: InlineView,
}

/// Platzhalter für die Typst-Vorschau: Texte des Kapitels, Überschriften nach `=`. Je Abschnitt die
/// aktuelle Fassung, oder die in `pick` probeweise gewählte (die Markierung bleibt unberührt); die
/// offenen Vorschläge jedes Abschnitts stehen an ihrer Stelle zwischen seinen Blöcken (Runde 8, nur
/// Zeilen-Diff), Basis ist die gezeigte Fassung des Abschnitts.
fn preview(work: &Work, node: &Node, pick: &[(usize, u32)], ctx: &ProposalContext<'_>) -> Vec<PreviewSection> {
    let mut sections = Vec::new();
    for n in chapter_nodes(work, node.id) {
        let picked = pick.iter().find(|(id, _)| *id == n.id).and_then(|&(_, rev)| n.revision(rev));
        let Some(revision) = picked.or_else(|| n.current_index().map(|i| &n.revisions[i])) else { continue };
        let proposals = proposals_of(n, revision, ctx, false);
        sections.push(PreviewSection { node: n.id, flow: inline_view(n, revision, proposals, true, None) });
    }
    sections
}

/// Zeilen eines Texts für die Vorschau: Überschriften `=`, `==`, `===`, sonst Text; leere entfallen.
fn preview_lines(body: &str) -> Vec<PreviewLine> {
    let mut lines = Vec::new();
    for line in body.lines() {
        // Überschriften `=`, `==`, `===`, sonst Text (Platzhalter bis zum Typst-Satz).
        let equals = line.bytes().take_while(|&b| b == b'=').count();
        let (level, text) = match line.get(equals..) {
            Some(rest) if (1..=3).contains(&equals) && rest.starts_with(' ') => (equals as u8, rest.trim().to_owned()),
            _ => (0, line.to_owned()),
        };
        if !text.trim().is_empty() {
            lines.push(PreviewLine { level, text });
        }
    }
    lines
}

/// Was die Vorschläge im Lesefluss brauchen: Werk (für Bildadressen), Fassungsbaum (Basis),
/// Persona-Farben (Reihenfolge, Name), Alternativtexte der Bilder und eingeklappte Vorschläge. Ist die
/// gezeigte Fassung ein Vorschlag, steht er offen und hervorgehoben ([`proposals_of`]).
struct ProposalContext<'a> {
    work: WorkId,
    revisions: &'a [NodeRevisions],
    colors: &'a [PersonaColor],
    image_alts: &'a HashMap<String, String>,
    folded: &'a [(i64, u32)],
}

impl ProposalContext<'_> {
    /// Bild zu einem Pfad wie im Satzbild (`wysiwyg.parse`): Adresse der Bilderseite, Alternativtext
    /// nach Normalform.
    fn image(&self, path: &str) -> Option<baruch_wysiwyg::ImageRef> {
        Some(crate::wysiwyg::image_ref_in(self.work, path, self.image_alts))
    }
}

/// Stand von `counter(heading).get().first()` im Kapitel eines Knotens: die Zahl der Knoten bis zu
/// diesem Kapitel (Baumreihenfolge), deren Überschrift Typst zählt (`counted`, aus
/// [`baruch_store::Store::counted_headings`]). Typst zählt nur nummerierte Überschriften: Ein Kapitel mit
/// „Nummer aus“ (z. B. Vorwort) zählt nicht mit; in ihm gilt der Stand davor (vor dem ersten: 0).
/// `Some(None)` = Stand nicht sicher bekannt; `None` für Teile und Abschnitte ohne Kapitel.
fn chapter_number(work: &Work, node: usize, counted: Option<&[i64]>) -> Option<Option<usize>> {
    let chapter = chapter_of(work, node);
    if work.nodes.get(chapter)?.kind != NodeKind::Chapter {
        return None;
    }
    Some(counted.map(|keys| work.nodes.iter().filter(|n| n.id <= chapter && keys.contains(&n.key)).count()))
}

/// Kapitelbild für den Editorkopf (Runde 11): sicher aus der Vorlage ermittelt (Bild, Unterschrift,
/// Alternativtext; `missing`, wenn der Pfad zu keinem Bild des Werks gehört), sonst nur der Hinweis
/// „Bild aus der Vorlage“; ohne berechnete Bilder in der Vorlage nichts.
fn chapter_image(work: &Work, node: usize, work_id: WorkId, images: &crate::wysiwyg::ImageView) -> Option<Value> {
    use crate::wysiwyg::ChapterImage;
    let number = chapter_number(work, node, images.counted.as_deref())?;
    Some(match images.chapter_image(number)? {
        ChapterImage::Known { path, asset } => {
            // Bildnamen bestehen nur aus `[a-z0-9._-]` (valid_asset_name): die Adressen sind sicher.
            let url = |tail: &str| {
                asset.as_ref().map(|a| Value::from_safe_string(format!("/werke/{work_id}/bilder/{}{tail}", a.name)))
            };
            context! {
                known => true, chapter => number, path => crate::assets::path_text(&path),
                missing => asset.is_none(), name => asset.as_ref().map(|a| a.name.clone()),
                caption => asset.as_ref().map(|a| a.caption.clone()), alt => asset.as_ref().map(|a| a.alt.clone()),
                thumb => url("/vorschau?b=320"), detail => url(""),
            }
        }
        ChapterImage::FromTemplate { expr } => {
            context! { known => false, chapter => number, expr => crate::assets::path_text(&expr) }
        }
    })
}

/// Bildpfade der Vorschau, zu denen es im Werk kein Bild gibt (Runde 11: deutlich melden), ohne
/// Doppelte in Reihenfolge; dazu der Pfad des Kapitelbilds, wenn er fehlt.
fn missing_images(bodies: &[&str], images: &crate::wysiwyg::ImageView) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for body in bodies {
        for path in baruch_typst::images::image_paths(body) {
            if images.asset_by_path(&path).is_none() && !out.contains(&path) {
                out.push(path);
            }
        }
    }
    out
}

/// Ein offener Vorschlag (Formulare „Übernehmen“/„Verwerfen“ einmal je Vorschlag) mit seinen
/// Stücken an ihrer Stelle.
#[derive(Serialize)]
struct ProposalView {
    /// `<key>-<rev>`: Eintrag im Cookie `baruch_folded`, `data-pv` und Teil der Formular-ids.
    id: String,
    key: i64,
    node: usize,
    rev: u32,
    #[serde(skip)]
    hunks: Vec<HunkView>,
}

/// Ein Stück eines offenen Vorschlags an seiner Stelle (`fragments/proposals.html`, Runde 8): je
/// Stück ein `<details class="pv pc">` beim Block `at` der gezeigten Fassung.
#[derive(Serialize)]
struct HunkView {
    #[serde(flatten)]
    meta: HunkMeta,
    /// Index des ersten betroffenen Blocks der gezeigten Fassung (`data-b`), bei eingefügten der
    /// Block, vor dem eingefügt wird.
    at: usize,
    /// Lücke, in der das Stück steht: vor Block `gap` (= hinter Block `gap - 1`; am Ende die Blockzahl).
    gap: usize,
    kind: &'static str,
    /// Satzbild des Stücks (`baruch_wysiwyg::hunk_html_with`, nur im Editor).
    satz: Option<String>,
    lines: Vec<DiffLine>,
}

/// Was alle Stücke eines Vorschlags teilen.
#[derive(Serialize, Clone)]
struct HunkMeta {
    id: String,
    handle: String,
    /// Name der Persona; ohne Persona-Eintrag der Handle.
    name: String,
    note: String,
    /// Gibt es zum Autor einen Persona-Eintrag? (sonst `FALLBACK_COLOR`, am Ende der Liste)
    known: bool,
    /// CSS-Variablen `--pc-light`, `--pc-dark`, `--pc-line`, `--pc-line-dark` (nur geprüfte `#rrggbb`).
    style: String,
    /// Der einzige Kopf: „Name (@handle) · Fassung n[ · Modell][ · auf Fassung m][ · Teil i/k]“.
    head: String,
    /// Nur beim gezeigten Vorschlag: „Unterschied zu Fassung b[ (aktuell)] · gezeigt“ (seine Stücke
    /// stehen dann an ihrer Stelle in ihm selbst, verglichen mit seiner Basis); oder der Hinweis,
    /// dass er der gezeigten Fassung gleicht.
    base_note: Option<String>,
    open: bool,
    shown: bool,
}

/// Eintrag im Lesefluss: ein Block der gezeigten Fassung (`b`, `raw`, in der Vorschau `lines`) oder
/// die Stücke, die an dieser Stelle stehen.
#[derive(Serialize)]
struct FlowEntry {
    b: Option<usize>,
    raw: String,
    lines: Vec<PreviewLine>,
    hunks: Vec<HunkView>,
}

/// Lesefluss eines Abschnitts mit den offenen Vorschlägen an ihrer Stelle (Editor: `proposals`,
/// Vorschau: je Abschnitt `flow`). `sheet`: im Editor das Satzbild der gezeigten Fassung, fertig vom
/// Server (wie `wysiwyg.parse`), damit das Widget es ohne Warten auf RPC öffnet (Runde 8, 2d).
#[derive(Serialize)]
struct InlineView {
    key: i64,
    rev: u32,
    count: usize,
    /// Runde 22 (R22a): Zahl der weiteren nicht aktuellen Fassungen außer Vorschlägen (Hinweis in der Vorschau).
    alternatives: usize,
    proposals: Vec<ProposalView>,
    entries: Vec<FlowEntry>,
    sheet: Option<String>,
}

/// Persona-Eintrag zu einem Handle: Stelle in der Reihenfolge und Farbe.
fn color_of<'a>(colors: &'a [PersonaColor], handle: &str) -> Option<(usize, &'a PersonaColor)> {
    colors.iter().enumerate().find(|(_, c)| c.handle == handle)
}

/// Stil einer Persona (Farbe aus der Liste, sonst `FALLBACK_COLOR`).
fn persona_style(colors: &[PersonaColor], handle: &str) -> String {
    crate::personas::style(color_of(colors, handle).map_or(FALLBACK_COLOR, |(_, c)| c.color.as_str()))
}

/// Zeilen-Diff für die Vorlage.
fn diff_lines(base: &str, proposal: &str) -> Vec<DiffLine> {
    line_diff(base, proposal)
        .into_iter()
        .map(|(kind, text)| DiffLine {
            kind: match kind {
                DiffKind::Same => "same",
                DiffKind::Added => "added",
                DiffKind::Removed => "removed",
            },
            text,
        })
        .collect()
}

/// Quelltext der obersten Blöcke `range` (gekürzt auf die Blockzahl).
fn raw_of(blocks: &[baruch_wysiwyg::Block], range: &std::ops::Range<usize>) -> String {
    let (start, end) = (range.start.min(blocks.len()), range.end.min(blocks.len()));
    blocks[start..end].iter().map(|b| b.raw.as_str()).collect()
}

/// Offene Vorschläge eines Abschnitts, nach Persona-Reihenfolge (`position`), darin nach Nummer;
/// Autoren ohne Persona-Eintrag am Ende. Jeder mit seinen Stücken gegen die gezeigte Fassung
/// `shown` (Runde 8: Basis der Einblendung ist, was „Übernehmen“ ersetzt). Ist `shown` selbst der
/// Vorschlag, stehen seine Stücke gegen seine Basis (Elternteil, sonst die aktuelle) an ihrer Stelle
/// in ihm. `satz`: auch das Satzbild der Stücke erzeugen.
fn proposals_of(
    node: &Node,
    shown: &baruch_core::Revision,
    ctx: &ProposalContext<'_>,
    satz: bool,
) -> Vec<ProposalView> {
    let infos = infos_of(ctx.revisions, node.id);
    let current = node.current_number();
    let shown_blocks = baruch_wysiwyg::parse(&shown.body).blocks;
    let mut out: Vec<(usize, ProposalView)> = node
        .revisions
        .iter()
        .filter(|r| r.status == RevisionStatus::Proposed)
        .map(|revision| {
            let (handle, model) = match &revision.author {
                Author::Assistant { persona, model } => (persona.clone(), model.clone()),
                Author::Human { name } => (name.clone(), String::new()),
            };
            let entry = color_of(ctx.colors, &handle);
            let name = entry.map_or(handle.clone(), |(_, c)| c.name.clone());
            let based_on = infos
                .iter()
                .find(|i| i.number == revision.number)
                .and_then(|i| i.based_on)
                .filter(|&b| node.revision(b).is_some());
            let is_shown = shown.number == revision.number;
            // Gezeigt ist der Vorschlag selbst: seine Stücke gegen seine Basis, Stellen in ihm.
            let base =
                if is_shown { based_on.or(current).filter(|&b| b != revision.number) } else { Some(shown.number) };
            let base_body = base.and_then(|b| node.revision(b)).map_or("", |r| r.body.as_str());
            let base_note = if is_shown {
                Some(match base {
                    Some(b) => format!(
                        "Unterschied zu Fassung {b}{} · gezeigt",
                        if base == current { " (aktuell)" } else { "" }
                    ),
                    None => "gezeigt".to_owned(),
                })
            } else {
                None
            };
            let mut head = format!("{name} ({handle}) · Fassung {}", revision.number);
            if !model.is_empty() {
                head.push_str(&format!(" · {model}"));
            }
            // Runde 22 (§2.5): Stand der Basis, wenn sie fortgeschrieben war („auf Fassung n, Stand k“).
            let base_amend =
                infos.iter().find(|i| i.number == revision.number).and_then(|i| i.based_on_amend).filter(|&k| k > 0);
            match (based_on.filter(|&b| b != shown.number), base_amend) {
                (Some(b), Some(k)) => head.push_str(&format!(" · beruht auf Fassung {b}, Stand {k}")),
                (Some(b), None) => head.push_str(&format!(" · auf Fassung {b}")),
                (None, Some(k)) => head.push_str(&format!(" · beruht auf Stand {k}")),
                (None, None) => {}
            }
            // Gutachten R22: Basis seit dem Vorschlag weiter fortgeschrieben → Hinweis „veraltet“; Übernehmen nähme
            // die späteren Fortschreibungen aus dem aktuellen Text.
            let raw_amend = infos.iter().find(|i| i.number == revision.number).and_then(|i| i.based_on_amend);
            if let (Some(b), Some(k)) = (based_on, raw_amend)
                && let Some(now) = infos.iter().find(|i| i.number == b).map(|i| i.amend_seq)
                && now > k
            {
                head.push_str(&format!(" · veraltet: Fassung {b} ist inzwischen bei Stand {now}"));
            }
            let hunks = baruch_wysiwyg::align(base_body, &revision.body);
            let (old, new) = (baruch_wysiwyg::parse(base_body).blocks, baruch_wysiwyg::parse(&revision.body).blocks);
            let id = format!("{}-{}", node.key, revision.number);
            let meta = HunkMeta {
                id: id.clone(),
                style: persona_style(ctx.colors, &handle),
                known: entry.is_some(),
                open: is_shown || !ctx.folded.iter().any(|&f| f == (node.key, revision.number) || f == FOLD_ALL),
                shown: is_shown,
                handle,
                name,
                note: revision.note.clone(),
                head: head.clone(),
                base_note,
            };
            let parts = hunks.len();
            let mut views: Vec<HunkView> = hunks
                .iter()
                .enumerate()
                .map(|(k, hunk)| {
                    // Stelle in der gezeigten Fassung: beim gezeigten Vorschlag in ihm selbst.
                    let place = if is_shown { &hunk.proposal } else { &hunk.base };
                    let mut meta = meta.clone();
                    if parts > 1 {
                        meta.head = format!("{head} · Teil {}/{parts}", k + 1);
                    }
                    HunkView {
                        meta,
                        at: place.start,
                        gap: if place.is_empty() { place.start } else { place.end },
                        kind: match hunk.kind {
                            baruch_wysiwyg::HunkKind::Changed => "changed",
                            baruch_wysiwyg::HunkKind::Inserted => "inserted",
                            baruch_wysiwyg::HunkKind::Removed => "removed",
                        },
                        satz: satz.then(|| {
                            baruch_wysiwyg::hunk_html_with(base_body, &revision.body, hunk, &|p| ctx.image(p))
                        }),
                        // Ohne Leerraum am Ende: Die Leerzeile hinter einem Block gehört zu seinem `raw` und
                        // stand sonst als leere „−“-Zeile am Ende des Stücks (Backlog Runde 8, Runde 18c).
                        lines: diff_lines(raw_of(&old, &hunk.base).trim_end(), raw_of(&new, &hunk.proposal).trim_end()),
                    }
                })
                .collect();
            if views.is_empty() {
                // Gleicht der gezeigten Fassung: eine Einblendung am Ende, damit Übernehmen/Verwerfen bleiben.
                let mut meta = meta;
                if meta.base_note.is_none() {
                    meta.base_note = Some("gleicht der gezeigten Fassung".to_owned());
                }
                let end = shown_blocks.len();
                views.push(HunkView { meta, at: end, gap: end, kind: "changed", satz: None, lines: Vec::new() });
            }
            let proposal = ProposalView { id, key: node.key, node: node.id, rev: revision.number, hunks: views };
            (entry.map_or(usize::MAX, |(position, _)| position), proposal)
        })
        .collect();
    out.sort_by_key(|(position, view)| (*position, view.rev));
    out.into_iter().map(|(_, view)| view).collect()
}

/// Lesefluss der gezeigten Fassung `shown` eines Abschnitts: ihre obersten Blöcke und dazwischen die
/// Stücke aller offenen Vorschläge in ihrer Lücke, an derselben Stelle untereinander in
/// Persona-Reihenfolge, darin nach Nummer (Reihenfolge von [`proposals_of`]). `lines`: Zeilen der
/// Vorschau je Block; `sheet`: fertiges Satzbild für den Editor.
fn inline_view(
    node: &Node,
    shown: &baruch_core::Revision,
    mut proposals: Vec<ProposalView>,
    lines: bool,
    sheet: Option<String>,
) -> InlineView {
    let blocks = baruch_wysiwyg::parse(&shown.body).blocks;
    let mut gaps: Vec<Vec<HunkView>> = (0..=blocks.len()).map(|_| Vec::new()).collect();
    for proposal in &mut proposals {
        for hunk in std::mem::take(&mut proposal.hunks) {
            gaps[hunk.gap.min(blocks.len())].push(hunk);
        }
    }
    let mut entries = Vec::new();
    for (gap, hunks) in gaps.into_iter().enumerate() {
        if !hunks.is_empty() {
            entries.push(FlowEntry { b: None, raw: String::new(), lines: Vec::new(), hunks });
        }
        if let Some(block) = blocks.get(gap) {
            let shown_lines = if lines { preview_lines(&block.raw) } else { Vec::new() };
            entries.push(FlowEntry { b: Some(gap), raw: block.raw.clone(), lines: shown_lines, hunks: Vec::new() });
        }
    }
    let alternatives = alternatives_of(node).len();
    InlineView { key: node.key, rev: shown.number, count: proposals.len(), alternatives, proposals, entries, sheet }
}

/// Größter Abschnittstext (Bytes, 64 KiB), zu dem der Server das Satzbild vorab ins HTML setzt (Runde 18c).
pub const READY_SHEET_MAX: usize = 64 * 1024;

/// Satzbild der gezeigten Fassung fertig vom Server (wie `wysiwyg.parse`), damit das Widget es beim
/// Laden sofort zeigt (Runde 8, 2d). Nur, wenn das Textfeld denselben Text liefert, auf den sich die
/// Indizes `data-b` beziehen: Der Browser streicht einen Zeilenumbruch direkt hinter `<textarea>` und
/// macht aus `\r\n`/`\r` ein `\n`; dann holt das Widget das Satzbild wie bisher über RPC.
///
/// Runde 18c: Abschnitte über [`READY_SHEET_MAX`] Bytes bekommen kein vorab gesetztes Satzbild (es verdoppelte den
/// Text im HTML: Textfeld, Blatt, mit Vorschlägen auch Lesefluss); das Widget holt es per RPC.
fn ready_sheet(body: &str, ctx: &ProposalContext<'_>) -> Option<String> {
    if body.len() > READY_SHEET_MAX || body.starts_with('\n') || body.contains('\r') {
        return None;
    }
    let doc = baruch_wysiwyg::parse(body);
    Some(baruch_wysiwyg::render_html_with(&doc, &|p| ctx.image(p)))
}

/// Eine Fassung in der kleinen Baumansicht (eingerückt nach `based_on`).
#[derive(Serialize)]
struct TreeRevision {
    number: u32,
    depth: usize,
    based_on: Option<u32>,
    status: &'static str,
    status_key: &'static str,
    author: String,
    persona: bool,
    /// Bei Persona-Fassungen die CSS-Variablen ihrer Farbe.
    style: Option<String>,
    created: String,
    edited: Option<String>,
    current: bool,
    shown: bool,
    show_url: String,
    try_url: String,
    /// Runde 28: übernommene Befunde, die diese Fassung umsetzt ([`baruch_store::Store::findings_adopted_in`]).
    findings: Vec<i64>,
    /// Befunde mit offenem Vorschlag „übernommen in dieser Fassung“.
    proposed_findings: Vec<i64>,
    /// `/werke/{work}/befunde`
    findings_base: String,
    /// Runde 30b: Kennzeichen einer Variante ([`crate::assist::variants::revision_variant`]); `None` = keine.
    variant: Option<serde_json::Value>,
    /// Runde 22: Stand der Fassung (0 = nie fortgeschrieben) und Link auf ihren Verlauf (nur bei Stand > 0).
    amend_seq: u32,
    history_url: Option<String>,
    /// Runde 22: Stand der Basis eines Vorschlags („beruht auf Fassung n, Stand k“, nur bei k > 0).
    based_on_amend: Option<u32>,
    /// 0.9.0: unter der Marke ausgeblendet (steht hinter dem Schalter „n ältere Fassungen“).
    hidden: bool,
    /// 0.9.0: „bis hierhin ausblenden“ wird an dieser Fassung angeboten (nur im gezeigten Abschnitt).
    fold: bool,
}

/// Zeitstempel `2026-11-05T10:11:12.123Z` (UTC) → `2026-11-05 11:11` in Ortszeit.
fn short_time(time: &str) -> String {
    crate::localtime::minutes(time)
}

fn status_key(status: RevisionStatus) -> &'static str {
    match status {
        RevisionStatus::Current => "current",
        RevisionStatus::Proposed => "proposed",
        RevisionStatus::Rejected => "rejected",
        RevisionStatus::Superseded => "superseded",
    }
}

/// Fassungen eines Abschnitts in Baumreihenfolge: Wurzeln (ohne oder mit unbekanntem Elternteil)
/// nach Nummer, darunter je die Kinder nach Nummer, mit Tiefe.
fn ordered_tree(infos: &[RevisionInfo]) -> Vec<(usize, &RevisionInfo)> {
    let known: Vec<u32> = infos.iter().map(|r| r.number).collect();
    let mut children: HashMap<Option<u32>, Vec<&RevisionInfo>> = HashMap::new();
    for info in infos {
        let parent = info.based_on.filter(|b| known.contains(b) && *b < info.number);
        children.entry(parent).or_default().push(info);
    }
    for list in children.values_mut() {
        list.sort_by_key(|r| r.number);
    }
    let mut out = Vec::with_capacity(infos.len());
    let mut stack: Vec<(usize, &RevisionInfo)> =
        children.get(&None).map(|roots| roots.iter().rev().map(|r| (0, *r)).collect()).unwrap_or_default();
    while let Some((depth, info)) = stack.pop() {
        out.push((depth, info));
        if let Some(list) = children.get(&Some(info.number)) {
            stack.extend(list.iter().rev().map(|r| (depth + 1, *r)));
        }
    }
    out
}

/// Zusammenhang für die Links der Baumansicht.
struct TreeLinks<'a> {
    work: WorkId,
    at: &'a Resolved,
    lq_q: &'a str,
    colors: &'a [PersonaColor],
    findings: &'a crate::findings::view::FindingsView,
    /// 0.9.0: Abschnitte des Werks und ihre Marken (ausgeblendete Fassungen).
    nodes: &'a [Node],
    folds: &'a HashMap<i64, u32>,
}

/// Fassungen eines Abschnitts (`node` = Stelle, `key` = fester Schlüssel) für die Baumansicht; die
/// Links tragen den Schlüssel (`zeigen`) bzw. den des gezeigten Abschnitts (`probieren`), `pick` bleibt
/// `<seq>:<rev>`.
fn tree_revisions(node: usize, key: i64, infos: &[RevisionInfo], links: &TreeLinks<'_>) -> Vec<TreeRevision> {
    let mark = links.folds.get(&key).copied();
    ordered_tree(infos)
        .into_iter()
        .map(|(depth, info)| {
            let mut pick: Vec<(usize, u32)> = links.at.pick.iter().copied().filter(|&(n, _)| n != node).collect();
            pick.push((node, info.number));
            pick.sort_unstable();
            let (work, at, lq_q) = (links.work, links.at, links.lq_q);
            TreeRevision {
                number: info.number,
                depth,
                based_on: info.based_on,
                status: status_label(info.status),
                status_key: status_key(info.status),
                author: author_label(&info.author),
                persona: matches!(&info.author, Author::Assistant { persona, .. } if !is_lektorat(persona)),
                style: match &info.author {
                    Author::Assistant { persona, .. } => Some(persona_style(links.colors, persona)),
                    Author::Human { .. } => None,
                },
                created: short_time(&info.created),
                edited: info.edited.as_deref().map(short_time),
                current: info.status == RevisionStatus::Current,
                shown: node == at.node && info.number == at.rev,
                show_url: node_url(work, key, Some(info.number), false, "", lq_q),
                try_url: node_url(work, at.key, Some(at.rev), true, &pick_param(&pick), lq_q),
                findings: links.findings.adopted_in(key, info.number),
                proposed_findings: links.findings.proposed_for(key, info.number),
                findings_base: format!("/werke/{work}/befunde"),
                variant: crate::assist::variants::revision_variant(info),
                amend_seq: info.amend_seq,
                history_url: (info.amend_seq > 0).then(|| history_url(work, key, info.number)),
                based_on_amend: info.based_on_amend.filter(|&k| k > 0),
                hidden: links
                    .nodes
                    .get(node)
                    .is_some_and(|n| n.revision(info.number).is_some_and(|r| revision_fold::is_hidden(n, mark, r))),
                fold: node == at.node
                    && links.nodes.get(node).is_some_and(|n| revision_fold::offered(n, mark, info.number)),
            }
        })
        .collect()
}

/// 0.9.0 (Nutzer 2026-11-24): Marke „ältere Fassungen ausblenden“ des gezeigten Abschnitts für Kopf und Akkordeon.
#[derive(Debug, Serialize)]
struct FoldView {
    /// Gespeicherte Marke (Nummer) und wirksame Grenze (höchstens die aktuelle Fassung).
    mark: Option<u32>,
    cutoff: Option<u32>,
    /// Zahl der ausgeblendeten Fassungen.
    hidden: usize,
    /// Schalter offen: `?fassungen=alle` oder die gezeigte Fassung ist selbst ausgeblendet.
    open: bool,
    /// Zähler und Nachbarn über die sichtbaren Fassungen (bei offenem Schalter über alle).
    position: usize,
    total: usize,
    prev: Option<u32>,
    next: Option<u32>,
    /// Zusatz der Blätter-Links (`&fassungen=alle` bei offenem Schalter, sonst leer).
    q: &'static str,
    /// „zeigen“ (Schalter auf) und „verbergen“ (Schalter zu; steht die gezeigte Fassung unter der Marke, auf die
    /// aktuelle Fassung).
    show_url: String,
    hide_url: String,
    /// „bis hierhin ausblenden“ an der gezeigten Fassung, und alle Fassungen mit diesem Angebot (Formulare
    /// `rev-fold-<n>` im Akkordeon, Knöpfe im Kopf und in der Baumansicht zeigen über `form` darauf).
    offered: bool,
    numbers: Vec<u32>,
}

fn fold_view(
    node: &Node,
    revision: &baruch_core::Revision,
    at: &Resolved,
    folds: &HashMap<i64, u32>,
    work: WorkId,
    lq_q: &str,
    pick_q: &str,
) -> FoldView {
    let mark = folds.get(&node.key).copied();
    let hidden = revision_fold::hidden_count(node, mark);
    let open = hidden > 0 && (at.show_all || revision_fold::is_hidden(node, mark, revision));
    let visible: Vec<u32> = node
        .revisions
        .iter()
        .filter(|r| open || r.number == revision.number || !revision_fold::is_hidden(node, mark, r))
        .map(|r| r.number)
        .collect();
    let index = visible.iter().position(|&n| n == revision.number).unwrap_or(0);
    let url = |rev: u32| node_url(work, node.key, Some(rev), at.preview, pick_q, lq_q);
    let back = if revision_fold::is_hidden(node, mark, revision) {
        node.current_number().unwrap_or(revision.number)
    } else {
        revision.number
    };
    FoldView {
        mark,
        cutoff: revision_fold::cutoff(node, mark),
        hidden,
        open,
        position: index + 1,
        total: visible.len(),
        prev: index.checked_sub(1).and_then(|i| visible.get(i).copied()),
        next: visible.get(index + 1).copied(),
        q: if open { FOLD_OPEN_Q } else { "" },
        show_url: format!("{}{FOLD_OPEN_Q}", url(revision.number)),
        hide_url: url(back),
        offered: revision_fold::offered(node, mark, revision.number),
        numbers: node.revisions.iter().map(|r| r.number).filter(|&n| revision_fold::offered(node, mark, n)).collect(),
    }
}

/// Runde 22: Seite „Verlauf“ einer Fassung (`GET /werke/{id}/fassungen/verlauf?key=K&rev=R`).
pub fn history_url(work: WorkId, key: i64, rev: u32) -> String {
    format!("/werke/{work}/fassungen/verlauf?key={key}&rev={rev}")
}

/// Runde 22 (R22a): nicht aktuelle Fassungen eines Abschnitts außer offenen Vorschlägen (ersetzt, verworfen), nach
/// Nummer – der Hinweis „weitere Fassung(en)“ im Lesefluss und im Baum.
fn alternatives_of(node: &Node) -> Vec<&baruch_core::Revision> {
    node.revisions
        .iter()
        .filter(|r| matches!(r.status, RevisionStatus::Superseded | RevisionStatus::Rejected))
        .collect()
}

/// Runde 22: Rückmeldung nach dem Speichern ohne JavaScript aus `saved` (`amended-<k>`, `new-<n>[-<grund>]`);
/// Ungültiges → `None`.
fn saved_notice(saved: Option<&str>) -> Option<String> {
    let saved = saved?;
    let mut parts = saved.split('-');
    match (parts.next()?, parts.next()?.parse::<u32>().ok()?, parts.next(), parts.next()) {
        ("amended", k, None, None) => Some(format!("Gespeichert: fortgeschrieben (Stand {k}).")),
        ("new", n, None, None) => Some(format!("Gespeichert: neue Fassung {n}.")),
        ("new", n, Some(reason), None) => {
            let label = REASONS.iter().find(|r| r.id() == reason)?.label();
            Some(format!("Gespeichert: neue Fassung {n} ({label})."))
        }
        _ => None,
    }
}

/// Alle Gründe für eine neue Fassung (für [`saved_notice`]).
const REASONS: [baruch_store::NewReason; 8] = [
    baruch_store::NewReason::Requested,
    baruch_store::NewReason::NoCurrent,
    baruch_store::NewReason::PolicyOff,
    baruch_store::NewReason::NotOwn,
    baruch_store::NewReason::NotOnCurrent,
    baruch_store::NewReason::HeadingChanged,
    baruch_store::NewReason::LargeChange,
    baruch_store::NewReason::TimeElapsed,
];

/// Fassungen eines Abschnitts aus dem Revisionsbaum des Werks (leer, wenn unbekannt).
fn infos_of(tree: &[NodeRevisions], node: usize) -> &[RevisionInfo] {
    tree.iter().find(|n| n.node == node).map_or(&[], |n| n.revisions.as_slice())
}

/// Beiträge mit aufgelösten Verweisen (Nummer, Titel, Adresse der Stelle in Key-Form), id und
/// Zeitpunkt (`data-post-id`, `data-post-created` für das Widget „chat“: neu seit dem letzten Besuch).
/// Runde 28: nur die Beiträge in [`crate::posts::visible_range`], je Beitrag dazu `edited` (Zeit der letzten
/// Änderung oder none), `edited_time` (kurz) und `own` (Beitrag des Menschen, also bearbeitbar).
/// Runde 30 (Haken): `extra` ([`crate::chat_render::post_extra`]) und `solutions`
/// ([`crate::assist::solutions::post_solutions`] gegen die gezeigte Fassung `shown_rev`).
#[allow(clippy::too_many_arguments)]
fn posts(
    work: &Work,
    node: &Node,
    work_id: WorkId,
    lq: &str,
    colors: &[PersonaColor],
    edits: &HashMap<i64, String>,
    shown_rev: u32,
    extras: &ViewExtras,
    lektor: Option<&crate::auth::Identity>,
) -> Value {
    let shown_body = node.revisions.iter().find(|r| r.number == shown_rev).map_or("", |r| r.body.as_str());
    let ctx = crate::chat_render::PostCtx { work_id, work, key: node.key, shown_rev, extras };
    let range = crate::posts::visible_range(node.posts.len());
    let posts: Vec<Value> = node.posts[range]
        .iter()
        .map(|post| {
            let refs: Vec<Value> = post
                .refs
                .iter()
                .filter_map(|r| work.nodes.get(r.node).map(|target| (r, target)))
                .map(|(r, target)| {
                    context! {
                        number => target.number.as_str(), title => target.title.as_str(),
                        rev => r.rev, quote => r.quote.as_str(),
                        url => node_url(work_id, target.key, Some(r.rev), false, "", lq),
                    }
                })
                .collect();
            // Farbe nur für Beiträge von Personas (Handle mit Eintrag), nicht für den Menschen.
            let style = color_of(colors, &post.author).map(|(_, c)| crate::personas::style(&c.color));
            let edited = edits.get(&post.id);
            context! {
                id => post.id, created => post.created.as_str(), time => short_time(&post.created),
                author => post.author.as_str(), body => post.body.as_str(), style => style,
                sources => Value::from(Serde(&post.sources)), refs => refs,
                edited => edited.map(String::as_str), edited_time => edited.map(|t| short_time(t)),
                own => post.author == crate::mirror::HUMAN_NAME,
                // Runde 35: eigener Beitrag eines Lektors (Autor des Beitrags = seine Herkunft) – nur den darf er
                // ändern oder entfernen.
                mine => lektor.is_some_and(|l| post.author == l.handle),
                extra => post_extra_for(&ctx, post, lektor.is_some()),
                solutions => Value::from(Serde(crate::assist::solutions::post_solutions(&post.body, shown_body))),
            }
        })
        .collect();
    Value::from(posts)
}

/// Zusatz eines Beitrags für die Vorlage ([`crate::chat_render::post_extra`]). Für Lektoren mit leerer Liste der
/// Karten der Befund-Blöcke: Die Karten tragen die Formulare zum Status (nur der Autor); ohne Karte zeigt die Vorlage
/// den Befund als gewöhnlichen Text des Beitrags. Die Liste bleibt als leere Abbildung stehen – die Vorlage schlägt
/// darin nach, und ein Nachschlagen in einem fehlenden Wert wäre ein Fehler der Vorlage.
fn post_extra_for(ctx: &crate::chat_render::PostCtx<'_>, post: &baruch_core::Post, lektor: bool) -> Value {
    let extra = crate::chat_render::post_extra(ctx, post);
    if !lektor {
        return Value::from(Serde(extra));
    }
    match serde_json::to_value(&extra) {
        Ok(serde_json::Value::Object(mut map)) => {
            map.insert("finding_cards".to_owned(), serde_json::json!({}));
            Value::from(Serde(serde_json::Value::Object(map)))
        }
        Ok(other) => Value::from(Serde(other)),
        Err(_) => Value::from(()),
    }
}

/// Ziele für „Verschieben…“ im Abschnitts-Chat: alle Abschnitte des Werks außer dem gezeigten, je
/// `{key, label}` mit „Nummer Titel“ (eingerückt nach Ebene).
fn post_targets(work: &Work, shown: usize) -> Vec<Value> {
    let levels = levels(work);
    work.nodes
        .iter()
        .filter(|n| n.id != shown)
        .map(|n| {
            // Geschützte Leerzeichen: gewöhnliche fielen in <option> zusammen.
            let indent = "\u{a0}\u{a0}".repeat(usize::from(levels[n.id].saturating_sub(1)));
            let title = if n.title.is_empty() { "(ohne Titel)" } else { n.title.as_str() };
            let label =
                if n.number.is_empty() { format!("{indent}{title}") } else { format!("{indent}{} {title}", n.number) };
            context! { key => n.key, label => label }
        })
        .collect()
}

/// Akkordeon „Befunde“ am gezeigten Abschnitt (Runde 28): offene Befunde (ohne Lob, angehängte unter dem Original)
/// als Karten mit Statusformularen, entschiedene und Lob eingeklappt, offene Vorschläge, Ziele für „verschoben“ und
/// der Rücksprung auf die Arbeitsseite.
fn findings_panel(
    work: &Work,
    node: &Node,
    work_id: WorkId,
    at: &Resolved,
    findings: &crate::findings::view::FindingsView,
    lq: &Option<String>,
) -> serde_json::Value {
    use crate::findings::{FindingKind, FindingStatus};
    let all = &findings.findings;
    let own: Vec<&crate::findings::Finding> = findings.of_key(node.key).collect();
    // Angehängte Befunde stehen unter ihrem Original (in der Karte), nicht für sich.
    let top: Vec<&crate::findings::Finding> =
        own.iter().copied().filter(|f| f.similar_to.is_none_or(|o| !own.iter().any(|x| x.id == o))).collect();
    let cards = |keep: &dyn Fn(&crate::findings::Finding) -> bool| -> Vec<serde_json::Value> {
        top.iter().filter(|f| keep(f)).map(|f| crate::findings::view::card(work_id, work, f, all)).collect()
    };
    let open = cards(&|f| f.kind != FindingKind::Praise && (f.status == FindingStatus::Open || f.pending.is_some()));
    let decided = cards(&|f| f.kind != FindingKind::Praise && f.status != FindingStatus::Open && f.pending.is_none());
    let praise = cards(&|f| f.kind == FindingKind::Praise);
    let back = node_url(work_id, node.key, Some(at.rev), at.preview, "", &lq_param(lq.as_deref()));
    serde_json::json!({
        "open": findings.open_by_key.get(&node.key).copied().unwrap_or(0),
        "pending": own.iter().filter(|f| f.pending.is_some()).count(),
        "total": own.iter().filter(|f| f.similar_to.is_none()).count(),
        "cards": open,
        "decided": decided,
        "praise": praise,
        "targets": crate::findings::page::defer_targets(work),
        "back": back,
        "page_url": format!("/werke/{work_id}/befunde?key={}#zelle", node.key),
    })
}

/// Muster zur Auswahl im Kopf der Gliederung (dazu „aus der Vorlage“, „keine Nummern“ und ein
/// eigenes Muster in der Mustersprache, Runde 6: „I|1“ nur Kapitel, „…;…“ je Ebene).
const NUMBERING_PRESETS: &[&str] = &["1.1", "I.1", "1.a)", "A.1", "I|1", "1|1", "I.;A.;1.;a);aa)"];

pub struct Panes {
    pub tree: String,
    pub editor: String,
    pub assistants: String,
}

fn values(work: &Work, shelf: &Shelf, at: &Resolved, settings: &Settings, library: &LibraryView) -> Value {
    // Runde 35: Wer fragt? Ein Lektor bekommt eigene Vorlagen (siehe Modulkopf).
    let who = crate::auth::current();
    let lektor = who.lektor();
    let node = &work.nodes[at.node];
    let revision = &node.revisions[at.index];
    let total = node.revisions.len();
    let proposal = revision.status == RevisionStatus::Proposed;
    let infos = infos_of(&library.revisions, node.id);
    let info = infos.iter().find(|r| r.number == revision.number);
    let current = node.current_number();
    let colors = library.persona_colors.as_slice();
    // Offene Vorschläge an ihrer Stelle im Lesefluss: im Editor die des Abschnitts mit Satzbild- und
    // Zeilen-Diff, in der Vorschau je Abschnitt des Kapitels.
    let proposal_ctx = ProposalContext {
        work: shelf.id,
        revisions: &library.revisions,
        colors,
        image_alts: &library.image_alts,
        folded: &settings.folded,
    };
    // Runde 8: im Editor die Stücke an ihrer Stelle in der gezeigten Fassung, dazu ihr fertiges
    // Satzbild (nur wenn das Satzbild die Vorgabe ist, sonst bliebe es ungenutzt).
    let proposals = (!at.preview).then(|| {
        let sheet = (settings.editor == "satz").then(|| ready_sheet(&revision.body, &proposal_ctx)).flatten();
        inline_view(node, revision, proposals_of(node, revision, &proposal_ctx, true), false, sheet)
    });
    // Runde 11: Bilder des Werks und der Vorlage (Kapitelbild im Kopf, Einfügen, fehlende Bilder).
    let images = &library.images;
    let chapter_picture = chapter_image(work, node.id, shelf.id, images);
    let missing = if at.preview {
        let bodies: Vec<&str> = chapter_nodes(work, node.id)
            .into_iter()
            .filter_map(|n| {
                let picked = at.pick.iter().find(|(id, _)| *id == n.id).and_then(|&(_, rev)| n.revision(rev));
                picked.or_else(|| n.current_index().map(|i| &n.revisions[i])).map(|r| r.body.as_str())
            })
            .collect();
        missing_images(&bodies, images)
    } else {
        missing_images(&[revision.body.as_str()], images)
    };
    let image_choices: Vec<Value> = images
        .assets
        .iter()
        .map(|a| context! { name => a.name.as_str(), caption => a.caption.as_str(), alt => a.alt.as_str() })
        .collect();
    // Runde 18c: Stellen für „Bild einfügen“ ohne JavaScript (Ende, Anfang, vor jedem obersten Block der gezeigten
    // Fassung); mit JavaScript kommt „an der Cursorstelle“ dazu (web/src/sheet.ts).
    let image_places: Vec<Value> = if at.preview || image_choices.is_empty() {
        Vec::new()
    } else {
        crate::assets::image_places(&revision.body)
            .into_iter()
            .map(|(value, label)| context! { value => value, label => label })
            .collect()
    };
    // Integrator Runde 17 (L4): Lektor und Kontext wie in Einstellungen → Assistenten (wirksam, ohne Netzabfrage);
    // ohne geladene Liste wie bisher nach `Work::lector` und der Einstellung.
    let lector = library.extras.lector.clone().or_else(|| work.lector().map(|p| p.handle.clone()));
    let personas: Vec<_> = work
        .personas
        .iter()
        .map(|p| {
            // Runde 16b: lokal/extern aus persona.list; extern ohne Freigabe ist gesperrt (nicht wählbar).
            let access = library.extras.personas.iter().find(|a| a.handle == p.handle);
            context! {
                handle => p.handle.as_str(), name => p.name.as_str(), role => p.role.as_str(), model => p.model.as_str(),
                context_k => access.and_then(|a| a.context_tokens).unwrap_or(p.context_length) / 1024,
                active => p.active,
                lector => lector.as_deref() == Some(p.handle.as_str()),
                style => persona_style(colors, &p.handle),
                locality => access.map_or("lokal", |a| a.locality.as_str()),
                connection => access.map_or("", |a| a.connection.as_str()),
                blocked => access.is_some_and(PersonaAccess::blocked),
            }
        })
        .collect();
    let mut tree_items = tree(work, None, at.node, &settings.closed);
    // Runde 28: Abzeichen „k offen“ je Punkt, aufsummiert für Kapitel und Teile (Zählregel §1.5).
    let findings_view = &library.extras.findings;
    let open_sums = crate::findings::view::subtree_open(work, &findings_view.open_by_key);
    attach_findings(&mut tree_items, &open_sums, &findings_view.open_by_key);
    let shown_timing = library.extras.timing.as_ref().filter(|t| t.shown());
    if let Some(timing) = shown_timing {
        attach_timing(&mut tree_items, timing);
    }
    let timing = timing_context(shown_timing, node, &revision.body);
    let view = if at.preview { "preview" } else { "edit" };
    let lq_q = lq_param(library.query.as_deref());
    let shown_work = shelf.id;
    let pick_q = pick_param(&at.pick);
    let links = TreeLinks {
        work: shelf.id,
        at,
        lq_q: &lq_q,
        colors,
        findings: findings_view,
        nodes: &work.nodes,
        folds: &library.extras.revision_folds,
    };
    let fold = fold_view(node, revision, at, &library.extras.revision_folds, shelf.id, &lq_q, &pick_q);
    let findings_panel = findings_panel(work, node, shelf.id, at, findings_view, &library.query);
    let section_tree = tree_revisions(node.id, node.key, infos, &links);
    let chapter_tree: Vec<Value> = chapter_nodes(work, node.id)
        .into_iter()
        .map(|n| {
            let revisions = tree_revisions(n.id, n.key, infos_of(&library.revisions, n.id), &links);
            context! {
                node => n.id, key => n.key, number => n.number.as_str(), title => n.title.as_str(),
                current => n.current_number(), selected => n.id == node.id,
                revisions => Value::from(Serde(revisions)),
            }
        })
        .collect();
    let picked: Vec<Value> = at
        .pick
        .iter()
        .filter_map(|&(id, rev)| work.nodes.get(id).map(|n| (n, rev)))
        .map(|(n, rev)| context! { node => n.id, key => n.key, number => n.number.as_str(), title => n.title.as_str(), rev => rev, current => n.current_number() == Some(rev) })
        .collect();
    context! {
        work => context! {
            id => shelf.id, slug => shelf.slug.as_str(), title => work.title.as_str(), author => work.author.as_str(),
        },
        // Kopfzeile in base.html (Runde 9): Werkwahl (`topbar.works`), Menüs, Abzeichen (Anträge,
        // offene Vorschläge).
        topbar => crate::menu::workspace_topbar(work, shelf, at, library),
        lq_q => lq_q.as_str(),
        pick_q => pick_q.as_str(),
        tree => Value::from(Serde(tree_items)),
        // Runde 16b: Zeitwächter (nur bei Predigt oder Zielzeit, sonst none).
        timing => timing,
        // `key` (fester Schlüssel) für Editor-, Chat- und Literaturformulare: Die Stelle `id` kann
        // veralten, wenn ein anderer Tab die Gliederung umstellt.
        node => context! {
            id => node.id, key => node.key, number => node.number.as_str(), title => node.title.as_str(),
            posts => posts(work, node, shelf.id, &lq_q, colors, &library.extras.post_edits, at.rev, &library.extras, lektor),
            // Runde 28: Stelle des ersten gezeigten Beitrags (0 = alle gezeigt; ältere über /beitraege).
            post_older => crate::posts::visible_range(node.posts.len()).start,
            post_count => node.posts.len(),
            post_targets => post_targets(work, node.id),
            // Runde 28: offene Befunde samt Unterpunkten (Editorkopf) und am Abschnitt selbst.
            open_findings => open_sums.get(node.id).copied().unwrap_or(0),
            open_findings_own => findings_view.open_by_key.get(&node.key).copied().unwrap_or(0),
            findings_url => format!("/werke/{}/befunde?key={}#zelle", shelf.id, node.key),
            // Die Adresse nannte einen Schlüssel, den es nicht (mehr) gibt (Hinweis im Editor-Kopf).
            missing_key => at.missing_key,
        },
        numbering => library.numbering.as_ref().map(|n| Value::from(Serde(n))),
        numbering_presets => Value::from(Serde(NUMBERING_PRESETS)),
        outline_requests => library.outline_requests,
        cover => library.cover.as_ref().map(|c| Value::from(Serde(c))),
        rev => context! {
            // 0.9.0: Zähler und Blättern über die sichtbaren Fassungen (ausgeblendete nur bei offenem Schalter).
            number => revision.number, position => fold.position, total => fold.total, all_total => total,
            status => status_label(revision.status), status_key => status_key(revision.status),
            author => author_label(&revision.author),
            author_style => match &revision.author {
                Author::Assistant { persona, .. } => Some(persona_style(colors, persona)),
                Author::Human { .. } => None,
            },
            body => revision.body.as_str(), note => revision.note.as_str(), proposal => proposal,
            is_current => revision.status == RevisionStatus::Current, current => current,
            based_on => info.and_then(|i| i.based_on), edited => info.and_then(|i| i.edited.as_deref()).map(short_time),
            prev => fold.prev, next => fold.next,
            // Abzeichen im Kopf des Akkordeons „Fassungen“: offene Vorschläge des Abschnitts.
            open_proposals => node.open_proposals(),
            // Fehler 0.9.0 (Kritik-Zyklus): offene Vorschläge für die Prüfgrundlage („Vorschlag n von @x“).
            open_list => node.revisions.iter().filter(|r| r.status == RevisionStatus::Proposed).map(|r| context! {
                number => r.number,
                label => match &r.author {
                    Author::Assistant { persona, .. } => format!("Vorschlag {} von {persona}", r.number),
                    Author::Human { .. } => format!("Vorschlag {}", r.number),
                },
            }).collect::<Vec<_>>(),
            // Runde 22: Stand der gezeigten Fassung, Verlauf, Stand der Basis eines Vorschlags.
            amend_seq => info.map_or(0, |i| i.amend_seq),
            history_url => info.filter(|i| i.amend_seq > 0).map(|i| history_url(shown_work, node.key, i.number)),
            based_on_amend => info.and_then(|i| i.based_on_amend).filter(|&k| k > 0),
            own_current => revision.status == RevisionStatus::Current && matches!(revision.author, Author::Human { .. }),
            // Runde 22 (R22a): weitere Fassungen außer Vorschlägen mit Link (?rev=) und „aktuell machen“.
            alternatives => alternatives_of(node).iter().map(|r| context! {
                number => r.number, status => status_label(r.status), author => author_label(&r.author),
                shown => r.number == revision.number,
                url => node_url(shown_work, node.key, Some(r.number), at.preview, "", &lq_q),
                hidden => revision_fold::is_hidden(node, fold.mark, r),
                fold => revision_fold::offered(node, fold.mark, r.number),
            }).collect::<Vec<_>>(),
            // 0.9.0: Marke „bis hierhin ausblenden“ (fragments/revisions.html, Editor-Kopf).
            fold => Value::from(Serde(&fold)),
            saved_notice => saved_notice(at.saved.as_deref()),
        },
        proposals => Value::from(Serde(&proposals)),
        preview => Value::from(Serde(if at.preview { preview(work, node, &at.pick, &proposal_ctx) } else { Vec::new() })),
        picked => picked,
        chapter_image => chapter_picture,
        missing_images => missing.iter().map(|p| crate::assets::path_text(p)).collect::<Vec<_>>(),
        image_choices => image_choices,
        image_places => image_places,
        revtree => context! { section => Value::from(Serde(section_tree)), chapter => chapter_tree },
        // Runde 28: Akkordeon „Befunde“ (fragments/findings_panel.html).
        findings_panel => Value::from(Serde(findings_panel)),
        personas => personas,
        // Runde 10: Titel des Lektor-Abzeichens („fest gewählt“ bzw. „größter Kontext“).
        lector_fixed => work.lector_choice.handle().is_some_and(|h| {
            lector.as_deref().is_some_and(|l| l.trim_start_matches('@') == h.trim().trim_start_matches('@'))
        }),
        view => view,
        other_view => if at.preview { "edit" } else { "preview" },
        url => at.url(shelf.id, library.query.as_deref()),
        settings => Value::from(Serde(settings)),
        html_style => settings.style(),
        library => Value::from(Serde(&library.titles)),
        library_query => library.query.as_deref(),
        library_hits => Value::from(Serde(&library.hits)),
        // Runde 23a: Kasten „Bilder des Werks“ zur Suche, auch ohne JavaScript im ausgelieferten HTML.
        library_images_html => Value::from_safe_string(library.images_html.clone()),
        // Runde 19c: Bibel-Kontext der Literatur auf der Arbeitsseite (ohne JavaScript; mit JavaScript lädt das Popup
        // denselben Kasten über `literature.render`).
        bibles => library.extras.bibles.as_ref().map(|b| b.context_at(Some(at.key), Some(at.rev))),
        // Runde 18a/18c (Schnittstelle R18-S1): Knöpfe des Menübands und CSS des Satzbilds des Werks.
        ribbon => Value::from(Serde(&library.extras.ribbon)),
        sheet_css => library.extras.sheet_css.as_deref(),
        // Runde 35: none außer für einen angemeldeten Lektor (dann Herkunft und Name für die Lektor-Vorlagen).
        lektor => lektor.map(|l| context! {
            handle => l.handle.as_str(),
            name => if l.display.trim().is_empty() { l.name.as_str() } else { l.display.as_str() },
        }),
    }
}

/// Vorlagen der drei Bereiche: für Lektoren die eigenen (`fragments/lektor_*.html`), sonst die des Autors.
fn pane_templates() -> [&'static str; 3] {
    if crate::auth::current().lektor().is_some() {
        ["fragments/lektor_tree.html", "fragments/lektor_editor.html", "fragments/lektor_assistants.html"]
    } else {
        ["fragments/tree.html", "fragments/editor.html", "fragments/assistants.html"]
    }
}

pub fn panes(
    env: &Environment<'_>,
    work: &Work,
    shelf: &Shelf,
    at: Resolved,
    settings: &Settings,
    library: &LibraryView,
) -> Result<Panes, minijinja::Error> {
    let ctx = values(work, shelf, &at, settings, library);
    let [tree, editor, assistants] = pane_templates();
    Ok(Panes {
        tree: env.get_template(tree)?.render(&ctx)?,
        editor: env.get_template(editor)?.render(&ctx)?,
        assistants: env.get_template(assistants)?.render(&ctx)?,
    })
}

pub fn page(
    env: &Environment<'_>,
    work: &Work,
    shelf: &Shelf,
    at: Resolved,
    settings: &Settings,
    library: &LibraryView,
) -> Result<String, minijinja::Error> {
    env.get_template("workspace.html")?.render(values(work, shelf, &at, settings, library))
}

pub fn environment() -> Environment<'static> {
    let mut env = Environment::new();
    for (name, source) in [
        ("base.html", include_str!("../templates/base.html")),
        ("workspace.html", include_str!("../templates/workspace.html")),
        ("fragments/tree.html", include_str!("../templates/fragments/tree.html")),
        ("fragments/editor.html", include_str!("../templates/fragments/editor.html")),
        ("fragments/edit_toolbar.html", include_str!("../templates/fragments/edit_toolbar.html")),
        ("fragments/revtree.html", include_str!("../templates/fragments/revtree.html")),
        ("fragments/revisions.html", include_str!("../templates/fragments/revisions.html")),
        ("fragments/findings_panel.html", include_str!("../templates/fragments/findings_panel.html")),
        ("fragments/assistants.html", include_str!("../templates/fragments/assistants.html")),
        // Runde 35: die drei Bereiche für Lektoren.
        ("fragments/lektor_tree.html", include_str!("../templates/fragments/lektor_tree.html")),
        ("fragments/lektor_editor.html", include_str!("../templates/fragments/lektor_editor.html")),
        ("fragments/lektor_assistants.html", include_str!("../templates/fragments/lektor_assistants.html")),
        ("fragments/proposals.html", include_str!("../templates/fragments/proposals.html")),
        ("fragments/library.html", include_str!("../templates/fragments/library.html")),
        ("fragments/literature_bibles.html", include_str!("../templates/fragments/literature_bibles.html")),
        ("fragments/work_new.html", include_str!("../templates/fragments/work_new.html")),
        ("fragments/idea_capture.html", include_str!("../templates/fragments/idea_capture.html")),
        ("fragments/toast.html", include_str!("../templates/fragments/toast.html")),
        ("fragments/assist_scope.html", include_str!("../templates/fragments/assist_scope.html")),
        // Runde 30 (Schnittstelle R30-S1): Gerüste, von keiner Vorlage eingebunden.
        ("fragments/search_bar.html", include_str!("../templates/fragments/search_bar.html")),
        ("fragments/post_item.html", include_str!("../templates/fragments/post_item.html")),
        ("fragments/finding_block.html", include_str!("../templates/fragments/finding_block.html")),
        ("fragments/post_solutions.html", include_str!("../templates/fragments/post_solutions.html")),
        ("fragments/basis_select.html", include_str!("../templates/fragments/basis_select.html")),
        // Runde 24 (Schnittstelle R24-S1): Über Baruch (src/about.rs).
        ("about.html", include_str!("../templates/about.html")),
    ] {
        env.add_template(name, source).expect("Vorlage fehlerhaft");
    }
    env
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_from_cookies() {
        let s = Settings::from_cookie_header(Some(
            "other=1; baruch_theme=dark; baruch_left=22.5%; baruch_right=99%; baruch_accordion=personas; baruch_closed=1.6.x",
        ));
        assert_eq!(s.theme, Some("dark"));
        assert_eq!(s.left, Some(22.5));
        assert_eq!(s.right, None, "außerhalb 12–45 % wird übergangen");
        assert_eq!(s.accordion, "personas");
        let revisions = Settings::from_cookie_header(Some("baruch_accordion=revisions"));
        assert_eq!(revisions.accordion, "revisions", "das Akkordeon „Fassungen“ bleibt nach dem Neuladen offen");
        assert_eq!(s.closed, vec![1, 6]);
        assert_eq!(s.style(), "--left:22.5%;");

        let bad = Settings::from_cookie_header(Some("baruch_theme=\"><script>; baruch_accordion=evil"));
        assert_eq!(bad.theme, None);
        assert_eq!(bad.accordion, "chat");
        assert_eq!(bad.editor, "satz", "ohne Cookie baruch_editor ist das Satzbild die Vorgabe");

        let source = Settings::from_cookie_header(Some("baruch_editor=quelltext"));
        assert_eq!(source.editor, "quelltext", "nur die ausdrückliche Wahl „quelltext“ öffnet den Quelltext");
        for other in ["satz", "x\"><b>", ""] {
            let s = Settings::from_cookie_header(Some(&format!("baruch_editor={other}")));
            assert_eq!(s.editor, "satz", "{other:?} gilt nicht als Quelltext");
        }
        assert_eq!(Settings::default().editor, "satz", "auch die Vorgabe ohne Kopf ist das Satzbild");
    }

    #[test]
    fn proposals_start_open_unless_the_pv_cookie_says_closed() {
        let open = Settings::from_cookie_header(Some("baruch_folded=12-3"));
        assert_eq!(open.pv, "open", "ohne baruch_pv stehen Vorschläge offen (Vorgabe wie bisher)");
        assert_eq!(open.folded, vec![(12, 3)], "ohne baruch_pv kein Merker FOLD_ALL");
        assert_eq!(Settings::default().pv, "open", "Vorgabe ohne Kopf: offen");
        let closed = Settings::from_cookie_header(Some("baruch_pv=closed; baruch_folded=12-3"));
        assert_eq!(closed.pv, "closed", "baruch_pv=closed wird gelesen");
        assert_eq!(closed.folded, vec![(12, 3), FOLD_ALL], "der Merker steht hinter den echten Einträgen");
        for other in ["open", "zu", "\"><b>", ""] {
            let s = Settings::from_cookie_header(Some(&format!("baruch_pv={other}")));
            assert_eq!((s.pv, s.folded.len()), ("open", 0), "{other:?} gilt nicht als eingeklappt");
        }
    }

    #[test]
    fn folded_proposals_come_from_a_clean_cookie_only() {
        let s = Settings::from_cookie_header(Some("baruch_folded=12-3.12-3.7-1.-2.0-4.5-0.9.3--4; baruch_theme=dark"));
        assert_eq!(s.folded, vec![(12, 3), (7, 1)], "doppelte, leere Nummern und kaputte Einträge übergangen");
        let bad = Settings::from_cookie_header(Some("baruch_folded=12-3.\"><b>"));
        assert!(bad.folded.is_empty(), "ein Wert mit fremden Zeichen gilt nicht: {:?}", bad.folded);
        let many: Vec<String> = (1..=250).map(|k| format!("{k}-1")).collect();
        let long = Settings::from_cookie_header(Some(&format!("baruch_folded={}", many.join("."))));
        assert_eq!(long.folded.len(), MAX_FOLDED, "höchstens {MAX_FOLDED} Einträge");
        assert_eq!(long.folded[0], (1, 1), "die ersten zählen");
    }

    #[test]
    fn urls_carry_work_key_and_the_encoded_search() {
        let at = Resolved {
            node: 2,
            key: 31,
            index: 0,
            rev: 1,
            preview: true,
            pick: vec![],
            missing_key: None,
            saved: None,
            show_all: false,
        };
        assert_eq!(
            at.url(7, Some(" langmütig & mehr ")),
            "/?work=7&key=31&rev=1&view=preview&lq=langm%C3%BCtig%20%26%20mehr",
            "Reihenfolge work, key, rev, view, lq; kein node= mehr (Runde 8)"
        );
        assert_eq!(at.url(1, Some("  ")), "/?work=1&key=31&rev=1&view=preview", "leere Suche fällt weg");
        let picked = Resolved { pick: vec![(2, 3), (4, 1)], ..at };
        assert_eq!(picked.url(1, None), "/?work=1&key=31&rev=1&view=preview&pick=2:3,4:1", "pick nach view");
        assert_eq!(node_url(3, 9, None, false, "", ""), "/?work=3&key=9", "ohne rev nur Werk und Schlüssel");
    }

    /// Beispielwerk mit festen Schlüsseln (im Speicher sind sie 0): Schlüssel = 100 + Stelle.
    fn keyed_sample() -> Work {
        let mut work = baruch_core::sample::work();
        for node in &mut work.nodes {
            node.key = 100 + node.id as i64;
        }
        work
    }

    #[test]
    fn the_address_names_the_revision_number_not_its_position() {
        let mut work = keyed_sample();
        work.nodes[2].revisions.remove(0); // Lücke: Fassungen 2 und 3
        let at = Location { node: Some(2), rev: Some(3), ..Location::default() }.resolve(&work);
        assert_eq!((at.index, at.rev), (1, 3), "Nummer 3 steht an Stelle 1");
        assert_eq!(at.url(1, None), "/?work=1&key=102&rev=3", "Adresse trägt Schlüssel und Nummer");
        let gone = Location { node: Some(2), rev: Some(1), ..Location::default() }.resolve(&work);
        assert_eq!(gone.rev, 2, "eine gelöschte Nummer führt auf die aktuelle Fassung");
    }

    #[test]
    fn the_key_goes_before_the_position_and_an_unknown_key_falls_back() {
        let work = keyed_sample();
        let at = Location { key: Some(104), node: Some(1), ..Location::default() }.resolve(&work);
        assert_eq!((at.node, at.key, at.missing_key), (4, 104, None), "der Schlüssel gewinnt gegen node=1");
        let gone = Location { key: Some(999), node: Some(5), ..Location::default() }.resolve(&work);
        assert_eq!((gone.node, gone.missing_key), (5, Some(999)), "unbekannter Schlüssel → node, mit Hinweis");
        let fallback = Location { key: Some(999), ..Location::default() }.resolve(&work);
        assert_eq!(fallback.node, default_node(&work), "ohne node der Vorgabe-Abschnitt");
        assert_eq!(fallback.missing_key, Some(999), "auch dann mit Hinweis");
        let old = Location { node: Some(3), ..Location::default() }.resolve(&work);
        assert_eq!((old.node, old.key, old.missing_key), (3, 103, None), "alte Adressen mit node= gehen weiter");
    }

    #[test]
    fn move_targets_list_every_other_section_with_number_and_title() {
        let work = keyed_sample();
        let targets = post_targets(&work, 2);
        assert_eq!(targets.len(), work.nodes.len() - 1, "alle außer dem gezeigten");
        let keys: Vec<i64> = targets.iter().map(|t| t.get_attr("key").unwrap().as_i64().unwrap()).collect();
        assert!(!keys.contains(&102), "der gezeigte Abschnitt fehlt: {keys:?}");
        let label = targets[3].get_attr("label").unwrap().to_string();
        assert!(label.contains(&work.nodes[4].title), "Ziel nennt den Titel: {label}");
    }

    #[test]
    fn pick_keeps_only_valid_entries_one_per_section() {
        let work = baruch_core::sample::work();
        let pick = parse_pick(&work, Some("2:1, x:1,4:9,99:1,4:1,2:3,;:"));
        assert_eq!(pick, vec![(2, 3), (4, 1)], "ungültige Einträge übergangen, je Abschnitt der letzte");
        assert_eq!(pick_param(&pick), "&pick=2:3,4:1");
        assert_eq!(pick_param(&[]), "", "ohne Auswahl kein Parameter");
    }

    #[test]
    fn the_tree_indents_children_under_their_parent() {
        let info = |number, based_on| RevisionInfo {
            number,
            based_on,
            status: RevisionStatus::Superseded,
            author: Author::Human { name: "Autor".into() },
            note: String::new(),
            created: "2026-11-05T10:11:12.123Z".into(),
            edited: None,
            variant: None,
            amend_seq: 0,
            based_on_amend: None,
        };
        let infos = [info(1, None), info(2, Some(1)), info(4, Some(1)), info(5, Some(2)), info(6, None)];
        let order: Vec<(usize, u32)> = ordered_tree(&infos).into_iter().map(|(d, r)| (d, r.number)).collect();
        assert_eq!(order, vec![(0, 1), (1, 2), (2, 5), (1, 4), (0, 6)], "Tiefensuche, Kinder nach Nummer");
        assert_eq!(
            short_time("2026-11-05T10:11:12.123Z"),
            crate::localtime::minutes("2026-11-05T10:11:12.123Z"),
            "Fassungen zeigen ihre Zeit in Ortszeit (Umrechnung selbst: localtime::tests)"
        );
    }

    #[test]
    fn closed_chapter_stays_open_when_it_holds_the_selection() {
        let work = baruch_core::sample::work();
        // Teil I (1) mit Kapitel 2 (3) und Abschnitt 2.1 (4); gewählt ist 2.1.
        let items = tree(&work, None, 4, &[1, 3, 6]);
        let part1 = items.iter().find(|i| i.id == 1).unwrap();
        assert!(part1.open);
        assert!(part1.children.iter().find(|i| i.id == 3).unwrap().open);
        assert!(!items.iter().find(|i| i.id == 6).unwrap().open);
    }
}
