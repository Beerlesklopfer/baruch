// SPDX-License-Identifier: AGPL-3.0-or-later
//! `track_changes` (contract Round 32 §3.4): the proposal lies as tracked changes over the current revision.
//! Probe with a small resolution of its own (not `resolve.rs` of the neighbouring task): deletions removed and
//! insertions kept yields the proposal, the other way round the current revision. All texts are freely invented.

use baruch_docx::diff::track_changes;
use baruch_docx::from_typst::from_typst;
use baruch_docx::{Block, Change, ChangeKind, Inline, ListItem, Marks};

const AUTHOR: &str = "@lektor";
const DATE: &str = "2026-11-24T18:05:00Z";

fn change() -> Change {
    // The kind in the pattern does not matter: track_changes sets it per place itself.
    Change { kind: ChangeKind::Insert, author: AUTHOR.to_owned(), date: DATE.to_owned() }
}

fn text(text: &str) -> Inline {
    Inline::Text { text: text.to_owned(), marks: Marks::default() }
}

fn paragraph(content: &str) -> Block {
    Block::Paragraph { content: vec![text(content)], mark: None }
}

fn blocks_of(typst: &str) -> Vec<Block> {
    from_typst(typst, &["bilder/garten.png".to_owned()]).blocks
}

fn merge(inlines: Vec<Inline>) -> Vec<Inline> {
    let mut out: Vec<Inline> = Vec::new();
    for inline in inlines {
        match (out.last_mut(), inline) {
            (Some(Inline::Text { text: last, marks: m }), Inline::Text { text, marks }) if *m == marks => {
                last.push_str(&text)
            }
            (_, inline) => out.push(inline),
        }
    }
    out
}

/// Own resolution: `keep` is the kind of the changes that stay (Insert = final state, Delete = original).
fn resolve_inlines(inlines: &[Inline], keep: ChangeKind) -> Vec<Inline> {
    let mut out = Vec::new();
    for inline in inlines {
        match inline {
            Inline::Tracked { change, content } => {
                if change.kind == keep {
                    out.extend(resolve_inlines(content, keep));
                }
            }
            Inline::Footnote { content } => out.push(Inline::Footnote { content: resolve_inlines(content, keep) }),
            Inline::Quote { content } => out.push(Inline::Quote { content: resolve_inlines(content, keep) }),
            Inline::Link { url, content } => {
                out.push(Inline::Link { url: url.clone(), content: resolve_inlines(content, keep) });
            }
            other => out.push(other.clone()),
        }
    }
    merge(out)
}

fn resolve(blocks: &[Block], keep: ChangeKind) -> Vec<Block> {
    let mut out = Vec::new();
    // Content of a paragraph whose mark does not apply in this state: it belongs to the following paragraph.
    let mut carried: Vec<Inline> = Vec::new();
    let gone = |mark: &Option<Change>| mark.as_ref().is_some_and(|m| m.kind != keep);
    let with_carried = |content: &[Inline], carried: &mut Vec<Inline>| {
        let mut all = std::mem::take(carried);
        all.extend(resolve_inlines(content, keep));
        merge(all)
    };
    for block in blocks {
        match block {
            Block::Paragraph { content, mark } => {
                let content = with_carried(content, &mut carried);
                if gone(mark) { carried = content } else { out.push(Block::Paragraph { content, mark: None }) }
            }
            Block::Part { content, mark } => {
                let content = with_carried(content, &mut carried);
                if gone(mark) { carried = content } else { out.push(Block::Part { content, mark: None }) }
            }
            Block::Heading { level, numbered, content, mark } => {
                let content = with_carried(content, &mut carried);
                if gone(mark) {
                    carried = content;
                } else {
                    out.push(Block::Heading { level: *level, numbered: *numbered, content, mark: None });
                }
            }
            Block::Quote { blocks, attribution } => {
                let (blocks, attribution) = (resolve(blocks, keep), resolve_inlines(attribution, keep));
                if !blocks.is_empty() || !attribution.is_empty() {
                    out.push(Block::Quote { blocks, attribution });
                }
            }
            Block::List { ordered, start, items } => {
                let items: Vec<ListItem> = items
                    .iter()
                    .filter(|item| !gone(&item.mark))
                    .map(|item| ListItem {
                        depth: item.depth,
                        content: resolve_inlines(&item.content, keep),
                        mark: None,
                    })
                    .collect();
                if !items.is_empty() {
                    out.push(Block::List { ordered: *ordered, start: *start, items });
                }
            }
            Block::Table { header_rows, rows } => out.push(Block::Table {
                header_rows: *header_rows,
                rows: rows.iter().map(|row| row.iter().map(|cell| resolve(cell, keep)).collect()).collect(),
            }),
            Block::Figure { image, alt, caption, raw } => out.push(Block::Figure {
                image: *image,
                alt: alt.clone(),
                caption: resolve_inlines(caption, keep),
                raw: raw.clone(),
            }),
            other => out.push(other.clone()),
        }
    }
    assert!(carried.is_empty(), "nach dem letzten Absatz bleibt kein Text übrig, der nirgends hingehört: {carried:?}");
    out
}

/// Check the promise of the contract on one pair and return the result.
fn tracked(current: &[Block], proposed: &[Block]) -> Vec<Block> {
    let result = track_changes(current, proposed, &change());
    assert_eq!(
        resolve(&result, ChangeKind::Insert),
        proposed,
        "Löschungen entfernt und Einfügungen behalten ergibt den Vorschlag"
    );
    assert_eq!(
        resolve(&result, ChangeKind::Delete),
        current,
        "Einfügungen entfernt und Löschungen behalten ergibt die aktuelle Fassung"
    );
    result
}

fn changes_in(inlines: &[Inline]) -> Vec<(ChangeKind, String)> {
    inlines
        .iter()
        .filter_map(|inline| match inline {
            Inline::Tracked { change, content } => {
                assert_eq!(change.author, AUTHOR, "jede Änderung nennt die Persona als Bearbeiter");
                assert_eq!(change.date, DATE, "und den Zeitpunkt des Vorschlags");
                let text: String = content
                    .iter()
                    .map(|i| match i {
                        Inline::Text { text, .. } => text.as_str(),
                        _ => "¤",
                    })
                    .collect();
                Some((change.kind, text))
            }
            _ => None,
        })
        .collect()
}

#[test]
fn unchanged_text_carries_no_mark() {
    let current =
        blocks_of("= Der Mühlbach\n\nHinter der Mühle lag ein Garten.#footnote[Erfunden.]\n\n- Bohnen\n- Lauch\n");
    let result = tracked(&current, &current);
    assert_eq!(result, current, "ohne Unterschied steht die Fassung unverändert und ohne jede Marke da");
}

#[test]
fn changed_words_are_tracked_inside_the_paragraph() {
    let current = vec![paragraph("Der Bach hinter dem Zaun war schmal und kalt.")];
    let proposed = vec![paragraph("Der Bach hinter dem Zaun war breit und sehr kalt, sagte sie.")];
    let result = tracked(&current, &proposed);
    let [Block::Paragraph { content, mark: None }] = result.as_slice() else {
        panic!("ein Absatz ohne Marke an der Absatzmarke erwartet (der Absatz bleibt bestehen): {result:?}");
    };
    assert_eq!(
        changes_in(content),
        vec![
            (ChangeKind::Delete, "schmal".to_owned()),
            (ChangeKind::Insert, "breit".to_owned()),
            (ChangeKind::Insert, "sehr ".to_owned()),
            (ChangeKind::Insert, ", sagte sie".to_owned()),
        ],
        "nur die geänderten Wörter sind verfolgt, erst die Löschung, dann die Einfügung"
    );
    assert_eq!(
        content[0],
        text("Der Bach hinter dem Zaun war "),
        "der gleich gebliebene Anfang steht als gewöhnlicher Text da"
    );
}

#[test]
fn neighbouring_changes_form_one_place() {
    let current = vec![paragraph("Am alten Zaun blühte Mohn.")];
    let proposed = vec![paragraph("Am neuen Steg blühte Mohn.")];
    let result = tracked(&current, &proposed);
    let Block::Paragraph { content, .. } = &result[0] else { panic!("Absatz erwartet") };
    assert_eq!(
        changes_in(content),
        vec![(ChangeKind::Delete, "alten Zaun".to_owned()), (ChangeKind::Insert, "neuen Steg".to_owned())],
        "zwei benachbarte geänderte Wörter sind eine Stelle – das Leerzeichen dazwischen zerhackt sie nicht"
    );
}

#[test]
fn new_and_removed_blocks_are_marked_at_the_paragraph_mark() {
    let current = vec![
        paragraph("Erster Absatz bleibt."),
        paragraph("Dieser Absatz entfällt ganz."),
        paragraph("Schluss bleibt."),
    ];
    let proposed = vec![
        paragraph("Erster Absatz bleibt."),
        paragraph("Schluss bleibt."),
        Block::Heading { level: 2, numbered: true, content: vec![text("Nachtrag")], mark: None },
        paragraph("Ein völlig neuer Gedanke am Ende."),
    ];
    let result = tracked(&current, &proposed);
    assert_eq!(result.len(), 5, "drei alte und zwei neue Blöcke: {result:?}");
    assert_eq!(result[0], current[0], "gleich gebliebene Absätze tragen keine Marke");
    assert_eq!(result[2], current[2], "auch der Schluss nicht");
    let Block::Paragraph { content, mark: Some(mark) } = &result[1] else {
        panic!("der entfallene Absatz trägt die Marke an der Absatzmarke: {:?}", result[1]);
    };
    assert_eq!(
        (mark.kind, mark.author.as_str(), mark.date.as_str()),
        (ChangeKind::Delete, AUTHOR, DATE),
        "als Löschung der Persona"
    );
    assert_eq!(
        changes_in(content),
        vec![(ChangeKind::Delete, "Dieser Absatz entfällt ganz.".to_owned())],
        "sein ganzer Text ist gelöscht"
    );
    assert!(
        matches!(&result[3], Block::Heading { level: 2, mark: Some(m), .. } if m.kind == ChangeKind::Insert),
        "die neue Überschrift ist als Einfügung markiert: {:?}",
        result[3]
    );
    assert!(
        matches!(&result[4], Block::Paragraph { mark: Some(m), .. } if m.kind == ChangeKind::Insert),
        "der neue Absatz ebenso: {:?}",
        result[4]
    );
}

#[test]
fn a_rewritten_paragraph_next_to_a_new_one_is_matched_by_similarity() {
    let current = vec![paragraph("Die Müllerin zählte am Abend die Säcke im Speicher.")];
    let proposed = vec![
        paragraph("Draußen fiel der erste Schnee."),
        paragraph("Die Müllerin zählte am Abend müde die Säcke im Speicher."),
    ];
    let result = tracked(&current, &proposed);
    assert!(
        matches!(&result[0], Block::Paragraph { mark: Some(m), .. } if m.kind == ChangeKind::Insert),
        "der neue Absatz davor ist ganz eingefügt: {:?}",
        result[0]
    );
    let Block::Paragraph { content, mark: None } = &result[1] else {
        panic!("der überarbeitete Absatz bleibt bestehen: {:?}", result[1])
    };
    assert_eq!(
        changes_in(content),
        vec![(ChangeKind::Insert, "müde ".to_owned())],
        "und zeigt nur das eine neue Wort – nicht Löschung und Neufassung des ganzen Absatzes"
    );
}

#[test]
fn one_block_replaced_by_one_of_the_same_kind_stays_one_block() {
    let current = vec![paragraph("Bleibt."), paragraph("Oktober"), paragraph("Schluss.")];
    let proposed = vec![paragraph("Bleibt."), paragraph("November"), paragraph("Schluss.")];
    let result = tracked(&current, &proposed);
    let [_, Block::Paragraph { content, mark: None }, _] = result.as_slice() else {
        panic!("drei Absätze erwartet, der mittlere ohne Marke an der Absatzmarke: {result:?}");
    };
    assert_eq!(
        changes_in(content),
        vec![(ChangeKind::Delete, "Oktober".to_owned()), (ChangeKind::Insert, "November".to_owned())],
        "auch ohne gemeinsames Wort ist das eine Änderung im selben Absatz – nicht ein gestrichener und ein neuer Absatz"
    );
}

#[test]
fn headings_lists_quotes_footnotes_and_marks_survive_both_ways() {
    let current = blocks_of(
        "= Das Beet\n\nIm Beet wuchsen *Bohnen* und Ringelblumen.#footnote[Beides erfunden.]\n\n- Bohnen stecken\n- Lauch \
         setzen\n- gießen\n\n#quote(block: true, attribution: [Spruch der Müllerin])[Wer sät, braucht Geduld.]\n\n\
         #table(columns: 2, [Sorte], [Zeit], [Bohne], [Mitte Mai])\n",
    );
    let proposed = blocks_of(
        "= Das große Beet\n\nIm Beet wuchsen _Bohnen_ und Ringelblumen.#footnote[Beides frei erfunden.]\n\n- Bohnen \
         stecken\n- Lauch pflanzen\n  - mit Abstand\n\n#quote(block: true, attribution: [Spruch des Müllers])[Wer sät, \
         braucht viel Geduld.]\n\n#table(columns: 2, [Sorte], [Zeit], [Bohne], [Mitte Juni])\n\nEin Schlusssatz.\n",
    );
    let result = tracked(&current, &proposed);
    assert!(
        matches!(&result[0], Block::Heading { level: 1, mark: None, content, .. }
            if changes_in(content) == vec![(ChangeKind::Insert, "große ".to_owned())]),
        "die Überschrift bleibt Überschrift 1 und zeigt das neue Wort: {:?}",
        result[0]
    );
    let Block::List { items, .. } = &result[2] else { panic!("Liste erwartet: {:?}", result[2]) };
    let marks: Vec<Option<ChangeKind>> = items.iter().map(|i| i.mark.as_ref().map(|m| m.kind)).collect();
    assert_eq!(
        marks,
        vec![None, None, Some(ChangeKind::Delete), Some(ChangeKind::Insert)],
        "gleicher Punkt ohne Marke, geänderter Punkt wortweise, der entfallene und der neue (tiefere) Punkt mit Marke"
    );
    let Block::Table { rows, .. } = &result[4] else { panic!("Tabelle erwartet: {:?}", result[4]) };
    let Block::Paragraph { content, .. } = &rows[1][1][0] else { panic!("Absatz in der Zelle erwartet") };
    assert_eq!(
        changes_in(content),
        vec![(ChangeKind::Delete, "Mai".to_owned()), (ChangeKind::Insert, "Juni".to_owned())],
        "in einer Tabelle gleicher Größe ist die geänderte Zelle wortweise verfolgt"
    );
}

#[test]
fn new_figures_and_islands_vanish_in_the_other_state() {
    let current = blocks_of("Vor dem Bild.\n\nNach dem Bild.\n");
    let proposed = blocks_of(
        "Vor dem Bild.\n\n#figure(image(\"bilder/garten.png\", alt: \"Beet\"), caption: [Das Beet])\n\n#unbekannt[Kasten]\n\nNach \
         dem Bild.\n",
    );
    let result = track_changes(&current, &proposed, &change());
    assert_eq!(
        resolve(&result, ChangeKind::Delete),
        current,
        "im Original sind die neue Abbildung und die neue Insel wirklich nicht da"
    );
    let inserted: Vec<&Inline> = result
        .iter()
        .filter_map(|b| match b {
            Block::Paragraph { content, mark: Some(m) } if m.kind == ChangeKind::Insert => content.first(),
            _ => None,
        })
        .collect();
    assert!(
        matches!(inserted[0], Inline::Tracked { content, .. } if matches!(content[0], Inline::Image { image: 0, ref alt } if alt == "Beet")),
        "die Abbildung steht als verfolgtes Bild in einem eingefügten Absatz (das Modell kennt keine verfolgte Abbildung): {inserted:?}"
    );
    assert_eq!(
        changes_in(std::slice::from_ref(inserted[1])),
        vec![(ChangeKind::Insert, "Das Beet".to_owned())],
        "ihre Unterschrift als eingefügter Absatz"
    );
    assert!(
        matches!(inserted[2], Inline::Tracked { content, .. } if matches!(&content[0], Inline::Typst { raw, .. } if raw == "#unbekannt[Kasten]")),
        "die Insel steht mit wörtlichem Quelltext als verfolgte Insel im Text: {inserted:?}"
    );

    // The other way round: what the proposal deletes is gone in the final state.
    let back = track_changes(&proposed, &current, &change());
    assert_eq!(resolve(&back, ChangeKind::Insert), current, "eine gestrichene Abbildung und Insel fehlen im Endstand");
}

#[test]
fn page_breaks_follow_the_proposal_and_whole_tables_keep_their_grid() {
    let current = vec![
        paragraph("Text."),
        Block::PageBreak,
        Block::PageBreak,
        blocks_of("#table(columns: 1, [Rübe])\n").remove(0),
    ];
    let proposed = vec![Block::PageBreak, paragraph("Text.")];
    let result = track_changes(&current, &proposed, &change());
    assert_eq!(
        result.iter().filter(|b| matches!(b, Block::PageBreak)).count(),
        1,
        "Seitenumbrüche lassen sich nicht verfolgen: es gelten die des Vorschlags (einer), ein gestrichener entfällt"
    );
    let Some(Block::Table { rows, .. }) = result.iter().find(|b| matches!(b, Block::Table { .. })) else {
        panic!("die gestrichene Tabelle bleibt als Gitter stehen: {result:?}");
    };
    assert!(
        matches!(&rows[0][0][0], Block::Paragraph { content, mark: Some(m) }
            if m.kind == ChangeKind::Delete && changes_in(content) == vec![(ChangeKind::Delete, "Rübe".to_owned())]),
        "ihr Inhalt ist in jeder Zelle als gelöscht verfolgt (ehrliche Grenze: die leere Tabelle bleibt im Endstand)"
    );
}
