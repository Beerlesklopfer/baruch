#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Bildschirmfotos für die Webseite: die Bilder der Hilfe (site/static/bilder/hilfe/<name>.png, Liste in
site/data/hilfebilder.toml) und die sieben Bilder der Seite „Funktionen“ (site/static/bilder/*.png) – alle aus
einem frei erfundenen Werk („Ein Jahr auf Mövenoog“, site/tools/beispielwerk.typ) in einem Wegwerf-Datenordner.

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
WORK_FILE = TOOLS / "beispielwerk.typ"
LIST_FILE = REPO / "site" / "data" / "hilfebilder.toml"
CONTENT = REPO / "site" / "content"
TARGET = REPO / "site" / "static" / "bilder"
HELP_DIR = "hilfe"
# Alle Bilder zusammen (site/static/bilder) bleiben unter dieser Grenze (Vertrag Runde 37, §8).
SIZE_LIMIT = 20 * 1024 * 1024
WINDOW = "1400x860"
# Unter 1600 px zeichnet der Laptop-Modus („automatisch“) das Menüband kompakt (COMPACT_BELOW_PX in
# crates/baruch-server/src/laptop.rs); die Breite misst das Skript der Seite.
LAPTOP_WINDOW = "1366x768"
# Die Verbindungen des Wegwerf-Servers zeigen auf den Discard-Port: Dort antwortet niemand.
NOWHERE = "http://127.0.0.1:9"
# Adresse und Datenordner des Dienstes beim Nutzer: nie berühren.
FORBIDDEN_PORT = 8765
FORBIDDEN_DATA = Path.home() / ".local" / "share" / "baruch"

# Cookies, die jedes Bild bekommt, solange sein Eintrag nichts anderes sagt. 1400 px liegen unter der Schwelle für
# das kompakte Band; damit die Bilder das volle Band zeigen, steht der Laptop-Modus auf „Aus“.
DEFAULT_COOKIES = {"baruch_laptop": "off"}
# Cookies der Oberfläche, die der Vermittler verwaltet: Was der Browser dazu schickt, zählt nicht.
MANAGED_COOKIES = (
    "baruch_work", "baruch_laptop", "baruch_theme", "baruch_accordion", "baruch_editor", "baruch_band",
    "baruch_bandx", "baruch_pv", "baruch_folded", "baruch_closed", "baruch_left", "baruch_right", "baruch_lrail",
    "baruch_rrail", "baruch_session",
)
SESSION_COOKIE = "baruch_session"

# Erfundene Menschen (Vertrag §4): Autorin des Beispielwerks und ein Lektor.
AUTHOR = {"name": "mara", "display": "Mara Tiedemann"}
LEKTOR = {"name": "jonte", "display": "Jonte Harksen"}
# Name, unter dem sich das Skript am MCP-Zugang des eigenen Servers meldet (Herkunft @mcp:schreibhilfe).
MCP_CLIENT = "Schreibhilfe"

# Die sieben Bilder der Seite „Funktionen“ (site/static/bilder/*.png): Namen und Adressen wie seit Runde 24/33.
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
# Bildliste
# ---------------------------------------------------------------------------------------------------------------

STEP_KINDS = ("cookie", "offen", "klick", "sitzung")
KNOWN_NEEDS = ("klick", "ohne-js", "konten", "sword")


def load_pictures():
    """Liest site/data/hilfebilder.toml und die sieben Bilder der Seite „Funktionen“ als eine Liste von Einträgen
    mit den Feldern name, datei (relativ zu site/static/bilder), adresse, fenster, thema, dunkel, ausschnitt,
    schritt, erwartet, stufe, braucht, seite, zeigt. Bricht bei einem fehlerhaften Eintrag ab."""
    with open(LIST_FILE, "rb") as handle:
        listed = tomllib.load(handle).get("bild", [])
    pictures = []
    for entry in FUNCTION_PICTURES:
        pictures.append(normalised(entry, f"{entry['name']}.png", ["funktionen"]))
    for entry in listed:
        if "name" not in entry:
            fail(f"{LIST_FILE.name}: ein Block [[bild]] hat keinen Namen.")
        pictures.append(normalised(entry, f"{HELP_DIR}/{entry['name']}.png", entry.get("seite", [])))
    names = [p["name"] for p in pictures if p["datei"].startswith(HELP_DIR + "/")]
    doubled = sorted({n for n in names if names.count(n) > 1})
    if doubled:
        fail(f"{LIST_FILE.name}: doppelte Namen: {', '.join(doubled)}")
    return pictures


def normalised(entry, file, pages):
    name = entry["name"]
    if not re.fullmatch(r"[a-z0-9-]+", name):
        fail(f"Bild „{name}“: Namen bestehen aus Kleinbuchstaben, Ziffern und Bindestrich.")
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
        "erwartet": list(entry.get("erwartet", [])), "stufe": stage, "braucht": list(needs),
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
# Abgleich ohne Server
# ---------------------------------------------------------------------------------------------------------------

USE = re.compile(r"\{\{<\s*hilfebild\b([^>]*?)>\}\}", re.S)


def reconcile(pictures):
    """--abgleich: verwendete Namen gegen die Liste, fehlende Dateien, Gesamtgröße. Exit 1 bei einem unbekannten
    Namen oder wenn site/static/bilder die Grenze überschreitet; fehlende Dateien sind nur eine Liste."""
    listed = {p["name"]: p for p in pictures if p["datei"].startswith(HELP_DIR + "/")}
    used = {}
    for page in sorted(CONTENT.rglob("*.md")):
        text = page.read_text(encoding="utf-8")
        for call in USE.finditer(text):
            found = re.search(r'\bname\s*=\s*"([^"]*)"', call.group(1))
            name = found.group(1) if found else "(ohne name)"
            used.setdefault(name, []).append(page.relative_to(CONTENT).as_posix())
    unknown = sorted(name for name in used if name not in listed)
    unused = sorted(name for name in listed if name not in used)
    print(f"Bildliste: {len(listed)} Namen in {LIST_FILE.relative_to(REPO)}; "
          f"{sum(len(v) for v in used.values())} Aufrufe von hilfebild in site/content, {len(used)} verschiedene Namen.")
    if unknown:
        print(f"\nUNBEKANNT ({len(unknown)}) – stehen nicht in der Liste, der Bau der Webseite bricht ab:")
        for name in unknown:
            print(f"  {name}  ({', '.join(sorted(set(used[name])))})")
    else:
        print("Unbekannte Namen: keine.")
    if unused:
        print(f"\nUngenutzt ({len(unused)}) – stehen in der Liste, aber auf keiner Seite:")
        for name in unused:
            print(f"  {name}  (vorgesehen: {', '.join(listed[name]['seite']) or '–'})")
    else:
        print("Ungenutzte Namen: keine.")
    elsewhere = []
    for name, pages in sorted(used.items()):
        if name in listed:
            planned = set(listed[name]["seite"])
            for page in sorted(set(pages)):
                stem = Path(page).stem if Path(page).stem != "_index" else Path(page).parent.name
                if stem not in planned:
                    elsewhere.append(f"  {name}  auf {page} (Liste: {', '.join(sorted(planned)) or '–'})")
    if elsewhere:
        print(f"\nAuf einer anderen Seite als in der Liste vorgesehen ({len(elsewhere)}, nur Hinweis):")
        print("\n".join(elsewhere))
    missing = [file for p in pictures for file, _ in variants(p) if not (TARGET / file).is_file()]
    if missing:
        print(f"\nFehlende Dateien ({len(missing)}) unter {TARGET.relative_to(REPO)} – die Seiten lassen sie aus:")
        for file in missing:
            print(f"  {file}")
    else:
        print("Fehlende Dateien: keine.")
    stray = sorted(
        path.relative_to(TARGET).as_posix() for path in (TARGET / HELP_DIR).glob("*.png")
        if path.relative_to(TARGET).as_posix() not in {file for p in pictures for file, _ in variants(p)}
    ) if (TARGET / HELP_DIR).is_dir() else []
    if stray:
        print(f"\nDateien ohne Eintrag in der Liste ({len(stray)}, nur Hinweis):")
        for file in stray:
            print(f"  {file}")
    total = sum(path.stat().st_size for path in TARGET.rglob("*") if path.is_file()) if TARGET.is_dir() else 0
    over = total > SIZE_LIMIT
    print(f"\nGröße von {TARGET.relative_to(REPO)}: {total / 1024 / 1024:.2f} MB von höchstens "
          f"{SIZE_LIMIT / 1024 / 1024:.0f} MB" + (" – ÜBERSCHRITTEN" if over else "."))
    if unknown or over:
        sys.exit(1)


# ---------------------------------------------------------------------------------------------------------------
# Wegwerf-Server
# ---------------------------------------------------------------------------------------------------------------

def find_server(given):
    for candidate in (given, os.environ.get("BARUCH_SERVER")):
        if candidate and Path(candidate).is_file():
            return Path(candidate).resolve()
    # Ohne Angabe die zuletzt gebaute Datei: Ein alter Release-Bau neben einem frischen Testbau zeigte sonst eine
    # alte Oberfläche (Runde 37: Bau vom Vortag, 404 beim Word-Export). Welche Datei läuft, steht in der Ausgabe.
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


def import_work(server, data):
    done = subprocess.run(
        [str(server), "import", str(WORK_FILE), "--data", str(data)],
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
    # Weitere Ausgaben des Servers mitlesen, damit er nie an einer vollen Leitung hängt.
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
# Anfragen an den eigenen Server (nur 127.0.0.1)
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
    Servers: Die Formulare stehen hinter guard::same_origin."""

    def __init__(self, base, session=None):
        self.base = base
        self.session = session

    def headers(self, extra=None):
        headers = {"Origin": self.base}
        if self.session:
            headers["Cookie"] = f"{SESSION_COOKIE}={self.session}"
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
# Erzeugte Dateien: Bilder (PNG mit zlib/struct), ein kleines EPUB als Quelle, eine Markdown-Datei
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


def source_epub():
    """Ein winziges EPUB als Quelle für die Literatur: ein erfundenes Dienstbuch, zwei Kapitel."""
    chapters = [
        ("Vom Anzünden", "Die Lampe wird eine halbe Stunde vor Sonnenuntergang angezündet und brennt bis zum vollen "
                         "Tag. Der Wärter grüßt sie, ehe er die Treppe hinabsteigt; so ist es Brauch auf den Halligen."),
        ("Wenn der Docht reißt", "Reißt der Docht in der Nacht, so tritt der Wärter mit der Handlaterne an das "
                                 "seewärtige Fenster und schwenkt sie in gleichem Takt, bis der Schaden behoben ist. "
                                 "Jede Stunde ohne Feuer wird im Logbuch vermerkt."),
    ]
    items = "".join(f'<item id="k{i}" href="k{i}.xhtml" media-type="application/xhtml+xml"/>' for i in (1, 2))
    refs = "".join(f'<itemref idref="k{i}"/>' for i in (1, 2))
    opf = (
        '<?xml version="1.0" encoding="UTF-8"?>\n'
        '<package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="id">'
        '<metadata xmlns:dc="http://purl.org/dc/elements/1.1/">'
        '<dc:identifier id="id">urn:uuid:6f0f2a3e-51c7-4b0e-9d7a-0a5b3c1d2e4f</dc:identifier>'
        "<dc:title>Dienstbuch für die Leuchtfeuerwärter der Halligen</dc:title>"
        "<dc:creator>Feuerschiffamt Süderhaven</dc:creator><dc:language>de</dc:language>"
        '<meta property="dcterms:modified">1911-03-01T00:00:00Z</meta></metadata>'
        f'<manifest><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>{items}'
        f"</manifest><spine>{refs}</spine></package>"
    )
    nav = (
        '<?xml version="1.0" encoding="UTF-8"?>\n<html xmlns="http://www.w3.org/1999/xhtml" '
        'xmlns:epub="http://www.idpf.org/2007/ops"><head><title>Inhalt</title></head><body>'
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


# Wegwerf-Werk: wird als Markdown eingelesen (der Bericht nennt das fehlende Bild) und danach gelöscht (Archiv).
THROWAWAY_TITLE = "Notizen vom Festland"
THROWAWAY_MD = """# Notizen vom Festland

Was ich im Winter in Süderhaven aufgeschrieben habe, bevor das Boot wieder fuhr.

## Die Fähre

Sie fährt nur bei Hochwasser und nur, wenn der Schiffer Lust hat. Beides trifft selten zusammen.

![Die Fähre am Anleger](bilder/faehre.png)

## Der Laden am Hafen

Mehl, Petroleum, Dochte in drei Stärken. Die Zeitung ist immer von vorgestern.
"""


# ---------------------------------------------------------------------------------------------------------------
# Beispielwerk ausbauen – Stufe „offen“ (ohne Konten)
# ---------------------------------------------------------------------------------------------------------------

REVISED = "Der erste Morgen"
SECOND_REVISION = """== Der erste Morgen

Ich wachte auf, weil es still war. Kein Wind, kein Wasser an der Mauer – nur die Uhr über der Treppe.
Hundertzwölf Stufen sind es bis zur Laterne. Ich habe sie an diesem Morgen zum ersten Mal gezählt, mich zweimal
verzählt und beim dritten Mal laut gezählt, damit es jemand hört.

#szene[Oben stand das Fenster offen. Auf dem Sims lag eine Feder, grau mit weißem Rand.]
"""

LOGBOOK = "Das Logbuch"
LOGBOOK_FIGURE = (
    '\n#figure(image("bilder/leuchtturm.png", width: 55%), caption: [Der Turm von Mövenoog, vom Anleger aus '
    "gesehen])\n"
)
PROPOSED = "Besuch vom Festland"
PROPOSAL = """== Besuch vom Festland

Am Johannistag brachte das Versorgungsboot Mehl, Petroleum und einen Jungen. Er hieß Ole, sollte drei Wochen
bleiben und fragte gleich am ersten Abend, ob das Licht auch für die Fische leuchte.

Am zweiten Abend wollte er wissen, wer die Lampe anzündet, wenn ich krank bin. „Der Wärter von Süderoog“, sagte
ich, „wenn er es rechtzeitig sieht.“ Ole fand, das sei keine gute Antwort. Er hatte recht.
"""
XREF = "Was das Licht nicht zeigt"
NIGHT = "Die Nacht ohne Lampe"
FAREWELL = "Abschied"
LEKTOR_PROPOSAL = """== Die Nacht ohne Lampe

In der Nacht zum elften riss der Docht. Vier Stunden stand ich mit der Handlaterne am Fenster und schwenkte sie,
wie es das Dienstbuch von 1911 beschreibt. Draußen fuhr niemand. Das wusste ich aber erst am Morgen.
"""


def isbn13(first_twelve):
    """Hängt die Prüfziffer an – eine erfundene Nummer aus dem Bereich 978-3-99999 (kein Verlag führt sie)."""
    total = sum(int(digit) * (1 if index % 2 == 0 else 3) for index, digit in enumerate(first_twelve))
    return first_twelve + str((10 - total % 10) % 10)


def build_open(base, work, data, sword):
    """Baut das Beispielwerk aus, solange es keine Konten gibt. Liefert (Platzhalter für die Adressen, gespeicherte
    Antworten von Formularen)."""
    me = Client(base)
    values = {"work": work, "sample": 1}
    answers = {}

    # Bibelmodule: nie die des Nutzers (~/.sword, /usr/share/sword) – nur der angegebene oder ein leerer Ordner.
    if sword:
        bibles = me.rpc("bible.set_dirs", {"dirs": [str(Path(sword).expanduser().resolve())]})
        print(f"Bibelmodule aus {sword}: {len(bibles.get('modules', []))} gefunden.")
        # Die erste lesbare Bibel wird die Bibel des Werks – sonst zeigten „Zitate prüfen“ und die Einstellungen
        # trotz Modul den Leerstand.
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
    for title in (REVISED, LOGBOOK, PROPOSED, XREF, NIGHT, FAREWELL):
        if title not in key_of:
            fail(f"Abschnitt „{title}“ fehlt im importierten Werk.")
    values.update(key=key_of[REVISED], key_logbook=key_of[LOGBOOK], key_proposal=key_of[PROPOSED],
                  key_xref=key_of[XREF], key_night=key_of[NIGHT], key_farewell=key_of[FAREWELL])

    def current_text(title):
        node = me.rpc("node.get", {"work": work, "node": index_of[title]})
        node = node.get("node", node)
        revisions = node["revisions"]
        current = next((r for r in revisions if r.get("current")), revisions[-1])
        return current["body"], current["number"]

    # Zweite Fassung (wie der Mensch in der Oberfläche).
    saved = me.rpc("revision.save", {"work": work, "node": index_of[REVISED], "body": SECOND_REVISION, "mode": "new"})
    print(f"Abschnitt „{REVISED}“ (key {values['key']}) hat jetzt Fassung {saved.get('rev')}.")

    # Bilder: ein Werkbild und ein Umschlagbild, beide im Skript gezeichnet.
    picture = base64.b64encode(lighthouse_png(1200, 800)).decode()
    me.rpc("asset.add", {
        "work": work, "file_name": "leuchtturm.png", "content": picture, "encoding": "base64",
        "path": "bilder/leuchtturm.png", "caption": "Der Turm von Mövenoog, vom Anleger aus gesehen",
        "alt": "Rot-weiß gestreifter Leuchtturm vor Abendhimmel und Meer",
    })
    cover = base64.b64encode(lighthouse_png(1748, 2480)).decode()
    me.rpc("asset.add", {
        "work": work, "file_name": "umschlag.png", "content": cover, "encoding": "base64",
        "caption": "Umschlagbild", "alt": "Leuchtturm in der Abenddämmerung",
    })
    me.rpc("work.set_cover", {"work": work, "name": "umschlag.png"})
    print("Bilder leuchtturm.png und umschlag.png angelegt, Umschlag gesetzt.")

    # Bild im Text, danach Registermarken (jede Marke ist eine neue Fassung des Menschen).
    body, _ = current_text(LOGBOOK)
    me.rpc("revision.save", {"work": work, "node": index_of[LOGBOOK], "body": body.rstrip("\n") + "\n" + LOGBOOK_FIGURE,
                             "mode": "new"})
    for title, word in ((LOGBOOK, "Dienstzimmer"), (LOGBOOK, "Postkarte"), (NIGHT, "Handlaterne"), (NIGHT, "Docht")):
        _, rev = current_text(title)
        me.rpc("index.mark", {"work": work, "key": key_of[title], "rev": rev, "word": word})
    print("Bild im Abschnitt „Das Logbuch“, vier Registermarken gesetzt.")

    # Querverweis auf den Abschied (feste Sprungmarke <kSchlüssel>).
    body, _ = current_text(XREF)
    link = f"\nWie es ausging, steht im #link(<k{key_of[FAREWELL]}>)[Abschied].\n"
    linked = me.rpc("revision.save", {"work": work, "node": index_of[XREF],
                                      "body": body.rstrip("\n") + "\n" + link, "mode": "new"})
    values["rev_xref"] = linked["rev"]

    # Glossar.
    for term in (
        {"term": "Hallig", "short": "kleine Marschinsel ohne Deich", "variants": ["Halligen"],
         "long": "Bei Sturmflut steht das Land unter Wasser; nur die Warften mit den Häusern ragen heraus."},
        {"term": "Laterne", "short": "der verglaste Raum an der Spitze des Turms, in dem die Lampe brennt"},
        {"term": "Priel", "short": "Wasserlauf im Watt, der auch bei Ebbe nicht trockenfällt"},
    ):
        me.rpc("glossary.add", {"work": work, **term})

    # Buchangaben (erfunden), Satz-Einstellungen, Zielzeit für den Zeitwächter.
    me.rpc("work.meta.set", {
        "work": work, "subtitle": "Aufzeichnungen einer Leuchtturmwärterin", "publisher": "Verlag Priel & Tonne",
        "place": "Süderhaven", "year": "2026", "edition": "1. Auflage", "isbn": isbn13("978399999012"),
        "printer": "Druckhaus am Deich, Süderhaven", "dedication": "Für alle, die abends ein Licht anzünden.",
        "blurb": "Ein Jahr lang hütet Mara Tiedemann das Leuchtfeuer der Hallig Mövenoog. Sie zählt Stufen, führt "
                 "das Logbuch und lernt, was ein Licht zeigt – und was nicht.",
        "bio": "Mara Tiedemann gibt es nicht. Sie ist die erfundene Autorin dieses Beispielwerks.",
    })
    me.rpc("work.typeset_set", {"work": work, "settings": {
        "chapter_start": "right_page", "page_format": {"kind": "a5"}, "font_size_pt": 10.5,
        "page_numbers": {"position": "bottom_outside", "start": "first_chapter"}, "running_head": "chapter",
    }})
    me.rpc("work.timing_set", {"work": work, "words_per_minute": 130, "target_minutes": 5})
    print("Glossar (3 Begriffe), Buchangaben, Satz-Einstellungen und Zielzeit gesetzt.")

    # Einfälle: Hauptthema mit Unterknoten, ein Einfall am Abschnitt, zwei in der Wolke, zwei Verbindungen.
    def idea(**fields):
        return me.rpc("idea.create", {"work": work, **fields})["idea"]["id"]

    fire = idea(title="Leuchtfeuer", words=["Licht", "Dienst"], cloud=False,
                note="Was das Feuer braucht und was es von der Wärterin verlangt.")
    wick = idea(title="Docht und Petroleum", words=["Docht"], parent=fire, note="Drei Stärken; der Laden am Hafen.")
    lamp = idea(title="Handlaterne von 1911", words=["Dienstbuch"], parent=fire,
                source="Dienstbuch für die Leuchtfeuerwärter der Halligen")
    idea(title="Takt beim Schwenken", words=["Takt"], parent=lamp)
    guest = idea(title="Ole", words=["Besuch", "Fragen"], cloud=False,
                 note="Ein Kind stellt die Fragen, die die Wärterin sich nicht stellt.")
    idea(title="Leuchtet das Licht für Fische?", words=["Frage"], parent=guest, key=key_of[PROPOSED])
    at_section = idea(title="Die Feder auf dem Sims", words=["Feder", "Zeichen"], key=key_of[REVISED],
                      quote="Auf dem Sims lag eine Feder, grau mit weißem Rand.")
    idea(words=["Sturmflut", "Warft"], note="Einmal das Land unter Wasser zeigen – im Herbst?")
    idea(title="Brief an den alten Wärter", words=["Brief"], note="Vielleicht als Schluss, nach dem Abschied.")
    me.rpc("idea.link", {"from": wick, "to": lamp, "kind": "leads_to", "note": "reißt der Docht, bleibt die Laterne"})
    me.rpc("idea.link", {"from": guest, "to": fire, "kind": "contradicts"})
    values.update(idea=at_section, branch=fire)
    print("Einfälle angelegt (zwei Hauptthemen mit Unterknoten, einer am Abschnitt, zwei in der Wolke).")

    # Assistenten ohne Modell: über den MCP-Zugang des eigenen Servers, unter dem Namen der Rolle „Lektor“.
    assistant = Mcp(base, MCP_CLIENT)
    first_post = assistant.call("add_post", {
        "work": work, "key": key_of[REVISED], "persona": "lektor",
        "body": "Der Einstieg über die Stille trägt. Die Feder am Schluss ist stark – ich würde sie nicht erklären.",
    })
    values["post"] = first_post["id"]
    me.rpc("post.add", {"work": work, "key": key_of[REVISED],
                        "body": "Danke. Zählt sie die Stufen zu oft? Dreimal „gezählt“ in zwei Sätzen."})
    assistant.call("add_post", {
        "work": work, "key": key_of[REVISED], "persona": "lektor",
        "body": "Das dritte „gezählt“ ist das wichtige: laut, damit es jemand hört. Die ersten beiden ließen sich "
                "zusammenziehen.",
    })
    proposed = assistant.call("propose_revision", {
        "work": work, "key": key_of[PROPOSED], "body": PROPOSAL, "persona": "lektor",
        "note": "Wörtliche Rede statt Bericht; ein Schlusssatz.",
    })
    values["rev_proposal"] = proposed["rev"]
    asked = assistant.call("outline_rename", {
        "work": work, "key": key_of[FAREWELL], "title": "Abschied von der Hallig",
        "note": "Der Ort gehört in den Titel des letzten Abschnitts.",
    })
    values["request"] = asked["request"]["id"]
    thread = me.rpc("thread.post", {"work": work, "message": "Reihenfolge der Kapitel: Soll der Herbst vor dem "
                                                              "Sommer stehen?"})
    if thread.get("answers") or thread.get("errors"):
        fail("thread.post hat ein Modell angesprochen; das darf beim Aufbau nicht geschehen.")
    values["thread"] = thread["thread"]
    assistant.call("chat", {"work": work, "thread": thread["thread"],
                            "message": "Chronologisch bleibt es klarer. Ein Vorgriff auf den Sturm im ersten "
                                       "Kapitel würde reichen."})
    me.rpc("thread.post", {"work": work, "thread": thread["thread"],
                           "message": "Dann bleibt es beim Jahreslauf. Den Vorgriff probiere ich in „Ankunft“."})
    print("Beiträge, Vorschlag und Gliederungsantrag eines Assistenten (über MCP, ohne Modell), ein Faden.")

    # Quelle für die Literatur: ein kleines erfundenes EPUB, lokal gelesen, nur in die Bibliothek.
    draft = me.rpc("source_import.preview", {
        "file_name": "dienstbuch-1911.epub", "content": base64.b64encode(source_epub()).decode(),
    })
    adopted = me.rpc("source_import.apply", {"draft": draft["draft"], "target": "library"})
    values["title"] = adopted["library_title"]

    # Wegwerf-Werk als Markdown einlesen: Die Antwort ist der Bericht (ein Bild fehlt). Später wird es gelöscht.
    status, headers, content = me.upload("/werke/import", {"title": THROWAWAY_TITLE}, "file", "notizen-vom-festland.md",
                                         THROWAWAY_MD.encode(), "text/markdown")
    if status == 200:
        answers["einlesen-bericht"] = content
    else:
        fail(f"Einlesen der Markdown-Datei: Status {status} statt des Berichts (200).")
    found = re.search(r"/\?work=(\d+)", content.decode("utf-8", "replace"))
    throwaway = next((w for w in me.rpc("work.list", {})["works"] if found and w["id"] == int(found.group(1))), None)
    if not throwaway or throwaway["id"] in (work, sample):
        fail("Der Bericht des Einlesens nennt das Wegwerf-Werk nicht.")

    # Vorschauen des Einlesens aus Ausgaben des eigenen Servers.
    def export(path, what):
        status, _, content = me.get(path)
        if status != 200 or not content:
            fail(f"{what}: {path} antwortet mit {status}.")
        return content

    def preview(path, file_name, content, mime, name, fields=None, file_field="file"):
        status, headers, page = me.upload(path, fields or {}, file_field, file_name, content, mime)
        if status == 303 and headers.get("Location"):
            # Manche Formulare legen einen Entwurf an und leiten auf seine Seite weiter: dann zählt diese Seite.
            status, _, page = me.get(headers["Location"])
        if status != 200 or not page:
            fail(f"Vorschau {name}: {path} antwortet mit {status}: {page[:300].decode('utf-8', 'replace')}")
        answers[name] = page

    docx = export(f"/werke/{work}/word.docx", "Word-Export")
    preview("/werke/import/word/vorschau", "ein-jahr-auf-moevenoog.docx", docx,
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document", "einlesen-word-vorschau")
    exchange = export(f"/werke/{work}/austausch.baruch", "Austauschdatei")
    preview("/austausch/vorschau", "ein-jahr-auf-moevenoog.baruch", exchange, "application/octet-stream",
            "austausch-vorschau", file_field="datei")
    pdf = export(f"/werke/{work}/pdf", "Buch als PDF")
    preview("/werke/import/quelle/vorschau", "ein-jahr-auf-moevenoog.pdf", pdf, "application/pdf",
            "einlesen-pdf-vorschau", {"way": "local"})
    print("Vorschauen des Einlesens (Word, Austauschdatei, PDF) aus Ausgaben des eigenen Servers gespeichert.")

    # Wegwerf-Werk löschen: Es liegt danach im Archiv.
    me.rpc("work.delete", {"work": throwaway["id"], "confirm_title": throwaway["title"]})
    print(f"Wegwerf-Werk „{throwaway['title']}“ eingelesen und gelöscht (Archiv).")
    return values, answers


# ---------------------------------------------------------------------------------------------------------------
# Stufe „konten“: Autorin, Lektor, Freigabe, Befunde
# ---------------------------------------------------------------------------------------------------------------

def session_from(headers):
    for value in headers.get_all("Set-Cookie") or []:
        found = re.match(rf"{SESSION_COOKIE}=([^;]+)", value)
        if found and found.group(1):
            return found.group(1)
    return None


def build_accounts(base, values):
    """Legt die Konten an (danach verlangt jede Seite eine Anmeldung) und lässt den Lektor Befunde, einen Vorschlag
    und einen Beitrag schreiben. Liefert die Sitzungen {"autorin": …, "lektor": …}; sie werden nie ausgegeben.
    Die Passwörter entstehen je Lauf zufällig und verlassen das Skript nicht."""
    work = values["work"]
    passwords = {"autorin": secrets.token_urlsafe(18), "lektor": secrets.token_urlsafe(18)}
    open_door = Client(base)
    status, headers, _ = open_door.form("/einrichten", {
        "name": AUTHOR["name"], "display": AUTHOR["display"],
        "password": passwords["autorin"], "password2": passwords["autorin"],
    })
    author_session = session_from(headers)
    if status != 303 or not author_session:
        fail(f"Konten einrichten: Status {status}, keine Sitzung.")
    author = Client(base, author_session)
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
    lektor = Client(base, lektor_session)
    for finding in (
        {"key": values["key"], "kind": "hint", "quote": "mich zweimal verzählt",
         "body": "Dreimal „gezählt“ kurz nacheinander. Absicht? Wenn ja, das dritte Mal deutlicher absetzen."},
        {"key": values["key_night"], "kind": "critical", "quote": "Dienstbuch von 1911",
         "body": "Im Kapitel „Ankunft“ ist der Vorgänger dreißig Jahre im Dienst; ein Dienstbuch von 1911 passt "
                 "dazu nur, wenn die Geschichte nach 1941 spielt. Bitte die Jahreszahlen abgleichen."},
        {"key": values["key_xref"], "kind": "praise", "quote": "Wohin man fahren soll, muss jeder selbst wissen.",
         "body": "Der stärkste Satz des Kapitels. So stehen lassen."},
        {"key": values["key_logbook"], "kind": "hint",
         "body": "Die Liste „Was ins Logbuch gehört“ wiederholt die vier Spalten von oben – eine von beiden kürzen."},
        {"kind": "hint", "body": "Zum ganzen Buch: Der Winter fehlt. Das Jahr beginnt im Februar und endet im Herbst."},
    ):
        lektor.rpc("finding.add", {"work": work, **finding})
    proposed = lektor.rpc("revision.propose", {"work": work, "key": values["key_night"], "body": LEKTOR_PROPOSAL,
                                               "note": "Zwei Sätze gestrafft."})
    values["rev_lektor"] = proposed["rev"]
    lektor.rpc("post.add", {"work": work, "key": values["key_night"],
                            "body": "Vorschlag liegt am Abschnitt. Die Frage nach 1911 habe ich als Befund notiert."})
    print(f"Konten angelegt ({AUTHOR['display']} als Autorin, {LEKTOR['display']} als Lektor), Werk freigegeben, "
          "fünf Befunde, ein Vorschlag und ein Beitrag des Lektors.")
    return {"autorin": author_session, "lektor": lektor_session}


# ---------------------------------------------------------------------------------------------------------------
# Vermittler: http.server auf 127.0.0.1:0 vor dem Wegwerf-Server
# ---------------------------------------------------------------------------------------------------------------

PIXEL = base64.b64decode("R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7")
# So lange hält ein nachgeladenes Bild das load-Ereignis auf, auf das `firefox --screenshot` wartet (Millisekunden):
# Klick nach 500 ms, Messung des Ausschnitts nach 2500 ms.
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

    def __init__(self, upstream):
        super().__init__(("127.0.0.1", 0), RelayHandler)
        self.upstream = upstream
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
        # Host, Origin und Referer tragen die Adresse des Servers: sonst lehnt guard::same_origin mit 403 ab.
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
                    continue  # die Sitzung bleibt im Vermittler, sie gelangt nie in das Profil des Browsers
                if low == "location":
                    value = value.replace(relay.upstream, "")
                    if picked:  # die Weiterleitung bleibt dieselbe Aufnahme
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
            # Cookies auch an den Browser: Die Widgets lesen document.cookie (das Thema stellte sonst zurück).
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
# Prüfen, Fotografieren, Nachbearbeiten
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


def report_size(target):
    total = sum(path.stat().st_size for path in target.rglob("*") if path.is_file()) if target.is_dir() else 0
    print(f"Größe von {target}: {total / 1024 / 1024:.2f} MB von höchstens {SIZE_LIMIT // 1024 // 1024} MB"
          + (" – ÜBERSCHRITTEN" if total > SIZE_LIMIT else "."))


# ---------------------------------------------------------------------------------------------------------------
# Aufruf
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
    parser.add_argument("--ziel", default=str(TARGET), help="Ordner für die Bilder (Vorgabe: site/static/bilder)")
    parser.add_argument("--temp", help="Ordner auf einer Platte für Wegwerf-Datenordner und Firefox-Profile "
                                       "(Vorgabe: $TMPDIR – oft Arbeitsspeicher)")
    parser.add_argument("--breite", type=int, default=1400, help="größte Breite der Bilder in Pixeln (Vorgabe 1400)")
    parser.add_argument("--ohne-palette", action="store_true", help="Bilder nicht auf 256 Farben verringern")
    args = parser.parse_args()

    pictures = load_pictures()
    if args.nur:
        wanted = [name.strip() for name in args.nur.split(",") if name.strip()]
        known = {p["name"] for p in pictures}
        strangers = [name for name in wanted if name not in known]
        if strangers:
            fail(f"--nur: unbekannte Namen: {', '.join(strangers)}")
        pictures = [p for p in pictures if p["name"] in wanted]

    if args.liste:
        print(f"{len(pictures)} Bilder, {sum(len(variants(p)) for p in pictures)} Aufnahmen. Platzhalter: {{work}} = "
              "Nummer des Beispielwerks, {key} = Abschnitt „" + REVISED + "“, weitere siehe site/tools/README.md.")
        show_list(pictures)
        return
    if args.abgleich:
        reconcile(load_pictures())
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
        work = import_work(server, data)
        point_connections_nowhere(data)
        process, base = start_server(server, data)
        relay = Relay(base)
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
                shoot(relay, listed, values, Path(args.ziel), temp, args)

        values, answers = build_open(base, work, data, args.sword)
        relay.answers.update(answers)
        print("Platzhalter: " + ", ".join(f"{{{name}}}={value}" for name, value in sorted(values.items())))
        run("offen", "ohne Konten")
        if args.pruefen and not args.nur:
            failures += probe_relay(relay)
        if stages["konten"] or args.halten:
            relay.sessions.update(build_accounts(base, values))
            run("konten", "mit Konten, jede Seite verlangt eine Anmeldung")
        if args.pruefen:
            total = sum(len(variants(p)) for p in pictures)
            if failures:
                print(f"\n{failures} von {total} Aufnahmen mit Fehlern.")
            else:
                print(f"\nAlle {total} Aufnahmen ({len(pictures)} Bilder) antworten mit 200 und Inhalt; "
                      "jeder Schritt hat gewirkt.")
        if args.fotos:
            report_size(Path(args.ziel))
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
        # Ausgabe in ein Programm, das früher aufhört zu lesen (etwa „| head“): still beenden.
        os.dup2(os.open(os.devnull, os.O_WRONLY), sys.stdout.fileno())
        sys.exit(1)
