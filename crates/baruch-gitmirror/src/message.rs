// SPDX-License-Identifier: AGPL-3.0-or-later
//! Commit messages: subject, body and trailers.

/// Builds the commit message.
///
/// Structure: `summary`, empty line, `body` (if not empty), empty line, one line `Key: Value` per trailer. If body
/// and trailers are missing, only the subject without empty lines remains. Line breaks in the subject and in the
/// trailers become spaces, so that the structure is kept; the body keeps its lines (CRLF → LF), empty lines at the
/// edges are dropped. The message ends without a line break.
pub fn format_message(summary: &str, body: &str, trailers: &[(String, String)]) -> String {
    let mut message = one_line(summary);
    let body = body.replace("\r\n", "\n");
    let body = body.trim_matches(|c: char| c == '\n' || c == '\r' || c == ' ' || c == '\t');
    if !body.is_empty() {
        message.push_str("\n\n");
        message.push_str(body);
    }
    if !trailers.is_empty() {
        message.push_str("\n\n");
        let lines: Vec<String> =
            trailers.iter().map(|(key, value)| format!("{}: {}", one_line(key), one_line(value))).collect();
        message.push_str(&lines.join("\n"));
    }
    message
}

fn one_line(s: &str) -> String {
    s.split(['\r', '\n']).map(str::trim).filter(|part| !part.is_empty()).collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_summary() {
        assert_eq!(format_message("Betreff", "", &[]), "Betreff");
        assert_eq!(format_message("Betreff", "  \n", &[]), "Betreff");
    }

    #[test]
    fn full() {
        let trailers =
            vec![("Persona".to_string(), "@lektor".to_string()), ("Model".to_string(), "qwen3:4b".to_string())];
        assert_eq!(
            format_message("2.1 Titel: Vorschlag 3 übernommen", "Rumpf\r\nZeile 2", &trailers),
            "2.1 Titel: Vorschlag 3 übernommen\n\nRumpf\nZeile 2\n\nPersona: @lektor\nModel: qwen3:4b"
        );
    }

    #[test]
    fn trailers_without_body() {
        let trailers = vec![("Baruch-Node".to_string(), "4".to_string())];
        assert_eq!(format_message("S", "", &trailers), "S\n\nBaruch-Node: 4");
    }

    #[test]
    fn newlines_in_summary_flattened() {
        assert_eq!(format_message("a\nb", "", &[]), "a b");
    }
}
