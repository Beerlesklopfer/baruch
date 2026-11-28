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
 3. Unter /hilfe/ und /en/hilfe/ steht im <main> kein Link zu github.com: Die Hilfe ist für Anwender und muss
    ohne das Repository auskommen (beide Sprachen, Runde 39c).
 4. Jedes <img> hat einen nichtleeren alt-Text.
 5. /bedienung/index.html leitet auf /hilfe/ weiter (alte Adresse der Bedienung) und /hilfe/index.html liegt vor.
    --ohne-hilfe lässt diese eine Prüfung aus (für Seiten ohne Abschnitt Hilfe, etwa die Probe in
    scripts/ci-selftest.sh).
 6. Kein JavaScript (kein <script>, keine on…-Attribute, kein javascript:-Link) und keine fremde Adresse für
    Stil, Schrift oder Bild (src, srcset, <link rel=stylesheet|icon|preload…>, url(…) und @import in CSS).

Zwei Sprachen (Runde 39b, Vertrag §2.2): Deutsch liegt in der Wurzel, jede weitere Sprache aus SPRACHEN_UNTER in
ihrem Unterordner (en/). Geprüft wird zusätzlich:

 7. Jede Seite (außer Weiterleitungen) trägt <html lang> ihrer Sprache: en unter en/, sonst de.
 8. Bilder unter /bilder/ kommen auf englischen Seiten aus /bilder/en/, auf deutschen nie aus /bilder/en/.
 9. Der Sprachumschalter (<a hreflang=…>) führt auf eine vorhandene Seite, deren <html lang> sein hreflang ist, und
    nie auf die eigene Sprache. Liegt eine zweite Sprache vor (en/index.html), hat jede Seite einen Umschalter.
10. Mit Hilfe (ohne --ohne-hilfe) hat jede weitere Sprache ihre Startseite der Hilfe (en/hilfe/index.html), aber
    keine Weiterleitung en/bedienung/: Die alte Adresse /bedienung/ gibt es nur auf Deutsch.

Entwicklerdokumentation (Runde 39c, /en/docs/, erzeugt von site/tools/docs_pages.py aus docs/*.md; Runde 39d: auch
Deutsch unter /docs/, aus docs/de/); ihre Links und Anker prüft schon Punkt 1, die Sprachregeln 7 und 9 gelten für
beide Abschnitte (der Umschalter einer Seite unter /docs/ führt auf dieselbe Seite unter /en/docs/ und zurück). Zusätzlich:

11. Keine Seite verlinkt in die Ausgabe des Generators (Pfad mit site/generated, site/generated-de, /generated/ oder
    /generated-de/): Sie ist nicht eingecheckt, ein solcher Link führte auf GitHub ins Leere.
12. Unter /en/docs/ und /docs/ zeigt kein Link auf eine der neun Quellen bei GitHub (blob/main/docs/<Quelle>.md, auch
    docs/de/<Quelle>.md): Diese Dateien stehen selbst im Abschnitt; der Generator schreibt solche Links auf
    /en/docs/<Seite>/ bzw. /docs/<Seite>/ um.

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
# Language of the root and languages with a folder of their own (Hugo: defaultContentLanguageInSubdir = false).
SPRACHE_WURZEL = "de"
SPRACHEN_UNTER = ("en",)
# Files in the repository whose content has stood on the website since Round 37.
VERBOTENE_DOKU = ("blob/main/docs/Bedienung.md", "blob/main/docs/Betrieb.md")
# The nine sources of the developer documentation (site/tools/docs_pages.py, DOCS); under /en/docs/ a link to them on
# GitHub instead of the site is a finding.
DOKU_QUELLEN = ("Bedienung.md", "Architektur.md", "RPC.md", "MCP.md", "Austauschformat.md", "WYSIWYG.md", "Betrieb.md",
                "Verteilung.md", "Plan.md")
# The sections of the developer documentation: German in the root (docs/), the other languages below their folder.
DOKU_ABSCHNITT = ("docs/",) + tuple(f"{s}/docs/" for s in SPRACHEN_UNTER)
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
        self.sprache = None   # lang of <html>; "" = <html> without lang
        self.umschalter = []  # (address, hreflang) of every <a hreflang> – the language switch

    def handle_starttag(self, tag, attrs):
        a = {k: (v or "") for k, v in attrs}
        if tag == "main":
            self.im_main += 1
        if tag == "html" and self.sprache is None:
            self.sprache = a.get("lang", "").strip()
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
        if tag == "a" and "href" in a and a.get("hreflang", "").strip():
            self.umschalter.append((a["href"], a["hreflang"].strip()))
        elif tag == "a" and "href" in a:
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


def sprache_von(rel):
    """Sprache einer Datei nach ihrem Pfad relativ zum Ausgabeordner: der Unterordner aus SPRACHEN_UNTER, sonst die
    Sprache der Wurzel."""
    erster = rel.replace(os.sep, "/").split("/")[0]
    return erster if erster in SPRACHEN_UNTER else SPRACHE_WURZEL


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

    def aufloesen(quelle, adresse):
        """(Datei, Anker) einer internen Adresse, ("", "") für extern oder /apt/; Datei None = nicht vorhanden."""
        ziel = intern(adresse, basis)
        if ziel is None:
            return "", ""
        pfad, anker, relativ = ziel
        if pfad.startswith("/apt/") or pfad == "/apt":
            return "", ""
        if not pfad:
            datei = quelle
        elif relativ:
            datei = ziel_datei(ordner, "/" + os.path.relpath(
                os.path.join(os.path.dirname(quelle), unquote(pfad)), ordner).replace(os.sep, "/"))
            if pfad.endswith("/") and datei and not datei.endswith("index.html"):
                datei = None
        else:
            datei = ziel_datei(ordner, pfad)
        if datei is not None and not os.path.isfile(datei):
            datei = None
        return datei, anker

    def pruefe_ziel(quelle, adresse, was):
        rel = os.path.relpath(quelle, ordner)
        datei, anker = aufloesen(quelle, adresse)
        if datei == "":
            return
        if datei is None:
            funde.append((rel, f"toter Verweis {was}: {adresse}"))
            return
        if anker:
            ziel_seite = seiten.get(datei)
            if ziel_seite is None:
                funde.append((rel, f"Anker in einer Datei, die keine Seite ist ({was}): {adresse}"))
            elif unquote(anker) not in ziel_seite.ids:
                funde.append((rel, f"toter Anker {was}: {adresse}"))

    zweisprachig = any(os.path.isfile(os.path.join(ordner, sprache, "index.html")) for sprache in SPRACHEN_UNTER)
    for voll, seite in sorted(seiten.items()):
        rel = os.path.relpath(voll, ordner)
        sprache = sprache_von(rel)
        in_hilfe = rel.replace(os.sep, "/").startswith(("hilfe/",) + tuple(f"{s}/hilfe/" for s in SPRACHEN_UNTER))
        in_doku = rel.replace(os.sep, "/").startswith(DOKU_ABSCHNITT)
        if seite.weiterleitung is None:
            if seite.sprache != sprache:
                funde.append((rel, f"<html lang> ist „{seite.sprache or '(fehlt)'}“, die Seite gehört zur Sprache "
                                   f"„{sprache}“"))
            if zweisprachig and not seite.umschalter:
                funde.append((rel, "kein Sprachumschalter (<a hreflang>), obwohl die Seite zwei Sprachen hat"))
        for adresse, hreflang in seite.umschalter:
            datei, _ = aufloesen(voll, adresse)
            if datei is None:
                funde.append((rel, f"Sprachumschalter zeigt ins Leere: {adresse}"))
            elif datei and hreflang == sprache:
                funde.append((rel, f"Sprachumschalter führt in die eigene Sprache ({hreflang}): {adresse}"))
            elif datei and seiten.get(datei) is not None and seiten[datei].sprache != hreflang:
                funde.append((rel, f"Sprachumschalter hreflang={hreflang} führt auf eine Seite mit "
                                   f"<html lang> „{seiten[datei].sprache}“: {adresse}"))
        for adresse, was in seite.geladen:
            ziel = intern(adresse, basis)
            if ziel is None or not ziel[0].startswith("/bilder/"):
                continue
            aus_unterordner = next((s for s in SPRACHEN_UNTER if ziel[0].startswith(f"/bilder/{s}/")), None)
            if sprache != SPRACHE_WURZEL and aus_unterordner != sprache:
                funde.append((rel, f"Bild nicht aus /bilder/{sprache}/ auf einer Seite der Sprache {sprache} "
                                   f"({was}): {adresse}"))
            elif sprache == SPRACHE_WURZEL and aus_unterordner:
                funde.append((rel, f"Bild aus /bilder/{aus_unterordner}/ auf einer Seite der Sprache {sprache} "
                                   f"({was}): {adresse}"))
        for adresse, im_main in seite.links:
            pruefe_ziel(voll, adresse, "<a href>")
            for verboten in VERBOTENE_DOKU:
                if verboten in adresse:
                    funde.append((rel, f"Link auf {verboten.split('/', 2)[2]} bei GitHub (steht auf der Webseite): {adresse}"))
            if in_hilfe and im_main and urlsplit(adresse).netloc.lower().endswith("github.com"):
                funde.append((rel, f"Link zu github.com im Text der Hilfe: {adresse}"))
            if "site/generated" in adresse or any(f"/{g}/" in urlsplit(adresse).path for g in ("generated", "generated-de")):
                funde.append((rel, f"Link in die Ausgabe des Generators (nicht eingecheckt): {adresse}"))
            if in_doku:
                for quelle in DOKU_QUELLEN:
                    pfad = urlsplit(adresse).path
                    if pfad.endswith(f"/blob/main/docs/{quelle}") or pfad.endswith(f"/blob/main/docs/de/{quelle}"):
                        funde.append((rel, f"Link auf docs/{quelle} bei GitHub statt auf die Seite des Abschnitts "
                                           f"(/docs/ oder /en/docs/): {adresse}"))
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
        for sprache in SPRACHEN_UNTER:
            if not os.path.isfile(os.path.join(ordner, sprache, "index.html")):
                continue
            if not os.path.isfile(os.path.join(ordner, sprache, "hilfe", "index.html")):
                funde.append((f"{sprache}/hilfe/index.html", f"die Startseite der Hilfe fehlt für die Sprache {sprache}"))
            if os.path.exists(os.path.join(ordner, sprache, "bedienung")):
                funde.append((f"{sprache}/bedienung/index.html", "die alte Adresse /bedienung/ gibt es nur auf Deutsch "
                                                                 "(keine aliases in englischen Seiten)"))
    return funde


# ---- Self-test ----------------------------------------------------------------------------------------------

def schreibe(ordner, pfad, inhalt):
    voll = os.path.join(ordner, pfad)
    os.makedirs(os.path.dirname(voll), exist_ok=True)
    with open(voll, "w", encoding="utf-8") as datei:
        datei.write(inhalt)


def mini_baum(ordner, zweisprachig=False):
    """Erfundene Seite in der Form, die Hugo mit --minify schreibt (Attribute teils ohne Anführungszeichen).
    zweisprachig: dazu englische Seiten unter en/ mit Bildern aus /bilder/en/ und auf jeder Seite ein
    Sprachumschalter (<a hreflang>) auf die Übersetzung bzw. die Startseite der anderen Sprache."""
    def kopf_mit(sprache, umschalter):
        anderer = "en" if sprache == "de" else "de"
        knopf = (f'<a href={umschalter} hreflang={anderer} lang={anderer}>{"English" if anderer == "en" else "Deutsch"}</a>'
                 if zweisprachig else "")
        return (f'<!doctype html><html lang={sprache}><head><link rel=stylesheet href=/css/stil.css>'
                f'<link rel=canonical href=https://baruch.bernau.family/></head><body><header>{knopf}</header>')

    kopf = kopf_mit("de", "/en/")
    schreibe(ordner, "index.html", kopf + '<main id=inhalt><a href=/hilfe/>Hilfe</a> <a href=/betrieb/#sicherung>Sicherung</a>'
             ' <a href=https://github.com/beispiel/baruch>Quellcode</a> <a href=/apt/baruch-archive-keyring.gpg>Schlüssel</a>'
             ' <a href="mailto:niemand@example.org">Post</a></main></body></html>')
    schreibe(ordner, "betrieb/index.html", kopf_mit("de", "/en/") + '<main><h2 id=sicherung>Sicherung</h2><a href=#sicherung>hier</a>'
             ' <a href=../hilfe/moewen/>relativ</a></main></body></html>')
    schreibe(ordner, "hilfe/index.html", kopf_mit("de", "/en/hilfe/") + '<main><a href=/hilfe/moewen/>Möwen</a></main>'
             '<footer><a href=https://github.com/beispiel/baruch>Quellcode</a></footer></body></html>')
    schreibe(ordner, "hilfe/moewen/index.html", kopf_mit("de", "/en/hilfe/moewen/") + '<main><h2 id=zaehlen>Zählen</h2>'
             '<picture><source media="(prefers-color-scheme: dark)" srcset=/bilder/hilfe/watt-dunkel.png>'
             '<img src=/bilder/hilfe/watt.png alt="Das Watt bei Ebbe"></picture>'
             '<a href=/hilfe/moewen/#zaehlen>Zählen</a></main></body></html>')
    schreibe(ordner, "bedienung/index.html", '<!doctype html><html lang=de><head><link rel=canonical href=https://baruch.bernau.family/hilfe/>'
             '<meta http-equiv=refresh content="0; url=https://baruch.bernau.family/hilfe/"></head></html>')
    schreibe(ordner, "css/stil.css", "body { background: url(/bilder/hilfe/watt.png); } /* url(https://fremd.example/x) im Kommentar */")
    schreibe(ordner, "bilder/hilfe/watt.png", "kein echtes Bild")
    schreibe(ordner, "bilder/hilfe/watt-dunkel.png", "kein echtes Bild")
    if not zweisprachig:
        return
    schreibe(ordner, "en/index.html", kopf_mit("en", "/") + '<main id=inhalt><a href=/en/hilfe/>Help</a></main></body></html>')
    schreibe(ordner, "en/hilfe/index.html", kopf_mit("en", "/hilfe/") + '<main><a href=/en/hilfe/moewen/>Gulls</a></main>'
             '</body></html>')
    schreibe(ordner, "en/hilfe/moewen/index.html", kopf_mit("en", "/hilfe/moewen/") + '<main><h2 id=zaehlen>Counting</h2>'
             '<picture><source media="(prefers-color-scheme: dark)" srcset=/bilder/en/hilfe/watt-dunkel.png>'
             '<img src=/bilder/en/hilfe/watt.png alt="The mudflats at low tide"></picture>'
             '<a href=/en/hilfe/moewen/#zaehlen>Counting</a></main></body></html>')
    schreibe(ordner, "bilder/en/hilfe/watt.png", "kein echtes Bild")
    schreibe(ordner, "bilder/en/hilfe/watt-dunkel.png", "kein echtes Bild")
    # Developer documentation (Round 39c English, Round 39d German): the same slugs in both languages, so the switch
    # leads to the same page; links between the pages with anchors, a link into the repository (allowed outside the
    # help).
    schreibe(ordner, "en/docs/index.html", kopf_mit("en", "/docs/") + '<main><a href=/en/docs/rpc/>RPC</a>'
             ' <a href=/en/docs/rpc/#work-create>work.create</a></main></body></html>')
    schreibe(ordner, "en/docs/rpc/index.html", kopf_mit("en", "/docs/rpc/") + '<main><h2 id=work-create>work.create</h2>'
             '<a href=https://github.com/beispiel/baruch/blob/main/crates/baruch-server/src/rpc.rs>rpc.rs</a>'
             ' <a href=/en/docs/>Documentation</a></main></body></html>')
    schreibe(ordner, "docs/index.html", kopf_mit("de", "/en/docs/") + '<main><a href=/docs/rpc/>RPC</a>'
             ' <a href=/docs/rpc/#work-create>work.create</a></main></body></html>')
    schreibe(ordner, "docs/rpc/index.html", kopf_mit("de", "/en/docs/rpc/") + '<main><h2 id=work-create>work.create</h2>'
             '<a href=https://github.com/beispiel/baruch/blob/main/crates/baruch-server/src/rpc.rs>rpc.rs</a>'
             ' <a href=/docs/>Dokumentation</a></main></body></html>')


def selbsttest():
    fehler = 0

    def zusicherung(ok, was, warum):
        nonlocal fehler
        print(f"{'ok ' if ok else 'FEHLER'}  {was}\n        Begründung: {warum}")
        if not ok:
            fehler += 1

    def stoerung(was, warum, erwartet, aendern, zweisprachig=False):
        with tempfile.TemporaryDirectory(prefix="site-check-") as ordner:
            mini_baum(ordner, zweisprachig)
            aendern(ordner)
            funde = pruefe(ordner)
            # One finding per expected text (a tuple when one fault rightly shows in two places), nothing else.
            alle = (erwartet,) if isinstance(erwartet, str) else erwartet
            passend = all(len([f for f in funde if text in f[1]]) == 1 for text in alle) and len(funde) == len(alle)
            zusicherung(passend, was, warum + ("" if passend else f" (gemeldet: {funde})"))

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
    with tempfile.TemporaryDirectory(prefix="site-check-") as ordner:
        mini_baum(ordner, zweisprachig=True)
        funde = pruefe(ordner)
        zusicherung(funde == [], "der zweisprachige Mini-Baum (de in der Wurzel, en unter en/) ist fehlerfrei",
                    "Normalfall für die Regeln der zweiten Sprache: <html lang> je Sprache, Bilder aus /bilder/en/, "
                    "Umschalter auf jeder Seite, auch einer ohne Übersetzung (betrieb → /en/). Ohne ihn bewiesen die "
                    f"Störungen unten nichts. (gemeldet: {funde})")

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

    def ersetzen(pfad, alt, neu):
        def tun(ordner):
            voll = os.path.join(ordner, pfad)
            inhalt = lies(voll)
            assert alt in inhalt, (pfad, alt)
            schreibe(ordner, pfad, inhalt.replace(alt, neu, 1))
        return tun

    stoerung("eine englische Seite mit <html lang=de> wird gefunden (auch am Umschalter der deutschen Seite)",
             "Vorleser und Suchmaschinen lesen die Sprache aus <html lang>; eine englische Seite mit deutschem lang "
             "würde mit deutscher Aussprache vorgelesen. Der Umschalter der deutschen Hilfe, der auf sie zeigt, "
             "meldet denselben Fehler von seiner Seite aus.",
             ("<html lang> ist „de“", "Sprachumschalter hreflang=en führt auf eine Seite mit <html lang> „de“"),
             ersetzen("en/hilfe/index.html", "<html lang=en>", "<html lang=de>"), True)
    stoerung("eine deutsche Seite ohne <html lang> wird gefunden",
             "Auch die Wurzel (Deutsch) muss ihre Sprache nennen; ein fehlendes lang zählt wie ein falsches.",
             "<html lang> ist „(fehlt)“", ersetzen("betrieb/index.html", "<html lang=de>", "<html>"))
    stoerung("ein deutsches Bild auf einer englischen Seite wird gefunden",
             "Englische Seiten zeigen die englische Oberfläche (/bilder/en/); ein deutsches Bild dort hieße, dass der "
             "Shortcode den Sprachordner nicht gewählt hat.",
             "Bild nicht aus /bilder/en/",
             ersetzen("en/hilfe/moewen/index.html", "<img src=/bilder/en/hilfe/watt.png", "<img src=/bilder/hilfe/watt.png"),
             True)
    stoerung("ein englisches Bild auf einer deutschen Seite wird gefunden",
             "Gegenrichtung: Die deutschen Seiten behalten ihre Bilder unter /bilder/ ohne Sprachordner.",
             "Bild aus /bilder/en/",
             ersetzen("hilfe/moewen/index.html", "<img src=/bilder/hilfe/watt.png", "<img src=/bilder/en/hilfe/watt.png"),
             True)
    stoerung("eine Seite ohne Sprachumschalter wird gefunden, sobald es zwei Sprachen gibt",
             "Von jeder Seite muss man in die andere Sprache wechseln können (Vertrag §2.2); fehlt der Umschalter im "
             "Kopf einer Vorlage, fällt das sonst niemandem auf.",
             "kein Sprachumschalter",
             ersetzen("betrieb/index.html", "<a href=/en/ hreflang=en lang=en>English</a>", ""), True)
    stoerung("ein Sprachumschalter ins Leere wird gefunden",
             "Der Umschalter führt auf die Übersetzung der Seite; gibt es sie nicht, muss er auf die Startseite der "
             "anderen Sprache zeigen, nie auf eine Seite, die es nicht gibt.",
             "Sprachumschalter zeigt ins Leere",
             ersetzen("hilfe/moewen/index.html", "href=/en/hilfe/moewen/ hreflang", "href=/en/hilfe/robben/ hreflang"), True)
    stoerung("ein Sprachumschalter, dessen hreflang nicht zur Zielseite passt, wird gefunden",
             "hreflang verspricht die Sprache des Ziels; eine Seite in der eigenen Sprache als „English“ anzubieten, "
             "führte im Kreis.",
             "führt auf eine Seite mit <html lang> „de“",
             ersetzen("hilfe/moewen/index.html", "href=/en/hilfe/moewen/ hreflang", "href=/hilfe/ hreflang"), True)
    stoerung("ein Sprachumschalter in die eigene Sprache wird gefunden",
             "Der Umschalter einer englischen Seite muss nach Deutsch führen, nicht auf eine andere englische Seite.",
             "Sprachumschalter führt in die eigene Sprache",
             ersetzen("en/index.html", "<a href=/ hreflang=de lang=de>", "<a href=/en/hilfe/ hreflang=en lang=en>"), True)
    stoerung("eine Weiterleitung /en/bedienung/ wird gefunden",
             "Die alte Adresse /bedienung/ gibt es nur auf Deutsch; englische Seiten tragen keine aliases (Vertrag §2.2).",
             "gibt es nur auf Deutsch",
             lambda o: schreibe(o, "en/bedienung/index.html", '<!doctype html><html lang=en><head>'
                                '<meta http-equiv=refresh content="0; url=https://baruch.bernau.family/en/hilfe/"></head></html>'),
             True)
    stoerung("eine fehlende Startseite der englischen Hilfe wird gefunden",
             "Die Hilfe gibt es in beiden Sprachen; die Startseite /en/hilfe/ ist das Ziel des Hilfe-Links der "
             "englischen Oberfläche.",
             "die Startseite der Hilfe fehlt für die Sprache en",
             lambda o: [os.remove(os.path.join(o, "en/hilfe/index.html")),
                        schreibe(o, "en/index.html", lies(os.path.join(o, "en/index.html")).replace(
                            "<a href=/en/hilfe/>Help</a>", "")),
                        schreibe(o, "hilfe/index.html", lies(os.path.join(o, "hilfe/index.html")).replace(
                            "href=/en/hilfe/ hreflang", "href=/en/ hreflang"))],
             True)

    stoerung("ein Link zu github.com im Text der englischen Hilfe wird gefunden",
             "Die Regel gilt für die Hilfe in beiden Sprachen (Runde 39c, Backlog „Open – shortcodes“); auch die "
             "englischen Anwender sollen nicht ins Repository geschickt werden.",
             "github.com im Text der Hilfe",
             anhaengen("en/hilfe/moewen/index.html", "<a href=https://github.com/beispiel/baruch/blob/main/docs/MCP.md>MCP</a>"),
             True)
    stoerung("ein toter Anker in der Entwicklerdokumentation wird gefunden",
             "Die Seiten unter /en/docs/ verweisen mit Ankern aufeinander (vom Generator umgeschrieben); ein Anker, den "
             "es auf der erzeugten Seite nicht gibt, muss auffallen.",
             "toter Anker", anhaengen("en/docs/index.html", "<a href=/en/docs/rpc/#work-delete>work.delete</a>"), True)
    stoerung("ein Link in die Ausgabe des Generators wird gefunden",
             "site/generated ist nicht eingecheckt (.gitignore); ein Link dorthin führte auf GitHub ins Leere.",
             "Ausgabe des Generators",
             anhaengen("en/docs/rpc/index.html",
                       "<a href=https://github.com/beispiel/baruch/blob/main/site/generated/docs/rpc.md>Quelle</a>"), True)
    stoerung("ein Link auf eine der neun Quellen bei GitHub unter /en/docs/ wird gefunden",
             "Die Quelle steht selbst im Abschnitt; der Generator schreibt solche Links auf die Seite um, ein Link nach "
             "GitHub hieße, dass das Umschreiben versagt hat.",
             "Link auf docs/RPC.md bei GitHub statt",
             anhaengen("en/docs/index.html", "<a href=https://github.com/beispiel/baruch/blob/main/docs/RPC.md>RPC</a>"), True)

    stoerung("ein toter Anker in der deutschen Entwicklerdokumentation wird gefunden",
             "Die Seiten unter /docs/ verweisen mit denselben englischen Ankern aufeinander wie die unter /en/docs/ (vom "
             "Generator geprüft); ein Anker, den es auf der erzeugten Seite nicht gibt, muss auch hier auffallen.",
             "toter Anker", anhaengen("docs/index.html", "<a href=/docs/rpc/#work-delete>work.delete</a>"), True)
    stoerung("ein Link in die Ausgabe des deutschen Generators wird gefunden",
             "site/generated-de ist nicht eingecheckt (.gitignore); ein Link dorthin führte auf GitHub ins Leere.",
             "Ausgabe des Generators",
             anhaengen("docs/rpc/index.html",
                       "<a href=https://github.com/beispiel/baruch/blob/main/site/generated-de/docs/rpc.md>Quelle</a>"), True)
    stoerung("ein Link auf eine der neun Quellen bei GitHub unter /docs/ wird gefunden",
             "Die Übersetzung steht selbst im Abschnitt; der Generator schreibt solche Links auf die Seite um, ein Link "
             "nach GitHub hieße, dass das Umschreiben versagt hat.",
             "Link auf docs/RPC.md bei GitHub statt",
             anhaengen("docs/index.html", "<a href=https://github.com/beispiel/baruch/blob/main/docs/de/RPC.md>RPC</a>"), True)
    stoerung("ein Link auf das englische Original bei GitHub unter /docs/ wird gefunden",
             "Auch ein Link auf die englische Quelle aus dem deutschen Abschnitt zeigt, dass der Generator ihn nicht auf "
             "die deutsche Seite umgeschrieben hat.",
             "Link auf docs/MCP.md bei GitHub statt",
             anhaengen("docs/rpc/index.html", "<a href=https://github.com/beispiel/baruch/blob/main/docs/MCP.md>MCP</a>"), True)
    stoerung("ein Sprachumschalter der deutschen Dokumentation, der ins Leere führt, wird gefunden",
             "Der Schalter muss auf die gleiche Seite unter /en/docs/ führen (gleiche Namen der Adressen in beiden "
             "Sprachen); fehlt sie, landete der Leser auf einer 404-Seite.",
             "Sprachumschalter zeigt ins Leere",
             lambda o: schreibe(o, "docs/rpc/index.html", lies(os.path.join(o, "docs/rpc/index.html")).replace(
                 "href=/en/docs/rpc/ hreflang", "href=/en/docs/gibt-es-nicht/ hreflang")), True)
    stoerung("die deutsche Dokumentation mit <html lang=en> wird gefunden",
             "Der Abschnitt /docs/ ist deutsch; ein falsches lang-Attribut verfälschte Silbentrennung und Vorlesen. "
             "Der Fehler zeigt sich zweimal: an der Seite selbst und am Umschalter ihrer englischen Übersetzung.",
             ("<html lang> ist", "führt auf eine Seite mit <html lang>"),
             lambda o: schreibe(o, "docs/index.html", lies(os.path.join(o, "docs/index.html")).replace(
                 "<html lang=de>", "<html lang=en>")), True)

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
