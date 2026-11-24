// SPDX-License-Identifier: AGPL-3.0-or-later
//! Stückeln und Zusammenführen (Runde 15, „lektor“): reine Hilfen ohne Modell und Speicher.
//!
//! Passt ein Auftrag mit dem ganzen Kapitel nicht in die Kontextlänge der Persona, teilt [`super::run`] den
//! Prüfgegenstand in Stücke, die je im Wortlaut geprüft werden; was nicht im Kern des Stücks liegt, sieht die
//! Persona als Zusammenfassung (vom Modell, nacheinander) oder – wenn auch das nicht passt – gar nicht, und
//! genau das steht im Vermerk „Komprimiert: …“. Hier liegen nur die Bausteine dafür:
//!
//! - [`split`]: Text in Stücke von höchstens `max` Zeichen, Grenzen bevorzugt an Absätzen, dann am Satzende,
//!   dann an Leerraum; aufeinanderfolgende Stücke überlappen um etwa `overlap` Zeichen (ein Befund an der
//!   Grenze fällt so nicht durch).
//! - [`pack`]: aufeinanderfolgende Teile (Abschnitte) zu Stücken bündeln.
//! - [`span_label`]: Bezeichnung eines Teilstücks („Zeichen 1–4000 von Abschnitt 1.2 Pollen“).
//! - [`findings`], [`merge_pieces`]: Befunde aus den Antworten je Stück lesen und ohne Doppelte zusammenführen
//!   (gleiche Befunde aus der Überlappung benachbarter Stücke erscheinen nur einmal; der ausführlichere bleibt;
//!   seit Runde 16a bleiben gleich formulierte Befunde zu verschiedenen Abschnitten beide stehen).

/// Bereich eines Texts in Zeichen (nicht Bytes): `start` einschließlich, `end` ausschließlich.
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

    /// Der Text dieses Bereichs.
    pub fn of(&self, text: &str) -> String {
        text.chars().skip(self.start).take(self.len()).collect()
    }
}

/// Beste Schnittstelle in `chars[from..to]` (Index nach der Grenze): Absatz, sonst Satzende, sonst Leerraum,
/// sonst `to`.
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

/// Text in Stücke von höchstens `max` Zeichen teilen (siehe Moduldoku); das nächste Stück beginnt etwa
/// `overlap` Zeichen vor dem Ende des vorigen, an einer Wortgrenze. Leerer Text → keine Stücke; passt der
/// Text ganz, genau ein Stück. Die Stücke decken den ganzen Text ab, jedes ist nicht leer.
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
        // Nicht vor der Mitte des Stücks schneiden, sonst würden die Stücke unnötig klein.
        let end = cut_point(&chars, start + max / 2, hard_end).max(start + 1);
        spans.push(Span { start, end });
        let mut next = end.saturating_sub(overlap).max(start + 1);
        // An einer Wortgrenze beginnen (nach Leerraum), aber nie hinter dem Ende des vorigen Stücks.
        if next < end
            && let Some(space) = (next..end).find(|&i| chars[i].is_whitespace())
        {
            next = space + 1;
        }
        start = next.min(end);
    }
}

/// Aufeinanderfolgende Teile mit den Größen `sizes` zu Stücken von höchstens `max` bündeln (ein Teil, der
/// allein größer ist, bildet ein eigenes Stück). Liefert je Stück die Indizes seiner Teile.
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

/// Bezeichnung eines Teilstücks eines Abschnitts (Zeichen ab 1, wie ein Mensch zählt).
pub fn span_label(span: Span, label: &str) -> String {
    format!("Zeichen {}–{} von Abschnitt {label}", span.start + 1, span.end)
}

/// Beginnt die Zeile einen neuen Befund? Listenzeichen, Aufzählung oder „Kritisch:“.
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

/// Befunde einer Antwort: je Listenpunkt (bzw. „Kritisch:“-Zeile oder Absatz) ein Eintrag samt
/// Folgezeilen; Leerzeilen trennen.
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

/// Vergleichsform eines Befunds: klein, nur Buchstaben und Ziffern, Leerraum zusammengezogen, ohne
/// Listenzeichen und Nummer am Anfang.
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

/// Kürzeste Länge (Vergleichsform), ab der „steckt im anderen“ als doppelt gilt; kürzere zählen nur bei
/// Gleichheit.
const CONTAINED_MIN: usize = 24;

fn duplicate(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let (short, long) = if a.len() <= b.len() { (a, b) } else { (b, a) };
    short.len() >= CONTAINED_MIN && long.contains(short)
}

/// Antworten je Stück zu einer zusammenführen, wenn **jedes** Stück mit seinem Vorgänger überlappt (Teilstücke eines
/// Abschnitts): wie [`merge_pieces`] mit `overlaps_previous` überall wahr.
pub fn merge(answers: &[String]) -> String {
    merge_pieces(answers, &vec![true; answers.len()])
}

/// Antworten je Stück zu einer zusammenführen (Runde 16a): Befunde in der Reihenfolge der Stücke. Doppelt (gleich
/// oder einer steckt ganz im anderen) ist ein Befund nur gegenüber einem Befund des **direkt vorigen** Stücks, und
/// nur wenn dieses Stück mit ihm überlappt (`overlaps_previous[k]` für Stück `k`) – dann bleibt der ausführlichere an
/// der Stelle des ersten. So erscheint ein Befund aus der Überlappung einmal, während gleich formulierte Befunde zu
/// verschiedenen Abschnitten (oder innerhalb einer Antwort) beide stehen bleiben.
pub fn merge_pieces(answers: &[String], overlaps_previous: &[bool]) -> String {
    // (Vergleichsform, Befund, Stück, aus dem er zuletzt kam)
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

/// Wie ein Teil des Umfelds in einem Stück erscheint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Repr {
    Wording,
    Summary,
    Omitted,
}

/// Plan eines Stücks: die Teile im Kern (im Wortlaut geprüft) und je übrigem Teil seine Darstellung, in der
/// Reihenfolge des Werks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PiecePlan {
    pub focus: Vec<usize>,
    pub context: Vec<(usize, Repr)>,
}

/// Platz, den eine Zeile ausgelassener Teile trotzdem braucht (Kopf mit Hinweis und Marken); seit Runde 18b
/// einmal je Folge aufeinanderfolgender ausgelassener Teile, die in einer Zeile stehen
/// (`context::OMITTED_HEAD`).
pub const OMITTED_OVERHEAD: usize = 100;
/// Platz je ausgelassenem Teil in dieser Zeile neben seinem Titel (Trenner `; `).
pub const OMITTED_ITEM: usize = 2;

/// Plan für ein Stück mit dem Kern `focus` unter `count` Teilen: zunächst alles im Wortlaut; solange es nicht in
/// `room` passt, wird der vom Kern am weitesten entfernte Teil durch seine Zusammenfassung ersetzt, danach – wenn
/// es immer noch nicht passt – ausgelassen (wieder der fernste zuerst). `wording(i)`, `summary(i)`, `title(i)`
/// geben den Platz eines Teils im Wortlaut, als Zusammenfassung und als bloßer Titel. Ein ausgelassener Teil
/// kostet seinen Titel und [`OMITTED_OVERHEAD`] (neue Zeile); neben einem schon ausgelassenen Nachbarn steht er
/// in dessen Zeile und kostet nur Titel und [`OMITTED_ITEM`] (Runde 18b).
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
    // Fernste zuerst; bei gleichem Abstand der spätere (der frühere Text ist für das Verständnis wichtiger).
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
        // Fünf Teile zu je 400 Zeichen, Kern ist Teil 2; Zusammenfassung 150, Titel 10 (+100 Hinweis).
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
        // Alle zusammengefasst: 400 + 4·150 = 1000; ohne Teil 4: 1000 − 150 + 110 = 960.
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
        // Runde 18b: Zwanzig Teile zu je 400 Zeichen, Kern Teil 0, Zusammenfassungen zu lang (500), Titel 10.
        // Der erste ausgelassene Teil kostet 10 + 100, jeder Nachbar danach nur 10 + 2 (dieselbe Zeile).
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
