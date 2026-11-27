// SPDX-License-Identifier: AGPL-3.0-or-later
//! Cancellation of a Typst run (Round 18a): [`baruch_render::compile_cancellable`] with [`baruch_render::Cancel`]
//! on freely invented, deliberately slow sources. What is measured is what is really interrupted; the numbers
//! are in the output with `--nocapture` (`Messung …`) and in `docs/Architektur.md` („Runde 18a“).
//!
//! Only what does not depend on the load of the machine can be asserted firmly: a cancelled run delivers
//! `None` (never half pages), a cancellation before the run returns immediately, and a later run of the same
//! source typesets completely (the cancellation poisons no intermediate results of Typst).

use std::thread;
use std::time::{Duration, Instant};

use baruch_render::{Cancel, Source, compile, compile_cancellable};

/// Long running text of invented words; `tag` makes the text different per run, so that no run reuses
/// typesetting results of another from the memory of Typst.
fn long_text(tag: &str, paragraphs: usize) -> Source {
    let mut main = String::from("#set page(width: 10cm, height: 14cm)\n#set text(lang: \"de\", size: 9pt)\n");
    main.push_str("#set par(justify: true)\n= Langer Text\n");
    for p in 0..paragraphs {
        for w in 0..90 {
            main.push_str(&format!("{tag}wort{p}x{w} "));
        }
        main.push_str("\n\n");
    }
    Source { main, files: Vec::new() }
}

/// Long evaluation loop of the template (no typesetting, only computing).
fn long_loop(tag: &str, rounds: u64) -> Source {
    let main = format!("// {tag}\n#let n = 0\n#for i in range({rounds}) {{ n += i }}\n= Schleife\nErgebnis #n.\n");
    Source { main, files: Vec::new() }
}

/// `compile_cancellable` in a thread of its own, cancellation after `after`; (result there?, duration from start).
fn cancel_after(source: Source, after: Duration) -> (bool, Duration) {
    let cancel = Cancel::new();
    let run = cancel.clone();
    let started = Instant::now();
    let handle = thread::spawn(move || compile_cancellable(&source, &run).is_some());
    thread::sleep(after);
    cancel.cancel();
    let some = handle.join().expect("der Lauf darf nicht abstürzen");
    (some, started.elapsed())
}

#[test]
fn a_cancel_before_the_run_returns_none_at_once() {
    let cancel = Cancel::new();
    assert!(!cancel.is_cancelled(), "ein neuer Abbruch-Zustand ist nicht abgebrochen");
    let clone = cancel.clone();
    clone.cancel();
    assert!(cancel.is_cancelled(), "Klone teilen den Zustand: Abbruch über den Klon wirkt auf das Original");
    let started = Instant::now();
    let rendered = compile_cancellable(&long_text("vorab", 400), &cancel);
    assert!(rendered.is_none(), "vor dem Lauf abgebrochen: kein Ergebnis");
    assert!(
        started.elapsed() < Duration::from_millis(500),
        "vor dem Lauf abgebrochen kehrt sofort zurück, ohne Typst zu bemühen: {:?}",
        started.elapsed()
    );
}

#[test]
fn without_cancel_the_cancellable_run_equals_the_plain_one() {
    let source = long_text("gleich", 6);
    let plain = compile(&source);
    let cancellable = compile_cancellable(&source, &Cancel::new()).expect("ohne Abbruch gibt es ein Ergebnis");
    assert_eq!(
        cancellable, plain,
        "ohne Abbruch setzt compile_cancellable genau wie compile (Seiten, Meldungen, Stand)"
    );
    assert_eq!(plain.page_lines.len(), plain.pages.len(), "je Seite eine Zeilenliste");
    assert!(plain.page_lines[0].contains(&4), "auf Seite 1 steht die Überschrift aus Zeile 4: {:?}", plain.page_lines);
}

/// Long typesetting: cancellation after 200 ms. Typst typesets to the end; only the writing of the SVG pages is
/// saved (check before every page). The result is always `None`.
#[test]
fn cancelling_a_long_layout_returns_none_and_a_later_run_is_complete() {
    let paragraphs = 500;
    let (some, cancelled) = cancel_after(long_text("abbr", paragraphs), Duration::from_millis(200));
    assert!(!some, "während des Satzes abgebrochen: kein Ergebnis, nie halbe Seiten");

    let started = Instant::now();
    let full = compile(&long_text("voll", paragraphs));
    let full_time = started.elapsed();
    assert!(!full.has_errors(), "der lange Text setzt ohne Fehler: {:?}", full.diagnostics);
    eprintln!(
        "Messung Satz ({paragraphs} Absätze, {} Seiten): abgebrochen nach 200 ms → zurück nach {} ms; ohne Abbruch {} ms",
        full.pages.len(),
        cancelled.as_millis(),
        full_time.as_millis()
    );

    // The same source as the cancelled run: intermediate results of Typst from the cancellation must spoil nothing.
    let again = compile(&long_text("abbr", paragraphs));
    assert!(!again.has_errors(), "keine Folgefehler aus dem Abbruch: {:?}", again.diagnostics);
    assert_eq!(again.page_lines.len(), again.pages.len(), "alle Seiten geschrieben, je Seite eine Zeilenliste");
    // Four header lines, then per paragraph one text line and one empty line: the last paragraph is in line 5 + 2·499.
    let last_line = 5 + 2 * (paragraphs - 1);
    assert!(
        again.page_lines.last().is_some_and(|lines| lines.contains(&last_line)),
        "auf der letzten Seite steht der letzte Absatz (Zeile {last_line}): der Satz ist vollständig"
    );
    assert_eq!(full.page_lines.len(), full.pages.len(), "auch der volle Lauf schreibt jede Seite");
}

/// Evaluation loop: Typst cannot be interrupted in it – the cancellation takes effect only when it is through.
#[test]
fn an_evaluation_loop_is_not_interrupted_but_still_yields_none() {
    let rounds = 3_000_000;
    let (some, cancelled) = cancel_after(long_loop("schleife-abbruch", rounds), Duration::from_millis(100));
    assert!(!some, "auch nach einer nicht unterbrechbaren Schleife: abgebrochen heißt kein Ergebnis");
    let started = Instant::now();
    let full = compile(&long_loop("schleife-voll", rounds));
    let full_time = started.elapsed();
    assert!(!full.has_errors(), "die Schleife setzt ohne Fehler: {:?}", full.diagnostics);
    eprintln!(
        "Messung Schleife ({rounds} Runden): abgebrochen nach 100 ms → zurück nach {} ms; ohne Abbruch {} ms",
        cancelled.as_millis(),
        full_time.as_millis()
    );
}

#[test]
fn warming_the_fonts_finds_at_least_the_embedded_ones() {
    let families = baruch_render::warm_fonts();
    assert!(
        families >= 3,
        "mindestens die eingebetteten Familien (Libertinus, New Computer Modern, DejaVu): {families}"
    );
    let started = Instant::now();
    baruch_render::warm_fonts();
    assert!(
        started.elapsed() < Duration::from_millis(200),
        "der zweite Aufruf sucht nicht erneut: {:?}",
        started.elapsed()
    );
}
