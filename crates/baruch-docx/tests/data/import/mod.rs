// SPDX-License-Identifier: AGPL-3.0-or-later
//! Freely invented Word file „Das Haus am Deich“ for the tests of the reader, built as an archive in the test (no
//! real Word involved – what Word really writes is an assumption here according to ECMA-376 and `ooxml.rs`).
#![allow(dead_code)]

use std::io::Write;

pub const NS: &str = "xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\" \
    xmlns:r=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships\" \
    xmlns:wp=\"http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing\" \
    xmlns:a=\"http://schemas.openxmlformats.org/drawingml/2006/main\" \
    xmlns:pic=\"http://schemas.openxmlformats.org/drawingml/2006/picture\" \
    xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\" \
    xmlns:mc=\"http://schemas.openxmlformats.org/markup-compatibility/2006\" \
    xmlns:v=\"urn:schemas-microsoft-com:vml\" \
    xmlns:o=\"urn:schemas-microsoft-com:office:office\" \
    xmlns:wps=\"http://schemas.microsoft.com/office/word/2010/wordprocessingShape\" \
    xmlns:w14=\"http://schemas.microsoft.com/office/word/2010/wordml\" \
    xmlns:w15=\"http://schemas.microsoft.com/office/word/2012/wordml\"";

pub const ERIKA: &str = "Erika Beispiel";
pub const JOERG: &str = "Jörg Müller";
pub const DATE: &str = "2026-11-20T09:30:00Z";

/// A valid PNG with a single pixel.
pub const PNG: &[u8] = baruch_docx::sample::SAMPLE_PNG;

/// ZIP from parts (all DEFLATE).
pub fn zip(parts: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (name, bytes) in parts {
        writer.start_file(*name, options).expect("Eintrag beginnen");
        writer.write_all(bytes).expect("Eintrag schreiben");
    }
    writer.finish().expect("ZIP abschließen").into_inner()
}

pub const CONTENT_TYPES: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
<Default Extension=\"xml\" ContentType=\"application/xml\"/><Default Extension=\"png\" ContentType=\"image/png\"/>\
<Override PartName=\"/word/document.xml\" \
ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/></Types>";

pub const ROOT_RELS: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
<Relationship Id=\"rId1\" \
Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" \
Target=\"word/document.xml\"/></Relationships>";

fn rel(id: &str, kind: &str, target: &str, extra: &str) -> String {
    format!(
        "<Relationship Id=\"{id}\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/{kind}\" \
         Target=\"{target}\"{extra}/>"
    )
}

/// Relationships of the document; `more` appends further ones.
pub fn document_rels(more: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\
         <Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">{}{}{}{}{}{}{}{more}\
         </Relationships>",
        rel("rId1", "styles", "styles.xml", ""),
        rel("rId2", "numbering", "numbering.xml", ""),
        rel("rId3", "footnotes", "footnotes.xml", ""),
        rel("rId4", "comments", "comments.xml", ""),
        rel("rId5", "image", "media/leuchtturm.png", ""),
        rel("rId6", "hyperlink", "https://example.org/deich", " TargetMode=\"External\""),
        "<Relationship Id=\"rId7\" Type=\"http://schemas.microsoft.com/office/2011/relationships/commentsExtended\" \
         Target=\"commentsExtended.xml\"/>",
    )
}

pub const STYLES: &str = "<w:styles NS>\
<w:docDefaults><w:rPrDefault><w:rPr><w:lang w:val=\"de-DE\"/></w:rPr></w:rPrDefault></w:docDefaults>\
<w:style w:type=\"paragraph\" w:styleId=\"Title\"><w:name w:val=\"Title\"/></w:style>\
<w:style w:type=\"paragraph\" w:styleId=\"Heading1\"><w:name w:val=\"heading 1\"/></w:style>\
<w:style w:type=\"paragraph\" w:styleId=\"berschrift2\"><w:name w:val=\"Überschrift 2\"/></w:style>\
<w:style w:type=\"paragraph\" w:styleId=\"Eigene3\"><w:name w:val=\"Eigene Zwischenzeile\"/>\
<w:pPr><w:outlineLvl w:val=\"2\"/></w:pPr></w:style>\
<w:style w:type=\"paragraph\" w:styleId=\"Abgeleitet\"><w:name w:val=\"Kapitel rot\"/><w:basedOn w:val=\"Heading1\"/></w:style>\
<w:style w:type=\"paragraph\" w:styleId=\"Heading4\"><w:name w:val=\"heading 4\"/></w:style>\
<w:style w:type=\"paragraph\" w:styleId=\"Quote\"><w:name w:val=\"Quote\"/></w:style>\
<w:style w:type=\"paragraph\" w:styleId=\"Caption\"><w:name w:val=\"caption\"/></w:style>\
<w:style w:type=\"paragraph\" w:styleId=\"ListParagraph\"><w:name w:val=\"List Paragraph\"/></w:style>\
</w:styles>";

pub const NUMBERING: &str = "<w:numbering NS>\
<w:abstractNum w:abstractNumId=\"10\"><w:lvl w:ilvl=\"0\"><w:start w:val=\"1\"/><w:numFmt w:val=\"bullet\"/></w:lvl>\
<w:lvl w:ilvl=\"1\"><w:numFmt w:val=\"bullet\"/></w:lvl></w:abstractNum>\
<w:abstractNum w:abstractNumId=\"11\"><w:lvl w:ilvl=\"0\"><w:start w:val=\"3\"/><w:numFmt w:val=\"decimal\"/></w:lvl>\
</w:abstractNum>\
<w:num w:numId=\"1\"><w:abstractNumId w:val=\"10\"/></w:num><w:num w:numId=\"2\"><w:abstractNumId w:val=\"11\"/></w:num>\
</w:numbering>";

pub const FOOTNOTES: &str = "<w:footnotes NS>\
<w:footnote w:type=\"separator\" w:id=\"-1\"><w:p><w:r><w:separator/></w:r></w:p></w:footnote>\
<w:footnote w:id=\"2\"><w:p><w:r><w:rPr><w:rStyle w:val=\"FootnoteReference\"/></w:rPr><w:footnoteRef/></w:r>\
<w:r><w:t xml:space=\"preserve\"> Gemessen wurde am erfundenen Pegel Süderhörn.</w:t></w:r></w:p></w:footnote>\
</w:footnotes>";

pub const COMMENTS: &str = "<w:comments NS>\
<w:comment w:id=\"0\" w:author=\"Erika Beispiel\" w:initials=\"EB\" w:date=\"2026-11-20T09:30:00Z\">\
<w:p w14:paraId=\"0A0A0A01\"><w:r><w:annotationRef/></w:r><w:r><w:t>Woher kam der Wind genau?</w:t></w:r></w:p></w:comment>\
<w:comment w:id=\"1\" w:author=\"Jörg Müller\" w:initials=\"JM\" w:date=\"2026-11-21T08:00:00Z\">\
<w:p w14:paraId=\"0A0A0A02\"><w:r><w:t>Aus Nordwest &amp; zwar stetig.</w:t></w:r></w:p></w:comment>\
<w:comment w:id=\"2\" w:author=\"Erika Beispiel\" w:initials=\"EB\" w:date=\"2026-11-20T10:00:00Z\">\
<w:p w14:paraId=\"0A0A0A03\"><w:r><w:t>Schleuse oder Siel?</w:t></w:r></w:p></w:comment>\
</w:comments>";

pub const COMMENTS_EXTENDED: &str = "<w15:commentsEx NS>\
<w15:commentEx w15:paraId=\"0A0A0A01\" w15:done=\"0\"/>\
<w15:commentEx w15:paraId=\"0A0A0A02\" w15:paraIdParent=\"0A0A0A01\" w15:done=\"0\"/>\
<w15:commentEx w15:paraId=\"0A0A0A03\" w15:done=\"0\"/></w15:commentsEx>";

pub fn p(style: &str, inner: &str) -> String {
    if style.is_empty() {
        format!("<w:p>{inner}</w:p>")
    } else {
        format!("<w:p><w:pPr><w:pStyle w:val=\"{style}\"/></w:pPr>{inner}</w:p>")
    }
}

pub fn r(text: &str) -> String {
    format!("<w:r><w:t xml:space=\"preserve\">{text}</w:t></w:r>")
}

pub fn rp(props: &str, text: &str) -> String {
    format!("<w:r><w:rPr>{props}</w:rPr><w:t xml:space=\"preserve\">{text}</w:t></w:r>")
}

pub fn ins(author: &str, text: &str) -> String {
    format!("<w:ins w:id=\"90\" w:author=\"{author}\" w:date=\"{DATE}\">{}</w:ins>", r(text))
}

pub fn del(author: &str, text: &str) -> String {
    format!(
        "<w:del w:id=\"91\" w:author=\"{author}\" w:date=\"{DATE}\"><w:r><w:delText xml:space=\"preserve\">{text}\
         </w:delText></w:r></w:del>"
    )
}

pub fn list_item(num: u32, level: u8, text: &str) -> String {
    format!(
        "<w:p><w:pPr><w:pStyle w:val=\"ListParagraph\"/><w:numPr><w:ilvl w:val=\"{level}\"/><w:numId w:val=\"{num}\"/>\
         </w:numPr></w:pPr>{}</w:p>",
        r(text)
    )
}

pub fn drawing(rid: &str, alt: &str) -> String {
    format!(
        "<w:r><w:drawing><wp:inline><wp:extent cx=\"9525\" cy=\"9525\"/><wp:docPr id=\"1\" name=\"Bild 1\" \
         descr=\"{alt}\"/><a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/picture\">\
         <pic:pic><pic:blipFill><a:blip r:embed=\"{rid}\"/></pic:blipFill></pic:pic></a:graphicData></a:graphic>\
         </wp:inline></w:drawing></w:r>"
    )
}

pub fn cell(text: &str) -> String {
    format!("<w:tc>{}</w:tc>", p("", &r(text)))
}

/// The body of the sample.
pub fn body() -> String {
    let mut b = String::new();
    b.push_str(&p("Title", &r("Das Haus am Deich")));
    b.push_str(&p("Heading1", &r("Ankunft")));
    b.push_str(&p(
        "",
        &format!(
            "<w:commentRangeStart w:id=\"0\"/><w:commentRangeStart w:id=\"1\"/>{}<w:commentRangeEnd w:id=\"0\"/>\
             <w:commentRangeEnd w:id=\"1\"/><w:r><w:commentReference w:id=\"0\"/></w:r>\
             <w:r><w:commentReference w:id=\"1\"/></w:r>{}{}{}{}{}{}<w:r><w:footnoteReference w:id=\"2\"/></w:r>{}{}",
            r("Der Wind kam"),
            r(" "),
            rp("<w:b/>", "kräftig"),
            r(" und "),
            rp("<w:i/><w:b w:val=\"0\"/>", "salzig"),
            r(" vom Meer, "),
            del(ERIKA, "leise"),
            ins(ERIKA, "laut"),
            ins(JOERG, " Abends wurde es still."),
        ),
    ));
    b.push_str(&p("berschrift2", &r("Der Deich")));
    b.push_str(&p("Quote", &r("Wer nicht deichen will, muss weichen.")));
    b.push_str(&p("BaruchZitatquelle", &r("Alter Spruch")));
    b.push_str(&list_item(1, 0, "Schafe"));
    b.push_str(&list_item(1, 1, "mit Lämmern"));
    b.push_str(&list_item(1, 0, "Möwen"));
    b.push_str(&list_item(2, 0, "Deich bauen"));
    b.push_str(&list_item(2, 0, "Deich halten"));
    b.push_str(&format!(
        "<w:tbl><w:tr><w:trPr><w:tblHeader/></w:trPr>{}{}</w:tr><w:tr>{}{}</w:tr></w:tbl>",
        cell("Pegel"),
        cell("Stand"),
        cell("Süderhörn"),
        cell("2,10 m")
    ));
    b.push_str(&p("", &drawing("rId5", "Ein Leuchtturm im Nebel")));
    b.push_str(&p("Caption", &r("Der Leuchtturm bei Nacht")));
    b.push_str(&p("Eigene3", &format!("{}{}", r("Die Schleuse"), ins(JOERG, " am Hafen"))));
    b.push_str(&p(
        "",
        &format!(
            "<w:commentRangeStart w:id=\"2\"/>{}<w:commentRangeEnd w:id=\"2\"/><w:r><w:commentReference w:id=\"2\"/>\
             </w:r>{}<w:hyperlink r:id=\"rId6\">{}</w:hyperlink>{}",
            r("Die Schleuse"),
            r(" öffnete sich nur bei Ebbe, siehe "),
            r("Deichverband"),
            r(".")
        ),
    ));
    b.push_str(&p("Heading4", &r("Kleingedrucktes")));
    b.push_str(&p("", &r("#1 kostet 5 $ – *nicht* verhandelbar.")));
    b
}

pub fn document(body: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?><w:document {NS}><w:body>{body}\
         <w:sectPr/></w:body></w:document>"
    )
}

fn ns(text: &str) -> Vec<u8> {
    text.replace(" NS>", &format!(" {NS}>")).into_bytes()
}

/// The parts of a file with this body and further relationships.
pub fn parts(body: &str, more_rels: &str) -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("[Content_Types].xml", CONTENT_TYPES.as_bytes().to_vec()),
        ("_rels/.rels", ROOT_RELS.as_bytes().to_vec()),
        ("word/document.xml", document(body).into_bytes()),
        ("word/_rels/document.xml.rels", document_rels(more_rels).into_bytes()),
        ("word/styles.xml", ns(STYLES)),
        ("word/numbering.xml", ns(NUMBERING)),
        ("word/footnotes.xml", ns(FOOTNOTES)),
        ("word/comments.xml", ns(COMMENTS)),
        ("word/commentsExtended.xml", ns(COMMENTS_EXTENDED)),
        ("word/media/leuchtturm.png", PNG.to_vec()),
    ]
}

/// The whole invented file.
pub fn sample() -> Vec<u8> {
    zip(&parts(&body(), ""))
}

/// A file with its own body (all other parts as in the sample).
pub fn with_body(body: &str) -> Vec<u8> {
    zip(&parts(body, ""))
}
