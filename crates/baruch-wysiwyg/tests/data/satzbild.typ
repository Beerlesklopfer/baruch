// Frei erfundene Testdatei für das Satzbild (baruch-wysiwyg): alle erkannten Konstrukte und Inseln.
#set text(font: "Erfundene Antiqua", size: 10.5pt, fill: rgb("#2a2a33"))
#let kasten(inhalt) = block(fill: luma(235), inset: 6pt)[#inhalt]

= Der Brunnen am Marktplatz <brunnen>

Am Morgen holte Mira _frisches_ Wasser vom *alten* Brunnen.
Die Glocke schlug sieben Mal.#footnote[Nach der Uhr am Rathaus, die oft nachgeht.]

== Ein Gespräch // Notiz: Ton prüfen

Sie fragte: #quote[Wer hat den Brunnen gebaut?] und wartete.
Der Händler sagte nur „Hm“ und zeigte auf die Inschrift <inschrift>.

Preise: 3 \$ für den Krug, 2 \* 2 Becher, Pfad C:\\Daten\\krug.txt, Datei_name_lang.
Geschützt~Leerzeichen und ein Unicode-Zeichen \u{2014} dazwischen.

- Ein Krug aus Ton
- Zwei Becher, _blau_ glasiert
  und mit Sprung
- Ein Tuch

+ Erstens: Wasser holen
+ Zweitens: Feuer machen

#quote(block: true, attribution: [Inschrift am Brunnen])[
  Wer hier schöpft, denke an die, die gruben.
]

#szene[
  Mira setzt sich auf den Rand.

  Ein Hund läuft vorbei, *schnell* und ohne Halsband.
]

#arbeit[
  Wann hast du zuletzt an die gedacht, die vor dir gegraben haben?
]

=== Inseln

Eine Formel $a^2 + b^2 = c^2$ im Satz, `rohtext` und ein Verweis auf @brunnen.
Ein Link #link("https://example.org")[Beispiel] und http://example.org/pfad bleiben.
Eine eigene Funktion #kasten[im Kasten] mitten im Absatz. /* Kommentar im Satz */ Weiter.

#pagebreak()

$ sum_(k=1)^n k = (n(n+1)) / 2 $

```rust
fn main() { println!("= keine Überschrift"); }
```

1. Aufzählung mit Ziffer
2. zweiter Punkt

/ Begriff: Erklärung dazu

- äußerer Punkt
  - innerer Punkt

Ein Umbruch \
mitten im Satz.

#show heading: set text(fill: red)
#let zahl = 3

Zum Schluss ein Satz mit [eckigen Klammern] und ein _offener Satz.

- gemischt außen
  + innen mit anderem Zeichen

/ Außen: eins
  / Innen: zwei

3. springt
5. weiter
