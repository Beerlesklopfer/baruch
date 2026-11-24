// SPDX-License-Identifier: AGPL-3.0-or-later
//! Lokale SWORD-Bibelmodule lesen (Wunsch des Nutzers 2026-11-06: „Wir brauchen das SWORD-Modul“).
//!
//! - [`scan`]: alle Bibelmodule in `<Ordner>/mods.d/*.conf` der gegebenen Modulordner ([`default_dirs`]:
//!   `$SWORD_PATH`, `~/.sword`, `/usr/share/sword`), mit Beschreibung, Verszählung und Lizenzvermerk.
//! - [`open`]: ein Modul öffnen ([`Bible`]); `DataPath` gilt **relativ zum Modulordner**, nie zum
//!   aktuellen Verzeichnis, und darf ihn nicht verlassen (`..`).
//! - [`Bible::verse`], [`Bible::passage`], [`Bible::search`]: Klartext ([`text::to_plain`]).
//!
//! Das Dateiformat liest die Crate `sword-modules` 0.1.0 (CC0-1.0, reines Rust, ohne libsword; Treiber
//! zText/zText4/RawText/RawText4, Kompression ZIP, LZSS, BZIP2 und XZ). Baruch lässt jede Zählung zu, die
//! [`baruch_scripture::versification::versification`] kennt; andere meldet [`BibleError::Unsupported`]
//! („Verszählung „X“ kennt Baruch nicht“).
//!
//! Runde 17 (R17c): **verschlüsselte Module** (`CipherKey` in der conf). [`BibleInfo::enciphered`] sagt, ob die
//! conf einen `CipherKey` trägt (leer = gesperrt). [`open_with_key`] setzt einen vom Nutzer hinterlegten
//! Schlüssel nur im Speicher in die conf (wie `SwordLibrary::open_module_with_key`); ohne Schlüssel meldet
//! [`open`] [`BibleError::Locked`]. [`Bible::probe`] prüft einen Schlüssel am ersten vorhandenen Eintrag.
//! Strong-Nummern eines Verses liefert [`Bible::verse_strongs`].
//!
//! Seit Runde 15 (Punkt 5a) auch Kommentare (RawCom/RawCom4/zCom/zCom4) und Lexika (RawLD/RawLD4/zLD) als
//! Recherchewerkzeug ([`reference`]): [`scan`] meldet sie getrennt in [`Scan::commentaries`] und
//! [`Scan::lexicons`] (mit [`SourceKind`]); nicht Lesbares steht mit `supported: false` und Grund dabei.

pub mod fixture;
pub mod reference;
pub mod text;

pub use reference::{
    Commentary, CommentaryEntry, CommentaryHit, KeyList, Lexicon, LexiconArticle, LexiconHit, MAX_ENTRY_CHARS,
    open_commentary, open_commentary_with_key, open_lexicon, open_lexicon_with_key,
};

use std::path::{Component, Path, PathBuf};
use std::time::Instant;

use baruch_scripture::versification::{Versification, versification};
use serde::Serialize;
use sword_modules::conf::{CompressType, ModDrv, ModuleConf, ModuleKind, parse_conf};
use sword_modules::{Module, VerseKey, VerseModule};

/// Höchstzahl der Verse, die [`Bible::passage`] liest.
pub const MAX_PASSAGE_VERSES: usize = 400;

/// Art eines Moduls, wie Baruch es nutzt (R15): Bibel, Kommentar oder Lexikon (SWORD „Dictionary“).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    #[default]
    Bible,
    Commentary,
    Lexicon,
}

impl SourceKind {
    /// `bible`, `commentary`, `lexicon`.
    pub fn as_str(self) -> &'static str {
        match self {
            SourceKind::Bible => "bible",
            SourceKind::Commentary => "commentary",
            SourceKind::Lexicon => "lexicon",
        }
    }
}

/// Ein gefundenes Modul (Bibel, Kommentar oder Lexikon) mit den Angaben seiner conf.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BibleInfo {
    /// Modulname (Abschnitt der conf, z. B. `GerSch`).
    pub name: String,
    /// Art des Moduls (R15; Bibeln wie bisher `bible`).
    pub kind: SourceKind,
    pub description: String,
    /// `About` als Klartext.
    pub about: String,
    /// `Versification` (Vorgabe `KJV`).
    pub versification: String,
    pub lang: String,
    pub version: String,
    pub driver: String,
    pub compress: String,
    pub block_type: String,
    pub source_type: String,
    pub encoding: String,
    /// `DistributionLicense` der conf.
    pub license: String,
    /// `Copyright` bzw. `ShortCopyright` der conf.
    pub copyright: String,
    /// Lizenzvermerk für Antworten: Lizenz, Copyright, sonst „keine Angabe in der conf“.
    pub notice: String,
    /// Modulordner (enthält `mods.d/`).
    pub dir: PathBuf,
    pub conf_file: PathBuf,
    /// Lesbar? Sonst steht der Grund in `problem`. Ein verschlüsseltes Modul ohne Schlüssel gilt als lesbar
    /// (das Format kennt Baruch); ob ein Schlüssel fehlt, sagen [`BibleInfo::enciphered`] und
    /// [`BibleInfo::locked`].
    pub supported: bool,
    pub problem: Option<String>,
    /// R17c: Die conf trägt `CipherKey` (leer oder nicht): Das Modul ist verschlüsselt.
    pub enciphered: bool,
}

impl BibleInfo {
    /// Verschlüsselt und ohne Schlüssel in der conf (`CipherKey=` leer): Lesen braucht einen Schlüssel vom
    /// Nutzer. Liest die conf neu (kein eigenes Feld, damit [`BibleInfo`] nur die Felder des Vertrags trägt).
    pub fn locked(&self) -> bool {
        self.enciphered && matches!(conf_with_key(self, None).map(|c| c.is_locked()), Ok(true))
    }
}

/// Ergebnis von [`scan`]: Bibeln (erste Fundstelle eines Namens gewinnt), Kommentare, Lexika und Hinweise je
/// Ordner bzw. conf.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Scan {
    /// Nur Bibeln (wie vor Runde 15).
    pub modules: Vec<BibleInfo>,
    pub problems: Vec<String>,
    /// Kommentare (R15).
    pub commentaries: Vec<BibleInfo>,
    /// Lexika bzw. Wörterbücher (R15).
    pub lexicons: Vec<BibleInfo>,
}

fn find_in<'a>(list: &'a [BibleInfo], name: &str) -> Option<&'a BibleInfo> {
    list.iter().find(|m| m.name == name).or_else(|| list.iter().find(|m| m.name.eq_ignore_ascii_case(name)))
}

impl Scan {
    /// Bibelmodul nach Name (Groß-/Kleinschreibung egal).
    pub fn find(&self, name: &str) -> Option<&BibleInfo> {
        find_in(&self.modules, name)
    }

    /// Kommentar nach Name.
    pub fn find_commentary(&self, name: &str) -> Option<&BibleInfo> {
        find_in(&self.commentaries, name)
    }

    /// Lexikon nach Name.
    pub fn find_lexicon(&self, name: &str) -> Option<&BibleInfo> {
        find_in(&self.lexicons, name)
    }

    /// Modul einer Art nach Name.
    pub fn find_kind(&self, kind: SourceKind, name: &str) -> Option<&BibleInfo> {
        match kind {
            SourceKind::Bible => self.find(name),
            SourceKind::Commentary => self.find_commentary(name),
            SourceKind::Lexicon => self.find_lexicon(name),
        }
    }

    /// Alle Module: Bibeln, Kommentare, Lexika.
    pub fn all(&self) -> impl Iterator<Item = &BibleInfo> {
        self.modules.iter().chain(&self.commentaries).chain(&self.lexicons)
    }
}

/// Fehler beim Öffnen oder Lesen eines Moduls (Meldungen auf Deutsch).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BibleError {
    NotFound(String),
    Unsupported(String),
    Read(String),
    /// R17c: verschlüsseltes Modul ohne Schlüssel (Modulname).
    Locked(String),
}

impl std::fmt::Display for BibleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BibleError::NotFound(name) => write!(f, "Bibelmodul {name} nicht gefunden"),
            BibleError::Unsupported(why) => write!(f, "nicht unterstützt: {why}"),
            BibleError::Read(why) => write!(f, "Modul nicht lesbar: {why}"),
            BibleError::Locked(name) => write!(
                f,
                "Modul {name} ist verschlüsselt; der Schlüssel fehlt (vom Herausgeber des Moduls, unter \
                 Einstellungen → Quellen → Bibeln hinterlegen)"
            ),
        }
    }
}

impl std::error::Error for BibleError {}

/// Vorgabe der Modulordner: jeder Eintrag von `$SWORD_PATH`, dann `~/.sword`, dann `/usr/share/sword`.
pub fn default_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(paths) = std::env::var_os("SWORD_PATH") {
        dirs.extend(std::env::split_paths(&paths).filter(|p| !p.as_os_str().is_empty()));
    }
    if let Some(home) = std::env::var_os("HOME").filter(|h| !h.is_empty()) {
        dirs.push(PathBuf::from(home).join(".sword"));
    }
    dirs.push(PathBuf::from("/usr/share/sword"));
    dirs.dedup();
    dirs
}

pub(crate) fn decode_conf(bytes: &[u8]) -> String {
    let bytes = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes);
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_owned(),
        Err(_) => sword_modules::encoding::decode_text(bytes, sword_modules::encoding::TextEncoding::Latin1),
    }
}

/// `DataPath` relativ zum Modulordner ohne `..`; `Err` mit Grund.
pub(crate) fn checked_data_path(conf: &ModuleConf) -> Result<PathBuf, String> {
    let raw = conf.data_path().filter(|p| !p.trim().is_empty()).ok_or("conf ohne DataPath")?;
    let path = Path::new(raw);
    if path.components().any(|c| !matches!(c, Component::Normal(_) | Component::CurDir)) {
        return Err(format!("DataPath „{raw}“ verlässt den Modulordner"));
    }
    Ok(path.to_path_buf())
}

fn info_of(dir: &Path, conf_file: &Path, conf: &ModuleConf, kind: SourceKind) -> BibleInfo {
    let get = |k: &str| conf.get(k).map(str::trim).unwrap_or_default().to_owned();
    let license = get("DistributionLicense");
    let copyright = [get("Copyright"), get("ShortCopyright")].into_iter().find(|s| !s.is_empty()).unwrap_or_default();
    let notice = match (license.is_empty(), copyright.is_empty()) {
        (false, false) => format!("{license}; {copyright}"),
        (false, true) => license.clone(),
        (true, false) => copyright.clone(),
        (true, true) => "keine Lizenzangabe in der conf".to_owned(),
    };
    let compress_raw = get("CompressType");
    let mut info = BibleInfo {
        name: conf.name().to_owned(),
        kind,
        description: get("Description"),
        about: conf.about_text().unwrap_or_default().trim().to_owned(),
        versification: conf.versification().to_owned(),
        lang: get("Lang"),
        version: get("Version"),
        driver: get("ModDrv"),
        compress: compress_raw.clone(),
        block_type: get("BlockType"),
        source_type: conf.source_type().as_str().to_owned(),
        encoding: get("Encoding"),
        license,
        copyright,
        notice,
        dir: dir.to_path_buf(),
        conf_file: conf_file.to_path_buf(),
        supported: true,
        problem: None,
        enciphered: conf.cipher_key().is_some(),
    };
    let checked = match kind {
        SourceKind::Bible => support(conf).map(|_| ()),
        SourceKind::Commentary => reference::support_commentary(conf).map(|_| ()),
        SourceKind::Lexicon => reference::support_lexicon(conf),
    };
    if let Err(why) = checked {
        info.supported = false;
        info.problem = Some(why);
    }
    if kind == SourceKind::Lexicon {
        // Lexika haben keine Verszählung.
        info.versification = String::new();
    }
    info
}

/// Kann Baruch das Modul lesen? Treiber, Kompression, Verschlüsselung, Verszählung, DataPath.
fn support(conf: &ModuleConf) -> Result<&'static Versification, String> {
    let drv = conf.mod_drv().map_err(|e| e.to_string())?;
    let compressed = match drv {
        ModDrv::RawText | ModDrv::RawText4 => false,
        ModDrv::ZText | ModDrv::ZText4 => true,
        other => return Err(format!("Treiber {other:?} (nur zText und RawText)")),
    };
    common_support(conf, compressed)?;
    known_versification(conf)
}

/// Verszählung der conf, wenn Baruch sie kennt; sonst „Verszählung „X“ kennt Baruch nicht“.
pub(crate) fn known_versification(conf: &ModuleConf) -> Result<&'static Versification, String> {
    versification(conf.versification())
        .ok_or_else(|| format!("Verszählung „{}“ kennt Baruch nicht", conf.versification()))
}

/// Kompression und DataPath (für alle Arten gleich). Verschlüsselte Module (`CipherKey`) lehnt Baruch seit
/// Runde 17 nicht mehr ab: Mit Schlüssel sind sie lesbar ([`open_with_key`]).
pub(crate) fn common_support(conf: &ModuleConf, compressed: bool) -> Result<(), String> {
    let known =
        matches!(conf.compress_type(), CompressType::Zip | CompressType::Lzss | CompressType::Bzip2 | CompressType::Xz);
    if compressed && !known {
        return Err(format!(
            "Kompression {} (nur ZIP, LZSS, BZIP2 und XZ)",
            conf.get("CompressType").unwrap_or("LZSS")
        ));
    }
    checked_data_path(conf)?;
    Ok(())
}

/// conf des Moduls `info` neu lesen. Mit `cipher_key` (getrimmt, nicht leer) steht der Schlüssel danach **nur im
/// Speicher** in der conf (`CipherKey`, wie `SwordLibrary::open_module_with_key`); die Datei bleibt unberührt.
/// Ein unverschlüsseltes Modul bekommt keinen Schlüssel.
pub(crate) fn conf_with_key(info: &BibleInfo, cipher_key: Option<&str>) -> Result<ModuleConf, BibleError> {
    let bytes =
        std::fs::read(&info.conf_file).map_err(|e| BibleError::Read(format!("{}: {e}", info.conf_file.display())))?;
    let confs = parse_conf(&decode_conf(&bytes)).map_err(|e| BibleError::Read(e.to_string()))?;
    let mut conf =
        confs.into_iter().find(|c| c.name() == info.name).ok_or_else(|| BibleError::NotFound(info.name.clone()))?;
    if let Some(key) = cipher_key.map(str::trim).filter(|k| !k.is_empty())
        && conf.cipher_key().is_some()
    {
        conf.set("CipherKey", key);
    }
    Ok(conf)
}

/// Fehler von `sword-modules` beim Öffnen: gesperrt wird [`BibleError::Locked`], sonst [`BibleError::Read`].
pub(crate) fn open_error(name: &str, e: sword_modules::Error) -> BibleError {
    match e {
        sword_modules::Error::Locked(_) => BibleError::Locked(name.to_owned()),
        other => BibleError::Read(other.to_string()),
    }
}

/// Module in den Modulordnern `dirs` (je Ordner `mods.d/*.conf`, nach Dateinamen sortiert): Bibeln in
/// `modules`, Kommentare in `commentaries`, Lexika in `lexicons` (R15); allgemeine Bücher (GenBook) werden
/// übergangen. Ordner ohne `mods.d` und unlesbare confs stehen in `problems`; ein Name, der schon in einem
/// früheren Ordner stand, ebenfalls.
pub fn scan(dirs: &[PathBuf]) -> Scan {
    let mut out = Scan::default();
    for dir in dirs {
        let mods = dir.join("mods.d");
        let entries = match std::fs::read_dir(&mods) {
            Ok(e) => e,
            Err(_) => {
                out.problems.push(format!("{}: kein Ordner mods.d", dir.display()));
                continue;
            }
        };
        let mut files: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().is_some_and(|x| x.eq_ignore_ascii_case("conf")))
            .collect();
        files.sort();
        for file in files {
            let confs = std::fs::read(&file)
                .map_err(|e| e.to_string())
                .and_then(|b| parse_conf(&decode_conf(&b)).map_err(|e| e.to_string()));
            let confs = match confs {
                Ok(c) => c,
                Err(e) => {
                    out.problems.push(format!("{}: {e}", file.display()));
                    continue;
                }
            };
            for conf in confs.into_iter().filter(|c| c.get("ModDrv").is_some()) {
                let kind = match conf.module_kind().ok() {
                    Some(ModuleKind::Bible) => SourceKind::Bible,
                    Some(ModuleKind::Commentary) => SourceKind::Commentary,
                    Some(ModuleKind::Dictionary) => SourceKind::Lexicon,
                    _ => continue,
                };
                if out.all().any(|m| m.name.eq_ignore_ascii_case(conf.name())) {
                    out.problems.push(format!(
                        "{}: Modul {} steht schon in einem früheren Ordner",
                        file.display(),
                        conf.name()
                    ));
                    continue;
                }
                let info = info_of(dir, &file, &conf, kind);
                match kind {
                    SourceKind::Bible => out.modules.push(info),
                    SourceKind::Commentary => out.commentaries.push(info),
                    SourceKind::Lexicon => out.lexicons.push(info),
                }
            }
        }
    }
    out
}

/// Ein geöffnetes Bibelmodul.
pub struct Bible {
    pub info: BibleInfo,
    pub v11n: &'static Versification,
    module: VerseModule,
}

impl std::fmt::Debug for Bible {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bible").field("info", &self.info.name).field("v11n", &self.v11n.name).finish()
    }
}

/// Öffnet das Modul `info` (aus [`scan`]) ohne Schlüssel: [`open_with_key`]`(info, None)`.
pub fn open(info: &BibleInfo) -> Result<Bible, BibleError> {
    open_with_key(info, None)
}

/// Öffnet das Modul `info` mit dem Schlüssel `cipher_key` für ein verschlüsseltes Modul (nur im Speicher; die
/// conf-Datei bleibt unverändert). Ein unverschlüsseltes Modul übergeht den Schlüssel. Ohne Schlüssel bei
/// leerem `CipherKey` der conf: [`BibleError::Locked`] (nach den übrigen Gründen wie Zählung und Treiber). Ein
/// falscher Schlüssel fällt erst beim Lesen auf ([`Bible::probe`]).
pub fn open_with_key(info: &BibleInfo, cipher_key: Option<&str>) -> Result<Bible, BibleError> {
    let conf = conf_with_key(info, cipher_key)?;
    let v11n = support(&conf).map_err(BibleError::Unsupported)?;
    if conf.is_locked() {
        return Err(BibleError::Locked(info.name.clone()));
    }
    // `Module::open` liest DataPath relativ zum übergebenen Modulordner (führendes „./“ oder „/“ entfernt).
    let module = Module::open(&info.dir, conf).and_then(Module::into_verse).map_err(|e| open_error(&info.name, e))?;
    if !module.versification().name().eq_ignore_ascii_case(v11n.name) {
        return Err(BibleError::Unsupported(format!("Verszählung {}", module.versification().name())));
    }
    Ok(Bible { info: info.clone(), v11n, module })
}

/// Ein Vers mit Klartext.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Verse {
    pub chapter: u16,
    pub verse: u16,
    /// `None`: Stelle gibt es in der Zählung, das Modul hat dort keinen Text.
    pub text: Option<String>,
}

/// Treffer der Suche.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Hit {
    /// OSIS-Buch, Kapitel, Vers.
    pub book: String,
    pub chapter: u16,
    pub verse: u16,
    pub text: String,
}

/// Ergebnis von [`Bible::search`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Search {
    pub hits: Vec<Hit>,
    /// Abgebrochen wegen Treffer- oder Zeitgrenze.
    pub truncated: bool,
    pub searched_verses: usize,
}

impl Bible {
    fn key(book: &str, chapter: u16, verse: u16) -> Option<VerseKey> {
        VerseKey::from_osis(book, u8::try_from(chapter).ok()?, u8::try_from(verse).ok()?)
    }

    /// Klartext eines Verses (OSIS-Buch, Kapitel, Vers ab 1); `Ok(None)`, wenn das Modul dort nichts hat
    /// oder es die Stelle in seiner Zählung nicht gibt.
    pub fn verse(&self, book: &str, chapter: u16, verse: u16) -> Result<Option<String>, BibleError> {
        if self.v11n.verses_in(book, chapter).is_none_or(|n| verse == 0 || verse > n) {
            return Ok(None);
        }
        let Some(key) = Self::key(book, chapter, verse) else { return Ok(None) };
        let entry = self.module.verse(&key).map_err(|e| BibleError::Read(e.to_string()))?;
        Ok(entry.map(|e| text::to_plain(&e.raw, e.markup)).filter(|t| !t.is_empty()))
    }

    /// Rohtext an einer Index-Stelle (`testament` 0 = AT, 1 = NT), z. B. Modulkopf (0) und Testamentkopf (1).
    pub fn slot_text(&self, testament: usize, slot: u32) -> Result<Option<String>, BibleError> {
        let testament = if testament == 0 { sword_modules::Testament::Old } else { sword_modules::Testament::New };
        let entry =
            self.module.at(sword_modules::IndexPos { testament, slot }).map_err(|e| BibleError::Read(e.to_string()))?;
        Ok(entry.map(|e| e.raw))
    }

    /// Klartext einer Buch- (Kapitel 0) bzw. Kapiteleinleitung (Vers 0).
    pub fn intro(&self, book: &str, chapter: u16) -> Result<Option<String>, BibleError> {
        let Some(key) = Self::key(book, chapter, 0) else { return Ok(None) };
        let entry = self.module.verse(&key).map_err(|e| BibleError::Read(e.to_string()))?;
        Ok(entry.map(|e| e.raw))
    }

    /// Verse von `start` bis `end` (je Kapitel, Vers; Vers 0 = ganzes Kapitel) in der Zählung des Moduls,
    /// höchstens [`MAX_PASSAGE_VERSES`]. `Err(Unsupported)` bei unbekanntem Buch oder Kapitel.
    pub fn passage(&self, book: &str, start: (u16, u16), end: (u16, u16)) -> Result<Vec<Verse>, BibleError> {
        let chapters = self.v11n.chapters_of(book).ok_or_else(|| BibleError::Unsupported(format!("Buch {book}")))?;
        let last_of = |c: u16| chapters.get(usize::from(c).wrapping_sub(1)).copied();
        let mut out = Vec::new();
        for chapter in start.0..=end.0.max(start.0) {
            let Some(max) = last_of(chapter) else {
                return Err(BibleError::Unsupported(format!("{book} {chapter}: Kapitel fehlt in {}", self.v11n.name)));
            };
            let from = if chapter == start.0 && start.1 > 0 { start.1 } else { 1 };
            let to = if chapter == end.0 && end.1 > 0 { end.1.min(max) } else { max };
            for verse in from..=to {
                if out.len() == MAX_PASSAGE_VERSES {
                    return Ok(out);
                }
                out.push(Verse { chapter, verse, text: self.verse(book, chapter, verse)? });
            }
        }
        Ok(out)
    }

    /// Lineare Suche nach einer Wortfolge (normalisiert wie der Wortlautvergleich: Satzzeichen, Groß/Klein,
    /// Bindestriche zählen nicht) über alle Verse; höchstens `limit` Treffer, bis `deadline`.
    pub fn search(&self, query: &str, limit: usize, deadline: Instant) -> Result<Search, BibleError> {
        let needle: Vec<String> = baruch_scripture::wording::words(query).into_iter().map(|w| w.key).collect();
        let mut hits = Vec::new();
        let mut searched = 0;
        if needle.is_empty() {
            return Ok(Search { hits, truncated: false, searched_verses: 0 });
        }
        for item in self.module.iter() {
            let (key, entry) = item.map_err(|e| BibleError::Read(e.to_string()))?;
            if key.chapter == 0 || key.verse == 0 {
                continue;
            }
            searched += 1;
            if searched % 256 == 0 && Instant::now() > deadline {
                return Ok(Search { hits, truncated: true, searched_verses: searched });
            }
            let plain = text::to_plain(&entry.raw, entry.markup);
            let words: Vec<String> = baruch_scripture::wording::words(&plain).into_iter().map(|w| w.key).collect();
            if words.windows(needle.len()).any(|w| w == needle.as_slice()) {
                let Some(book) = key.osis_book() else { continue };
                hits.push(Hit {
                    book: book.to_owned(),
                    chapter: key.chapter.into(),
                    verse: key.verse.into(),
                    text: plain,
                });
                if hits.len() >= limit {
                    return Ok(Search { hits, truncated: true, searched_verses: searched });
                }
            }
        }
        Ok(Search { hits, truncated: false, searched_verses: searched })
    }

    /// Runde 19c: Klartext der ersten Verse des Moduls (in seiner Reihenfolge, ohne Einleitungen), höchstens
    /// `max_verses` bzw. bis `deadline` – die Stichprobe für die Schätzung der Wortfragmente
    /// (`baruch_scripture::wording::fragment_survey`). `truncated`: wegen einer Grenze abgebrochen.
    pub fn sample_texts(&self, max_verses: usize, deadline: Instant) -> Result<(Vec<String>, bool), BibleError> {
        let mut out = Vec::new();
        for item in self.module.iter() {
            let (key, entry) = item.map_err(|e| BibleError::Read(e.to_string()))?;
            if key.chapter == 0 || key.verse == 0 {
                continue;
            }
            if out.len() >= max_verses || (out.len() % 256 == 255 && Instant::now() > deadline) {
                return Ok((out, true));
            }
            let plain = text::to_plain(&entry.raw, entry.markup);
            if !plain.is_empty() {
                out.push(plain);
            }
        }
        Ok((out, false))
    }

    /// R17c: Strong-Nummern eines Verses („H1234“, „G26“; ohne führende Nullen, in der Reihenfolge des Textes,
    /// ohne Doppel) aus der Auszeichnung: OSIS `lemma="strong:…"`, ThML `<sync type="Strongs" value="…"/>`, GBF
    /// `<WH…>`/`<WG…>`. Leer ohne Auszeichnung oder ohne Text an der Stelle.
    pub fn verse_strongs(&self, book: &str, chapter: u16, verse: u16) -> Result<Vec<String>, BibleError> {
        if self.v11n.verses_in(book, chapter).is_none_or(|n| verse == 0 || verse > n) {
            return Ok(Vec::new());
        }
        let Some(key) = Self::key(book, chapter, verse) else { return Ok(Vec::new()) };
        let entry = self.module.verse(&key).map_err(|e| BibleError::Read(e.to_string()))?;
        let old = self
            .v11n
            .books()
            .position(|(id, _)| id == book)
            .is_some_and(|i| i < baruch_scripture::versification::OLD_TESTAMENT_BOOKS);
        Ok(entry.map(|e| text::strongs(&e.raw, e.markup, if old { 'H' } else { 'G' })).unwrap_or_default())
    }

    /// R17c: Trägt das Modul Strong-Nummern? conf: `GlobalOptionFilter` mit „Strongs“ (z. B. `OSISStrongs`) oder
    /// `Feature=StrongsNumbers`.
    pub fn has_strongs(&self) -> bool {
        let conf = self.module.conf();
        conf.global_option_filters().iter().any(|f| f.contains("Strongs"))
            || conf.features().iter().any(|f| f.trim().eq_ignore_ascii_case("StrongsNumbers"))
    }

    /// R17c: Probelesen ([`probe_entries`]): die ersten Einträge müssen sich entschlüsseln, entpacken und als Text
    /// lesen lassen. So fällt ein falscher Schlüssel auf, bevor er gespeichert wird. `Err` mit Grund; ein Modul ganz
    /// ohne Einträge gilt als nicht prüfbar (`Err`).
    pub fn probe(&self) -> Result<(), BibleError> {
        probe_entries(self.module.iter().map(|r| r.map(|(_, e)| e)))
    }
}

/// Höchstens so viele Einträge liest [`probe_entries`].
const PROBE_ENTRIES: usize = 64;
/// Nach so vielen Zeichen Text hört [`probe_entries`] auf.
const PROBE_CHARS: usize = 2048;

/// Sieht ein Eintrag aus wie Text? Kein Ersatzzeichen (U+FFFD: ungültiges UTF-8) und kein Steuerzeichen außer
/// Tabulator und Zeilenwechsel. Ein falsch entschlüsselter Eintrag besteht aus Zufallsbytes; in Latin-1 (Vorgabe
/// von SWORD ohne `Encoding`, gelesen als cp1252) gibt es kein Ersatzzeichen, aber etwa jedes achte Zufallsbyte ist
/// ein Steuerzeichen (0x00–0x1F, 0x7F und die in cp1252 freien 0x81, 0x8D, 0x8F, 0x90, 0x9D).
fn plausible_text(raw: &str) -> bool {
    !raw.chars().any(|c| c == '\u{FFFD}' || (c.is_control() && !matches!(c, '\t' | '\n' | '\r')))
}

/// R17c, Nachbesserung Runde 17: Probelesen für einen Schlüssel. Liest die ersten nicht leeren Einträge (höchstens
/// [`PROBE_ENTRIES`] bzw. [`PROBE_CHARS`] Zeichen) und verlangt, dass jeder wie Text aussieht ([`plausible_text`]).
/// Vorher galt nur der erste Eintrag und nur U+FFFD/NUL: Ein falsch entschlüsselter Latin-1-Eintrag aus 150
/// Zufallsbytes enthält mit rund 55 % kein NUL, ein falscher Schlüssel wäre als „gültig“ gespeichert worden. Jetzt
/// liegt die Wahrscheinlichkeit dafür bei 150 Zeichen unter 10⁻⁸.
pub(crate) fn probe_entries(
    entries: impl Iterator<Item = Result<sword_modules::Entry, sword_modules::Error>>,
) -> Result<(), BibleError> {
    let (mut seen, mut chars) = (0usize, 0usize);
    for entry in entries.take(PROBE_ENTRIES) {
        let entry = entry.map_err(|e| BibleError::Read(e.to_string()))?;
        if entry.raw.is_empty() {
            continue;
        }
        if !plausible_text(&entry.raw) {
            return Err(BibleError::Read("Eintrag ist kein gültiger Text".to_owned()));
        }
        seen += 1;
        chars += entry.raw.chars().count();
        if chars >= PROBE_CHARS {
            break;
        }
    }
    if seen == 0 {
        return Err(BibleError::Read("Modul ohne Einträge – Schlüssel nicht prüfbar".to_owned()));
    }
    Ok(())
}
