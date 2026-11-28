== imprint lines
Bibliografische Information der Deutschen Nationalbibliothek
Die Deutsche Nationalbibliothek verzeichnet diese Publikation in der Deutschen Nationalbibliografie; detaillierte bibliografische Daten sind im Internet über https://dnb.de abrufbar.
© 2026 Erika Beispiel, Beispielverlag, Musterstadt
Weg 1, 12345 Musterstadt
1. Auflage
ISBN 978-3-16-148410-0
Bibelzitate: Erfundene Übersetzung. Mit freundlicher Genehmigung.
Umschlaggestaltung und Bildnachweis: Zeichnungen: Erika Beispiel
Gesetzt mit Baruch
Druck: Druckerei Muster, Druckweg 2, 12345 Musterstadt
== imprint lines (bare)
Bibliografische Information der Deutschen Nationalbibliothek
Die Deutsche Nationalbibliothek verzeichnet diese Publikation in der Deutschen Nationalbibliografie; detaillierte bibliografische Daten sind im Internet über https://dnb.de abrufbar.
©
Gesetzt mit Baruch
== frame front
// Baruch: Umschlag und Titelei (Runde 11)
// Umschlag, Vorderseite
#page(header: none, footer: none, numbering: none, margin: 0pt, {
  place(top + left, image(".baruch/umschlag.png", alt: "Weide", width: 100%, height: 100%, fit: "cover"))
  place(top + center, block(width: 100%, inset: (x: 10%, y: 9%), fill: gradient.linear(black.transparentize(30%), black.transparentize(100%), angle: 90deg), {
      set align(center)
      set par(leading: 0.5em)
      text(size: 13pt, fill: white, "Erika Beispiel")
      v(0.8em)
      text(size: 30pt, weight: "bold", fill: white, "Das Gartenhaus am Bach")
      v(0.5em)
      text(size: 15pt, fill: white, "Ein Bautagebuch")
  }))
})
// Umschlag, Innenseite (leer)
#page(header: none, footer: none, numbering: none, {
  counter(page).update(0)
  hide[ ]
})
// Titelei
#page(header: none, footer: none, numbering: none, {
  set align(center)
  v(22%)
  text(size: 16pt, "Das Gartenhaus am Bach")
})
#page(header: none, footer: none, numbering: none, {
  set align(center)
  v(22%)
  text(size: 11pt, style: "italic", "Bauen am Wasser")
})
#page(header: none, footer: none, numbering: none, {
  set align(center)
  v(12%)
  text(size: 13pt, "Erika Beispiel")
  v(2em)
  text(size: 26pt, weight: "bold", "Das Gartenhaus am Bach")
  v(1em)
  text(size: 14pt, "Ein Bautagebuch")
  v(1fr)
  text(size: 11pt, "Beispielverlag")
  linebreak()
  text(size: 10pt, "Musterstadt")
})
#page(header: none, footer: none, numbering: none, {
  set text(size: 8.5pt)
  set par(justify: false)
  v(1fr)
  block(spacing: 0.8em, text("Bibliografische Information der Deutschen Nationalbibliothek", weight: "bold"))
  block(spacing: 0.8em, text("Die Deutsche Nationalbibliothek verzeichnet diese Publikation in der Deutschen Nationalbibliografie; detaillierte bibliografische Daten sind im Internet über https://dnb.de abrufbar."))
  block(spacing: 0.8em, text("© 2026 Erika Beispiel, Beispielverlag, Musterstadt"))
  block(spacing: 0.8em, text("Weg 1, 12345 Musterstadt"))
  block(spacing: 0.8em, text("1. Auflage"))
  block(spacing: 0.8em, text("ISBN 978-3-16-148410-0"))
  block(spacing: 0.8em, text("Bibelzitate: Erfundene Übersetzung. Mit freundlicher Genehmigung."))
  block(spacing: 0.8em, text("Umschlaggestaltung und Bildnachweis: Zeichnungen: Erika Beispiel"))
  block(spacing: 0.8em, text("Gesetzt mit Baruch"))
  block(spacing: 0.8em, text("Druck: Druckerei Muster, Druckweg 2, 12345 Musterstadt"))
})
#page(header: none, footer: none, numbering: none, {
  set align(center)
  v(25%)
  set text(style: "italic")
  par("Für alle, die am Bach bauen.")
})
#page(header: none, footer: none, numbering: none, {
  hide[ ]
})

== frame back
// Baruch: Umschlag, Rückseite (Runde 11)
#page(header: none, footer: none, numbering: none, margin: 0pt, fill: rgb("#1f2a33"), {
  pad(x: 14.000mm, y: 14.000mm, block(width: 100%, height: 100%, {
    set text(fill: white, size: 10.5pt)
    set par(justify: true)
    par("Wie aus einer Weide am Bach ein Gartenhaus wurde.")
    v(1fr)
    grid(columns: (auto, 1fr), column-gutter: 4mm, [], { set text(size: 9pt); par("Erika Beispiel baut gern.") })
    v(6mm)
    grid(columns: (1fr, auto), align: (left + bottom, right + bottom), { text(size: 9pt, "Beispielverlag"); linebreak(); text(size: 9pt, "12,00 €") }, box(fill: white, inset: 3pt, { image(".baruch/isbn.svg", width: 36mm); align(center, text(size: 7.5pt, fill: black, "ISBN 978-3-16-148410-0")) }))
  }))
})

== wrap with barcode
// Baruch: Umschlag für die Druckerei (Runde 11) – Rückseite | Rücken | Vorderseite, Beschnitt 3.000mm
#set page(width: 255.000mm, height: 186.000mm, margin: 0pt)
#set text(lang: "de")
#place(top + left, block(width: 123.000mm, height: 186.000mm, fill: rgb("#1f2a33"), clip: true, inset: (left: 17.000mm, right: 14.000mm, top: 17.000mm, bottom: 17.000mm), {
  set text(fill: white, size: 10.5pt)
  set par(justify: true)
  par("Wie aus einer Weide am Bach ein Gartenhaus wurde.")
  v(1fr)
  grid(columns: (auto, 1fr), column-gutter: 4mm, [], { set text(size: 9pt); par("Erika Beispiel baut gern.") })
  v(6mm)
  grid(columns: (1fr, auto), align: (left + bottom, right + bottom), { text(size: 9pt, "Beispielverlag"); linebreak(); text(size: 9pt, "12,00 €") }, box(fill: white, inset: 3pt, { image(".baruch/isbn.svg", width: 36mm); align(center, text(size: 7.5pt, fill: black, "ISBN 978-3-16-148410-0")) }))
}))
#place(top + left, dx: 123.000mm, block(width: 9.000mm, height: 186.000mm, fill: rgb("#1f2a33"), clip: true, align(center + horizon, rotate(90deg, reflow: true, text(size: 11.00pt, fill: white, "Erika Beispiel · Das Gartenhaus am Bach")))))
#place(top + left, dx: 132.000mm, block(width: 123.000mm, height: 186.000mm, clip: true, {
  place(top + left, image(".baruch/umschlag.png", alt: "Weide", width: 100%, height: 100%, fit: "cover"))
  place(horizon + center, block(width: 100%, inset: (x: 10%, y: 9%), fill: black.transparentize(50%), {
      set align(center)
      set par(leading: 0.5em)
      text(size: 13pt, fill: white, "Erika Beispiel")
      v(0.8em)
      text(size: 30pt, weight: "bold", fill: white, "Das Gartenhaus am Bach")
      v(0.5em)
      text(size: 15pt, fill: white, "Ein Bautagebuch")
  }))
}))

== wrap without isbn
// Baruch: Umschlag für die Druckerei (Runde 11) – Rückseite | Rücken | Vorderseite, Beschnitt 3.000mm
#set page(width: 255.000mm, height: 186.000mm, margin: 0pt)
#set text(lang: "de")
#place(top + left, block(width: 123.000mm, height: 186.000mm, fill: rgb("#1f2a33"), clip: true, inset: (left: 17.000mm, right: 14.000mm, top: 17.000mm, bottom: 17.000mm), {
  set text(fill: white, size: 10.5pt)
  set par(justify: true)
  par("Wie aus einer Weide am Bach ein Gartenhaus wurde.")
  v(1fr)
  grid(columns: (auto, 1fr), column-gutter: 4mm, [], { set text(size: 9pt); par("Erika Beispiel baut gern.") })
  v(6mm)
  grid(columns: (1fr, auto), align: (left + bottom, right + bottom), { text(size: 9pt, "Beispielverlag"); linebreak(); text(size: 9pt, "12,00 €") }, box(width: 36mm, height: 22mm, stroke: 0.5pt + white, align(center + horizon, text(size: 7.5pt, "ISBN und Strichcode"))))
}))
#place(top + left, dx: 123.000mm, block(width: 9.000mm, height: 186.000mm, fill: rgb("#1f2a33"), clip: true, align(center + horizon, rotate(90deg, reflow: true, text(size: 11.00pt, fill: white, "Erika Beispiel · Das Gartenhaus am Bach")))))
#place(top + left, dx: 132.000mm, block(width: 123.000mm, height: 186.000mm, clip: true, {
  place(top + left, rect(width: 100%, height: 100%, fill: rgb("#1f2a33")))
  place(horizon + center, block(width: 100%, inset: (x: 10%, y: 9%), fill: black.transparentize(50%), {
      set align(center)
      set par(leading: 0.5em)
      text(size: 13pt, fill: white, "Erika Beispiel")
      v(0.8em)
      text(size: 30pt, weight: "bold", fill: white, "Das Gartenhaus am Bach")
      v(0.5em)
      text(size: 15pt, fill: white, "Ein Bautagebuch")
  }))
}))

== front
// Baruch: Umschlag-Vorderseite als EPUB-Cover (Runde 21c)
#set page(width: 120.000mm, height: 180.000mm, margin: 0pt)
#set text(lang: "de")
#place(top + left, block(width: 100%, height: 100%, clip: true, {
  place(top + left, image(".baruch/umschlag.png", alt: "Weide", width: 100%, height: 100%, fit: "cover"))
  place(horizon + center, block(width: 100%, inset: (x: 10%, y: 9%), fill: black.transparentize(50%), {
      set align(center)
      set par(leading: 0.5em)
      text(size: 13pt, fill: white, "Erika Beispiel")
      v(0.8em)
      text(size: 30pt, weight: "bold", fill: white, "Das Gartenhaus am Bach")
      v(0.5em)
      text(size: 15pt, fill: white, "Ein Bautagebuch")
  }))
}))

== book dict
#let baruch_buch = (
  titel: "Das Gartenhaus am Bach",
  untertitel: "Ein Bautagebuch",
  autor: "Erika Beispiel",
  verlag: "Beispielverlag",
  verlag_anschrift: "Weg 1\n12345 Musterstadt",
  ort: "Musterstadt",
  jahr: "2026",
  auflage: "1. Auflage",
  isbn: "978-3-16-148410-0",
  preis: "12,00 €",
  reihe: "Bauen am Wasser",
  druck: "Druckerei Muster",
  druck_anschrift: "Druckweg 2\n12345 Musterstadt",
  bibel: "Erfundene Übersetzung",
  bibel_hinweis: "Mit freundlicher Genehmigung.",
  bildnachweis: "Zeichnungen: Erika Beispiel",
  widmung: "Für alle, die am Bach bauen.",
  klappentext: "Wie aus einer Weide am Bach ein Gartenhaus wurde.",
  vita: "Erika Beispiel baut gern.",
  copyright: "© 2026 Erika Beispiel, Beispielverlag, Musterstadt",
  dnb_titel: "Bibliografische Information der Deutschen Nationalbibliothek",
  dnb_text: "Die Deutsche Nationalbibliothek verzeichnet diese Publikation in der Deutschen Nationalbibliografie; detaillierte bibliografische Daten sind im Internet über https://dnb.de abrufbar.",
  gesetzt: "Gesetzt mit Baruch",
  impressum_zeilen: ("Bibliografische Information der Deutschen Nationalbibliothek", "Die Deutsche Nationalbibliothek verzeichnet diese Publikation in der Deutschen Nationalbibliografie; detaillierte bibliografische Daten sind im Internet über https://dnb.de abrufbar.", "© 2026 Erika Beispiel, Beispielverlag, Musterstadt", "Weg 1, 12345 Musterstadt", "1. Auflage", "ISBN 978-3-16-148410-0", "Bibelzitate: Erfundene Übersetzung. Mit freundlicher Genehmigung.", "Umschlaggestaltung und Bildnachweis: Zeichnungen: Erika Beispiel", "Gesetzt mit Baruch", "Druck: Druckerei Muster, Druckweg 2, 12345 Musterstadt", ),
)

