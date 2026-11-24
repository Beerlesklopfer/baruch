// Grenzfälle für den Parser von baruch-typst (eigene Datei, frei erfunden).
/*
#set document(title: "Auskommentierter Titel")
*/
#set document(title: "Grenzfälle", author: "Test") // #set document(title: "hinten")
#let hinweis = "
= Kein Kapitel aus einem String
"
#show heading.where(level: 1): it => [
= Kein Kapitel aus einer Show-Regel
#it
]

= Erstes Kapitel // Arbeitstitel

#block(inset: 4pt)[
  Ein Kasten.
= Kein Kapitel im Kasten
  [eckige Klammern im Kasten]
]

Inline-Raw über zwei Zeilen: `erste Zeile
= Kein Kapitel im Raw
` und weiter.

Fließtext mit [offener Klammer, die nichts verschluckt.

== Abschnitt mit Link https://example.org/a

#figure(
  caption: [
= Kein Kapitel in der Bildunterschrift
  ],
)[Bild]

= Zweites Kapitel

Ende.
