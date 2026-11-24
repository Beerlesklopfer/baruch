// SPDX-License-Identifier: AGPL-3.0-or-later
//! Geheimnisse aus Meldungen schwärzen (Runde 23): die **eine** Stelle für alle Wege (Modell-Backends,
//! „Verbindung prüfen“, MultiGPT, pgvector, Abgleich).
//!
//! [`scrub`] ersetzt durch [`MASK`]:
//! - jedes wörtliche Vorkommen des Geheimnisses;
//! - ab [`ENCODED_MIN`] Byte Länge zusätzlich
//!   - die Prozent-Kodierung: jedes Byte des Geheimnisses darf wörtlich oder als `%XX` (große oder kleine
//!     Hex-Ziffern) stehen, ein Leerzeichen auch als `+` (Formular-Schreibweise) – so deckt eine Regel alle
//!     Kodierer ab, gleich welche Zeichen sie stehen lassen;
//!   - Base64 im Standard- und im URL-sicheren Alphabet, mit oder ohne `=`, auch als Teil eines längeren
//!     Base64 (`Basic base64(user:geheim)`): gesucht werden die Zeichen, die allein vom Geheimnis abhängen,
//!     in allen drei Versätzen; die Randzeichen, in die Bits des Geheimnisses und des Nachbarn gemischt
//!     sind, und die Füllzeichen gehen mit.
//!
//! Kürzere Geheimnisse werden nur wörtlich ersetzt: Ihre kodierten Formen sind so kurz, dass sie in
//! gewöhnlichem Text zufällig vorkämen. Nicht erkannt werden andere Kodierungen (Hex, JSON-Escapes,
//! doppelte Kodierung, umbrochenes Base64).

use base64::Engine;
use base64::engine::general_purpose::STANDARD_NO_PAD;

/// Ersatz für ein Geheimnis.
pub const MASK: &str = "***";
/// Ab dieser Länge (Bytes) werden auch kodierte Formen geschwärzt.
pub const ENCODED_MIN: usize = 6;

/// Jedes Vorkommen von `secret` durch `***` ersetzen – wörtlich und, ab [`ENCODED_MIN`] Byte, auch
/// Prozent-kodiert und als Base64 (siehe Modulbeschreibung).
pub fn scrub(text: &str, secret: Option<&str>) -> String {
    let Some(secret) = secret.filter(|s| !s.is_empty()) else { return text.to_owned() };
    let literal = text.replace(secret, MASK);
    if secret.len() < ENCODED_MIN {
        return literal;
    }
    let text = scrub_percent(&literal, secret.as_bytes());
    scrub_base64(&text, secret.as_bytes())
}

fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Ende des Geheimnisses, wenn es bei `at` steht: je Byte `%XX`, wörtlich oder (Leerzeichen) `+`.
/// Nur Treffer mit mindestens einem kodierten Byte (der wörtliche Fall ist vorher ersetzt).
fn percent_match(text: &[u8], at: usize, secret: &[u8]) -> Option<usize> {
    let mut j = at;
    let mut encoded = false;
    for &b in secret {
        let here = *text.get(j)?;
        let escaped = if here == b'%' {
            match (text.get(j + 1).copied().and_then(hex), text.get(j + 2).copied().and_then(hex)) {
                (Some(hi), Some(lo)) => Some(hi * 16 + lo),
                _ => None,
            }
        } else {
            None
        };
        if escaped == Some(b) {
            j += 3;
            encoded = true;
        } else if here == b {
            j += 1;
        } else if b == b' ' && here == b'+' {
            j += 1;
            encoded = true;
        } else {
            return None;
        }
    }
    encoded.then_some(j)
}

fn scrub_percent(text: &str, secret: &[u8]) -> String {
    let bytes = text.as_bytes();
    // Ohne `%` und ohne `+` gibt es nichts Kodiertes.
    if !bytes.iter().any(|&b| b == b'%' || b == b'+') {
        return text.to_owned();
    }
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match percent_match(bytes, i, secret) {
            Some(end) => {
                out.extend_from_slice(MASK.as_bytes());
                i = end;
            }
            None => {
                out.push(bytes[i]);
                i += 1;
            }
        }
    }
    // Ein Treffer beginnt und endet an Zeichengrenzen: `%XX` und `+` sind ASCII, und ein wörtlich
    // getroffenes Mehrbyte-Zeichen steht in gültigem UTF-8 immer ganz da. Sicherheitshalber ohne Panik.
    String::from_utf8(out).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
}

fn is_base64(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'-' | b'_')
}

/// Kern (nur vom Geheimnis bestimmte Zeichen) je Versatz, dazu: gemischtes Zeichen davor? dahinter?
fn base64_cores(secret: &[u8]) -> Vec<(String, bool, bool)> {
    let mut cores = Vec::new();
    for shift in 0..3usize {
        let mut padded = vec![0u8; shift];
        padded.extend_from_slice(secret);
        let encoded = STANDARD_NO_PAD.encode(&padded);
        // Versatz 1: Zeichen 0 gehört dem Vorgänger, Zeichen 1 ist gemischt; Versatz 2: Zeichen 0 und 1
        // gehören dem Vorgänger, Zeichen 2 ist gemischt.
        let skip = [0, 2, 3][shift];
        let bits = padded.len() * 8;
        let full = bits / 6;
        if full <= skip {
            continue;
        }
        let core = &encoded[skip..full];
        let (before, after) = (shift != 0, bits % 6 != 0);
        cores.push((core.to_owned(), before, after));
        let url_safe = core.replace('+', "-").replace('/', "_");
        if url_safe != core {
            cores.push((url_safe, before, after));
        }
    }
    cores
}

fn scrub_base64(text: &str, secret: &[u8]) -> String {
    let mut text = text.to_owned();
    for (core, before, after) in base64_cores(secret) {
        if !text.contains(&core) {
            continue;
        }
        let bytes = text.as_bytes();
        let mut out = String::with_capacity(text.len());
        let mut done = 0;
        while let Some(found) = text[done..].find(&core) {
            let mut start = done + found;
            let mut end = start + core.len();
            if before && start > done && is_base64(bytes[start - 1]) {
                start -= 1;
            }
            if after && end < bytes.len() && is_base64(bytes[end]) {
                end += 1;
            }
            let mut padding = 0;
            while padding < 2 && end < bytes.len() && bytes[end] == b'=' {
                end += 1;
                padding += 1;
            }
            out.push_str(&text[done..start]);
            out.push_str(MASK);
            done = end;
        }
        out.push_str(&text[done..]);
        text = out;
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    // Erfundenes Geheimnis; die Base64-Formen stammen aus `printf … | base64`, nicht aus dem geprüften Code.
    const SECRET: &str = "Zinnober-4711";

    fn clean(text: &str) -> String {
        scrub(text, Some(SECRET))
    }

    #[test]
    fn the_literal_secret_is_masked_everywhere() {
        let out = clean("Bearer Zinnober-4711 abgelehnt; Schlüssel Zinnober-4711 prüfen");
        assert_eq!(out, "Bearer *** abgelehnt; Schlüssel *** prüfen", "jedes wörtliche Vorkommen wird ersetzt");
    }

    #[test]
    fn base64_of_the_secret_is_masked_with_and_without_padding() {
        assert_eq!(clean("key=Wmlubm9iZXItNDcxMQ== abgelehnt"), "key=*** abgelehnt", "Standard-Base64 mit ==");
        assert_eq!(clean("key=Wmlubm9iZXItNDcxMQ abgelehnt"), "key=*** abgelehnt", "Base64 ohne Füllzeichen");
        assert_eq!(clean("Wmlubm9iZXItNDcxMQ="), "***", "auch mit nur einem = bleibt kein Rest");
    }

    #[test]
    fn url_safe_base64_is_masked_too() {
        // „k?>~ü ß/+9“ ergibt im Standard-Alphabet „az8+fsO8IMOfLys5“ (mit „+“), URL-sicher mit „-“.
        let secret = "k?>~ü ß/+9";
        assert_eq!(scrub("t=az8+fsO8IMOfLys5;", Some(secret)), "t=***;", "Standard-Alphabet mit +");
        assert_eq!(scrub("t=az8-fsO8IMOfLys5;", Some(secret)), "t=***;", "URL-sicheres Alphabet mit -");
        // base64("x:k?>~ü ß/+9") = „eDprPz5+w7wgw58vKzk=“, hier URL-sicher und ohne Füllzeichen.
        let out = scrub("Basic eDprPz5-w7wgw58vKzk", Some(secret));
        assert_eq!(out, "Basic eD***", "URL-sicher, versetzt und ohne =: nur der Anteil des Benutzers bleibt");
    }

    #[test]
    fn base64_inside_a_longer_base64_is_masked_at_every_offset() {
        // Basic-Anmeldung: base64("<benutzer>:<geheimnis><rest>") – der Benutzername verschiebt das
        // Geheimnis um 0, 1 oder 2 Byte im Dreierraster. Erwartet: Vorspann des Benutzers, dann ***,
        // danach höchstens Zeichen, die allein dem Rest gehören.
        for (before, encoded, expected) in [
            ("leser:", "bGVzZXI6Wmlubm9iZXItNDcxMQ==", "bGVzZXI6***"),
            ("ab:", "YWI6Wmlubm9iZXItNDcxMQ==", "YWI6***"),
            ("a:", "YTpaaW5ub2Jlci00NzEx", "YT***"),
            ("abc: (und x dahinter)", "YWJjOlppbm5vYmVyLTQ3MTF4", "YWJjO***4"),
        ] {
            let out = clean(&format!("Authorization: Basic {encoded} abgelehnt"));
            assert_eq!(
                out,
                format!("Authorization: Basic {expected} abgelehnt"),
                "Vorspann „{before}“: alle Zeichen mit Bits des Geheimnisses sind ersetzt"
            );
        }
    }

    #[test]
    fn percent_encoding_is_masked_in_upper_and_lower_case() {
        assert_eq!(clean("?key=Zinnober%2D4711&x=1"), "?key=***&x=1", "ein kodiertes Zeichen, große Hex-Ziffern");
        assert_eq!(clean("?key=Zinnober%2d4711&x=1"), "?key=***&x=1", "kleine Hex-Ziffern");
        let all_upper: String = SECRET.bytes().map(|b| format!("%{b:02X}")).collect();
        let all_lower: String = SECRET.bytes().map(|b| format!("%{b:02x}")).collect();
        assert_eq!(clean(&format!("a {all_upper} b")), "a *** b", "jedes Byte kodiert, groß");
        assert_eq!(clean(&format!("a {all_lower} b")), "a *** b", "jedes Byte kodiert, klein");
    }

    #[test]
    fn form_encoding_and_non_ascii_bytes_are_masked() {
        let secret = "Grüße an Tür 7";
        assert_eq!(scrub("p=Gr%C3%BC%C3%9Fe+an+T%c3%bcr+7&q", Some(secret)), "p=***&q", "Formular: + für Leerzeichen");
        assert_eq!(scrub("p=Gr%C3%BC%C3%9Fe%20an%20T%C3%BCr%207", Some(secret)), "p=***", "%20 für Leerzeichen");
        assert_eq!(scrub("„Grüße an Tür 7“ – größer", Some(secret)), "„***“ – größer", "wörtlich mit Umlauten");
    }

    #[test]
    fn short_secrets_are_only_masked_literally() {
        // „abcde“ (5 Byte) → Base64 „YWJjZGU=“, Prozentform „%61bcde“.
        let out = scrub("abcde YWJjZGU= %61bcde", Some("abcde"));
        assert_eq!(
            out, "*** YWJjZGU= %61bcde",
            "unter 6 Byte nur wörtlich – kodierte Formen wären zu kurz für eine sichere Suche"
        );
        assert_eq!(ENCODED_MIN, 6, "die Grenze steht im Vertrag der Runde 23");
        // Genau 6 Byte: „abcdef“ → „YWJjZGVm“.
        assert_eq!(scrub("YWJjZGVm %61bcdef", Some("abcdef")), "*** ***", "ab 6 Byte auch kodiert");
    }

    #[test]
    fn ordinary_text_stays_untouched() {
        let text =
            "Die Prüfung von 100 % der Fälle (A+B) ergab: QmFzZTY0IGlzdCBoaWVyIG51ciBCZWlzcGllbA== – größer als 50%2.";
        assert_eq!(clean(text), text, "Text ohne Geheimnis bleibt byte-gleich, auch mit %, + und fremdem Base64");
        assert_eq!(scrub(text, None), text, "ohne Geheimnis nichts zu tun");
        assert_eq!(scrub(text, Some("")), text, "ein leeres Geheimnis ersetzt nichts");
    }

    #[test]
    fn a_secret_with_a_percent_sign_is_masked_in_both_forms() {
        let secret = "100%-Bio";
        assert_eq!(scrub("x 100%-Bio y", Some(secret)), "x *** y", "wörtlich");
        assert_eq!(scrub("x 100%25-Bio y", Some(secret)), "x *** y", "das Prozentzeichen selbst kodiert");
    }
}
