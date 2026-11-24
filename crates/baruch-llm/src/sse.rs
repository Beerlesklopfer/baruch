// SPDX-License-Identifier: AGPL-3.0-or-later
//! Server-Sent Events, Stück für Stück: Bytes kommen in beliebigen Paketen (eine Zeile, ja ein
//! UTF-8-Zeichen kann über zwei Pakete gehen), Zeilen enden mit `\n` oder `\r\n`. Ein Ereignis endet
//! an einer Leerzeile; nur `data:`-Zeilen zählen (mehrere werden mit `\n` verbunden), `event:`, `id:`,
//! `retry:` und Kommentare (`:`) werden übergangen. `data: [DONE]` ist das Ende des Stroms.

use crate::LlmError;

/// Ein Ereignis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// Inhalt der `data:`-Zeilen.
    Data(String),
    /// `data: [DONE]`.
    Done,
}

/// Zerlegt einen Byte-Strom in Ereignisse.
#[derive(Debug)]
pub struct Parser {
    pending: Vec<u8>,
    data: Vec<String>,
    /// Bytes der gesammelten `data:`-Zeilen des offenen Ereignisses (Runde 23).
    data_bytes: usize,
    max_line: usize,
}

impl Parser {
    /// `max_line`: längste Zeile in Bytes (Schutz vor einem Strom ohne Zeilenende). Dieselbe Grenze gilt seit
    /// Runde 23 für die Summe der `data:`-Zeilen eines Ereignisses (Schutz vor einem Strom ohne Leerzeile:
    /// Viele kurze Zeilen ließen den Speicher sonst bis zum Zeitlimit wachsen).
    pub fn new(max_line: usize) -> Parser {
        Parser { pending: Vec::new(), data: Vec::new(), data_bytes: 0, max_line }
    }

    /// Bytes anhängen; liefert die damit fertigen Ereignisse.
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

    /// Ende des Stroms: eine letzte Zeile ohne Zeilenende und ein Ereignis ohne Leerzeile gelten noch.
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
            // +1 für das `\n`, mit dem die Zeilen verbunden werden.
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
        // Codereview 2026-11-24 (ki-3): Jede Zeile ist kurz, aber ohne Leerzeile endet das Ereignis nie.
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
        // Gegenprobe: Mit Leerzeilen dazwischen ist derselbe Umfang in Ordnung (die Grenze gilt je Ereignis).
        let mut parser = Parser::new(64);
        let mut events = 0;
        for _ in 0..100 {
            events += parser.push(b"data: 01234567\n\n").expect("einzelne kleine Ereignisse").len();
        }
        assert_eq!(events, 100, "abgeschlossene Ereignisse zählen nicht zusammen");
    }
}
