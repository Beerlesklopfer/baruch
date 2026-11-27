#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Prüft die gebaute Webseite (site/public) nach dem Hugo-Bau. Nur Python-Standardbibliothek, kein Netz.

    python3 scripts/site-check.py [ORDNER] [--ohne-hilfe] [--basis ADRESSE]
    python3 scripts/site-check.py --selbsttest

ORDNER ist die Ausgabe von Hugo (Vorgabe: site/public im Repository). Geprüft wird:

 1. Jedes interne href/src/srcset zeigt auf eine vorhandene Datei, jeder Anker (#…) auf eine vorhandene id.
    Intern ist, was mit „/“ beginnt, relativ ist oder mit der Adresse der Seite (--basis) beginnt. Ausgenommen
    ist /apt/: Das Repository kommt erst in der CI dazu (scripts/ci-site.sh --apt).
 2. Kein Link auf docs/Bedienung.md oder docs/Betrieb.md bei GitHub: Die Webseite ist seit Runde 37 selbst die
    Quelle für Bedienung und Betrieb; ein Link dorthin führte Leser im Kreis.
 3. Unter /hilfe/ steht im <main> kein Link zu github.com: Die Hilfe ist für Anwender und muss ohne das
    Repository auskommen.
 4. Jedes <img> hat einen nichtleeren alt-Text.
 5. /bedienung/index.html leitet auf /hilfe/ weiter (alte Adresse der Bedienung) und /hilfe/index.html liegt vor.
    --ohne-hilfe lässt diese eine Prüfung aus (für Seiten ohne Abschnitt Hilfe, etwa die Probe in
    scripts/ci-selftest.sh).
 6. Kein JavaScript (kein <script>, keine on…-Attribute, kein javascript:-Link) und keine fremde Adresse für
    Stil, Schrift oder Bild (src, srcset, <link rel=stylesheet|icon|preload…>, url(…) und @import in CSS).

Exit 0 ohne Fund, 1 mit der Liste der Fundstellen (Datei: Meldung), 2 bei falschem Aufruf.
--selbsttest baut einen erfundenen Mini-Baum, prüft ihn fehlerfrei und danach je eine eingebaute Störung;
jede Zusicherung nennt ihre Begründung.
"""

import argparse
import os
import re
import sys
import tempfile
from html.parser import HTMLParser
from urllib.parse import unquote, urlsplit

BASIS = "https://baruch.bernau.family/"
# Files in the repository whose content has stood on the website since Round 37.
VERBOTENE_DOKU = ("blob/main/docs/Bedienung.md", "blob/main/docs/Betrieb.md")
# rel values of <link> that make the browser load something.
LADENDE_REL = {"stylesheet", "icon", "preload", "prefetch", "modulepreload", "apple-touch-icon", "manifest"}
# Attributes whose value the browser loads without the reader doing anything.
LADENDE_ATTRIBUTE = {
    "img": ("src", "srcset"),
    "source": ("src", "srcset"),
    "video": ("src", "poster"),
    "audio": ("src",),
    "track": ("src",),
    "iframe": ("src",),
    "embed": ("src",),
    "object": ("data",),
    "input": ("src",),
}


class Seite(HTMLParser):
    """Sammelt aus einer HTML-Datei ids, Verweise, Bilder und alles, was nach JavaScript aussieht."""

    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.ids = set()
        self.links = []       # (address, im_main)
        self.geladen = []     # (address, description) – the browser loads it by itself
        self.bilder_ohne_alt = []
        self.skripte = []
        self.weiterleitung = None
        self.im_main = 0

    def handle_starttag(self, tag, attrs):
        a = {k: (v or "") for k, v in attrs}
        if tag == "main":
            self.im_main += 1
        if a.get("id"):
            self.ids.add(a["id"])
        if tag == "a" and a.get("name"):
            self.ids.add(a["name"])
        if tag == "script":
            self.skripte.append("<script>")
        for name, value in a.items():
            if name.startswith("on"):
                self.skripte.append(f"Attribut {name} an <{tag}>")
            if name in ("href", "src", "action") and value.strip().lower().startswith("javascript:"):
                self.skripte.append(f"javascript:-Adresse an <{tag}>")
        if tag == "a" and "href" in a:
            self.links.append((a["href"], self.im_main > 0))
        if tag == "link" and "href" in a:
            rel = set(a.get("rel", "").lower().split())
            if rel & LADENDE_REL:
                self.geladen.append((a["href"], f"<link rel={a.get('rel', '')}>"))
            else:
                self.links.append((a["href"], False))
        for name in LADENDE_ATTRIBUTE.get(tag, ()):
            if name not in a:
                continue
            if name == "srcset":
                for teil in a[name].split(","):
                    teil = teil.strip().split()
                    if teil:
                        self.geladen.append((teil[0], f"<{tag} srcset>"))
            else:
                self.geladen.append((a[name], f"<{tag} {name}>"))
        if tag == "img" and not a.get("alt", "").strip():
            self.bilder_ohne_alt.append(a.get("src", "(ohne src)"))
        if tag == "meta" and a.get("http-equiv", "").lower() == "refresh":
            treffer = re.search(r"url\s*=\s*(.+)$", a.get("content", ""), re.I)
            if treffer:
                self.weiterleitung = treffer.group(1).strip().strip("'\"")

    def handle_endtag(self, tag):
        if tag == "main" and self.im_main:
            self.im_main -= 1


def lies(pfad):
    with open(pfad, encoding="utf-8", errors="replace") as datei:
        return datei.read()


def intern(adresse, basis):
    """Pfad (mit führendem /) einer internen Adresse samt Anker, sonst None. `relativ` meldet relative Pfade."""
    teile = urlsplit(adresse)
    if teile.scheme in ("mailto", "tel", "data"):
        return None
    wurzel = urlsplit(basis)
    if teile.scheme or teile.netloc:
        if teile.scheme in ("http", "https", "") and teile.netloc == wurzel.netloc:
            return teile.path or "/", teile.fragment, False
        return None
    return teile.path, teile.fragment, not teile.path.startswith("/")


def fremd(adresse, basis):
    teile = urlsplit(adresse)
    if teile.scheme == "data":
        return False
    if teile.scheme or teile.netloc:
        return teile.netloc != urlsplit(basis).netloc
    return False


def ziel_datei(ordner, pfad):
    """Datei im Ausgabeordner zu einem Pfad der Seite; Ordner stehen für ihre index.html."""
    pfad = unquote(pfad)
    voll = os.path.normpath(os.path.join(ordner, pfad.lstrip("/")))
    if not (voll == ordner or voll.startswith(ordner + os.sep)):
        return None
    if os.path.isdir(voll):
        voll = os.path.join(voll, "index.html")
    return voll


def pruefe(ordner, basis=BASIS, mit_hilfe=True):
    """Liste der Fundstellen als (Datei relativ zum Ordner, Meldung)."""
    ordner = os.path.abspath(ordner)
    funde = []
    seiten = {}
    stile = []
    for wurzel, verzeichnisse, dateien in os.walk(ordner):
        verzeichnisse.sort()
        if os.path.relpath(wurzel, ordner).split(os.sep)[0] == "apt":
            verzeichnisse[:] = []
            continue
        for name in sorted(dateien):
            voll = os.path.join(wurzel, name)
            if name.endswith((".html", ".htm")):
                seite = Seite()
                seite.feed(lies(voll))
                seite.close()
                seiten[voll] = seite
            elif name.endswith(".css"):
                stile.append(voll)
            elif name.endswith((".js", ".mjs")):
                funde.append((os.path.relpath(voll, ordner), "JavaScript-Datei in der Ausgabe"))

    def pruefe_ziel(quelle, adresse, was):
        rel = os.path.relpath(quelle, ordner)
        ziel = intern(adresse, basis)
        if ziel is None:
            return
        pfad, anker, relativ = ziel
        if pfad.startswith("/apt/") or pfad == "/apt":
            return
        if not pfad:
            datei = quelle
        elif relativ:
            datei = ziel_datei(ordner, "/" + os.path.relpath(
                os.path.join(os.path.dirname(quelle), unquote(pfad)), ordner).replace(os.sep, "/"))
            if pfad.endswith("/") and datei and not datei.endswith("index.html"):
                datei = None
        else:
            datei = ziel_datei(ordner, pfad)
        if datei is None or not os.path.isfile(datei):
            funde.append((rel, f"toter Verweis {was}: {adresse}"))
            return
        if anker:
            ziel_seite = seiten.get(datei)
            if ziel_seite is None:
                funde.append((rel, f"Anker in einer Datei, die keine Seite ist ({was}): {adresse}"))
            elif unquote(anker) not in ziel_seite.ids:
                funde.append((rel, f"toter Anker {was}: {adresse}"))

    for voll, seite in sorted(seiten.items()):
        rel = os.path.relpath(voll, ordner)
        in_hilfe = rel.replace(os.sep, "/").startswith("hilfe/")
        for adresse, im_main in seite.links:
            pruefe_ziel(voll, adresse, "<a href>")
            for verboten in VERBOTENE_DOKU:
                if verboten in adresse:
                    funde.append((rel, f"Link auf {verboten.split('/', 2)[2]} bei GitHub (steht auf der Webseite): {adresse}"))
            if in_hilfe and im_main and urlsplit(adresse).netloc.lower().endswith("github.com"):
                funde.append((rel, f"Link zu github.com im Text der Hilfe: {adresse}"))
        for adresse, was in seite.geladen:
            if fremd(adresse, basis):
                funde.append((rel, f"fremde Adresse für Stil, Schrift oder Bild ({was}): {adresse}"))
            else:
                pruefe_ziel(voll, adresse, was)
        for bild in seite.bilder_ohne_alt:
            funde.append((rel, f"Bild ohne alt-Text: {bild}"))
        for skript in seite.skripte:
            funde.append((rel, f"JavaScript: {skript}"))

    for voll in stile:
        rel = os.path.relpath(voll, ordner)
        text = re.sub(r"/\*.*?\*/", "", lies(voll), flags=re.S)
        if re.search(r"@import\b", text):
            funde.append((rel, "@import im Stylesheet"))
        for adresse in re.findall(r"url\(\s*['\"]?([^'\")]+)", text):
            if fremd(adresse, basis) or adresse.startswith("//"):
                funde.append((rel, f"fremde Adresse im Stylesheet: {adresse}"))

    if mit_hilfe:
        if not os.path.isfile(os.path.join(ordner, "hilfe", "index.html")):
            funde.append(("hilfe/index.html", "die Startseite der Hilfe fehlt"))
        alt = os.path.join(ordner, "bedienung", "index.html")
        seite = seiten.get(alt)
        if seite is None:
            funde.append(("bedienung/index.html", "die alte Adresse /bedienung/ fehlt (aliases in content/de/hilfe/_index.md)"))
        else:
            ziel = intern(seite.weiterleitung, basis) if seite.weiterleitung else None
            if ziel is None or ziel[0].rstrip("/") != "/hilfe":
                funde.append(("bedienung/index.html", f"leitet nicht auf /hilfe/ weiter, sondern: {seite.weiterleitung}"))
    return funde


# ---- Self-test ----------------------------------------------------------------------------------------------

def schreibe(ordner, pfad, inhalt):
    voll = os.path.join(ordner, pfad)
    os.makedirs(os.path.dirname(voll), exist_ok=True)
    with open(voll, "w", encoding="utf-8") as datei:
        datei.write(inhalt)


def mini_baum(ordner):
    """Erfundene Seite in der Form, die Hugo mit --minify schreibt (Attribute teils ohne Anführungszeichen)."""
    kopf = '<!doctype html><html lang=de><head><link rel=stylesheet href=/css/stil.css><link rel=canonical href=https://baruch.bernau.family/></head><body>'
    schreibe(ordner, "index.html", kopf + '<main id=inhalt><a href=/hilfe/>Hilfe</a> <a href=/betrieb/#sicherung>Sicherung</a>'
             ' <a href=https://github.com/beispiel/baruch>Quellcode</a> <a href=/apt/baruch-archive-keyring.gpg>Schlüssel</a>'
             ' <a href="mailto:niemand@example.org">Post</a></main></body></html>')
    schreibe(ordner, "betrieb/index.html", kopf + '<main><h2 id=sicherung>Sicherung</h2><a href=#sicherung>hier</a>'
             ' <a href=../hilfe/moewen/>relativ</a></main></body></html>')
    schreibe(ordner, "hilfe/index.html", kopf + '<main><a href=/hilfe/moewen/>Möwen</a></main>'
             '<footer><a href=https://github.com/beispiel/baruch>Quellcode</a></footer></body></html>')
    schreibe(ordner, "hilfe/moewen/index.html", kopf + '<main><h2 id=zaehlen>Zählen</h2>'
             '<picture><source media="(prefers-color-scheme: dark)" srcset=/bilder/hilfe/watt-dunkel.png>'
             '<img src=/bilder/hilfe/watt.png alt="Das Watt bei Ebbe"></picture>'
             '<a href=/hilfe/moewen/#zaehlen>Zählen</a></main></body></html>')
    schreibe(ordner, "bedienung/index.html", '<!doctype html><html lang=de><head><link rel=canonical href=https://baruch.bernau.family/hilfe/>'
             '<meta http-equiv=refresh content="0; url=https://baruch.bernau.family/hilfe/"></head></html>')
    schreibe(ordner, "css/stil.css", "body { background: url(/bilder/hilfe/watt.png); } /* url(https://fremd.example/x) im Kommentar */")
    schreibe(ordner, "bilder/hilfe/watt.png", "kein echtes Bild")
    schreibe(ordner, "bilder/hilfe/watt-dunkel.png", "kein echtes Bild")


def selbsttest():
    fehler = 0

    def zusicherung(ok, was, warum):
        nonlocal fehler
        print(f"{'ok ' if ok else 'FEHLER'}  {was}\n        Begründung: {warum}")
        if not ok:
            fehler += 1

    def stoerung(was, warum, erwartet, aendern):
        with tempfile.TemporaryDirectory(prefix="site-check-") as ordner:
            mini_baum(ordner)
            aendern(ordner)
            funde = pruefe(ordner)
            treffer = [f for f in funde if erwartet in f[1]]
            zusicherung(len(treffer) == 1 and len(funde) == 1, was,
                        warum + (f" (gemeldet: {funde})" if len(funde) != 1 or not treffer else ""))

    def anhaengen(pfad, html):
        def tun(ordner):
            voll = os.path.join(ordner, pfad)
            schreibe(ordner, pfad, lies(voll).replace("</main>", html + "</main>"))
        return tun

    with tempfile.TemporaryDirectory(prefix="site-check-") as ordner:
        mini_baum(ordner)
        funde = pruefe(ordner)
        zusicherung(funde == [], "der erfundene Mini-Baum ist fehlerfrei",
                    "Ohne diesen Normalfall bewiesen die Störungen unten nichts: Eine Prüfung, die immer meldet, "
                    f"fände auch jede Störung. Er enthält absichtlich Zulässiges: /apt/, mailto, GitHub in der "
                    f"Fußzeile der Hilfe, einen relativen Link, eine fremde Adresse nur im CSS-Kommentar. (gemeldet: {funde})")
        zusicherung(pruefe(ordner, mit_hilfe=False) == [], "--ohne-hilfe meldet am fehlerfreien Baum ebenfalls nichts",
                    "Die Option darf nur die Prüfung der Weiterleitung auslassen, nichts hinzufügen.")

    stoerung("ein toter interner Link wird gefunden",
             "Kernaufgabe: Ein Verweis auf eine Seite, die es nicht gibt, darf nicht ausgeliefert werden.",
             "toter Verweis", anhaengen("index.html", "<a href=/hilfe/robben/>Robben</a>"))
    stoerung("ein toter Anker wird gefunden",
             "Die Seiten verweisen untereinander auf feste Anker; verschwindet einer, landet der Leser am Seitenanfang.",
             "toter Anker", anhaengen("index.html", "<a href=/hilfe/moewen/#fuettern>Füttern</a>"))
    stoerung("ein toter Anker auf derselben Seite wird gefunden",
             "Inhaltsverzeichnisse und „Direkt zu“-Zeilen verweisen nur mit #…; auch diese Form muss geprüft sein.",
             "toter Anker", anhaengen("betrieb/index.html", "<a href=#umzug>Umzug</a>"))
    stoerung("ein toter relativer Link wird gefunden",
             "Relative Adressen löst die Prüfung von der Seite aus auf, nicht von der Wurzel.",
             "toter Verweis", anhaengen("betrieb/index.html", "<a href=../hilfe/robben/>Robben</a>"))
    stoerung("ein fehlendes Bild wird gefunden",
             "Der Shortcode lässt fehlende Bilder aus; ein von Hand gesetztes oder umbenanntes Bild fiele sonst erst dem Leser auf.",
             "toter Verweis", lambda o: os.remove(os.path.join(o, "bilder/hilfe/watt-dunkel.png")))
    stoerung("ein Link auf docs/Bedienung.md bei GitHub wird gefunden",
             "Seit Runde 37 ist die Webseite die Quelle der Bedienung; der Link führte auf eine Datei, die nur zurückverweist.",
             "Link auf docs/Bedienung.md",
             anhaengen("index.html", "<a href=https://github.com/beispiel/baruch/blob/main/docs/Bedienung.md>Bedienung</a>"))
    stoerung("ein Link auf docs/Betrieb.md bei GitHub wird gefunden",
             "Dasselbe für den Betrieb (Quelle: content/betrieb.md).",
             "Link auf docs/Betrieb.md",
             anhaengen("betrieb/index.html", "<a href=https://github.com/beispiel/baruch/blob/main/docs/Betrieb.md>Betrieb</a>"))
    stoerung("ein Link zu github.com im Text der Hilfe wird gefunden",
             "Die Hilfe ist für Anwender; im Text (<main>) darf sie niemanden ins Repository schicken. Kopf- und Fußzeile dürfen es.",
             "github.com im Text der Hilfe",
             anhaengen("hilfe/moewen/index.html", "<a href=https://github.com/beispiel/baruch/blob/main/docs/MCP.md>MCP</a>"))
    stoerung("ein Bild ohne alt-Text wird gefunden",
             "Ohne alt-Text erfährt, wer das Bild nicht sieht, nicht, was es zeigt; ein leeres alt zählt wie ein fehlendes.",
             "Bild ohne alt-Text", anhaengen("hilfe/moewen/index.html", '<img src=/bilder/hilfe/watt.png alt=" ">'))
    stoerung("eine fehlende Weiterleitung von /bedienung/ wird gefunden",
             "Alte Links und Lesezeichen auf /bedienung/ sollen in der Hilfe ankommen.",
             "alte Adresse /bedienung/ fehlt", lambda o: os.remove(os.path.join(o, "bedienung/index.html")))
    stoerung("eine Weiterleitung von /bedienung/ an ein falsches Ziel wird gefunden",
             "Es genügt nicht, dass die Datei da ist; sie muss auf /hilfe/ zeigen.",
             "leitet nicht auf /hilfe/ weiter",
             lambda o: schreibe(o, "bedienung/index.html", '<meta http-equiv=refresh content="0; url=https://baruch.bernau.family/betrieb/">'))
    stoerung("ein <script> wird gefunden",
             "Die Webseite verspricht in der Fußzeile, ohne JavaScript auszukommen.",
             "JavaScript: <script>", anhaengen("index.html", "<script>void 0</script>"))
    stoerung("ein Ereignis-Attribut wird gefunden",
             "JavaScript kommt auch ohne <script> auf die Seite, etwa als onclick.",
             "JavaScript: Attribut onclick", anhaengen("index.html", '<p onclick="void 0">Text</p>'))
    stoerung("ein Stylesheet von einer fremden Adresse wird gefunden",
             "Fremde Adressen für Stil, Schrift oder Bild verraten dem fremden Server jeden Besuch.",
             "fremde Adresse für Stil, Schrift oder Bild",
             anhaengen("index.html", "<link rel=stylesheet href=https://fremd.example/stil.css>"))
    stoerung("ein Bild von einer fremden Adresse wird gefunden",
             "Dasselbe für Bilder; ein Link (<a>) nach außen bleibt dagegen erlaubt.",
             "fremde Adresse für Stil, Schrift oder Bild",
             anhaengen("index.html", '<img src=https://fremd.example/bild.png alt="Fremdes Bild">'))
    stoerung("eine fremde Adresse im Stylesheet wird gefunden",
             "Schriften und Hintergründe lädt der Browser aus dem CSS; dort gilt dieselbe Regel.",
             "fremde Adresse im Stylesheet",
             lambda o: schreibe(o, "css/stil.css", "@font-face { src: url(https://fremd.example/schrift.woff2); }"))

    print("site-check.py --selbsttest: " + ("alles in Ordnung" if not fehler else f"{fehler} Zusicherung(en) verletzt"))
    return 1 if fehler else 0


def main():
    wurzel = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    eingabe = argparse.ArgumentParser(description="Prüft die gebaute Webseite (Links, Anker, alt-Texte, kein JavaScript).")
    eingabe.add_argument("ordner", nargs="?", default=os.path.join(wurzel, "site", "public"),
                         help="Ausgabe von Hugo (Vorgabe: site/public)")
    eingabe.add_argument("--basis", default=BASIS, help=f"Adresse der Seite (Vorgabe: {BASIS})")
    eingabe.add_argument("--ohne-hilfe", action="store_true",
                         help="die Weiterleitung /bedienung/ → /hilfe/ nicht verlangen (Seite ohne Abschnitt Hilfe)")
    eingabe.add_argument("--selbsttest", action="store_true", help="das Skript an einem erfundenen Mini-Baum prüfen")
    wahl = eingabe.parse_args()
    if wahl.selbsttest:
        return selbsttest()
    if not os.path.isfile(os.path.join(wahl.ordner, "index.html")):
        print(f"site-check.py: {wahl.ordner}/index.html fehlt – erst bauen: hugo --minify --panicOnWarning -s site",
              file=sys.stderr)
        return 2
    funde = pruefe(wahl.ordner, wahl.basis, not wahl.ohne_hilfe)
    for datei, meldung in funde:
        print(f"{datei}: {meldung}")
    if funde:
        print(f"site-check.py: {len(funde)} Fundstelle(n) in {wahl.ordner}", file=sys.stderr)
        return 1
    print(f"site-check.py: {wahl.ordner} ohne Fund.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
