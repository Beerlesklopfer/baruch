// Baruch: Umschlag, Rückseite (Runde 11)
#page(header: none, footer: none, numbering: none, margin: 0pt, fill: rgb("#1f2a33"), {
  pad(x: 14.000mm, y: 14.000mm, block(width: 100%, height: 100%, {
    set text(fill: white, size: 10.5pt)
    set par(justify: true)
    par("Ein Haus aus Wolken.\nZweite Zeile.")
    par("Ein zweiter Absatz.")
    v(1fr)
    grid(columns: (auto, 1fr), column-gutter: 4mm, image(".baruch/foto.png", width: 22mm), { set text(size: 9pt); par("Mira Mondschein wohnt unterm Dach.") })
    v(6mm)
    grid(columns: (1fr, auto), align: (left + bottom, right + bottom), { text(size: 9pt, "Lampenverlag"); linebreak(); text(size: 9pt, "18,00 €") }, box(fill: white, inset: 3pt, { image(".baruch/isbn.svg", width: 36mm); align(center, text(size: 7.5pt, fill: black, "ISBN 978-3-16-148410-0")) }))
  }))
})
