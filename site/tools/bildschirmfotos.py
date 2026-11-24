#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Bildschirmfotos für die Webseite (site/static/bilder/*.png, 1400×860; die Arbeitsseite zusätzlich auf
Laptop-Breite 1366×768) aus einem frei erfundenen Werk.

Ablauf:
  1. Wegwerf-Datenordner anlegen und site/tools/beispielwerk.typ importieren (baruch-server import).
  2. In der Datenbank die Verbindungen `eliezer` und `multigpt` auf http://127.0.0.1:9 stellen – sonst
     bettet der Server beim Start über die Vorgabe 127.0.0.1:1234 ein (SKILLS.md, „Wegwerf-Server“).
  3. Eigenen Server starten: serve --listen 127.0.0.1:0 --data <Ordner> --secrets memory.
  4. Über RPC einem Abschnitt eine zweite Fassung geben (für das Bild „fassungen“).
  5. Je nach Angabe: Adressen nur zeigen (--liste), abrufen und prüfen (--pruefen), bis zur Eingabetaste
     offen halten (--halten) oder mit Firefox fotografieren (--fotos).
  6. Den eigenen Server über seine PID beenden, den Wegwerf-Ordner löschen.

Fasst nichts außerhalb des Wegwerf-Ordners an: nicht den Dienst des Nutzers (127.0.0.1:8765), nicht
~/.local/share/baruch, nicht den Schlüsselbund. Kein Netz außer 127.0.0.1.

Nur Python 3 (Standardbibliothek). Aufruf aus dem Hauptordner des Repositorys:

    site/tools/bildschirmfotos.py --pruefen
    site/tools/bildschirmfotos.py --fotos            # nur der Integrator; nie parallel zu Testläufen
"""

import argparse
import json
import os
import re
import shutil
import signal
import sqlite3
import subprocess
import sys
import tempfile
import threading
import urllib.error
import urllib.request
from pathlib import Path

TOOLS = Path(__file__).resolve().parent
REPO = TOOLS.parent.parent
WORK_FILE = TOOLS / "beispielwerk.typ"
TARGET = REPO / "site" / "static" / "bilder"
WINDOW = "1400,860"
# Runde 33: die Arbeitsseite zusätzlich auf Laptop-Breite. Unter 1600 px zeichnet der Laptop-Modus („automatisch“)
# das Menüband kompakt (COMPACT_BELOW_PX in crates/baruch-server/src/laptop.rs); die Breite misst das Skript der
# Seite, darum braucht die Adresse keinen Schalter.
LAPTOP_WINDOW = "1366,768"
# Die Verbindungen des Wegwerf-Servers zeigen auf den Discard-Port: Dort antwortet niemand.
NOWHERE = "http://127.0.0.1:9"
# Abschnitt, der eine zweite Fassung bekommt (Titel aus beispielwerk.typ).
REVISED = "Der erste Morgen"
SECOND_REVISION = """== Der erste Morgen

Ich wachte auf, weil es still war. Kein Wind, kein Wasser an der Mauer – nur die Uhr über der Treppe.
Hundertzwölf Stufen sind es bis zur Laterne. Ich habe sie an diesem Morgen zum ersten Mal gezählt, mich zweimal
verzählt und beim dritten Mal laut gezählt, damit es jemand hört.

#szene[Oben stand das Fenster offen. Auf dem Sims lag eine Feder, grau mit weißem Rand.]
"""


def pictures(work, key):
    """Bild → (Pfad der Seite, Beschreibung, Fenstergröße). Die Reihenfolge ist die der Seite „Funktionen“; das
    Bild auf Laptop-Breite steht zuletzt (eigene Fenstergröße, eigenes Firefox-Profil)."""
    return [
        ("arbeitsplatz.png", f"/?work={work}&key={key}",
         "Arbeitsplatz: Menüband (Reiter Start), Gliederung, Editor im Satzbild, Akkordeons", WINDOW),
        ("gliederung.png", f"/gliederung?work={work}", "Seite Gliederung (ohne JavaScript bedienbar)", WINDOW),
        ("fassungen.png", f"/?work={work}&key={key}&rev=1",
         "Abschnitt mit zwei Fassungen, gezeigt ist die ältere", WINDOW),
        ("assistenten.png", f"/einstellungen/assistenten?work={work}",
         "Einstellungen → Assistenten (Personas)", WINDOW),
        ("satz.png", f"/werke/{work}/satz", "Satz-Vorschau des Werks (Typst), Menüband im Reiter Buch", WINDOW),
        ("einstellungen.png", f"/einstellungen/werk?work={work}", "Einstellungen → Werk mit dem Block Satz", WINDOW),
        ("arbeitsplatz-laptop.png", f"/?work={work}&key={key}",
         "Arbeitsplatz auf Laptop-Breite: Menüband kompakt (Laptop-Modus „automatisch“)", LAPTOP_WINDOW),
    ]


def show(base, listed):
    for name, path, what, window in listed:
        size = window.replace(",", "×")
        print(f"  {name:<24} {base}{path}\n  {'':<24} {what} · Fenster {size}")


def find_server(given):
    candidates = [given, os.environ.get("BARUCH_SERVER")]
    candidates += [REPO / "target" / "release" / "baruch-server", REPO / "target" / "debug" / "baruch-server"]
    for candidate in candidates:
        if candidate and Path(candidate).is_file():
            return Path(candidate).resolve()
    sys.exit(
        "baruch-server nicht gefunden. Erst bauen (nice -n 10 ionice -c3 cargo build -p baruch-server -j 4) "
        "oder mit --server PFAD angeben."
    )


def clean_env():
    """Umgebung ohne BARUCH_*: Der Wegwerf-Server erbt weder Adresse noch Datenordner des Dienstes."""
    return {name: value for name, value in os.environ.items() if not name.startswith("BARUCH_")}


def import_work(server, data):
    done = subprocess.run(
        [str(server), "import", str(WORK_FILE), "--data", str(data)],
        capture_output=True, text=True, env=clean_env(), check=False,
    )
    sys.stdout.write(done.stdout)
    sys.stderr.write(done.stderr)
    if done.returncode != 0:
        sys.exit(f"Import gescheitert (Exit {done.returncode}).")
    found = re.search(r"\(id (\d+)\)", done.stdout)
    if not found:
        sys.exit("Import: Die Ausgabe nennt keine Werk-id.")
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
        stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, env=clean_env(),
    )
    base = None
    for line in process.stdout:
        sys.stdout.write("  [server] " + line)
        found = re.search(r"Baruch läuft: (http://127\.0\.0\.1:\d+)/", line)
        if found:
            base = found.group(1)
            break
    if base is None:
        process.wait()
        sys.exit(f"Der Server ist nicht gestartet (Exit {process.returncode}).")
    # Weitere Ausgaben des Servers mitlesen, damit er nie an einer vollen Leitung hängt.
    threading.Thread(target=lambda: [None for _ in process.stdout], daemon=True).start()
    return process, base


def stop_server(process):
    """Nur den eigenen Prozess beenden (PID), erst freundlich, dann hart."""
    if process.poll() is not None:
        return
    process.send_signal(signal.SIGINT)
    try:
        process.wait(timeout=10)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait()


def rpc(base, method, params):
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params}).encode()
    request = urllib.request.Request(base + "/rpc", data=body, headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(request, timeout=60) as response:
        answer = json.load(response)
    if answer.get("error"):
        sys.exit(f"{method}: {answer['error']}")
    return answer["result"]


def prepare(base, work):
    """Zweite Fassung für einen Abschnitt; liefert dessen festen Schlüssel."""
    nodes = rpc(base, "work.get", {"work": work})["work"]["nodes"]
    index = next((i for i, node in enumerate(nodes) if node.get("title") == REVISED), None)
    if index is None:
        sys.exit(f"Abschnitt „{REVISED}“ fehlt im importierten Werk.")
    key = nodes[index]["key"]
    saved = rpc(base, "revision.save", {"work": work, "node": index, "body": SECOND_REVISION, "mode": "new"})
    print(f"Abschnitt „{REVISED}“ (key {key}) hat jetzt Fassung {saved.get('rev')}.")
    return key


def fetch(url):
    try:
        with urllib.request.urlopen(url, timeout=120) as response:
            return response.status, len(response.read())
    except urllib.error.HTTPError as error:
        return error.code, 0


def check(base, listed):
    failed = False
    for name, path, _, _ in listed:
        status, size = fetch(base + path)
        print(f"  {status}  {size:>8} Bytes  {name:<24} {path}")
        failed = failed or status != 200 or size == 0
    if failed:
        sys.exit("Mindestens eine Seite kam nicht mit 200.")
    print("Alle Seiten antworten mit 200.")


def shoot(base, listed, target):
    """Fotos mit Firefox (headless, eigenes Profil). Nur der Integrator, nie parallel zu Testläufen."""
    firefox = shutil.which("firefox")
    if not firefox:
        sys.exit("firefox nicht gefunden.")
    target.mkdir(parents=True, exist_ok=True)
    # Je Fenstergröße ein eigenes Profil: Der Laptop-Modus merkt sich die gemessene Breite im Cookie baruch_vw;
    # ein Foto in anderer Größe soll davon nichts erben.
    for window in dict.fromkeys(window for _, _, _, window in listed):
        with tempfile.TemporaryDirectory(prefix="baruch-fotos-profil-") as profile:
            for name, path, _, size in listed:
                if size != window:
                    continue
                file = target / name
                subprocess.run(
                    [firefox, "--headless", "--no-remote", "--profile", profile,
                     f"--window-size={window}", "--screenshot", str(file), base + path],
                    check=True, env=clean_env(),
                )
                print(f"  {file}  ({window.replace(',', '×')})")


def main():
    parser = argparse.ArgumentParser(description="Bildschirmfotos der Webseite aus einem erfundenen Werk.")
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--liste", action="store_true", help="nur zeigen, welche Adresse welches Bild ergibt")
    mode.add_argument("--pruefen", action="store_true", help="Server starten und jede Adresse abrufen (200?)")
    mode.add_argument("--halten", action="store_true", help="Server starten und bis zur Eingabetaste laufen lassen")
    mode.add_argument("--fotos", action="store_true", help="Server starten und mit Firefox fotografieren")
    parser.add_argument("--server", help="Pfad zu baruch-server (Vorgabe: target/release, dann target/debug)")
    parser.add_argument("--ziel", default=str(TARGET), help="Ordner für die Bilder (Vorgabe: site/static/bilder)")
    parser.add_argument("--temp", help="Ordner, in dem der Wegwerf-Datenordner entsteht (Vorgabe: $TMPDIR)")
    args = parser.parse_args()

    if args.liste:
        print("Bild → Adresse (WERK = id aus dem Import, KEY = Schlüssel von „" + REVISED + "“):")
        show("http://127.0.0.1:PORT", pictures("WERK", "KEY"))
        return

    server = find_server(args.server)
    data = Path(tempfile.mkdtemp(prefix="baruch-fotos-", dir=args.temp))
    process = None
    try:
        print(f"Server: {server}\nWegwerf-Datenordner: {data}")
        work = import_work(server, data)
        point_connections_nowhere(data)
        process, base = start_server(server, data)
        key = prepare(base, work)
        listed = pictures(work, key)
        print("Bild → Adresse:")
        show(base, listed)
        if args.pruefen:
            check(base, listed)
        elif args.fotos:
            shoot(base, listed, Path(args.ziel))
        else:
            input("Server läuft. Eingabetaste beendet ihn … ")
    finally:
        if process is not None:
            stop_server(process)
        shutil.rmtree(data, ignore_errors=True)
        print("Server beendet, Wegwerf-Datenordner gelöscht.")


if __name__ == "__main__":
    main()
