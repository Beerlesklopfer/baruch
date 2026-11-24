// Kleines Testbuch für baruch-typst (eigene Datei, frei erfunden).
#set document(title: "Größe und Übermaß", author: ("Jörg Müller", "Anna Weiß"))
#set page(paper: "a5", margin: (x: 18mm, y: 20mm))
#set text(lang: "de", font: "Linux Libertine")
#let szene(body) = block(inset: 4pt, body)
#let pfad = "kapitel/*.typ"

/*
= Kein Kapitel, nur ein Kommentar
*/

= Vorwort <vorwort>

Ein kurzes Vorwort mit Umlauten: äöü ÄÖÜ ß – „Anführung“.

= Erstes Kapitel

Einleitender Text des ersten Kapitels.
 = eingerückt, also keine Überschrift

== Ein Abschnitt

```typst
= Das steht in einem Raw-Block
== und das auch
```

==== Vierte Ebene bleibt Text

=== Ein Unterabschnitt <sec:unter>

#szene[Mit `= inline` und einem Ende.]

== Zweiter Abschnitt

Text ohne Schluss-Zeilenende.