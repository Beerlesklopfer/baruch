// SPDX-License-Identifier: AGPL-3.0-or-later
//! Werke durch den Menschen verwalten (Vertrag Runde 4, Abschnitte 1 und 3b): eindeutige Titel,
//! Anlegen, Metadaten, Fassungen der Werkvorlage, Löschen ganzer Werke.
//!
//! - **Eindeutige Titel:** Zwei Werke dürfen nicht denselben [`title_key`] haben (Teilindex
//!   `work_title_key` aus Migration `0005_works.sql`). Anlegen, Import und Ändern mit einem schon
//!   vergebenen Titel → [`StoreError::DuplicateTitle`], nichts geändert. Vorhandene Doppel aus der
//!   Zeit vor 0005 löst [`ensure_title_keys`] beim Öffnen auf, nur über den Titel (slug, Spiegel,
//!   Gliederung und Texte bleiben).
//! - **Werkvorlage:** `work.template` ist die aktuelle Vorlage, jede Änderung eine neue Fassung in
//!   `work_template_revision` (Nummern ab 1, nur anhängen). Snapshot und Export lesen weiter
//!   `work.template`.
//! - **Löschen** ([`Store::delete_work`]) nur mit eingetipptem Titel und in einer Transaktion: Der
//!   Trigger `revision_no_delete_current` (0004) schützt die aktuelle Fassung lebender Abschnitte;
//!   darum setzt das Löschen zuerst alle aktuellen Fassungen des Werks auf `superseded` (erlaubt
//!   laut `revision_status_transition`) und löscht danach Revisionen, Verweise, Beiträge, Knoten,
//!   Vorlagenfassungen und das Werk. id, slug und Titel wandern nach `work_retired` (Migration 0007).
//! - **ids und slugs nie wiederverwenden** (Vertrag Runde 5): `work.id` ist seit 0007
//!   `AUTOINCREMENT`, ein neues Werk bekommt nach dem Löschen nie die id eines alten; neue Ordnernamen
//!   meiden auch die slugs gelöschter Werke (`import::free_slug`).

use std::collections::{BTreeMap, HashMap, HashSet};

use baruch_core::WorkKind;
use sqlx::{SqliteConnection, SqlitePool};

use crate::{Imported, Store, StoreError, WorkId, WorkSummary, import, slug, to_u32, to_usize, words};

/// Vergleichsschlüssel eines Titels: getrimmt, Folgen von Leerraum (Unicode) → ein Leerzeichen,
/// Kleinbuchstaben (Unicode). „ Das Buch “ und „das  buch“ ergeben denselben Schlüssel.
pub fn title_key(title: &str) -> String {
    one_line(title).to_lowercase()
}

/// Eine Angabe in einer Zeile: Folgen von Leerraum (auch Zeilenumbrüche) → ein Leerzeichen, getrimmt.
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Grundgerüst eines neuen Werks ([`Store::create_work`]).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Preset {
    /// Werkvorlage mit `#set document` und `#set heading(numbering: "1.1")`, ein Kapitel „Erstes Kapitel“.
    #[default]
    Book,
    /// Werkvorlage mit `#set document`, Gliederungsschema `sermon` („Einleitung · I. · II. · III. · Schluss“) und
    /// den Punkten aus [`SERMON_OUTLINE`]: Predigttext und Anwendung ohne Nummer, dazwischen Einleitung, drei
    /// Hauptpunkte und Schluss (Runde 16a).
    Sermon,
}

/// Angaben für [`Store::create_work`]. Titel und Angaben werden getrimmt, Zeilenumbrüche zu Leerzeichen.
#[derive(Debug, Clone, Copy)]
pub struct NewWork<'a> {
    pub title: &'a str,
    pub author: &'a str,
    pub kind: WorkKind,
    pub preset: Preset,
    pub subtitle: &'a str,
    /// Zielgruppe.
    pub audience: &'a str,
    /// Bibelübersetzung.
    pub bible: &'a str,
}

/// Was [`Store::update_work`] ändern soll; `None` = bleibt.
#[derive(Debug, Clone, Copy, Default)]
pub struct WorkEdit<'a> {
    pub title: Option<&'a str>,
    pub subtitle: Option<&'a str>,
    pub author: Option<&'a str>,
    pub kind: Option<WorkKind>,
    pub audience: Option<&'a str>,
    pub bible: Option<&'a str>,
    /// Neue Werkvorlage (CRLF → LF); nur ein anderer Text ergibt eine neue Fassung.
    pub template: Option<&'a str>,
    /// Notiz zur neuen Fassung der Werkvorlage.
    pub note: Option<&'a str>,
}

/// Alle Angaben eines Werks ([`Store::work_meta`]).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct WorkMeta {
    pub id: WorkId,
    /// Weltweit eindeutige Kennung (Migration 0041, Austauschformat); nur lesen.
    pub uuid: String,
    pub slug: String,
    pub kind: WorkKind,
    pub title: String,
    pub subtitle: String,
    pub author: String,
    /// Zielgruppe.
    pub audience: String,
    /// Bibelübersetzung.
    pub bible: String,
    /// Aktuelle Werkvorlage.
    pub template: String,
    /// Nummer der aktuellen Fassung der Werkvorlage (0 = keine Fassung gespeichert).
    pub template_rev: u32,
    /// Dateiname beim Import; `None` bei gesäten und angelegten Werken.
    pub source_name: Option<String>,
    pub created: String,
    /// Letzte Änderung von Metadaten oder Werkvorlage; `None` = nie geändert.
    pub updated: Option<String>,
}

/// Eine Fassung der Werkvorlage (ohne Text, [`Store::template_revisions`]).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct TemplateRevision {
    pub number: u32,
    pub author: String,
    pub note: String,
    pub created: String,
}

/// Ergebnis von [`Store::update_work`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct WorkUpdate {
    pub work: WorkId,
    /// Geänderte Felder: `title`, `subtitle`, `author`, `kind`, `audience`, `bible`, `template`.
    pub changed: Vec<&'static str>,
    /// Nummer der neuen Fassung, wenn sich die Werkvorlage geändert hat.
    pub template_rev: Option<u32>,
}

/// Ergebnis von [`Store::delete_work`]: was gelöscht wurde.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct DeletedWork {
    pub work: WorkId,
    pub slug: String,
    pub title: String,
    pub nodes: usize,
    pub revisions: usize,
}

/// Ein gelöschtes (archiviertes) Werk aus `work_retired` (Runde 22, Archiv).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct RetiredWork {
    pub id: WorkId,
    pub slug: String,
    pub title: String,
    pub deleted: String,
}

/// Ergebnis von [`Store::restore_retired`]: das wiederhergestellte Werk.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct RestoredWork {
    pub work: WorkId,
    pub slug: String,
    pub title: String,
    pub nodes: usize,
}

/// Notiz der Fassungen und der Werkvorlage eines aus dem Archiv wiederhergestellten Werks (Runde 22).
pub const RESTORED_NOTE: &str = "aus dem Archiv wiederhergestellt";

/// Notiz der ersten Fassungen eines angelegten Werks.
const CREATED_NOTE: &str = "Werk angelegt";

/// Titel des Punkts, der den Predigttext trägt (ohne Nummer; [`Store::create_work_with_text`]).
pub const SERMON_TEXT_TITLE: &str = "Predigttext";

/// Gliederung einer neuen Predigt (Runde 16a): Titel und ob der Punkt gezählt wird. Das Schema `sermon` nennt
/// den ersten gezählten Punkt „Einleitung“, den letzten „Schluss“ und die übrigen „I.“, „II.“, „III.“. Darum
/// tragen die gezählten Punkte Titel, die die Nummer nicht wiederholen (sonst stünde in der Gliederung
/// „Einleitung Einleitung“ oder „I. Erster Hauptpunkt“); Predigttext und Anwendung haben keine Nummer und
/// heißen, was sie sind. Begründung in `docs/Architektur.md`, „Runde 16a“.
pub const SERMON_OUTLINE: &[(&str, bool)] = &[
    (SERMON_TEXT_TITLE, false),
    ("Hinführung", true),
    ("Beobachtung am Text", true),
    ("Deutung", true),
    ("Zuspruch", true),
    ("Anwendung", false),
    ("Ausblick", true),
];

/// Kapitel bzw. Predigtteile eines neuen Werks: Titel und „mit Nummer“.
fn preset_chapters(preset: Preset) -> &'static [(&'static str, bool)] {
    match preset {
        Preset::Book => &[("Erstes Kapitel", true)],
        Preset::Sermon => SERMON_OUTLINE,
    }
}

/// Gliederungsschema eines neuen Werks (`work.numbering_scheme`) und die Nummer je Punkt nach diesem Schema.
fn preset_numbers(preset: Preset, chapters: &[(&str, bool)]) -> (&'static str, Vec<String>) {
    match preset {
        // Wie bisher: Schema aus der Werkvorlage, vorläufige Nummer 1, 2, … (die Vorlage zählt „1.1“).
        Preset::Book => ("template", (1..=chapters.len()).map(|n| n.to_string()).collect()),
        Preset::Sermon => {
            let scheme = baruch_typst::scheme::Scheme::Sermon;
            let numbering = baruch_typst::scheme::Numbering::of_scheme(scheme, None, "")
                .expect("das Predigtschema braucht kein Muster");
            let counted = chapters.iter().filter(|(_, numbered)| *numbered).count();
            let mut own = 0;
            let numbers = chapters
                .iter()
                .map(|(_, numbered)| {
                    if *numbered {
                        own += 1;
                        numbering.heading(&[own], counted)
                    } else {
                        String::new()
                    }
                })
                .collect();
            (scheme.id(), numbers)
        }
    }
}

/// Typst-String ohne Anführungszeichen: `\` und `"` maskiert.
fn typst_string(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Werkvorlage eines neuen Werks.
fn preset_template(preset: Preset, title: &str, author: &str) -> String {
    let document = format!("#set document(title: \"{}\", author: \"{}\")\n", typst_string(title), typst_string(author));
    match preset {
        // Runde 27: Impressum-Vorlage (Titelei-Funktionen schmutztitel, haupttitel, impressum, widmung) angehängt.
        Preset::Book => {
            format!("{document}#set heading(numbering: \"1.1\")\n\n{}", baruch_typst::cover::imprint_template())
        }
        Preset::Sermon => format!("{document}\n"),
    }
}

/// Ist der Schlüssel schon an ein anderes Werk als `except` vergeben → `DuplicateTitle`.
pub(crate) async fn title_free(
    conn: &mut SqliteConnection,
    key: &str,
    except: Option<WorkId>,
) -> Result<(), StoreError> {
    let taken: Option<(WorkId, String, String)> =
        sqlx::query_as("SELECT id, slug, title FROM work WHERE title_key = ? AND id IS NOT ? ORDER BY id LIMIT 1")
            .bind(key)
            .bind(except)
            .fetch_optional(&mut *conn)
            .await?;
    match taken {
        Some((work, slug, title)) => Err(StoreError::DuplicateTitle { work, slug, title }),
        None => Ok(()),
    }
}

/// Fassung `number` der Werkvorlage anhängen.
pub(crate) async fn insert_template_revision(
    conn: &mut SqliteConnection,
    work: WorkId,
    number: u32,
    body: &str,
    author: &str,
    note: &str,
) -> Result<(), StoreError> {
    sqlx::query("INSERT INTO work_template_revision (work_id, number, body, author_name, note) VALUES (?, ?, ?, ?, ?)")
        .bind(work)
        .bind(i64::from(number))
        .bind(body)
        .bind(author)
        .bind(note)
        .execute(&mut *conn)
        .await?;
    Ok(())
}

// ---- Nachtrag der Schlüssel beim Öffnen ------------------------------------------------------

/// Ein Werk beim Nachtrag: id, Titel, gespeicherter Schlüssel, gesät (ohne Quelle).
#[derive(Debug, Clone)]
struct Entry {
    id: WorkId,
    title: String,
    key: Option<String>,
    seeded: bool,
}

/// Neue `(id, Titel, Schlüssel)` für alle Werke, deren Titel oder Schlüssel sich ändert. `entries`
/// ist nach id sortiert. Regeln (Vertrag Runde 4, Abschnitt 1): Bei gleichem Schlüssel behält das
/// Werk mit der kleineren id seinen Titel; trifft dabei ein gesätes Werk (ohne Quelle) auf ein
/// importiertes, behält das importierte mit der kleinsten id den Titel, und das gesäte bekommt den
/// Titel des Beispielwerks (`sample_title`), sofern frei. Sonst und in allen anderen Fällen
/// „<Titel> (2)“, „ (3)“ … mit der kleinsten freien Zahl.
fn plan_titles(entries: &[Entry], sample_title: &str) -> Vec<(WorkId, String, String)> {
    let keys: Vec<String> = entries.iter().map(|e| title_key(&e.title)).collect();
    let mut groups: BTreeMap<&str, Vec<usize>> = BTreeMap::new();
    for (index, key) in keys.iter().enumerate() {
        groups.entry(key.as_str()).or_default().push(index);
    }
    // Verlierer mit dem Hinweis, ob ein gesätes Werk auf ein importiertes traf.
    let mut losers: Vec<(usize, bool)> = Vec::new();
    for members in groups.values().filter(|m| m.len() > 1) {
        let first_imported = members.iter().copied().find(|&i| !entries[i].seeded);
        let keeper = first_imported.unwrap_or(members[0]);
        for &i in members.iter().filter(|&&i| i != keeper) {
            losers.push((i, entries[i].seeded && first_imported.is_some()));
        }
    }
    losers.sort_unstable();
    let loser_set: HashSet<usize> = losers.iter().map(|&(i, _)| i).collect();
    let mut used: HashSet<String> =
        keys.iter().enumerate().filter(|(i, _)| !loser_set.contains(i)).map(|(_, k)| k.clone()).collect();
    let mut renamed: HashMap<usize, String> = HashMap::new();
    for (index, seeded_against_import) in losers {
        let base = entries[index].title.trim();
        let title = if seeded_against_import && !used.contains(&title_key(sample_title)) {
            sample_title.to_owned()
        } else {
            (2u32..)
                .map(|n| format!("{base} ({n})"))
                .find(|t| !used.contains(&title_key(t)))
                .expect("eine freie Zahl gibt es immer")
        };
        used.insert(title_key(&title));
        renamed.insert(index, title);
    }
    entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| {
            let title = renamed.remove(&index).unwrap_or_else(|| entry.title.clone());
            let key = title_key(&title);
            (title != entry.title || entry.key.as_deref() != Some(key.as_str())).then_some((entry.id, title, key))
        })
        .collect()
}

/// Schlüssel nachtragen (nach `MIGRATOR.run`, vor der Saat; `BEGIN IMMEDIATE`, wiederholbar): Werke
/// ohne oder mit veraltetem `title_key` bekommen ihn; kollidiert er, ändert sich nur der Titel
/// (Regeln bei [`plan_titles`]). Slug, Spiegel, Gliederung und Texte bleiben. Stimmt schon alles,
/// ändert sich nichts.
pub(crate) async fn ensure_title_keys(pool: &SqlitePool) -> Result<(), StoreError> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let rows: Vec<(WorkId, String, Option<String>, Option<String>)> =
        sqlx::query_as("SELECT id, title, title_key, source_name FROM work ORDER BY id").fetch_all(&mut *tx).await?;
    let entries: Vec<Entry> =
        rows.into_iter().map(|(id, title, key, source)| Entry { id, title, key, seeded: source.is_none() }).collect();
    let plan = plan_titles(&entries, &baruch_core::sample::work().title);
    if plan.is_empty() {
        return Ok(());
    }
    // Erst alle betroffenen Schlüssel lösen, dann neu setzen: So stört der eindeutige Index nicht
    // in einer Zwischenstellung.
    for (id, _, _) in &plan {
        sqlx::query("UPDATE work SET title_key = NULL WHERE id = ?").bind(id).execute(&mut *tx).await?;
    }
    for (id, title, key) in &plan {
        sqlx::query("UPDATE work SET title = ?, title_key = ? WHERE id = ?")
            .bind(title)
            .bind(key)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

// ---- Store ------------------------------------------------------------------------------------

type MetaRow = (
    WorkId,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
    Option<String>,
    String,
    Option<String>,
    i64,
    String,
);
type SummaryRow = (WorkId, String, String, String, String, String, String, i64, i64, i64, Option<String>);

impl Store {
    /// Alle Werke für Übersicht und Werkwahl, nach id: Autor, Art, Untertitel, Anzahl der Knoten
    /// (`nodes`) und der Abschnitte ohne Teile (`sections`), offene Vorschläge, angelegt und zuletzt
    /// geändert (größter Zeitpunkt aus Metadaten, angelegten und geänderten Revisionen).
    pub async fn work_list(&self) -> Result<Vec<WorkSummary>, StoreError> {
        let rows: Vec<SummaryRow> = sqlx::query_as(
            "SELECT w.id, w.slug, w.title, w.author, w.kind, w.subtitle, w.created, \
                    (SELECT count(*) FROM node n WHERE n.work_id = w.id), \
                    (SELECT count(*) FROM node n WHERE n.work_id = w.id AND n.kind <> 'part'), \
                    (SELECT count(*) FROM revision r JOIN node n ON n.id = r.node_id \
                      WHERE n.work_id = w.id AND r.status = 'proposed'), \
                    nullif(max(coalesce(w.updated, ''), \
                        coalesce((SELECT max(r.created) FROM revision r JOIN node n ON n.id = r.node_id \
                                   WHERE n.work_id = w.id), ''), \
                        coalesce((SELECT max(r.edited) FROM revision r JOIN node n ON n.id = r.node_id \
                                   WHERE n.work_id = w.id), '')), '') \
               FROM work w ORDER BY w.id",
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|(id, slug, title, author, kind, subtitle, created, nodes, sections, proposals, updated)| {
                Ok(WorkSummary {
                    id,
                    slug,
                    title,
                    author,
                    nodes: to_usize(nodes, "Anzahl der Knoten")?,
                    kind: words::parse_work_kind(&kind)?,
                    subtitle,
                    sections: to_usize(sections, "Anzahl der Abschnitte")?,
                    proposals: to_usize(proposals, "Anzahl der Vorschläge")?,
                    created,
                    updated,
                })
            })
            .collect()
    }

    /// Neues Werk aus einem Grundgerüst (atomar): Werkvorlage (Fassung 1), Kapitel bzw. Predigtteile
    /// mit Revision 1 von `human` (aktuell, Text `= <Titel>\n\n`). Leerer Titel → `EmptyTitle`,
    /// vergebener Titel → `DuplicateTitle`; Ordnername aus dem Titel, bei Kollision `-2`, `-3` ….
    pub async fn create_work(&self, new: &NewWork<'_>, human: &str) -> Result<Imported, StoreError> {
        self.create_work_with_text(new, human, None).await
    }

    /// Wie [`Store::create_work`]; bei einer Predigt steht `sermon_text` (Typst, z. B. das Zitat des
    /// Predigttexts mit Quelle) unter der Überschrift des Punkts [`SERMON_TEXT_TITLE`]. Beim Buch und ohne
    /// Text bleibt jeder Punkt nur seine Überschrift. Die Predigt bekommt das Gliederungsschema `sermon`,
    /// Predigttext und Anwendung `numbered = 0` (Runde 16a).
    pub async fn create_work_with_text(
        &self,
        new: &NewWork<'_>,
        human: &str,
        sermon_text: Option<&str>,
    ) -> Result<Imported, StoreError> {
        self.create_work_with_bible(new, human, sermon_text, None).await
    }

    /// Wie [`Store::create_work_with_text`], dazu die SWORD-Bibel des Werks (`work_bible`) **in derselben
    /// Transaktion** (Runde 23). Vorher waren es zwei Schritte (`create_work_with_text`, dann
    /// `set_work_bible`): Scheiterte der zweite, blieb ein Werk ohne Spiegel zurück, und ein zweiter Versuch
    /// endete mit `DuplicateTitle`. Jetzt entsteht bei einem Fehler nichts. Ein unzulässiger Modulname (leer,
    /// über 100 Zeichen, mit Zeilenumbruch – die Regel der Tabelle) → `Refused`, bevor etwas geschrieben ist.
    pub async fn create_work_with_bible(
        &self,
        new: &NewWork<'_>,
        human: &str,
        sermon_text: Option<&str>,
        bible: Option<&str>,
    ) -> Result<Imported, StoreError> {
        if let Some(module) = bible
            && (module.is_empty() || module.chars().count() > 100 || module.contains('\n'))
        {
            return Err(StoreError::Refused(
                "Bibelmodul: Name mit 1 bis 100 Zeichen in einer Zeile erwartet; das Werk wurde nicht angelegt.",
            ));
        }
        let title = one_line(new.title);
        if title.is_empty() {
            return Err(StoreError::EmptyTitle);
        }
        let author = one_line(new.author);
        let key = title_key(&title);
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        title_free(&mut tx, &key, None).await?;
        let slug = import::free_slug(&mut tx, &slug(&title)).await?;
        let template = preset_template(new.preset, &title, &author);
        let chapters = preset_chapters(new.preset);
        let (scheme, numbers) = preset_numbers(new.preset, chapters);
        let work: WorkId = sqlx::query_scalar(
            "INSERT INTO work (slug, kind, title, author, template, title_key, subtitle, audience, bible, \
                               numbering_scheme) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(&slug)
        .bind(words::work_kind(new.kind))
        .bind(&title)
        .bind(&author)
        .bind(&template)
        .bind(&key)
        .bind(one_line(new.subtitle))
        .bind(one_line(new.audience))
        .bind(one_line(new.bible))
        .bind(scheme)
        .fetch_one(&mut *tx)
        .await?;
        insert_template_revision(&mut tx, work, 1, &template, human, CREATED_NOTE).await?;
        for (seq, ((chapter, numbered), number)) in chapters.iter().zip(&numbers).enumerate() {
            let node_id: i64 = sqlx::query_scalar(
                "INSERT INTO node (work_id, seq, parent_seq, position, number, title, kind) \
                 VALUES (?, ?, NULL, ?, ?, ?, 'chapter') RETURNING id",
            )
            .bind(work)
            .bind(seq as i64)
            .bind(seq as i64)
            .bind(number)
            .bind(chapter)
            .fetch_one(&mut *tx)
            .await?;
            // Der Trigger `node_numbered_default` (0006) setzt nur bei leerer Nummer `numbered = 0`; hier
            // ausdrücklich, damit „ohne Nummer“ nicht von der Nummer abhängt.
            sqlx::query("UPDATE node SET numbered = ? WHERE id = ?")
                .bind(i64::from(*numbered))
                .bind(node_id)
                .execute(&mut *tx)
                .await?;
            let body = match sermon_text.map(str::trim).filter(|t| !t.is_empty()) {
                Some(text) if new.preset == Preset::Sermon && *chapter == SERMON_TEXT_TITLE => {
                    format!("= {chapter}\n\n{}\n\n", text.replace("\r\n", "\n"))
                }
                _ => format!("= {chapter}\n\n"),
            };
            sqlx::query(
                "INSERT INTO revision (node_id, number, body, author_kind, author_name, model, status, note) \
                 VALUES (?, 1, ?, 'human', ?, NULL, 'current', ?)",
            )
            .bind(node_id)
            .bind(body)
            .bind(human)
            .bind(CREATED_NOTE)
            .execute(&mut *tx)
            .await?;
        }
        if let Some(module) = bible {
            // Scheitert das (z. B. an der CHECK-Regel der Tabelle), rollt die Transaktion alles zurück.
            sqlx::query("INSERT INTO work_bible (work_id, module) VALUES (?, ?)")
                .bind(work)
                .bind(module)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(Imported { work, slug, title, author, nodes: chapters.len() })
    }

    /// Wie [`Store::import_typst_noted`], wahlweise mit eigenem Titel (`None` oder leer: aus
    /// `#set document`, sonst aus dem Dateinamen). Reihenfolge der Prüfungen: dieselbe Quelle schon
    /// importiert → `AlreadyImported`; keine Überschrift → `Import`; Titel vergeben →
    /// `DuplicateTitle`. Die Werkvorlage wird Fassung 1 (Notiz wie die Revisionen).
    pub async fn import_typst_titled(
        &self,
        source: &str,
        file_name: &str,
        human: &str,
        note: &str,
        title: Option<&str>,
    ) -> Result<Imported, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let imported = import::import_typst(&mut tx, source, file_name, human, note, title).await?;
        tx.commit().await?;
        Ok(imported)
    }

    /// Alle Angaben eines Werks; unbekannt → `UnknownWork`.
    pub async fn work_meta(&self, work: WorkId) -> Result<WorkMeta, StoreError> {
        let row: Option<MetaRow> = sqlx::query_as(
            "SELECT w.id, w.slug, w.kind, w.title, w.subtitle, w.author, w.audience, w.bible, w.template, \
                    w.source_name, w.created, w.updated, \
                    coalesce((SELECT max(t.number) FROM work_template_revision t WHERE t.work_id = w.id), 0), \
                    w.uuid \
               FROM work w WHERE w.id = ?",
        )
        .bind(work)
        .fetch_optional(&self.pool)
        .await?;
        let (
            id,
            slug,
            kind,
            title,
            subtitle,
            author,
            audience,
            bible,
            template,
            source_name,
            created,
            updated,
            rev,
            uuid,
        ) = row.ok_or(StoreError::UnknownWork(work))?;
        Ok(WorkMeta {
            id,
            uuid,
            slug,
            kind: words::parse_work_kind(&kind)?,
            title,
            subtitle,
            author,
            audience,
            bible,
            template,
            template_rev: to_u32(rev, "Fassung der Werkvorlage")?,
            source_name,
            created,
            updated,
        })
    }

    /// Fassungen der Werkvorlage nach Nummer (ohne Text); unbekanntes Werk → `UnknownWork`.
    pub async fn template_revisions(&self, work: WorkId) -> Result<Vec<TemplateRevision>, StoreError> {
        self.work_slug(work).await?;
        let rows: Vec<(i64, String, String, String)> = sqlx::query_as(
            "SELECT number, author_name, note, created FROM work_template_revision WHERE work_id = ? ORDER BY number",
        )
        .bind(work)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter()
            .map(|(number, author, note, created)| {
                Ok(TemplateRevision { number: to_u32(number, "Fassung der Werkvorlage")?, author, note, created })
            })
            .collect()
    }

    /// Text der Fassung `number` der Werkvorlage; `None`, wenn es sie nicht gibt. Unbekanntes Werk →
    /// `UnknownWork`.
    pub async fn template_revision(&self, work: WorkId, number: u32) -> Result<Option<String>, StoreError> {
        self.work_slug(work).await?;
        Ok(sqlx::query_scalar("SELECT body FROM work_template_revision WHERE work_id = ? AND number = ?")
            .bind(work)
            .bind(i64::from(number))
            .fetch_optional(&self.pool)
            .await?)
    }

    /// Metadaten und Werkvorlage ändern (atomar, nur der Mensch). Nur Felder mit anderem Wert zählen
    /// (`changed`); dann wird `updated` gesetzt. Titel leer → `EmptyTitle`, an ein anderes Werk
    /// vergeben → `DuplicateTitle` (nichts geändert). Eine andere Werkvorlage (CRLF → LF) wird neue
    /// Fassung (`template_rev`) und `work.template`; enthält sie eine Überschrift (`=` am
    /// Zeilenanfang), gehörte diese zur Gliederung → `Refused`. Slug und Spiegelordner bleiben.
    pub async fn update_work(&self, work: WorkId, edit: &WorkEdit<'_>, human: &str) -> Result<WorkUpdate, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let row: Option<(String, String, String, String, String, String, String)> =
            sqlx::query_as("SELECT title, subtitle, author, kind, audience, bible, template FROM work WHERE id = ?")
                .bind(work)
                .fetch_optional(&mut *tx)
                .await?;
        let (title, subtitle, author, kind, audience, bible, template) = row.ok_or(StoreError::UnknownWork(work))?;
        let mut changed: Vec<&'static str> = Vec::new();

        let mut new_title = title.clone();
        if let Some(wanted) = edit.title {
            let wanted = one_line(wanted);
            if wanted.is_empty() {
                return Err(StoreError::EmptyTitle);
            }
            if wanted != title {
                title_free(&mut tx, &title_key(&wanted), Some(work)).await?;
                new_title = wanted;
                changed.push("title");
            }
        }
        let new_subtitle = pick(&mut changed, "subtitle", subtitle, edit.subtitle);
        let new_author = pick(&mut changed, "author", author, edit.author);
        let new_kind = match edit.kind {
            Some(wanted) if words::work_kind(wanted) != kind => {
                changed.push("kind");
                words::work_kind(wanted).to_owned()
            }
            _ => kind,
        };
        let new_audience = pick(&mut changed, "audience", audience, edit.audience);
        let new_bible = pick(&mut changed, "bible", bible, edit.bible);

        let mut template_rev = None;
        let mut new_template = template.clone();
        if let Some(wanted) = edit.template.map(|t| t.replace("\r\n", "\n"))
            && wanted != template
        {
            if baruch_typst::parse(&wanted).is_ok() {
                return Err(StoreError::Refused(
                    "Die Werkvorlage darf keine Überschrift (= am Zeilenanfang) enthalten; Überschriften gehören zur Gliederung.",
                ));
            }
            let last: i64 =
                sqlx::query_scalar("SELECT coalesce(max(number), 0) FROM work_template_revision WHERE work_id = ?")
                    .bind(work)
                    .fetch_one(&mut *tx)
                    .await?;
            let number = to_u32(last, "Fassung der Werkvorlage")? + 1;
            insert_template_revision(&mut tx, work, number, &wanted, human, &one_line(edit.note.unwrap_or(""))).await?;
            template_rev = Some(number);
            new_template = wanted;
            changed.push("template");
        }

        if !changed.is_empty() {
            sqlx::query(
                "UPDATE work SET title = ?, title_key = ?, subtitle = ?, author = ?, kind = ?, audience = ?, \
                        bible = ?, template = ?, updated = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
            )
            .bind(&new_title)
            .bind(title_key(&new_title))
            .bind(&new_subtitle)
            .bind(&new_author)
            .bind(&new_kind)
            .bind(&new_audience)
            .bind(&new_bible)
            .bind(&new_template)
            .bind(work)
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
        }
        Ok(WorkUpdate { work, changed, template_rev })
    }

    /// Werk ganz löschen (atomar, nur der Mensch). `confirm_title` muss denselben [`title_key`] wie
    /// der Titel haben, sonst `Refused`, nichts gelöscht. In einer Transaktion: alle aktuellen
    /// Fassungen → `superseded` (damit `revision_no_delete_current` nicht greift), dann Revisionen,
    /// Verweise, Beiträge, Knoten, Fassungen der Werkvorlage und das Werk; id, slug und Titel kommen
    /// nach `work_retired` (der slug bleibt reserviert). Volltextindex: Revisionen entfernen die
    /// Trigger aus `baruch-library` 0104, Beiträge die aus 0105 (`post_fts_delete`). Den Spiegel
    /// räumt der Server auf. Sind danach keine Werke mehr da, sät der nächste Start nicht neu.
    pub async fn delete_work(&self, work: WorkId, confirm_title: &str) -> Result<DeletedWork, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (slug, title): (String, String) = sqlx::query_as("SELECT slug, title FROM work WHERE id = ?")
            .bind(work)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(StoreError::UnknownWork(work))?;
        if title_key(confirm_title) != title_key(&title) {
            return Err(StoreError::Refused("Der eingegebene Titel passt nicht; nichts gelöscht."));
        }
        let nodes: i64 =
            sqlx::query_scalar("SELECT count(*) FROM node WHERE work_id = ?").bind(work).fetch_one(&mut *tx).await?;
        let revisions: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM revision WHERE node_id IN (SELECT id FROM node WHERE work_id = ?)",
        )
        .bind(work)
        .fetch_one(&mut *tx)
        .await?;
        delete_work_rows(&mut tx, work).await?;
        tx.commit().await?;
        Ok(DeletedWork {
            work,
            slug,
            title,
            nodes: to_usize(nodes, "Anzahl der Knoten")?,
            revisions: to_usize(revisions, "Anzahl der Revisionen")?,
        })
    }
}

impl Store {
    /// Gelöschte Werke aus `work_retired` (neueste zuerst; Runde 22, Archiv).
    pub async fn retired_works(&self) -> Result<Vec<RetiredWork>, StoreError> {
        let rows: Vec<(i64, String, String, String)> =
            sqlx::query_as("SELECT id, slug, title, deleted FROM work_retired ORDER BY deleted DESC, id DESC")
                .fetch_all(&self.pool)
                .await?;
        Ok(rows.into_iter().map(|(id, slug, title, deleted)| RetiredWork { id, slug, title, deleted }).collect())
    }

    /// Legt ein archiviertes Werk mit seinem reservierten slug aus dem Typst-Text des Archivordners wieder an (Runde 22,
    /// Archiv), alles in einer Transaktion (`BEGIN IMMEDIATE`):
    ///
    /// - `slug` muss in `work_retired` stehen, sonst `Refused` (nichts geändert); der Titel kommt aus `work_retired`
    ///   (nicht aus `#set document`), damit das Werk so heißt wie vor dem Löschen.
    /// - Trägt ein lebendes Werk denselben Titel ([`title_key`]) → `Refused`; dieselbe Quelle schon importiert →
    ///   `Refused`; keine Überschrift → `Import`.
    /// - Angelegt wird wie beim Import ([`import::import_typst`]): Präambel als Werkvorlage (Fassung 1), jede
    ///   Überschrift ein Gliederungspunkt mit Fassung 1 vom Menschen `human`, aktuell, Notiz [`RESTORED_NOTE`]; ein
    ///   Regelblock „// Baruch: Satz“ wird wieder Satz-Einstellung. Die Werkart ist danach „Buch“ (`work_retired`
    ///   kennt sie nicht).
    /// - Die Zeile in `work_retired` entfällt, das Werk bekommt genau den reservierten slug (der Spiegelordner kann
    ///   so nach `works/<slug>` zurück) und eine neue id (ids werden nie wiederverwendet).
    pub async fn restore_retired(&self, slug: &str, typst: &str, human: &str) -> Result<RestoredWork, StoreError> {
        let slug = slug.trim();
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let title: Option<String> = sqlx::query_scalar("SELECT title FROM work_retired WHERE slug = ?")
            .bind(slug)
            .fetch_optional(&mut *tx)
            .await?;
        let Some(title) = title else {
            return Err(StoreError::Refused(
                "Dieses Werk steht nicht im Archiv (unbekannter Ordnername); nichts wiederhergestellt.",
            ));
        };
        match title_free(&mut tx, &title_key(&title), None).await {
            Ok(()) => {}
            Err(StoreError::DuplicateTitle { .. }) => {
                return Err(StoreError::Refused(
                    "Ein lebendes Werk trägt schon denselben Titel; erst jenes umbenennen, dann wiederherstellen. \
                     Nichts wiederhergestellt.",
                ));
            }
            Err(e) => return Err(e),
        }
        sqlx::query("DELETE FROM work_retired WHERE slug = ?").bind(slug).execute(&mut *tx).await?;
        let imported = match import::import_typst(&mut tx, typst, "main.typ", human, RESTORED_NOTE, Some(&title)).await
        {
            Ok(imported) => imported,
            Err(StoreError::AlreadyImported { .. }) => {
                return Err(StoreError::Refused(
                    "Dieselbe Quelle ist schon als lebendes Werk importiert; nichts wiederhergestellt.",
                ));
            }
            Err(e) => return Err(e),
        };
        if imported.slug != slug {
            sqlx::query("UPDATE work SET slug = ? WHERE id = ?")
                .bind(slug)
                .bind(imported.work)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(RestoredWork { work: imported.work, slug: slug.to_owned(), title: imported.title, nodes: imported.nodes })
    }
}

/// Neuer Wert eines Textfelds: `wanted` (in einer Zeile), wenn angegeben und anders als `now`; dann
/// steht `name` in `changed`.
fn pick(changed: &mut Vec<&'static str>, name: &'static str, now: String, wanted: Option<&str>) -> String {
    match wanted.map(one_line) {
        Some(wanted) if wanted != now => {
            changed.push(name);
            wanted
        }
        _ => now,
    }
}

/// Alle Zeilen eines Werks löschen, innerhalb der laufenden Transaktion (siehe [`Store::delete_work`]).
async fn delete_work_rows(conn: &mut SqliteConnection, work: WorkId) -> Result<(), StoreError> {
    // Bilder zuerst (Migration 0009): Pfade, Fassungen (die aktuelle vorher auf superseded), Bilder.
    crate::assets::delete_work_assets(conn, work).await?;
    const NODES: &str = "SELECT id FROM node WHERE work_id = ?";
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "UPDATE revision SET status = 'superseded' WHERE status = 'current' AND node_id IN ({NODES})"
    )))
    .bind(work)
    .execute(&mut *conn)
    .await?;
    sqlx::query(sqlx::AssertSqlSafe(format!("DELETE FROM revision WHERE node_id IN ({NODES})")))
        .bind(work)
        .execute(&mut *conn)
        .await?;
    sqlx::query(sqlx::AssertSqlSafe(format!(
        "DELETE FROM post_ref WHERE post_id IN (SELECT id FROM post WHERE node_id IN ({NODES}))"
    )))
    .bind(work)
    .execute(&mut *conn)
    .await?;
    sqlx::query(sqlx::AssertSqlSafe(format!("DELETE FROM post WHERE node_id IN ({NODES})")))
        .bind(work)
        .execute(&mut *conn)
        .await?;
    sqlx::query("DELETE FROM node WHERE work_id = ?").bind(work).execute(&mut *conn).await?;
    sqlx::query("DELETE FROM work_template_revision WHERE work_id = ?").bind(work).execute(&mut *conn).await?;
    // id, slug und Titel merken (Migration 0007): Der slug bleibt reserviert ([`import::free_slug`]),
    // die id vergibt `AUTOINCREMENT` ohnehin nie wieder.
    sqlx::query("INSERT INTO work_retired (id, slug, title) SELECT id, slug, title FROM work WHERE id = ?")
        .bind(work)
        .execute(&mut *conn)
        .await?;
    sqlx::query("DELETE FROM work WHERE id = ?").bind(work).execute(&mut *conn).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: WorkId, title: &str, seeded: bool) -> Entry {
        Entry { id, title: title.into(), key: None, seeded }
    }

    #[test]
    fn title_keys_ignore_case_and_surrounding_or_repeated_whitespace() {
        assert_eq!(title_key(" Das Buch "), title_key("das  buch"), "Ränder und doppelte Leerzeichen zählen nicht");
        assert_eq!(title_key("ÄRGER\u{a0}im\tGarten"), "ärger im garten", "Unicode-Leerraum und Umlaute");
        assert_ne!(title_key("Das Buch"), title_key("Das Buch 2"), "verschiedene Titel bleiben verschieden");
    }

    #[test]
    fn a_seeded_work_meeting_an_imported_one_takes_the_sample_title() {
        let plan = plan_titles(&[entry(1, "Mövenoog", true), entry(2, "Mövenoog", false)], "Beispielwerk");
        assert_eq!(
            plan,
            vec![(1, "Beispielwerk".into(), "beispielwerk".into()), (2, "Mövenoog".into(), "mövenoog".into())],
            "das gesäte Werk weicht, obwohl es die kleinere id hat; beide bekommen ihren Schlüssel"
        );
    }

    #[test]
    fn otherwise_the_larger_id_gets_the_smallest_free_number() {
        let entries =
            [entry(1, "Buch", false), entry(2, "buch", false), entry(3, "Buch (2)", false), entry(4, " BUCH ", false)];
        let plan = plan_titles(&entries, "Beispielwerk");
        let titles: Vec<(WorkId, &str)> = plan.iter().map(|(id, t, _)| (*id, t.as_str())).collect();
        assert_eq!(
            titles,
            vec![(1, "Buch"), (2, "buch (3)"), (3, "Buch (2)"), (4, "BUCH (4)")],
            "kleinere id behält, „(2)“ ist schon vergeben, danach die kleinste freie Zahl"
        );
    }

    #[test]
    fn a_taken_sample_title_falls_back_to_a_number() {
        let entries = [entry(1, "Beispielwerk", false), entry(2, "Mövenoog", true), entry(3, "Mövenoog", false)];
        let plan = plan_titles(&entries, "Beispielwerk");
        assert!(plan.contains(&(2, "Mövenoog (2)".into(), "mövenoog (2)".into())), "Beispieltitel vergeben: {plan:?}");
    }

    #[test]
    fn keys_that_are_already_right_need_no_change() {
        let entries = [Entry { id: 1, title: "Eins".into(), key: Some("eins".into()), seeded: true }];
        assert!(plan_titles(&entries, "Beispielwerk").is_empty(), "zweiter Lauf ändert nichts");
    }

    #[test]
    fn new_works_get_a_template_with_document_and_numbering() {
        let book = preset_template(Preset::Book, "Ein \"Buch\"", "A\\B");
        let head =
            "#set document(title: \"Ein \\\"Buch\\\"\", author: \"A\\\\B\")\n#set heading(numbering: \"1.1\")\n\n";
        assert_eq!(
            book,
            format!("{head}{}", baruch_typst::cover::imprint_template()),
            "Buch: Titel und Autor als Typst-String maskiert, Nummerierung 1.1, danach die Impressum-Vorlage (Runde 27)"
        );
        assert_eq!(
            baruch_typst::cover::template_functions(&book),
            baruch_typst::cover::TemplateFunctions {
                half_title: true,
                main_title: true,
                imprint: true,
                dedication: true
            },
            "ein neues Buch kennt alle vier Titelei-Funktionen, sonst bliebe die Titelei beim Rückfall"
        );
        assert_eq!(
            preset_template(Preset::Sermon, "Predigt", ""),
            "#set document(title: \"Predigt\", author: \"\")\n\n",
            "Predigt: nur #set document"
        );
    }
}
