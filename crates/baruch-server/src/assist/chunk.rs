// SPDX-License-Identifier: AGPL-3.0-or-later
//! Chunking and merging (Round 15, “lektor”): pure helpers without model and storage.
//!
//! If an order with the whole chapter does not fit into the persona's context length, [`super::run`] splits the
//! review subject into pieces that are each reviewed verbatim; what is not in the core of the piece the
//! persona sees as a summary (by the model, one after another) or – if that does not fit either – not at all, and
//! exactly that is stated in the note „Komprimiert: …“. Only the building blocks for this live here:
//!
//! - [`split`]: text into pieces of at most `max` characters, boundaries preferably at paragraphs, then at the end
//!   of a sentence, then at whitespace; consecutive pieces overlap by about `overlap` characters (so a finding at
//!   the boundary does not fall through).
//! - [`pack`]: bundle consecutive parts (sections) into pieces.
//! - [`span_label`]: label of a partial piece („Zeichen 1–4000 von Abschnitt 1.2 Pollen“).
//! - [`findings`], [`merge_pieces`]: read findings from the answers per piece and merge them without duplicates
//!   (equal findings from the overlap of neighbouring pieces appear only once; the more detailed one stays;
//!   since Round 16a equally worded findings on different sections both stay).

/// Range of a text in characters (not bytes): `start` inclusive, `end` exclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    pub fn is_empty(&self) -> bool {
        self.end <= self.start
    }

    /// The text of this range.
    pub fn of(&self, text: &str) -> String {
        text.chars().skip(self.start).take(self.len()).collect()
    }
}

/// Best cut in `chars[from..to]` (index after the boundary): paragraph, else end of sentence, else whitespace,
/// else `to`.
fn cut_point(chars: &[char], from: usize, to: usize) -> usize {
    let paragraph = (from + 1..to).rev().find(|&i| chars[i] == '\n' && chars[i - 1] == '\n').map(|i| i + 1);
    if let Some(cut) = paragraph {
        return cut;
    }
    let sentence = (from + 1..to)
        .rev()
        .find(|&i| chars[i].is_whitespace() && matches!(chars[i - 1], '.' | '!' | '?' | ':' | ';'))
        .map(|i| i + 1);
    if let Some(cut) = sentence {
        return cut;
    }
    (from..to).rev().find(|&i| chars[i].is_whitespace()).map_or(to, |i| i + 1)
}

/// Split text into pieces of at most `max` characters (see module doc); the next piece starts about
/// `overlap` characters before the end of the previous one, at a word boundary. Empty text → no pieces; if the
/// text fits as a whole, exactly one piece. The pieces cover the whole text, each is non-empty.
pub fn split(text: &str, max: usize, overlap: usize) -> Vec<Span> {
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    if len == 0 {
        return Vec::new();
    }
    let max = max.max(1);
    let overlap = overlap.min(max / 2);
    let mut spans = Vec::new();
    let mut start = 0;
    loop {
        let hard_end = (start + max).min(len);
        if hard_end == len {
            spans.push(Span { start, end: len });
            return spans;
        }
        // Do not cut before the middle of the piece, otherwise the pieces would be needlessly small.
        let end = cut_point(&chars, start + max / 2, hard_end).max(start + 1);
        spans.push(Span { start, end });
        let mut next = end.saturating_sub(overlap).max(start + 1);
        // Start at a word boundary (after whitespace), but never behind the end of the previous piece.
        if next < end
            && let Some(space) = (next..end).find(|&i| chars[i].is_whitespace())
        {
            next = space + 1;
        }
        start = next.min(end);
    }
}

/// Bundle consecutive parts with the sizes `sizes` into pieces of at most `max` (a part that is larger
/// on its own forms its own piece). Returns per piece the indices of its parts.
pub fn pack(sizes: &[usize], max: usize) -> Vec<Vec<usize>> {
    let mut groups: Vec<Vec<usize>> = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    let mut used = 0;
    for (index, &size) in sizes.iter().enumerate() {
        if !current.is_empty() && used + size > max {
            groups.push(std::mem::take(&mut current));
            used = 0;
        }
        current.push(index);
        used += size;
    }
    if !current.is_empty() {
        groups.push(current);
    }
    groups
}

/// Label of a partial piece of a section (characters from 1, as a human counts).
pub fn span_label(span: Span, label: &str) -> String {
    format!("Zeichen {}–{} von Abschnitt {label}", span.start + 1, span.end)
}

/// Does the line start a new finding? List marker, enumeration or „Kritisch:“.
fn starts_item(line: &str) -> bool {
    let t = line.trim_start();
    if t.starts_with(['-', '*', '•', '–']) {
        return true;
    }
    if t.to_lowercase().starts_with("kritisch:") {
        return true;
    }
    let digits = t.chars().take_while(char::is_ascii_digit).count();
    digits > 0 && t[digits..].starts_with(['.', ')'])
}

/// Findings of an answer: per list item (or „Kritisch:“ line or paragraph) one entry including
/// continuation lines; empty lines separate.
pub fn findings(answer: &str) -> Vec<String> {
    let mut items: Vec<String> = Vec::new();
    let mut current = String::new();
    for line in answer.lines() {
        if line.trim().is_empty() {
            if !current.trim().is_empty() {
                items.push(current.trim_end().to_owned());
            }
            current.clear();
            continue;
        }
        if starts_item(line) && !current.trim().is_empty() {
            items.push(current.trim_end().to_owned());
            current.clear();
        }
        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(line.trim_end());
    }
    if !current.trim().is_empty() {
        items.push(current.trim_end().to_owned());
    }
    items
}

/// Comparison form of a finding: lower case, letters and digits only, whitespace collapsed, without
/// list marker and number at the start.
fn normal(item: &str) -> String {
    let t = item.trim_start().trim_start_matches(['-', '*', '•', '–', ' ']);
    let digits = t.chars().take_while(char::is_ascii_digit).count();
    let t = if digits > 0 && t[digits..].starts_with(['.', ')']) { &t[digits + 1..] } else { t };
    t.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Shortest length (comparison form) from which “contained in the other” counts as duplicate; shorter ones count
/// only when equal.
const CONTAINED_MIN: usize = 24;

fn duplicate(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    short.len() >= CONTAINED_MIN && long.contains(short)
}

/// Merge answers per piece into one if **every** piece overlaps its predecessor (partial pieces of a
/// section): like [`merge_pieces`] with `overlaps_previous` true everywhere.
pub fn merge(answers: &[String]) -> String {
    merge_pieces(answers, &vec![true; answers.len()])
}

/// Merge answers per piece into one (Round 16a): findings in the order of the pieces. A finding is a duplicate
/// (equal, or one fully contained in the other) only against a finding of the **directly preceding** piece, and
/// only if this piece overlaps it (`overlaps_previous[k]` for piece `k`) – then the more detailed one stays in
/// the place of the first. So a finding from the overlap appears once, while equally worded findings on
/// different sections (or within one answer) both stay.
pub fn merge_pieces(answers: &[String], overlaps_previous: &[bool]) -> String {
    // (comparison form, finding, piece it last came from)
    let mut kept: Vec<(String, String, usize)> = Vec::new();
    for (k, answer) in answers.iter().enumerate() {
        let joins = k > 0 && overlaps_previous.get(k).copied().unwrap_or(false);
        for item in findings(answer) {
            let key = normal(&item);
            if key.is_empty() {
                continue;
            }
            let slot =
                if joins { kept.iter_mut().find(|(c, _, from)| *from == k - 1 && duplicate(c, &key)) } else { None };
            match slot {
                Some(slot) if key.len() > slot.0.len() => *slot = (key, item, k),
                Some(slot) => slot.2 = k,
                None => kept.push((key, item, k)),
            }
        }
    }
    kept.into_iter().map(|(_, item, _)| item).collect::<Vec<_>>().join("\n")
}

/// How a part of the surroundings appears in a piece.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Repr {
    Wording,
    Summary,
    Omitted,
}

/// Plan of a piece: the parts in the core (reviewed verbatim) and per remaining part its representation, in the
/// order of the work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PiecePlan {
    pub focus: Vec<usize>,
    pub context: Vec<(usize, Repr)>,
}

/// Room that a line of omitted parts needs anyway (head with note and markers); since Round 18b
/// once per run of consecutive omitted parts that stand in one line
/// (`context::OMITTED_HEAD`).
pub const OMITTED_OVERHEAD: usize = 100;
/// Room per omitted part in this line next to its title (separator `; `).
pub const OMITTED_ITEM: usize = 2;

/// Plan for a piece with the core `focus` among `count` parts: first everything verbatim; as long as it does not
/// fit into `room`, the part farthest from the core is replaced by its summary, then – if it still does not
/// fit – omitted (again the farthest first). `wording(i)`, `summary(i)`, `title(i)`
/// give the room of a part verbatim, as a summary and as a bare title. An omitted part
/// costs its title and [`OMITTED_OVERHEAD`] (new line); next to an already omitted neighbour it stands
/// in that neighbour's line and costs only title and [`OMITTED_ITEM`] (Round 18b).
pub fn plan_piece(
    count: usize,
    focus: &[usize],
    room: usize,
    wording: &dyn Fn(usize) -> usize,
    summary: &dyn Fn(usize) -> usize,
    title: &dyn Fn(usize) -> usize,
) -> PiecePlan {
    let others: Vec<usize> = (0..count).filter(|i| !focus.contains(i)).collect();
    let mut repr: Vec<Repr> = vec![Repr::Wording; count];
    let mut total: usize =
        focus.iter().map(|&i| wording(i)).sum::<usize>() + others.iter().map(|&i| wording(i)).sum::<usize>();
    let center = if focus.is_empty() { 0.0 } else { focus.iter().sum::<usize>() as f64 / focus.len() as f64 };
    let mut far = others.clone();
    // Farthest first; at equal distance the later one (the earlier text matters more for understanding).
    far.sort_by(|&a, &b| {
        let (da, db) = ((a as f64 - center).abs(), (b as f64 - center).abs());
        db.partial_cmp(&da).unwrap_or(std::cmp::Ordering::Equal).then(b.cmp(&a))
    });
    for &i in &far {
        if total <= room {
            break;
        }
        let (w, s) = (wording(i), summary(i));
        if s < w {
            total = total - w + s;
            repr[i] = Repr::Summary;
        }
    }
    for &i in &far {
        if total <= room {
            break;
        }
        let now = if repr[i] == Repr::Summary { summary(i) } else { wording(i) };
        let joins = (i > 0 && repr[i - 1] == Repr::Omitted) || repr.get(i + 1) == Some(&Repr::Omitted);
        let marker = title(i) + if joins { OMITTED_ITEM } else { OMITTED_OVERHEAD };
        if marker < now {
            total = total - now + marker;
            repr[i] = Repr::Omitted;
        }
    }
    PiecePlan { focus: focus.to_vec(), context: others.into_iter().map(|i| (i, repr[i])).collect() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_short_text_is_one_piece_and_an_empty_text_none() {
        assert_eq!(split("kurz", 100, 10), vec![Span { start: 0, end: 4 }], "passt ganz → ein Stück");
        assert!(split("", 100, 10).is_empty(), "leerer Text → keine Stücke (nichts zu prüfen)");
    }

    #[test]
    fn pieces_cover_the_text_overlap_and_prefer_paragraphs() {
        let text = format!(
            "{}\n\n{}\n\n{}",
            "Erster Absatz. ".repeat(10),
            "Zweiter Absatz. ".repeat(10),
            "Dritter. ".repeat(10)
        );
        let spans = split(&text, 200, 40);
        let len = text.chars().count();
        assert!(spans.len() >= 2, "zu lang für ein Stück: {spans:?}");
        assert_eq!(spans[0].start, 0, "das erste Stück beginnt am Anfang");
        assert_eq!(spans.last().unwrap().end, len, "das letzte endet am Ende – nichts fällt weg");
        for pair in spans.windows(2) {
            assert!(pair[1].start < pair[0].end, "aufeinanderfolgende Stücke überlappen: {pair:?}");
            assert!(pair[1].start > pair[0].start, "jedes Stück kommt voran (keine Endlosschleife): {pair:?}");
        }
        for span in &spans {
            assert!(span.len() <= 200 && !span.is_empty(), "höchstens max Zeichen, nie leer: {span:?}");
        }
        let first = spans[0].of(&text);
        assert!(first.ends_with("\n\n"), "Schnitt an der Absatzgrenze, nicht mitten im Satz: {first:?}");
    }

    #[test]
    fn without_paragraphs_the_cut_falls_after_a_sentence_and_never_inside_a_word() {
        let text = "Die Biene fliegt. ".repeat(30);
        let spans = split(&text, 100, 20);
        for span in &spans[..spans.len() - 1] {
            let piece = span.of(&text);
            assert!(piece.trim_end().ends_with('.'), "Schnitt am Satzende: {piece:?}");
        }
        for span in &spans[1..] {
            let before = text.chars().nth(span.start - 1).unwrap();
            assert!(before.is_whitespace(), "ein Stück beginnt nach Leerraum, nie mitten im Wort: {span:?}");
        }
        let word = "x".repeat(250);
        let spans = split(&word, 100, 10);
        assert_eq!(spans.last().unwrap().end, 250, "auch ein Wurm ohne Leerraum wird ganz abgedeckt: {spans:?}");
    }

    #[test]
    fn parts_are_packed_in_order_and_an_oversized_part_stands_alone() {
        assert_eq!(pack(&[30, 30, 30, 120, 10], 70), vec![vec![0, 1], vec![2], vec![3], vec![4]], "Reihenfolge bleibt");
        assert!(pack(&[], 10).is_empty(), "keine Teile, keine Stücke");
    }

    #[test]
    fn a_piece_label_counts_from_one() {
        assert_eq!(
            span_label(Span { start: 0, end: 4000 }, "1.2 Pollen"),
            "Zeichen 1–4000 von Abschnitt 1.2 Pollen",
            "wie ein Mensch zählt (Vertrag §2: „Zeichen a–b von Abschnitt …“)"
        );
    }

    #[test]
    fn the_farthest_parts_become_summaries_first_and_are_omitted_only_when_even_that_fails() {
        // Five parts of 400 characters each, core is part 2; summary 150, title 10 (+100 note).
        let wording = |_: usize| 400;
        let summary = |_: usize| 150;
        let title = |_: usize| 10;
        let all = plan_piece(5, &[2], 2000, &wording, &summary, &title);
        assert!(all.context.iter().all(|(_, r)| *r == Repr::Wording), "passt: alles im Wortlaut: {all:?}");
        let tight = plan_piece(5, &[2], 1500, &wording, &summary, &title);
        assert_eq!(
            tight.context,
            vec![(0, Repr::Summary), (1, Repr::Wording), (3, Repr::Wording), (4, Repr::Summary)],
            "2000 → 1750 → 1500: die beiden fernsten werden zusammengefasst, die Nachbarn bleiben im Wortlaut"
        );
        // All summarised: 400 + 4·150 = 1000; without part 4: 1000 − 150 + 110 = 960.
        let tiny = plan_piece(5, &[2], 970, &wording, &summary, &title);
        assert_eq!(
            tiny.context.iter().filter(|(_, r)| *r == Repr::Omitted).map(|(i, _)| *i).collect::<Vec<_>>(),
            vec![4],
            "reicht auch die Zusammenfassung nicht, fällt zuerst der fernste (bei gleichem Abstand der spätere)"
        );
        assert_eq!(tiny.focus, vec![2], "der Kern bleibt immer im Wortlaut");
        let small = |_: usize| 50;
        let never = plan_piece(3, &[0], 10, &wording, &small, &title);
        assert!(
            never.context.iter().all(|(_, r)| *r == Repr::Summary),
            "Auslassen nur, wenn es Platz spart (der Hinweis ist länger als eine kurze Zusammenfassung): {never:?}"
        );
    }

    #[test]
    fn neighbouring_omitted_parts_share_one_line_and_cost_only_their_titles() {
        // Round 18b: twenty parts of 400 characters each, core part 0, summaries too long (500), title 10.
        // The first omitted part costs 10 + 100, every neighbour after it only 10 + 2 (the same line).
        let plan = plan_piece(20, &[0], 400 + 110 + 18 * 12, &|_| 400, &|_| 500, &|_| 10);
        assert!(
            plan.context.iter().all(|(_, r)| *r == Repr::Omitted),
            "alle 19 Umfeldteile passen ausgelassen, weil sie sich eine Zeile teilen (früher 19·110): {plan:?}"
        );
        let old_cost = 400 + 19 * (10 + OMITTED_OVERHEAD);
        assert!(old_cost > 400 + 110 + 18 * 12, "nach der alten Rechnung hätte es nicht gepasst");
    }

    #[test]
    fn findings_split_on_list_items_critical_lines_and_blank_lines() {
        let answer = "Kritisch: „immer“ in Satz 3 ist zu stark.\n- Wiederholung von „Licht“\n  im zweiten Absatz\n\nSchluss fehlt.";
        assert_eq!(
            findings(answer),
            vec![
                "Kritisch: „immer“ in Satz 3 ist zu stark.".to_owned(),
                "- Wiederholung von „Licht“\n  im zweiten Absatz".to_owned(),
                "Schluss fehlt.".to_owned(),
            ],
            "Folgezeilen bleiben beim Listenpunkt, Absätze sind eigene Befunde"
        );
    }

    #[test]
    fn merging_drops_duplicates_from_the_overlap_and_keeps_the_fuller_one() {
        let one = "Kritisch: „immer“ in Satz 3 behauptet zu viel.\n- Der Übergang zu den Pollen fehlt.".to_owned();
        let two = "- Kritisch: „immer“ in Satz 3 behauptet zu viel!\n\
                   - Der Übergang zu den Pollen fehlt; ein Satz über das Brutnest würde helfen.\n\
                   - Das Wort „Höschen“ erklären."
            .to_owned();
        let merged = merge(&[one, two]);
        assert_eq!(
            merged,
            "Kritisch: „immer“ in Satz 3 behauptet zu viel.\n\
             - Der Übergang zu den Pollen fehlt; ein Satz über das Brutnest würde helfen.\n\
             - Das Wort „Höschen“ erklären.",
            "gleicher Befund nur einmal (Satzzeichen und Listenzeichen egal), der ausführlichere ersetzt den \
             kürzeren an seiner Stelle, neue kommen dazu"
        );
        assert_eq!(
            merge(&["- ja".to_owned(), "- ja, aber".to_owned()]),
            "- ja\n- ja, aber",
            "kurze nur bei Gleichheit doppelt"
        );
    }

    #[test]
    fn only_overlaps_of_neighbouring_pieces_are_merged_and_equal_findings_on_other_sections_stay() {
        let same = "- Kritisch: „immer“ behauptet zu viel.".to_owned();
        assert_eq!(
            merge_pieces(&[same.clone(), same.clone()], &[false, false]),
            format!("{same}\n{same}"),
            "zwei Stücke verschiedener Abschnitte (keine Überlappung): gleich formulierte Befunde bleiben beide stehen"
        );
        assert_eq!(
            merge_pieces(&[same.clone(), "- Neu.".into(), same.clone()], &[false, true, true]),
            format!("{same}\n- Neu.\n{same}"),
            "Stück 3 überlappt nur mit Stück 2, nicht mit Stück 1: der Befund aus Stück 1 zählt nicht als Doppel"
        );
        assert_eq!(
            merge_pieces(&[format!("{same}\n{same}")], &[false]),
            format!("{same}\n{same}"),
            "innerhalb einer Antwort wird nichts zusammengeführt"
        );
        assert_eq!(
            merge_pieces(&[same.clone(), same.clone(), same.clone()], &[false, true, true]),
            same,
            "eine Kette von Überlappungen (Teilstücke eines Abschnitts): derselbe Befund nur einmal"
        );
    }
}
