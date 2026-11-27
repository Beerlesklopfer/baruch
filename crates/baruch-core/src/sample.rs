//! Sample data for the user interface (until database and import are in place).

use crate::{Author, LectorChoice, Node, NodeKind, Persona, Post, Revision, RevisionStatus, TextRef, Work, WorkKind};

fn human(number: u32, body: &str, status: RevisionStatus) -> Revision {
    Revision {
        number,
        body: body.to_owned(),
        author: Author::Human { name: "Autor".into() },
        status,
        note: String::new(),
    }
}

fn proposal(number: u32, persona: &str, model: &str, body: &str, note: &str) -> Revision {
    Revision {
        number,
        body: body.to_owned(),
        author: Author::Assistant { persona: persona.into(), model: model.into() },
        status: RevisionStatus::Proposed,
        note: note.to_owned(),
    }
}

fn node(id: usize, parent: Option<usize>, number: &str, title: &str, kind: NodeKind) -> Node {
    Node {
        id,
        key: 0,
        parent,
        number: number.into(),
        title: title.into(),
        kind,
        revisions: Vec::new(),
        posts: Vec::new(),
        findings: 0,
    }
}

fn persona(handle: &str, name: &str, role: &str, model: &str, context_length: u32) -> Persona {
    Persona {
        handle: handle.into(),
        name: name.into(),
        role: role.into(),
        model: model.into(),
        context_length,
        active: true,
    }
}

const INTRO_V1: &str =
    "= Einleitung\n\nDieses Buch handelt von Abhängigkeit.\nEs richtet sich an Betroffene und an Gemeinden.\n";
const INTRO_V2: &str = "= Einleitung\n\nDieses Buch handelt von Abhängigkeit in Beziehungen.\nEs richtet sich an Betroffene, an Seelsorger und an Gemeinden.\n";
const INTRO_V3: &str = "= Einleitung\n\nDieses Buch handelt von Abhängigkeit in Beziehungen – oft leise, oft fromm begründet.\nEs richtet sich an Betroffene, an Seelsorger und an Gemeinden.\nWer Gewalt erlebt, findet am Ende Hilfe und Notrufnummern.\n";

const SIGNS_V1: &str = "== Erste Anzeichen\n\nAbhängigkeit beginnt selten laut.\nSie beginnt mit kleinen Zugeständnissen.\n\n#quote[Die Liebe ist langmütig und freundlich.] (1. Kor 13,4)\n";
const SIGNS_V2: &str = "== Erste Anzeichen\n\nAbhängigkeit beginnt selten laut.\nSie beginnt mit kleinen Zugeständnissen, die sich wie Liebe anfühlen.\n\n#quote[Die Liebe ist langmütig und freundlich.] (1. Kor 13,4)\n\nLangmut heißt nicht, Unrecht zu dulden.\n";

const ROMANS: &str = "== Der Text\n\nRömer 12,1–2: Stellt euch nicht dieser Welt gleich,\nsondern lasst euch verwandeln durch die Erneuerung eures Sinnes.\n";

/// A book with parts, chapters and sections, revisions, a proposal and personas.
pub fn work() -> Work {
    let mut nodes = vec![
        node(0, None, "", "Vorwort", NodeKind::Chapter),
        node(1, None, "I", "Teil I – Erkennen", NodeKind::Part),
        node(2, Some(1), "1", "Einleitung", NodeKind::Chapter),
        node(3, Some(1), "2", "Das heimliche Muster", NodeKind::Chapter),
        node(4, Some(3), "2.1", "Erste Anzeichen", NodeKind::Section),
        node(5, Some(3), "2.2", "Fromme Begründungen", NodeKind::Section),
        node(6, None, "II", "Teil II – Lösen", NodeKind::Part),
        node(7, Some(6), "3", "Hilfe holen", NodeKind::Chapter),
        node(8, Some(7), "3.1", "Seelsorge", NodeKind::Section),
        node(9, Some(6), "4", "Predigtskizze Römer 12", NodeKind::Chapter),
        node(10, Some(9), "4.1", "Der Text", NodeKind::Section),
    ];

    nodes[0].revisions = vec![human(1, "= Vorwort\n\nFür alle, die schweigen mussten.\n", RevisionStatus::Current)];

    nodes[2].revisions = vec![
        human(1, INTRO_V1, RevisionStatus::Superseded),
        human(2, INTRO_V2, RevisionStatus::Current),
        proposal(
            3,
            "@lektor",
            "google/gemma-4-26b-a4b",
            INTRO_V3,
            "Zielgruppe geschärft, Hinweis auf Hilfe am Ende ergänzt (Nathan, Befund 1).",
        ),
    ];
    nodes[2].findings = 1;
    nodes[2].posts = vec![
        Post {
            id: 0,
            created: String::new(),
            author: "@nathan".into(),
            body: "Befund: Die Einleitung nennt Gewalt nicht beim Namen. Betroffene sollten früh erfahren, dass es Hilfe gibt.".into(),
            sources: vec![],
            refs: vec![],
        },
        Post {
            id: 0,
            created: String::new(),
            author: "@hoerer".into(),
            body: "Als Betroffene lese ich „Abhängigkeit“ zuerst als Sucht. „In Beziehungen“ hilft.".into(),
            sources: vec![],
            refs: vec![],
        },
        Post {
            id: 0,
            created: String::new(),
            author: "@lektor".into(),
            body: "Beides aufgenommen: Vorschlag 3/3 liegt im Editor. @schrift, braucht die Einleitung einen Bibelvers?".into(),
            sources: vec![],
            refs: vec![],
        },
        Post {
            id: 0,
            created: String::new(),
            author: "@schrift".into(),
            body: "Nicht nötig; Kapitel 2 belegt mit 1. Kor 13,4. Wortlaut geprüft.".into(),
            sources: vec!["Lutherbibel 2017, 1. Kor 13,4 (MultiGPT: Bibeln)".into()],
            refs: vec![],
        },
    ];

    nodes[3].revisions = vec![human(
        1,
        "= Das heimliche Muster\n\nWarum Abhängigkeit und Einsamkeit verwandt sind.\n",
        RevisionStatus::Current,
    )];
    nodes[4].revisions =
        vec![human(1, SIGNS_V1, RevisionStatus::Superseded), human(2, SIGNS_V2, RevisionStatus::Current)];
    nodes[5].revisions = vec![human(
        1,
        "== Fromme Begründungen\n\n„Die Frau sei dem Mann untertan“ – und was der Text nicht sagt.\n",
        RevisionStatus::Current,
    )];
    nodes[5].findings = 2;
    nodes[5].posts = vec![
        Post {
            id: 0,
            created: String::new(),
            author: "@nathan".into(),
            body: "Befund: Zitat ohne Stellenangabe (Eph 5,22) und ohne den Vers davor (Eph 5,21). Angreifbar.".into(),
            sources: vec![],
            refs: vec![],
        },
        Post {
            id: 0,
            created: String::new(),
            author: "@lektor".into(),
            body: "Du hast in 2.1 schon geschrieben: „Langmut heißt nicht, Unrecht zu dulden.“ Das trägt auch hier – \
               verweisen statt neu begründen?"
                .into(),
            sources: vec![],
            refs: vec![TextRef { node: 4, rev: 2, quote: "Langmut heißt nicht, Unrecht zu dulden.".into() }],
        },
    ];
    nodes[8].revisions = vec![human(
        1,
        "== Seelsorge\n\nWas Seelsorger wissen müssen, bevor sie zur Versöhnung raten.\n",
        RevisionStatus::Current,
    )];
    nodes[10].revisions = vec![
        human(1, ROMANS, RevisionStatus::Current),
        proposal(
            2,
            "@exeget",
            "llama3.2:3b",
            &format!("{ROMANS}\nKernaussage: Verwandlung geschieht nicht durch Anpassung, sondern durch Gott.\n"),
            "Kernaussage für die Predigt ergänzt.",
        ),
    ];

    // Remaining chapters only get their heading. Parts have no heading line (like `outline.add` for parts): their
    // text is empty, otherwise they would appear in the export as chapters.
    for n in nodes.iter_mut().filter(|n| n.revisions.is_empty()) {
        let body = if n.kind == NodeKind::Part { String::new() } else { format!("= {}\n", n.title) };
        n.revisions = vec![human(1, &body, RevisionStatus::Current)];
    }

    Work {
        kind: WorkKind::Book,
        // Own title, so that it differs from the real book. Only takes effect when seeding into an empty database
        // (baruch-store `seed.rs`); existing works keep title and slug.
        title: "Beispielwerk: Leise Abhängigkeit (Auszug)".into(),
        author: "Erika Mustermann".into(),
        nodes,
        personas: vec![
            persona(
                "@lektor",
                "Lektor",
                "Aufbau, Klarheit, Stil; kennt Kapitelumfeld und Einfälle; schreibt Vorschläge",
                "google/gemma-4-26b-a4b",
                // Eliezer loads Gemma with 32768 context (backlog „ein gemeinsames Modell“); Baruch calculates the
                // truncation budget with this number, therefore it must not be larger than the loaded one.
                32_768,
            ),
            persona("@exeget", "Exeget", "Text, Zusammenhang, Hauptaussage", "llama3.2:3b", 8_192),
            persona("@schrift", "Schrift", "Bibelstellen und Wortlaut belegen", "qwen3:4b", 32_768),
            persona("@belege", "Belege", "Quellen und Fußnoten prüfen", "qwen3:4b", 32_768),
            persona("@nathan", "Nathan", "Auditor: nur Befunde, nie Freigabe", "gpt-4.1-mini", 16_384),
            persona("@hoerer", "Hörer", "Liest als Zielgruppe", "llama3.2:3b", 8_192),
        ],
        lector_choice: LectorChoice::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sample work is part of every package (seed into an empty database). It must therefore carry neither the
    /// name of a real human nor the title of a real book (review R24).
    #[test]
    fn the_sample_work_names_no_real_author_and_no_real_book() {
        let work = work();
        assert!(
            work.title.starts_with("Beispielwerk: ") && work.title.ends_with("(Auszug)"),
            "der Titel weist das Werk als Beispiel aus, nicht als Buch eines Dritten: {}",
            work.title
        );
        assert_eq!(
            work.author, "Erika Mustermann",
            "Autor ist der übliche deutsche Platzhaltername – so hält ihn niemand für einen echten Verfasser"
        );
    }
}
