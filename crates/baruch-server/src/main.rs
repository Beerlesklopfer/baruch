// SPDX-License-Identifier: AGPL-3.0-or-later
//! Aufruf:
//! - `baruch-server [serve] [--listen 127.0.0.1:8765] [--data DIR] [--secrets keyring|memory]` (auch
//!   `BARUCH_LISTEN`, `BARUCH_DATA`, `BARUCH_SECRETS`). Geheimnisse der Verbindungen: Vorgabe ist der
//!   Schlüsselbund des Systems nur mit dem Vorgabe-Datenordner; mit jedem anderen `--data` bleiben sie im
//!   Speicher des Prozesses ([`baruch_server::connections::secrets_mode`]), damit Wegwerf- und Prüfserver
//!   die Einträge `baruch/…` des eigentlichen Dienstes weder lesen noch überschreiben oder löschen.
//! - `baruch-server --version`: gibt `baruch-server <Version>` aus (Exit 0).
//! - `--allow-host NAME` (mehrfach, nur `serve`; auch `BARUCH_ALLOWED_HOSTS`, Komma-getrennt): zusätzlich erlaubte
//!   Rechnernamen für den Betrieb hinter einem Reverse-Proxy ([`baruch_server::guard::AllowedHosts`]). Ein
//!   ungültiger Name (Port, Platzhalter, IP-Adresse) beendet `serve` mit Exit 2 und nennt ihn; `import` und
//!   `export` lesen die Angabe nicht. Unter einem solchen Namen verlangt `/mcp` immer einen Token; den
//!   Zugangsschutz stellt der Proxy (`docs/Betrieb.md`).
//! - `baruch-server import <datei.typ> [--title "…"] [--data DIR]`: Typst-Datei als neues Werk; ist
//!   der Titel schon vergeben, Exit 1 mit Hinweis auf `--title`
//! - `baruch-server export <id|slug> [--out ORDNER] [--data DIR]`: Werk als Typst-Datei auf stdout;
//!   mit `--out` die Typst-Datei und alle Bilder in einen Ordner (vorhandene Dateien → Exit 1)
//! - `baruch-server export <id|slug> --pdf DATEI [--data DIR]`: das Buch als PDF (Runde 11, wie
//!   `/werke/{id}/pdf`); vorhandene Datei → Exit 1 ohne Überschreiben, Satzfehler → Exit 1 ohne Datei;
//!   `--pdf` und `--out` schließen sich aus (Exit 2)
//! - `baruch-server export <id|slug> --stichwort DATEI [--format a5|karten] [--schrift PT] [--data DIR]`:
//!   Stichwortzettel als PDF (Runde 16c, wie `/werke/{id}/stichwortzettel.pdf`); schließt sich mit `--pdf`
//!   und `--out` aus (Exit 2); vorhandene Datei → Exit 1 ohne Überschreiben, Satzfehler → Exit 1 ohne
//!   Datei. Logik in `baruch_server::pdf::notes::write_notes_file`.
//! - `baruch-server export <id|slug> --baruch DATEI [--data DIR]`: Austauschdatei (Runde 26); endet DATEI auf
//!   `.xml`, nur XML, sonst ZIP; schließt sich mit `--out`, `--pdf` und `--stichwort` aus (Exit 2); vorhandene
//!   Datei → Exit 1 ohne Überschreiben. Logik in `baruch_server::exchange::export::cli_export`.
//! - `baruch-server export <id|slug> --epub DATEI [--data DIR]`: das Werk als EPUB (Runde 20a, wie
//!   `/werke/{id}/epub`, Fußnoten als Popup); schließt sich mit `--out`, `--pdf`, `--stichwort` und `--baruch` aus
//!   (Exit 2); vorhandene Datei → Exit 1 ohne Überschreiben. Logik in `baruch_server::epub::write_epub_file`.
//! - `baruch-server import <datei.baruch|datei.baruch.xml> [--abgleichen] [--vorschau] [--title "…"] [--data DIR]`:
//!   Austauschdatei einlesen (Runde 26; entschieden über `exchange::is_exchange_name`): ohne Flag und unbekannt
//!   → neues Werk, ohne Flag und bekannt → Exit 1, `--abgleichen` gleicht ein bekanntes Werk ab, `--vorschau`
//!   schreibt nichts. `--abgleichen`/`--vorschau` mit einer `.typ`-Datei → Exit 2. Logik in
//!   `baruch_server::exchange::import::cli_import`.
//! - `baruch-server import <datei.pdf|datei.epub> [--uebernehmen] [--bibliothek] [--title "…"] [--data DIR]` (Runde 21a,
//!   entschieden über `pdf_import::is_source_name`): ohne `--uebernehmen` nur Gliederung und Warnungen, nichts
//!   geschrieben; mit `--uebernehmen` das Werk, mit `--bibliothek` (zusätzlich) die Quelle; immer lokal, ohne MultiGPT.
//!   Die beiden Flags mit einer anderen Dateiart → Exit 2. Logik in `baruch_server::pdf_import::cli_import`.
//! - `baruch-server import <datei.md|datei.markdown|datei.zip> [--title "…"] [--data DIR]` (Runde 21b): Markdown bzw.
//!   Archiv als neues Werk. Logik in `baruch_server::markdown_import::cli_import`.
//! - `baruch-server import <datei.docx> [--uebernehmen] [--title "…"] [--data DIR]` (Runde 32, entschieden über
//!   `docx_import::is_docx_name`): ohne `--uebernehmen` nur Gliederung, Bearbeiter und Warnungen, nichts geschrieben;
//!   mit `--uebernehmen` das Werk samt Vorschlägen (Änderungsverfolgung) und Beiträgen (Kommentare). `--bibliothek`
//!   mit einer Word-Datei → Exit 2. Logik in `baruch_server::docx_import::cli_import`.
//! - `baruch-server export <id|slug> --docx DATEI [--vorschlaege] [--data DIR]` (Runde 32): das Werk als Word-Datei,
//!   mit `--vorschlaege` die offenen Vorschläge als Änderungsverfolgung; schließt sich mit `--out`, `--pdf`,
//!   `--stichwort`, `--baruch` und `--epub` aus (Exit 2), `--vorschlaege` ohne `--docx` → Exit 2; vorhandene Datei →
//!   Exit 1 ohne Überschreiben. Logik in `baruch_server::docx_export::cli_export`.
//! - Beenden (Runde 32): `serve` läuft in `baruch_server::shutdown::serve` und endet sauber auf ein Signal oder über
//!   die Seite `/beenden` (nur vom eigenen Rechner).
//! - `baruch-server passwort <name> [--data DIR]` (Runde 35): Notweg bei vergessenem Passwort – liest das neue
//!   Passwort als eine Zeile von der Standardeingabe, setzt es für das vorhandene Konto und beendet dessen
//!   Sitzungen; eine Sperre nach Fehlversuchen wird aufgehoben; unbekanntes Konto oder ungültiges Passwort →
//!   Exit 1. Das Passwort steht nie auf der Befehlszeile (dort läse es jeder Benutzer des Rechners in der
//!   Prozessliste). Logik in `baruch_server::auth::cli::reset_password`.
//! - `baruch-server typst …` (Runde 36), ebenso das Programm unter dem Namen `baruch-typst` oder `typst` gerufen
//!   (Symlink; die Pakete liefern nur `baruch-typst`): die Typst-Kommandozeile mit der eingebauten Engine –
//!   `compile EINGABE [AUSGABE]` mit `--root`, `--font-path`, `--format pdf|png|svg`, `--ppi`; `fonts`; `--version`
//!   (`typst <Typst-Version> (Baruch <Version>)`); `--help`. Kein Datenordner, kein Server, kein Netz, keine Pakete;
//!   Nicht-Unterstütztes (watch, query …) → Exit 2. Entschieden wird vor allem anderen in `main`
//!   ([`baruch_server::typst_cli::dispatch`]), Argumente in `baruch_server::typst_cli`, Satz in
//!   `baruch_render::cli`.
//!
//! Vorgabe für die Adresse: `127.0.0.1:8765`. Vorgabe für die Daten: [`baruch_store::default_data_dir`],
//! unter Linux `~/.local/share/baruch` (bzw. `$XDG_DATA_HOME/baruch`), unter Windows
//! `%LOCALAPPDATA%\baruch`.
//! Die Logik steht in der Bibliothek (`src/lib.rs`), hier nur Start und Argumente.

use std::path::PathBuf;

use baruch_server::{AppState, rpc};
use baruch_store::{Store, StoreError};

const HELP: &str =
    "baruch-server [serve] [--listen ADRESSE:PORT] [--data ORDNER] [--secrets keyring|memory] [--allow-host NAME]…
baruch-server --version
baruch-server import DATEI.typ [--title \"TITEL\"] [--data ORDNER]
baruch-server export ID|SLUG [--out ORDNER | --pdf DATEI] [--data ORDNER]
baruch-server export ID|SLUG --stichwort DATEI [--format a5|karten] [--schrift PT] [--data ORDNER]
baruch-server export ID|SLUG --baruch DATEI [--data ORDNER]
baruch-server export ID|SLUG --epub DATEI [--data ORDNER]
baruch-server import DATEI.baruch|DATEI.baruch.xml [--abgleichen] [--vorschau] [--title TITEL] [--data ORDNER]
baruch-server import DATEI.md|DATEI.markdown|DATEI.zip [--title \"TITEL\"] [--data ORDNER]
baruch-server import DATEI.pdf|DATEI.epub [--uebernehmen] [--bibliothek] [--title \"TITEL\"] [--data ORDNER]
baruch-server import DATEI.docx [--uebernehmen] [--title \"TITEL\"] [--data ORDNER]
baruch-server export ID|SLUG --docx DATEI [--vorschlaege] [--data ORDNER]
baruch-server passwort NAME [--data ORDNER]
baruch-server typst compile EINGABE.typ [AUSGABE] [--root ORDNER] [--font-path ORDNER]… [--format pdf|png|svg]
baruch-server typst fonts | --version | --help
  serve    Server starten (Vorgabe); beenden mit Strg+C oder über die Seite /beenden (Baruch ▾ → Baruch beenden…,
            nur vom eigenen Rechner)
  import    Typst-Datei als neues Werk anlegen (Präambel = Werkvorlage, =/==/=== = Gliederung)
            Markdown (.md, .markdown) wird nach Typst umgewandelt; ein ZIP bringt eine Haupt-Datei
            (.typ oder .md) und Bilder mit; PDF und EPUB zeigen ohne --uebernehmen nur die Gliederung
  --title   eigener Titel beim Import (Vorgabe: #set document, sonst Dateiname); Titel sind eindeutig
  --uebernehmen beim PDF/EPUB-Import: das Werk mit der gezeigten Gliederung anlegen (ohne das Flag nur
            Vorschau der Gliederung und Warnungen, nichts wird geschrieben; die CLI liest immer lokal,
            ohne MultiGPT)
  --bibliothek beim PDF/EPUB-Import: die Datei als Quelle in die Bibliothek (mit --uebernehmen zusätzlich
            zum Werk)
            Word (.docx) zeigt ohne --uebernehmen nur Gliederung, Bearbeiter und Warnungen; mit --uebernehmen
            entsteht das Werk im Stand vor den verfolgten Änderungen, die Änderungen je Bearbeiter werden
            offene Vorschläge, Kommentare werden Beiträge
  export    Werk als eine Typst-Datei auf die Standardausgabe schreiben
  --out     beim Export: Typst-Datei und Bilder in diesen Ordner schreiben (nichts wird überschrieben)
  --pdf     beim Export: das Buch als PDF in diese Datei setzen (Werkvorlage, aktuelle Fassungen,
            Bilder; nichts wird überschrieben, bei einem Satzfehler entsteht keine Datei)
  --stichwort beim Export: Stichwortzettel als PDF in diese Datei (je Abschnitt Zeitmarke,
            Stichpunkte, Bibelstellen; nichts wird überschrieben)
  --format  beim Stichwortzettel: a5 (A5 hoch, Vorgabe) oder karten (A6 quer, je Abschnitt eine Karte)
  --schrift beim Stichwortzettel: Schriftgröße 14–28 Punkt (Vorgabe 16 bei a5, 14 bei karten)
  --baruch  beim Export: das ganze Werk als Austauschdatei für eine andere Baruch-Instanz (DATEI.baruch als
            ZIP, DATEI.baruch.xml nur XML; nichts wird überschrieben)
  --epub    beim Export: das Werk als EPUB 3 in diese Datei (aktuelle Fassungen, Bilder, Umschlag; nichts wird
            überschrieben)
  --docx    beim Export: das Werk als Word-Datei (.docx) in diese Datei (aktuelle Fassungen, Bilder; nichts wird
            überschrieben)
  --vorschlaege beim Word-Export: offene Vorschläge als Änderungsverfolgung mitgeben (nur mit --docx)
  --abgleichen beim Einlesen einer Austauschdatei: ein hier schon vorhandenes Werk abgleichen (nur
            ergänzen, nie löschen oder überschreiben; Gliederungsänderungen werden Anträge)
  --vorschau beim Einlesen einer Austauschdatei: nichts schreiben, nur zeigen, was hinzukäme
  passwort  Notweg bei vergessenem Passwort: das neue Passwort (10 bis 200 Zeichen) als eine Zeile von der
            Standardeingabe lesen und für das Konto NAME setzen; alle Sitzungen des Kontos enden, eine Sperre
            nach Fehlversuchen wird aufgehoben. Beispiel:
            baruch-server passwort Erika < datei-mit-passwort   (oder eintippen und mit Eingabe abschließen)
  typst     die Typst-Kommandozeile mit der eingebauten Engine (ohne Datenordner, ohne Netz, ohne Pakete):
            eine .typ-Datei zu PDF, PNG oder SVG setzen; eine vorhandene Ausgabedatei wird überschrieben.
            Auch als baruch-typst … (Symlink auf dieses Programm). Einzelheiten: baruch-server typst --help
  --listen  Adresse des Servers (Vorgabe 127.0.0.1:8765, auch BARUCH_LISTEN)
  --data    Datenordner mit baruch.sqlite und den Git-Spiegeln unter works/
            (Vorgabe ~/.local/share/baruch bzw. %LOCALAPPDATA%\\baruch, auch BARUCH_DATA)
  --secrets Ablage der Geheimnisse der Verbindungen: keyring (Schlüsselbund des Systems, Dienst baruch)
            oder memory (nur im Speicher dieses Prozesses); Vorgabe: keyring nur mit dem
            Vorgabe-Datenordner, sonst memory (auch BARUCH_SECRETS)
  --allow-host zusätzlich erlaubter Rechnername hinter einem Reverse-Proxy (mehrfach; auch BARUCH_ALLOWED_HOSTS,
            Komma-getrennt), z. B. baruch.example.org – ohne Port, ohne Platzhalter, keine IP-Adresse. Der Proxy
            muss Host durchreichen und – solange keine Konten eingerichtet sind – den Zugang schützen (ohne
            Konten hat Baruch keine Anmeldung); /mcp verlangt unter diesem Namen immer einen Token.
            Siehe docs/Betrieb.md
  --version Version ausgeben (baruch-server VERSION)";

enum Command {
    Serve,
    Import(PathBuf),
    Export(String),
    /// Runde 35: Passwort eines Kontos neu setzen (Name des Kontos).
    Password(String),
}

fn usage(message: &str) -> ! {
    eprintln!("{message}\n\n{HELP}");
    std::process::exit(2);
}

fn main() {
    // Runde 36: Typst-Kommandozeile (Unterbefehl `typst` oder Aliasname in argv[0]) vor allem anderen, ohne Laufzeit.
    if let Some(code) = baruch_server::typst_cli::dispatch(std::env::args_os().collect()) {
        std::process::exit(code)
    }
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Tokio-Laufzeit lässt sich nicht starten")
        .block_on(run_main())
}

async fn run_main() {
    let mut args = std::env::args().skip(1).peekable();
    let mut listen = std::env::var("BARUCH_LISTEN").unwrap_or_else(|_| "127.0.0.1:8765".into());
    let mut data = std::env::var_os("BARUCH_DATA").map(PathBuf::from).unwrap_or_else(baruch_store::default_data_dir);
    let mut title: Option<String> = None;
    let mut out: Option<PathBuf> = None;
    let mut pdf: Option<PathBuf> = None;
    let mut notes: Option<PathBuf> = None;
    let mut notes_format: Option<String> = None;
    let mut notes_font: Option<String> = None;
    let mut baruch: Option<PathBuf> = None;
    let mut epub: Option<PathBuf> = None;
    let mut docx: Option<PathBuf> = None;
    let mut docx_proposals = false;
    let mut merge = false;
    let mut preview_only = false;
    let mut source_apply = false;
    let mut source_library = false;
    let mut secrets: Option<String> = std::env::var("BARUCH_SECRETS").ok().filter(|s| !s.trim().is_empty());
    // Runde 24: zusätzlich erlaubte Rechnernamen (Umgebung zuerst, dann jede Angabe `--allow-host`).
    let mut allow_hosts: Vec<String> = std::env::var("BARUCH_ALLOWED_HOSTS").ok().into_iter().collect();
    let command = match args.peek().map(String::as_str) {
        Some("serve") => {
            args.next();
            Command::Serve
        }
        Some("import") => {
            args.next();
            match args.next() {
                Some(file) if !file.starts_with("--") => Command::Import(PathBuf::from(file)),
                _ => usage("import braucht eine Datei"),
            }
        }
        Some("export") => {
            args.next();
            match args.next() {
                Some(work) if !work.starts_with("--") => Command::Export(work),
                _ => usage("export braucht eine Werk-id oder einen Ordnernamen (slug)"),
            }
        }
        Some("passwort") => {
            args.next();
            match args.next() {
                Some(name) if !name.starts_with("--") => Command::Password(name),
                _ => usage("passwort braucht den Namen des Kontos"),
            }
        }
        _ => Command::Serve,
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--listen" => listen = args.next().unwrap_or_else(|| usage("--listen braucht eine Adresse")),
            "--data" => data = PathBuf::from(args.next().unwrap_or_else(|| usage("--data braucht einen Ordner"))),
            "--secrets" if matches!(command, Command::Serve) => {
                secrets = Some(args.next().unwrap_or_else(|| usage("--secrets braucht keyring oder memory")))
            }
            "--allow-host" if matches!(command, Command::Serve) => {
                allow_hosts.push(args.next().unwrap_or_else(|| usage("--allow-host braucht einen Rechnernamen")))
            }
            "--version" | "-V" => {
                println!("baruch-server {}", baruch_server::about::VERSION);
                return;
            }
            "--title" if matches!(command, Command::Import(_)) => {
                title = Some(args.next().unwrap_or_else(|| usage("--title braucht einen Titel")))
            }
            "--out" if matches!(command, Command::Export(_)) => {
                out = Some(PathBuf::from(args.next().unwrap_or_else(|| usage("--out braucht einen Ordner"))))
            }
            "--pdf" if matches!(command, Command::Export(_)) => {
                pdf = Some(PathBuf::from(args.next().unwrap_or_else(|| usage("--pdf braucht eine Datei"))))
            }
            "--stichwort" if matches!(command, Command::Export(_)) => {
                notes = Some(PathBuf::from(args.next().unwrap_or_else(|| usage("--stichwort braucht eine Datei"))))
            }
            "--format" if matches!(command, Command::Export(_)) => {
                notes_format = Some(args.next().unwrap_or_else(|| usage("--format braucht a5 oder karten")))
            }
            "--schrift" if matches!(command, Command::Export(_)) => {
                notes_font = Some(args.next().unwrap_or_else(|| usage("--schrift braucht eine Zahl (Punkt)")))
            }
            "--baruch" if matches!(command, Command::Export(_)) => {
                baruch = Some(PathBuf::from(args.next().unwrap_or_else(|| usage("--baruch braucht eine Datei"))))
            }
            "--epub" if matches!(command, Command::Export(_)) => {
                epub = Some(PathBuf::from(args.next().unwrap_or_else(|| usage("--epub braucht eine Datei"))))
            }
            "--docx" if matches!(command, Command::Export(_)) => {
                docx = Some(PathBuf::from(args.next().unwrap_or_else(|| usage("--docx braucht eine Datei"))))
            }
            "--vorschlaege" if matches!(command, Command::Export(_)) => docx_proposals = true,
            "--abgleichen" if matches!(command, Command::Import(_)) => merge = true,
            "--vorschau" if matches!(command, Command::Import(_)) => preview_only = true,
            "--uebernehmen" if matches!(command, Command::Import(_)) => source_apply = true,
            "--bibliothek" if matches!(command, Command::Import(_)) => source_library = true,
            "--help" | "-h" => {
                println!("{HELP}");
                return;
            }
            other => usage(&format!("unbekannte Angabe: {other}")),
        }
    }
    if out.is_some() && pdf.is_some() {
        usage("--pdf und --out schließen sich aus: entweder das PDF oder Typst-Datei und Bilder");
    }
    if notes.is_some() && (out.is_some() || pdf.is_some()) {
        usage("--stichwort schließt sich mit --pdf und --out aus: ein Export zur Zeit");
    }
    if baruch.is_some() && (out.is_some() || pdf.is_some() || notes.is_some()) {
        usage("--baruch schließt sich mit --out, --pdf und --stichwort aus: ein Export zur Zeit");
    }
    if epub.is_some() && (out.is_some() || pdf.is_some() || notes.is_some() || baruch.is_some()) {
        usage("--epub schließt sich mit --out, --pdf, --stichwort und --baruch aus: ein Export zur Zeit");
    }
    if docx.is_some() && (out.is_some() || pdf.is_some() || notes.is_some() || baruch.is_some() || epub.is_some()) {
        usage("--docx schließt sich mit --out, --pdf, --stichwort, --baruch und --epub aus: ein Export zur Zeit");
    }
    if docx_proposals && docx.is_none() {
        usage("--vorschlaege gilt nur mit --docx");
    }
    let exchange_import = match &command {
        Command::Import(file) => baruch_server::exchange::is_exchange_name(&file.to_string_lossy()),
        _ => false,
    };
    if (merge || preview_only) && !exchange_import {
        usage("--abgleichen und --vorschau gelten nur für Austauschdateien (.baruch, .baruch.xml)");
    }
    // Runde 21: Weiche nach Dateiart (Austausch → PDF/EPUB → Markdown/ZIP → Typst).
    let file_name = match &command {
        Command::Import(file) => file.to_string_lossy().into_owned(),
        _ => String::new(),
    };
    // Runde 32: Word nach dem Austausch, vor PDF/EPUB.
    let docx_import = !exchange_import && baruch_server::docx_import::is_docx_name(&file_name);
    let source_import = !exchange_import && !docx_import && baruch_server::pdf_import::is_source_name(&file_name);
    let markdown_import = !exchange_import
        && !docx_import
        && !source_import
        && (baruch_server::markdown_import::is_markdown_name(&file_name)
            || baruch_server::markdown_import::is_archive_name(&file_name));
    if source_apply && !source_import && !docx_import {
        usage("--uebernehmen gilt nur für PDF/EPUB und Word (.docx)");
    }
    if source_library && !source_import {
        usage("--bibliothek gilt nur für PDF/EPUB");
    }
    if notes.is_none() && (notes_format.is_some() || notes_font.is_some()) {
        usage("--format und --schrift gelten nur mit --stichwort");
    }
    let notes_options = match &notes {
        Some(_) => match baruch_server::pdf::notes::NotesOptions::parse(notes_format.as_deref(), notes_font.as_deref())
        {
            Ok(options) => Some(options),
            Err(e) => usage(&e),
        },
        None => None,
    };
    match command {
        Command::Password(name) => reset_password(&name, &data).await,
        Command::Serve => {
            // Nur `serve` liest die erlaubten Rechnernamen; ein ungültiger Name beendet den Start (Exit 2).
            let allowed_hosts =
                baruch_server::guard::AllowedHosts::parse(&allow_hosts.join(",")).unwrap_or_else(|e| usage(&e));
            serve(&listen, data, secrets.as_deref(), allowed_hosts).await
        }
        Command::Import(file) if exchange_import => {
            let options = baruch_server::exchange::import::CliImport { merge, preview_only, title };
            import_exchange(&file, &options, data).await
        }
        Command::Import(file) if docx_import => {
            let options = baruch_server::docx_import::CliDocxImport { title, apply: source_apply };
            let state = open(&data).await;
            finish(baruch_server::docx_import::cli_import(&state, &file, &options).await);
        }
        Command::Import(file) if source_import => {
            let options =
                baruch_server::pdf_import::CliSourceImport { title, apply: source_apply, library: source_library };
            let state = open(&data).await;
            finish(baruch_server::pdf_import::cli_import(&state, &file, &options).await);
        }
        Command::Import(file) if markdown_import => {
            let state = open(&data).await;
            finish(baruch_server::markdown_import::cli_import(&state, &file, title.as_deref()).await);
        }
        Command::Import(file) => import(&file, title.as_deref(), data).await,
        Command::Export(work) if docx.is_some() => {
            let file = docx.expect("geprüft");
            let options = baruch_server::docx_export::DocxExportOptions { proposals: docx_proposals };
            let state = open(&data).await;
            let id = resolve(&state, &work).await;
            finish(baruch_server::docx_export::cli_export(&state, id, &file, &options).await);
        }
        Command::Export(work) if epub.is_some() => export_epub(&work, epub.as_deref().expect("geprüft"), data).await,
        Command::Export(work) => match (baruch, pdf, notes.zip(notes_options)) {
            (Some(file), _, _) => export_exchange(&work, &file, data).await,
            (None, Some(file), _) => export_pdf(&work, &file, data).await,
            (None, None, Some((file, options))) => export_notes(&work, &file, options, data).await,
            (None, None, None) => export(&work, out.as_deref(), data).await,
        },
    }
}

/// `passwort <name>`: das neue Passwort als eine Zeile von der Standardeingabe (ohne den Zeilenumbruch), nie von der
/// Befehlszeile. Ausgegeben wird nur der Name des Kontos.
async fn reset_password(name: &str, data: &std::path::Path) {
    use std::io::IsTerminal;
    if std::io::stdin().is_terminal() {
        eprintln!("Neues Passwort für „{name}“ (die Eingabe ist sichtbar), dann Eingabe:");
    }
    let mut line = String::new();
    if let Err(e) = std::io::stdin().read_line(&mut line) {
        eprintln!("Standardeingabe: {e}");
        std::process::exit(1);
    }
    let password = line.trim_end_matches(['\n', '\r']);
    match baruch_server::auth::cli::reset_password(data, name, password).await {
        Ok(()) => println!("Passwort für „{name}“ gesetzt; alle Sitzungen des Kontos sind beendet."),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}

async fn open(data: &std::path::Path) -> AppState {
    match AppState::open(data).await {
        Ok(state) => state,
        Err(e) => {
            eprintln!("Datenordner {}: {e}", data.display());
            std::process::exit(1);
        }
    }
}

async fn serve(listen: &str, data: PathBuf, secrets: Option<&str>, allowed_hosts: baruch_server::guard::AllowedHosts) {
    // Geheimnisse der Verbindungen (Runde 9); import/export brauchen keine.
    use baruch_server::connections::{MemorySecrets, SecretsMode, secrets_mode, system_secrets};
    let mode = secrets_mode(secrets, &data, &baruch_store::default_data_dir()).unwrap_or_else(|e| usage(&e));
    let store: std::sync::Arc<dyn baruch_server::connections::SecretStore> = match mode {
        SecretsMode::Keyring => system_secrets(),
        SecretsMode::Memory => std::sync::Arc::new(MemorySecrets::default()),
    };
    // Modell-Backend der Personas (Runde 10): Eliezer über die Verbindung `eliezer`.
    let model = std::sync::Arc::new(baruch_server::assist::eliezer::EliezerPort);
    let mut state = match AppState::open_with(&data, store, model).await {
        Ok(state) => state,
        Err(e) => {
            eprintln!("Datenordner {}: {e}", data.display());
            std::process::exit(1);
        }
    };
    let proxy_names = allowed_hosts.names().join(", ");
    state.allowed_hosts = allowed_hosts;
    // Befunde (Runde 28): einmal je Datenbank die vorhandenen Antworten der Personas zerlegen (ändert keinen Beitrag);
    // ein Fehler hält den Start nicht auf.
    match baruch_server::findings::backfill(&state).await {
        Ok(0) => {}
        Ok(n) => println!("Befunde nachgetragen: {n}"),
        Err(e) => eprintln!("Befunde nachtragen: {e}"),
    }
    // Bedeutungssuche (Runde 12): Eliezer als Einbetter, alle Werke und die Bibliothek im Hintergrund in
    // den LanceDB-Index nachziehen (blockiert den Start nicht).
    baruch_server::rag::catch_up(&state).await;
    // Typst-Satz (Runde 18a): Schriften im Hintergrund vorab suchen, damit der erste Satz nicht wartet.
    baruch_server::typeset::warm_fonts();
    let listener =
        tokio::net::TcpListener::bind(listen).await.unwrap_or_else(|e| panic!("{listen} lässt sich nicht öffnen: {e}"));
    println!("Daten: {}", data.display());
    println!("Git-Spiegel: {}", data.join("works").display());
    println!("Geheimnisse: {}", mode.describe());
    if !proxy_names.is_empty() {
        println!(
            "Erlaubte Rechnernamen (Reverse-Proxy): {proxy_names} – ohne Konten hat Baruch keine Anmeldung; dann \
             muss der Proxy den Zugangsschutz stellen. /mcp verlangt unter diesen Namen einen Token."
        );
    }
    // Runde 35: Mit Konten verlangen Seiten, Formulare und /rpc eine Anmeldung.
    match state.store.account_count().await {
        Ok(0) => println!("Anmeldung: aus (keine Konten; einrichten unter /einrichten am eigenen Rechner)"),
        Ok(n) => println!("Anmeldung: an ({n} Konten)"),
        Err(e) => eprintln!("Konten: {e}"),
    }
    // Runde 32: Bedienen und Herunterfahren (Signale, Seite /beenden) stehen in `shutdown`. Die Empfänger der Signale
    // sind eingerichtet, bevor die Zeile „Baruch läuft“ dasteht: Wer auf sie wartet und gleich ein SIGTERM schickt,
    // trifft schon das saubere Herunterfahren.
    let signal = baruch_server::shutdown::system_signal();
    println!("Baruch läuft: http://{}/", listener.local_addr().unwrap());
    if let Err(e) = baruch_server::shutdown::serve_with(listener, std::sync::Arc::new(state), signal).await {
        eprintln!("Server: {e}");
        std::process::exit(1);
    }
    // Die Datenbank ist geschlossen, die Schlusszeile steht da. Ohne diesen Ausgang wartete die Laufzeit beim
    // Verlassen von `main` noch auf blockierende Aufträge (etwa einen laufenden Typst-Satz) – über die Frist hinaus.
    std::process::exit(0);
}

async fn import(file: &std::path::Path, title: Option<&str>, data: PathBuf) {
    let state = open(&data).await;
    match baruch_server::import::import_file_as(&state, file, title).await {
        Ok(report) => {
            println!("Werk: {} (id {})", report.title, report.work);
            println!("Ordnername: {}", report.slug);
            println!("Abschnitte: {}", report.nodes);
            println!("Bilder: {}", report.assets.len());
            for skipped in &report.skipped {
                eprintln!("Bild übersprungen: {} ({})", skipped.path, skipped.reason);
            }
            for warning in &report.warnings {
                eprintln!("Hinweis ({}): {}", warning.code, warning.message);
            }
            println!("Git-Spiegel: {}", data.join("works").join(&report.slug).display());
            match (&report.commit, &report.mirror) {
                (Some(commit), _) => println!("Commit: {commit}"),
                (None, Some(error)) => eprintln!("Git-Spiegel nicht geschrieben (das Werk ist angelegt): {error}"),
                (None, None) => println!("Commit: keiner (Spiegel war schon auf dem Stand)"),
            }
        }
        Err(e) if e.code == rpc::REFUSED => {
            eprintln!("{}", e.message);
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Import von {}: {}", file.display(), e.message);
            std::process::exit(1);
        }
    }
}

/// Ergebnis der Austausch-Befehle: `Ok(text)` → Standardausgabe, Exit 0; `Err(text)` → Fehlerausgabe, Exit 1.
fn finish(result: Result<String, String>) {
    match result {
        Ok(text) => println!("{text}"),
        Err(text) => {
            eprintln!("{text}");
            std::process::exit(1);
        }
    }
}

/// `import DATEI.baruch|DATEI.baruch.xml [--abgleichen] [--vorschau] [--title T]` (Runde 26).
async fn import_exchange(file: &std::path::Path, options: &baruch_server::exchange::import::CliImport, data: PathBuf) {
    let state = open(&data).await;
    finish(baruch_server::exchange::import::cli_import(&state, file, options).await);
}

/// `export ID|SLUG --baruch DATEI` (Runde 26).
async fn export_exchange(work: &str, file: &std::path::Path, data: PathBuf) {
    let state = open(&data).await;
    let id = resolve(&state, work).await;
    finish(baruch_server::exchange::export::cli_export(&state, id, file).await);
}

/// `export ID|SLUG --epub DATEI` (Runde 20a): Die Datei darf es nicht geben (Exit 1 ohne Überschreiben).
async fn export_epub(work: &str, file: &std::path::Path, data: PathBuf) {
    if std::fs::symlink_metadata(file).is_ok() {
        eprintln!("Export: {} gibt es schon; nichts überschrieben", file.display());
        std::process::exit(1);
    }
    let state = open(&data).await;
    let id = resolve(&state, work).await;
    match baruch_server::epub::write_epub_file(&state, id, file).await {
        Ok((bytes, warnings)) => {
            println!("EPUB geschrieben: {} ({bytes} Bytes)", file.display());
            for warning in &warnings {
                eprintln!("Hinweis ({}): {}", warning.code, warning.message);
            }
        }
        Err(e) => {
            eprintln!("Export: {e}");
            std::process::exit(1);
        }
    }
}

/// Werk-id aus id oder slug; unbekannt → Exit 1.
async fn resolve(state: &AppState, work: &str) -> i64 {
    let id = match work.parse::<i64>() {
        Ok(id) => Ok(id),
        Err(_) => state.store.work_by_slug(work).await,
    };
    match id {
        Ok(id) => match state.store.work_slug(id).await {
            Ok(_) => id,
            Err(_) => {
                eprintln!("unbekanntes Werk: {work}");
                std::process::exit(1);
            }
        },
        Err(StoreError::UnknownWork(_)) => {
            eprintln!("unbekanntes Werk: {work}");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Export: {e}");
            std::process::exit(1);
        }
    }
}

/// `export <werk> --stichwort DATEI`: Stichwortzettel als PDF. Die Datei darf es nicht geben; scheitert der
/// Satz, bleibt sie ungeschrieben (Exit 1).
async fn export_notes(
    work: &str,
    file: &std::path::Path,
    options: baruch_server::pdf::notes::NotesOptions,
    data: PathBuf,
) {
    if std::fs::symlink_metadata(file).is_ok() {
        eprintln!("Export: {} gibt es schon; nichts überschrieben", file.display());
        std::process::exit(1);
    }
    let state = open(&data).await;
    let id = resolve(&state, work).await;
    match baruch_server::pdf::notes::write_notes_file(&state, id, file, options).await {
        Ok(pdf) => {
            println!(
                "Stichwortzettel geschrieben: {} ({} Seiten, {} Bytes)",
                file.display(),
                pdf.pages,
                pdf.bytes.len()
            )
        }
        Err(e) => {
            eprintln!("Export: {e}");
            std::process::exit(1);
        }
    }
}

/// `export <werk> --pdf DATEI`: Buch als PDF. Die Datei darf es nicht geben; scheitert der Satz, bleibt
/// sie ungeschrieben (Exit 1).
async fn export_pdf(work: &str, file: &std::path::Path, data: PathBuf) {
    if std::fs::symlink_metadata(file).is_ok() {
        eprintln!("Export: {} gibt es schon; nichts überschrieben", file.display());
        std::process::exit(1);
    }
    let state = open(&data).await;
    let id = resolve(&state, work).await;
    match baruch_server::pdf::write_pdf_file(&state, id, file).await {
        Ok(pdf) => {
            println!("PDF geschrieben: {} ({} Seiten, {} Bytes)", file.display(), pdf.pages, pdf.bytes.len());
            for warning in &pdf.print {
                eprintln!("Druckprüfung (Seite {}): {}", warning.page, warning.message);
            }
        }
        Err(e) => {
            eprintln!("Export: {e}");
            std::process::exit(1);
        }
    }
}

async fn export(work: &str, out: Option<&std::path::Path>, data: PathBuf) {
    let store = match Store::open(&data).await {
        Ok(store) => store,
        Err(e) => {
            eprintln!("Datenordner {}: {e}", data.display());
            std::process::exit(1);
        }
    };
    let id = match work.parse::<i64>() {
        Ok(id) => Ok(id),
        Err(_) => store.work_by_slug(work).await,
    };
    if let (Some(out), Ok(id)) = (out, &id) {
        match baruch_server::import::export_to_dir(&store, *id, out).await {
            Ok(files) => {
                println!("Geschrieben nach {}: {} Dateien", out.display(), files.len());
                for file in files {
                    println!("  {}", file.strip_prefix(out).unwrap_or(&file).display());
                }
                return;
            }
            Err(e) => {
                eprintln!("Export: {e}");
                std::process::exit(1);
            }
        }
    }
    let typst = match id {
        Ok(id) => store.export_typst(id).await,
        Err(e) => Err(e),
    };
    match typst {
        Ok(typst) => {
            use std::io::Write;
            let mut out = std::io::stdout().lock();
            if out.write_all(typst.as_bytes()).and_then(|()| out.flush()).is_err() {
                std::process::exit(1);
            }
        }
        Err(StoreError::UnknownWork(_)) => {
            eprintln!("unbekanntes Werk: {work}");
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Export: {e}");
            std::process::exit(1);
        }
    }
}
