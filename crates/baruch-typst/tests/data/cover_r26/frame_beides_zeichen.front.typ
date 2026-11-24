// Baruch: Umschlag und Titelei (Runde 11)
// Umschlag, Vorderseite
#page(header: none, footer: none, numbering: none, margin: 0pt, {
  place(top + left, image(".baruch/wolke.png", alt: "Eine \"Wolke\" ]", width: 100%, height: 100%, fit: "cover"))
  place(horizon + center, block(width: 100%, inset: (x: 10%, y: 9%), fill: black.transparentize(50%), {
      set align(center)
      set par(leading: 0.5em)
      text(size: 13pt, fill: white, "Zitat \" Rückstrich \\ #raute ] klammer ) $mathe$ // kommentar /* block */ *fett* _x_\nzweite Zeile")
      v(0.8em)
      text(size: 30pt, weight: "bold", fill: white, "Zitat \" Rückstrich \\ #raute ] klammer ) $mathe$ // kommentar /* block */ *fett* _x_\nzweite Zeile")
      v(0.5em)
      text(size: 15pt, fill: white, "Zitat \" Rückstrich \\ #raute ] klammer ) $mathe$ // kommentar /* block */ *fett* _x_\nzweite Zeile")
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
  text(size: 16pt, "Zitat \" Rückstrich \\ #raute ] klammer ) $mathe$ // kommentar /* block */ *fett* _x_\nzweite Zeile")
})
#page(header: none, footer: none, numbering: none, {
  set align(center)
  v(22%)
  text(size: 11pt, style: "italic", "Zitat \" Rückstrich \\ #raute ] klammer ) $mathe$ // kommentar /* block */ *fett* _x_\nzweite Zeile")
})
#page(header: none, footer: none, numbering: none, {
  set align(center)
  v(12%)
  text(size: 13pt, "Zitat \" Rückstrich \\ #raute ] klammer ) $mathe$ // kommentar /* block */ *fett* _x_\nzweite Zeile")
  v(2em)
  text(size: 26pt, weight: "bold", "Zitat \" Rückstrich \\ #raute ] klammer ) $mathe$ // kommentar /* block */ *fett* _x_\nzweite Zeile")
  v(1em)
  text(size: 14pt, "Zitat \" Rückstrich \\ #raute ] klammer ) $mathe$ // kommentar /* block */ *fett* _x_\nzweite Zeile")
  v(1fr)
  text(size: 11pt, "Zitat \" Rückstrich \\ #raute ] klammer ) $mathe$ // kommentar /* block */ *fett* _x_\nzweite Zeile")
  linebreak()
  text(size: 10pt, "Zitat \" Rückstrich \\ #raute ] klammer ) $mathe$ // kommentar /* block */ *fett* _x_\nzweite Zeile")
})
#page(header: none, footer: none, numbering: none, {
  set text(size: 8.5pt)
  set par(justify: false)
  v(1fr)
  block(spacing: 0.8em, text("Bibliografische Information der Deutschen Nationalbibliothek", weight: "bold"))
  block(spacing: 0.8em, text("Die Deutsche Nationalbibliothek verzeichnet diese Publikation in der Deutschen Nationalbibliografie; detaillierte bibliografische Daten sind im Internet über https://dnb.de abrufbar."))
  block(spacing: 0.8em, text("© 20\"26 Zitat \" Rückstrich \\ #raute ] klammer ) $mathe$ // kommentar /* block */ *fett* _x_\nzweite Zeile, Zitat \" Rückstrich \\ #raute ] klammer ) $mathe$ // kommentar /* block */ *fett* _x_\nzweite Zeile, Zitat \" Rückstrich \\ #raute ] klammer ) $mathe$ // kommentar /* block */ *fett* _x_\nzweite Zeile"))
  block(spacing: 0.8em, text("Zitat \" Rückstrich \\ #raute ] klammer ) $mathe$ // kommentar /* block */ *fett* _x_\nzweite Zeile"))
  block(spacing: 0.8em, text("ISBN kein ) ISBN"))
  block(spacing: 0.8em, text("Bibelzitate: Zitat \" Rückstrich \\ #raute ] klammer ) $mathe$ // kommentar /* block */ *fett* _x_\nzweite Zeile. Zitat \" Rückstrich \\ #raute ] klammer ) $mathe$ // kommentar /* block */ *fett* _x_\nzweite Zeile"))
  block(spacing: 0.8em, text("Umschlaggestaltung und Bildnachweis: Zitat \" Rückstrich \\ #raute ] klammer ) $mathe$ // kommentar /* block */ *fett* _x_\nzweite Zeile"))
  block(spacing: 0.8em, text("Gesetzt mit Baruch"))
  block(spacing: 0.8em, text("Druck: Zitat \" Rückstrich \\ #raute ] klammer ) $mathe$ // kommentar /* block */ *fett* _x_\nzweite Zeile"))
})
#page(header: none, footer: none, numbering: none, {
  set align(center)
  v(25%)
  set text(style: "italic")
  par("Zitat \" Rückstrich \\ #raute ] klammer ) $mathe$ // kommentar /* block */ *fett* _x_\nzweite Zeile")
})
#page(header: none, footer: none, numbering: none, {
  hide[ ]
})
