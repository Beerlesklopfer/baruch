== a5
// Stichwortzettel – erzeugt von Baruch (baruch-render::notes). Alle Texte sind Zeichenketten.
#set document(title: "Stichwortzettel – Das verlorene Schaf")
#set page(paper: "a5", margin: (x: 14mm, top: 16mm, bottom: 14mm), header: context align(right, text(size: 0.6em, fill: luma(90))[#"Das verlorene Schaf" · #counter(page).display()]))
#set text(size: 16pt, lang: "de")
#set par(justify: false, leading: 0.75em)
#set list(indent: 0.4em, body-indent: 0.5em, spacing: 0.7em)
#let card(c) = block(width: 100%, breakable: true, below: 1.1em)[
  #metadata(c.title) <baruch-note>
  #for p in c.points [#metadata(p) <baruch-note>]
  #block(sticky: true, below: 0.65em)[
    #grid(columns: (1fr, auto), column-gutter: 0.8em,
      pad(left: c.depth * 0.8em, text(weight: "bold", size: 1.1em)[#if c.number != "" [#c.number ]#c.title]),
      align(right, text(size: 0.75em, fill: luma(70))[#c.start (#c.clock)]))
    #v(-0.3em)
    #line(length: 100%, stroke: 0.5pt + luma(150))
  ]
  #if c.points.len() > 0 [#list(..c.points.map(p => [#p]))]
  #if c.refs.len() > 0 [#text(size: 0.85em, style: "italic")[#c.refs.join(" · ")]]
]
#text(size: 0.8em)[#strong("Das verlorene Schaf") #h(1fr) #"Gesamt 18:30 · Ziel 20:00"]

#card((number: "Einleitung", title: "Hundert Schafe", depth: 0, start: "0:00", clock: "2:10", points: ("Einer fehlt", "Der Hirte zählt \"genau\"", ), refs: ("Lk 15,4–6", )))
#card((number: "I.", title: "Suchen", depth: 1, start: "2:10", clock: "6:00", points: (), refs: ()))
== karten
// Stichwortzettel – erzeugt von Baruch (baruch-render::notes). Alle Texte sind Zeichenketten.
#set document(title: "Stichwortzettel – Das verlorene Schaf")
#set page(paper: "a6", flipped: true, margin: (x: 9mm, top: 11mm, bottom: 8mm), header: context align(right, text(size: 0.6em, fill: luma(90))[#"Das verlorene Schaf" · #counter(page).display()]))
#set text(size: 14pt, lang: "de")
#set par(justify: false, leading: 0.75em)
#set list(indent: 0.4em, body-indent: 0.5em, spacing: 0.7em)
#let card(c) = block(width: 100%, breakable: true, below: 0.8em)[
  #metadata(c.title) <baruch-note>
  #for p in c.points [#metadata(p) <baruch-note>]
  #block(sticky: true, below: 0.65em)[
    #grid(columns: (1fr, auto), column-gutter: 0.8em,
      pad(left: c.depth * 0.8em, text(weight: "bold", size: 1.1em)[#if c.number != "" [#c.number ]#c.title]),
      align(right, text(size: 0.75em, fill: luma(70))[#c.start (#c.clock)]))
    #v(-0.3em)
    #line(length: 100%, stroke: 0.5pt + luma(150))
  ]
  #if c.points.len() > 0 [#list(..c.points.map(p => [#p]))]
  #if c.refs.len() > 0 [#text(size: 0.85em, style: "italic")[#c.refs.join(" · ")]]
]
#text(size: 0.8em)[#strong("Das verlorene Schaf") #h(1fr) #"Gesamt 18:30 · Ziel 20:00"]

#card((number: "Einleitung", title: "Hundert Schafe", depth: 0, start: "0:00", clock: "2:10", points: ("Einer fehlt", "Der Hirte zählt \"genau\"", ), refs: ("Lk 15,4–6", )))
#pagebreak()
#card((number: "I.", title: "Suchen", depth: 1, start: "2:10", clock: "6:00", points: (), refs: ()))
== empty
// Stichwortzettel – erzeugt von Baruch (baruch-render::notes). Alle Texte sind Zeichenketten.
#set document(title: "Stichwortzettel – Leer")
#set page(paper: "a5", margin: (x: 14mm, top: 16mm, bottom: 14mm), header: context align(right, text(size: 0.6em, fill: luma(90))[#"Leer" · #counter(page).display()]))
#set text(size: 16pt, lang: "de")
#set par(justify: false, leading: 0.75em)
#set list(indent: 0.4em, body-indent: 0.5em, spacing: 0.7em)
#let card(c) = block(width: 100%, breakable: true, below: 1.1em)[
  #metadata(c.title) <baruch-note>
  #for p in c.points [#metadata(p) <baruch-note>]
  #block(sticky: true, below: 0.65em)[
    #grid(columns: (1fr, auto), column-gutter: 0.8em,
      pad(left: c.depth * 0.8em, text(weight: "bold", size: 1.1em)[#if c.number != "" [#c.number ]#c.title]),
      align(right, text(size: 0.75em, fill: luma(70))[#c.start (#c.clock)]))
    #v(-0.3em)
    #line(length: 100%, stroke: 0.5pt + luma(150))
  ]
  #if c.points.len() > 0 [#list(..c.points.map(p => [#p]))]
  #if c.refs.len() > 0 [#text(size: 0.85em, style: "italic")[#c.refs.join(" · ")]]
]
#text(size: 0.8em)[#strong("Leer") #h(1fr) #"Gesamt "]

#emph("Keine Abschnitte.")
