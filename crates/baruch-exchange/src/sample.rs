// SPDX-License-Identifier: AGPL-3.0-or-later
//! Erfundenes Beispielwerk für Tests des Austauschformats (Vertrag R26, S1 (7)); alle Texte sind frei erfunden.
//!
//! Buch mit zwei Teilen, drei Kapiteln und einem Abschnitt (das Kapitel „Das Seil“ ist nicht nummeriert). Das
//! Kapitel „Der Eimer“ hat vier Fassungen (aktuell, abgelöst mit Basis, Vorschlag des Assistenten mit Modell,
//! verworfen); die aktuelle trägt führende und folgende Leerzeichen, Tab, `<&>`, `]]>`, Umlaute und `\r`, genau
//! der Vorschlag ein in XML 1.0 verbotenes Steuerzeichen (U+0001). Dazu Vorlage mit zwei Fassungen,
//! Buchangaben mit Umschlag, Sprechdauer, zwei SWORD-Quellen, drei Einfälle (erledigt, gelöscht, mit Bezug),
//! zwei Beiträge mit Quelle und Bezug, ein Faden mit zwei Beiträgen, zwei Bilder (PNG 1×1 mit zwei Fassungen,
//! SVG) und ein Grabstein. Seit Runde 25 dazu die Karte (Unterversion 2): eine Verbindung „führt weiter“ und eine
//! gelöschte „widerspricht“ mit Notiz (Zeilenumbruch, `&`, `<`), zwei Lagen (eine auf der Grenze −1 000 000, eine
//! mit einer Zahl, die nur ihre kürzeste Darstellung genau trifft) und je ein Grabstein `verbindung` und `lage`.
//! Seit Runde 27 (Unterversion 3): Anschriften des Verlags und der Druckerei, drei Umschlag-Blöcke (Titel vorn,
//! Titel auf dem Rücken aufwärts, Motto mit eigenem Text im Beschnitt), Satzbild-Einstellungen und ein Grabstein
//! `umschlag-block`. Seit Runde 28 (Unterversion 4): je ein bearbeiteter Beitrag im Abschnitt und im Faden mit alter
//! Fassung, fünf Befunde (verworfen mit Begründung und veraltetem Vorschlag, übernommen in eine Fassung über einen
//! bestätigten Vorschlag, verschoben in einen Einfall, ein Hinweis im Faden mit Anhang und Sammlung, ein Lob ohne
//! Beitrag) und je ein Grabstein `befund`, `befund-status`, `beitrag-fassung`. Seit Runde 29 (Unterversion 6):
//! Satz-Einstellungen mit allen sechzehn Feldern ([`crate::typeset::full_sample`]) und ein Grabstein `satz` (früher
//! zurückgesetzte Einstellungen). Seit Runde 31 (Unterversion 7): zwei eigene Register, drei Glossarbegriffe (zwei
//! angenommen, davon einer mit allen Feldern, einer vorgeschlagen von @lektor), die Register/Glossar-Felder der
//! Satz-Einstellungen und je ein Grabstein `glossar-begriff` und `register`. Feste UUIDs ([`sample_uuid`]) und Zeiten.
//!
//! Seit Runde 22 (Unterversion 8) gibt es zusätzlich [`sample_document_u8`]: dasselbe Werk, dazu drei Fortschreibungen
//! der aktuellen Fassung von „Der Eimer“ (fortgeschrieben, zurückgenommen, fortgeschrieben; Stand 3), ein Vorschlag
//! mit `beruht-stand`, der Stand am übernommenen Befund und seinem Verlaufseintrag, ein Entwurf in den
//! Satz-Einstellungen ([`crate::typeset::full_draft`]) und ein Grabstein `fortschreibung`. [`sample_document`] selbst
//! bleibt ohne diese Teile (ältere Tests bauen aus ihm Dateien der Unterversionen 1 bis 7).
//!
//! export darf hier nur ergänzen (Vertrag Abschnitt 6).

use crate::model::*;

/// Feste UUID Nummer `n` des Beispiels (`26000000-0000-4000-8000-<n als 12 Hex-Stellen>`).
pub fn sample_uuid(n: u32) -> String {
    format!("26000000-0000-4000-8000-{n:012x}")
}

/// Gültiges PNG 1×1 (grün, RGBA).
pub const SAMPLE_PNG_GREEN: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00,
    0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49,
    0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0xd0, 0xab, 0x35, 0xfa, 0x0f, 0x00, 0x03, 0x97, 0x01, 0xdd, 0x13, 0xb8, 0x4b,
    0x37, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

/// Gültiges PNG 1×1 (rot, RGBA).
pub const SAMPLE_PNG_RED: &[u8] = &[
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00,
    0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49,
    0x44, 0x41, 0x54, 0x78, 0xda, 0x63, 0x38, 0x60, 0xa9, 0xfd, 0x1f, 0x00, 0x05, 0x05, 0x02, 0x24, 0x87, 0xf1, 0x2c,
    0xb5, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

/// Kleines SVG (Lampe).
pub const SAMPLE_SVG: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\" height=\"10\" viewBox=\"0 0 10 10\">\
<circle cx=\"5\" cy=\"4\" r=\"3\" fill=\"#f5c542\"/><rect x=\"4\" y=\"7\" width=\"2\" height=\"3\" fill=\"#555\"/></svg>\n";

/// Aktuelle Fassung von „Der Eimer“: Leerzeichen vorn und hinten, Tab, `<&>`, `]]>`, Umlaute, `\r`.
pub const SAMPLE_TRICKY_BODY: &str =
    "  == Der Eimer\n\n\tDer Eimer hält <&> und schließt nie mit ]]> – Grüße aus Übersee.\r\nÄrger, Öl, Füße.  ";

/// Vorschlag von „Der Eimer“ mit dem verbotenen Steuerzeichen U+0001.
pub const SAMPLE_CONTROL_BODY: &str = "== Der Eimer\n\nDer Eimer \u{1}klirrt leise am Rand des Brunnens.\n";

fn t(day: u32, hour: u32) -> String {
    format!("2026-01-{day:02}T{hour:02}:00:00.000Z")
}

fn human(name: &str) -> ExchangeAuthor {
    ExchangeAuthor::Human { name: name.to_owned() }
}

fn revision(n: u32, number: u32, status: ExchangeRevisionStatus, body: &str, day: u32) -> ExchangeRevision {
    ExchangeRevision {
        uuid: sample_uuid(n),
        number,
        status,
        author: human("Hanna Probe"),
        based_on: None,
        note: String::new(),
        body: body.to_owned(),
        created: t(day, 9),
        edited: None,
        variant: None,
        amend_seq: 0,
        based_on_amend: None,
        amends: Vec::new(),
    }
}

fn node(n: u32, kind: ExchangeNodeKind, title: &str, heading: &str, rev: u32, day: u32) -> ExchangeNode {
    ExchangeNode {
        uuid: sample_uuid(n),
        kind,
        numbered: true,
        title: title.to_owned(),
        key: Some(i64::from(n)),
        revisions: vec![revision(
            rev,
            1,
            ExchangeRevisionStatus::Current,
            &format!("{heading} {title}\n\nErfundener Text zu „{title}“.\n"),
            day,
        )],
        posts: Vec::new(),
        children: Vec::new(),
    }
}

fn asset_revision(n: u32, number: u32, mime: &str, data: &[u8], status: ExchangeAssetStatus) -> ExchangeAssetRevision {
    ExchangeAssetRevision {
        uuid: sample_uuid(n),
        number,
        based_on: None,
        mime: mime.to_owned(),
        sha256: sha256_hex(data),
        data: data.to_vec(),
        caption: String::new(),
        alt: String::new(),
        author: "Hanna Probe".into(),
        note: String::new(),
        status,
        created: t(6, 10 + number),
        edited: None,
    }
}

/// Befunde des Beispielwerks (Unterversion 4).
fn sample_findings() -> Vec<ExchangeFinding> {
    use ExchangeFindingStatus as F;
    let lektor = |n: u32, position: u32, kind: ExchangeFindingKind, body: &str| ExchangeFinding {
        uuid: sample_uuid(n),
        node: Some(sample_uuid(11)),
        post: Some(sample_uuid(30)),
        position,
        persona: "@lektor".into(),
        kind,
        body: body.into(),
        created: t(7, 11),
        updated: t(7, 11),
        ..ExchangeFinding::default()
    };
    let entry = |n: u32, status: F, actor_kind: ExchangeOriginKind, actor: &str, hour: u32| ExchangeFindingEntry {
        uuid: sample_uuid(n),
        status,
        actor_kind,
        actor: actor.into(),
        proposed: actor_kind != ExchangeOriginKind::Human,
        created: t(8, hour),
        ..ExchangeFindingEntry::default()
    };
    let mut rejected = lektor(310, 0, ExchangeFindingKind::Critical, "„Der Eimer hält“ wirkt zu glatt & <knapp>.");
    rejected.quote = "Der Eimer hält".into();
    rejected.quote_check = "(nicht im Text belegt)".into();
    rejected.status = F::Rejected;
    rejected.reason = "Absicht: der Eimer bleibt glatt.
Zweite Zeile."
        .into();
    rejected.updated = t(8, 2);
    let mut stale = entry(321, F::Done, ExchangeOriginKind::Persona, "@lektor", 1);
    stale.note = "Erledigt sich mit Fassung 3.".into();
    stale.decision = Some(ExchangeDecision::Stale);
    stale.decided = Some(t(8, 3));
    stale.decided_by = Some("Hanna Probe".into());
    let mut set = entry(320, F::Rejected, ExchangeOriginKind::Human, "Hanna Probe", 2);
    set.reason = rejected.reason.clone();
    rejected.history = vec![stale, set];
    // Unterversion 5 (Runde 30): Lösungen je Befund – eine Ersetzung mit Sonderzeichen und ein Schritt.
    rejected.solutions = vec![ExchangeSolution {
        quote: "Der Eimer hält".into(),
        replacement: "Der Eimer trägt & <hält>".into(),
        step: String::new(),
    }];

    let mut adopted = lektor(311, 1, ExchangeFindingKind::Hint, "Ein Bild am Anfang fehlt.");
    adopted.status = F::Adopted;
    adopted.adopted_revision = Some(sample_uuid(111));
    adopted.updated = t(8, 4);
    let mut confirmed = entry(322, F::Adopted, ExchangeOriginKind::Mcp, "@mcp:probe", 3);
    confirmed.adopted_revision = Some(sample_uuid(111));
    confirmed.decision = Some(ExchangeDecision::Confirmed);
    confirmed.decided = Some(t(8, 4));
    confirmed.decided_by = Some("Hanna Probe".into());
    adopted.history = vec![confirmed];
    adopted.solutions = vec![ExchangeSolution {
        quote: String::new(),
        replacement: String::new(),
        step: "Bild am Anfang setzen.".into(),
    }];

    let mut deferred = lektor(312, 2, ExchangeFindingKind::Hint, "Das Seil früher einführen.");
    deferred.status = F::Deferred;
    deferred.deferred_idea = Some(sample_uuid(40));
    deferred.updated = t(8, 5);
    let mut moved = entry(323, F::Deferred, ExchangeOriginKind::Human, "Hanna Probe", 5);
    moved.deferred_idea = Some(sample_uuid(40));
    deferred.history = vec![moved];

    let thread = ExchangeFinding {
        uuid: sample_uuid(313),
        thread_post: Some(sample_uuid(51)),
        position: 0,
        persona: "@lektor".into(),
        kind: ExchangeFindingKind::Hint,
        body: "Die Tiefe des Brunnens fehlt [B12].".into(),
        similar_to: Some(sample_uuid(311)),
        summary_of: vec![sample_uuid(310), sample_uuid(312)],
        created: t(7, 14),
        updated: t(7, 14),
        ..ExchangeFinding::default()
    };
    let praise = ExchangeFinding {
        uuid: sample_uuid(314),
        node: Some(sample_uuid(12)),
        position: 3,
        persona: "@hoerer".into(),
        kind: ExchangeFindingKind::Praise,
        body: "Das Seil trägt schön.".into(),
        created: t(7, 17),
        updated: t(7, 17),
        ..ExchangeFinding::default()
    };
    vec![rejected, adopted, deferred, thread, praise]
}

/// Das Beispielwerk als ganzes Dokument.
pub fn sample_document() -> ExchangeDocument {
    use ExchangeNodeKind::{Chapter, Part, Section};
    use ExchangeRevisionStatus as S;

    // Kapitel „Der Eimer“ mit vier Fassungen.
    let mut bucket = node(11, Chapter, "Der Eimer", "==", 111, 5);
    let mut current = revision(111, 1, S::Current, SAMPLE_TRICKY_BODY, 5);
    current.note = "Erste Niederschrift.".into();
    let mut superseded = revision(112, 2, S::Superseded, "== Der Eimer\n\nDer Eimer war voll.\n", 6);
    superseded.based_on = Some(sample_uuid(111));
    superseded.edited = Some(t(6, 15));
    let mut proposal = revision(113, 3, S::Proposed, SAMPLE_CONTROL_BODY, 7);
    proposal.author = ExchangeAuthor::Assistant { persona: "@lektor".into(), model: "probe/modell-7b".into() };
    proposal.based_on = Some(sample_uuid(111));
    proposal.note = "Klang ergänzt.".into();
    // Unterversion 5: zwei Varianten derselben Gruppe auf derselben Basis (A offen, B verworfen).
    proposal.variant = Some(ExchangeVariant {
        group: sample_uuid(390),
        label: "A".into(),
        summary: "knapp: nur „Klang“ & <Bild>".into(),
        findings: vec![sample_uuid(310)],
    });
    let mut rejected = revision(114, 4, S::Rejected, "== Der Eimer\n\nDer Eimer ist ein Symbol.\n", 7);
    rejected.author = ExchangeAuthor::Assistant { persona: "@lektor".into(), model: "probe/modell-7b".into() };
    rejected.based_on = Some(sample_uuid(111));
    rejected.variant = Some(ExchangeVariant {
        group: sample_uuid(390),
        label: "B".into(),
        summary: "gründlich".into(),
        findings: vec![sample_uuid(310), sample_uuid(311)],
    });
    bucket.revisions = vec![current, superseded, proposal, rejected];
    bucket.posts = vec![
        ExchangePost {
            uuid: sample_uuid(30),
            position: 1,
            author: "@lektor".into(),
            body: "Der Eimer braucht ein Bild am Anfang.".into(),
            sources: vec!["Probequelle, S. 3".into()],
            refs: vec![ExchangePostRef {
                node: sample_uuid(11),
                revision: Some(sample_uuid(111)),
                quote: "Der Eimer hält".into(),
            }],
            created: t(7, 11),
            // Unterversion 5: geprüfte Fassung und Kopfzeile.
            meta: ExchangePostMeta {
                basis_revision: Some(sample_uuid(111)),
                basis_label: "geprüft: aktuelle Fassung 1".into(),
                ..ExchangePostMeta::default()
            },
            ..ExchangePost::default()
        },
        ExchangePost {
            uuid: sample_uuid(31),
            position: 2,
            author: "Hanna Probe".into(),
            body: "Ja, und die Lampe später auch.".into(),
            sources: vec!["https://example.invalid/brunnen".into()],
            refs: vec![ExchangePostRef { node: sample_uuid(21), revision: None, quote: "Lampe".into() }],
            created: t(7, 12),
            edited: Some(t(7, 13)),
            versions: vec![ExchangePostVersion {
                uuid: sample_uuid(300),
                body: "Ja, und die Lampe & <das Seil>.".into(),
                valid_from: t(7, 12),
                replaced: t(7, 13),
                edited_by: "Hanna Probe".into(),
            }],
            meta: ExchangePostMeta { reply_to: Some(sample_uuid(30)), ..ExchangePostMeta::default() },
        },
    ];

    // Kapitel „Das Seil“ (nicht nummeriert) mit dem Abschnitt „Knoten“.
    let mut rope = node(12, Chapter, "Das Seil", "==", 121, 5);
    rope.numbered = false;
    rope.children = vec![node(13, Section, "Knoten", "===", 131, 5)];

    let mut water = node(10, Part, "Erster Teil – Wasser", "=", 101, 5);
    water.children = vec![bucket, rope];
    let mut light = node(20, Part, "Zweiter Teil – Licht", "=", 201, 5);
    light.children = vec![node(21, Chapter, "Die Lampe", "==", 211, 5)];

    let template_v1 = "#set text(lang: \"de\")\n";
    let template_v2 = "#set page(paper: \"a5\")\n#set text(lang: \"de\")\n";

    let mut png_v2 = asset_revision(62, 2, "image/png", SAMPLE_PNG_RED, ExchangeAssetStatus::Current);
    png_v2.based_on = Some(sample_uuid(61));
    png_v2.caption = "Der Brunnen im Abendlicht".into();
    png_v2.alt = "Ein roter Punkt".into();
    png_v2.note = "Farbe gewechselt.".into();
    let mut svg = asset_revision(64, 1, "image/svg+xml", SAMPLE_SVG.as_bytes(), ExchangeAssetStatus::Current);
    svg.caption = "Die Lampe".into();
    svg.alt = "Eine gelbe Lampe".into();

    let work = ExchangeWork {
        uuid: sample_uuid(1),
        kind: ExchangeWorkKind::Book,
        title: "Der Brunnen im Hof".into(),
        subtitle: "Ein erfundenes Buch für Prüfungen".into(),
        author: "Hanna Probe".into(),
        audience: "Leser mit Geduld".into(),
        bible: "Freie Probeübersetzung".into(),
        created: t(5, 8),
        updated: Some(t(8, 18)),
        numbering_scheme: "custom".into(),
        numbering: Some("I.;1.;a)".into()),
        bible_module: Some("ProbeBibel".into()),
        sword_sources: vec![
            ExchangeSwordSource { kind: ExchangeSourceKind::Commentary, module: "ProbeKommentar".into() },
            ExchangeSwordSource { kind: ExchangeSourceKind::Lexicon, module: "ProbeLexikon".into() },
        ],
        timing: Some(ExchangeTiming { words_per_minute: 150, target_seconds: Some(1800) }),
        book: Some(ExchangeBook {
            title_area: ExchangeTitleArea::Middle,
            front_matter: true,
            cover_pages: false,
            paper_mm: 0.1,
            bleed_mm: 3.0,
            cover: Some(sample_uuid(60)),
            author_photo: None,
            updated: Some(t(8, 17)),
            publisher: "Probeverlag".into(),
            place: "Musterstadt".into(),
            year: "2026".into(),
            edition: "1. Auflage".into(),
            isbn: String::new(),
            price: "12,00 €".into(),
            series: "Erfundene Reihe".into(),
            printer: "Probedruck".into(),
            bible_notice: "Bibelzitate frei erfunden.".into(),
            image_credit: "Bilder: Hanna Probe".into(),
            dedication: "Für alle, die Brunnen graben.".into(),
            blurb: "Ein Buch über Wasser & Licht.".into(),
            bio: "Hanna Probe schreibt Prüftexte.".into(),
            publisher_address: "Probeverlag\nAm Brunnen 1\n12345 Musterstadt".into(),
            printer_address: "Probedruck\nDruckgasse 2\n12345 Musterstadt".into(),
            cover_blocks: vec![
                ExchangeCoverBlock {
                    uuid: sample_uuid(160),
                    kind: ExchangeCoverBlockKind::FrontTitle,
                    x_mm: 15.0,
                    y_mm: 30.5,
                    width_mm: 118.0,
                    align: ExchangeCoverAlign::Center,
                    size_pt: 28.0,
                    leading_em: 0.65,
                    color: "light".into(),
                    font: Some("Libertinus Serif".into()),
                    style: ExchangeFontStyle::Bold,
                    backdrop: ExchangeBackdrop::Shadow,
                    visible: true,
                    direction: None,
                    text: None,
                    updated: t(8, 16),
                },
                ExchangeCoverBlock {
                    uuid: sample_uuid(161),
                    kind: ExchangeCoverBlockKind::SpineTitle,
                    x_mm: 2.0,
                    y_mm: 20.0,
                    width_mm: 170.0,
                    align: ExchangeCoverAlign::Left,
                    size_pt: 10.0,
                    leading_em: 0.5,
                    color: "dark".into(),
                    font: None,
                    style: ExchangeFontStyle::Normal,
                    backdrop: ExchangeBackdrop::None,
                    visible: true,
                    direction: Some(ExchangeSpineDirection::Up),
                    text: None,
                    updated: t(8, 16),
                },
                ExchangeCoverBlock {
                    uuid: sample_uuid(162),
                    kind: ExchangeCoverBlockKind::BackMotto,
                    x_mm: -2.5,
                    y_mm: 15.0,
                    width_mm: 100.0,
                    align: ExchangeCoverAlign::Justify,
                    size_pt: 11.0,
                    leading_em: 0.8,
                    color: "#1f2a33".into(),
                    font: None,
                    style: ExchangeFontStyle::Italic,
                    backdrop: ExchangeBackdrop::Gradient,
                    visible: false,
                    direction: None,
                    text: Some("Erfundenes Motto & <Spruch>\nzweite Zeile".into()),
                    updated: t(8, 16),
                },
            ],
        }),
        template: ExchangeTemplate {
            current: template_v2.into(),
            revisions: vec![
                ExchangeTemplateRevision {
                    uuid: sample_uuid(80),
                    number: 1,
                    body: template_v1.into(),
                    author: "Hanna Probe".into(),
                    note: String::new(),
                    created: t(5, 8),
                },
                ExchangeTemplateRevision {
                    uuid: sample_uuid(81),
                    number: 2,
                    body: template_v2.into(),
                    author: "Hanna Probe".into(),
                    note: "A5 gesetzt.".into(),
                    created: t(6, 8),
                },
            ],
        },
        outline: vec![water, light],
        ideas: vec![
            ExchangeIdea {
                uuid: sample_uuid(40),
                node: Some(sample_uuid(12)),
                words: vec!["Seil".into(), "Knoten".into()],
                note: "Das Seil reißt nie ganz.".into(),
                quote: String::new(),
                source: String::new(),
                bible: "Pred 4,12".into(),
                origin_kind: ExchangeOriginKind::Human,
                origin_name: "Hanna Probe".into(),
                status: ExchangeIdeaStatus::Open,
                done_as: None,
                done_ref: None,
                created: t(6, 9),
                updated: t(6, 9),
                deleted: None,
            },
            ExchangeIdea {
                uuid: sample_uuid(41),
                node: None,
                words: vec!["Lampe".into()],
                note: "Licht als Bild für Klarheit.".into(),
                quote: "Die Lampe".into(),
                source: "Probequelle".into(),
                bible: String::new(),
                origin_kind: ExchangeOriginKind::Persona,
                origin_name: "@lektor".into(),
                status: ExchangeIdeaStatus::Done,
                done_as: Some(ExchangeIdeaUse::Post),
                done_ref: Some(sample_uuid(31)),
                created: t(6, 10),
                updated: t(7, 12),
                deleted: None,
            },
            ExchangeIdea {
                uuid: sample_uuid(42),
                node: None,
                words: vec!["Eimer".into()],
                note: "Verworfen: zu viel Eimer.".into(),
                quote: String::new(),
                source: String::new(),
                bible: String::new(),
                origin_kind: ExchangeOriginKind::Mcp,
                origin_name: "@mcp:probe".into(),
                status: ExchangeIdeaStatus::Open,
                done_as: None,
                done_ref: None,
                created: t(6, 11),
                updated: t(7, 9),
                deleted: Some(t(7, 9)),
            },
        ],
        threads: vec![ExchangeThread {
            uuid: sample_uuid(50),
            title: "Fragen zum Brunnen".into(),
            created_by: "@lektor".into(),
            created: t(7, 14),
            updated: t(7, 15),
            posts: vec![
                ExchangeThreadPost {
                    uuid: sample_uuid(51),
                    position: 1,
                    author: "@lektor".into(),
                    body: "Wie tief ist der Brunnen?".into(),
                    sources: Vec::new(),
                    created: t(7, 14),
                    ..ExchangeThreadPost::default()
                },
                ExchangeThreadPost {
                    uuid: sample_uuid(52),
                    position: 2,
                    author: "Hanna Probe".into(),
                    body: "Sieben Meter, erfunden.".into(),
                    sources: vec!["Notizbuch".into()],
                    created: t(7, 15),
                    edited: Some(t(7, 16)),
                    versions: vec![ExchangePostVersion {
                        uuid: sample_uuid(301),
                        body: "Acht Meter.".into(),
                        valid_from: t(7, 15),
                        replaced: t(7, 16),
                        edited_by: "Hanna Probe".into(),
                    }],
                    meta: ExchangePostMeta {
                        basis_label: "geprüft: Faden".into(),
                        reply_to: Some(sample_uuid(51)),
                        repeat_of: Some(sample_uuid(51)),
                        relay_from: Some("@lektor".into()),
                        ..ExchangePostMeta::default()
                    },
                },
            ],
        }],
        assets: vec![
            ExchangeAsset {
                uuid: sample_uuid(60),
                name: "brunnen.png".into(),
                created: t(6, 10),
                paths: vec!["bilder/brunnen.png".into()],
                revisions: vec![
                    asset_revision(61, 1, "image/png", SAMPLE_PNG_GREEN, ExchangeAssetStatus::Superseded),
                    png_v2,
                ],
            },
            ExchangeAsset {
                uuid: sample_uuid(63),
                name: "lampe.svg".into(),
                created: t(6, 10),
                paths: vec!["bilder/lampe.svg".into(), "/bilder/lampe.svg".into()],
                revisions: vec![svg],
            },
        ],
        deleted: vec![
            ExchangeTombstone { uuid: sample_uuid(70), kind: ExchangeTombstoneKind::Node, deleted: t(7, 16) },
            ExchangeTombstone { uuid: sample_uuid(154), kind: ExchangeTombstoneKind::Link, deleted: t(7, 17) },
            ExchangeTombstone { uuid: sample_uuid(155), kind: ExchangeTombstoneKind::Position, deleted: t(7, 18) },
            ExchangeTombstone { uuid: sample_uuid(163), kind: ExchangeTombstoneKind::CoverBlock, deleted: t(7, 19) },
            ExchangeTombstone { uuid: sample_uuid(315), kind: ExchangeTombstoneKind::Finding, deleted: t(7, 20) },
            ExchangeTombstone { uuid: sample_uuid(324), kind: ExchangeTombstoneKind::FindingStatus, deleted: t(7, 21) },
            ExchangeTombstone { uuid: sample_uuid(302), kind: ExchangeTombstoneKind::PostVersion, deleted: t(7, 22) },
            ExchangeTombstone { uuid: sample_uuid(401), kind: ExchangeTombstoneKind::Typeset, deleted: t(7, 23) },
            ExchangeTombstone { uuid: sample_uuid(1103), kind: ExchangeTombstoneKind::GlossaryTerm, deleted: t(8, 16) },
            ExchangeTombstone { uuid: sample_uuid(1112), kind: ExchangeTombstoneKind::Register, deleted: t(8, 17) },
        ],
        map: ExchangeMap {
            links: vec![
                ExchangeLink {
                    uuid: sample_uuid(150),
                    from: sample_uuid(40),
                    to: sample_uuid(41),
                    kind: ExchangeLinkKind::LeadsTo,
                    note: "Vom Seil zur Lampe.".into(),
                    origin_kind: ExchangeOriginKind::Human,
                    origin_name: "Hanna Probe".into(),
                    created: t(7, 19),
                    updated: t(7, 19),
                    deleted: None,
                },
                ExchangeLink {
                    uuid: sample_uuid(151),
                    // „widerspricht“ ist ungerichtet; in der Reihenfolge der Einfälle, wie die Ausgabe sie schreibt.
                    from: sample_uuid(41),
                    to: sample_uuid(42),
                    kind: ExchangeLinkKind::Contradicts,
                    note: "Eimer & <Lampe>\nstreiten sich.".into(),
                    origin_kind: ExchangeOriginKind::Persona,
                    origin_name: "@lektor".into(),
                    created: t(7, 20),
                    updated: t(7, 21),
                    deleted: Some(t(7, 21)),
                },
            ],
            positions: vec![
                ExchangePosition {
                    uuid: sample_uuid(152),
                    idea: sample_uuid(40),
                    x: 120.5,
                    y: 0.1 + 0.2,
                    updated: t(7, 22),
                },
                ExchangePosition {
                    uuid: sample_uuid(153),
                    idea: sample_uuid(41),
                    x: -MAP_COORD_LIMIT,
                    y: -80.25,
                    updated: t(7, 23),
                },
            ],
        },
        sheet: Some(ExchangeSheet {
            font: Some("\"Erfundene Antiqua\", serif".into()),
            size: Some("11pt".into()),
            background: Some("#fbf8f1".into()),
            ..ExchangeSheet::default()
        }),
        findings: sample_findings(),
        typeset: Some(ExchangeTypeset {
            uuid: sample_uuid(400),
            updated: t(8, 19),
            updated_by: "Hanna Probe".into(),
            settings: crate::typeset::full_sample(),
        }),
        registers: vec![
            ExchangeRegister {
                uuid: sample_uuid(1110),
                slug: "orte".into(),
                title: "Orte im Hof".into(),
                created: t(8, 10),
            },
            ExchangeRegister {
                uuid: sample_uuid(1111),
                slug: "werkzeuge".into(),
                title: "Werkzeuge & Geräte".into(),
                created: t(8, 11),
            },
        ],
        glossary: vec![
            ExchangeGlossaryTerm {
                uuid: sample_uuid(1100),
                term: "Brunnenstube".into(),
                short: "Gemauerter Raum, in dem das Quellwasser gesammelt wird.".into(),
                long: "Im Hof liegt sie unter der Linde.\nDer Deckel ist aus Eiche.".into(),
                source: "frei erfunden".into(),
                variants: vec!["Brunnenstuben".into()],
                bible_refs: vec!["Gen 26,19".into(), "Joh 4,6".into()],
                status: ExchangeGlossaryStatus::Accepted,
                origin: "Hanna Probe".into(),
                created: t(8, 12),
                updated: Some(t(8, 13)),
                updated_by: "Hanna Probe".into(),
            },
            ExchangeGlossaryTerm {
                uuid: sample_uuid(1101),
                term: "Schöpfeimer".into(),
                status: ExchangeGlossaryStatus::Accepted,
                origin: "Hanna Probe".into(),
                created: t(8, 14),
                ..ExchangeGlossaryTerm::default()
            },
            ExchangeGlossaryTerm {
                uuid: sample_uuid(1102),
                term: "Zisterne".into(),
                short: "Sammelbecken für Regenwasser <kein Markup> & Co.".into(),
                status: ExchangeGlossaryStatus::Proposed,
                origin: "@lektor".into(),
                created: t(8, 15),
                ..ExchangeGlossaryTerm::default()
            },
        ],
    };

    ExchangeDocument {
        version: FORMAT_VERSION,
        exported: t(8, 19),
        instance: ExchangeInstance { uuid: sample_uuid(2), name: "Probeinstanz".into() },
        generator: "baruch (Beispiel)".into(),
        work,
        unknown: Vec::new(),
    }
}

/// Das Beispielwerk mit den Teilen der Unterversion 8 (Runde 22, siehe Modulkopf).
pub fn sample_document_u8() -> ExchangeDocument {
    let mut doc = sample_document();
    let before = "== Der Eimer\n\nErster Entwurf.\n";
    let middle = "== Der Eimer\n\nZweiter Entwurf & <mehr>.\n";
    let step = |n: u32, seq: u32, kind: ExchangeAmendKind, undo_of: Option<u32>, from: &str, to: &str| ExchangeAmend {
        uuid: sample_uuid(n),
        seq,
        kind,
        undo_of,
        author: "Hanna Probe".into(),
        created: t(5, 9 + seq),
        before: from.to_owned(),
        after: to.to_owned(),
    };
    fn find<'a>(nodes: &'a mut [ExchangeNode], uuid: &str) -> Option<&'a mut ExchangeNode> {
        for node in nodes {
            if node.uuid == uuid {
                return Some(node);
            }
            if let Some(found) = find(&mut node.children, uuid) {
                return Some(found);
            }
        }
        None
    }
    let bucket = find(&mut doc.work.outline, &sample_uuid(11)).expect("„Der Eimer“ im Beispiel");
    for revision in &mut bucket.revisions {
        if revision.uuid == sample_uuid(111) {
            revision.amends = vec![
                step(2200, 1, ExchangeAmendKind::Amend, None, before, middle),
                step(2201, 2, ExchangeAmendKind::Undo, Some(1), middle, before),
                step(2202, 3, ExchangeAmendKind::Amend, None, before, SAMPLE_TRICKY_BODY),
            ];
            revision.amend_seq = 3;
        }
        if revision.uuid == sample_uuid(113) {
            revision.based_on_amend = Some(2);
        }
    }
    for finding in &mut doc.work.findings {
        if finding.uuid == sample_uuid(311) {
            finding.adopted_amend = Some(3);
            for entry in &mut finding.history {
                entry.adopted_amend = Some(3);
            }
        }
    }
    if let Some(typeset) = doc.work.typeset.as_mut() {
        typeset.settings.draft = Some(crate::typeset::full_draft());
    }
    doc.work.deleted.push(ExchangeTombstone {
        uuid: sample_uuid(2210),
        kind: ExchangeTombstoneKind::Amend,
        deleted: t(8, 18),
    });
    doc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_u8_sample_adds_only_the_u8_parts() {
        let (old, new) = (sample_document(), sample_document_u8());
        assert!(new.work.typeset.as_ref().and_then(|t| t.settings.draft.as_ref()).is_some(), "Entwurf gesetzt");
        assert!(old.work.typeset.as_ref().is_some_and(|t| t.settings.draft.is_none()), "das alte Beispiel ohne");
        assert_eq!(new.work.deleted.len(), old.work.deleted.len() + 1, "genau ein Grabstein mehr");
        assert!(new.work.findings.iter().any(|f| f.adopted_amend == Some(3)), "Stand am übernommenen Befund");
    }

    fn walk<'a>(nodes: &'a [ExchangeNode], out: &mut Vec<&'a ExchangeNode>) {
        for node in nodes {
            out.push(node);
            walk(&node.children, out);
        }
    }

    #[test]
    fn sample_has_the_promised_shape() {
        let doc = sample_document();
        let mut nodes = Vec::new();
        walk(&doc.work.outline, &mut nodes);
        let count = |kind| nodes.iter().filter(|n| n.kind == kind).count();
        // Vertrag S1 (7): 2 Teile, 3 Kapitel, 1 Abschnitt, einer davon nicht nummeriert.
        assert_eq!(count(ExchangeNodeKind::Part), 2, "zwei Teile");
        assert_eq!(count(ExchangeNodeKind::Chapter), 3, "drei Kapitel");
        assert_eq!(count(ExchangeNodeKind::Section), 1, "ein Abschnitt");
        assert_eq!(nodes.iter().filter(|n| !n.numbered).count(), 1, "genau ein Punkt ohne Nummer");
        // Je Punkt genau eine aktuelle Fassung, sonst wäre das Beispiel selbst ungültig.
        for node in &nodes {
            let current = node.revisions.iter().filter(|r| r.status == ExchangeRevisionStatus::Current).count();
            assert_eq!(current, 1, "{}: genau eine aktuelle Fassung", node.title);
        }
        // Das verbotene Steuerzeichen steht in genau einer Fassung (Base64-Zweig des Schreibers).
        let control = nodes.iter().flat_map(|n| &n.revisions).filter(|r| r.body.contains('\u{1}')).count();
        assert_eq!(control, 1, "U+0001 in genau einer Fassung");
        // Alle festen Kennungen sind kanonisch und eindeutig.
        let mut uuids: Vec<&str> = nodes.iter().map(|n| n.uuid.as_str()).collect();
        uuids.extend(nodes.iter().flat_map(|n| n.revisions.iter().map(|r| r.uuid.as_str())));
        uuids.extend(doc.work.assets.iter().flat_map(|a| a.revisions.iter().map(|r| r.uuid.as_str())));
        assert!(uuids.iter().all(|u| is_uuid(u)), "kanonische UUIDs: {uuids:?}");
        let before = uuids.len();
        uuids.sort_unstable();
        uuids.dedup();
        assert_eq!(uuids.len(), before, "keine UUID doppelt");
        // Karte (Runde 25): je eine aktive und eine gelöschte Verbindung, zwei Lagen, Grabsteine beider neuer Arten.
        let map = &doc.work.map;
        assert_eq!(map.links.iter().filter(|l| l.deleted.is_none()).count(), 1, "eine aktive Verbindung");
        assert_eq!(map.links.iter().filter(|l| l.deleted.is_some()).count(), 1, "eine gelöschte Verbindung");
        assert_eq!(map.positions.len(), 2, "zwei Lagen");
        for kind in [ExchangeTombstoneKind::Link, ExchangeTombstoneKind::Position] {
            assert!(doc.work.deleted.iter().any(|t| t.kind == kind), "Grabstein der Art {kind:?}");
        }
        // Unterversion 3 (Runde 27): Anschriften, drei Umschlag-Blöcke (einer auf dem Rücken, einer mit eigenem Text),
        // Satzbild und ein Grabstein der Art umschlag-block.
        let book = doc.work.book.as_ref().unwrap();
        assert!(book.publisher_address.contains('\n'), "mehrzeilige Anschrift des Verlags");
        assert_eq!(book.cover_blocks.len(), 3, "drei Umschlag-Blöcke");
        assert!(book.cover_blocks.iter().any(|b| b.direction.is_some()), "ein Rücken-Block mit Laufrichtung");
        assert!(book.cover_blocks.iter().any(|b| b.text.is_some()), "ein Block mit eigenem Text");
        assert!(doc.work.sheet.as_ref().is_some_and(|s| !s.is_empty()), "Satzbild-Einstellungen");
        assert!(
            doc.work.deleted.iter().any(|t| t.kind == ExchangeTombstoneKind::CoverBlock),
            "Grabstein eines Umschlag-Blocks"
        );
        assert_eq!(doc.version, FORMAT_VERSION, "das Beispiel trägt die Unterversion dieses Programms");
        // Unterversion 5 (Runde 30): Lösungen, zwei Varianten einer Gruppe, Metadaten an Beitrag und Fadenbeitrag.
        assert!(doc.work.findings.iter().any(|f| !f.solutions.is_empty()), "ein Befund mit Lösung");
        let variants: Vec<&ExchangeVariant> =
            nodes.iter().flat_map(|n| &n.revisions).filter_map(|r| r.variant.as_ref()).collect();
        assert_eq!(variants.len(), 2, "zwei Varianten");
        assert!(variants.iter().all(|v| v.group == variants[0].group), "gleiche Gruppe");
        assert!(nodes.iter().flat_map(|n| &n.posts).any(|p| p.meta.basis_revision.is_some()), "Grundlage am Beitrag");
        assert!(doc.work.threads.iter().flat_map(|t| &t.posts).any(|p| p.meta.relay_from.is_some()), "Relay im Faden");
    }

    #[test]
    fn sample_images_carry_matching_checksums() {
        let doc = sample_document();
        for revision in doc.work.assets.iter().flat_map(|a| &a.revisions) {
            // sha256 muss zu den Bytes passen, sonst lehnte der Leser die eigene Ausgabe ab.
            assert_eq!(revision.sha256, sha256_hex(&revision.data), "Prüfsumme von {}", revision.uuid);
            assert!(image_extension(&revision.mime).is_some(), "Typ {} gehört zum Format", revision.mime);
        }
        // Die PNGs sind echte PNG-Dateien (Signatur), damit dieselbe Prüfung wie beim Hochladen greift.
        assert!(SAMPLE_PNG_GREEN.starts_with(b"\x89PNG\r\n\x1a\n") && SAMPLE_PNG_RED.starts_with(b"\x89PNG\r\n\x1a\n"));
    }
}
