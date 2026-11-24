// Baruch: Umschlag, Rückseite (Runde 11)
#page(header: none, footer: none, numbering: none, margin: 0pt, fill: rgb("#1f2a33"), {
  pad(x: 14.000mm, y: 14.000mm, block(width: 100%, height: 100%, {
    set text(fill: white, size: 10.5pt)
    set par(justify: true)
    v(1fr)
    grid(columns: (1fr, auto), align: (left + bottom, right + bottom), [], box(width: 36mm, height: 22mm, stroke: 0.5pt + white, align(center + horizon, text(size: 7.5pt, "ISBN und Strichcode"))))
  }))
})
