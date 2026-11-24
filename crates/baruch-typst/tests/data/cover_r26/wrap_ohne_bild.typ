// Baruch: Umschlag für die Druckerei (Runde 11) – Rückseite | Rücken | Vorderseite, Beschnitt 3.000mm
#set page(width: 311.000mm, height: 216.000mm, margin: 0pt)
#set text(lang: "de")
#place(top + left, block(width: 151.000mm, height: 216.000mm, fill: rgb("#1f2a33"), clip: true, inset: (left: 17.000mm, right: 14.000mm, top: 17.000mm, bottom: 17.000mm), {
  set text(fill: white, size: 10.5pt)
  set par(justify: true)
  v(1fr)
  grid(columns: (1fr, auto), align: (left + bottom, right + bottom), [], box(width: 36mm, height: 22mm, stroke: 0.5pt + white, align(center + horizon, text(size: 7.5pt, "ISBN und Strichcode"))))
}))
#place(top + left, dx: 151.000mm, block(width: 9.000mm, height: 216.000mm, fill: rgb("#1f2a33"), clip: true, align(center + horizon, rotate(90deg, reflow: true, text(size: 11.00pt, fill: white, "Nur ein Titel")))))
#place(top + left, dx: 160.000mm, block(width: 151.000mm, height: 216.000mm, clip: true, {
  place(top + left, rect(width: 100%, height: 100%, fill: rgb("#1f2a33")))
  place(top + center, block(width: 100%, inset: (x: 10%, y: 9%), fill: gradient.linear(black.transparentize(30%), black.transparentize(100%), angle: 90deg), {
      set align(center)
      set par(leading: 0.5em)
      text(size: 30pt, weight: "bold", fill: white, "Nur ein Titel")
  }))
}))
