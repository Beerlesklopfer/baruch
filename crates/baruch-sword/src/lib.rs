// SPDX-License-Identifier: AGPL-3.0-or-later
//! Read local SWORD Bible modules (wish of the user 2026-11-06: „Wir brauchen das SWORD-Modul“).
//!
//! - [`scan`]: all Bible modules in `<Ordner>/mods.d/*.conf` of the given module folders ([`default_dirs`]:
//!   `$SWORD_PATH`, `~/.sword`, `/usr/share/sword`), with description, versification and licence note.
//! - [`open`]: open a module ([`Bible`]); `DataPath` applies **relative to the module folder**, never to the
//!   current directory, and must not leave it (`..`).
//! - [`Bible::verse`], [`Bible::passage`], [`Bible::search`]: plain text ([`text::to_plain`]).
//!
//! The file format is read by the crate `sword-modules` 0.1.0 (CC0-1.0, pure Rust, without libsword; drivers
//! zText/zText4/RawText/RawText4, compression ZIP, LZSS, BZIP2 and XZ). Baruch admits every versification that
//! [`baruch_scripture::versification::versification`] knows; others are reported by [`BibleError::Unsupported`]
//! („Verszählung „X“ kennt Baruch nicht“).
//!
//! Round 17 (R17c): **enciphered modules** (`CipherKey` in the conf). [`BibleInfo::enciphered`] says whether the
//! conf carries a `CipherKey` (empty = locked). [`open_with_key`] puts a key stored by the user into the conf
//! only in memory (like `SwordLibrary::open_module_with_key`); without a key [`open`] reports
//! [`BibleError::Locked`]. [`Bible::probe`] checks a key on the first existing entry.
//! Strong's numbers of a verse are delivered by [`Bible::verse_strongs`].
//!
//! Since Round 15 (item 5a) also commentaries (RawCom/RawCom4/zCom/zCom4) and lexicons (RawLD/RawLD4/zLD) as a
//! research tool ([`reference`]): [`scan`] reports them separately in [`Scan::commentaries`] and
//! [`Scan::lexicons`] (with [`SourceKind`]); what cannot be read is listed with `supported: false` and a reason.

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

/// Maximum number of verses that [`Bible::passage`] reads.
pub const MAX_PASSAGE_VERSES: usize = 400;

/// Kind of a module as Baruch uses it (R15): Bible, commentary or lexicon (SWORD "Dictionary").
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

/// A module that was found (Bible, commentary or lexicon) with the details of its conf.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BibleInfo {
    /// Module name (section of the conf, e.g. `GerSch`).
    pub name: String,
    /// Kind of the module (R15; Bibles as before `bible`).
    pub kind: SourceKind,
    pub description: String,
    /// `About` as plain text.
    pub about: String,
    /// `Versification` (default `KJV`).
    pub versification: String,
    pub lang: String,
    pub version: String,
    pub driver: String,
    pub compress: String,
    pub block_type: String,
    pub source_type: String,
    pub encoding: String,
    /// `DistributionLicense` of the conf.
    pub license: String,
    /// `Copyright` or `ShortCopyright` of the conf.
    pub copyright: String,
    /// Licence note for answers: licence, copyright, otherwise „keine Angabe in der conf“.
    pub notice: String,
    /// Module folder (contains `mods.d/`).
    pub dir: PathBuf,
    pub conf_file: PathBuf,
    /// Readable? Otherwise the reason is in `problem`. An enciphered module without a key counts as readable
    /// (Baruch knows the format); whether a key is missing is said by [`BibleInfo::enciphered`] and
    /// [`BibleInfo::locked`].
    pub supported: bool,
    pub problem: Option<String>,
    /// R17c: the conf carries `CipherKey` (empty or not): the module is enciphered.
    pub enciphered: bool,
}

impl BibleInfo {
    /// Enciphered and without a key in the conf (`CipherKey=` empty): reading needs a key from the
    /// user. Reads the conf anew (no field of its own, so that [`BibleInfo`] carries only the fields of the contract).
    pub fn locked(&self) -> bool {
        self.enciphered && matches!(conf_with_key(self, None).map(|c| c.is_locked()), Ok(true))
    }
}

/// Result of [`scan`]: Bibles (the first place a name is found wins), commentaries, lexicons and notes per
/// folder or conf.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Scan {
    /// Only Bibles (as before Round 15).
    pub modules: Vec<BibleInfo>,
    pub problems: Vec<String>,
    /// Commentaries (R15).
    pub commentaries: Vec<BibleInfo>,
    /// Lexicons or dictionaries (R15).
    pub lexicons: Vec<BibleInfo>,
}

fn find_in<'a>(list: &'a [BibleInfo], name: &str) -> Option<&'a BibleInfo> {
    list.iter().find(|m| m.name == name).or_else(|| list.iter().find(|m| m.name.eq_ignore_ascii_case(name)))
}

impl Scan {
    /// Bible module by name (case does not matter).
    pub fn find(&self, name: &str) -> Option<&BibleInfo> {
        find_in(&self.modules, name)
    }

    /// Commentary by name.
    pub fn find_commentary(&self, name: &str) -> Option<&BibleInfo> {
        find_in(&self.commentaries, name)
    }

    /// Lexicon by name.
    pub fn find_lexicon(&self, name: &str) -> Option<&BibleInfo> {
        find_in(&self.lexicons, name)
    }

    /// Module of a kind by name.
    pub fn find_kind(&self, kind: SourceKind, name: &str) -> Option<&BibleInfo> {
        match kind {
            SourceKind::Bible => self.find(name),
            SourceKind::Commentary => self.find_commentary(name),
            SourceKind::Lexicon => self.find_lexicon(name),
        }
    }

    /// All modules: Bibles, commentaries, lexicons.
    pub fn all(&self) -> impl Iterator<Item = &BibleInfo> {
        self.modules.iter().chain(&self.commentaries).chain(&self.lexicons)
    }
}

/// Errors when opening or reading a module (messages in German).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BibleError {
    NotFound(String),
    Unsupported(String),
    Read(String),
    /// R17c: enciphered module without a key (module name).
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

/// Default of the module folders: every entry of `$SWORD_PATH`, then `~/.sword`, then `/usr/share/sword`.
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

/// `DataPath` relative to the module folder without `..`; `Err` with reason.
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
        // Lexicons have no versification.
        info.versification = String::new();
    }
    info
}

/// Can Baruch read the module? Driver, compression, encipherment, versification, DataPath.
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

/// Versification of the conf, if Baruch knows it; otherwise „Verszählung „X“ kennt Baruch nicht“.
pub(crate) fn known_versification(conf: &ModuleConf) -> Result<&'static Versification, String> {
    versification(conf.versification())
        .ok_or_else(|| format!("Verszählung „{}“ kennt Baruch nicht", conf.versification()))
}

/// Compression and DataPath (the same for all kinds). Baruch no longer rejects enciphered modules (`CipherKey`)
/// since Round 17: with a key they are readable ([`open_with_key`]).
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

/// Read the conf of the module `info` anew. With `cipher_key` (trimmed, not empty) the key is afterwards **only
/// in memory** in the conf (`CipherKey`, like `SwordLibrary::open_module_with_key`); the file stays untouched.
/// An unenciphered module gets no key.
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

/// Error of `sword-modules` on opening: locked becomes [`BibleError::Locked`], otherwise [`BibleError::Read`].
pub(crate) fn open_error(name: &str, e: sword_modules::Error) -> BibleError {
    match e {
        sword_modules::Error::Locked(_) => BibleError::Locked(name.to_owned()),
        other => BibleError::Read(other.to_string()),
    }
}

/// Modules in the module folders `dirs` (per folder `mods.d/*.conf`, sorted by file name): Bibles in
/// `modules`, commentaries in `commentaries`, lexicons in `lexicons` (R15); general books (GenBook) are
/// skipped. Folders without `mods.d` and unreadable confs are in `problems`; a name that already was in an
/// earlier folder likewise.
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

/// An opened Bible module.
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

/// Opens the module `info` (from [`scan`]) without a key: [`open_with_key`]`(info, None)`.
pub fn open(info: &BibleInfo) -> Result<Bible, BibleError> {
    open_with_key(info, None)
}

/// Opens the module `info` with the key `cipher_key` for an enciphered module (only in memory; the
/// conf file stays unchanged). An unenciphered module ignores the key. Without a key with an
/// empty `CipherKey` of the conf: [`BibleError::Locked`] (after the other reasons such as versification and
/// driver). A wrong key only shows up on reading ([`Bible::probe`]).
pub fn open_with_key(info: &BibleInfo, cipher_key: Option<&str>) -> Result<Bible, BibleError> {
    let conf = conf_with_key(info, cipher_key)?;
    let v11n = support(&conf).map_err(BibleError::Unsupported)?;
    if conf.is_locked() {
        return Err(BibleError::Locked(info.name.clone()));
    }
    // `Module::open` reads DataPath relative to the passed module folder (leading "./" or "/" removed).
    let module = Module::open(&info.dir, conf).and_then(Module::into_verse).map_err(|e| open_error(&info.name, e))?;
    if !module.versification().name().eq_ignore_ascii_case(v11n.name) {
        return Err(BibleError::Unsupported(format!("Verszählung {}", module.versification().name())));
    }
    Ok(Bible { info: info.clone(), v11n, module })
}

/// A verse with plain text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Verse {
    pub chapter: u16,
    pub verse: u16,
    /// `None`: the reference exists in the versification, the module has no text there.
    pub text: Option<String>,
}

/// Hit of the search.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Hit {
    /// OSIS book, chapter, verse.
    pub book: String,
    pub chapter: u16,
    pub verse: u16,
    pub text: String,
}

/// Result of [`Bible::search`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Search {
    pub hits: Vec<Hit>,
    /// Aborted because of the hit or time limit.
    pub truncated: bool,
    pub searched_verses: usize,
}

impl Bible {
    fn key(book: &str, chapter: u16, verse: u16) -> Option<VerseKey> {
        VerseKey::from_osis(book, u8::try_from(chapter).ok()?, u8::try_from(verse).ok()?)
    }

    /// Plain text of a verse (OSIS book, chapter, verse from 1); `Ok(None)` if the module has nothing there
    /// or the reference does not exist in its versification.
    pub fn verse(&self, book: &str, chapter: u16, verse: u16) -> Result<Option<String>, BibleError> {
        if self.v11n.verses_in(book, chapter).is_none_or(|n| verse == 0 || verse > n) {
            return Ok(None);
        }
        let Some(key) = Self::key(book, chapter, verse) else { return Ok(None) };
        let entry = self.module.verse(&key).map_err(|e| BibleError::Read(e.to_string()))?;
        Ok(entry.map(|e| text::to_plain(&e.raw, e.markup)).filter(|t| !t.is_empty()))
    }

    /// Raw text at an index slot (`testament` 0 = OT, 1 = NT), e.g. module header (0) and testament header (1).
    pub fn slot_text(&self, testament: usize, slot: u32) -> Result<Option<String>, BibleError> {
        let testament = if testament == 0 { sword_modules::Testament::Old } else { sword_modules::Testament::New };
        let entry =
            self.module.at(sword_modules::IndexPos { testament, slot }).map_err(|e| BibleError::Read(e.to_string()))?;
        Ok(entry.map(|e| e.raw))
    }

    /// Plain text of a book introduction (chapter 0) or chapter introduction (verse 0).
    pub fn intro(&self, book: &str, chapter: u16) -> Result<Option<String>, BibleError> {
        let Some(key) = Self::key(book, chapter, 0) else { return Ok(None) };
        let entry = self.module.verse(&key).map_err(|e| BibleError::Read(e.to_string()))?;
        Ok(entry.map(|e| e.raw))
    }

    /// Verses from `start` to `end` (chapter, verse each; verse 0 = whole chapter) in the versification of the
    /// module, at most [`MAX_PASSAGE_VERSES`]. `Err(Unsupported)` for an unknown book or chapter.
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

    /// Linear search for a word sequence (normalised like the wording comparison: punctuation, case,
    /// hyphens do not count) over all verses; at most `limit` hits, until `deadline`.
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

    /// Round 19c: plain text of the first verses of the module (in its order, without introductions), at most
    /// `max_verses` or until `deadline` – the sample for the estimate of the word fragments
    /// (`baruch_scripture::wording::fragment_survey`). `truncated`: aborted because of a limit.
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

    /// R17c: Strong's numbers of a verse ("H1234", "G26"; without leading zeros, in the order of the text,
    /// without duplicates) from the markup: OSIS `lemma="strong:…"`, ThML `<sync type="Strongs" value="…"/>`, GBF
    /// `<WH…>`/`<WG…>`. Empty without markup or without text at the reference.
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

    /// R17c: does the module carry Strong's numbers? conf: `GlobalOptionFilter` with "Strongs" (e.g.
    /// `OSISStrongs`) or `Feature=StrongsNumbers`.
    pub fn has_strongs(&self) -> bool {
        let conf = self.module.conf();
        conf.global_option_filters().iter().any(|f| f.contains("Strongs"))
            || conf.features().iter().any(|f| f.trim().eq_ignore_ascii_case("StrongsNumbers"))
    }

    /// R17c: trial read ([`probe_entries`]): the first entries must decipher, unpack and read as text.
    /// That way a wrong key shows up before it is stored. `Err` with reason; a module entirely
    /// without entries counts as not checkable (`Err`).
    pub fn probe(&self) -> Result<(), BibleError> {
        probe_entries(self.module.iter().map(|r| r.map(|(_, e)| e)))
    }
}

/// [`probe_entries`] reads at most this many entries.
const PROBE_ENTRIES: usize = 64;
/// After this many characters of text [`probe_entries`] stops.
const PROBE_CHARS: usize = 2048;

/// Does an entry look like text? No replacement character (U+FFFD: invalid UTF-8) and no control character
/// except tab and line change. A wrongly deciphered entry consists of random bytes; in Latin-1 (default of
/// SWORD without `Encoding`, read as cp1252) there is no replacement character, but about every eighth random
/// byte is a control character (0x00–0x1F, 0x7F and the 0x81, 0x8D, 0x8F, 0x90, 0x9D that are free in cp1252).
fn plausible_text(raw: &str) -> bool {
    !raw.chars().any(|c| c == '\u{FFFD}' || (c.is_control() && !matches!(c, '\t' | '\n' | '\r')))
}

/// R17c, follow-up fix Round 17: trial read for a key. Reads the first non-empty entries (at most
/// [`PROBE_ENTRIES`] or [`PROBE_CHARS`] characters) and demands that each looks like text ([`plausible_text`]).
/// Before, only the first entry counted and only U+FFFD/NUL: a wrongly deciphered Latin-1 entry of 150 random
/// bytes contains no NUL with about 55 %, a wrong key would have been stored as "valid". Now the probability of
/// that is below 10⁻⁸ at 150 characters.
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
