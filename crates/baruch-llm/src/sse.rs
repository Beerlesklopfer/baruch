// SPDX-License-Identifier: AGPL-3.0-or-later
//! Server-Sent Events, piece by piece: bytes come in arbitrary packets (a line, even a UTF-8 character can span two
//! packets), lines end with `\n` or `\r\n`. An event ends at an empty line; only `data:` lines count (several are
//! joined with `\n`), `event:`, `id:`, `retry:` and comments (`:`) are skipped. `data: [DONE]` is the end of the
//! stream.

use crate::LlmError;

/// An event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// Content of the `data:` lines.
    Data(String),
    /// `data: [DONE]`.
    Done,
}

/// Splits a byte stream into events.
#[derive(Debug)]
pub struct Parser {
    pending: Vec<u8>,
    data: Vec<String>,
    /// Bytes of the collected `data:` lines of the open event (Round 23).
    data_bytes: usize,
    max_line: usize,
}

impl Parser {
    /// `max_line`: longest line in bytes (protection against a stream without a line end). Since Round 23 the same
    /// limit applies to the sum of the `data:` lines of an event (protection against a stream without an empty
    /// line: many short lines would otherwise let the memory grow until the timeout).
    pub fn new(max_line: usize) -> Parser {
        Parser { pending: Vec::new(), data: Vec::new(), data_bytes: 0, max_line }
    }

    /// Append bytes; yields the events finished by that.
    pub fn push(&mut self, bytes: &[u8]) -> Result<Vec<Event>, LlmError> {
        self.pending.extend_from_slice(bytes);
        let mut events = Vec::new();
        while let Some(end) = self.pending.iter().position(|&b| b == b'\n') {
            let mut line: Vec<u8> = self.pending.drain(..=end).collect();
            line.pop();
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            let line =
                String::from_utf8(line).map_err(|_| LlmError::Failed("Antwortstrom ist kein UTF-8-Text.".into()))?;
            self.line(&line, &mut events);
            if self.data_bytes > self.max_line {
                return Err(LlmError::Failed("Ein Ereignis im Antwortstrom ist zu groß; abgebrochen.".into()));
            }
        }
        if self.pending.len() > self.max_line {
            return Err(LlmError::Failed("Eine Zeile im Antwortstrom ist zu lang; abgebrochen.".into()));
        }
        Ok(events)
    }

    /// End of the stream: a last line without a line end and an event without an empty line still count.
    pub fn finish(&mut self) -> Result<Vec<Event>, LlmError> {
        let mut events = Vec::new();
        if !self.pending.is_empty() {
            let mut rest = std::mem::take(&mut self.pending);
            if rest.last() == Some(&b'\r') {
                rest.pop();
            }
            let line =
                String::from_utf8(rest).map_err(|_| LlmError::Failed("Antwortstrom ist kein UTF-8-Text.".into()))?;
            self.line(&line, &mut events);
        }
        self.line("", &mut events);
        Ok(events)
    }

    fn line(&mut self, line: &str, events: &mut Vec<Event>) {
        if line.is_empty() {
            if !self.data.is_empty() {
                self.data_bytes = 0;
                let data = std::mem::take(&mut self.data).join("\n");
                events.push(if data.trim() == "[DONE]" { Event::Done } else { Event::Data(data) });
            }
        } else if let Some(rest) = line.strip_prefix("data:") {
            let rest = rest.strip_prefix(' ').unwrap_or(rest);
            // +1 for the `\n` the lines are joined with.
            self.data_bytes += rest.len() + 1;
            self.data.push(rest.to_owned());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_survive_any_split_of_the_bytes() {
        let stream = "data: {\"a\":\"Grüße\"}\r\n\r\n: Kommentar\nevent: x\ndata: eins\ndata: zwei\n\ndata: [DONE]\n\n";
        let bytes = stream.as_bytes();
        for size in 1..bytes.len() {
            let mut parser = Parser::new(1024);
            let mut events = Vec::new();
            for piece in bytes.chunks(size) {
                events.extend(parser.push(piece).unwrap());
            }
            events.extend(parser.finish().unwrap());
            assert_eq!(
                events,
                [Event::Data("{\"a\":\"Grüße\"}".into()), Event::Data("eins\nzwei".into()), Event::Done],
                "Pakete zu {size} Bytes (auch mitten im ü) ergeben dieselben Ereignisse"
            );
        }
    }

    #[test]
    fn a_last_event_without_blank_line_still_counts_and_long_lines_stop() {
        let mut parser = Parser::new(8);
        assert!(parser.push(b"data: x").unwrap().is_empty(), "noch nicht fertig");
        assert_eq!(parser.finish().unwrap(), [Event::Data("x".into())], "Ende des Stroms schließt ab");
        let mut parser = Parser::new(8);
        assert!(parser.push(b"data: 0123456789").is_err(), "Zeile ohne Ende über der Grenze bricht ab");
    }

    #[test]
    fn many_short_data_lines_without_a_blank_line_stop_at_the_limit() {
        // Code review 2026-11-24 (ki-3): every line is short, but without an empty line the event never ends.
        let mut parser = Parser::new(64);
        let mut outcome = Ok(Vec::new());
        let mut pushed = 0;
        for _ in 0..100 {
            outcome = parser.push(b"data: 01234567\n");
            pushed += 1;
            if outcome.is_err() {
                break;
            }
        }
        let error = outcome.expect_err("ohne Summengrenze wüchse der Speicher bis zum Zeitlimit");
        assert!(error.to_string().contains("zu groß"), "deutsche Meldung nennt den Grund: {error}");
        assert!(
            pushed <= 8,
            "abgebrochen, sobald die Summe die Grenze von 64 Byte überschreitet (nach {pushed} Zeilen)"
        );
        // Counter-check: with empty lines in between the same volume is fine (the limit applies per event).
        let mut parser = Parser::new(64);
        let mut events = 0;
        for _ in 0..100 {
            events += parser.push(b"data: 01234567\n\n").expect("einzelne kleine Ereignisse").len();
        }
        assert_eq!(events, 100, "abgeschlossene Ereignisse zählen nicht zusammen");
    }
}
