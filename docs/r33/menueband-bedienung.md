<!-- Runde 33, Aufgabe „menueband“: Abschnitt für docs/Bedienung.md (Platzhalter „R33 menueband“). Der Integrator setzt ihn ein und löscht docs/r33/. -->

## Menüband

Oben auf jeder Seite steht das Menüband. Es ersetzt die Menüs „Werk ▾“, „Ansicht ▾“ und „Werkzeuge ▾“ und die
Knopfreihe über dem Blatt; keine Funktion ist entfallen, alles ist auch ohne JavaScript erreichbar.

**Zeile 1:** das Menü **Baruch ▾**, die Reiter, auf der Arbeitsseite die Werkwahl, der Titel der Seite und rechts
der kleine Schalter ▴/▾ zum Ein- und Ausklappen. **Zeile 2:** die Gruppen des gewählten Reiters.

### Baruch ▾

Was das Programm betrifft und nicht ein einzelnes Werk: **Alle Werke**, **Neues Werk / Importieren…**, **PDF oder
EPUB importieren…**, **Austausch: Einlesen…**, **⚙ Einstellungen** (Alt+,) und **Über Baruch**. Mit der Tastatur: ↓
oder Enter öffnet, ↑/↓ wandern, Esc schließt.

### Reiter und Gruppen

| Reiter | Gruppen | Wo es früher stand |
|---|---|---|
| **Start** | Arbeitsseite: *Ansicht des Textes* (Editor ⇄ Vorschau), *Text* (Satzbild \| Quelltext), *Absatz* (Absatzformat, K, F), *Fassung* (‹ n/m ›, Speichern, Als neue Fassung), *Assistenten* (Prüfen, Vorschlag), *Einfälle* (Einfall festhalten Alt+E, Einfälle, Gedankenkarte). Andere Seiten: *Werk* (Zum Text), die Knöpfe der Seite, *Einfälle*. | Ansicht ▾, Werkzeuge ▾, Knopfreihe über dem Blatt, Statuszeile |
| **Einfügen** (nur Arbeitsseite) | *Einfügen* (Formate der Vorlage, Fußnote, Zitat, Liste, Aufzählung, Begriff, Umbruch, Link, Verweis), *Bild*, *Register und Glossar* (In Glossar, In Register, Nachschlagen, Register) | Knopfreihe über dem Blatt |
| **Prüfen** | *Befunde und Zitate* (Befunde, Zitate prüfen), *Suchen und Verweise* (Im Werk suchen und ersetzen Strg+Umschalt+F, Querverweise prüfen), *Quellen* (Literatur Alt+L, Abgleichen) | Werk ▾, Werkzeuge ▾ |
| **Buch** | *Werk* (Werk bearbeiten, Gliederung, Anträge zur Gliederung, Gliederungsschema, Bilder), *Satz* (Satz-Vorschau, EPUB-Vorschau), *Ausgabe* (PDF, Entwurf, EPUB, Stichwortzettel …, Exportieren (Typst), Austausch: Ausgeben), *Umschlag* (Umschlag, Umschlag-PDF), *Register und Glossar* (Glossar, Register) | Werk ▾, Ansicht ▾ |
| **Ansicht** | *Seitenleisten* (nur Arbeitsseite: Gliederung, Assistenten), *Laptop-Modus* (Automatisch, An, Aus), *Darstellung* (Thema, Zoom zurücksetzen) | Kopfzeile rechts |

Die Arbeitsseite hat alle fünf Reiter, Seiten eines Werks haben Start · Prüfen · Buch · Ansicht, Seiten ohne Werk
nur Start · Ansicht (dort führt „Werk wählen“ zur Liste der Werke; „Literatur“ und „Abgleichen“ stehen unter Start).
Jede Seite öffnet mit dem Reiter, zu dem sie gehört – die Satz-Vorschau mit **Buch**, die Befunde mit **Prüfen**.

Umbenannt: „Satz“ heißt **Satz-Vorschau**, „Als PDF/Entwurf/EPUB herunterladen“ heißen in der Gruppe *Ausgabe* kurz
**PDF**, **Entwurf**, **EPUB**; „Editor“ außerhalb der Arbeitsseite heißt **Zum Text**.

**Abzeichen** an den Reitern zeigen, wo etwas wartet: an **Start** die offenen Vorschläge im Werk, an **Prüfen**
„N offen“ (offene Befunde), an **Buch** die offenen Anträge zur Gliederung. Sie stehen auf jeder Seite, auch ohne
JavaScript.

**Reiter wechseln:** Klick, oder mit der Tastatur ←/→ (läuft am Ende um), Pos1 und Ende; Tab führt vom gewählten
Reiter in sein Feld. Tastenkürzel wie bisher: Strg+S (⌘+S) speichert, Alt+L Literatur, Alt+E Einfall festhalten,
Alt+, Einstellungen, Alt+←/→ vorige/nächste Fassung, Strg+Umschalt+F im Werk suchen.

**Speichern im Band:** „Speichern“ und „Als neue Fassung“ in der Gruppe *Fassung* tun dasselbe wie die Knöpfe der
Statuszeile unter dem Blatt; ohne Änderung speichert „Speichern“ nichts. „Prüfen“ und „Vorschlag“ in der Gruppe
*Assistenten* gelten für die Persona, die rechts im Akkordeon „Assistenten“ gewählt ist (samt Hinweis, Bereich und
Prüfgrundlage).

### Band ein- und ausklappen

Ein Klick auf den **gewählten Reiter** oder auf **▴** rechts klappt das Band zu; es bleibt nur Zeile 1. Baruch merkt
sich das (Cookie `baruch_band`). Im zugeklappten Band zeigt ein Klick auf einen Reiter dessen Gruppen
**vorübergehend** über dem Text – nach dem Auslösen eines Knopfs, mit Esc oder einem Klick daneben verschwinden sie
wieder. **▾** rechts klappt das Band dauerhaft auf.

### Schmale Fenster (Laptop-Modus)

Im Laptop-Modus (Reiter Ansicht) ist das Band kompakt: Jede Gruppe ist ein Knopf „Name ▾“, der seine Einträge als
Popup zeigt; es ist immer nur ein Popup offen, Esc oder ein Klick daneben schließt es.

### Ohne JavaScript

- Reiter, Gruppen, das Menü Baruch ▾, Band auf/zu und der Laptop-Modus sind gewöhnliche Links und aufklappbare
  Bereiche: Ein Klick lädt die Seite mit dem neuen Zustand (`?tab=…`, `?band=…`, `?laptop=…`); Werk, Abschnitt und
  Fassung bleiben in der Adresse.
- Die Knöpfe des Editors (Gruppen *Text*, *Absatz*, *Fassung*, *Assistenten* und der ganze Reiter **Einfügen**)
  brauchen JavaScript. Ohne JavaScript arbeitet man wie bisher im Textfeld und speichert mit „Speichern“ unter dem
  Blatt; der Reiter Einfügen nennt dann die beiden Wege am Blatt: „Bild einfügen …“ und „Markierung …“ (Glossar und
  Register). „Zoom zurücksetzen“ gibt es nur mit JavaScript.
- **Bekannte Grenze:** Ist eine Seite die Antwort auf ein abgeschicktes Formular (zum Beispiel die Rückfrage
  „Speichern bestätigen“), führt ein Reiter ohne JavaScript per einfachem Aufruf auf dieselbe Adresse – die Antwort
  des Formulars steht dann nicht mehr da. Mit JavaScript wechseln die Reiter ohne Neuladen.
