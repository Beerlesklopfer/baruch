// SPDX-License-Identifier: AGPL-3.0-or-later
//! `baruch_render::compile_pdf` (contract Round 11, section 3): the same world as `compile`, valid
//! PDF with embedded font and image, same page count, no bytes on errors. Works and
//! images freely invented; the PNG is created in the test.

use baruch_render::{Severity, Source, compile, compile_pdf};

/// Small PNG (greyscale, unpacked deflate blocks) from a function per pixel.
fn png(width: u32, height: u32) -> Vec<u8> {
    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = 0xffff_ffffu32;
        for &b in bytes {
            crc ^= u32::from(b);
            for _ in 0..8 {
                crc = if crc & 1 == 1 { (crc >> 1) ^ 0xedb8_8320 } else { crc >> 1 };
            }
        }
        !crc
    }
    fn chunk(out: &mut Vec<u8>, kind: &[u8], data: &[u8]) {
        out.extend((data.len() as u32).to_be_bytes());
        let mut body = kind.to_vec();
        body.extend(data);
        out.extend(&body);
        out.extend(crc32(&body).to_be_bytes());
    }
    let mut raw = Vec::new();
    for y in 0..height {
        raw.push(0);
        raw.extend((0..width).map(|x| ((x * 7 + y * 3) % 256) as u8));
    }
    let mut z = vec![0x78, 0x01];
    let blocks: Vec<&[u8]> = raw.chunks(65_535).collect();
    for (i, block) in blocks.iter().enumerate() {
        z.push(u8::from(i + 1 == blocks.len()));
        let len = block.len() as u16;
        z.extend(len.to_le_bytes());
        z.extend((!len).to_le_bytes());
        z.extend(*block);
    }
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in &raw {
        a = (a + u32::from(byte)) % 65_521;
        b = (b + a) % 65_521;
    }
    z.extend(((b << 16) | a).to_be_bytes());
    let mut out = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::new();
    ihdr.extend(width.to_be_bytes());
    ihdr.extend(height.to_be_bytes());
    ihdr.extend([8, 0, 0, 0, 0]);
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &z);
    chunk(&mut out, b"IEND", &[]);
    out
}

/// How often `name` occurs with the value `value` (`/Type /Page` or compact `/Type/Page`), without counting
/// longer names (`/Pages`).
fn entries(pdf: &[u8], name: &str, value: &str) -> usize {
    let mut count = 0;
    let mut at = 0;
    while let Some(found) = pdf[at..].windows(name.len()).position(|w| w == name.as_bytes()) {
        let mut i = at + found + name.len();
        while pdf.get(i).is_some_and(u8::is_ascii_whitespace) {
            i += 1;
        }
        let rest = &pdf[i..];
        if rest.starts_with(value.as_bytes()) && !rest.get(value.len()).is_some_and(u8::is_ascii_alphanumeric) {
            count += 1;
        }
        at += found + name.len();
    }
    count
}

/// Number of page objects (`/Type /Page`, not `/Pages`).
fn page_objects(pdf: &[u8]) -> usize {
    entries(pdf, "/Type", "/Page")
}

fn contains(pdf: &[u8], text: &str) -> bool {
    pdf.windows(text.len()).any(|w| w == text.as_bytes())
}

/// Invented work: three chapters with page break, one with an image.
fn book(with_image: bool) -> Source {
    let figure = if with_image { "#figure(image(\"bilder/beet.png\", width: 4cm), caption: [Das Beet])\n" } else { "" };
    let main = format!(
        "#set page(width: 12cm, height: 16cm)\n#set text(lang: \"de\")\n= Aussaat\nIm März kommen die Bohnen in die Erde.\n\
         {figure}#pagebreak()\n= Ernte\nIm August füllt sich der Korb.\n#pagebreak()\n= Ruhe\nIm Winter schläft das Beet.\n"
    );
    let files = if with_image { vec![("bilder/beet.png".to_owned(), png(64, 48))] } else { Vec::new() };
    Source { main, files }
}

#[test]
fn a_work_becomes_a_valid_pdf_with_the_same_pages_as_the_preview() {
    let source = book(true);
    let pdf = compile_pdf(&source);
    assert!(!pdf.has_errors(), "das erfundene Werk muss fehlerfrei setzen: {:?}", pdf.diagnostics);
    let bytes = pdf.bytes.as_deref().expect("ohne Fehler gibt es Bytes");
    assert!(bytes.starts_with(b"%PDF-"), "ein PDF beginnt mit dem Kopf %PDF-");
    let tail = &bytes[bytes.len().saturating_sub(32)..];
    assert!(contains(tail, "%%EOF"), "ein vollständiges PDF endet mit %%EOF: {:?}", String::from_utf8_lossy(tail));
    let preview = compile(&source);
    assert_eq!(pdf.pages, preview.pages.len(), "PDF und Satz-Ansicht haben gleich viele Seiten (dieselbe Welt)");
    assert_eq!(page_objects(bytes), pdf.pages, "die Seitenobjekte im PDF stimmen mit pages überein");
    assert_eq!(pdf.pages, 3, "drei Kapitel mit zwei Umbrüchen ergeben drei Seiten");
    assert!(contains(bytes, "/FontFile"), "die Schrift ist eingebettet (Druckerei braucht alle Schriften)");
    assert!(entries(bytes, "/Subtype", "/Image") >= 1, "das PNG steht als Bildobjekt im PDF");
    assert_eq!(pdf.stand, baruch_render::stand(&source), "der Stand ist derselbe wie für die Vorschau");
}

#[test]
fn without_image_there_is_no_image_object() {
    let pdf = compile_pdf(&book(false));
    let bytes = pdf.bytes.expect("fehlerfrei");
    assert!(entries(&bytes, "/Subtype", "/Image") == 0, "ohne Bild kein Bildobjekt – die Prüfung oben ist also echt");
}

#[test]
fn an_error_yields_no_bytes_but_the_messages() {
    let source = Source { main: "= Fehler\n#unbekannt()\n".into(), files: Vec::new() };
    let pdf = compile_pdf(&source);
    assert!(pdf.has_errors(), "ein unbekannter Aufruf ist ein Fehler");
    assert!(pdf.bytes.is_none(), "bei einem Fehler nie eine (halbe) Datei");
    assert_eq!(pdf.pages, 0, "ohne Dokument keine Seiten");
    let error = pdf.diagnostics.iter().find(|d| d.severity == Severity::Error).expect("Meldung");
    assert_eq!(error.line, Some(2), "die Meldung nennt die Zeile des Fehlers");
}

#[test]
fn a_missing_image_is_an_error_with_a_german_message() {
    let source = Source { main: "= Bild\n#image(\"bilder/fehlt.png\")\n".into(), files: Vec::new() };
    let pdf = compile_pdf(&source);
    assert!(pdf.bytes.is_none(), "fehlendes Bild → kein PDF");
    assert!(
        pdf.diagnostics.iter().any(|d| d.message.contains("fehlt im Werk")),
        "dieselbe deutsche Meldung wie in der Vorschau: {:?}",
        pdf.diagnostics
    );
}
