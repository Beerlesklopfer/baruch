#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
"""Drittlizenzen der ausgelieferten Binärdatei aus `cargo metadata` erzeugen.

Schreibt eine Textdatei mit allen Crates, die für das Ziel in `baruch-server` gelinkt werden
(gewöhnliche Abhängigkeiten, keine reinen Bau- oder Test-Abhängigkeiten), samt Version, Lizenz
(SPDX-Ausdruck aus dem Manifest), Quelle und den Lizenzdateien, die das Crate mitbringt. Gleiche
Lizenztexte stehen nur einmal in der Datei.

Daneben entsteht im selben Ordner `QUELLTEXT.txt`: das Quellcode-Angebot (AGPL-3.0, §6 und §13) mit der
Adresse des Repos und dem Tag genau dieser Version. Jedes Paket nimmt beide Dateien auf (.deb, AppImage,
.dmg, .msi), darum erzeugt sie ein Skript an einer Stelle.

    scripts/third-party-licenses.py [--target TRIPEL] [--package baruch-server] --out DATEI

Braucht nur Python 3 und cargo; baut nichts. Die Quellen der Crates müssen im Cargo-Cache liegen
(`cargo fetch --locked`). Ein Crate ohne Lizenzangabe ist ein Fehler (Status 1).
"""

import argparse
import hashlib
import json
import pathlib
import re
import subprocess
import sys

SOURCE_OFFER_NAME = "QUELLTEXT.txt"
REPOSITORY = "https://github.com/Beerlesklopfer/baruch"
HOMEPAGE = "https://baruch.bernau.family/"


def source_offer(version: str) -> str:
    """Text des Quellcode-Angebots für die Version `version` (liegt in jedem Paket neben der Lizenz)."""
    return "\n".join([
        f"Baruch {version} – Quelltext",
        "=" * (len(f"Baruch {version} – Quelltext")),
        "",
        "Baruch ist freie Software unter der GNU Affero General Public License, Version 3 oder (nach Wahl)",
        "jeder späteren Version (AGPL-3.0-or-later). Der Lizenztext liegt diesem Paket als LICENSE bei; im",
        "laufenden Programm zeigt ihn die Seite „Über Baruch“ (Werkzeuge → Über Baruch, /ueber/lizenz).",
        "",
        "Der vollständige Quelltext genau dieser Version steht unter",
        "",
        f"    {REPOSITORY}",
        "",
        f"beim Tag v{version}:",
        "",
        f"    {REPOSITORY}/releases/tag/v{version}",
        f"    git clone --branch v{version} {REPOSITORY}.git",
        "",
        "Wie man ihn baut, steht in README.md (Abschnitt „Building from source“). Wer Baruch verändert und",
        "anderen über ein Netz anbietet, muss ihnen den Quelltext seiner Fassung ebenso zugänglich machen",
        "(AGPL §13).",
        "",
        "Die Bibliotheken Dritter, die im Programm baruch-server enthalten sind, stehen mit Version, Lizenz,",
        "Quelle und Lizenztext in THIRD-PARTY-LICENSES.txt neben dieser Datei.",
        "",
        "Baruch kommt ohne jede Gewährleistung, soweit das Gesetz es zulässt (AGPL §15 und §16).",
        "",
        f"Webseite: {HOMEPAGE}",
        "",
        "----",
        "",
        f"Baruch {version} is free software under the GNU Affero General Public License, version 3 or (at your",
        "option) any later version. The complete corresponding source code of exactly this version is available",
        f"at {REPOSITORY} (tag v{version}).",
        "",
    ])


LICENSE_FILE = re.compile(r"^(licen[sc]e|copying|copyright|notice|unlicense|authors)([-_.].*)?$", re.IGNORECASE)
MAX_LICENSE_BYTES = 256 * 1024


def cargo_metadata(root: pathlib.Path, target: str | None) -> dict:
    cmd = ["cargo", "metadata", "--format-version", "1", "--locked"]
    if target:
        cmd += ["--filter-platform", target]
    out = subprocess.run(cmd, cwd=root, check=True, capture_output=True, text=True).stdout
    return json.loads(out)


def linked_packages(meta: dict, package: str) -> list[dict]:
    """Alle Pakete, die über gewöhnliche Abhängigkeiten von `package` erreichbar sind."""
    by_id = {p["id"]: p for p in meta["packages"]}
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    roots = [p["id"] for p in meta["packages"] if p["name"] == package and p["id"] in meta["workspace_members"]]
    if not roots:
        sys.exit(f"Paket {package} ist kein Mitglied des Workspace.")
    seen: set[str] = set()
    stack = list(roots)
    while stack:
        pid = stack.pop()
        if pid in seen:
            continue
        seen.add(pid)
        for dep in nodes[pid]["deps"]:
            # kind None = ordinary dependency; "build" and "dev" do not end up in the binary.
            if any(k.get("kind") is None for k in dep["dep_kinds"]):
                stack.append(dep["pkg"])
    return sorted((by_id[i] for i in seen), key=lambda p: (p["name"].lower(), p["version"]))


def license_files(pkg: dict) -> list[pathlib.Path]:
    folder = pathlib.Path(pkg["manifest_path"]).parent
    found = []
    if pkg.get("license_file"):
        explicit = (folder / pkg["license_file"]).resolve()
        if explicit.is_file():
            found.append(explicit)
    if folder.is_dir():
        for entry in sorted(folder.iterdir()):
            if entry.is_file() and LICENSE_FILE.match(entry.name) and entry.resolve() not in found:
                found.append(entry.resolve())
    return found


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--target", help="Ziel-Tripel, z. B. x86_64-unknown-linux-gnu (Vorgabe: alle Plattformen)")
    parser.add_argument("--package", default="baruch-server")
    parser.add_argument("--out", required=True)
    args = parser.parse_args()

    root = pathlib.Path(__file__).resolve().parent.parent
    meta = cargo_metadata(root, args.target)
    members = set(meta["workspace_members"])
    version = next(p["version"] for p in meta["packages"] if p["name"] == args.package and p["id"] in members)
    packages = [p for p in linked_packages(meta, args.package) if p["id"] not in members]

    texts: dict[str, tuple[int, str]] = {}  # sha256 -> (number, text)
    users: dict[int, list[str]] = {}
    lines: list[str] = []
    missing_license: list[str] = []
    without_text: list[str] = []

    for pkg in packages:
        label = f"{pkg['name']} {pkg['version']}"
        license_expr = pkg.get("license") or (f"siehe Lizenzdatei {pkg['license_file']}" if pkg.get("license_file") else "")
        if not license_expr:
            missing_license.append(label)
        source = pkg.get("repository") or pkg.get("homepage") or f"https://crates.io/crates/{pkg['name']}"
        if pkg.get("source") is None:
            source += " (im Quelltext von Baruch eingebettet, vendor/)"
        numbers = []
        for path in license_files(pkg):
            raw = path.read_bytes()[:MAX_LICENSE_BYTES]
            text = raw.decode("utf-8", errors="replace").replace("\r\n", "\n").strip("\n")
            if not text.strip():
                continue
            digest = hashlib.sha256(" ".join(text.split()).encode()).hexdigest()
            if digest not in texts:
                texts[digest] = (len(texts) + 1, text)
            number = texts[digest][0]
            if number not in numbers:
                numbers.append(number)
                users.setdefault(number, []).append(label)
        if not numbers:
            without_text.append(label)
        refs = ", ".join(f"[{n}]" for n in numbers) if numbers else "keine Lizenzdatei im Crate"
        authors = "; ".join(pkg.get("authors") or [])
        lines.append(f"{label}\n    Lizenz:      {license_expr}\n    Quelle:      {source}\n"
                     + (f"    Urheber:     {authors}\n" if authors else "")
                     + f"    Lizenztexte: {refs}\n")

    target = args.target or "alle Plattformen"
    out = [
        "Baruch – Lizenzen der eingebundenen Bibliotheken Dritter",
        "=========================================================",
        "",
        "Baruch selbst steht unter der AGPL-3.0-or-later (siehe LICENSE). Die Binärdatei baruch-server",
        f"enthält die folgenden {len(packages)} Rust-Crates (Ziel: {target}). Erzeugt von",
        "scripts/third-party-licenses.py aus `cargo metadata` und den Lizenzdateien der Crates; Nummern in",
        "eckigen Klammern verweisen auf die Lizenztexte im zweiten Teil.",
        "",
        "Teil 1: Crates",
        "--------------",
        "",
        *lines,
        "Teil 2: Lizenztexte",
        "-------------------",
        "",
    ]
    for number, text in sorted(texts.values()):
        used = users[number]
        shown = ", ".join(used[:12]) + (f" und {len(used) - 12} weitere" if len(used) > 12 else "")
        out += [f"[{number}] verwendet von: {shown}", "-" * 78, text, "", ""]

    path = pathlib.Path(args.out)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(out), encoding="utf-8")

    print(f"{path}: {len(packages)} Crates, {len(texts)} verschiedene Lizenztexte, {path.stat().st_size} Bytes")
    offer = path.parent / SOURCE_OFFER_NAME
    offer.write_text(source_offer(version), encoding="utf-8")
    print(f"{offer}: Quellcode-Angebot für Version {version}")
    if without_text:
        print(f"Hinweis: {len(without_text)} Crates ohne Lizenzdatei im Crate (nur SPDX-Angabe): "
              + ", ".join(without_text), file=sys.stderr)
    if missing_license:
        print("FEHLER: Crates ohne Lizenzangabe: " + ", ".join(missing_license), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
