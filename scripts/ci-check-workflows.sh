#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-or-later
# Prüft die Regeln der Arbeitsabläufe unter .github/workflows (docs/Verteilung.md, „Regeln der Arbeitsabläufe“).
#
#   scripts/ci-check-workflows.sh [ORDNER]
#
# Braucht python3 mit PyYAML (Debian/Ubuntu: python3-yaml). Jede Regel nennt ihren Grund; ein Verstoß endet mit
# Exit 1 und einer Zeile je Fundstelle. Ersetzt weder actionlint noch einen echten Lauf.
set -eu

root="$(cd "$(dirname "$0")/.." && pwd)"
dir="${1:-$root/.github/workflows}"

exec python3 - "$dir" <<'PY'
import glob
import os
import re
import sys

try:
    import yaml
except ImportError:
    sys.exit("ci-check-workflows.sh: PyYAML fehlt (python3-yaml).")

directory = sys.argv[1]
files = sorted(glob.glob(os.path.join(directory, "*.yml")) + glob.glob(os.path.join(directory, "*.yaml")))
if not files:
    sys.exit(f"ci-check-workflows.sh: keine Arbeitsabläufe in {directory}.")

problems = []


def problem(name, text):
    problems.append(f"{name}: {text}")


USES = re.compile(r"^\s*(?:-\s+)?uses:\s*(\S+)(?:\s+#\s*(\S+))?\s*$")
PINNED = re.compile(r"^[A-Za-z0-9_.-]+/[A-Za-z0-9_./-]+@[0-9a-f]{40}$")
VERSION = re.compile(r"^v\d+\.\d+\.\d+$")
# Rechte, die über das Lesen hinausgehen, und wo sie stehen dürfen (Datei, Job).
WRITE_CONTENTS = {("release.yml", "publish")}
# Geheimnisse des Archivschlüssels und die Umgebung, an die sie gebunden sind.
ARCHIVE_SECRET = re.compile(r"secrets\.APT_GPG_")
ARCHIVE_ENVIRONMENT = "release"
rust_versions = {}
# Die beiden Aktionen der Auslieferung über Pages (Regel 11).
UPLOAD_PAGES = "actions/upload-pages-artifact"
DEPLOY_PAGES = "actions/deploy-pages"


def action_of(step):
    """Name der Aktion eines Schritts ohne `@SHA`; leer bei einem `run`-Schritt."""
    return str((step or {}).get("uses") or "").split("@")[0]


for path in files:
    name = os.path.basename(path)
    text = open(path, encoding="utf-8").read()
    try:
        doc = yaml.safe_load(text)
    except yaml.YAMLError as error:
        problem(name, f"kein gültiges YAML: {error}")
        continue
    if not isinstance(doc, dict):
        problem(name, "kein Arbeitsablauf (oberste Ebene ist keine Zuordnung).")
        continue

    # 1. Aktionen nur mit 40-stelligem Commit-SHA und der Version als Kommentar. Grund: Ein Tag lässt sich
    #    verschieben, ein SHA nicht; der Kommentar sagt, gegen welches Tag der SHA geprüft wurde.
    for number, line in enumerate(text.splitlines(), 1):
        if re.match(r"^\s*(?:-\s+)?uses:", line):
            match = USES.match(line)
            if not match or not PINNED.match(match.group(1)):
                problem(name, f"Zeile {number}: `uses` ohne 40-stelligen SHA: {line.strip()}")
            elif not match.group(2) or not VERSION.match(match.group(2)):
                problem(name, f"Zeile {number}: `uses` ohne Versionskommentar (# vX.Y.Z): {line.strip()}")

    # 2. Auslöser. Grund: pull_request_target führt fremden Code mit den Rechten und Geheimnissen des
    #    Repositorys aus. (PyYAML liest den Schlüssel `on` als True.)
    triggers = doc.get("on", doc.get(True))
    listed = triggers if isinstance(triggers, (dict, list)) else [triggers]
    if "pull_request_target" in listed:
        problem(name, "Auslöser pull_request_target ist verboten.")

    # 3. Oberste Rechte genau `contents: read`. Grund: Jeder Job ohne eigene Angabe erbt sie.
    if doc.get("permissions") != {"contents": "read"}:
        problem(name, f"oberste permissions müssen genau contents: read sein, sind: {doc.get('permissions')!r}")

    # 4. Rust in fester Version in genau einer env-Variablen je Ablauf. Grund: derselbe Compiler wie lokal;
    #    `stable` wandert und bringt neue clippy-Warnungen in einen sonst unveränderten Stand.
    uses_rust = re.search(r"\b(cargo|rustup)\b", text) is not None
    rust = (doc.get("env") or {}).get("RUST_VERSION")
    if uses_rust:
        if not isinstance(rust, str) or not re.match(r"^\d+\.\d+\.\d+$", rust):
            problem(name, f"env RUST_VERSION fehlt oder ist keine feste Version: {rust!r}")
        else:
            rust_versions[name] = rust
        if re.search(r"rustup\s+(toolchain\s+install|default)\s+(stable|beta|nightly)\b", text):
            problem(name, "rustup mit stable/beta/nightly statt RUST_VERSION.")
        if text.count("RUST_VERSION:") != 1:
            problem(name, "RUST_VERSION muss an genau einer Stelle gesetzt sein.")

    jobs = doc.get("jobs") or {}
    if not jobs:
        problem(name, "keine Jobs.")
    for job_name, job in jobs.items():
        where = f"Job {job_name}"
        permissions = job.get("permissions") or {}
        environment = job.get("environment")
        environment = environment.get("name") if isinstance(environment, dict) else environment
        steps = job.get("steps") or []

        # 5. Schreibrechte nur dort, wo sie gebraucht werden. Grund: Ein übernommener Bauschritt soll weder
        #    Releases noch die Webseite ändern können.
        for scope, level in permissions.items():
            if level != "write":
                continue
            if scope == "contents":
                if (name, job_name) not in WRITE_CONTENTS:
                    problem(name, f"{where}: contents: write ist nur in release.yml/publish erlaubt.")
            elif scope in ("pages", "id-token"):
                if environment != "github-pages":
                    problem(name, f"{where}: {scope}: write nur mit environment github-pages.")
            else:
                problem(name, f"{where}: unerwartetes Schreibrecht {scope}.")
        if environment == "github-pages":
            wanted = {"contents": "read", "pages": "write", "id-token": "write"}
            if permissions != wanted:
                problem(name, f"{where}: Pages-Job braucht genau {wanted}, hat {permissions!r}")
            # 11. Bauen und Ausliefern in zwei Jobs. Grund: Am 2026-11-24 fand deploy-pages im selben Job das eben
            #     hochgeladene Artefakt zweimal nicht, und das Wiederholen legte ein zweites Artefakt an. Außerdem
            #     soll der Job mit Schreibrecht und OIDC-Token keinen Code des Repositorys ausführen. Darum: Der
            #     Job der Umgebung github-pages besteht nur aus deploy-pages und hängt (needs) an einem Job
            #     derselben Datei, der upload-pages-artifact ausführt und nichts schreiben darf.
            if any(action_of(step) == UPLOAD_PAGES for step in steps):
                problem(name, f"{where}: upload-pages-artifact und deploy-pages im selben Job; Bauen gehört in einen eigenen Job davor.")
            if not steps or any(action_of(step) != DEPLOY_PAGES for step in steps):
                problem(name, f"{where}: der Pages-Job darf nur actions/deploy-pages ausführen.")
            needs = job.get("needs") or []
            needs = [needs] if isinstance(needs, str) else needs
            builders = [
                other for other in needs
                if isinstance(jobs.get(other), dict)
                and any(action_of(step) == UPLOAD_PAGES for step in jobs[other].get("steps") or [])
            ]
            if not builders:
                problem(name, f"{where}: needs nennt keinen Job mit upload-pages-artifact (Bauen vor dem Ausliefern).")
        elif any(action_of(step) == DEPLOY_PAGES for step in steps):
            problem(name, f"{where}: deploy-pages nur in einem Job mit environment github-pages.")
        # 11. (Gegenstück) Wer das Artefakt der Seite baut, darf nichts schreiben.
        if any(action_of(step) == UPLOAD_PAGES for step in steps):
            if any(level == "write" for level in permissions.values()):
                problem(name, f"{where}: Job mit upload-pages-artifact darf keine Schreibrechte haben.")

        for index, step in enumerate(job.get("steps") or [], 1):
            label = f"{where}, Schritt {index} ({step.get('name') or step.get('uses') or '?'})"
            uses = step.get("uses") or ""
            # 6. checkout ohne gespeichertes Token. Grund: Spätere Schritte (Bauskripte, npm) sollen das Token
            #    nicht in .git/config vorfinden.
            if uses.startswith("actions/checkout@"):
                if (step.get("with") or {}).get("persist-credentials") is not False:
                    problem(name, f"{label}: checkout ohne persist-credentials: false.")
            run = step.get("run")
            if isinstance(run, str):
                # 7. Keine Ausdrücke in Befehlen. Grund: `${{ … }}` wird vor der Shell eingesetzt – ein
                #    Geheimnis stünde in der Befehlszeile, ein fremder Wert (Tag, Titel) wäre Schadcode.
                #    Werte kommen über `env` in den Schritt.
                if "${{" in run:
                    problem(name, f"{label}: `${{{{ … }}}}` in run; Werte nur über env.")
            # 8. Geheimnisse nur in `env` eines Schritts. Grund: so sieht sie nur dieser eine Schritt.
            for key, value in step.items():
                if key != "env" and "secrets." in str(value):
                    problem(name, f"{label}: Geheimnis außerhalb von env ({key}).")
            # 10. Der Archivschlüssel nur in einem Job der Umgebung `release`. Grund: Ein Secret des
            #     Repositorys sieht jeder Lauf; ein Secret einer Umgebung nur der Job, der ihre Schutzregeln
            #     (Freigabe, nur Tags v*) bestanden hat. Der Schlüssel signiert, was Nutzer als root installieren.
            if ARCHIVE_SECRET.search(str(step.get("env", ""))) and environment != ARCHIVE_ENVIRONMENT:
                problem(name, f"{label}: APT_GPG_* nur in einem Job mit environment {ARCHIVE_ENVIRONMENT}.")
        for key in ("env", "with", "if", "name", "runs-on"):
            if "secrets." in str(job.get(key, "")):
                problem(name, f"{where}: Geheimnis auf Job-Ebene ({key}).")
    if "secrets." in str(doc.get("env", "")):
        problem(name, "Geheimnis in der obersten env (gälte für alle Schritte).")

# 9. Dieselbe Rust-Version in allen Abläufen. Grund: Getestet (ci.yml) und ausgeliefert (release.yml) wird mit
#    demselben Compiler.
if len(set(rust_versions.values())) > 1:
    problem("alle", f"RUST_VERSION weicht ab: {rust_versions}")

if problems:
    print("\n".join(problems), file=sys.stderr)
    sys.exit(1)
print(f"ci-check-workflows.sh: {len(files)} Arbeitsabläufe in Ordnung ({', '.join(os.path.basename(f) for f in files)}).")
PY
