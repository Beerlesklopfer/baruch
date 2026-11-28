#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Bildschirmfotos für die Webseite: die Bilder der Hilfe (site/static/bilder/hilfe/<name>.png, Liste in
site/data/hilfebilder.toml) und die sieben Bilder der Seite „Funktionen“ (site/static/bilder/*.png) – alle aus
einem frei erfundenen Werk („Ein Jahr auf Mövenoog“, site/tools/beispielwerk.typ) in einem Wegwerf-Datenordner.

Zwei Sprachen (Runde 39b): --lang de (Vorgabe) bzw. --lang en wählt die Sprache der Oberfläche (Cookie baruch_lang,
Accept-Language, in Firefox intl.accept_languages), die Werksprache des Beispielwerks (RPC work.language_set) und
die erfundenen Daten (für en site/tools/beispielwerk.en.typ „A Year on Gull Holm“ und englische Beiträge, Einfälle,
Quellen). Ziel: de site/static/bilder (Hilfe in hilfe/), en site/static/bilder/en (Hilfe in hilfe/).

Ablauf (ausführlich in site/tools/README.md):
  1. Wegwerf-Datenordner anlegen und beispielwerk.typ importieren (baruch-server import).
  2. In der Datenbank die Verbindungen `eliezer` und `multigpt` auf http://127.0.0.1:9 stellen – erst danach
     startet der Server (sonst bettete er beim Start über die Vorgabe-Adresse ein; SKILLS.md, „Wegwerf-Server“).
  3. Eigenen Server starten: serve --listen 127.0.0.1:0 --data <Ordner> --secrets memory, ohne BARUCH_*-Variablen.
  4. Stufe „offen“ (noch ohne Konten): Beispielwerk über RPC, MCP und Formulare des eigenen Servers ausbauen
     (zweite Fassung, Bilder, Vorschlag, Beiträge, Einfälle, Glossar, Register, Buchangaben, Antrag, Vorschauen des
     Einlesens …), dann die Bilder dieser Stufe prüfen bzw. fotografieren.
  5. Stufe „konten“: Autorin und Lektor anlegen, Werk freigeben, als Lektor Befunde und einen Vorschlag schreiben,
     dann die Bilder dieser Stufe (jede Seite verlangt jetzt eine Anmeldung; das Sitzungs-Cookie ergänzt der
     Vermittler).
  6. Den eigenen Server über seine PID beenden, den Wegwerf-Ordner löschen.

Jede Seite läuft über einen Vermittler im Skript (http.server auf 127.0.0.1:0 vor dem Wegwerf-Server): Er ergänzt
Cookies, öffnet <details>, fügt ein Klick-Skript ein und liefert gespeicherte POST-Antworten aus – `firefox
--screenshot` kann nichts davon selbst.

Fasst nichts außerhalb des Wegwerf-Ordners an: nicht den Dienst des Nutzers (127.0.0.1:8765), nicht
~/.local/share/baruch, nicht den Schlüsselbund. Kein Netz außer 127.0.0.1, keine Anfrage an ein Modell.
Firefox startet nur mit --fotos.

Nur Python 3.11+ (Standardbibliothek, tomllib). Aufruf aus dem Hauptordner des Repositorys:

    site/tools/bildschirmfotos.py --liste
    site/tools/bildschirmfotos.py --abgleich
    site/tools/bildschirmfotos.py --abgleich --lang en
    site/tools/bildschirmfotos.py --pruefen --temp ~/tmp
    site/tools/bildschirmfotos.py --fotos --temp /var/tmp/baruch-hilfe   # nur der Integrator; nie parallel zu
                                                                         # Testläufen; Pfad ohne Benutzernamen
"""

import argparse
import base64
import html
import http.client
import http.server
import io
import json
import os
import re
import secrets
import shutil
import signal
import sqlite3
import struct
import subprocess
import sys
import tempfile
import threading
import time
import tomllib
import urllib.parse
import zipfile
import zlib
from pathlib import Path

TOOLS = Path(__file__).resolve().parent
REPO = TOOLS.parent.parent
LIST_FILE = REPO / "site" / "data" / "hilfebilder.toml"
# Languages of the website and of the pictures (Round 39b, contract §2.2): German is the default without a folder of
# its own, every other language gets bilder/<lang>/ and content/<lang>/.
LANGS = ("de", "en")
DEFAULT_LANG = "de"
TARGET = REPO / "site" / "static" / "bilder"
HELP_DIR = "hilfe"
# Cookie of the interface language (crates/baruch-server/src/i18n.rs, COOKIE).
LANG_COOKIE = "baruch_lang"
# The images of one language stay below this limit (contract Round 37, §8; per language since Round 39b: German =
# site/static/bilder without the folders of the other languages, English = site/static/bilder/en).
SIZE_LIMIT = 20 * 1024 * 1024


def content_dir(lang):
    """Folder of the help pages of a language (site/content/<lang>)."""
    return REPO / "site" / "content" / lang


def target_dir(lang):
    """Image folder of a language: German site/static/bilder, every other language site/static/bilder/<lang>
    (the same rule as the partial site/themes/baruch/layouts/_partials/bildpfad.html)."""
    return TARGET if lang == DEFAULT_LANG else TARGET / lang


def language_files(lang):
    """All files of the image folder of a language; German leaves out the folders of the other languages."""
    folder = target_dir(lang)
    if not folder.is_dir():
        return []
    others = [folder / other for other in LANGS if other != DEFAULT_LANG] if lang == DEFAULT_LANG else []
    return [path for path in folder.rglob("*")
            if path.is_file() and not any(other in path.parents for other in others)]
WINDOW = "1400x860"
# Below 1600 px the laptop mode („automatisch“) draws the ribbon compact (COMPACT_BELOW_PX in
# crates/baruch-server/src/laptop.rs); the script of the page measures the width.
LAPTOP_WINDOW = "1366x768"
# The connections of the throwaway server point to the discard port: nobody answers there.
NOWHERE = "http://127.0.0.1:9"
# Address and data directory of the service at the user: never touch.
FORBIDDEN_PORT = 8765
FORBIDDEN_DATA = Path.home() / ".local" / "share" / "baruch"

# Cookies that every image gets as long as its entry says nothing else. 1400 px lie below the threshold for
# the compact ribbon; so that the images show the full ribbon, the laptop mode is set to „Aus“.
DEFAULT_COOKIES = {"baruch_laptop": "off"}
# Cookies of the user interface that the mediator manages: what the browser sends for them does not count.
MANAGED_COOKIES = (
    "baruch_work", "baruch_laptop", "baruch_theme", "baruch_accordion", "baruch_editor", "baruch_band",
    "baruch_bandx", "baruch_pv", "baruch_folded", "baruch_closed", "baruch_left", "baruch_right", "baruch_lrail",
    "baruch_rrail", "baruch_session", LANG_COOKIE,
)
SESSION_COOKIE = "baruch_session"

# Invented people (contract §4): the author of the sample work and a Lektor (names are the same in both languages).
AUTHOR = {"name": "mara", "display": "Mara Tiedemann"}
LEKTOR = {"name": "jonte", "display": "Jonte Harksen"}

# The seven images of the page „Funktionen“ (site/static/bilder/*.png): names and addresses as since Round 24/33.
FUNCTION_PICTURES = [
    {"name": "arbeitsplatz", "adresse": "/?work={work}&key={key}&laptop=off",
     "zeigt": "Arbeitsplatz: volles Menüband (Reiter Start, Laptop-Modus „Aus“), Gliederung, Editor im Satzbild, "
              "rechte Spalte"},
    {"name": "gliederung", "adresse": "/gliederung?work={work}", "zeigt": "Seite Gliederung"},
    {"name": "fassungen", "adresse": "/?work={work}&key={key}&rev=1",
     "zeigt": "Abschnitt mit zwei Fassungen, gezeigt ist die ältere"},
    {"name": "assistenten", "adresse": "/einstellungen/assistenten?work={work}",
     "zeigt": "Einstellungen → Assistenten"},
    {"name": "satz", "adresse": "/werke/{work}/satz", "zeigt": "Satz-Vorschau des Werks, Menüband im Reiter Buch"},
    {"name": "einstellungen", "adresse": "/einstellungen/werk?work={work}",
     "zeigt": "Einstellungen → Werk mit dem Block Satz"},
    {"name": "arbeitsplatz-laptop", "adresse": "/?work={work}&key={key}", "fenster": LAPTOP_WINDOW,
     "schritt": ["cookie:baruch_laptop="],
     "zeigt": "Arbeitsplatz auf Laptop-Breite: Menüband kompakt (Laptop-Modus „automatisch“)"},
]


def fail(message):
    sys.exit("Fehler: " + message)


# ---------------------------------------------------------------------------------------------------------------
# Image list
# ---------------------------------------------------------------------------------------------------------------

STEP_KINDS = ("cookie", "offen", "klick", "sitzung")
KNOWN_NEEDS = ("klick", "ohne-js", "konten", "sword")

# Texts of `erwartet` for the English pictures (contract §2.5 allows no field erwartet_en): German text -> English
# text. Words of the sample work follow beispielwerk.en.typ and SAMPLES["en"]; words of the interface are the values
# of crates/baruch-server/locales/en/*.ftl for the key whose German value the list names. A German text that is
# neither here nor in EXPECTED_NEUTRAL stops the tool for --lang en (no silent check without evidence).
EXPECTED_EN = {
    "Ein Jahr auf Mövenoog": "A Year on Gull Holm",
    "Der erste Morgen": "The First Morning",
    "Mara Tiedemann": "Mara Tiedemann",
    "Notizen vom Festland": "Notes from the Mainland",
    "Hilfe mit Bildern": "Help with pictures",
    "faehre.png": "ferry.png",
    "Ankunft": "Arrival",
    "3 offen": "3 open",
    "4 offen": "4 open",
    # Help address of the English interface (contract §2.3, HELP_URL_EN in crates/baruch-server/src/about.rs).
    "https://baruch.bernau.family/hilfe/": "https://baruch.bernau.family/en/hilfe/",
    ">Hilfe</a>": ">Help</a>",
    "Die Nacht ohne Lampe": "The Night Without the Lamp",
    "Abschied": "Farewell",
    "Abschied von der Hallig": "Farewell to the Island",
    # A word of its own: the index mark on "oilcloth" splits "oilcloth cover" in the HTML of the typeset view.
    "Wachstuchdeckel": "oilcloth",
    "2 Fassungen dieses Abschnitts": "Revisions of this section",
    "Verlauf von Fassung 2": "History of revision 2",
    "Ältere Fassungen ausblenden": "Hide older",
    "leuchtturm.png": "lighthouse.png",
    "umschlag.png": "cover.png",
    "Ziel 5:00": "target 5:00",
    "Feder": "feather",
    "Reihenfolge der Kapitel": "Order of the chapters",
    "@mcp:schreibhilfe": "@mcp:scribe",
    "Jahreszahlen abgleichen": "check the years",
    "Er hatte recht.": "He was right.",
    "Belege zu Beitrag": "Evidence for post",
    "Leuchtfeuer": "Beacon",
    "Sturmflut": "storm surge",
    "Die Feder auf dem Sims": "The feather on the sill",
    "Docht und Petroleum": "Wick and paraffin",
    "Bibeln (SWORD)": "Bibles (SWORD)",
    "Ps 119,105": "119",
    "Dienstbuch": "Service Book",
    "Dienstbuch für die Leuchtfeuerwärter der Halligen": "Service Book for the Keepers of the Island Lights",
    "Was das Licht nicht zeigt": "What the Light Does Not Show",
    "Vorschau: „Lampe“ → „Leuchte“": "Preview: “lamp” → “lantern”",
    "Querverweise": "Cross references",
    "Hallig": "Holm",
    "Priel": "Creek",
    "Handlaterne": "lantern",
    "Laterne": "lantern",
    "Verlag Priel & Tonne": "Creek & Barrel Press",
    "Konten einrichten": "Set up accounts",
    "Jonte Harksen": "Jonte Harksen",
}
# Texts of `erwartet` that are the same in both languages (markup, addresses, names of roles and files).
EXPECTED_NEUTRAL = {
    "#quote(block: true", "pv-accept", "@lektor", "@lektorat:jonte", "127.0.0.1:9", 'name="password"',
    'data-rolle="lektor"',
}


def expected_in(lang, text, name):
    """Text of `erwartet` in the language of the pictures; for en from EXPECTED_EN (see there)."""
    if lang == DEFAULT_LANG or text in EXPECTED_NEUTRAL:
        return text
    if text not in EXPECTED_EN:
        fail(f"Bild „{name}“: erwartet „{text}“ hat keine englische Entsprechung (EXPECTED_EN in {Path(__file__).name}).")
    return EXPECTED_EN[text]


def load_pictures(lang=DEFAULT_LANG):
    """Liest site/data/hilfebilder.toml und die sieben Bilder der Seite „Funktionen“ als eine Liste von Einträgen
    mit den Feldern name, datei (relativ zum Bildordner der Sprache), adresse, fenster, thema, dunkel, ausschnitt,
    schritt, erwartet, stufe, braucht, seite, zeigt – für die Sprache `lang` (nur Bilder, deren `sprachen` sie
    nennt; für en gelten adresse_en, ausschnitt_en, schritt_en und die Tabelle EXPECTED_EN). Bricht bei einem
    fehlerhaften Eintrag ab."""
    with open(LIST_FILE, "rb") as handle:
        listed = tomllib.load(handle).get("bild", [])
    pictures = []
    # Every entry is checked in both languages, whichever is asked for: a broken English override or a text of
    # `erwartet` without an English counterpart already stops --liste and --abgleich for German.
    for entry in FUNCTION_PICTURES:
        for code in LANGS:
            picture = normalised(entry, f"{entry['name']}.png", ["funktionen"], code)
            if code == lang:
                pictures.append(picture)
    for entry in listed:
        if "name" not in entry:
            fail(f"{LIST_FILE.name}: ein Block [[bild]] hat keinen Namen.")
        languages = entry.get("sprachen", list(LANGS))
        if not isinstance(languages, list) or not languages or any(code not in LANGS for code in languages):
            fail(f"Bild „{entry['name']}“: sprachen ist eine nichtleere Liste aus {', '.join(LANGS)}.")
        for code in LANGS:
            picture = normalised(entry, f"{HELP_DIR}/{entry['name']}.png", entry.get("seite", []), code)
            if code == lang and code in languages:
                pictures.append(picture)
    names = [p["name"] for p in pictures if p["datei"].startswith(HELP_DIR + "/")]
    doubled = sorted({n for n in names if names.count(n) > 1})
    if doubled:
        fail(f"{LIST_FILE.name}: doppelte Namen: {', '.join(doubled)}")
    return pictures


def normalised(entry, file, pages, lang=DEFAULT_LANG):
    name = entry["name"]
    if not re.fullmatch(r"[a-z0-9-]+", name):
        fail(f"Bild „{name}“: Namen bestehen aus Kleinbuchstaben, Ziffern und Bindestrich.")
    # English overrides (contract §2.5).
    for field in ("adresse", "ausschnitt", "schritt"):
        if f"{field}_en" in entry and not isinstance(entry[f"{field}_en"], type(entry.get(field, entry[f"{field}_en"]))):
            fail(f"Bild „{name}“: {field}_en hat eine andere Form als {field}.")
    if lang == "en":
        entry = dict(entry)
        for field in ("adresse", "ausschnitt", "schritt"):
            if f"{field}_en" in entry:
                entry[field] = entry[f"{field}_en"]
    address = entry.get("adresse")
    if not address or not address.startswith("/"):
        fail(f"Bild „{name}“: adresse fehlt oder beginnt nicht mit „/“.")
    window = entry.get("fenster", WINDOW)
    if not re.fullmatch(r"\d{3,4}x\d{3,4}", window):
        fail(f"Bild „{name}“: fenster muss wie „1400x860“ aussehen, nicht „{window}“.")
    theme = entry.get("thema", "hell")
    if theme not in ("hell", "dunkel"):
        fail(f"Bild „{name}“: thema ist „hell“ oder „dunkel“.")
    stage = entry.get("stufe", "offen")
    if stage not in ("offen", "konten"):
        fail(f"Bild „{name}“: stufe ist „offen“ oder „konten“.")
    steps = []
    for step in entry.get("schritt", []):
        kind, _, value = step.partition(":")
        if kind not in STEP_KINDS:
            fail(f"Bild „{name}“: unbekannter Schritt „{step}“ (erlaubt: {', '.join(STEP_KINDS)}).")
        if kind == "sitzung" and value not in ("autorin", "lektor"):
            fail(f"Bild „{name}“: sitzung ist „autorin“ oder „lektor“.")
        if kind == "sitzung" and stage != "konten":
            fail(f"Bild „{name}“: eine Sitzung gibt es erst in der Stufe „konten“.")
        if kind == "cookie" and "=" not in value:
            fail(f"Bild „{name}“: cookie braucht NAME=WERT.")
        steps.append((kind, value))
    needs = entry.get("braucht", [])
    for need in needs:
        if need not in KNOWN_NEEDS:
            fail(f"Bild „{name}“: braucht kennt „{need}“ nicht.")
    if "ohne-js" in needs and any(kind == "klick" for kind, _ in steps):
        fail(f"Bild „{name}“: ohne JavaScript gibt es keinen Klick-Schritt (stattdessen „offen:“).")
    cut = entry.get("ausschnitt")
    if cut and not (cut.startswith("css:") or re.fullmatch(r"\d+x\d+\+\d+\+\d+", cut)):
        fail(f"Bild „{name}“: ausschnitt ist „BxH+X+Y“ oder „css:SELEKTOR“.")
    if cut and cut.startswith("css:") and "ohne-js" in needs:
        fail(f"Bild „{name}“: ein Ausschnitt nach Selektor braucht JavaScript; hier „BxH+X+Y“ angeben.")
    return {
        "name": name, "datei": file, "adresse": address, "fenster": window, "thema": theme,
        "dunkel": bool(entry.get("dunkel", False)), "ausschnitt": cut, "schritt": steps,
        "erwartet": [expected_in(lang, text, name) for text in entry.get("erwartet", [])], "stufe": stage,
        "braucht": list(needs),
        "seite": list(pages), "zeigt": entry.get("zeigt", ""),
    }


def variants(picture):
    """Aufnahmen eines Eintrags: (Datei, Thema). Mit dunkel = true zusätzlich <name>-dunkel.png."""
    shots = [(picture["datei"], picture["thema"])]
    if picture["dunkel"] and picture["thema"] != "dunkel":
        shots.append((picture["datei"][: -len(".png")] + "-dunkel.png", "dunkel"))
    return shots


def describe_steps(picture):
    words = []
    for kind, value in picture["schritt"]:
        if kind == "cookie":
            name, _, content = value.partition("=")
            words.append(f"Cookie {name}={content}" if content else f"Cookie {name} weglassen")
        elif kind == "offen":
            words.append(f"<details> mit {value} öffnen")
        elif kind == "klick":
            words.append(f"Klick auf {value}")
        elif kind == "sitzung":
            words.append(f"angemeldet als {'Autorin' if value == 'autorin' else 'Lektor'}")
    if picture["adresse"].startswith("/_antwort/"):
        words.append("gespeicherte Antwort eines Formulars (POST)")
    if "ohne-js" in picture["braucht"]:
        words.append("Profil ohne JavaScript")
    return words


def show_list(pictures, values=None, base="http://127.0.0.1:PORT"):
    for picture in pictures:
        address = fill(picture["adresse"], values) if values else picture["adresse"]
        files = " + ".join(file for file, _ in variants(picture))
        themes = " + ".join(theme for _, theme in variants(picture))
        steps = "; ".join(describe_steps(picture)) or "keine"
        print(f"  {picture['name']}")
        print(f"      Datei    {files}")
        print(f"      Adresse  {base}{address}")
        line = f"      Fenster  {picture['fenster'].replace('x', '×')} · Thema {themes} · Stufe {picture['stufe']}"
        if picture["ausschnitt"]:
            line += f" · Ausschnitt {picture['ausschnitt']}"
        if picture["braucht"]:
            line += f" · braucht {', '.join(picture['braucht'])}"
        print(line)
        print(f"      Schritte {steps}")
        print(f"      Zeigt    {picture['zeigt']}" + (f"  [{', '.join(picture['seite'])}]" if picture["seite"] else ""))


def fill(address, values):
    """Setzt die Platzhalter {work}, {key} … ein; ein unbekannter Platzhalter ist ein Fehler der Liste."""
    try:
        return address.format(**values)
    except KeyError as missing:
        fail(f"Adresse „{address}“: Platzhalter {missing} kennt das Skript nicht "
             f"(bekannt: {', '.join(sorted(values))}).")


# ---------------------------------------------------------------------------------------------------------------
# Reconciliation without a server
# ---------------------------------------------------------------------------------------------------------------

USE = re.compile(r"\{\{<\s*hilfebild\b([^>]*?)>\}\}", re.S)
USE_FUNCTION = re.compile(r"\{\{<\s*bild\b([^>]*?)>\}\}", re.S)


def reconcile(pictures, lang=DEFAULT_LANG):
    """--abgleich: verwendete Namen gegen die Liste, fehlende Dateien, Gesamtgröße – für die Seiten
    (site/content/<lang>) und den Bildordner der Sprache `lang`. Exit 1 bei einem unbekannten Namen, bei einem
    fehlenden hellen Bild, das eine Seite verwendet (der Bau der Webseite bricht dann ab), oder wenn der Bildordner
    der Sprache die Grenze überschreitet; übrige fehlende Dateien sind nur eine Liste."""
    content = content_dir(lang)
    target = target_dir(lang)
    listed = {p["name"]: p for p in pictures if p["datei"].startswith(HELP_DIR + "/")}
    used = {}
    used_files = set()
    pages = sorted(content.rglob("*.md")) if content.is_dir() else []
    for page in pages:
        text = page.read_text(encoding="utf-8")
        for call in USE.finditer(text):
            found = re.search(r'\bname\s*=\s*"([^"]*)"', call.group(1))
            name = found.group(1) if found else "(ohne name)"
            used.setdefault(name, []).append(page.relative_to(content).as_posix())
            used_files.add(f"{HELP_DIR}/{name}.png")
        for call in USE_FUNCTION.finditer(text):
            found = re.search(r'\bdatei\s*=\s*"([^"]*)"', call.group(1))
            if found:
                used_files.add(found.group(1))
    print(f"Sprache {lang}: Bilder unter {target.relative_to(REPO)}, Seiten unter {content.relative_to(REPO)}"
          + ("" if content.is_dir() else " (fehlt – noch keine Seiten dieser Sprache)") + ".")
    print(f"Bildliste: {len(listed)} Namen in {LIST_FILE.relative_to(REPO)}; "
          f"{sum(len(v) for v in used.values())} Aufrufe von hilfebild in {content.relative_to(REPO)}, "
          f"{len(used)} verschiedene Namen.")
    if unknown := sorted(name for name in used if name not in listed):
        print(f"\nUNBEKANNT ({len(unknown)}) – stehen nicht in der Liste (oder nicht für {lang}), der Bau der "
              "Webseite bricht ab:")
        for name in unknown:
            print(f"  {name}  ({', '.join(sorted(set(used[name])))})")
    else:
        print("Unbekannte Namen: keine.")
    unused = sorted(name for name in listed if name not in used)
    if unused:
        print(f"\nUngenutzt ({len(unused)}) – stehen in der Liste, aber auf keiner Seite:")
        for name in unused:
            print(f"  {name}  (vorgesehen: {', '.join(listed[name]['seite']) or '–'})")
    else:
        print("Ungenutzte Namen: keine.")
    elsewhere = []
    for name, pages_of in sorted(used.items()):
        if name in listed:
            planned = set(listed[name]["seite"])
            for page in sorted(set(pages_of)):
                stem = Path(page).stem if Path(page).stem != "_index" else Path(page).parent.name
                if stem not in planned:
                    elsewhere.append(f"  {name}  auf {page} (Liste: {', '.join(sorted(planned)) or '–'})")
    if elsewhere:
        print(f"\nAuf einer anderen Seite als in der Liste vorgesehen ({len(elsewhere)}, nur Hinweis):")
        print("\n".join(elsewhere))
    missing = [file for p in pictures for file, _ in variants(p) if not (target / file).is_file()]
    breaking = sorted(file for file in missing if file in used_files)
    if missing:
        print(f"\nFehlende Dateien ({len(missing)}) unter {target.relative_to(REPO)}:")
        for file in missing:
            print(f"  {file}" + ("  – eine Seite verwendet es, der Bau bricht ab" if file in breaking else ""))
    else:
        print("Fehlende Dateien: keine.")
    known = {file for p in pictures for file, _ in variants(p)}
    stray = sorted(
        path.relative_to(target).as_posix() for path in (target / HELP_DIR).glob("*.png")
        if path.relative_to(target).as_posix() not in known
    ) if (target / HELP_DIR).is_dir() else []
    if stray:
        print(f"\nDateien ohne Eintrag in der Liste ({len(stray)}, nur Hinweis):")
        for file in stray:
            print(f"  {file}")
    total = sum(path.stat().st_size for path in language_files(lang))
    over = total > SIZE_LIMIT
    print(f"\nGröße der Bilder für {lang} ({target.relative_to(REPO)}"
          + (", ohne " + ", ".join(f"{other}/" for other in LANGS if other != DEFAULT_LANG) if lang == DEFAULT_LANG else "")
          + f"): {total / 1024 / 1024:.2f} MB von höchstens {SIZE_LIMIT / 1024 / 1024:.0f} MB"
          + (" – ÜBERSCHRITTEN" if over else "."))
    if unknown or breaking or over:
        sys.exit(1)


# ---------------------------------------------------------------------------------------------------------------
# Throwaway server
# ---------------------------------------------------------------------------------------------------------------

def find_server(given):
    for candidate in (given, os.environ.get("BARUCH_SERVER")):
        if candidate and Path(candidate).is_file():
            return Path(candidate).resolve()
    # Without a value the file built last: an old release build next to a fresh test build otherwise showed an
    # old user interface (Round 37: build from the day before, 404 on the Word export). Which file runs is in the
    # output.
    built = [path for path in (REPO / "target" / "release" / "baruch-server", REPO / "target" / "debug" / "baruch-server")
             if path.is_file()]
    if built:
        newest = max(built, key=lambda path: path.stat().st_mtime)
        sys.stderr.write(f"Server: {newest} (zuletzt gebaut; anderer mit --server PFAD)\n")
        return newest.resolve()
    fail("baruch-server nicht gefunden. Erst bauen (nach der Stromregel in AGENTS.md: "
         "nice -n 10 ionice -c3 taskset -c 0-7 cargo build -p baruch-server -j 4) oder mit --server PFAD angeben.")


def clean_env():
    """Umgebung ohne BARUCH_*: Der Wegwerf-Server erbt weder Adresse noch Datenordner des Dienstes."""
    return {name: value for name, value in os.environ.items() if not name.startswith("BARUCH_")}


def server_env(data):
    """Umgebung des Wegwerf-Servers: ohne BARUCH_* und mit einem eigenen, leeren Heimatordner neben dem
    Datenordner. „Über Baruch“ nennt als Vorgabe den Datenordner unter dem Heimatordner – mit dem echten stünde
    der Name des Benutzers im Bild."""
    env = {name: value for name, value in clean_env().items() if not name.startswith("XDG_")}
    home = Path(data).parent / "heim"
    home.mkdir(parents=True, exist_ok=True)
    env["HOME"] = str(home)
    return env


def private_parts(*paths):
    """Pfade, die den Heimatordner oder den Anmeldenamen des Benutzers enthalten – sie stünden in den Bildern
    (Einstellungen → Daten, → Quellen, „Über Baruch“)."""
    home = str(Path.home())
    user = os.environ.get("USER") or os.environ.get("LOGNAME") or ""
    return [str(path) for path in paths
            if path and (home in str(path) or (len(user) > 2 and user.lower() in str(path).lower()))]


def import_work(server, data, work_file):
    done = subprocess.run(
        [str(server), "import", str(work_file), "--data", str(data)],
        capture_output=True, text=True, env=server_env(data), check=False,
    )
    sys.stdout.write(done.stdout)
    sys.stderr.write(done.stderr)
    if done.returncode != 0:
        fail(f"Import gescheitert (Exit {done.returncode}).")
    found = re.search(r"\(id (\d+)\)", done.stdout)
    if not found:
        fail("Import: Die Ausgabe nennt keine Werk-id.")
    return int(found.group(1))


def point_connections_nowhere(data):
    """Verbindungen `eliezer` und `multigpt` auf 127.0.0.1:9, bevor der Server zum ersten Mal startet."""
    config = json.dumps({"url": NOWHERE})
    with sqlite3.connect(data / "baruch.sqlite") as db:
        for kind in ("eliezer", "multigpt"):
            db.execute(
                "INSERT INTO connection (kind, config) VALUES (?, ?) "
                "ON CONFLICT(kind) DO UPDATE SET config = excluded.config",
                (kind, config),
            )
    print(f"Verbindungen eliezer und multigpt zeigen auf {NOWHERE}.")


def start_server(server, data):
    """Startet den Server und liefert (Prozess, Basisadresse)."""
    process = subprocess.Popen(
        [str(server), "serve", "--listen", "127.0.0.1:0", "--data", str(data), "--secrets", "memory"],
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, env=server_env(data),
    )
    base = None
    for line in process.stdout:
        sys.stdout.write("  [server] " + line)
        found = re.search(r"Baruch läuft: (http://127\.0\.0\.1:(\d+))/", line)
        if found:
            if int(found.group(2)) == FORBIDDEN_PORT:
                stop_server(process)
                fail("Der Wegwerf-Server hat den Port des Dienstes bekommen; abgebrochen.")
            base = found.group(1)
            break
    if base is None:
        process.wait()
        fail(f"Der Server ist nicht gestartet (Exit {process.returncode}).")
    # Keep reading further output of the server so that it never hangs on a full pipe.
    threading.Thread(target=lambda: [None for _ in process.stdout], daemon=True).start()
    return process, base


def stop_server(process):
    """Nur den eigenen Prozess beenden (PID), erst freundlich, dann hart."""
    if process.poll() is not None:
        return
    process.send_signal(signal.SIGINT)
    try:
        process.wait(timeout=15)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait()


# ---------------------------------------------------------------------------------------------------------------
# Requests to the own server (127.0.0.1 only)
# ---------------------------------------------------------------------------------------------------------------

def request(base, method, path, body=None, headers=None, timeout=300):
    """Eine Anfrage ohne Weiterleitungen: (Status, Kopfzeilen, Inhalt). Nur an 127.0.0.1."""
    target = urllib.parse.urlsplit(base)
    if target.hostname != "127.0.0.1" or target.port == FORBIDDEN_PORT:
        fail(f"Anfrage an {base} abgelehnt: Das Skript spricht nur mit dem eigenen Server auf 127.0.0.1.")
    connection = http.client.HTTPConnection(target.hostname, target.port, timeout=timeout)
    try:
        connection.request(method, path, body=body, headers=headers or {})
        response = connection.getresponse()
        return response.status, response.headers, response.read()
    finally:
        connection.close()


class Client:
    """Formulare und RPC am eigenen Server, auf Wunsch mit Sitzung (Cookie). `Origin` trägt die Adresse des
    Servers: Die Formulare stehen hinter guard::same_origin. Cookie baruch_lang und Accept-Language tragen die
    Sprache der Bilder: Gespeicherte Antworten (Bericht und Vorschauen des Einlesens) kommen so in dieser Sprache."""

    def __init__(self, base, session=None, lang=DEFAULT_LANG):
        self.base = base
        self.session = session
        self.lang = lang

    def headers(self, extra=None):
        headers = {"Origin": self.base, "Accept-Language": self.lang}
        cookies = [f"{LANG_COOKIE}={self.lang}"]
        if self.session:
            cookies.append(f"{SESSION_COOKIE}={self.session}")
        headers["Cookie"] = "; ".join(cookies)
        headers.update(extra or {})
        return headers

    def get(self, path):
        return request(self.base, "GET", path, headers=self.headers())

    def rpc(self, method, params, allow_error=False):
        body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode()
        status, _, content = request(
            self.base, "POST", "/rpc", body, self.headers({"Content-Type": "application/json"})
        )
        try:
            answer = json.loads(content)
        except ValueError:
            fail(f"{method}: Antwort {status} ist kein JSON.")
        if answer.get("error"):
            if allow_error:
                return None
            fail(f"{method}: {answer['error'].get('message')} (Code {answer['error'].get('code')})")
        return answer["result"]

    def form(self, path, fields):
        body = urllib.parse.urlencode(fields).encode()
        return request(
            self.base, "POST", path, body, self.headers({"Content-Type": "application/x-www-form-urlencoded"})
        )

    def upload(self, path, fields, file_field, file_name, content, mime="application/octet-stream"):
        boundary = "baruch-fotos-" + secrets.token_hex(12)
        parts = []
        for name, value in fields.items():
            parts.append(
                f'--{boundary}\r\nContent-Disposition: form-data; name="{name}"\r\n\r\n{value}\r\n'.encode()
            )
        parts.append(
            f'--{boundary}\r\nContent-Disposition: form-data; name="{file_field}"; filename="{file_name}"\r\n'
            f"Content-Type: {mime}\r\n\r\n".encode() + content + b"\r\n"
        )
        parts.append(f"--{boundary}--\r\n".encode())
        return request(
            self.base, "POST", path, b"".join(parts),
            self.headers({"Content-Type": f"multipart/form-data; boundary={boundary}"}),
        )


class Mcp:
    """Der MCP-Zugang des eigenen Servers (POST /mcp, JSON-RPC): So schreiben Assistenten – als Vorschlag, Beitrag
    oder Antrag. Ohne Token über 127.0.0.1; die Herkunft ist der Name des Clients bzw. die genannte Persona."""

    def __init__(self, base, name):
        self.base = base
        self.session = None
        status, headers, content = self.send("initialize", {
            "protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": name, "version": "1"},
        })
        self.session = headers.get("Mcp-Session-Id")
        if status != 200 or not self.session:
            fail(f"MCP initialize: Status {status}, keine Mcp-Session-Id.")

    def send(self, method, params):
        headers = {"Content-Type": "application/json", "Accept": "application/json, text/event-stream"}
        if self.session:
            headers["Mcp-Session-Id"] = self.session
        body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode()
        return request(self.base, "POST", "/mcp", body, headers)

    def call(self, tool, arguments):
        status, _, content = self.send("tools/call", {"name": tool, "arguments": arguments})
        text = content.decode("utf-8", "replace")
        if text.lstrip().startswith("event:") or text.lstrip().startswith("data:"):
            text = "\n".join(line[5:].strip() for line in text.splitlines() if line.startswith("data:"))
        try:
            answer = json.loads(text)
        except ValueError:
            fail(f"MCP {tool}: Antwort {status} ist kein JSON.")
        if answer.get("error"):
            fail(f"MCP {tool}: {answer['error'].get('message')}")
        result = answer["result"]
        if result.get("isError"):
            said = " ".join(part.get("text", "") for part in result.get("content", []))
            fail(f"MCP {tool}: {said}")
        if "structuredContent" in result:
            return result["structuredContent"]
        for part in result.get("content", []):
            try:
                return json.loads(part.get("text", ""))
            except ValueError:
                continue
        return result


# ---------------------------------------------------------------------------------------------------------------
# Generated files: images (PNG with zlib/struct), a small EPUB as a source, a Markdown file
# ---------------------------------------------------------------------------------------------------------------

def lighthouse_png(width, height):
    """Ein gezeichneter Leuchtturm vor Himmel und Meer – frei erzeugt, kein Foto, keine fremde Vorlage."""
    def blend(a, b, t):
        return bytes(round(a[i] + (b[i] - a[i]) * t) for i in range(3))

    horizon = round(height * 0.74)
    top, foot = round(height * 0.22), round(height * 0.80)
    lantern = round(height * 0.05)
    rows = bytearray()
    for y in range(height):
        if y < horizon:
            back = blend((36, 62, 104), (238, 176, 120), y / horizon)
        else:
            back = blend((40, 92, 122), (16, 44, 70), (y - horizon) / max(1, height - horizon))
        row = bytearray(back * width)
        if top - lantern <= y < foot:
            if y < top:
                half, colour = round(width * 0.035), (250, 222, 120)
            else:
                grow = (y - top) / (foot - top)
                half = round(width * (0.045 + 0.04 * grow))
                colour = (190, 52, 44) if int(grow * 7) % 2 == 0 else (244, 240, 230)
            left = width // 2 - half
            row[left * 3:(left + 2 * half) * 3] = bytes(colour) * (2 * half)
        elif foot <= y < foot + round(height * 0.03):
            half = round(width * 0.22)
            left = width // 2 - half
            row[left * 3:(left + 2 * half) * 3] = bytes((70, 88, 62)) * (2 * half)
        rows.append(0)
        rows += row

    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)

    header = struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0)
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", header) + chunk(b"IDAT", zlib.compress(bytes(rows), 9))
            + chunk(b"IEND", b""))


def source_epub(sample):
    """Ein winziges EPUB als Quelle für die Literatur: ein erfundenes Dienstbuch, zwei Kapitel (in der Sprache der
    Bilder)."""
    source = sample["source"]
    chapters = source["chapters"]
    items = "".join(f'<item id="k{i}" href="k{i}.xhtml" media-type="application/xhtml+xml"/>' for i in (1, 2))
    refs = "".join(f'<itemref idref="k{i}"/>' for i in (1, 2))
    opf = (
        '<?xml version="1.0" encoding="UTF-8"?>\n'
        '<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id">'
        '<metadata xmlns:dc="http://purl.org/dc/elements/1.1/">'
        f'<dc:identifier id="id">{source["identifier"]}</dc:identifier>'
        f"<dc:title>{html.escape(source['title'])}</dc:title>"
        f"<dc:creator>{html.escape(source['creator'])}</dc:creator><dc:language>{source['language']}</dc:language>"
        '<meta property="dcterms:modified">1911-03-01T00:00:00Z</meta></metadata>'
        f'<manifest><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>{items}'
        f"</manifest><spine>{refs}</spine></package>"
    )
    nav = (
        '<?xml version="1.0" encoding="UTF-8"?>\n<html xmlns="http://www.w3.org/1999/xhtml" '
        f'xmlns:epub="http://www.idpf.org/2007/ops"><head><title>{html.escape(source["toc"])}</title></head><body>'
        '<nav epub:type="toc"><ol>'
        + "".join(f'<li><a href="k{i + 1}.xhtml">{html.escape(title)}</a></li>' for i, (title, _) in enumerate(chapters))
        + "</ol></nav></body></html>"
    )
    container = (
        '<?xml version="1.0"?>\n<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">'
        '<rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/>'
        "</rootfiles></container>"
    )
    packed = io.BytesIO()
    with zipfile.ZipFile(packed, "w") as archive:
        archive.writestr(zipfile.ZipInfo("mimetype"), "application/epub+zip", zipfile.ZIP_STORED)
        archive.writestr("META-INF/container.xml", container, zipfile.ZIP_DEFLATED)
        archive.writestr("OEBPS/content.opf", opf, zipfile.ZIP_DEFLATED)
        archive.writestr("OEBPS/nav.xhtml", nav, zipfile.ZIP_DEFLATED)
        for i, (title, text) in enumerate(chapters, start=1):
            page = (
                '<?xml version="1.0" encoding="UTF-8"?>\n<html xmlns="http://www.w3.org/1999/xhtml"><head>'
                f"<title>{html.escape(title)}</title></head><body><h1>{html.escape(title)}</h1>"
                f"<p>{html.escape(text)}</p></body></html>"
            )
            archive.writestr(f"OEBPS/k{i}.xhtml", page, zipfile.ZIP_DEFLATED)
    return packed.getvalue()


# ---------------------------------------------------------------------------------------------------------------
# Invented sample data per language (contract Round 37 §4, Round 39b §2.2). German = the texts since Round 37,
# unchanged; English = the same motifs in English (no German text in the English work). Section titles must match
# the work file of the language; quotations of findings and words of index marks must occur in the section text.
# ---------------------------------------------------------------------------------------------------------------

def isbn13(first_twelve):
    """Hängt die Prüfziffer an – eine erfundene Nummer aus dem Bereich 978-3-99999 (kein Verlag führt sie)."""
    total = sum(int(digit) * (1 if index % 2 == 0 else 3) for index, digit in enumerate(first_twelve))
    return first_twelve + str((10 - total % 10) % 10)


SAMPLES = {
    "de": {
        "work_file": TOOLS / "beispielwerk.typ",
        "mcp_client": "Schreibhilfe",
        "revised": "Der erste Morgen",
        "second_revision": """== Der erste Morgen

Ich wachte auf, weil es still war. Kein Wind, kein Wasser an der Mauer – nur die Uhr über der Treppe.
Hundertzwölf Stufen sind es bis zur Laterne. Ich habe sie an diesem Morgen zum ersten Mal gezählt, mich zweimal
verzählt und beim dritten Mal laut gezählt, damit es jemand hört.

#szene[Oben stand das Fenster offen. Auf dem Sims lag eine Feder, grau mit weißem Rand.]
""",
        "logbook": "Das Logbuch",
        "logbook_figure": (
            '\n#figure(image("bilder/leuchtturm.png", width: 55%), caption: [Der Turm von Mövenoog, vom Anleger aus '
            "gesehen])\n"
        ),
        "proposed": "Besuch vom Festland",
        "proposal": """== Besuch vom Festland

Am Johannistag brachte das Versorgungsboot Mehl, Petroleum und einen Jungen. Er hieß Ole, sollte drei Wochen
bleiben und fragte gleich am ersten Abend, ob das Licht auch für die Fische leuchte.

Am zweiten Abend wollte er wissen, wer die Lampe anzündet, wenn ich krank bin. „Der Wärter von Süderoog“, sagte
ich, „wenn er es rechtzeitig sieht.“ Ole fand, das sei keine gute Antwort. Er hatte recht.
""",
        "xref": "Was das Licht nicht zeigt",
        "night": "Die Nacht ohne Lampe",
        "farewell": "Abschied",
        "lektor_proposal": """== Die Nacht ohne Lampe

In der Nacht zum elften riss der Docht. Vier Stunden stand ich mit der Handlaterne am Fenster und schwenkte sie,
wie es das Dienstbuch von 1911 beschreibt. Draußen fuhr niemand. Das wusste ich aber erst am Morgen.
""",
        "picture": {"file_name": "leuchtturm.png", "path": "bilder/leuchtturm.png",
                    "caption": "Der Turm von Mövenoog, vom Anleger aus gesehen",
                    "alt": "Rot-weiß gestreifter Leuchtturm vor Abendhimmel und Meer"},
        "cover": {"file_name": "umschlag.png", "caption": "Umschlagbild", "alt": "Leuchtturm in der Abenddämmerung"},
        "index_marks": (("logbook", "Dienstzimmer"), ("logbook", "Postkarte"), ("night", "Handlaterne"),
                        ("night", "Docht")),
        "xref_link": "\nWie es ausging, steht im #link(<k{key}>)[Abschied].\n",
        "glossary": (
            {"term": "Hallig", "short": "kleine Marschinsel ohne Deich", "variants": ["Halligen"],
             "long": "Bei Sturmflut steht das Land unter Wasser; nur die Warften mit den Häusern ragen heraus."},
            {"term": "Laterne", "short": "der verglaste Raum an der Spitze des Turms, in dem die Lampe brennt"},
            {"term": "Priel", "short": "Wasserlauf im Watt, der auch bei Ebbe nicht trockenfällt"},
        ),
        "meta": {
            "subtitle": "Aufzeichnungen einer Leuchtturmwärterin", "publisher": "Verlag Priel & Tonne",
            "place": "Süderhaven", "year": "2026", "edition": "1. Auflage", "isbn": isbn13("978399999012"),
            "printer": "Druckhaus am Deich, Süderhaven", "dedication": "Für alle, die abends ein Licht anzünden.",
            "blurb": "Ein Jahr lang hütet Mara Tiedemann das Leuchtfeuer der Hallig Mövenoog. Sie zählt Stufen, führt "
                     "das Logbuch und lernt, was ein Licht zeigt – und was nicht.",
            "bio": "Mara Tiedemann gibt es nicht. Sie ist die erfundene Autorin dieses Beispielwerks.",
        },
        "ideas": {
            "fire": {"title": "Leuchtfeuer", "words": ["Licht", "Dienst"],
                     "note": "Was das Feuer braucht und was es von der Wärterin verlangt."},
            "wick": {"title": "Docht und Petroleum", "words": ["Docht"], "note": "Drei Stärken; der Laden am Hafen."},
            "lamp": {"title": "Handlaterne von 1911", "words": ["Dienstbuch"]},
            "swing": {"title": "Takt beim Schwenken", "words": ["Takt"]},
            "guest": {"title": "Ole", "words": ["Besuch", "Fragen"],
                      "note": "Ein Kind stellt die Fragen, die die Wärterin sich nicht stellt."},
            "fish": {"title": "Leuchtet das Licht für Fische?", "words": ["Frage"]},
            "feather": {"title": "Die Feder auf dem Sims", "words": ["Feder", "Zeichen"],
                        "quote": "Auf dem Sims lag eine Feder, grau mit weißem Rand."},
            "surge": {"words": ["Sturmflut", "Warft"], "note": "Einmal das Land unter Wasser zeigen – im Herbst?"},
            "letter": {"title": "Brief an den alten Wärter", "words": ["Brief"],
                       "note": "Vielleicht als Schluss, nach dem Abschied."},
            "link_note": "reißt der Docht, bleibt die Laterne",
        },
        "posts": {
            "first": "Der Einstieg über die Stille trägt. Die Feder am Schluss ist stark – ich würde sie nicht erklären.",
            "human": "Danke. Zählt sie die Stufen zu oft? Dreimal „gezählt“ in zwei Sätzen.",
            "second": "Das dritte „gezählt“ ist das wichtige: laut, damit es jemand hört. Die ersten beiden ließen sich "
                      "zusammenziehen.",
            "proposal_note": "Wörtliche Rede statt Bericht; ein Schlusssatz.",
            "rename_title": "Abschied von der Hallig",
            "rename_note": "Der Ort gehört in den Titel des letzten Abschnitts.",
            "thread_question": "Reihenfolge der Kapitel: Soll der Herbst vor dem Sommer stehen?",
            "thread_answer": "Chronologisch bleibt es klarer. Ein Vorgriff auf den Sturm im ersten Kapitel würde reichen.",
            "thread_follow_up": "Dann bleibt es beim Jahreslauf. Den Vorgriff probiere ich in „Ankunft“.",
        },
        "source": {
            "file_name": "dienstbuch-1911.epub", "identifier": "urn:uuid:6f0f2a3e-51c7-4b0e-9d7a-0a5b3c1d2e4f",
            "title": "Dienstbuch für die Leuchtfeuerwärter der Halligen", "creator": "Feuerschiffamt Süderhaven",
            "language": "de", "toc": "Inhalt",
            "chapters": (
                ("Vom Anzünden", "Die Lampe wird eine halbe Stunde vor Sonnenuntergang angezündet und brennt bis zum "
                                 "vollen Tag. Der Wärter grüßt sie, ehe er die Treppe hinabsteigt; so ist es Brauch auf "
                                 "den Halligen."),
                ("Wenn der Docht reißt", "Reißt der Docht in der Nacht, so tritt der Wärter mit der Handlaterne an das "
                                         "seewärtige Fenster und schwenkt sie in gleichem Takt, bis der Schaden behoben "
                                         "ist. Jede Stunde ohne Feuer wird im Logbuch vermerkt."),
            ),
        },
        # Throwaway work: is read in as Markdown (the report names the missing image) and deleted afterwards (archive).
        "throwaway": {
            "title": "Notizen vom Festland", "file_name": "notizen-vom-festland.md",
            "markdown": """# Notizen vom Festland

Was ich im Winter in Süderhaven aufgeschrieben habe, bevor das Boot wieder fuhr.

## Die Fähre

Sie fährt nur bei Hochwasser und nur, wenn der Schiffer Lust hat. Beides trifft selten zusammen.

![Die Fähre am Anleger](bilder/faehre.png)

## Der Laden am Hafen

Mehl, Petroleum, Dochte in drei Stärken. Die Zeitung ist immer von vorgestern.
""",
        },
        "export_stem": "ein-jahr-auf-moevenoog",
        "findings": (
            {"at": "key", "kind": "hint", "quote": "mich zweimal verzählt",
             "body": "Dreimal „gezählt“ kurz nacheinander. Absicht? Wenn ja, das dritte Mal deutlicher absetzen."},
            {"at": "key_night", "kind": "critical", "quote": "Dienstbuch von 1911",
             "body": "Im Kapitel „Ankunft“ ist der Vorgänger dreißig Jahre im Dienst; ein Dienstbuch von 1911 passt "
                     "dazu nur, wenn die Geschichte nach 1941 spielt. Bitte die Jahreszahlen abgleichen."},
            {"at": "key_xref", "kind": "praise", "quote": "Wohin man fahren soll, muss jeder selbst wissen.",
             "body": "Der stärkste Satz des Kapitels. So stehen lassen."},
            {"at": "key_logbook", "kind": "hint",
             "body": "Die Liste „Was ins Logbuch gehört“ wiederholt die vier Spalten von oben – eine von beiden kürzen."},
            {"kind": "hint", "body": "Zum ganzen Buch: Der Winter fehlt. Das Jahr beginnt im Februar und endet im Herbst."},
        ),
        "lektor_proposal_note": "Zwei Sätze gestrafft.",
        "lektor_post": "Vorschlag liegt am Abschnitt. Die Frage nach 1911 habe ich als Befund notiert.",
    },
    "en": {
        "work_file": TOOLS / "beispielwerk.en.typ",
        "mcp_client": "Scribe",
        "revised": "The First Morning",
        "second_revision": """== The First Morning

I woke up because it was quiet. No wind, no water against the wall – only the clock above the stairs.
There are a hundred and twelve steps up to the lantern. That morning I counted them for the first time, got the
number wrong twice and the third time counted aloud, so that someone would hear it.

#scene[Up there the window stood open. On the sill lay a feather, grey with a white edge.]
""",
        "logbook": "The Logbook",
        "logbook_figure": (
            '\n#figure(image("images/lighthouse.png", width: 55%), caption: [The tower of Gull Holm, seen from the '
            "landing stage])\n"
        ),
        "proposed": "A Visitor from the Mainland",
        "proposal": """== A Visitor from the Mainland

On Midsummer Day the supply boat brought flour, paraffin and a boy. His name was Ole, he was to stay three weeks,
and on the very first evening he asked whether the light shone for the fish as well.

On the second evening he wanted to know who lights the lamp when I am ill. "The keeper on South Holm," I said, "if
he sees it in time." Ole thought that was not a good answer. He was right.
""",
        "xref": "What the Light Does Not Show",
        "night": "The Night Without the Lamp",
        "farewell": "Farewell",
        "lektor_proposal": """== The Night Without the Lamp

In the night before the eleventh the wick broke. For four hours I stood at the window with the hand lantern and
swung it, as the service book of 1911 describes. Nobody was out at sea. I only knew that in the morning.
""",
        "picture": {"file_name": "lighthouse.png", "path": "images/lighthouse.png",
                    "caption": "The tower of Gull Holm, seen from the landing stage",
                    "alt": "Red and white striped lighthouse against an evening sky and the sea"},
        "cover": {"file_name": "cover.png", "caption": "Cover picture", "alt": "Lighthouse at dusk"},
        "index_marks": (("logbook", "oilcloth"), ("logbook", "postcard"), ("night", "lantern"), ("night", "wick")),
        "xref_link": "\nHow it ended is told in the #link(<k{key}>)[Farewell].\n",
        "glossary": (
            {"term": "Holm", "short": "small island in the shallows, without a dyke", "variants": ["Holms"],
             "long": "In a storm surge the land lies under water; only the mounds with the houses stay above it."},
            {"term": "Lantern", "short": "the glazed room at the top of the tower in which the lamp burns"},
            {"term": "Creek", "short": "channel in the mudflats that does not run dry even at low tide"},
        ),
        "meta": {
            "subtitle": "Records of a Lighthouse Keeper", "publisher": "Creek & Barrel Press",
            "place": "Southhaven", "year": "2026", "edition": "First edition", "isbn": isbn13("978399999012"),
            "printer": "Dykeside Printers, Southhaven", "dedication": "For everyone who lights a lamp in the evening.",
            "blurb": "For one year Mara Tiedemann keeps the light of the island of Gull Holm. She counts steps, keeps "
                     "the logbook and learns what a light shows – and what it does not.",
            "bio": "Mara Tiedemann does not exist. She is the invented author of this sample work.",
        },
        "ideas": {
            "fire": {"title": "Beacon", "words": ["light", "duty"],
                     "note": "What the fire needs and what it asks of the keeper."},
            "wick": {"title": "Wick and paraffin", "words": ["wick"], "note": "Three strengths; the shop at the harbour."},
            "lamp": {"title": "Hand lantern of 1911", "words": ["service book"]},
            "swing": {"title": "Rhythm of the swinging", "words": ["rhythm"]},
            "guest": {"title": "Ole", "words": ["visit", "questions"],
                      "note": "A child asks the questions the keeper does not ask herself."},
            "fish": {"title": "Does the light shine for fish?", "words": ["question"]},
            "feather": {"title": "The feather on the sill", "words": ["feather", "sign"],
                        "quote": "On the sill lay a feather, grey with a white edge."},
            "surge": {"words": ["storm surge", "mound"], "note": "Show the land under water once – in autumn?"},
            "letter": {"title": "Letter to the old keeper", "words": ["letter"],
                       "note": "Perhaps as the ending, after the farewell."},
            "link_note": "if the wick breaks, the lantern remains",
        },
        "posts": {
            "first": "Opening with the silence works. The feather at the end is strong – I would not explain it.",
            "human": "Thanks. Does she count the steps too often? \"Counted\" three times in two sentences.",
            "second": "The third \"counted\" is the important one: aloud, so that someone hears it. The first two could "
                      "be merged.",
            "proposal_note": "Direct speech instead of a report; a closing sentence.",
            "rename_title": "Farewell to the Island",
            "rename_note": "The place belongs in the title of the last section.",
            "thread_question": "Order of the chapters: should autumn come before summer?",
            "thread_answer": "Chronological stays clearer. A hint of the storm in the first chapter would be enough.",
            "thread_follow_up": "Then it stays with the course of the year. I will try the hint in \"Arrival\".",
        },
        "source": {
            "file_name": "service-book-1911.epub", "identifier": "urn:uuid:3c8e5b1a-7d24-4f6e-a1b9-2e0d4c6f8a13",
            "title": "Service Book for the Keepers of the Island Lights", "creator": "Lightship Office Southhaven",
            "language": "en", "toc": "Contents",
            "chapters": (
                ("On Lighting", "The lamp is lit half an hour before sunset and burns until full day. The keeper "
                                "greets it before he goes down the stairs; such is the custom on the islands."),
                ("When the Wick Breaks", "If the wick breaks in the night, the keeper steps to the seaward window with "
                                         "the hand lantern and swings it in an even rhythm until the damage is "
                                         "repaired. Every hour without fire is entered in the logbook."),
            ),
        },
        "throwaway": {
            "title": "Notes from the Mainland", "file_name": "notes-from-the-mainland.md",
            "markdown": """# Notes from the Mainland

What I wrote down in Southhaven in winter, before the boat sailed again.

## The Ferry

It only sails at high water and only when the skipper feels like it. The two rarely coincide.

![The ferry at the landing stage](images/ferry.png)

## The Shop at the Harbour

Flour, paraffin, wicks in three strengths. The newspaper is always from the day before yesterday.
""",
        },
        "export_stem": "a-year-on-gull-holm",
        "findings": (
            {"at": "key", "kind": "hint", "quote": "got the number wrong twice",
             "body": "\"Counted\" three times in quick succession. On purpose? If so, set off the third time more "
                     "clearly."},
            {"at": "key_night", "kind": "critical", "quote": "service book of 1911",
             "body": "In the chapter \"Arrival\" the predecessor has served for thirty years; a service book of 1911 "
                     "only fits if the story is set after 1941. Please check the years."},
            {"at": "key_xref", "kind": "praise", "quote": "Where you ought to go, everyone has to know for themselves.",
             "body": "The strongest sentence of the chapter. Leave it as it is."},
            {"at": "key_logbook", "kind": "hint",
             "body": "The list \"What Goes in the Logbook\" repeats the four columns from above – shorten one of them."},
            {"kind": "hint", "body": "On the whole book: winter is missing. The year begins in February and ends in "
                                     "autumn."},
        ),
        "lektor_proposal_note": "Two sentences tightened.",
        "lektor_post": "The proposal is at the section. I noted the question about 1911 as a finding.",
    },
}


# ---------------------------------------------------------------------------------------------------------------
# Extend the sample work – stage „offen“ (without accounts)
# ---------------------------------------------------------------------------------------------------------------

def build_open(base, work, data, sword, lang=DEFAULT_LANG):
    """Baut das Beispielwerk aus, solange es keine Konten gibt. Liefert (Platzhalter für die Adressen, gespeicherte
    Antworten von Formularen)."""
    sample_data = SAMPLES[lang]
    me = Client(base, lang=lang)
    values = {"work": work, "sample": 1}
    answers = {}

    # Work language of the sample work = language of the pictures (Round 39b): book output and generated words follow
    # it, the interface follows the cookie.
    me.rpc("work.language_set", {"work": work, "language": lang})
    print(f"Werksprache des Beispielwerks: {me.rpc('work.language', {'work': work})['language']}")

    # Bible modules: never those of the user (~/.sword, /usr/share/sword) – only the given or an empty directory.
    if sword:
        bibles = me.rpc("bible.set_dirs", {"dirs": [str(Path(sword).expanduser().resolve())]})
        print(f"Bibelmodule aus {sword}: {len(bibles.get('modules', []))} gefunden.")
        # The first readable Bible becomes the Bible of the work – otherwise „Zitate prüfen“ and the settings would show
        # the empty state despite a module.
        readable = [m for m in bibles.get("modules", []) if m.get("kind", "bible") == "bible" and not m.get("problem")]
        if readable:
            me.rpc("bible.set_work", {"work": work, "module": readable[0]["module"]})
            print(f"Bibel des Werks: {readable[0]['module']}")
    else:
        empty = data / "keine-bibelmodule"
        (empty / "mods.d").mkdir(parents=True, exist_ok=True)
        me.rpc("bible.set_dirs", {"dirs": [str(empty)]})

    sample = me.rpc("work.list", {})["works"][0]["id"]
    values["sample"] = sample
    nodes = me.rpc("work.get", {"work": work})["work"]["nodes"]
    index_of = {node["title"]: index for index, node in enumerate(nodes)}
    key_of = {node["title"]: node["key"] for node in nodes}
    titles = {role: sample_data[role] for role in ("revised", "logbook", "proposed", "xref", "night", "farewell")}
    for title in titles.values():
        if title not in key_of:
            fail(f"Abschnitt „{title}“ fehlt im importierten Werk.")
    revised, logbook, proposed = titles["revised"], titles["logbook"], titles["proposed"]
    xref, night, farewell = titles["xref"], titles["night"], titles["farewell"]
    values.update(key=key_of[revised], key_logbook=key_of[logbook], key_proposal=key_of[proposed],
                  key_xref=key_of[xref], key_night=key_of[night], key_farewell=key_of[farewell])

    def current_text(title):
        node = me.rpc("node.get", {"work": work, "node": index_of[title]})
        node = node.get("node", node)
        revisions = node["revisions"]
        current = next((r for r in revisions if r.get("current")), revisions[-1])
        return current["body"], current["number"]

    # Second revision (like the human in the user interface).
    saved = me.rpc("revision.save", {"work": work, "node": index_of[revised], "body": sample_data["second_revision"],
                                     "mode": "new"})
    print(f"Abschnitt „{revised}“ (key {values['key']}) hat jetzt Fassung {saved.get('rev')}.")

    # Images: a work image and a cover image, both drawn in the script.
    shown = sample_data["picture"]
    picture = base64.b64encode(lighthouse_png(1200, 800)).decode()
    me.rpc("asset.add", {
        "work": work, "file_name": shown["file_name"], "content": picture, "encoding": "base64",
        "path": shown["path"], "caption": shown["caption"], "alt": shown["alt"],
    })
    wrapper = sample_data["cover"]
    cover = base64.b64encode(lighthouse_png(1748, 2480)).decode()
    me.rpc("asset.add", {
        "work": work, "file_name": wrapper["file_name"], "content": cover, "encoding": "base64",
        "caption": wrapper["caption"], "alt": wrapper["alt"],
    })
    me.rpc("work.set_cover", {"work": work, "name": wrapper["file_name"]})
    print(f"Bilder {shown['file_name']} und {wrapper['file_name']} angelegt, Umschlag gesetzt.")

    # Image in the text, then index marks (every mark is a new revision of the human).
    body, _ = current_text(logbook)
    me.rpc("revision.save", {"work": work, "node": index_of[logbook],
                             "body": body.rstrip("\n") + "\n" + sample_data["logbook_figure"], "mode": "new"})
    for role, word in sample_data["index_marks"]:
        title = titles[role]
        _, rev = current_text(title)
        me.rpc("index.mark", {"work": work, "key": key_of[title], "rev": rev, "word": word})
    print(f"Bild im Abschnitt „{logbook}“, {len(sample_data['index_marks'])} Registermarken gesetzt.")

    # Cross-reference to the farewell (fixed jump mark <k + key>).
    body, _ = current_text(xref)
    link = sample_data["xref_link"].replace("{key}", str(key_of[farewell]))
    linked = me.rpc("revision.save", {"work": work, "node": index_of[xref],
                                      "body": body.rstrip("\n") + "\n" + link, "mode": "new"})
    values["rev_xref"] = linked["rev"]

    # Glossary.
    for term in sample_data["glossary"]:
        me.rpc("glossary.add", {"work": work, **term})

    # Book details (invented), typeset settings, target time for the time guard.
    me.rpc("work.meta.set", {"work": work, **sample_data["meta"]})
    me.rpc("work.typeset_set", {"work": work, "settings": {
        "chapter_start": "right_page", "page_format": {"kind": "a5"}, "font_size_pt": 10.5,
        "page_numbers": {"position": "bottom_outside", "start": "first_chapter"}, "running_head": "chapter",
    }})
    me.rpc("work.timing_set", {"work": work, "words_per_minute": 130, "target_minutes": 5})
    print(f"Glossar ({len(sample_data['glossary'])} Begriffe), Buchangaben, Satz-Einstellungen und Zielzeit gesetzt.")

    # Ideas: main topic with sub-nodes, an idea at the section, two in the cloud, two links.
    ideas = sample_data["ideas"]

    def idea(**fields):
        return me.rpc("idea.create", {"work": work, **fields})["idea"]["id"]

    fire = idea(**ideas["fire"], cloud=False)
    wick = idea(**ideas["wick"], parent=fire)
    lamp = idea(**ideas["lamp"], parent=fire, source=sample_data["source"]["title"])
    idea(**ideas["swing"], parent=lamp)
    guest = idea(**ideas["guest"], cloud=False)
    idea(**ideas["fish"], parent=guest, key=key_of[proposed])
    at_section = idea(**ideas["feather"], key=key_of[revised])
    idea(**ideas["surge"])
    idea(**ideas["letter"])
    me.rpc("idea.link", {"from": wick, "to": lamp, "kind": "leads_to", "note": ideas["link_note"]})
    me.rpc("idea.link", {"from": guest, "to": fire, "kind": "contradicts"})
    values.update(idea=at_section, branch=fire)
    print("Einfälle angelegt (zwei Hauptthemen mit Unterknoten, einer am Abschnitt, zwei in der Wolke).")

    # Assistants without a model: through the MCP access of the own server, under the name of the role „Lektor“.
    posts = sample_data["posts"]
    assistant = Mcp(base, sample_data["mcp_client"])
    first_post = assistant.call("add_post", {
        "work": work, "key": key_of[revised], "persona": "lektor", "body": posts["first"],
    })
    values["post"] = first_post["id"]
    me.rpc("post.add", {"work": work, "key": key_of[revised], "body": posts["human"]})
    assistant.call("add_post", {
        "work": work, "key": key_of[revised], "persona": "lektor", "body": posts["second"],
    })
    proposed_revision = assistant.call("propose_revision", {
        "work": work, "key": key_of[proposed], "body": sample_data["proposal"], "persona": "lektor",
        "note": posts["proposal_note"],
    })
    values["rev_proposal"] = proposed_revision["rev"]
    asked = assistant.call("outline_rename", {
        "work": work, "key": key_of[farewell], "title": posts["rename_title"], "note": posts["rename_note"],
    })
    values["request"] = asked["request"]["id"]
    thread = me.rpc("thread.post", {"work": work, "message": posts["thread_question"]})
    if thread.get("answers") or thread.get("errors"):
        fail("thread.post hat ein Modell angesprochen; das darf beim Aufbau nicht geschehen.")
    values["thread"] = thread["thread"]
    assistant.call("chat", {"work": work, "thread": thread["thread"], "message": posts["thread_answer"]})
    me.rpc("thread.post", {"work": work, "thread": thread["thread"], "message": posts["thread_follow_up"]})
    print("Beiträge, Vorschlag und Gliederungsantrag eines Assistenten (über MCP, ohne Modell), ein Faden.")

    # Source for the literature: a small invented EPUB, read locally, only into the library.
    draft = me.rpc("source_import.preview", {
        "file_name": sample_data["source"]["file_name"], "content": base64.b64encode(source_epub(sample_data)).decode(),
    })
    adopted = me.rpc("source_import.apply", {"draft": draft["draft"], "target": "library"})
    values["title"] = adopted["library_title"]

    # Read in the throwaway work as Markdown: the answer is the report (an image is missing). It is deleted later.
    throwaway_data = sample_data["throwaway"]
    status, headers, content = me.upload("/werke/import", {"title": throwaway_data["title"]}, "file",
                                         throwaway_data["file_name"], throwaway_data["markdown"].encode(),
                                         "text/markdown")
    if status == 200:
        answers["einlesen-bericht"] = content
    else:
        fail(f"Einlesen der Markdown-Datei: Status {status} statt des Berichts (200).")
    found = re.search(r"/\?work=(\d+)", content.decode("utf-8", "replace"))
    throwaway = next((w for w in me.rpc("work.list", {})["works"] if found and w["id"] == int(found.group(1))), None)
    if not throwaway or throwaway["id"] in (work, sample):
        fail("Der Bericht des Einlesens nennt das Wegwerf-Werk nicht.")

    # Previews of reading in from outputs of the own server.
    def export(path, what):
        status, _, content = me.get(path)
        if status != 200 or not content:
            fail(f"{what}: {path} antwortet mit {status}.")
        return content

    def preview(path, file_name, content, mime, name, fields=None, file_field="file"):
        status, headers, page = me.upload(path, fields or {}, file_field, file_name, content, mime)
        if status == 303 and headers.get("Location"):
            # Some forms create a draft and redirect to its page: then this page counts.
            status, _, page = me.get(headers["Location"])
        if status != 200 or not page:
            fail(f"Vorschau {name}: {path} antwortet mit {status}: {page[:300].decode('utf-8', 'replace')}")
        answers[name] = page

    stem = sample_data["export_stem"]
    docx = export(f"/werke/{work}/word.docx", "Word-Export")
    preview("/werke/import/word/vorschau", f"{stem}.docx", docx,
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document", "einlesen-word-vorschau")
    exchange = export(f"/werke/{work}/austausch.baruch", "Austauschdatei")
    preview("/austausch/vorschau", f"{stem}.baruch", exchange, "application/octet-stream",
            "austausch-vorschau", file_field="datei")
    pdf = export(f"/werke/{work}/pdf", "Buch als PDF")
    preview("/werke/import/quelle/vorschau", f"{stem}.pdf", pdf, "application/pdf",
            "einlesen-pdf-vorschau", {"way": "local"})
    print("Vorschauen des Einlesens (Word, Austauschdatei, PDF) aus Ausgaben des eigenen Servers gespeichert.")

    # Delete the throwaway work: it lies in the archive afterwards.
    me.rpc("work.delete", {"work": throwaway["id"], "confirm_title": throwaway["title"]})
    print(f"Wegwerf-Werk „{throwaway['title']}“ eingelesen und gelöscht (Archiv).")
    return values, answers


# ---------------------------------------------------------------------------------------------------------------
# Stage „konten“: author, Lektor, share, findings
# ---------------------------------------------------------------------------------------------------------------

def session_from(headers):
    for value in headers.get_all("Set-Cookie") or []:
        found = re.match(rf"{SESSION_COOKIE}=([^;]+)", value)
        if found and found.group(1):
            return found.group(1)
    return None


def build_accounts(base, values, lang=DEFAULT_LANG):
    """Legt die Konten an (danach verlangt jede Seite eine Anmeldung) und lässt den Lektor Befunde, einen Vorschlag
    und einen Beitrag schreiben. Liefert die Sitzungen {"autorin": …, "lektor": …}; sie werden nie ausgegeben.
    Die Passwörter entstehen je Lauf zufällig und verlassen das Skript nicht."""
    sample_data = SAMPLES[lang]
    work = values["work"]
    passwords = {"autorin": secrets.token_urlsafe(18), "lektor": secrets.token_urlsafe(18)}
    open_door = Client(base, lang=lang)
    status, headers, _ = open_door.form("/einrichten", {
        "name": AUTHOR["name"], "display": AUTHOR["display"],
        "password": passwords["autorin"], "password2": passwords["autorin"],
    })
    author_session = session_from(headers)
    if status != 303 or not author_session:
        fail(f"Konten einrichten: Status {status}, keine Sitzung.")
    author = Client(base, author_session, lang)
    status, headers, _ = author.form("/einstellungen/konten/act", {
        "action": "create", "name": LEKTOR["name"], "display": LEKTOR["display"],
        "password": passwords["lektor"], "password2": passwords["lektor"],
    })
    if status != 303 or "ok=konto_angelegt" not in (headers.get("Location") or ""):
        fail(f"Lektor-Konto anlegen: Status {status}, Ziel {headers.get('Location')}.")
    accounts = author.rpc("account.list", {})["accounts"]
    lektor_id = next(a["id"] for a in accounts if a["kind"] == "lektor")
    author.rpc("member.grant", {"work": work, "account": lektor_id})
    status, headers, _ = open_door.form("/anmelden", {"name": LEKTOR["name"], "password": passwords["lektor"],
                                                      "next": "/"})
    lektor_session = session_from(headers)
    if status != 303 or not lektor_session:
        fail(f"Als Lektor anmelden: Status {status}, keine Sitzung.")
    lektor = Client(base, lektor_session, lang)
    for finding in sample_data["findings"]:
        fields = {name: value for name, value in finding.items() if name != "at"}
        if "at" in finding:
            fields["key"] = values[finding["at"]]
        lektor.rpc("finding.add", {"work": work, **fields})
    proposed = lektor.rpc("revision.propose", {"work": work, "key": values["key_night"],
                                               "body": sample_data["lektor_proposal"],
                                               "note": sample_data["lektor_proposal_note"]})
    values["rev_lektor"] = proposed["rev"]
    lektor.rpc("post.add", {"work": work, "key": values["key_night"], "body": sample_data["lektor_post"]})
    print(f"Konten angelegt ({AUTHOR['display']} als Autorin, {LEKTOR['display']} als Lektor), Werk freigegeben, "
          f"{len(sample_data['findings'])} Befunde, ein Vorschlag und ein Beitrag des Lektors.")
    return {"autorin": author_session, "lektor": lektor_session}


# ---------------------------------------------------------------------------------------------------------------
# Mediator: http.server on 127.0.0.1:0 in front of the throwaway server
# ---------------------------------------------------------------------------------------------------------------

PIXEL = base64.b64decode("R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7")
# This long a late-loaded image holds up the load event that `firefox --screenshot` waits for (milliseconds):
# click after 500 ms, measuring of the viewport after 2500 ms.
HOLD_MS = 3500
CLICK_AFTER_MS = 500
MEASURE_AFTER_MS = 2500
HOP_BY_HOP = {"connection", "keep-alive", "transfer-encoding", "content-length", "proxy-connection", "upgrade", "te"}


class Relay(http.server.ThreadingHTTPServer):
    """Der Vermittler. Je Aufnahme ein Rezept (Cookies, zu öffnende <details>, Klicks, Ausschnitt, Sitzung); die
    Adresse der Seite trägt `_bild=<Aufnahme>` und wählt es damit aus. Alle weiteren Anfragen der Seite (Skripte,
    Stile, RPC der Widgets) laufen mit denselben Cookies durch."""

    daemon_threads = True
    allow_reuse_address = True

    def __init__(self, upstream, lang=DEFAULT_LANG):
        super().__init__(("127.0.0.1", 0), RelayHandler)
        self.upstream = upstream
        # Language of the pictures: cookie baruch_lang in every recipe, Accept-Language on every request upstream.
        self.lang = lang
        self.base = f"http://127.0.0.1:{self.server_address[1]}"
        self.recipes = {}
        self.current = None
        self.answers = {}
        self.sessions = {}
        self.measures = {}
        self.applied = {}
        threading.Thread(target=self.serve_forever, daemon=True).start()

    def recipe_for(self, shot, picture, theme, values):
        cookies = dict(DEFAULT_COOKIES)
        cookies["baruch_work"] = str(values["work"])
        cookies[LANG_COOKIE] = self.lang
        recipe = {"cookies": cookies, "open": [], "click": [], "session": None, "measure": None}
        for kind, value in picture["schritt"]:
            if kind == "cookie":
                name, _, content = value.partition("=")
                if content:
                    cookies[name] = fill(content, values)
                else:
                    cookies.pop(name, None)
            elif kind == "offen":
                recipe["open"].append(value)
            elif kind == "klick":
                recipe["click"].append(value)
            elif kind == "sitzung":
                recipe["session"] = value
        if theme == "dunkel":
            cookies["baruch_theme"] = "dark"
        if picture["ausschnitt"] and picture["ausschnitt"].startswith("css:"):
            recipe["measure"] = picture["ausschnitt"][len("css:"):]
        self.recipes[shot] = recipe
        self.applied[shot] = []
        self.measures.pop(shot, None)
        return recipe

    def address(self, picture, shot, values):
        """Adresse der Aufnahme am Vermittler: die Adresse des Eintrags mit `_bild=<Aufnahme>` vor einem Anker."""
        path, mark, anchor = fill(picture["adresse"], values).partition("#")
        return f"{self.base}{path}{'&' if '?' in path else '?'}_bild={shot}{mark}{anchor}"


def shot_name(file):
    return Path(file).stem


class RelayHandler(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.0"

    def log_message(self, *_):
        pass

    def do_GET(self):
        self.relay()

    do_POST = do_GET
    do_HEAD = do_GET

    def plain(self, status, body, kind="text/plain; charset=utf-8"):
        self.send_response(status)
        self.send_header("Content-Type", kind)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        if self.command != "HEAD":
            self.wfile.write(body)

    def relay(self):
        relay = self.server
        split = urllib.parse.urlsplit(self.path)
        query = urllib.parse.parse_qsl(split.query, keep_blank_values=True)
        picked = [value for name, value in query if name == "_bild"]
        if picked:
            relay.current = picked[-1]
            query = [(name, value) for name, value in query if name != "_bild"]
        shot = relay.current
        recipe = relay.recipes.get(shot) or {"cookies": {}, "open": [], "click": [], "session": None, "measure": None}

        if split.path == "/_warte":
            wait = min(10000, int(dict(query).get("ms", "0") or 0))
            time.sleep(wait / 1000)
            return self.plain(200, PIXEL, "image/gif")
        if split.path == "/_mass":
            fields = dict(query)
            if fields.get("fehlt"):
                relay.applied.setdefault(fields.get("bild", ""), []).append(f"FEHLT im Browser: {fields['fehlt']}")
            else:
                try:
                    relay.measures[fields["bild"]] = tuple(round(float(fields[n])) for n in ("x", "y", "w", "h"))
                except (KeyError, ValueError):
                    pass
            return self.plain(200, PIXEL, "image/gif")
        if split.path.startswith("/_antwort/"):
            stored = relay.answers.get(split.path[len("/_antwort/"):])
            if stored is None:
                return self.plain(404, "Diese Antwort hat der Aufbau nicht gespeichert.".encode())
            page = self.transform(stored, shot, recipe) if picked else stored
            return self.page(200, [("Content-Type", "text/html; charset=utf-8")], page, recipe, bool(picked))

        target = urllib.parse.urlsplit(relay.upstream)
        path = split.path + ("?" + urllib.parse.urlencode(query) if query else "")
        headers = {}
        for name, value in self.headers.items():
            low = name.lower()
            if low in HOP_BY_HOP or low in ("host", "cookie", "accept-encoding", "origin", "referer"):
                continue
            headers[name] = value
        headers["Host"] = target.netloc
        headers["Accept-Language"] = relay.lang
        # Host, Origin and Referer carry the address of the server: otherwise guard::same_origin refuses with 403.
        if self.headers.get("Origin"):
            headers["Origin"] = relay.upstream
        if self.headers.get("Referer"):
            headers["Referer"] = self.headers["Referer"].replace(relay.base, relay.upstream)
        cookies = {}
        for part in (self.headers.get("Cookie") or "").split(";"):
            name, mark, value = part.strip().partition("=")
            if mark and name not in MANAGED_COOKIES:
                cookies[name] = value
        cookies.update(recipe["cookies"])
        if recipe["session"]:
            token = relay.sessions.get(recipe["session"])
            if token:
                cookies[SESSION_COOKIE] = token
        if cookies:
            headers["Cookie"] = "; ".join(f"{name}={value}" for name, value in cookies.items())
        length = int(self.headers.get("Content-Length") or 0)
        body = self.rfile.read(length) if length else None

        connection = http.client.HTTPConnection(target.hostname, target.port, timeout=600)
        try:
            connection.request(self.command, path, body=body, headers=headers)
            response = connection.getresponse()
            passed = []
            for name, value in response.getheaders():
                low = name.lower()
                if low in HOP_BY_HOP:
                    continue
                if low == "set-cookie" and value.startswith(SESSION_COOKIE + "="):
                    continue  # the session stays in the mediator, it never gets into the profile of the browser
                if low == "location":
                    value = value.replace(relay.upstream, "")
                    if picked:  # the redirect stays the same capture
                        before, mark, anchor = value.partition("#")
                        value = f"{before}{'&' if '?' in before else '?'}_bild={shot}{mark}{anchor}"
                passed.append((name, value))
            kind = response.getheader("Content-Type") or ""
            if picked and "text/html" in kind:
                page = self.transform(response.read(), shot, recipe)
                return self.page(response.status, passed, page, recipe, True)
            self.send_response(response.status)
            for name, value in passed:
                self.send_header(name, value)
            self.end_headers()
            if self.command != "HEAD":
                while True:
                    piece = response.read1(65536)
                    if not piece:
                        break
                    self.wfile.write(piece)
                    self.wfile.flush()
        except (BrokenPipeError, ConnectionResetError):
            pass
        except OSError as error:
            try:
                self.plain(502, f"Vermittler: {error}".encode())
            except OSError:
                pass
        finally:
            connection.close()

    def page(self, status, headers, page, recipe, is_shot):
        self.send_response(status)
        for name, value in headers:
            self.send_header(name, value)
        if is_shot:
            # Cookies to the browser as well: the widgets read document.cookie (otherwise the theme would switch back).
            for name in MANAGED_COOKIES:
                if name == SESSION_COOKIE:
                    continue
                if name in recipe["cookies"]:
                    self.send_header("Set-Cookie", f"{name}={recipe['cookies'][name]}; Path=/; SameSite=Lax")
                else:
                    self.send_header("Set-Cookie", f"{name}=; Path=/; Max-Age=0; SameSite=Lax")
        self.send_header("Content-Length", str(len(page)))
        self.send_header("Cache-Control", "no-store")
        self.end_headers()
        if self.command != "HEAD":
            self.wfile.write(page)

    def transform(self, page, shot, recipe):
        """Öffnet <details>, fügt Klick- und Mess-Skript samt Warte-Bild ein. Was gewirkt hat, steht danach in
        relay.applied[<Aufnahme>] (für --pruefen)."""
        notes = self.server.applied.setdefault(shot, [])
        text = page.decode("utf-8", "replace")
        for marker in recipe["open"]:
            pattern = re.compile(r"<details\b[^>]*" + re.escape(marker) + r"[^>]*>")
            count = 0

            def opened(match):
                nonlocal count
                count += 1
                tag = match.group(0)
                return tag if re.search(r"\sopen[\s>=]", tag) else tag[:-1] + " open>"

            text = pattern.sub(opened, text)
            notes.append(f"offen:{marker} ×{count}" if count else f"OHNE WIRKUNG offen:{marker}")
        if recipe["click"] or recipe["measure"]:
            for selector in recipe["click"] + ([recipe["measure"]] if recipe["measure"] else []):
                missing = [token for token in selector_tokens(selector) if token not in text]
                if missing:
                    notes.append(f"OHNE ZIEL {selector} (im HTML fehlt: {', '.join(missing)})")
            script = (
                "<script>(function(){var bild=%s,klicks=%s,mass=%s;"
                "function melde(q){new Image().src='/_mass?bild='+encodeURIComponent(bild)+'&'+q;}"
                "document.addEventListener('DOMContentLoaded',function(){"
                "klicks.forEach(function(s,i){setTimeout(function(){var e=document.querySelector(s);"
                "if(e){e.click();}else{melde('fehlt='+encodeURIComponent(s));}},%d+i*400);});"
                "if(mass){setTimeout(function(){var e=document.querySelector(mass);"
                "if(!e){melde('fehlt='+encodeURIComponent(mass));return;}var r=e.getBoundingClientRect();"
                "melde('x='+(r.left+window.scrollX)+'&y='+(r.top+window.scrollY)+'&w='+r.width+'&h='+r.height);},%d);}"
                "});})();</script>"
                '<img src="/_warte?ms=%d" alt="" width="1" height="1" style="position:fixed;left:-9px;top:-9px">'
            ) % (json.dumps(shot), json.dumps(recipe["click"]), json.dumps(recipe["measure"]),
                 CLICK_AFTER_MS, MEASURE_AFTER_MS, HOLD_MS)
            if "</body>" in text:
                text = text.replace("</body>", script + "</body>", 1)
                notes.append("Skript eingefügt")
            else:
                notes.append("OHNE WIRKUNG Skript (kein </body>)")
        return text.encode("utf-8")


def selector_tokens(selector):
    """Grobe Probe ohne Browser: Bestandteile des Selektors, die im HTML der Seite vorkommen müssen
    (Attributnamen, Klassen, ids). Ob der Klick wirkt, zeigt erst das Foto."""
    tokens = []
    for attribute, value in re.findall(r"\[([\w-]+)(?:[~|^$*]?=\"?([^\"\]]*)\"?)?\]", selector):
        tokens.append(attribute)
        if value:
            tokens.append(value)
    plain = re.sub(r"\[[^\]]*\]", "", selector)
    tokens += re.findall(r"[.#]([\w-]+)", plain)
    return tokens


# ---------------------------------------------------------------------------------------------------------------
# Checking, photographing, post-processing
# ---------------------------------------------------------------------------------------------------------------

def fetch_through(relay, url):
    """Ruft eine Aufnahme über den Vermittler ab und folgt Weiterleitungen: (Status, Inhalt, Weg)."""
    trail = []
    for _ in range(6):
        split = urllib.parse.urlsplit(url)
        path = split.path + ("?" + split.query if split.query else "")
        status, headers, content = request(relay.base, "GET", path, headers={"Accept": "text/html"})
        if status in (301, 302, 303, 307, 308) and headers.get("Location"):
            trail.append(f"{status} → {headers['Location'].split('_bild=')[0].rstrip('?&')}")
            url = urllib.parse.urljoin(relay.base + path, headers["Location"])
            continue
        return status, content, trail
    return 0, b"", trail + ["zu viele Weiterleitungen"]


def check(relay, pictures, values):
    """--pruefen: jede Aufnahme über den Vermittler abrufen. Verlangt 200, Inhalt, die Texte aus `erwartet` und dass
    jeder Schritt gewirkt hat. Liefert die Zahl der Fehler."""
    failures = 0
    for picture in pictures:
        for file, theme in variants(picture):
            shot = shot_name(file)
            relay.recipe_for(shot, picture, theme, values)
            status, content, trail = fetch_through(relay, relay.address(picture, shot, values))
            text = content.decode("utf-8", "replace")
            problems = []
            if status != 200:
                problems.append(f"Status {status}")
            if not content.strip():
                problems.append("leerer Inhalt")
            for wanted in picture["erwartet"]:
                wanted = fill(wanted, values) if "{" in wanted else wanted
                if wanted not in text and html.escape(wanted) not in text:
                    problems.append(f"erwarteter Text fehlt: „{wanted}“")
            notes = relay.applied.get(shot, [])
            problems += [note for note in notes if note.startswith(("OHNE", "FEHLT"))]
            if theme == "dunkel" and 'data-theme="dark"' not in text:
                problems.append("dunkle Fassung: die Seite trägt nicht data-theme=\"dark\" (Cookie kam nicht an)")
            if any(token and token in text for token in relay.sessions.values()):
                problems.append("die Seite enthält ein Sitzungs-Token")
            if "127.0.0.1:1234" in text or "localhost:1234" in text:
                problems.append("die Seite nennt die Vorgabe-Adresse eines Modell-Dienstes (127.0.0.1:1234)")
            # The language of the interface must be the language of the pictures (the cookie arrived).
            spoken = re.search(r"<html\b[^>]*\blang=\"([^\"]*)\"", text)
            if spoken and spoken.group(1) != relay.lang:
                problems.append(f"Sprache der Oberfläche ist „{spoken.group(1)}“ statt „{relay.lang}“ "
                                "(<html lang>; Cookie baruch_lang kam nicht an)")
            # Neither the home folder nor the login name of the user may stand in a picture.
            private = private_parts(*[part for part in re.findall(r"/[^\s\"'<>]+", text) if len(part) > 3])
            if private:
                problems.append("die Seite nennt den Heimatordner oder den Benutzer: "
                                + ", ".join(sorted(set(private))[:3]))
            marks = "; ".join([n for n in notes if not n.startswith(("OHNE", "FEHLT"))] + trail)
            print(f"  {status}  {len(content):>8} Bytes  {file:<46} {fill(picture['adresse'], values)}"
                  + (f"\n       {'':>8}        {marks}" if marks else ""))
            for problem in problems:
                print(f"       FEHLER: {problem}")
            failures += 1 if problems else 0
    return failures


def probe_relay(relay):
    """--pruefen: belegt, was der Vermittler außer den Seiten leisten muss. Liefert die Zahl der Fehler."""
    print("\n== Probe des Vermittlers ==")
    failures = 0

    def line(ok, what):
        nonlocal failures
        failures += 0 if ok else 1
        print(f"  {'ok    ' if ok else 'FEHLER'}  {what}")

    status, _, content = request(relay.base, "GET", "/static/app.css")
    line(status == 200 and content, f"Stylesheet über den Vermittler: Status {status}, {len(content)} Bytes")
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": "work.list", "params": {}}).encode()
    browser = {"Content-Type": "application/json", "Origin": relay.base, "Referer": relay.base + "/"}
    status, _, content = request(relay.base, "POST", "/rpc", body, browser)
    line(status == 200 and b'"works"' in content,
         f"RPC eines Widgets über den Vermittler (Origin = Adresse des Vermittlers): Status {status}")
    direct, _, _ = request(relay.upstream, "POST", "/rpc", body, browser)
    line(direct == 403, f"Gegenprobe ohne Vermittler mit derselben Origin: Status {direct} (403 = der Server lehnt "
                        "fremde Herkunft ab, der Vermittler muss Host und Origin also umschreiben)")
    started = time.monotonic()
    status, headers, content = request(relay.base, "GET", "/_warte?ms=300")
    waited = time.monotonic() - started
    line(status == 200 and content == PIXEL and waited >= 0.3,
         f"Warte-Bild (hält das load-Ereignis auf): Status {status} nach {waited:.2f} s")
    request(relay.base, "GET", "/_mass?bild=probe&x=10.4&y=0&w=1400&h=171.6")
    line(relay.measures.pop("probe", None) == (10, 0, 1400, 172), "Messung eines Ausschnitts kommt an und wird gerundet")
    status, headers, _ = request(relay.base, "GET", "/werke?_bild=probe")
    cookies = headers.get_all("Set-Cookie") or []
    line(status == 200 and any(c.startswith("baruch_theme=;") for c in cookies)
         and not any(c.startswith(SESSION_COOKIE + "=") for c in cookies),
         "Seite ohne Rezept: verwaltete Cookies werden im Browser gelöscht, keine Sitzung im Set-Cookie")
    status, headers, _ = request(relay.base, "GET", "/_antwort/gibt-es-nicht?_bild=probe")
    line(status == 404, f"unbekannte gespeicherte Antwort: Status {status}")
    relay.current = None
    return failures


def find_magick():
    return shutil.which("magick") or shutil.which("convert")


def postprocess(magick, file, cut, max_width, palette):
    """Ausschnitt, Breite begrenzen, Metadaten entfernen, Palette (256 Farben) und stärkste PNG-Kompression."""
    command = [magick, str(file)]
    if cut:
        command += ["-crop", cut, "+repage"]
    command += ["-resize", f"{max_width}x>", "-strip"]
    if palette:
        command += ["+dither", "-colors", "256"]
    command += ["-define", "png:compression-level=9"]
    command.append(("PNG8:" if palette else "") + str(file))
    subprocess.run(command, check=True)


def shoot(relay, pictures, values, target, temp, options):
    """Fotos mit Firefox (headless, je Aufnahme ein eigenes Wegwerf-Profil). Nur der Integrator, nie parallel zu
    Testläufen."""
    firefox = shutil.which("firefox")
    if not firefox:
        fail("firefox nicht gefunden.")
    magick = find_magick()
    if not magick:
        print("HINWEIS: magick/convert fehlt – die Bilder bleiben unbearbeitet (kein Ausschnitt, keine Palette). "
              "So überschreiten sie die Größengrenze vermutlich; ImageMagick installieren und wiederholen.")
    for picture in pictures:
        width, height = picture["fenster"].split("x")
        for file, theme in variants(picture):
            shot = shot_name(file)
            relay.recipe_for(shot, picture, theme, values)
            out = target / file
            out.parent.mkdir(parents=True, exist_ok=True)
            with tempfile.TemporaryDirectory(prefix="baruch-fotos-profil-", dir=temp) as profile:
                prefs = ['user_pref("browser.shell.checkDefaultBrowser", false);',
                         'user_pref("datareporting.policy.dataSubmissionEnabled", false);',
                         'user_pref("toolkit.telemetry.reportingpolicy.firstRun", false);',
                         'user_pref("app.update.enabled", false);',
                         'user_pref("network.captive-portal-service.enabled", false);',
                         'user_pref("network.connectivity-service.enabled", false);']
                if theme == "dunkel":
                    prefs += ['user_pref("layout.css.prefers-color-scheme.content-override", 0);',
                              'user_pref("ui.systemUsesDarkTheme", 1);']
                else:
                    prefs += ['user_pref("layout.css.prefers-color-scheme.content-override", 1);',
                              'user_pref("ui.systemUsesDarkTheme", 0);']
                if "ohne-js" in picture["braucht"]:
                    prefs.append('user_pref("javascript.enabled", false);')
                # Accept-Language of the browser = language of the pictures (the mediator sets it upstream as well).
                prefs.append(f'user_pref("intl.accept_languages", "{relay.lang}");')
                (Path(profile) / "user.js").write_text("\n".join(prefs) + "\n", encoding="utf-8")
                subprocess.run(
                    [firefox, "--headless", "--no-remote", "--profile", profile,
                     f"--window-size={width},{height}", "--screenshot", str(out),
                     relay.address(picture, shot, values)],
                    check=True, env=clean_env(), timeout=240, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                )
            notes = [n for n in relay.applied.get(shot, []) if n.startswith(("OHNE", "FEHLT"))]
            cut = picture["ausschnitt"]
            if cut and cut.startswith("css:"):
                box = relay.measures.get(shot)
                if box and box[2] > 0 and box[3] > 0:
                    cut = f"{box[2]}x{box[3]}+{max(0, box[0])}+{max(0, box[1])}"
                    height = int(picture["fenster"].split("x")[1])
                    if box[1] + box[3] > height:
                        notes.append(f"Ausschnitt {cut} reicht unter das Fenster ({picture['fenster']}) – "
                                     "das Fenster höher wählen")
                else:
                    notes.append(f"Ausschnitt {cut}: keine Messung aus dem Browser – Bild bleibt ganz")
                    cut = None
            if magick and out.is_file():
                postprocess(magick, out, cut, options.breite, not options.ohne_palette)
            size = out.stat().st_size if out.is_file() else 0
            print(f"  {out.relative_to(REPO) if out.is_relative_to(REPO) else out}  "
                  f"({picture['fenster'].replace('x', '×')}, {theme}, {size // 1024} KB)")
            for note in notes:
                print(f"       ACHTUNG: {note}")


def report_size(target, lang=None):
    """Size of the pictures of a language (its folder without the folders of the other languages) or, for a folder
    given with --ziel, of the whole folder."""
    if lang:
        total = sum(path.stat().st_size for path in language_files(lang))
    else:
        total = sum(path.stat().st_size for path in target.rglob("*") if path.is_file()) if target.is_dir() else 0
    print(f"Größe von {target}: {total / 1024 / 1024:.2f} MB von höchstens {SIZE_LIMIT // 1024 // 1024} MB"
          + (" – ÜBERSCHRITTEN" if total > SIZE_LIMIT else "."))


# ---------------------------------------------------------------------------------------------------------------
# Invocation
# ---------------------------------------------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(
        description="Bildschirmfotos der Webseite und der Hilfe aus einem erfundenen Werk.")
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--liste", action="store_true",
                      help="ohne Server: jedes Bild mit Adresse, Fenster, Thema und Schritten")
    mode.add_argument("--abgleich", action="store_true",
                      help="ohne Server: verwendete Namen gegen die Liste, fehlende Dateien, Gesamtgröße")
    mode.add_argument("--pruefen", action="store_true",
                      help="Server starten, Beispielwerk aufbauen, jede Adresse über den Vermittler abrufen")
    mode.add_argument("--halten", action="store_true",
                      help="wie --pruefen, aber je Stufe bis zur Eingabetaste offen halten")
    mode.add_argument("--fotos", action="store_true",
                      help="Server starten und mit Firefox fotografieren (nur der Integrator)")
    parser.add_argument("--nur", help="nur diese Bilder: NAME[,NAME] (Namen aus --liste)")
    parser.add_argument("--sword", help="Ordner mit Bibelmodulen (mods.d, modules) für Bilder mit braucht sword")
    parser.add_argument("--server", help="Pfad zu baruch-server (Vorgabe: target/release, dann target/debug)")
    parser.add_argument("--lang", choices=LANGS, default=DEFAULT_LANG,
                        help="Sprache der Bilder: Oberfläche, Werksprache und erfundene Daten; Ziel de site/static/bilder, "
                             "en site/static/bilder/en (Vorgabe de)")
    parser.add_argument("--ziel", help="Ordner für die Bilder (Vorgabe: Bildordner der Sprache, site/static/bilder "
                                       "bzw. site/static/bilder/<lang>)")
    parser.add_argument("--temp", help="Ordner auf einer Platte für Wegwerf-Datenordner und Firefox-Profile "
                                       "(Vorgabe: $TMPDIR – oft Arbeitsspeicher)")
    parser.add_argument("--breite", type=int, default=1400, help="größte Breite der Bilder in Pixeln (Vorgabe 1400)")
    parser.add_argument("--ohne-palette", action="store_true", help="Bilder nicht auf 256 Farben verringern")
    args = parser.parse_args()
    lang = args.lang
    target = Path(args.ziel) if args.ziel else target_dir(lang)
    sample_data = SAMPLES[lang]

    pictures = load_pictures(lang)
    if args.nur:
        wanted = [name.strip() for name in args.nur.split(",") if name.strip()]
        known = {p["name"] for p in pictures}
        strangers = [name for name in wanted if name not in known]
        if strangers:
            fail(f"--nur: unbekannte Namen: {', '.join(strangers)}")
        pictures = [p for p in pictures if p["name"] in wanted]

    if args.liste:
        print(f"Sprache {lang}: {len(pictures)} Bilder, {sum(len(variants(p)) for p in pictures)} Aufnahmen nach "
              f"{target.relative_to(REPO) if target.is_relative_to(REPO) else target}, Beispielwerk "
              f"{sample_data['work_file'].name}. Platzhalter: {{work}} = Nummer des Beispielwerks, {{key}} = Abschnitt "
              f"„{sample_data['revised']}“, weitere siehe site/tools/README.md.")
        show_list(pictures)
        return
    if args.abgleich:
        reconcile(load_pictures(lang), lang)
        return

    skipped = [p for p in pictures if "sword" in p["braucht"] and not args.sword]
    pictures = [p for p in pictures if p not in skipped]
    for picture in skipped:
        print(f"ÜBERSPRUNGEN (braucht --sword ORDNER): {picture['name']}")
    stages = {stage: [p for p in pictures if p["stufe"] == stage] for stage in ("offen", "konten")}

    server = find_server(args.server)
    if args.temp:
        Path(args.temp).expanduser().mkdir(parents=True, exist_ok=True)
    temp = str(Path(args.temp).expanduser().resolve()) if args.temp else None
    if args.fotos:
        sword_path = Path(args.sword).expanduser().resolve() if args.sword else None
        private = private_parts(temp or tempfile.gettempdir(), sword_path)
        if private:
            fail("Diese Pfade stünden in den Bildern und nennen den Benutzer: " + ", ".join(private)
                 + ". Für --fotos einen neutralen Ordner auf einer Platte nehmen, etwa --temp /var/tmp/baruch-hilfe "
                 "(und die Bibelmodule dorthin kopieren).")
    data = Path(tempfile.mkdtemp(prefix="baruch-fotos-", dir=temp))
    if FORBIDDEN_DATA in data.resolve().parents or data.resolve() == FORBIDDEN_DATA:
        fail("Der Wegwerf-Ordner läge im Datenordner des Dienstes; abgebrochen.")
    process = relay = None
    failures = 0
    try:
        print(f"Server: {server}\nWegwerf-Datenordner: {data}")
        print(f"Sprache der Bilder: {lang} (Cookie {LANG_COOKIE}={lang}, Accept-Language {lang}), Ziel {target}")
        work = import_work(server, data, sample_data["work_file"])
        point_connections_nowhere(data)
        process, base = start_server(server, data)
        relay = Relay(base, lang)
        print(f"Vermittler: {relay.base} → {base}")

        def run(stage, title):
            nonlocal failures
            listed = stages[stage]
            print(f"\n== Stufe „{stage}“: {title} – {len(listed)} Bilder ==")
            if args.halten:
                for picture in listed:
                    for file, theme in variants(picture):
                        relay.recipe_for(shot_name(file), picture, theme, values)
                        print(f"  {file:<46} {relay.address(picture, shot_name(file), values)}")
                input("Server und Vermittler laufen. Eingabetaste geht weiter … ")
            elif args.pruefen:
                failures += check(relay, listed, values)
            else:
                shoot(relay, listed, values, target, temp, args)

        values, answers = build_open(base, work, data, args.sword, lang)
        relay.answers.update(answers)
        print("Platzhalter: " + ", ".join(f"{{{name}}}={value}" for name, value in sorted(values.items())))
        run("offen", "ohne Konten")
        if args.pruefen and not args.nur:
            failures += probe_relay(relay)
        if stages["konten"] or args.halten:
            relay.sessions.update(build_accounts(base, values, lang))
            run("konten", "mit Konten, jede Seite verlangt eine Anmeldung")
        if args.pruefen:
            total = sum(len(variants(p)) for p in pictures)
            if failures:
                print(f"\n{failures} von {total} Aufnahmen mit Fehlern.")
            else:
                print(f"\nAlle {total} Aufnahmen ({len(pictures)} Bilder) antworten mit 200 und Inhalt; "
                      "jeder Schritt hat gewirkt.")
        if args.fotos:
            report_size(target, lang if not args.ziel else None)
    finally:
        if relay is not None:
            relay.shutdown()
            relay.server_close()
        if process is not None:
            stop_server(process)
        shutil.rmtree(data, ignore_errors=True)
        print(f"Server (PID {process.pid if process else '–'}) beendet, Wegwerf-Datenordner gelöscht: "
              f"{'ja' if not data.exists() else 'NEIN – ' + str(data)}")
    if failures:
        sys.exit(1)


if __name__ == "__main__":
    try:
        main()
    except BrokenPipeError:
        # Output into a program that stops reading earlier (e.g. „| head“): end quietly.
        os.dup2(os.open(os.devnull, os.O_WRONLY), sys.stdout.fileno())
        sys.exit(1)
