// SPDX-License-Identifier: AGPL-3.0-or-later
//! Texte in Stücke zerlegen (Runde 12) und Inhalts-Hashes bilden.
//!
//! Regel: Absätze (getrennt durch Leerzeilen) sind die Stücke. Kurze Absätze (unter [`MIN_CHARS`],
//! z. B. die Überschriftszeile) gehen mit dem folgenden zusammen, solange das Stück [`TARGET_CHARS`] nicht
//! überschreitet; ein Absatz über [`MAX_CHARS`] wird an Satzenden (sonst an Leerraum, notfalls hart)
//! geteilt. So wirkt eine Änderung nur auf ihr eigenes Stück und die Nachbarn kurzer Absätze; der Rest
//! behält Text und Hash und wird nicht neu eingebettet.

use sha2::{Digest, Sha256};

/// Kürzere Absätze werden mit dem folgenden zusammengelegt.
pub const MIN_CHARS: usize = 200;
/// Zusammenlegen nur bis zu dieser Länge.
pub const TARGET_CHARS: usize = 900;
/// Längere Absätze werden geteilt.
pub const MAX_CHARS: usize = 1500;

/// Ein Stück eines Textes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk {
    /// Stücknummer ab 1.
    pub number: usize,
    pub text: String,
    /// [`hash`] des Textes.
    pub hash: String,
}

/// SHA-256 als Hex (32 Zeichen, also 128 Bit – genug für Inhaltsvergleiche).
pub fn hash(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    digest.iter().take(16).map(|b| format!("{b:02x}")).collect()
}

fn chars(s: &str) -> usize {
    s.chars().count()
}

/// Absätze: durch Leerzeilen (nur Leerraum) getrennt, beschnitten, leere fallen weg.
pub fn paragraphs(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            if !current.is_empty() {
                out.push(current.join("\n").trim().to_owned());
                current.clear();
            }
        } else {
            current.push(line);
        }
    }
    if !current.is_empty() {
        out.push(current.join("\n").trim().to_owned());
    }
    out.retain(|p| !p.is_empty());
    out
}

/// Einen langen Absatz in Teile von höchstens [`MAX_CHARS`] Zeichen teilen: am letzten Satzende
/// (`.`, `!`, `?`, `…` mit folgendem Leerraum) bzw. Zeilenumbruch vor der Grenze, sonst am letzten
/// Leerraum, sonst hart.
fn split_long(paragraph: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest: Vec<char> = paragraph.chars().collect();
    while rest.len() > MAX_CHARS {
        let window = &rest[..MAX_CHARS];
        let sentence = (1..window.len())
            .rev()
            .find(|&i| {
                window[i].is_whitespace() && (matches!(window[i - 1], '.' | '!' | '?' | '…') || window[i] == '\n')
            })
            .filter(|&i| i >= MAX_CHARS / 3);
        let space = (1..window.len()).rev().find(|&i| window[i].is_whitespace()).filter(|&i| i >= MAX_CHARS / 3);
        let cut = sentence.or(space).unwrap_or(MAX_CHARS);
        let piece: String = rest[..cut].iter().collect();
        let piece = piece.trim().to_owned();
        if !piece.is_empty() {
            out.push(piece);
        }
        rest = rest[cut..].to_vec();
    }
    let last: String = rest.iter().collect();
    let last = last.trim().to_owned();
    if !last.is_empty() {
        out.push(last);
    }
    out
}

/// Text → Stücke (siehe Modulkopf). Ein leerer Text hat keine Stücke.
pub fn chunks(text: &str) -> Vec<Chunk> {
    let mut pieces: Vec<String> = Vec::new();
    let mut pending: Option<String> = None;
    for paragraph in paragraphs(text) {
        for part in split_long(&paragraph) {
            let joined = match pending.take() {
                Some(short) if chars(&short) + 2 + chars(&part) <= TARGET_CHARS => format!("{short}\n\n{part}"),
                Some(short) => {
                    pieces.push(short);
                    part
                }
                None => part,
            };
            if chars(&joined) < MIN_CHARS {
                pending = Some(joined);
            } else {
                pieces.push(joined);
            }
        }
    }
    if let Some(short) = pending {
        pieces.push(short);
    }
    pieces.into_iter().enumerate().map(|(i, text)| Chunk { number: i + 1, hash: hash(&text), text }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_paragraphs_join_the_next_and_long_ones_stay_alone() {
        let long = "Der Wärter stieg jeden Abend die Treppe hinauf. ".repeat(6);
        let text = format!("= Das Jahr am Leuchtturm\n\n{long}\n\n{long}\n\nEnde.");
        let parts = chunks(&text);
        assert_eq!(parts.len(), 3, "Überschrift mit dem ersten Absatz, zweiter allein, „Ende.“ am Schluss: {parts:?}");
        assert!(parts[0].text.starts_with("= Das Jahr am Leuchtturm\n\nDer Wärter"), "Überschrift vorn: {parts:?}");
        assert_eq!(parts[2].text, "Ende.", "kurzer letzter Absatz bleibt ein eigenes Stück");
        assert_eq!(parts.iter().map(|c| c.number).collect::<Vec<_>>(), vec![1, 2, 3], "Nummern ab 1");
        assert!(chunks("  \n\n ").is_empty(), "leerer Text hat keine Stücke");
    }

    #[test]
    fn a_change_touches_only_its_own_chunk() {
        let a = "Erster Absatz über das Senfkorn, das klein ist und groß wird. ".repeat(5);
        let b = "Zweiter Absatz über die Vögel, die in seinen Zweigen wohnen. ".repeat(5);
        let c = "Dritter Absatz über den Acker, auf dem es wächst und reift. ".repeat(5);
        let before = chunks(&format!("{a}\n\n{b}\n\n{c}"));
        let after = chunks(&format!("{a}\n\n{b} Neu.\n\n{c}"));
        assert_eq!(before.len(), 3, "drei lange Absätze, drei Stücke");
        assert_eq!(before[0].hash, after[0].hash, "erstes Stück unverändert");
        assert_ne!(before[1].hash, after[1].hash, "geändertes Stück hat einen neuen Hash");
        assert_eq!(before[2].hash, after[2].hash, "drittes Stück unverändert");
    }

    #[test]
    fn very_long_paragraphs_are_split_at_sentence_ends() {
        let sentence = "Das Licht kreist über dem Meer und zählt die Wellen. ";
        let long = sentence.repeat(80);
        let parts = chunks(&long);
        assert!(parts.len() >= 3, "über 4000 Zeichen ergeben mehrere Stücke: {}", parts.len());
        assert!(parts.iter().all(|p| p.text.chars().count() <= MAX_CHARS), "kein Stück über der Grenze");
        assert!(parts.iter().all(|p| p.text.ends_with('.')), "geteilt an Satzenden");
        let hard = "x".repeat(MAX_CHARS * 2 + 10);
        let parts = chunks(&hard);
        assert_eq!(parts.len(), 3, "ohne Leerraum hart geteilt");
        assert_eq!(parts.iter().map(|p| p.text.len()).sum::<usize>(), hard.len(), "nichts geht verloren");
    }

    #[test]
    fn hashes_are_stable_hex() {
        assert_eq!(hash("Senfkorn"), hash("Senfkorn"), "gleicher Text, gleicher Hash");
        assert_ne!(hash("Senfkorn"), hash("senfkorn"), "Groß/Klein zählt");
        assert_eq!(hash("x").len(), 32, "32 Hex-Zeichen");
        assert!(hash("x").chars().all(|c| c.is_ascii_hexdigit()), "nur Hex");
    }
}
