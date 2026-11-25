// SPDX-License-Identifier: AGPL-3.0-or-later
//! Zeichnung der Mindmap als SVG (Vertrag Runde 34, Schnittstelle R34-S1 §S1.3; danach Besitz mindmap_widget).
//!
//! Eine Anordnung, in Rust: Bildschirm (`standalone: false`, Fragment für die Seite) und Ausgabe
//! (`standalone: true`, eigenständige Datei für `.svg`/`.pdf`) entstehen aus derselben Funktion [`render`].
//!
//! Stand S1: gültiges, schlichtes SVG – je Knoten aus [`MindMap::walk`] eine eingerückte Textzeile. Anordnung,
//! Äste, Beziehungen und der DOM-Vertrag (Vertrag §6) kommen von mindmap_widget.

use std::fmt::Write as _;

use crate::idea_map::tree::MindMap;

#[derive(Debug, Clone, Default)]
pub struct RenderOptions {
    /// true = eigenständige Datei (eigener Stil, xmlns), false = Fragment für die Seite.
    pub standalone: bool,
    /// Kennungen eingeklappter Knoten ("n:<key>", "i:<id>").
    pub closed: Vec<String>,
    /// Abfrageteil des Filters für Links im Bild (ohne führendes "?").
    pub back: String,
}

#[derive(Debug, Clone)]
pub struct Rendered {
    pub svg: String,
    pub width: f64,
    pub height: f64,
}

/// Text für XML-Inhalt und -Attribute maskiert; Steuerzeichen (in XML 1.0 unzulässig) fallen weg.
fn esc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            '\n' | '\t' => out.push(' '),
            c if c.is_control() => {}
            c => out.push(c),
        }
    }
    out
}

pub fn render(map: &MindMap, options: &RenderOptions) -> Rendered {
    // STUB: Anordnung und Zeichnung der Mindmap fehlen (R34 mindmap_widget)
    const LINE: f64 = 22.0;
    const INDENT: f64 = 24.0;
    const MARGIN: f64 = 16.0;
    const CHAR: f64 = 8.0;
    let rows = map.walk();
    let width = rows
        .iter()
        .map(|(node, depth)| MARGIN * 2.0 + INDENT * *depth as f64 + CHAR * node.title.chars().count() as f64)
        .fold(200.0, f64::max);
    let height = MARGIN * 2.0 + LINE * rows.len() as f64;
    let mut svg = String::new();
    let xmlns = if options.standalone { " xmlns=\"http://www.w3.org/2000/svg\"" } else { "" };
    let _ = write!(
        svg,
        "<svg{xmlns} class=\"mm-svg\" viewBox=\"0 0 {width} {height}\" width=\"{width}\" height=\"{height}\" \
         role=\"img\" aria-label=\"{}\">",
        esc(&format!("Mindmap: {}", map.title))
    );
    if options.standalone {
        svg.push_str("<style>text{font-family:sans-serif;font-size:14px;fill:#222}</style>");
    }
    svg.push_str("<g class=\"mm-viewport\">");
    for (row, (node, depth)) in rows.iter().enumerate() {
        let x = MARGIN + INDENT * *depth as f64;
        let y = MARGIN + LINE * (row as f64 + 0.75);
        let _ = write!(svg, "<text x=\"{x}\" y=\"{y}\" data-node=\"{}\">{}</text>", esc(&node.id), esc(&node.title));
    }
    svg.push_str("</g></svg>");
    Rendered { svg, width, height }
}
