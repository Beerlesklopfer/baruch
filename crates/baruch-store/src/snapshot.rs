// SPDX-License-Identifier: AGPL-3.0-or-later
//! Typst-Spiegel (Vertrag B1, Abschnitt 6): `abschnitte/{seq:03}-{slug(title)}.typ` je Knoten mit
//! aktueller Revision und `main.typ` mit `#include` in Baumreihenfolge (Tiefensuche, Geschwister
//! nach position).
//!
//! Hat das Werk eine Werkvorlage (Import, Entscheidung 2026-11-05):
//! - `vorlage.typ` enthält die Vorlage;
//! - `main.typ` beginnt mit der Vorlage (Set- und Show-Regeln gelten so für alles Eingebundene) und
//!   bindet danach die Abschnitte ein;
//! - jede Abschnittsdatei beginnt mit der Zeile [`TEMPLATE_IMPORT`], damit `#let`-Funktionen der
//!   Vorlage (z. B. `#szene`) auch dort gelten.
//!
//! Gliederungsschema (Vertrag Runde 6, Abschnitt 2d): Ist ein anderes Schema als „aus der
//! Werkvorlage“ gewählt, folgt der Vorlage in `main.typ` und im Export eine Kommentarzeile und eine
//! `#set heading(numbering: …)`-Regel ([`baruch_typst::scheme::export_template`]); ohne Vorlage steht
//! sie hinter `// Baruch: <Titel>`. `vorlage.typ` bleibt die gespeicherte Vorlage.
//!
//! Nummer an/aus (Vertrag Runde 14, Abschnitt 4, [`project`]): Ist die wirksame Nummerierung nicht
//! „aus“, stellen Export, Export-Karte, Spiegel und Satz die erste Überschriftszeile jedes Knotens
//! (außer Teilen), dessen `numbered` nicht zur Textform passt, mit
//! [`baruch_typst::set_numbered_line`] um: `numbered = 0` → Markierung „ohne Nummer“
//! (`#heading(level: N, numbering: none)[…] <baruch-unnumbered>`), `numbered = 1` → `=`-Zeile. Nur
//! eine Überschrift mit eigenem Label lässt sich nicht markieren; für sie bleibt hinter der Vorlage
//! (auch bei „aus der Werkvorlage“) die Show-Regel `numbering: none` von Runde 6 (`fallback_rules`).
//! Ein frisch importiertes Werk (Import setzt `numbered` aus der Textform) bleibt byte-gleich.
//!
//! Sprungmarken (Runde 30a, Vertrag §3.3): Export, Export-Karte, Spiegel und Satz tragen an der ersten
//! Überschrift jedes Abschnitts die feste Sprungmarke `<k{key}>` ([`baruch_typst::xref::label_section`]: ` <kN>` am
//! Zeilenende bzw. bei eigenem Label die Ankerzeile `#metadata("baruch-key") <kN>` darunter). Ein Import entfernt sie
//! wieder (`import.rs`). Ein frisch importiertes Werk ergibt darum den Export „bis auf die Sprungmarken“ byte-gleich
//! (`baruch_typst::xref::strip_key_labels`).
//!
//! Register und Glossar (Runde 31, Vertrag §2.6): Export, Export-Karte und Spiegel lassen jeden Abschnitt nach der
//! Sprungmarke durch [`baruch_typst::index::Projector::project`] laufen (Bibelstellen-Marken, erste Erwähnung) und hängen
//! das Buchende ([`baruch_typst::index::back_matter`]: Registerseiten, Glossarseite) hinter den letzten Abschnitt bzw.
//! hinter die `#include`-Zeilen von `main.typ`; der Spiegel bekommt dazu `glossar.md`. Ohne R31-Einstellungen
//! ([`baruch_typst::index::BackMatter::is_active`]) ist das die Identität und das Buchende leer – byte-gleich.
//!
//! Show-Regel über die Sprungmarke (Runde 22, R21b-Rest): Trägt ein Knoten mit `numbered = 0`, dessen Überschrift sich
//! nicht markieren lässt (etwa ein Titel mit Code wie `#emph[…]`), dieselbe Überschrift wie ein nummerierter Knoten,
//! träfe die Show-Regel über Ebene und Titel beide. Dann zielt die Regel auf seine Sprungmarke:
//! `#show <kN>: set heading(numbering: none)` unter `// Baruch: Überschriften ohne Nummer über die Sprungmarke`
//! ([`label_rules`], vor dem Block des Rückfalls, damit `import::split_fallback` diesen weiter am Ende findet). Das geht
//! nur, wenn die Sprungmarke an der Überschrift selbst steht (kein eigenes Label); sonst bleibt es bei der Grenze von
//! Runde 14. Ein Import liest den Block mit [`split_label_fallback`] zurück (`import.rs`).
//!
//! Bilder im Spiegel (Runde 22, „Spiegel setzen“): Die Abschnittsdateien liegen in `abschnitte/`; Typst löst einen
//! relativen Pfad in `image("bilder/x.svg")` gegen den Ordner der Datei auf, die ihn nennt. Darum schreibt der Spiegel
//! relative Bildpfade in Abschnittsdateien als Pfade ab der Wurzel (`image("/bilder/x.svg")`, [`root_image_paths`]);
//! so lässt sich `main.typ` samt `abschnitte/` mit Typst setzen. Rasterbilder liegen im Spiegel als Base64-Text
//! (`bilder/<name>.b64`, `assets::mirror_files`) – sie muss man vor dem Setzen zurückwandeln (dokumentierte Abweichung).
//!
//! Der Spiegel ist eine Projektion: Die Revisionen in SQLite bleiben ohne die Import-Zeile und ohne
//! die Umstellung der Überschrift, und [`export`] setzt die Datei ohne die Import-Zeile zusammen. Ohne
//! Werkvorlage (Beispielwerk) gibt es weder `vorlage.typ` noch die Import-Zeile.

use std::borrow::Cow;
use std::collections::HashMap;

use sqlx::SqliteConnection;

use baruch_typst::scheme::{Numbering, Scheme, export_template};

use crate::{MirrorFile, StoreError, WorkId, broken, load, slug, to_usize};

/// Aktuelle Fassung eines Abschnitts in Baumreihenfolge ([`ordered_currents`]).
pub(crate) struct Current {
    pub(crate) seq: usize,
    /// Fester Schlüssel (`node.id`).
    pub(crate) key: i64,
    pub(crate) title: String,
    pub(crate) body: String,
}

/// Aktuelle Revisionen in Baumreihenfolge. Knoten ohne aktuelle Revision fehlen. Unbekanntes Werk
/// → `UnknownWork`.
pub(crate) async fn ordered_currents(conn: &mut SqliteConnection, work: WorkId) -> Result<Vec<Current>, StoreError> {
    let exists: Option<i64> =
        sqlx::query_scalar("SELECT id FROM work WHERE id = ?").bind(work).fetch_optional(&mut *conn).await?;
    if exists.is_none() {
        return Err(StoreError::UnknownWork(work));
    }

    let rows: Vec<(i64, Option<i64>, i64, String)> =
        sqlx::query_as("SELECT seq, parent_seq, position, title FROM node WHERE work_id = ? ORDER BY seq")
            .bind(work)
            .fetch_all(&mut *conn)
            .await?;
    let currents: Vec<(i64, i64, String)> = sqlx::query_as(
        "SELECT n.seq, n.id, r.body FROM revision r JOIN node n ON n.id = r.node_id \
         WHERE n.work_id = ? AND r.status = 'current' ORDER BY n.seq",
    )
    .bind(work)
    .fetch_all(&mut *conn)
    .await?;
    let mut bodies: HashMap<usize, (i64, String)> = HashMap::new();
    for (seq, key, body) in currents {
        if bodies.insert(to_usize(seq, "seq")?, (key, body)).is_some() {
            return Err(broken(format!("Abschnitt {seq}: mehr als eine aktuelle Revision")));
        }
    }

    // Kinder je Elternteil, nach (position, seq).
    let mut children: HashMap<Option<usize>, Vec<(i64, usize)>> = HashMap::new();
    let mut titles: HashMap<usize, String> = HashMap::new();
    for (seq, parent, position, node_title) in rows {
        let seq = to_usize(seq, "seq")?;
        let parent = parent.map(|p| to_usize(p, "parent_seq")).transpose()?;
        children.entry(parent).or_default().push((position, seq));
        titles.insert(seq, node_title);
    }
    for list in children.values_mut() {
        list.sort_unstable();
    }

    let mut order = Vec::with_capacity(titles.len());
    let mut stack: Vec<usize> =
        children.get(&None).map(|c| c.iter().rev().map(|&(_, s)| s).collect()).unwrap_or_default();
    let mut seen = std::collections::HashSet::new();
    while let Some(seq) = stack.pop() {
        if !seen.insert(seq) {
            continue; // Schleife in parent_seq: nicht zweimal aufnehmen
        }
        order.push(seq);
        if let Some(list) = children.get(&Some(seq)) {
            stack.extend(list.iter().rev().map(|&(_, s)| s));
        }
    }
    // Knoten, deren Elternteil fehlt, gehen nicht verloren: hinten, nach seq.
    let mut rest: Vec<usize> = titles.keys().copied().filter(|s| !seen.contains(s)).collect();
    rest.sort_unstable();
    order.extend(rest);

    let mut out = Vec::with_capacity(bodies.len());
    for seq in order {
        let Some((key, body)) = bodies.remove(&seq) else { continue };
        let title = titles.remove(&seq).unwrap_or_default();
        out.push(Current { seq, key, title, body });
    }
    Ok(out)
}

/// Kopf des Werks: gespeicherte Vorlage, Titel, die Vorlage für Export und `main.typ` (mit der Regel
/// des Gliederungsschemas, bei „aus der Werkvorlage“ unverändert, dahinter die Show-Regeln des
/// Rückfalls) und die umgestellten Texte je Schlüssel ([`project`]).
struct Head {
    template: String,
    title: String,
    exported: String,
    bodies: HashMap<i64, String>,
}

/// Werkzeile für Kopf und Projektion: Vorlage, Titel, eigenes Muster, Schema-id.
type WorkHead = (String, String, Option<String>, String);

async fn work_head(conn: &mut SqliteConnection, work: WorkId) -> Result<WorkHead, StoreError> {
    sqlx::query_as("SELECT template, title, numbering, numbering_scheme FROM work WHERE id = ?")
        .bind(work)
        .fetch_optional(&mut *conn)
        .await?
        .ok_or(StoreError::UnknownWork(work))
}

async fn head(conn: &mut SqliteConnection, work: WorkId) -> Result<Head, StoreError> {
    let (template, title, numbering, scheme) = work_head(conn, work).await?;
    let numbering = load::work_numbering(&scheme, numbering.as_deref(), &template);
    let scheme = Scheme::from_id(&scheme).unwrap_or(Scheme::Template);
    let mut exported = export_template(&template, scheme, &numbering);
    let projection = project(&heading_rows(conn, work).await?, &numbering);
    // Runde 22: Show-Regeln über die Sprungmarke (vor dem Block des Rückfalls, siehe Modulkopf).
    let labelled = label_rules(&projection.by_label);
    if !labelled.is_empty() {
        if !exported.is_empty() && !exported.ends_with('\n') {
            exported.push('\n');
        }
        exported.push_str(&labelled);
    }
    let rules = fallback_rules(&projection.fallback);
    if !rules.is_empty() {
        if !exported.is_empty() && !exported.ends_with('\n') {
            exported.push('\n');
        }
        exported.push_str(&rules);
    }
    // Runde 29: Satz-Einstellungen als Typst-Regeln hinter Werkvorlage, Schema- und Rückfall-Regeln – der eine Ort für
    // Export, Export-Karte, Spiegel (main.typ), Satz-Vorschau, PDF und Umschlag. Ohne Einstellungen "" (byte-gleich).
    let typeset = crate::typeset::typeset_in(conn, work).await?;
    let satz = baruch_typst::typeset::rules(&typeset);
    if !satz.is_empty() {
        if !exported.is_empty() && !exported.ends_with('\n') {
            exported.push('\n');
        }
        exported.push_str(&satz);
    }
    Ok(Head { template, title, exported, bodies: projection.bodies })
}

/// Show-Regeln (Rückfall seit Runde 14) für Überschriften der Knoten mit `numbered = 0`, die sich
/// nicht als „ohne Nummer“ markieren lassen (eigenes Label): Baruch zählt sie nicht
/// ([`load::numbers`]), Typst würde sie sonst mitnummerieren und alle folgenden Nummern verschieben.
/// Eine Überschrift mit `numbering: none` zählt in Typst nicht mit (auch nicht in
/// `counter(heading).final()`, das „Predigt“ für „Schluss“ liest). Erkannt wird sie an Ebene und Titel
/// (`heading.where(level: …, body: […])`, Titel wörtlich aus der Überschriftszeile, ohne Label und
/// Kommentar). Ohne solche Knoten: leer.
fn fallback_rules(keys: &[(u8, String)]) -> String {
    let rules: String = keys
        .iter()
        .map(|(level, title)| {
            format!("#show heading.where(level: {level}, body: [{title}]): set heading(numbering: none)\n")
        })
        .collect();
    if rules.is_empty() {
        return rules;
    }
    format!("// Baruch: Überschriften ohne Nummer\n{rules}")
}

/// Kennzeile der Show-Regeln über die Sprungmarke (Runde 22).
pub(crate) const LABEL_FALLBACK_MARK: &str = "// Baruch: Überschriften ohne Nummer über die Sprungmarke";

/// Show-Regeln über die Sprungmarke `<kN>` (Runde 22) für Knoten mit `numbered = 0`, deren Überschrift dieselbe ist wie
/// die eines nummerierten Knotens (die Regel über Ebene und Titel träfe beide). Ohne solche Knoten: leer.
fn label_rules(keys: &[i64]) -> String {
    if keys.is_empty() {
        return String::new();
    }
    let rules: String = keys.iter().map(|key| format!("#show <k{key}>: set heading(numbering: none)\n")).collect();
    format!("{LABEL_FALLBACK_MARK}\n{rules}")
}

/// Gegenstück zu [`label_rules`] beim Import (Runde 22): Steht der Block [`LABEL_FALLBACK_MARK`] in der Vorlage eines
/// Exports und besteht er nur aus Regeln `#show <kN>: set heading(numbering: none)`, kommt er aus der Vorlage heraus;
/// zurück kommen die alten Schlüssel `N` (ihre Überschriften sind „ohne Nummer“). Steht der Block des Rückfalls
/// dahinter, bleibt dieser stehen. Eine Zeile anderer Form → Vorlage unverändert, keine Schlüssel.
pub(crate) fn split_label_fallback(template: &str) -> (String, Vec<i64>) {
    let Some(at) = template.find(&format!("{LABEL_FALLBACK_MARK}\n")) else { return (template.to_owned(), Vec::new()) };
    if at > 0 && !template[..at].ends_with('\n') {
        return (template.to_owned(), Vec::new());
    }
    let block_start = at + LABEL_FALLBACK_MARK.len() + 1;
    let mut keys = Vec::new();
    let mut end = block_start;
    for line in template[block_start..].split_inclusive('\n') {
        let Some(key) = line
            .trim_end_matches('\n')
            .strip_prefix("#show <k")
            .and_then(|rest| rest.strip_suffix(">: set heading(numbering: none)"))
            .and_then(|key| key.parse::<i64>().ok())
        else {
            break;
        };
        keys.push(key);
        end += line.len();
    }
    if keys.is_empty() {
        return (template.to_owned(), Vec::new());
    }
    (format!("{}{}", &template[..at], &template[end..]), keys)
}

/// Relative Bildpfade einer Abschnittsdatei des Spiegels ab der Wurzel (Runde 22, siehe Modulkopf): `image("p")` mit
/// einem Pfad ohne führendes `/`, ohne `..` am Anfang, ohne Schema und ohne `"`/`\\` wird `image("/p")`. Andere
/// Aufrufe (etwa über eine Funktion der Werkvorlage) bleiben, wie sie sind.
pub(crate) fn root_image_paths(body: &str) -> Cow<'_, str> {
    let mut out = Cow::Borrowed(body);
    for path in baruch_typst::images::image_paths(body) {
        if path.is_empty()
            || path.starts_with('/')
            || path.starts_with("..")
            || path.contains("://")
            || path.contains(['"', '\\'])
        {
            continue;
        }
        let from = format!("image(\"{path}\"");
        if out.contains(&from) {
            out = Cow::Owned(out.replace(&from, &format!("image(\"/{path}\"")));
        }
    }
    out
}

/// Je Knoten außer Teilen (Baumreihenfolge, nur mit aktueller Fassung): Schlüssel, `numbered` und
/// der Text der aktuellen Fassung.
type HeadingRow = (i64, bool, String);

async fn heading_rows(conn: &mut SqliteConnection, work: WorkId) -> Result<Vec<HeadingRow>, StoreError> {
    let rows: Vec<(i64, i64, String)> = sqlx::query_as(
        "SELECT n.id, n.numbered, r.body FROM node n JOIN revision r ON r.node_id = n.id AND r.status = 'current' \
         WHERE n.work_id = ? AND n.kind <> 'part' ORDER BY n.seq",
    )
    .bind(work)
    .fetch_all(&mut *conn)
    .await?;
    Ok(rows.into_iter().map(|(key, flag, body)| (key, flag != 0, body)).collect())
}

/// Ergebnis von [`project`].
struct Projection {
    /// Umgestellte Texte je Schlüssel (nur die geänderten).
    bodies: HashMap<i64, String>,
    /// Überschriften `(Ebene, Titel wörtlich)` für die Show-Regeln des Rückfalls ([`fallback_rules`]).
    fallback: Vec<(u8, String)>,
    /// Runde 22: Schlüssel der Knoten mit Show-Regel über ihre Sprungmarke ([`label_rules`]).
    by_label: Vec<i64>,
    /// Schlüssel der Knoten, deren erste Überschrift (Ebene 1) Typst nach der Projektion zählt
    /// (nummerierte Textform, keine Show-Regel).
    counted: Vec<i64>,
}

/// Projektion „Nummer an/aus“ (Vertrag Runde 14, Abschnitt 4): Ist `numbering` nicht aus, wird die erste
/// Überschriftszeile jedes Knotens, dessen `numbered` nicht zur Textform passt, mit [`baruch_typst::set_numbered_line`]
/// umgestellt. Lässt sie sich für
/// `numbered = 0` nicht markieren (eigenes Label, Überschrift nicht am Anfang), bekommt sie eine Show-Regel – außer ein
/// Knoten mit Nummer trägt dieselbe Überschrift (die Regel träfe beide) oder der Titel ist leer.
///
/// Ist die Nummerierung aus („Ohne Nummern“ bzw. `numbering: none`), bleibt die Textform (Text wie gespeichert); seit
/// Runde 21b bekommen Knoten mit `numbered = 0` dann aber die Show-Regel des Rückfalls. Im Satz ändert sie nichts (es
/// gibt ohnehin keine Nummern), doch der Merker „Nummer aus“ übersteht so Export → Import (`import::split_fallback`);
/// vorher kam ein solcher Abschnitt als nummeriert zurück.
fn project(rows: &[HeadingRow], numbering: &Numbering) -> Projection {
    // Je Zeile: Schlüssel, `numbered`, erste Überschrift (Ebene, Titel wörtlich) und ob Typst sie nach
    // der Projektion nummeriert.
    type Shown = (i64, bool, Option<(u8, String)>, bool);
    let mut shown: Vec<Shown> = Vec::with_capacity(rows.len());
    let mut bodies = HashMap::new();
    // Runde 22: Steht die Sprungmarke an der Überschrift selbst (` <kN>` am Ende der Zeile)?
    let mut on_heading: std::collections::HashSet<i64> = std::collections::HashSet::new();
    for (key, flag, body) in rows {
        let marker = format!("<k{key}>");
        if baruch_typst::xref::label_section(body, *key).lines().next().is_some_and(|line| line.contains(&marker)) {
            on_heading.insert(*key);
        }
        let heading = baruch_typst::parse(body)
            .ok()
            .and_then(|doc| doc.headings.into_iter().next())
            .filter(|h| (1..=3).contains(&h.level));
        let Some(heading) = heading else {
            shown.push((*key, *flag, None, false));
            continue;
        };
        let mut form = heading.numbered;
        if !numbering.is_off()
            && form != *flag
            && let Some(projected) = baruch_typst::set_numbered_line(body, *flag)
        {
            bodies.insert(*key, projected);
            form = *flag;
        }
        shown.push((*key, *flag, Some((heading.level, heading.raw_title)), form));
    }
    // Rückfall: nummerierte Textform, aber `numbered = 0` (seit Runde 21b auch bei ausgeschalteter Nummerierung).
    let mut fallback: Vec<(u8, String)> = Vec::new();
    let mut taken = std::collections::HashSet::new();
    for (_, flag, heading, form) in &shown {
        let Some(heading) = heading else { continue };
        if *form && *flag {
            taken.insert(heading.clone());
        } else if *form && !fallback.contains(heading) {
            fallback.push(heading.clone());
        }
    }
    // Runde 22: Die Regel über Ebene und Titel träfe auch einen nummerierten Knoten gleicher Überschrift (oder der Titel
    // ist leer) → Regel über die Sprungmarke, wenn sie an der Überschrift steht.
    let by_label: Vec<i64> = shown
        .iter()
        .filter(|(key, flag, heading, form)| {
            *form
                && !*flag
                && on_heading.contains(key)
                && heading.as_ref().is_some_and(|h| taken.contains(h) || h.1.is_empty())
        })
        .map(|row| row.0)
        .collect();
    fallback.retain(|key| !taken.contains(key) && !key.1.is_empty());
    let counted = shown
        .iter()
        .filter(|(key, _, heading, form)| {
            *form && !by_label.contains(key) && heading.as_ref().is_some_and(|h| h.0 == 1 && !fallback.contains(h))
        })
        .map(|row| row.0)
        .collect();
    Projection { bodies, fallback, by_label, counted }
}

/// Schlüssel der Knoten (Baumreihenfolge), deren Überschrift Typst in `counter(heading).get().first()`
/// zählt: Überschriften der ersten Ebene, die nach der Projektion ([`project`]) nummeriert sind
/// (Typst zählt nur nummerierte Überschriften; Markierungen „ohne Nummer“ und Überschriften mit
/// Show-Regel zählen nicht). Bei „aus der Werkvorlage“ ohne Nummern in der Vorlage (`none` oder ohne
/// jede Regel) keine; ist ihre Regel nicht sicher lesbar, `None` (nicht raten). Bei „Ohne Nummern“
/// keine.
pub(crate) async fn counted_headings(
    conn: &mut SqliteConnection,
    work: WorkId,
) -> Result<Option<Vec<i64>>, StoreError> {
    let (template, _, custom, scheme) = work_head(conn, work).await?;
    let numbering = load::work_numbering(&scheme, custom.as_deref(), &template);
    let counted = project(&heading_rows(conn, work).await?, &numbering).counted;
    Ok(match Scheme::from_id(&scheme).unwrap_or(Scheme::Template) {
        Scheme::Template => match baruch_typst::heading_numbering(&template) {
            Some(pattern) if pattern.is_empty() => Some(Vec::new()),
            Some(_) => Some(counted),
            // Keine lesbare Regel: ohne jede Erwähnung nummeriert die Vorlage nicht, sonst unklar.
            None if template.contains("numbering") => None,
            None => Some(Vec::new()),
        },
        Scheme::None => Some(Vec::new()),
        // Ein eigenes Muster ohne Zählzeichen-Ebenen (leer) nummeriert nichts.
        _ if numbering.is_off() => Some(Vec::new()),
        _ => Some(counted),
    })
}

/// Name der Vorlagendatei im Spiegel (nur bei Werken mit Werkvorlage).
pub(crate) const TEMPLATE_FILE: &str = "vorlage.typ";

/// Erste Zeile jeder Abschnittsdatei eines Werks mit Werkvorlage (Abschnitte liegen in `abschnitte/`).
pub(crate) const TEMPLATE_IMPORT: &str = "#import \"../vorlage.typ\": *\n";

/// Dateien des Spiegels: `main.typ`, bei Werkvorlage `vorlage.typ`, dann die Abschnitte. Mit
/// Werkvorlage beginnt `main.typ` mit ihr, sonst mit `// Baruch: <Titel>`.
pub(crate) async fn files(conn: &mut SqliteConnection, work: WorkId) -> Result<Vec<MirrorFile>, StoreError> {
    let Head { template, title, exported, mut bodies } = head(conn, work).await?;
    let has_template = !template.is_empty();
    let mut main = if has_template { exported } else { format!("// Baruch: {title}\n{exported}\n") };
    let currents = ordered_currents(conn, work).await?;
    if !currents.is_empty() {
        end_line(&mut main);
    }
    let back = back_matter_in(conn, work).await?;
    let mut projector = baruch_typst::index::Projector::new(&back);
    let mut projected = Vec::with_capacity(currents.len());
    let mut sections = Vec::new();
    for Current { seq, key, title: node_title, body } in currents {
        let body = bodies.remove(&key).unwrap_or(body);
        // Sprungmarke `<kN>` (Runde 30a, [`baruch_typst::xref::label_section`]): nur in der Projektion.
        let body = baruch_typst::xref::label_section(&body, key).into_owned();
        // Register und Glossar (Runde 31): nur in der Projektion.
        let body = projector.project(&body).into_owned();
        let path = format!("abschnitte/{seq:03}-{}.typ", slug(&node_title));
        main.push_str(&format!("#include \"{path}\"\n"));
        projected.push(body.clone());
        // Runde 22: relative Bildpfade ab der Wurzel, damit Typst sie aus `abschnitte/` findet.
        let body = root_image_paths(&body).into_owned();
        let contents = if has_template { format!("{TEMPLATE_IMPORT}{body}") } else { body };
        sections.push(MirrorFile { path, contents });
    }
    // Buchende (Runde 31) hinter den `#include`-Zeilen; inaktiv "".
    let tail = baruch_typst::index::back_matter(&back, &projected.iter().map(String::as_str).collect::<Vec<_>>());
    append_tail(&mut main, &tail);
    let mut files = vec![MirrorFile { path: "main.typ".into(), contents: main }];
    if has_template {
        files.push(MirrorFile { path: TEMPLATE_FILE.into(), contents: template });
    }
    files.extend(sections);
    files.extend(crate::assets::mirror_files(conn, work).await?);
    files.extend(crate::ideas::mirror_files(conn, work).await?);
    files.extend(crate::findings::mirror_files(conn, work).await?);
    files.extend(crate::glossary::mirror_files(conn, work).await?);
    Ok(files)
}

/// Das Werk als eine Typst-Datei: Werkvorlage und aktuelle Texte in Baumreihenfolge. Endet ein Text
/// (außer dem letzten) nicht mit einem Zeilenende, kommt eins dazu, damit die nächste Überschrift am
/// Zeilenanfang steht. Ein frisch importiertes Werk ergibt so Byte für Byte die Eingabe.
pub(crate) async fn export(conn: &mut SqliteConnection, work: WorkId) -> Result<String, StoreError> {
    let (template, sections, tail) = export_parts(conn, work).await?;
    let mut out = baruch_typst::join(&template, sections.iter().map(|s| s.body.as_str()));
    append_tail(&mut out, &tail);
    Ok(out)
}

/// Hängt das Buchende (Runde 31, [`baruch_typst::index::back_matter`]) an: leer → nichts (byte-gleich), sonst erst ein
/// Zeilenende, falls der Text davor (nicht leer) keins hat, damit `// Baruch: Register und Glossar` eine eigene Zeile
/// ist. Export und Export-Karte nehmen beide diese Funktion (Invariante: Karte byte-gleich zum Export).
pub(crate) fn append_tail(out: &mut String, tail: &str) {
    if tail.is_empty() {
        return;
    }
    if !out.is_empty() && !out.ends_with(baruch_typst::is_newline) {
        out.push('\n');
    }
    out.push_str(tail);
}

/// Was Projektion und Buchende eines Werks brauchen (Runde 31, Vertrag §2.6): Satz-Einstellungen, eingebaute und eigene
/// Register, angenommene Glossarbegriffe.
pub(crate) async fn back_matter_in(
    conn: &mut SqliteConnection,
    work: WorkId,
) -> Result<baruch_typst::index::BackMatter, StoreError> {
    let settings = crate::typeset::typeset_in(conn, work).await?;
    let mut registers = baruch_typst::index::builtin_registers();
    registers.extend(crate::glossary::registers_in(conn, work).await?);
    let glossary = crate::glossary::accepted_entries_in(conn, work).await?;
    Ok(baruch_typst::index::BackMatter { settings, registers, glossary })
}

/// Eine nicht leere Vorlage, auf die noch etwas folgt, endet auf ein Zeilenende: sonst hinge die erste
/// Überschrift (bzw. `#include`) an ihrer letzten Zeile – `= Titel` würde Fließtext, und ein Fehler in
/// der letzten Vorlagenzeile gälte in der Vorschau als Fehler des ersten Abschnitts. Die gespeicherte
/// Vorlage (und `vorlage.typ`) bleibt, wie der Mensch sie geschrieben hat. Ein bloßer Byte-Order-Mark
/// (Import einer Datei, die mit BOM und Überschrift beginnt) ist keine Zeile und bleibt ohne.
fn end_line(template: &mut String) {
    let content = template.strip_prefix('\u{feff}').unwrap_or(template);
    if !content.is_empty() && !content.ends_with(baruch_typst::is_newline) {
        template.push('\n');
    }
}

/// Teile von [`export`]: die exportierte Vorlage (folgt ein Text, mit Zeilenende am Schluss, siehe
/// [`end_line`]) und die Abschnitte in Baumreihenfolge, deren Texte
/// schon mit dem ergänzten Zeilenende. [`export`] verkettet sie; `export_map` (Typst-Vorschau,
/// Runde 8) verkettet sie mit Bytebereichen ([`baruch_typst::join_with_offsets`]) – beide also aus
/// derselben Quelle, damit die Karte byte-gleich zum Export bleibt. Drittes Teil (Runde 31): das Buchende (Register-
/// und Glossarseiten, inaktiv ""), das beide mit [`append_tail`] hinter den letzten Abschnitt hängen.
pub(crate) async fn export_parts(
    conn: &mut SqliteConnection,
    work: WorkId,
) -> Result<(String, Vec<Current>, String), StoreError> {
    let Head { exported: mut template, mut bodies, .. } = head(conn, work).await?;
    let mut sections = ordered_currents(conn, work).await?;
    let back = back_matter_in(conn, work).await?;
    let mut projector = baruch_typst::index::Projector::new(&back);
    // Projektion „Nummer an/aus“ ([`project`]); die Texte in SQLite bleiben, wie sie sind.
    for section in &mut sections {
        if let Some(projected) = bodies.remove(&section.key) {
            section.body = projected;
        }
        // Sprungmarke `<kN>` an der Überschrift (Runde 30a): Export, Karte und Satz tragen sie, SQLite nicht.
        if let Cow::Owned(labelled) = baruch_typst::xref::label_section(&section.body, section.key) {
            section.body = labelled;
        }
        // Register und Glossar (Runde 31): Bibelstellen-Marken, erste Erwähnung; nur in der Projektion.
        if let Cow::Owned(projected) = projector.project(&section.body) {
            section.body = projected;
        }
    }
    if sections.iter().any(|s| !s.body.is_empty()) {
        end_line(&mut template);
    }
    let last = sections.len().saturating_sub(1);
    for (i, section) in sections.iter_mut().enumerate() {
        if i < last && !section.body.is_empty() && !section.body.ends_with('\n') {
            section.body.push('\n');
        }
    }
    let tail = baruch_typst::index::back_matter(&back, &sections.iter().map(|s| s.body.as_str()).collect::<Vec<_>>());
    Ok((template, sections, tail))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn numbering() -> Numbering {
        load::work_numbering("decimal", None, "")
    }

    #[test]
    fn a_title_with_code_shared_with_a_numbered_node_gets_a_rule_over_its_jump_label() {
        // R21b-Rest: Knoten 2 („ohne Nummer“, Titel mit Code, lässt sich nicht markieren) hat dieselbe Überschrift wie
        // der nummerierte Knoten 3. Die Regel über Ebene und Titel träfe beide – also eine Regel über <k2>.
        let rows = vec![
            (1, true, "= Anfang\n\nText.\n".to_owned()),
            (2, false, "= #emph[Rast]\n\nPause.\n".to_owned()),
            (3, true, "= #emph[Rast]\n\nWeiter.\n".to_owned()),
        ];
        let projection = project(&rows, &numbering());
        assert_eq!(projection.by_label, vec![2], "Regel über die Sprungmarke für Knoten 2");
        assert!(projection.fallback.is_empty(), "keine Regel über Ebene und Titel (träfe beide)");
        assert_eq!(projection.counted, vec![1, 3], "Typst zählt Knoten 2 nicht mehr mit");
        assert_eq!(
            label_rules(&projection.by_label),
            format!("{LABEL_FALLBACK_MARK}\n#show <k2>: set heading(numbering: none)\n"),
            "eine Regel je Knoten unter der Kennzeile"
        );
        // Ohne Doppelung bleibt es bei der Regel über Ebene und Titel (Exportform unverändert).
        let single = project(&rows[..2], &numbering());
        assert!(single.by_label.is_empty() && single.fallback.len() == 1, "Rückfall wie bisher");
    }

    #[test]
    fn the_label_block_reads_back_and_leaves_the_fallback_block_in_place() {
        let template = format!(
            "#set page(paper: \"a5\")\n{LABEL_FALLBACK_MARK}\n#show <k7>: set heading(numbering: none)\n\
             #show <k12>: set heading(numbering: none)\n// Baruch: Überschriften ohne Nummer\n\
             #show heading.where(level: 1, body: [Vorwort]): set heading(numbering: none)\n"
        );
        let (rest, keys) = split_label_fallback(&template);
        assert_eq!(keys, vec![7, 12], "alte Schlüssel der Überschriften ohne Nummer");
        assert_eq!(
            rest,
            "#set page(paper: \"a5\")\n// Baruch: Überschriften ohne Nummer\n\
             #show heading.where(level: 1, body: [Vorwort]): set heading(numbering: none)\n",
            "der Block verschwindet, der Rückfall bleibt für split_fallback"
        );
        let foreign = format!("{LABEL_FALLBACK_MARK}\n#show <k7>: set text(red)\n");
        assert_eq!(split_label_fallback(&foreign), (foreign.clone(), Vec::new()), "fremde Form bleibt in der Vorlage");
    }

    #[test]
    fn relative_image_paths_point_from_the_root_in_section_files() {
        let body = "= Hof\n\n#image(\"bilder/a.svg\")\n#figure(image(\"./bilder/b.png\", width: 2cm))\n\
                    #image(\"/bilder/c.svg\")\n#image(\"../draussen.svg\")\n#bild(\"bilder/d.svg\")\n";
        assert_eq!(
            root_image_paths(body),
            "= Hof\n\n#image(\"/bilder/a.svg\")\n#figure(image(\"/./bilder/b.png\", width: 2cm))\n\
             #image(\"/bilder/c.svg\")\n#image(\"../draussen.svg\")\n#bild(\"bilder/d.svg\")\n",
            "nur relative image(\"…\")-Pfade, absolute und ../ bleiben, Funktionen der Vorlage auch"
        );
        assert!(matches!(root_image_paths("= Ohne Bild\n"), Cow::Borrowed(_)), "ohne Bild unverändert geliehen");
    }
}
