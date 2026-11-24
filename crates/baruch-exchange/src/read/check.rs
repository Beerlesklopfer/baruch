// SPDX-License-Identifier: AGPL-3.0-or-later
//! Inhaltliche Prüfung eines gelesenen Dokuments ([`super::validate`], Vertrag R26 S1 (9)).
//!
//! - Alle Kennungen kanonisch ([`is_uuid`]) und im ganzen Dokument eindeutig (auch kein Grabstein mit der
//!   Kennung eines vorhandenen Stücks).
//! - Verweise: `basis` auf eine Fassung desselben Punkts bzw. Bilds mit kleinerer Nummer, Bezüge von
//!   Beiträgen und Einfällen auf Punkte (und Fassungen dieses Punkts) im Dokument, Umschlag und Autorenfoto auf
//!   Bilder im Dokument.
//! - Je Punkt mindestens eine und genau eine aktuelle Fassung; je Bild genau eine aktuelle Fassung; Nummern je
//!   Punkt, Bild und Vorlage eindeutig und ab 1; Positionen von Beiträgen je Punkt bzw. Faden eindeutig und ab 1.
//! - Gliederung: Teile nur auf oberster Ebene, Kapitel oben oder unter einem Teil, Abschnitte unter Kapitel oder
//!   Abschnitt, höchstens Ebene 3.
//! - Bilder: Typ des Formats, SHA-256 passt zu den Bytes; erledigte Einfälle nennen, wozu (`erledigt-als`).
//! - Karte (ab Unterversion 2): Verbindungen und Lagen nur zu Einfällen des Dokuments ([`check_map`]).
//! - Umschlag-Anordnung (ab Unterversion 3, [`check_cover`]): je Art höchstens ein Block, Grenzen wie die CHECKs der
//!   Migration 0045, `richtung` genau auf dem Rücken, `<text>` nur bei Arten mit eigenem Text, Schriftnamen nach der
//!   Regel der Satzbild-Schriften; Anschriften höchstens 500 Zeichen.
//! - Befunde (ab Unterversion 4, [`check_findings`]): Herkunft auf Punkte, Beiträge und Fadenbeiträge des Dokuments
//!   (nie Beitrag und Fadenbeitrag zugleich), `gleicht` und `sammelt` auf Befunde des Dokuments, Grenzen und Regeln
//!   wie die CHECKs der Migration 0046 (verworfen nur mit Begründung, übernommen nur mit Fassung, verschoben mit
//!   genau einem Ziel; im Verlauf setzt ohne Vorschlag nur der Mensch, entschieden genau mit Zeitpunkt). Alte
//!   Fassungen von Beiträgen tragen eigene Kennungen.
//! - Unterversion 5 ([`check_u5`]): Varianten (Gruppe, Etikett A/B/C, Zeile, `setzt-um` auf Befunde des Dokuments),
//!   Metadaten der Beiträge (Grundlage eine Fassung desselben Punkts, Antwort/Wiederholung ein anderer Beitrag
//!   derselben Stelle) und Lösungen der Befunde (Zitat mit Ersatz oder Schritt).

use std::collections::{HashMap, HashSet};

use crate::ExchangeError;
use crate::model::*;

fn invalid(message: impl Into<String>) -> ExchangeError {
    ExchangeError::Invalid(message.into())
}

struct Seen {
    all: HashSet<String>,
}

impl Seen {
    fn add(&mut self, uuid: &str, what: &str) -> Result<(), ExchangeError> {
        if !is_uuid(uuid) {
            return Err(invalid(format!("{what}: „{uuid}“ ist keine kanonische UUID (klein, 8-4-4-4-12)")));
        }
        if !self.all.insert(uuid.to_owned()) {
            return Err(invalid(format!("{what}: Kennung {uuid} kommt doppelt vor")));
        }
        Ok(())
    }
}

fn check_ref(uuid: &str, what: &str) -> Result<(), ExchangeError> {
    if is_uuid(uuid) { Ok(()) } else { Err(invalid(format!("{what}: Verweis „{uuid}“ ist keine kanonische UUID"))) }
}

/// Punkt-Kennung → (Fassungs-Kennungen) für Bezüge.
type NodeIndex = HashMap<String, HashSet<String>>;

fn walk<'a>(
    nodes: &'a [ExchangeNode],
    parent: Option<(ExchangeNodeKind, u8)>,
    seen: &mut Seen,
    index: &mut NodeIndex,
    all_nodes: &mut Vec<&'a ExchangeNode>,
) -> Result<(), ExchangeError> {
    for node in nodes {
        let what = format!("Punkt „{}“", node.title);
        seen.add(&node.uuid, &what)?;
        let level = match (node.kind, parent) {
            (ExchangeNodeKind::Part, None) => 0,
            (ExchangeNodeKind::Part, Some(_)) => {
                return Err(invalid(format!("{what}: ein Teil steht nur auf oberster Ebene")));
            }
            (ExchangeNodeKind::Chapter, None | Some((ExchangeNodeKind::Part, _))) => 1,
            (ExchangeNodeKind::Chapter, Some(_)) => {
                return Err(invalid(format!("{what}: ein Kapitel steht oben oder unter einem Teil")));
            }
            (ExchangeNodeKind::Section, Some((ExchangeNodeKind::Chapter | ExchangeNodeKind::Section, level))) => {
                level + 1
            }
            (ExchangeNodeKind::Section, _) => {
                return Err(invalid(format!("{what}: ein Abschnitt steht unter einem Kapitel oder Abschnitt")));
            }
        };
        if level > 3 {
            return Err(invalid(format!("{what}: tiefer als Ebene 3")));
        }
        if node.revisions.is_empty() {
            return Err(invalid(format!("{what}: keine Fassung")));
        }
        let current = node.revisions.iter().filter(|r| r.status == ExchangeRevisionStatus::Current).count();
        if current != 1 {
            return Err(invalid(format!("{what}: {current} aktuelle Fassungen statt genau einer")));
        }
        let mut numbers: HashMap<&str, u32> = HashMap::new();
        let mut used_numbers = HashSet::new();
        for revision in &node.revisions {
            seen.add(&revision.uuid, &format!("{what}, Fassung {}", revision.number))?;
            if revision.number == 0 || !used_numbers.insert(revision.number) {
                return Err(invalid(format!("{what}: Fassungsnummer {} doppelt oder 0", revision.number)));
            }
            numbers.insert(revision.uuid.as_str(), revision.number);
            check_amends(revision, &what, seen)?;
        }
        for revision in &node.revisions {
            if let Some(base) = &revision.based_on {
                check_ref(base, &what)?;
                match numbers.get(base.as_str()) {
                    Some(&n) if n < revision.number => {}
                    Some(_) => {
                        return Err(invalid(format!(
                            "{what}: Fassung {} beruht auf einer Fassung mit größerer Nummer",
                            revision.number
                        )));
                    }
                    None => {
                        return Err(invalid(format!(
                            "{what}: basis {base} von Fassung {} ist keine Fassung dieses Punkts",
                            revision.number
                        )));
                    }
                }
            }
        }
        index.insert(node.uuid.clone(), node.revisions.iter().map(|r| r.uuid.clone()).collect());
        all_nodes.push(node);
        walk(&node.children, Some((node.kind, level)), seen, index, all_nodes)?;
    }
    Ok(())
}

/// Unterversion 8 (Runde 22): Verlauf der Fortschreibungen einer Fassung – eigene Kennungen, Schritte ab 1 lückenlos
/// in Reihenfolge, genau die Rücknahmen (`undo`) nennen einen früheren Schritt (`undo-von`, wie der CHECK der
/// Migration 0050), und der Stand der Fassung ist die Zahl der Schritte.
///
/// Ob die Texte zusammenpassen (nachher eines Schritts = vorher des nächsten, letzter Schritt = Text der Fassung),
/// prüft das Lesen bewusst nicht: Das Ändern über den Baum (`edited`) ändert den Text einer Fassung ohne Schritt im
/// Verlauf, eine gültige Datei kann also Lücken haben. Maßgeblich ist das Anwenden: Vorgespult wird nur, wenn die
/// fehlenden Schritte die Änderung lückenlos wiedergeben (`baruch_store::exchange_apply`, `fast_forward`), sonst
/// kommt der Text als abweichende Fassung.
fn check_amends(revision: &ExchangeRevision, what: &str, seen: &mut Seen) -> Result<(), ExchangeError> {
    let label = format!("{what}, Fassung {}", revision.number);
    for (i, step) in revision.amends.iter().enumerate() {
        seen.add(&step.uuid, &format!("{label}, Fortschreibung {}", step.seq))?;
        // Wie der CHECK der Migration 0050 (`author_name` 1–200 Zeichen): sonst bräche erst das Schreiben ab.
        let author_len = step.author.trim().chars().count();
        if author_len == 0 || step.author.chars().count() > 200 {
            return Err(invalid(format!(
                "{label}: Fortschreibung {}: autor muss 1 bis 200 Zeichen lang sein",
                step.seq
            )));
        }
        if step.seq as usize != i + 1 {
            return Err(invalid(format!(
                "{label}: Fortschreibungen müssen lückenlos ab 1 stehen (Schritt {} an Stelle {})",
                step.seq,
                i + 1
            )));
        }
        match (step.kind, step.undo_of) {
            (ExchangeAmendKind::Amend, None) => {}
            (ExchangeAmendKind::Undo, Some(of)) if of >= 1 && of < step.seq => {}
            _ => {
                return Err(invalid(format!(
                    "{label}: Fortschreibung {}: undo-von steht genau bei einer Rücknahme und nennt einen früheren \
                     Schritt",
                    step.seq
                )));
            }
        }
    }
    if revision.amend_seq as usize != revision.amends.len() {
        return Err(invalid(format!(
            "{label}: stand {} passt nicht zu {} Fortschreibungen",
            revision.amend_seq,
            revision.amends.len()
        )));
    }
    Ok(())
}

fn positions(items: impl Iterator<Item = u32>, what: &str) -> Result<(), ExchangeError> {
    let mut used = HashSet::new();
    for position in items {
        if position == 0 || !used.insert(position) {
            return Err(invalid(format!("{what}: Position {position} doppelt oder 0")));
        }
    }
    Ok(())
}

/// `<karte>` (Runde 25, Vertrag §2/§4.6): Verbindungen nur zwischen zwei verschiedenen Einfällen des Dokuments,
/// je Paar und Art höchstens eine („widerspricht“ ungerichtet, also auch nicht umgekehrt doppelt), Notiz höchstens
/// [`MAX_LINK_NOTE_CHARS`] Zeichen ohne Steuerzeichen außer Zeilenumbruch; Lagen nur für Einfälle des Dokuments,
/// je Einfall höchstens eine, Koordinaten endlich und höchstens ±[`MAP_COORD_LIMIT`].
fn check_map(work: &ExchangeWork, seen: &mut Seen) -> Result<(), ExchangeError> {
    let ideas: HashSet<&str> = work.ideas.iter().map(|i| i.uuid.as_str()).collect();
    let mut pairs = HashSet::new();
    for link in &work.map.links {
        let what = format!("Verbindung {}", link.uuid);
        seen.add(&link.uuid, "Verbindung")?;
        for end in [&link.from, &link.to] {
            check_ref(end, &what)?;
            if !ideas.contains(end.as_str()) {
                return Err(invalid(format!("{what}: Einfall {end} steht nicht im Dokument")));
            }
        }
        if link.from == link.to {
            return Err(invalid(format!("{what}: ein Einfall verbindet sich nicht mit sich selbst")));
        }
        let (a, b) = if link.kind == ExchangeLinkKind::Contradicts && link.to < link.from {
            (&link.to, &link.from)
        } else {
            (&link.from, &link.to)
        };
        if !pairs.insert((a.as_str(), b.as_str(), link.kind)) {
            return Err(invalid(format!("{what}: dasselbe Paar mit derselben Art steht doppelt im Dokument")));
        }
        let chars = link.note.chars().count();
        if chars > MAX_LINK_NOTE_CHARS {
            return Err(invalid(format!("{what}: Notiz mit {chars} Zeichen (höchstens {MAX_LINK_NOTE_CHARS})")));
        }
        if link.note.chars().any(|c| c.is_control() && c != '\n') {
            return Err(invalid(format!("{what}: Steuerzeichen in der Notiz (nur Zeilenumbruch erlaubt)")));
        }
    }
    let mut placed = HashSet::new();
    for position in &work.map.positions {
        let what = format!("Lage {}", position.uuid);
        seen.add(&position.uuid, "Lage")?;
        check_ref(&position.idea, &what)?;
        if !ideas.contains(position.idea.as_str()) {
            return Err(invalid(format!("{what}: Einfall {} steht nicht im Dokument", position.idea)));
        }
        if !placed.insert(position.idea.as_str()) {
            return Err(invalid(format!("{what}: Einfall {} hat schon eine Lage", position.idea)));
        }
        for value in [position.x, position.y] {
            if !value.is_finite() || value.abs() > MAP_COORD_LIMIT {
                return Err(invalid(format!("{what}: Koordinate {value} außerhalb von ±{MAP_COORD_LIMIT}")));
            }
        }
    }
    Ok(())
}

/// `<umschlag>` und Anschriften in `<buch>` (Runde 27, Vertrag 1.1–1.3 und 1.7).
fn check_cover(book: &ExchangeBook, seen: &mut Seen) -> Result<(), ExchangeError> {
    for (value, what) in
        [(&book.publisher_address, "Anschrift des Verlags"), (&book.printer_address, "Anschrift der Druckerei")]
    {
        let chars = value.chars().count();
        if chars > MAX_ADDRESS_CHARS {
            return Err(invalid(format!("{what}: {chars} Zeichen (höchstens {MAX_ADDRESS_CHARS})")));
        }
    }
    let mut kinds = HashSet::new();
    for block in &book.cover_blocks {
        let what = format!("Umschlag-Block {} ({})", block.uuid, block.kind.as_xml());
        seen.add(&block.uuid, "Umschlag-Block")?;
        if !kinds.insert(block.kind) {
            return Err(invalid(format!("{what}: die Art {} steht doppelt", block.kind.as_xml())));
        }
        for (value, range, field) in [
            (block.x_mm, COVER_POS_MM, "x-mm"),
            (block.y_mm, COVER_POS_MM, "y-mm"),
            (block.width_mm, COVER_WIDTH_MM, "breite-mm"),
            (block.size_pt, COVER_SIZE_PT, "groesse-pt"),
            (block.leading_em, COVER_LEADING_EM, "zeilenabstand-em"),
        ] {
            if !value.is_finite() || value < range.0 || value > range.1 {
                return Err(invalid(format!("{what}: {field} {value} außerhalb von {} bis {}", range.0, range.1)));
            }
        }
        if block.kind.is_spine() != block.direction.is_some() {
            return Err(invalid(format!("{what}: richtung gibt es genau bei den Blöcken auf dem Rücken")));
        }
        match &block.text {
            Some(_) if !block.kind.own_text() => {
                return Err(invalid(format!("{what}: <text> nur bei Freitexten und Motto")));
            }
            Some(text) if text.chars().count() > COVER_MAX_TEXT_CHARS => {
                return Err(invalid(format!("{what}: Text länger als {COVER_MAX_TEXT_CHARS} Zeichen")));
            }
            _ => {}
        }
        if let Some(font) = &block.font {
            let ok = !font.trim().is_empty()
                && font.chars().count() <= COVER_MAX_FONT_CHARS
                && font.chars().all(|c| c.is_alphanumeric() || " -_.".contains(c));
            if !ok {
                return Err(invalid(format!(
                    "{what}: Schriftname „{font}“ ungültig (Buchstaben, Ziffern, Leerzeichen, - _ ., höchstens \
                     {COVER_MAX_FONT_CHARS} Zeichen)"
                )));
            }
        }
    }
    Ok(())
}

/// Status samt Pflichtangaben (wie die CHECKs der Migration 0046) – für Befund und Verlauf gleich.
fn check_status(
    what: &str,
    status: ExchangeFindingStatus,
    reason: &str,
    adopted: &Option<String>,
    node: &Option<String>,
    idea: &Option<String>,
) -> Result<(), ExchangeError> {
    for (value, name) in [(adopted, "fassung"), (node, "ziel-punkt"), (idea, "ziel-einfall")] {
        if let Some(uuid) = value {
            check_ref(uuid, &format!("{what}, {name}"))?;
        }
    }
    let chars = reason.chars().count();
    if chars > MAX_FINDING_REASON_CHARS {
        return Err(invalid(format!("{what}: Begründung mit {chars} Zeichen (höchstens {MAX_FINDING_REASON_CHARS})")));
    }
    if status == ExchangeFindingStatus::Rejected && reason.trim().is_empty() {
        return Err(invalid(format!("{what}: verworfen ohne Begründung")));
    }
    if status == ExchangeFindingStatus::Adopted && adopted.is_none() {
        return Err(invalid(format!("{what}: übernommen ohne Fassung")));
    }
    if status == ExchangeFindingStatus::Deferred && node.is_some() == idea.is_some() {
        return Err(invalid(format!("{what}: verschoben braucht genau ein Ziel (ziel-punkt oder ziel-einfall)")));
    }
    Ok(())
}

/// `<befunde>` (Runde 28, Unterversion 4).
fn check_findings(
    work: &ExchangeWork,
    nodes: &NodeIndex,
    posts: &HashSet<&str>,
    thread_posts: &HashSet<&str>,
    seen: &mut Seen,
) -> Result<(), ExchangeError> {
    let ids: HashSet<&str> = work.findings.iter().map(|f| f.uuid.as_str()).collect();
    for finding in &work.findings {
        let what = format!("Befund {}", finding.uuid);
        seen.add(&finding.uuid, "Befund")?;
        if let Some(node) = &finding.node {
            check_ref(node, &what)?;
            if !nodes.contains_key(node) {
                return Err(invalid(format!("{what}: Punkt {node} steht nicht im Dokument")));
            }
        }
        if finding.post.is_some() && finding.thread_post.is_some() {
            return Err(invalid(format!("{what}: stammt aus einem Beitrag oder einem Fadenbeitrag, nicht aus beiden")));
        }
        if let Some(post) = &finding.post {
            check_ref(post, &what)?;
            if !posts.contains(post.as_str()) {
                return Err(invalid(format!("{what}: Beitrag {post} steht nicht im Dokument")));
            }
        }
        if let Some(post) = &finding.thread_post {
            check_ref(post, &what)?;
            if !thread_posts.contains(post.as_str()) {
                return Err(invalid(format!("{what}: Fadenbeitrag {post} steht nicht im Dokument")));
            }
        }
        if let Some(original) = &finding.similar_to {
            check_ref(original, &what)?;
            if !ids.contains(original.as_str()) || original == &finding.uuid {
                return Err(invalid(format!(
                    "{what}: gleicht {original}, der nicht (als anderer Befund) im Dokument steht"
                )));
            }
        }
        for summarized in &finding.summary_of {
            check_ref(summarized, &what)?;
        }
        let persona = finding.persona.chars().count();
        if persona == 0 || persona > MAX_FINDING_PERSONA_CHARS {
            return Err(invalid(format!("{what}: Persona mit {persona} Zeichen (1 bis {MAX_FINDING_PERSONA_CHARS})")));
        }
        let body = finding.body.chars().count();
        if finding.body.trim().is_empty() || body > MAX_FINDING_CHARS {
            return Err(invalid(format!("{what}: Text leer oder länger als {MAX_FINDING_CHARS} Zeichen")));
        }
        if finding.quote.chars().count() > MAX_FINDING_QUOTE_CHARS {
            return Err(invalid(format!("{what}: Zitat länger als {MAX_FINDING_QUOTE_CHARS} Zeichen")));
        }
        if finding.quote_check.chars().count() > MAX_FINDING_CHECK_CHARS {
            return Err(invalid(format!("{what}: Prüfvermerk länger als {MAX_FINDING_CHECK_CHARS} Zeichen")));
        }
        check_status(
            &what,
            finding.status,
            &finding.reason,
            &finding.adopted_revision,
            &finding.deferred_node,
            &finding.deferred_idea,
        )?;
        for entry in &finding.history {
            let what = format!("{what}, Verlaufseintrag {}", entry.uuid);
            seen.add(&entry.uuid, &what)?;
            check_status(
                &what,
                entry.status,
                &entry.reason,
                &entry.adopted_revision,
                &entry.deferred_node,
                &entry.deferred_idea,
            )?;
            if !entry.proposed && entry.actor_kind != ExchangeOriginKind::Human {
                return Err(invalid(format!("{what}: den Status setzt ohne Vorschlag nur der Mensch")));
            }
            if !entry.proposed && entry.decision.is_some() {
                return Err(invalid(format!("{what}: eine Entscheidung gibt es nur zu einem Vorschlag")));
            }
            if entry.decision.is_some() != entry.decided.is_some() {
                return Err(invalid(format!("{what}: entscheidung und entschieden gehören zusammen")));
            }
            if entry.note.chars().count() > MAX_FINDING_REASON_CHARS {
                return Err(invalid(format!("{what}: Notiz länger als {MAX_FINDING_REASON_CHARS} Zeichen")));
            }
        }
    }
    Ok(())
}

/// Metadaten eines Beitrags (ab Unterversion 5): Grundlage eine Fassung derselben Stelle, Antwort auf und
/// Wiederholung von ein anderer Beitrag derselben Stelle, Längen wie die CHECKs der Migration 0047.
fn check_meta(
    meta: &ExchangePostMeta,
    own: &str,
    posts: &HashSet<&str>,
    revisions: Option<&HashSet<&str>>,
    what: &str,
) -> Result<(), ExchangeError> {
    match (&meta.basis_revision, revisions) {
        (Some(rev), Some(revisions)) => {
            check_ref(rev, what)?;
            if !revisions.contains(rev.as_str()) {
                return Err(invalid(format!("{what}: grundlage-fassung {rev} ist keine Fassung dieses Punkts")));
            }
        }
        (Some(_), None) => return Err(invalid(format!("{what}: ein Fadenbeitrag hat keine grundlage-fassung"))),
        (None, _) => {}
    }
    if meta.basis_label.chars().count() > MAX_BASIS_LABEL_CHARS {
        return Err(invalid(format!("{what}: grundlage länger als {MAX_BASIS_LABEL_CHARS} Zeichen")));
    }
    for (name, target) in [("antwort-auf", &meta.reply_to), ("wiederholt", &meta.repeat_of)] {
        if let Some(target) = target {
            check_ref(target, what)?;
            if target == own || !posts.contains(target.as_str()) {
                return Err(invalid(format!("{what}: {name} {target} ist kein anderer Beitrag derselben Stelle")));
            }
        }
    }
    if let Some(relay) = &meta.relay_from {
        let n = relay.chars().count();
        if n == 0 || n > MAX_RELAY_FROM_CHARS {
            return Err(invalid(format!("{what}: relay-von braucht 1 bis {MAX_RELAY_FROM_CHARS} Zeichen")));
        }
    }
    Ok(())
}

/// Unterversion 5 (Runde 30): Varianten der Fassungen (Gruppe als UUID, Etikett A/B/C, Zeile höchstens
/// [`MAX_VARIANT_SUMMARY_CHARS`], `setzt-um` auf Befunde des Dokuments), Metadaten der Beiträge ([`check_meta`]) und
/// Lösungen der Befunde (Zitat mit Formulierung oder ein Schritt, Längen wie `finding`).
/// Eigene Register und Glossar (ab Unterversion 7): Kennungen eindeutig, Register-Kennung gültig und nicht eingebaut,
/// je Werk eindeutig und höchstens 13; Begriff nicht leer, unter den angenommenen eindeutig (gleicher Schlüssel wie
/// `baruch_typst::index::term_key`, sonst ließe sich die Datei nicht einlesen). Runde 23: Der Titel eines Registers
/// hat (in einer Zeile, Leerraum zusammengezogen – so speichert ihn das Einlesen) höchstens
/// `baruch_typst::index::MAX_REGISTER_TITLE_CHARS` Zeichen, wie beim Anlegen in der Oberfläche. Die Zahl der
/// Glossarbegriffe zählt das Einlesen selbst (`baruch_store::MAX_GLOSSARY_TERMS`; diese Crate kennt den Speicher nicht).
fn check_u7(work: &ExchangeWork, seen: &mut Seen) -> Result<(), ExchangeError> {
    use baruch_typst::index::{
        MAX_CUSTOM_REGISTERS, MAX_REGISTER_TITLE_CHARS, builtin_registers, term_key, valid_register_slug,
    };
    let builtin: Vec<String> = builtin_registers().into_iter().map(|r| r.slug).collect();
    let mut slugs = HashSet::new();
    for register in &work.registers {
        let what = format!("Register {}", register.slug);
        seen.add(&register.uuid, &what)?;
        if !valid_register_slug(&register.slug) || builtin.contains(&register.slug) {
            return Err(invalid(format!("{what}: keine gültige Kennung eines eigenen Registers")));
        }
        if !slugs.insert(register.slug.as_str()) {
            return Err(invalid(format!("{what}: Kennung doppelt")));
        }
        if register.title.trim().is_empty() || register.created.is_empty() {
            return Err(invalid(format!("{what}: Titel oder angelegt ist leer")));
        }
        let chars = register.title.split_whitespace().collect::<Vec<_>>().join(" ").chars().count();
        if chars > MAX_REGISTER_TITLE_CHARS {
            return Err(invalid(format!(
                "{what}: Titel hat {chars} Zeichen (höchstens {MAX_REGISTER_TITLE_CHARS}); nichts eingelesen"
            )));
        }
    }
    if work.registers.len() > MAX_CUSTOM_REGISTERS {
        return Err(invalid(format!(
            "höchstens {MAX_CUSTOM_REGISTERS} eigene Register, nicht {}",
            work.registers.len()
        )));
    }
    let mut keys = HashSet::new();
    for term in &work.glossary {
        let what = format!("Glossarbegriff „{}“", term.term);
        seen.add(&term.uuid, &what)?;
        let key = term_key(&term.term);
        if key.is_empty() || term.created.is_empty() || term.origin.trim().is_empty() {
            return Err(invalid(format!("{what}: Begriff, Herkunft oder angelegt ist leer")));
        }
        if term.status == ExchangeGlossaryStatus::Accepted && !keys.insert(key) {
            return Err(invalid(format!("{what}: doppelt unter den angenommenen Begriffen")));
        }
    }
    Ok(())
}

fn check_u5(work: &ExchangeWork) -> Result<(), ExchangeError> {
    let findings: HashSet<&str> = work.findings.iter().map(|f| f.uuid.as_str()).collect();
    fn nodes<'a>(list: &'a [ExchangeNode], out: &mut Vec<&'a ExchangeNode>) {
        for node in list {
            out.push(node);
            nodes(&node.children, out);
        }
    }
    let mut all = Vec::new();
    nodes(&work.outline, &mut all);
    for node in all {
        let what = format!("Punkt „{}“", node.title);
        let revisions: HashSet<&str> = node.revisions.iter().map(|r| r.uuid.as_str()).collect();
        for revision in &node.revisions {
            let Some(variant) = &revision.variant else { continue };
            let here = format!("{what}, Fassung {}", revision.number);
            check_ref(&variant.group, &format!("{here}, variante-gruppe"))?;
            if !VARIANT_LABELS.contains(&variant.label.as_str()) {
                return Err(invalid(format!("{here}: variante „{}“ ist nicht A, B oder C", variant.label)));
            }
            if variant.summary.chars().count() > MAX_VARIANT_SUMMARY_CHARS {
                return Err(invalid(format!("{here}: variante-zeile länger als {MAX_VARIANT_SUMMARY_CHARS} Zeichen")));
            }
            for target in &variant.findings {
                check_ref(target, &here)?;
                if !findings.contains(target.as_str()) {
                    return Err(invalid(format!("{here}: setzt-um nennt unbekannten Befund {target}")));
                }
            }
        }
        let posts: HashSet<&str> = node.posts.iter().map(|p| p.uuid.as_str()).collect();
        for post in &node.posts {
            let here = format!("Beitrag {} zu „{}“", post.position, node.title);
            check_meta(&post.meta, &post.uuid, &posts, Some(&revisions), &here)?;
        }
    }
    for thread in &work.threads {
        let posts: HashSet<&str> = thread.posts.iter().map(|p| p.uuid.as_str()).collect();
        for post in &thread.posts {
            let here = format!("Faden „{}“, Beitrag {}", thread.title, post.position);
            check_meta(&post.meta, &post.uuid, &posts, None, &here)?;
        }
    }
    for finding in &work.findings {
        for solution in &finding.solutions {
            let what = format!("Befund {}, Lösung", finding.uuid);
            if solution.quote.chars().count() > MAX_FINDING_QUOTE_CHARS
                || solution.replacement.chars().count() > MAX_FINDING_CHARS
                || solution.step.chars().count() > MAX_FINDING_CHARS
            {
                return Err(invalid(format!("{what}: zu lang")));
            }
            let replace = !solution.quote.trim().is_empty() && !solution.replacement.trim().is_empty();
            if !replace && solution.step.trim().is_empty() {
                return Err(invalid(format!("{what}: braucht zitat mit ersatz oder einen schritt")));
            }
        }
    }
    Ok(())
}

pub(crate) fn validate(doc: &ExchangeDocument) -> Result<(), ExchangeError> {
    let work = &doc.work;
    check_ref(&doc.instance.uuid, "Instanz")?;
    let mut seen = Seen { all: HashSet::new() };
    seen.add(&work.uuid, "Werk")?;

    let mut template_numbers = HashSet::new();
    for revision in &work.template.revisions {
        seen.add(&revision.uuid, &format!("Vorlagenfassung {}", revision.number))?;
        if revision.number == 0 || !template_numbers.insert(revision.number) {
            return Err(invalid(format!("Vorlagenfassung: Nummer {} doppelt oder 0", revision.number)));
        }
    }

    let mut index = NodeIndex::new();
    let mut nodes = Vec::new();
    walk(&work.outline, None, &mut seen, &mut index, &mut nodes)?;
    let mut post_ids: HashSet<&str> = HashSet::new();
    for node in nodes {
        positions(node.posts.iter().map(|p| p.position), &format!("Beiträge zu „{}“", node.title))?;
        for post in &node.posts {
            let what = format!("Beitrag {} zu „{}“", post.position, node.title);
            seen.add(&post.uuid, &what)?;
            post_ids.insert(post.uuid.as_str());
            for version in &post.versions {
                seen.add(&version.uuid, &format!("{what}, alte Fassung"))?;
            }
            for reference in &post.refs {
                check_ref(&reference.node, &what)?;
                let Some(revisions) = index.get(&reference.node) else {
                    return Err(invalid(format!("{what}: Bezug auf unbekannten Punkt {}", reference.node)));
                };
                if let Some(revision) = &reference.revision {
                    check_ref(revision, &what)?;
                    if !revisions.contains(revision) {
                        return Err(invalid(format!(
                            "{what}: Bezug auf Fassung {revision}, die nicht zum Punkt gehört"
                        )));
                    }
                }
            }
        }
    }

    for idea in &work.ideas {
        let what = format!("Einfall {}", idea.uuid);
        seen.add(&idea.uuid, "Einfall")?;
        if let Some(node) = &idea.node {
            check_ref(node, &what)?;
            if !index.contains_key(node) {
                return Err(invalid(format!("{what}: Bezug auf unbekannten Punkt {node}")));
            }
        }
        if (idea.status == ExchangeIdeaStatus::Done) != idea.done_as.is_some() {
            return Err(invalid(format!("{what}: erledigt-als passt nicht zum Status")));
        }
    }

    let mut thread_post_ids: HashSet<&str> = HashSet::new();
    for thread in &work.threads {
        let what = format!("Faden „{}“", thread.title);
        seen.add(&thread.uuid, &what)?;
        positions(thread.posts.iter().map(|p| p.position), &what)?;
        for post in &thread.posts {
            seen.add(&post.uuid, &format!("{what}, Beitrag {}", post.position))?;
            thread_post_ids.insert(post.uuid.as_str());
            for version in &post.versions {
                seen.add(&version.uuid, &format!("{what}, Beitrag {}, alte Fassung", post.position))?;
            }
            if post.body.trim().is_empty() {
                return Err(invalid(format!("{what}: Beitrag {} ist leer", post.position)));
            }
        }
    }

    let mut asset_ids = HashSet::new();
    let mut names = HashSet::new();
    for asset in &work.assets {
        let what = format!("Bild „{}“", asset.name);
        seen.add(&asset.uuid, &what)?;
        asset_ids.insert(asset.uuid.as_str());
        if !names.insert(asset.name.as_str()) {
            return Err(invalid(format!("{what}: Name doppelt")));
        }
        if asset.revisions.is_empty() {
            return Err(invalid(format!("{what}: keine Fassung")));
        }
        let current = asset.revisions.iter().filter(|r| r.status == ExchangeAssetStatus::Current).count();
        if current != 1 {
            return Err(invalid(format!("{what}: {current} aktuelle Fassungen statt genau einer")));
        }
        let mut numbers: HashMap<&str, u32> = HashMap::new();
        let mut used = HashSet::new();
        for revision in &asset.revisions {
            seen.add(&revision.uuid, &format!("{what}, Fassung {}", revision.number))?;
            if revision.number == 0 || !used.insert(revision.number) {
                return Err(invalid(format!("{what}: Fassungsnummer {} doppelt oder 0", revision.number)));
            }
            if image_extension(&revision.mime).is_none() {
                return Err(invalid(format!("{what}: Typ {} gehört nicht zum Format", revision.mime)));
            }
            if revision.sha256 != sha256_hex(&revision.data) {
                return Err(ExchangeError::Checksum {
                    path: format!("Bild {}, Fassung {}", asset.name, revision.number),
                });
            }
            numbers.insert(revision.uuid.as_str(), revision.number);
        }
        for revision in &asset.revisions {
            if let Some(base) = &revision.based_on {
                check_ref(base, &what)?;
                if !numbers.get(base.as_str()).is_some_and(|&n| n < revision.number) {
                    return Err(invalid(format!("{what}: basis von Fassung {} passt nicht", revision.number)));
                }
            }
        }
    }
    if let Some(book) = &work.book {
        check_cover(book, &mut seen)?;
        for (uuid, what) in [(&book.cover, "Umschlag"), (&book.author_photo, "Autorenfoto")] {
            if let Some(uuid) = uuid {
                check_ref(uuid, what)?;
                if !asset_ids.contains(uuid.as_str()) {
                    return Err(invalid(format!("{what}: Bild {uuid} fehlt in <bilder>")));
                }
            }
        }
    }

    if let Some(typeset) = &work.typeset {
        seen.add(&typeset.uuid, "Satz-Einstellungen")?;
        if typeset.updated.is_empty() {
            return Err(invalid("Satz-Einstellungen: geaendert ist leer"));
        }
    }
    check_map(work, &mut seen)?;
    check_findings(work, &index, &post_ids, &thread_post_ids, &mut seen)?;
    check_u5(work)?;
    check_u7(work, &mut seen)?;

    let mut tombstones = HashSet::new();
    for tombstone in &work.deleted {
        check_ref(&tombstone.uuid, "Grabstein")?;
        if seen.all.contains(&tombstone.uuid) {
            return Err(invalid(format!(
                "Grabstein {}: dieselbe Kennung ist im Dokument noch vorhanden",
                tombstone.uuid
            )));
        }
        if !tombstones.insert(tombstone.uuid.as_str()) {
            return Err(invalid(format!("Grabstein {} doppelt", tombstone.uuid)));
        }
    }
    Ok(())
}
