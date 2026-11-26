# Baruch als Benutzerdienst (systemd --user)

Baruch läuft als Dienst des angemeldeten Benutzers, nicht als Systemdienst: So erreicht er den
Schlüsselbund (Schlüssel für Eliezer, MultiGPT, pgvector) und den eigenen Datenordner.

## Mit dem Paket (.deb)

Wer Baruch aus dem Paket `baruch` installiert hat (APT-Repository oder `baruch_<Version>-1_amd64.deb`, siehe
[docs/Verteilung.md](../../docs/Verteilung.md)), braucht nichts zu bauen. Das Paket richtet **keinen** Dienst
ein – „Baruch“ im Startmenü bzw. `baruch` im Terminal startet den Server bei Bedarf. Für den Dauerbetrieb
liegt die Unit als Beispiel bei (mit `ExecStart=/usr/bin/baruch-server`):

```sh
install -Dm644 /usr/share/doc/baruch/examples/baruch.service ~/.config/systemd/user/baruch.service
systemctl --user daemon-reload
systemctl --user enable --now baruch.service
```

Nach `sudo apt upgrade` den Dienst neu starten (`systemctl --user restart baruch.service`); das Paket tut das
nicht selbst, weil es keine Dienste anfasst. Entfernen des Pakets lässt die Unit unter `~/.config` und die
Daten unter `~/.local/share/baruch` unberührt – beides gehört dem Benutzer.

## Einrichten aus dem Quelltext

Die Unit in diesem Ordner (`packaging/systemd/baruch.service`) erwartet das Programm unter `~/.cargo/bin`;
die Unit des Pakets (`packaging/debian/baruch.service`) unterscheidet sich nur in dieser Zeile.

```sh
# 1. Programm bauen und installieren (nach ~/.cargo/bin/baruch-server)
cd crates/baruch-server/web && npm ci && cd -
nice -n 10 ionice -c3 cargo install --path crates/baruch-server -j 4

# 2. Unit installieren und starten
install -Dm644 packaging/systemd/baruch.service ~/.config/systemd/user/baruch.service
systemctl --user daemon-reload
systemctl --user enable --now baruch.service

# 3. Prüfen
systemctl --user status baruch.service
curl -s http://127.0.0.1:8765/ -o /dev/null -w '%{http_code}\n'   # 200
```

Danach im Browser `http://127.0.0.1:8765/` öffnen; Claude Code erreicht den MCP-Server unter
`http://127.0.0.1:8765/mcp`.

## Bedienen

| Was | Befehl |
|---|---|
| Protokoll ansehen | `journalctl --user -u baruch.service -f` |
| Neu starten (z. B. nach `cargo install` oder `apt upgrade`) | `systemctl --user restart baruch.service` |
| Anhalten | `systemctl --user stop baruch.service` |
| Abschalten | `systemctl --user disable --now baruch.service` |

## Hinweise

- **Port 8765** ist die Vorgabe des Programms und steht zur Klarheit auch in der Unit. Läuft schon ein
  Baruch von Hand (`cargo run -p baruch-server`) ohne eigenes `--listen`, belegt er denselben Port: vorher
  beenden, sonst kann der Dienst den Port nicht öffnen. Zum Ausprobieren `--listen 127.0.0.1:0` nehmen.
- **Datenordner:** ohne `BARUCH_DATA` `~/.local/share/baruch` (`dirs::data_local_dir()`, unter Linux gleich
  `$XDG_DATA_HOME`).
- **Auch ohne Anmeldung laufen lassen** (z. B. auf dem Schulrechner): `loginctl enable-linger $USER`.
  Dann fehlt allerdings die entsperrte Schlüsselbund-Sitzung; Schlüssel müssen dort anders hinterlegt werden.
- **Anderer Datenordner:** in der Unit `Environment=BARUCH_DATA=…` einkommentieren und
  `systemctl --user daemon-reload` ausführen. Ist er nicht der Vorgabe-Ordner, bleiben Geheimnisse sonst nur im
  Speicher; für den Schlüsselbund zusätzlich `Environment=BARUCH_SECRETS=keyring`.
- Nur `127.0.0.1`: Über andere Adressen ist Baruch nicht erreichbar. Für den Betrieb im Netz steht Baruch
  hinter einem Reverse-Proxy mit TLS, dazu Konten in Baruch (Einstellungen → Konten) oder ein Zugangsschutz im
  Proxy (ohne Konten hat Baruch keine Anmeldung); der Proxy-Name
  wird mit `--allow-host NAME` bzw. `Environment=BARUCH_ALLOWED_HOSTS=NAME` erlaubt (siehe `docs/Betrieb.md`).
