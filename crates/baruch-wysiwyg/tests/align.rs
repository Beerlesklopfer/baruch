// SPDX-License-Identifier: AGPL-3.0-or-later
//! Alignment of two revisions by blocks (contract Round 8, "inline", 2a): changed, inserted
//! (beginning, middle, end) and removed blocks, unchanged blocks byte-identical, typeset view of a hunk,
//! and a property test with a fixed seed (`WS_SEED`, `WS_CASES`) that `align` never fails and
//! its hunks describe the text without gaps. All texts freely invented.

use baruch_wysiwyg::{Hunk, HunkKind, ImageRef, align, hunk_html_with, parse};

const BASE: &str = "= Garten\n\nDie Bohnen ranken am Zaun.\n\nIm Beet stehen Kohlköpfe.\n\nAm Abend gießt Anna.\n";

fn h(kind: HunkKind, base: std::ops::Range<usize>, proposal: std::ops::Range<usize>) -> Hunk {
    Hunk { kind, base, proposal }
}

#[test]
fn a_changed_paragraph_is_one_changed_hunk_at_its_block() {
    let proposal = BASE.replace("Kohlköpfe", "Kürbisse");
    assert_eq!(
        align(BASE, &proposal),
        vec![h(HunkKind::Changed, 2..3, 2..3)],
        "nur Block 2 (der Absatz mit dem Beet) ist geändert, Überschrift und Nachbarn nicht"
    );
}

#[test]
fn inserted_blocks_name_the_block_they_stand_before() {
    let start = format!("Vorwort in einem Satz.\n\n{BASE}");
    assert_eq!(
        align(BASE, &start),
        vec![h(HunkKind::Inserted, 0..0, 0..1)],
        "am Anfang: vor Block 0 eingefügt, die Basis-Spanne ist leer"
    );
    let middle = BASE.replace("Im Beet", "Ein neuer Absatz.\n\nIm Beet");
    assert_eq!(align(BASE, &middle), vec![h(HunkKind::Inserted, 2..2, 2..3)], "in der Mitte: vor Block 2 eingefügt");
    let end = format!("{BASE}\nNachts ruht der Garten.\n");
    assert_eq!(
        align(BASE, &end),
        vec![h(HunkKind::Inserted, 4..4, 4..5)],
        "am Ende: base.start ist die Blockzahl (4); der bisher letzte Block gilt trotz neuer Leerzeile als gleich"
    );
}

#[test]
fn a_removed_block_has_an_empty_proposal_range() {
    let proposal = BASE.replace("Die Bohnen ranken am Zaun.\n\n", "");
    assert_eq!(
        align(BASE, &proposal),
        vec![h(HunkKind::Removed, 1..2, 1..1)],
        "Block 1 entfällt, im Vorschlag bleibt eine leere Spanne an Stelle 1"
    );
    let last = BASE.replace("\n\nAm Abend gießt Anna.\n", "\n");
    assert_eq!(align(BASE, &last), vec![h(HunkKind::Removed, 3..4, 3..3)], "der letzte Block entfällt");
}

#[test]
fn several_hunks_come_in_text_order_and_untouched_blocks_stay_byte_equal() {
    let proposal = "= Garten\n\nDie Bohnen ranken am Zaun.\n\nNeu hier.\n\nIm Beet stehen Kürbisse.\n";
    let hunks = align(BASE, proposal);
    assert_eq!(
        hunks,
        vec![h(HunkKind::Changed, 2..4, 2..4)],
        "Einfügen vor und Ändern direkt hintereinander bilden ein Stück: {hunks:?}"
    );
    let apart = "= Garten neu\n\nDie Bohnen ranken am Zaun.\n\nIm Beet stehen Kohlköpfe.\n";
    let hunks = align(BASE, apart);
    assert_eq!(
        hunks,
        vec![h(HunkKind::Changed, 0..1, 0..1), h(HunkKind::Removed, 3..4, 3..3)],
        "getrennte Stellen sind getrennte Stücke in Textreihenfolge: {hunks:?}"
    );
    let (old, new) = (parse(BASE).blocks, parse(apart).blocks);
    for (i, j) in [(1, 1), (2, 2)] {
        assert_eq!(
            old[i].raw.trim_end(),
            new[j].raw.trim_end(),
            "Block {i} steht in keinem Stück und ist byte-gleich (ohne Leerraum am Ende)"
        );
    }
    assert!(align(BASE, BASE).is_empty(), "gleiche Texte haben keine Stücke");
    assert!(align("", "").is_empty(), "leere Texte auch nicht");
    assert_eq!(align("", "Neu.\n"), vec![h(HunkKind::Inserted, 0..0, 0..1)], "alles neu");
    assert_eq!(align("Alt.\n", ""), vec![h(HunkKind::Removed, 0..1, 0..0)], "alles fort");
}

#[test]
fn the_hunk_html_shows_only_the_affected_blocks_as_deleted_and_inserted() {
    let proposal = BASE.replace("Kohlköpfe", "Kürbisse & \"Rüben\"");
    let hunk = &align(BASE, &proposal)[0];
    let html = hunk_html_with(BASE, &proposal, hunk, &|_| None);
    assert!(html.starts_with("<div class=\"ws-diff-doc\" data-kind=\"changed\">"), "Rahmen mit Art: {html}");
    assert!(
        html.contains(
            "<div class=\"ws-del\"><div class=\"ws-block\" data-kind=\"paragraph\">Im Beet stehen Kohlköpfe."
        ),
        "alter Block entfernt markiert: {html}"
    );
    assert!(
        html.contains("<div class=\"ws-ins\">") && html.contains("Kürbisse &amp; &quot;Rüben&quot;"),
        "neuer Block markiert und maskiert: {html}"
    );
    assert!(!html.contains("Bohnen") && !html.contains("Garten"), "unveränderte Blöcke stehen nicht im Stück: {html}");
    assert!(
        !html.contains("data-b="),
        "ohne data-b, damit das Widget die Blöcke des Satzbilds eindeutig findet: {html}"
    );

    let figure = "#figure(image(\"bilder/beet.png\"), caption: [Das Beet])\n";
    let with_image = format!("{BASE}\n{figure}");
    let hunk = &align(BASE, &with_image)[0];
    let html = hunk_html_with(BASE, &with_image, hunk, &|path| {
        Some(ImageRef { src: format!("/bild?p={path}"), alt: "Beet".into(), missing: false })
    });
    assert!(html.contains("data-kind=\"inserted\""), "eingefügt: {html}");
    assert!(html.contains("<img class=\"ws-image\" src=\"/bild?p=bilder/beet.png\""), "Bild über den Rückruf: {html}");
    let clipped = hunk_html_with(BASE, BASE, &h(HunkKind::Changed, 3..99, 7..9), &|_| None);
    assert!(clipped.contains("Am Abend") && !clipped.contains("ws-ins"), "zu große Bereiche werden gekürzt: {clipped}");
}

// ---- Property test --------------------------------------------------------------------------

const PIECES: &[&str] = &[
    "= Kapitel\n",
    "== Abschnitt\n",
    "Ein Absatz über Bohnen.\n",
    "Ein Absatz über Kohl.\n",
    "Noch ein Absatz.\n",
    "- Punkt eins\n- Punkt zwei\n",
    "#szene[\n  Anna gießt.\n]\n",
    "#let x = 1\n",
    "Ein _hervorgehobener_ Satz.\n",
    "#quote(block: true)[Ein Zitat.]\n",
];

fn text(rng: &mut fastrand::Rng, max: usize) -> Vec<&'static str> {
    (0..rng.usize(0..=max)).map(|_| PIECES[rng.usize(..PIECES.len())]).collect()
}

fn join(pieces: &[&str]) -> String {
    pieces.join("\n")
}

/// Checks that the hunks describe both texts without gaps and in order and that only
/// equal blocks stand in between.
fn check(base: &str, proposal: &str, hunks: &[Hunk], case: &str) {
    let (old, new) = (parse(base).blocks, parse(proposal).blocks);
    let (mut i, mut j) = (0, 0);
    for hunk in hunks {
        assert!(hunk.base.start >= i && hunk.proposal.start >= j, "{case}: Stücke in Reihenfolge: {hunks:?}");
        assert_eq!(hunk.base.start - i, hunk.proposal.start - j, "{case}: gleich viele gleiche Blöcke davor");
        for k in 0..hunk.base.start - i {
            assert_eq!(old[i + k].raw.trim_end(), new[j + k].raw.trim_end(), "{case}: Zwischenblock gleich");
        }
        assert!(hunk.base.end <= old.len() && hunk.proposal.end <= new.len(), "{case}: Bereiche im Text");
        match hunk.kind {
            HunkKind::Changed => assert!(!hunk.base.is_empty() && !hunk.proposal.is_empty(), "{case}: {hunk:?}"),
            HunkKind::Inserted => assert!(hunk.base.is_empty() && !hunk.proposal.is_empty(), "{case}: {hunk:?}"),
            HunkKind::Removed => assert!(!hunk.base.is_empty() && hunk.proposal.is_empty(), "{case}: {hunk:?}"),
        }
        i = hunk.base.end;
        j = hunk.proposal.end;
    }
    assert_eq!(old.len() - i, new.len() - j, "{case}: gleich viele Blöcke dahinter");
    for k in 0..old.len() - i {
        assert_eq!(old[i + k].raw.trim_end(), new[j + k].raw.trim_end(), "{case}: Schlussblock gleich");
    }
    for hunk in hunks {
        let html = hunk_html_with(base, proposal, hunk, &|_| None);
        assert!(html.starts_with("<div class=\"ws-diff-doc\"") && html.ends_with("</div>"), "{case}: HTML vollständig");
    }
}

#[test]
fn align_never_fails_and_its_hunks_describe_both_texts_without_gaps() {
    let seed = std::env::var("WS_SEED").ok().and_then(|s| s.parse().ok()).unwrap_or(0xa119e);
    let cases = std::env::var("WS_CASES").ok().and_then(|s| s.parse().ok()).unwrap_or(1500);
    let mut rng = fastrand::Rng::with_seed(seed);
    for case in 0..cases {
        let base = text(&mut rng, 8);
        // Proposal: base with random changes (replace, insert, delete), sometimes entirely new.
        let mut proposal = base.clone();
        if rng.u8(..10) == 0 {
            proposal = text(&mut rng, 8);
        } else {
            for _ in 0..rng.usize(0..4) {
                match rng.u8(..3) {
                    0 if !proposal.is_empty() => {
                        let at = rng.usize(..proposal.len());
                        proposal[at] = PIECES[rng.usize(..PIECES.len())];
                    }
                    1 => proposal.insert(rng.usize(0..=proposal.len()), PIECES[rng.usize(..PIECES.len())]),
                    _ if !proposal.is_empty() => {
                        proposal.remove(rng.usize(..proposal.len()));
                    }
                    _ => {}
                }
            }
        }
        let (base, proposal) = (join(&base), join(&proposal));
        let hunks = align(&base, &proposal);
        check(&base, &proposal, &hunks, &format!("Fall {case} (Saat {seed}): {base:?} → {proposal:?}"));
        // Torn text too (a piece without an end) does not fail.
        let cut = &proposal[..proposal.char_indices().nth(proposal.chars().count() / 2).map_or(0, |(i, _)| i)];
        let torn = align(&base, cut);
        check(&base, cut, &torn, &format!("Fall {case}, halbiert"));
    }
}

/// Backlog Round 8 ("LCS"): a reworded paragraph next to an inserted one appeared as a single
/// "changed" hunk across both. Now `align` pairs hunks of unequal size by similarity: the reworded
/// paragraph is "changed", the new one "inserted" – two hunks, each at its place.
#[test]
fn a_reworded_paragraph_next_to_an_inserted_one_gives_two_hunks() {
    let after = BASE.replace("Im Beet stehen Kohlköpfe.", "Im Beet stehen heute Kohlköpfe.\n\nEin ganz neuer Gedanke.");
    assert_eq!(
        align(BASE, &after),
        vec![h(HunkKind::Changed, 2..3, 2..3), h(HunkKind::Inserted, 3..3, 3..4)],
        "umformuliert (Block 2) und dahinter eingefügt (vor Block 3 der Basis): zwei Stücke"
    );
    let before =
        BASE.replace("Im Beet stehen Kohlköpfe.", "Ein ganz neuer Gedanke.\n\nIm Beet stehen heute Kohlköpfe.");
    assert_eq!(
        align(BASE, &before),
        vec![h(HunkKind::Inserted, 2..2, 2..3), h(HunkKind::Changed, 2..3, 3..4)],
        "davor eingefügt und umformuliert: erst „eingefügt“ vor Block 2, dann „geändert“ an Block 2"
    );
    let removed = "= Garten\n\nDie Bohnen ranken am Zaun.\n\nIm Beet stehen heute Kohlköpfe.\n";
    assert_eq!(
        align(BASE, removed),
        vec![h(HunkKind::Changed, 2..3, 2..3), h(HunkKind::Removed, 3..4, 3..3)],
        "umformuliert und der Absatz dahinter gestrichen: „geändert“ und „entfernt“"
    );
    let unlike = BASE.replace("Im Beet stehen Kohlköpfe.", "Ganz anders.\n\nNoch etwas.");
    assert_eq!(
        align(BASE, &unlike),
        vec![h(HunkKind::Changed, 2..3, 2..4)],
        "ohne ähnlichen Absatz bleibt es ein Stück (ersetzt durch zwei neue)"
    );
    for (text, case) in [(&after, "dahinter"), (&before, "davor")] {
        let hunks = align(BASE, text);
        check(BASE, text, &hunks, case);
    }
}
