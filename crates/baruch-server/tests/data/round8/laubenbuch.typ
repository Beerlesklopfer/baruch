#set text(font: ("Vorlagenschrift Fehlt", "Libertinus Serif"), lang: "de")
#set page(width: 12cm, height: 16cm)
#set heading(numbering: "1.1")
#show heading.where(level: 1): it => { pagebreak(weak: true); it }
#let szene(body) = block(inset: 4pt, stroke: 0.5pt, body)
= Die Laube
Anna sitzt im Herbstlaub und zählt die Äpfel.
#szene[Ein Windstoß fährt durch die Blätter.]
= Der Winter
Schnee liegt auf der leeren Bank.
== Am Ofen
#text(font: "Ofenschrift Fehlt")[Die Katze schläft.]
#figure(image("./bilder/laube.svg", width: 3cm), caption: [Die Laube im Schnee])
= Der Frühling
Die Äpfel blühen wieder.
