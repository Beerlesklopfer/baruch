// Testbuch mit Teilen (frei erfunden für Runde 14)
#set document(title: "Vom Brunnenbauer", author: "Testfeder")
#set heading(numbering: "1.1")
#set page(width: 120mm, height: 170mm)

// Ein frei geschriebenes heading bleibt Werkvorlage:
#heading(level: 1, numbering: none)[Kein Teil] <anderes-label>

#heading(level: 1, numbering: none)[Vorwort] <baruch-unnumbered> // steht vor allem
Ein Wort vorab über Brunnen und Geduld.

#heading(level: 1, numbering: none)[Erster Teil: Wasser suchen] <baruch-part>

= Die Wünschelrute
Der Brunnenbauer geht mit der Rute über die Wiese.

== Am Bach
Hier rauscht es schon.

#heading(level: 2, numbering: none)[Zwischenruf \[ohne Zahl\]] <baruch-unnumbered>
Ein kurzer Einschub.

=== Tiefer graben
Spaten, Eimer, Seil.

#heading(level: 1, numbering: none)[\- Zweiter Teil \# mit Zeichen] <baruch-part>	// Kommentar hinter dem Teil

= Der Stein im Weg
```typ
#heading(level: 1, numbering: none)[Im Raw] <baruch-part>
```
/* #heading(level: 1, numbering: none)[Im Kommentar] <baruch-part> */
#block[
#heading(level: 1, numbering: none)[Im Inhaltsblock] <baruch-part>
]

#heading(level: 1, numbering: none)[\= Gleich] <baruch-unnumbered>
Ein Kapitel ohne Nummer mit führendem Gleichheitszeichen.

#heading(level: 1, numbering: none)[2026\. ein Jahr] <baruch-unnumbered>
Und eines mit führender Jahreszahl.

= Nachwort
Ende.
