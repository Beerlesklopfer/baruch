<!-- Runde 33, Aufgabe „laptop“: Abschnitt für docs/Bedienung.md (Platzhalter `<!-- R33 laptop -->`). -->

### Laptop-Modus

Auf einem kleinen Bildschirm soll der Text den Platz bekommen, nicht die Bedienung. Der Laptop-Modus hat dafür
drei Teile:

1. **Kompaktes Band.** Die Gruppen des Menübands schrumpfen zu Knöpfen „Name ▾“, die ihren Inhalt als Popup
   zeigen (siehe Menüband).
2. **Seitenspalten als Leisten.** Gliederung (links) und Assistenten (rechts) klappen zu je einer schmalen Leiste am
   Rand ein. Der Editor bekommt fast die ganze Breite.
3. **Schubladen.** Ein Klick auf eine Leiste holt ihre Spalte über den Text – so breit wie nötig, ohne den Text
   zusammenzuschieben – und legt sie danach wieder weg.

#### Schalter: Ansicht → Laptop-Modus

Im Reiter **Ansicht** stehen in der Gruppe „Laptop-Modus“ drei Schalter:

| Schalter | Wirkung |
|---|---|
| **Automatisch** (Vorgabe) | Baruch richtet sich nach der Breite des Fensters (Schwellen unten). |
| **An** | Kompaktes Band und Leisten, gleich wie breit das Fenster ist. |
| **Aus** | Volles Band und offene Spalten, auch in einem schmalen Fenster. |

Die Wahl wird gemerkt (Cookie `baruch_laptop`, ein Jahr) und gilt auf allen Seiten. Der gedrückte Schalter ist
die geltende Wahl.

#### Schwellen bei „Automatisch“

Gemessen wird die innere Breite des Browserfensters, beim Laden und bei jeder Größenänderung:

| Fensterbreite | Band | Seitenspalten |
|---|---|---|
| ab 1600 px | voll | offen |
| 1280 bis 1599 px | kompakt | offen |
| unter 1280 px | kompakt | Leisten |

Ein Fenster in voller Größe auf einem Bildschirm mit 1920 × 1080 ist also „voll“, eines mit 1366 × 768
„kompakt“, ein halbes Fenster oder 1024 px Breite „kompakt mit Leisten“. Zieht man das Fenster über eine Schwelle,
wechselt die Ansicht sofort, ohne dass die Seite neu lädt; ungespeicherter Text bleibt stehen.

#### Leisten und Schubladen

- **Eine Spalte von Hand einklappen:** „Einklappen“ oben in der Spalte, oder Ansicht → Seitenleisten →
  „Gliederung“ bzw. „Assistenten“ (gedrückt = Spalte sichtbar). Die Spalte wird zur Leiste und bleibt es, bis man
  sie wieder öffnet (gemerkt in den Cookies `baruch_lrail` und `baruch_rrail`). Die andere Spalte bleibt, wie sie
  ist; die Grenze zur Leiste lässt sich nicht ziehen, die gemerkte Spaltenbreite bleibt aber erhalten.
- **Leiste im breiten Fenster:** Ein Klick auf die Leiste klappt die Spalte wieder dauerhaft auf.
- **Leiste im schmalen Zustand** (unter 1280 px oder Laptop-Modus „An“): Die Leiste öffnet die Spalte als
  **Schublade** über dem Text. Sie schließt sich mit Esc, mit einem Klick daneben, mit „Schließen“ oben in der
  Schublade – und von selbst, sobald man in der Gliederung einen Abschnitt wählt. Nach Esc und „Schließen“ steht
  der Fokus wieder dort, von wo die Schublade geöffnet wurde. Die Schublade wird nicht gemerkt: Nach dem Schließen
  ist die Spalte wieder Leiste.
- Die Schublade der Assistenten bleibt offen, während man darin arbeitet (Chat, Prüfen, Fassungen) – sie schließt
  erst auf Wunsch.
- Im schmalen Zustand lassen sich die Spalten nicht dauerhaft neben den Text stellen; dafür den Laptop-Modus auf
  „Aus“ stellen oder das Fenster verbreitern.

Alles Übrige bleibt, wie es ist: Spaltenbreiten (Grenzen ziehen oder ←/→), Zoom je Bereich (Strg+Mausrad, Strg+0
für den Bereich unter dem Mauszeiger). Neu im Reiter Ansicht ist **„Zoom zurücksetzen“**: Der Knopf setzt alle
Bereiche auf einmal auf 100 % zurück.

#### Ohne JavaScript

- Die Schalter sind gewöhnliche Links (`?laptop=on`, `?laptop=off`, `?laptop=auto`, `?lrail=1`, `?rrail=1`); der
  Server merkt die Wahl als Cookie und zeichnet die Seite entsprechend.
- **Die Fensterbreite kennt der Server ohne JavaScript nicht.** „Automatisch“ zeichnet dann immer breit (volles
  Band, offene Spalten). Auf einem kleinen Bildschirm stellt man den Laptop-Modus von Hand auf **„An“**.
- Eine Leiste öffnet ihre Spalte auch ohne JavaScript als Schublade (Sprungziel `#pane-tree` bzw.
  `#pane-assistants`); „Schließen“ oben in der Schublade oder ein Klick auf die abgedunkelte Fläche daneben legt
  sie wieder weg. Wer in der Gliederung einen Abschnitt wählt, lädt die Seite neu – die Schublade ist dann zu.
  Esc schließt ohne JavaScript nicht.
- „Zoom zurücksetzen“ gibt es nur mit JavaScript (der Bereichs-Zoom selbst auch).

#### Geändert gegenüber früher

Bisher stellte Baruch die drei Spalten in Fenstern unter 900 px untereinander. Das ist entfallen: In schmalen
Fenstern klappen die Seitenspalten jetzt zu Leisten ein, und der Text behält die volle Höhe.
