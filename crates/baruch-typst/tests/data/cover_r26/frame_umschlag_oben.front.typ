// Baruch: Umschlag und Titelei (Runde 11)
// Umschlag, Vorderseite
#page(header: none, footer: none, numbering: none, margin: 0pt, {
  place(top + left, image(".baruch/wolke.png", alt: "Eine \"Wolke\" ]", width: 100%, height: 100%, fit: "cover"))
  place(top + center, block(width: 100%, inset: (x: 10%, y: 9%), fill: gradient.linear(black.transparentize(30%), black.transparentize(100%), angle: 90deg), {
      set align(center)
      set par(leading: 0.5em)
      text(size: 13pt, fill: white, "Mira Mondschein")
      v(0.8em)
      text(size: 30pt, weight: "bold", fill: white, "Das Wolkenhaus")
      v(0.5em)
      text(size: 15pt, fill: white, "Geschichten vom Dachboden")
  }))
})
// Umschlag, Innenseite (leer)
#page(header: none, footer: none, numbering: none, {
  counter(page).update(0)
  hide[ ]
})
