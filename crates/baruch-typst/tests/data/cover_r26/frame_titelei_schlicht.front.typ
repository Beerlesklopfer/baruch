// Baruch: Umschlag und Titelei (Runde 11)
// Titelei
#page(header: none, footer: none, numbering: none, {
  set align(center)
  v(22%)
  text(size: 16pt, "Nur ein Titel")
})
#page(header: none, footer: none, numbering: none, {
  hide[ ]
})
#page(header: none, footer: none, numbering: none, {
  set align(center)
  v(12%)
  text(size: 26pt, weight: "bold", "Nur ein Titel")
  v(1fr)
})
#page(header: none, footer: none, numbering: none, {
  set text(size: 8.5pt)
  set par(justify: false)
  v(1fr)
  block(spacing: 0.8em, text("Bibliografische Information der Deutschen Nationalbibliothek", weight: "bold"))
  block(spacing: 0.8em, text("Die Deutsche Nationalbibliothek verzeichnet diese Publikation in der Deutschen Nationalbibliografie; detaillierte bibliografische Daten sind im Internet über https://dnb.de abrufbar."))
  block(spacing: 0.8em, text("©"))
  block(spacing: 0.8em, text("Gesetzt mit Baruch"))
})
