// Frei erfundene Testdatei für das Satzbild (baruch-wysiwyg): Szenen, Arbeitsaufträge, Labels an
// Überschriften, Fußnoten und eine Bildinsel.
#set text(size: 10.5pt)
#let szene(ort: none, body) = block(fill: luma(245), inset: 8pt)[#body]
#let arbeit(body) = block(fill: rgb("#eef3fb"), inset: 8pt)[#body]

= Die Werkstatt am Fluss
<werkstatt>
#szene[
  Der Morgen begann mit dem Geräusch von Hobeln.

  Jonas legte das Brett auf die Bank und strich mit der Hand darüber.#footnote[Eiche, sagte er, immer Eiche.]

  Draußen rief jemand seinen Namen, _zweimal_.
]

Im Fließtext steht eine Bemerkung#footnote[Erfunden wie alles hier.] und noch eine.#footnote[Auch diese.]

#arbeit[
  Überlege dir:

  - Welche Geräusche gehören zu deinem Morgen?
  - Wer ruft deinen Namen,
    und wie klingt es?
]

== Der Auftrag <auftrag>

#szene(ort: [Hof])[
  Die Kundin stand schon am Tor.

  #quote(block: true)[Ein Tisch für zwölf Leute.]
]

#figure(image("/bilder/erfunden.svg"), caption: [Ein erfundener Grundriss der Werkstatt.])

#arbeit[Wofür würdest du einen Tisch bauen?]

Zum Schluss ein gewöhnlicher Absatz.
