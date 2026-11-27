// SPDX-License-Identifier: AGPL-3.0-or-later
//! Scrubbing secrets from messages (Round 23): the **one** place for all ways (model backends, „Verbindung prüfen“,
//! MultiGPT, pgvector, sync).
//!
//! [`scrub`] replaces by [`MASK`]:
//! - every literal occurrence of the secret;
//! - from a length of [`ENCODED_MIN`] bytes on additionally
//!   - the percent encoding: every byte of the secret may stand literally or as `%XX` (upper or lower case hex
//!     digits), a space also as `+` (form notation) – this way one rule covers all encoders, no matter which
//!     characters they leave as they are;
//!   - Base64 in the standard and in the URL-safe alphabet, with or without `=`, also as part of a longer Base64
//!     (`Basic base64(user:geheim)`): searched are the characters that depend on the secret alone, in all three
//!     offsets; the edge characters, into which bits of the secret and of the neighbour are mixed, and the padding
//!     characters go along.
//!
//! Shorter secrets are only replaced literally: their encoded forms are so short that they would occur by chance in
//! ordinary text. Not recognised are other encodings (hex, JSON escapes, double encoding, wrapped Base64).

use base64::Engine;
use base64::engine::general_purpose::STANDARD_NO_PAD;

/// Replacement for a secret.
pub const MASK: &str = "***";
/// From this length (bytes) on encoded forms are scrubbed as well.
pub const ENCODED_MIN: usize = 6;

/// Replace every occurrence of `secret` by `***` – literally and, from [`ENCODED_MIN`] bytes on, also
/// percent-encoded and as Base64 (see module description).
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

/// End of the secret if it stands at `at`: per byte `%XX`, literally or (space) `+`. Only hits with at least one
/// encoded byte (the literal case has been replaced before).
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
    // Without `%` and without `+` there is nothing encoded.
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
    // A hit starts and ends at character boundaries: `%XX` and `+` are ASCII, and a literally hit multi-byte
    // character always stands there completely in valid UTF-8. Without a panic, to be safe.
    String::from_utf8(out).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
}

fn is_base64(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'+' | b'/' | b'-' | b'_')
}

/// Core (characters determined by the secret alone) per offset, plus: mixed character before it? behind it?
fn base64_cores(secret: &[u8]) -> Vec<(String, bool, bool)> {
    let mut cores = Vec::new();
    for shift in 0..3usize {
        let mut padded = vec![0u8; shift];
        padded.extend_from_slice(secret);
        let encoded = STANDARD_NO_PAD.encode(&padded);
        // Offset 1: character 0 belongs to the predecessor, character 1 is mixed; offset 2: characters 0 and 1
        // belong to the predecessor, character 2 is mixed.
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

    // Invented secret; the Base64 forms come from `printf … | base64`, not from the code under test.
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
        // „k?>~ü ß/+9“ yields „az8+fsO8IMOfLys5“ in the standard alphabet (with „+“), URL-safe with „-“.
        let secret = "k?>~ü ß/+9";
        assert_eq!(scrub("t=az8+fsO8IMOfLys5;", Some(secret)), "t=***;", "Standard-Alphabet mit +");
        assert_eq!(scrub("t=az8-fsO8IMOfLys5;", Some(secret)), "t=***;", "URL-sicheres Alphabet mit -");
        // base64("x:k?>~ü ß/+9") = „eDprPz5+w7wgw58vKzk=“, here URL-safe and without padding characters.
        let out = scrub("Basic eDprPz5-w7wgw58vKzk", Some(secret));
        assert_eq!(out, "Basic eD***", "URL-sicher, versetzt und ohne =: nur der Anteil des Benutzers bleibt");
    }

    #[test]
    fn base64_inside_a_longer_base64_is_masked_at_every_offset() {
        // Basic login: base64("<user>:<secret><rest>") – the user name shifts the secret by 0, 1 or 2 bytes in the
        // grid of three. Expected: prefix of the user, then ***, afterwards at most characters that belong to the
        // rest alone.
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
        // „abcde“ (5 bytes) → Base64 „YWJjZGU=“, percent form „%61bcde“.
        let out = scrub("abcde YWJjZGU= %61bcde", Some("abcde"));
        assert_eq!(
            out, "*** YWJjZGU= %61bcde",
            "unter 6 Byte nur wörtlich – kodierte Formen wären zu kurz für eine sichere Suche"
        );
        assert_eq!(ENCODED_MIN, 6, "die Grenze steht im Vertrag der Runde 23");
        // Exactly 6 bytes: „abcdef“ → „YWJjZGVm“.
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
