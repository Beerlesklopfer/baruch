// SPDX-License-Identifier: AGPL-3.0-or-later
//! SWORD Bibles (request 2026-11-06): module folders, Bible per work and wording check. RPC `bible.*`
//! (`docs/RPC.md`, „Bibeln“), MCP `bible_modules`/`bible_lookup`/`bible_search` ([`crate::mcp::bibles`]),
//! form on the page Einstellungen → Quellen ([`crate::connections`]).
//!
//! - Module folders: list in the store (migration 0030), else the defaults of [`baruch_sword::default_dirs`].
//! - Bible per work: module name in the store; read through [`baruch_sword`] (format reader `sword-modules`, CC0).
//! - [`check_wording`]: per reference the text of the Bible and comparison with a quotation next to the reference
//!   ([`baruch_scripture::wording`]). Without a Bible the wording stays „ungeprüft“.
//!
//! Answers always name module, versification and the licence notice of the conf. Read-only except
//! `bible.set_dirs`/`bible.set_work`/`bible.set_work_sources` (human only: not through MCP, not through `/act`).
//!
//! Round 15 (item 5a/5b): **commentaries and lexicons** as a research tool. `bible.modules` reports per module
//! `kind` (`bible`, `commentary`, `lexicon`); `commentary.lookup/search`, `lexicon.lookup/keys/search`,
//! `bible.work_sources`/`bible.set_work_sources` (selection per work, several, migration 0037); MCP
//! `commentary_lookup`, `lexicon_lookup`, `lexicon_search` (read-only). For the literature finished, escaped
//! HTML fragments: [`commentary_panel_html`] (`<details>`, without JavaScript) and [`commentary_hits_html`]
//! (hit groups „Kommentar“ in the search field, time limit 3 s).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

use baruch_scripture::versification::german;
use baruch_scripture::versification::mapping::{self, VerseRef};
use baruch_scripture::wording::{self, DiffPart, QuoteSpan};
use baruch_scripture::{Found, RefStatus};
use baruch_store::WorkId;
use baruch_sword::{Bible, BibleError, BibleInfo, Commentary, Lexicon, Scan, SourceKind};
use serde::Serialize;
use serde_json::{Value, json};

use crate::AppState;
use crate::i18n::tc;
use crate::rpc::{INTERNAL, INVALID_PARAMS, METHOD_NOT_FOUND, REFUSED, RpcError, work_param};

/// Maximum number of module folders.
pub const MAX_DIRS: usize = 20;
/// Maximum length of a folder (characters).
pub const MAX_DIR_CHARS: usize = 1000;
/// Default and maximum of the hits of `bible.search`.
pub const SEARCH_LIMIT: usize = 50;
pub const SEARCH_MAX: usize = 200;
/// Time limit of the linear search.
pub const SEARCH_TIME: Duration = Duration::from_secs(5);
/// Maximum length of the Bible text of a reference in answers (characters); longer references are shortened.
pub const MAX_TEXT_CHARS: usize = 4000;

const METHODS: &[&str] = &[
    "bible.modules",
    "bible.set_dirs",
    "bible.set_work",
    "bible.lookup",
    "bible.check",
    "bible.search",
    "bible.work_sources",
    "bible.set_work_sources",
    "commentary.lookup",
    "commentary.search",
    "lexicon.lookup",
    "lexicon.keys",
    "lexicon.search",
    "bible.marks",
    "bible.cipher_key_set",
    "bible.cipher_key_delete",
];

/// Methods that change something (human only: not through `/act`, not through MCP).
pub const WRITES: &[&str] =
    &["bible.set_dirs", "bible.set_work", "bible.set_work_sources", "bible.cipher_key_set", "bible.cipher_key_delete"];

/// Maximum length of a module key (characters).
pub const MAX_CIPHER_KEY_CHARS: usize = 255;

/// Answer to a key that does not decrypt the module.
pub const CIPHER_KEY_WRONG: &str = "Schlüssel passt nicht zum Modul; nichts gespeichert.";

/// Does this module serve the RPC method?
pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

// ---- Folders, modules, choice ----------------------------------------------------------------------

/// Module folders and whether they are the defaults.
pub async fn dirs(state: &AppState) -> Result<(Vec<PathBuf>, bool), RpcError> {
    Ok(match state.store.bible_dirs().await? {
        Some(dirs) => (dirs.into_iter().map(PathBuf::from).collect(), false),
        None => (baruch_sword::default_dirs(), true),
    })
}

/// All Bible modules of the module folders (reads only `mods.d`).
pub async fn scan(state: &AppState) -> Result<(Scan, Vec<PathBuf>, bool), RpcError> {
    let (dirs, default) = dirs(state).await?;
    let list = dirs.clone();
    let found = tokio::task::spawn_blocking(crate::i18n::carry(move || baruch_sword::scan(&list)))
        .await
        .map_err(|e| RpcError::new(INTERNAL, e.to_string()))?;
    Ok((found, dirs, default))
}

/// Account in the keyring (service “baruch”) for the key of an encrypted module (R17c).
pub fn cipher_account(module: &str) -> String {
    format!("sword-cipher:{}", module.to_lowercase())
}

/// Stored key of an encrypted module (only through `AppState::secrets`); `None` for an
/// unencrypted module (then Baruch does not ask the keyring at all), without an entry or if the
/// keyring does not answer. The key leaves this function only towards the SWORD reader.
pub async fn stored_cipher_key(state: &AppState, info: &BibleInfo) -> Option<String> {
    if !info.enciphered {
        return None;
    }
    crate::connections::secrets::get(&state.secrets, &cipher_account(&info.name))
        .await
        .ok()
        .flatten()
        .filter(|k| !k.trim().is_empty())
}

/// `none` (not encrypted), `locked` (encrypted, key missing), `unlocked` (key in the
/// keyring or already in the conf).
pub fn cipher_state(info: &BibleInfo, key_stored: bool) -> &'static str {
    if !info.enciphered {
        "none"
    } else if key_stored || !info.locked() {
        "unlocked"
    } else {
        "locked"
    }
}

/// Short details of a module for answers: module, description, versification, licence notice; since R17c also
/// `cipher` (without looking into the keyring: `locked` if the conf carries no key) and `notice`.
pub fn info_json(info: &BibleInfo) -> Value {
    info_json_with(info, cipher_state(info, false))
}

/// Like [`info_json`] with the encryption state `cipher` (`none`, `locked`, `unlocked`).
pub fn info_json_with(info: &BibleInfo, cipher: &str) -> Value {
    json!({
        "module": info.name,
        "kind": info.kind,
        "description": info.description,
        "versification": info.versification,
        "license": info.notice,
        "distribution_license": info.license,
        "copyright": info.copyright,
        "lang": info.lang,
        "version": info.version,
        "driver": info.driver,
        "dir": info.dir.display().to_string(),
        "supported": info.supported,
        "problem": info.problem,
        "cipher": cipher,
        "notice": info.notice,
    })
}

/// Opened modules, per conf file, name, modification time (a changed conf is opened anew) and checksum
/// of the key (0 without a key; never the key itself).
type CacheKey = (PathBuf, String, Option<SystemTime>, u64);

/// Checksum of a key for the cache (the key never stands in it in plain text).
fn key_hash(key: Option<&str>) -> u64 {
    use std::hash::{Hash, Hasher};
    let Some(key) = key.map(str::trim).filter(|k| !k.is_empty()) else { return 0 };
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    key.hash(&mut hasher);
    hasher.finish().max(1)
}

fn cache_key(info: &BibleInfo, key: Option<&str>) -> CacheKey {
    let modified = std::fs::metadata(&info.conf_file).and_then(|m| m.modified()).ok();
    (info.conf_file.clone(), info.name.clone(), modified, key_hash(key))
}

fn cache() -> &'static Mutex<HashMap<CacheKey, Arc<Bible>>> {
    static CACHE: OnceLock<Mutex<HashMap<CacheKey, Arc<Bible>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Opens a module (with cache, at most 8 open modules) without a key.
pub fn open_cached(info: &BibleInfo) -> Result<Arc<Bible>, BibleError> {
    open_cached_with_key(info, None)
}

/// Opens a module with the key of an encrypted module (R17c; cache like
/// [`open_cached`], the key counts only as a checksum).
pub fn open_cached_with_key(info: &BibleInfo, key: Option<&str>) -> Result<Arc<Bible>, BibleError> {
    let cache_key = cache_key(info, key);
    if let Some(bible) = cache().lock().expect("Zwischenspeicher der Bibeln").get(&cache_key) {
        return Ok(Arc::clone(bible));
    }
    let bible = Arc::new(baruch_sword::open_with_key(info, key)?);
    let mut map = cache().lock().expect("Zwischenspeicher der Bibeln");
    if map.len() >= 8 {
        map.clear();
    }
    map.insert(cache_key, Arc::clone(&bible));
    Ok(bible)
}

/// The Bible of a work.
pub enum Choice {
    /// None chosen.
    None,
    /// Chosen, but not found in any module folder.
    Missing(String),
    /// Found, but not readable or not supported.
    Unusable(Box<BibleInfo>, String),
    Ready(Arc<Bible>),
}

impl Choice {
    pub fn bible(&self) -> Option<&Bible> {
        match self {
            Choice::Ready(b) => Some(b),
            _ => None,
        }
    }

    /// Details for answers (`null` without a choice).
    pub fn json(&self) -> Value {
        match self {
            Choice::None => Value::Null,
            Choice::Missing(name) => json!({ "module": name, "supported": false,
                "problem": format!("Modul {name} in keinem Modulordner gefunden") }),
            Choice::Unusable(info, why) => {
                let mut v = info_json(info);
                v["supported"] = json!(false);
                v["problem"] = json!(why);
                v
            }
            Choice::Ready(b) => info_json_with(&b.info, if b.info.enciphered { "unlocked" } else { "none" }),
        }
    }
}

/// Bible by module name (from the module folders).
pub async fn by_name(state: &AppState, name: &str) -> Result<Choice, RpcError> {
    let (found, ..) = scan(state).await?;
    let Some(info) = found.find(name).cloned() else { return Ok(Choice::Missing(name.to_owned())) };
    if let Some(why) = info.problem.clone() {
        return Ok(Choice::Unusable(Box::new(info), why));
    }
    // R17c: Baruch fetches the key of an encrypted module from the keyring itself.
    let key = stored_cipher_key(state, &info).await;
    let opened = {
        let info = info.clone();
        tokio::task::spawn_blocking(crate::i18n::carry(move || open_cached_with_key(&info, key.as_deref())))
            .await
            .map_err(|e| RpcError::new(INTERNAL, e.to_string()))?
    };
    Ok(match opened {
        Ok(bible) => Choice::Ready(bible),
        Err(e) => Choice::Unusable(Box::new(info), e.to_string()),
    })
}

/// The Bible of the work `work`.
pub async fn of_work(state: &AppState, work: WorkId) -> Result<Choice, RpcError> {
    match state.store.work_bible(work).await? {
        None => Ok(Choice::None),
        Some(name) => by_name(state, &name).await,
    }
}

// ---- Wording ---------------------------------------------------------------------------------------

/// Result of the wording check of a reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WordingResult {
    /// `unchecked`, `invalid_ref`, `versification_mismatch`, `missing_verse`, `module_error`, `no_quote`,
    /// `match`, `differs`.
    pub code: &'static str,
    /// German, e.g. „Wortlaut stimmt“, „weicht ab“, „kein Zitat gefunden“.
    pub label: String,
    /// Text of the Bible for the reference (only with a Bible and a readable reference).
    pub bible_text: Option<String>,
    /// The quotation in the text next to the reference.
    pub quote: Option<String>,
    pub partial: bool,
    pub diff: Vec<DiffPart>,
    /// R17c: byte range of the quotation content (without quotation marks) in the checked text if one is assigned.
    pub quote_span: Option<(usize, usize)>,
    /// R17c: reference read in the module's versification („Joel 2,28–32 (KJV)“), only if it differs from the German.
    pub module_ref: Option<String>,
    /// Round 19c: the wording matches only without whitespace (remnants like „die ses“); `code` stays `match`.
    pub module_spacing: bool,
}

impl WordingResult {
    fn plain(code: &'static str, label: impl Into<String>) -> Self {
        WordingResult {
            code,
            label: label.into(),
            bible_text: None,
            quote: None,
            partial: false,
            diff: Vec::new(),
            quote_span: None,
            module_ref: None,
            module_spacing: false,
        }
    }

    pub fn unchecked() -> Self {
        Self::plain("unchecked", crate::scripture::UNCHECKED)
    }
}

/// R17c: text of a reference, read in the versification of the module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PassageText {
    /// Verses in the versification of the module (chapter, verse as in `module_refs`); `text: None` = verse missing.
    pub verses: Vec<baruch_sword::Verse>,
    /// The verses read in the versification of the module (same order as `verses`).
    pub module_refs: Vec<VerseRef>,
    /// The reference has more than [`baruch_sword::MAX_PASSAGE_VERSES`] verses in German versification.
    pub truncated: bool,
    /// Applied rules of the mapping (German short text, e.g. „Psalmüberschrift als Vers 1“).
    pub rules: Vec<&'static str>,
}

/// Start of the reason if a reference cannot be mapped into the versification of the module.
pub const MISMATCH_PREFIX: &str = "Versifikation passt nicht";

/// Read a reference in German versification from `bible`: mapping german() → bible.v11n (map_range, limit
/// baruch_sword::MAX_PASSAGE_VERSES), then bible.verse(…) per verse. Err(reason, German).
pub fn read_passage(bible: &Bible, book: &str, start: (u16, u16), end: (u16, u16)) -> Result<PassageText, String> {
    let mapped = mapping::map_range(german(), bible.v11n, book, start, end, baruch_sword::MAX_PASSAGE_VERSES)?;
    let mut verses = Vec::with_capacity(mapped.verses.len());
    for at in &mapped.verses {
        let text = bible.verse(at.book, at.chapter, at.verse).map_err(|e| e.to_string())?;
        verses.push(baruch_sword::Verse { chapter: at.chapter, verse: at.verse, text });
    }
    Ok(PassageText { verses, module_refs: mapped.verses, truncated: mapped.truncated, rules: mapped.rules })
}

/// Entries of a commentary on a reference in **German** versification (Round 23a, code review quellen-2).
pub struct CommentaryRead {
    /// Entries, chapter and verse in the versification of the commentary.
    pub entries: Vec<baruch_sword::CommentaryEntry>,
    /// Book (OSIS) in the versification of the commentary.
    pub book: &'static str,
    /// Name of the commentary's versification if it differs from the German one (for the label „… (KJV)“).
    pub v11n: Option<&'static str>,
}

impl CommentaryRead {
    /// Label of an entry: reference in the versification of the commentary, with its name if the versification differs
    /// („Joel 2,28–32 (KJV)“) so that nobody takes it for the German reference.
    pub fn label(&self, entry: &baruch_sword::CommentaryEntry) -> String {
        let range = range_label(self.book, entry.start, entry.end);
        match self.v11n {
            Some(name) => format!("{range} ({name})"),
            None => range,
        }
    }
}

/// Read a commentary on a reference in German versification: as for Bibles ([`read_passage`]) first map german() →
/// versification of the commentary, then look up the mapped range. Before, the German numbers were used
/// unchanged as numbers of the commentary; for a commentary in KJV versification „Joel 3,1“ thus returned the
/// commentary on KJV Joel 3,1 (German 4,1), labelled with the German reference. `Err` = reason (German).
/// Blocking.
pub fn read_commentary(
    commentary: &baruch_sword::Commentary,
    book: &'static str,
    start: (u16, u16),
    end: (u16, u16),
) -> Result<CommentaryRead, String> {
    if commentary.v11n.name == german().name {
        let entries = commentary.entries(book, start, end).map_err(|e| e.to_string())?;
        return Ok(CommentaryRead { entries, book, v11n: None });
    }
    let mapped = mapping::map_range(german(), commentary.v11n, book, start, end, baruch_sword::MAX_PASSAGE_VERSES)?;
    let (Some(first), Some(last)) = (mapped.verses.first(), mapped.verses.last()) else {
        return Ok(CommentaryRead { entries: Vec::new(), book, v11n: Some(commentary.v11n.name) });
    };
    let entries = commentary
        .entries(first.book, (first.chapter, first.verse), (last.chapter, last.verse))
        .map_err(|e| e.to_string())?;
    Ok(CommentaryRead { entries, book: first.book, v11n: Some(commentary.v11n.name) })
}

/// „Mal 4,1–6 (KJV)“: the reference read in the versification of the module, only if it differs from the German
/// one (other versification and other verses or an applied rule); otherwise `None`.
pub fn module_ref(bible: &Bible, book: &str, start: (u16, u16), end: (u16, u16), read: &PassageText) -> Option<String> {
    if bible.v11n.name == german().name {
        return None;
    }
    let (first, last) = (read.module_refs.first()?, read.module_refs.last()?);
    let from = (start.0, start.1.max(1));
    let to = if end.1 == 0 { (end.0, german().verses_in(book, end.0).unwrap_or(0)) } else { end };
    let same = read.rules.is_empty()
        && first.book == book
        && (first.chapter, first.verse) == from
        && (last.chapter, last.verse) == to;
    if same {
        return None;
    }
    Some(format!(
        "{} ({})",
        range_label(first.book, (first.chapter, first.verse), (last.chapter, last.verse)),
        bible.v11n.name
    ))
}

/// Wording per reference in `refs` (from `text`) against `bible`. Without a Bible: all „ungeprüft“. Quotations come
/// only from [`crate::quotes::quote_spans`] (R17).
pub fn check_wording(bible: Option<&Bible>, text: &str, refs: &[Found]) -> Vec<WordingResult> {
    let Some(bible) = bible else { return refs.iter().map(|_| WordingResult::unchecked()).collect() };
    let quotes = crate::quotes::quote_spans(text);
    refs.iter().enumerate().map(|(i, found)| one(bible, text, &quotes, refs, i, found)).collect()
}

fn one(bible: &Bible, text: &str, quotes: &[QuoteSpan], refs: &[Found], index: usize, found: &Found) -> WordingResult {
    let (Some(passage), RefStatus::Valid) = (found.passage.as_ref(), &found.status) else {
        return WordingResult::plain("invalid_ref", "Stelle ungültig – Wortlaut nicht geprüft");
    };
    let book = passage.book;
    // Other versification in the module: read through the mapping (R17); what cannot be mapped stays
    // „Versifikation passt nicht“ with reason.
    let read = match read_passage(bible, book.id, passage.start, passage.end) {
        Ok(read) => read,
        Err(why) if why.starts_with(MISMATCH_PREFIX) => {
            return WordingResult::plain("versification_mismatch", format!("{why} (Modul {})", bible.info.name));
        }
        Err(why) => return WordingResult::plain("module_error", why),
    };
    let module_ref = module_ref(bible, book.id, passage.start, passage.end, &read);
    // Round 23a (code review quellen-6): `read.verses` carries chapter and verse in the module's versification. So
    // the message names the missing verse in German versification (mapped back); if it cannot be mapped, the
    // module reference stands there with the name of its versification – never a module number under German flag.
    let same_counting = bible.v11n.name == german().name;
    let missing: Vec<String> = read
        .verses
        .iter()
        .zip(&read.module_refs)
        .filter(|(v, _)| v.text.is_none())
        .map(|(v, at)| {
            if same_counting {
                return format!("{} {},{}", book.abbr, v.chapter, v.verse);
            }
            match german_of(bible, *at).0 {
                Some(german) => german,
                None => format!("{} {},{} ({})", book.abbr, v.chapter, v.verse, bible.v11n.name),
            }
        })
        .collect();
    if !missing.is_empty() {
        let shown = if missing.len() > 5 {
            format!("{} … ({} Verse)", missing[..5].join(", "), missing.len())
        } else {
            missing.join(", ")
        };
        return WordingResult {
            module_ref,
            ..WordingResult::plain("missing_verse", format!("Vers fehlt im Modul {}: {shown}", bible.info.name))
        };
    }
    let mut bible_text: String = read.verses.iter().filter_map(|v| v.text.as_deref()).collect::<Vec<_>>().join(" ");
    if bible_text.chars().count() > MAX_TEXT_CHARS {
        bible_text = bible_text.chars().take(MAX_TEXT_CHARS).collect::<String>() + " …";
    }
    let Some(q) = wording::quote_for(text, quotes, refs, index) else {
        return WordingResult {
            bible_text: Some(bible_text),
            module_ref,
            ..WordingResult::plain("no_quote", "kein Zitat gefunden")
        };
    };
    let quote = text[q.start..q.end].to_owned();
    let cmp = wording::compare(&quote, &bible_text);
    let (code, label) = match (cmp.matches, cmp.partial, cmp.spacing) {
        (true, false, false) => ("match", "Wortlaut stimmt".to_owned()),
        (true, true, false) => ("match", "Wortlaut stimmt (Teilzitat)".to_owned()),
        (true, false, true) => ("match", SPACING_LABEL.to_owned()),
        (true, true, true) => ("match", "Wortlaut stimmt (Teilzitat, Leerraum im Modul)".to_owned()),
        _ => ("differs", "Wortlaut weicht ab".to_owned()),
    };
    WordingResult {
        code,
        label,
        bible_text: Some(bible_text),
        quote: Some(quote),
        partial: cmp.partial,
        diff: cmp.diff,
        quote_span: Some((q.start, q.end)),
        module_ref,
        module_spacing: cmp.spacing,
    }
}

// ---- Word fragments in the module (Round 19c) ---------------------------------------------------------

/// Label of a reference whose quotation matches only without whitespace (hyphenation remnants in the module).
pub const SPACING_LABEL: &str = "Wortlaut stimmt (Leerraum im Modul)";
/// At most this many verses the sample reads for the estimate of the word fragments.
pub const FRAGMENT_SAMPLE_VERSES: usize = 4000;
/// Time limit of the sample.
pub const FRAGMENT_SAMPLE_TIME: Duration = Duration::from_secs(2);

type NoteKey = (PathBuf, String, Option<SystemTime>);

/// What the sample found out about a module; the text of the note is made from it in the language of the caller
/// (Round 39c), so the cache does not depend on a language.
#[derive(Clone)]
struct FragmentFacts {
    module: String,
    suspects: usize,
    pairs: usize,
    examples: Vec<String>,
}

fn note_cache() -> &'static Mutex<HashMap<NoteKey, Option<FragmentFacts>>> {
    static CACHE: OnceLock<Mutex<HashMap<NoteKey, Option<FragmentFacts>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Note on a module with strikingly many word fragments (probably hyphenation remnants from a PDF), else `None`.
/// Estimated through [`baruch_scripture::wording::fragment_survey`] on a sample of the first
/// [`FRAGMENT_SAMPLE_VERSES`] verses (at most [`FRAGMENT_SAMPLE_TIME`]); the result stays in memory per conf file
/// and modification time. Blocking (reads the module); a read error yields no note.
pub fn module_note(bible: &Bible) -> Option<String> {
    module_facts(bible).map(|facts| fragment_text(&facts))
}

/// Like [`module_note`], in the work language (book catalog `book.scripture.fragment_*`): for the facts block and
/// the closing note of a persona answer, which follow the language of the work, not that of the interface.
pub fn module_note_in(bible: &Bible, lang: baruch_core::WorkLanguage) -> Option<String> {
    module_facts(bible).map(|facts| fragment_text_in(&facts, lang))
}

fn module_facts(bible: &Bible) -> Option<FragmentFacts> {
    let modified = std::fs::metadata(&bible.info.conf_file).and_then(|m| m.modified()).ok();
    let key = (bible.info.conf_file.clone(), bible.info.name.clone(), modified);
    if let Some(note) = note_cache().lock().expect("Zwischenspeicher der Modulhinweise").get(&key) {
        return note.clone();
    }
    // Round 23a (review R19): one run per module. Whoever asks for the same module at the same time waits for the
    // first run and then takes its result from the cache instead of reading the sample (up to 2 s) itself.
    static RUNNING: OnceLock<Mutex<HashMap<NoteKey, std::sync::Arc<Mutex<()>>>>> = OnceLock::new();
    let turn = {
        let mut running = RUNNING.get_or_init(Default::default).lock().expect("Läufe der Modulhinweise");
        if running.len() >= 64 {
            running.clear();
        }
        running.entry(key.clone()).or_default().clone()
    };
    let _held = turn.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(note) = note_cache().lock().expect("Zwischenspeicher der Modulhinweise").get(&key) {
        return note.clone();
    }
    MODULE_NOTE_RUNS.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let note = match bible.sample_texts(FRAGMENT_SAMPLE_VERSES, Instant::now() + FRAGMENT_SAMPLE_TIME) {
        Ok((sample, _)) => fragment_facts(&bible.info.name, &sample),
        Err(_) => None,
    };
    let mut map = note_cache().lock().expect("Zwischenspeicher der Modulhinweise");
    if map.len() >= 32 {
        map.clear();
    }
    map.insert(key, note.clone());
    note
}

/// Number of sample runs of [`module_note`] in this process (for tests of “one run per module”).
pub fn module_note_runs() -> usize {
    MODULE_NOTE_RUNS.load(std::sync::atomic::Ordering::SeqCst)
}

static MODULE_NOTE_RUNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Note text from a sample (public for tests): `None` if the module is inconspicuous. The text is in the current
/// interface language (`i18n::current()`; German keeps the old wording).
pub fn fragment_note(module: &str, sample: &[String]) -> Option<String> {
    fragment_facts(module, sample).map(|facts| fragment_text(&facts))
}

fn fragment_facts(module: &str, sample: &[String]) -> Option<FragmentFacts> {
    let survey = wording::fragment_survey(sample.iter().map(String::as_str));
    if !survey.fragmented() {
        return None;
    }
    Some(FragmentFacts {
        module: module.to_owned(),
        suspects: survey.suspects,
        pairs: survey.pairs,
        examples: survey.examples,
    })
}

fn fragment_text_in(facts: &FragmentFacts, lang: baruch_core::WorkLanguage) -> String {
    use baruch_core::book_text::book_t;
    let examples = facts
        .examples
        .iter()
        .map(|e| book_t(lang, "book.scripture.fragment_example", &[("example", e.as_str())]))
        .collect::<Vec<_>>()
        .join(", ");
    book_t(
        lang,
        "book.scripture.fragment_note",
        &[
            ("module", facts.module.as_str()),
            ("suspects", &facts.suspects.to_string()),
            ("pairs", &facts.pairs.to_string()),
            ("examples", &examples),
        ],
    )
}

fn fragment_text(facts: &FragmentFacts) -> String {
    let examples = facts
        .examples
        .iter()
        .map(|e| tc("reference_msg.bibles.fragment_example", &[("example", e.as_str().into())]))
        .collect::<Vec<_>>()
        .join(", ");
    tc(
        "reference_msg.bibles.fragment_note",
        &[
            ("module", facts.module.as_str().into()),
            ("suspects", facts.suspects.into()),
            ("pairs", facts.pairs.into()),
            ("examples", examples.into()),
        ],
    )
}

/// Note on the Bible of a choice (only for a readable Bible).
pub fn choice_note(choice: &Choice) -> Option<String> {
    choice.bible().and_then(module_note)
}

// ---- Commentaries and lexicons (R15, item 5a) -------------------------------------------------------

/// Time limit of the commentary and lexicon search through RPC/MCP.
pub const REFERENCE_SEARCH_TIME: Duration = Duration::from_secs(5);
/// Time limit of all hit groups „Kommentar“ in the search field of the literature together.
pub const HITS_TIME: Duration = Duration::from_secs(3);
/// Maximum number of hits per group in the search field.
pub const HITS_PER_GROUP: usize = 10;

type ReferenceCache<T> = Mutex<HashMap<CacheKey, Arc<T>>>;

fn commentary_cache() -> &'static ReferenceCache<Commentary> {
    static CACHE: OnceLock<ReferenceCache<Commentary>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lexicon_cache() -> &'static ReferenceCache<Lexicon> {
    static CACHE: OnceLock<ReferenceCache<Lexicon>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cached<T>(
    cache: &'static ReferenceCache<T>,
    info: &BibleInfo,
    cipher_key: Option<&str>,
    open: impl FnOnce(&BibleInfo, Option<&str>) -> Result<T, BibleError>,
) -> Result<Arc<T>, BibleError> {
    let key = cache_key(info, cipher_key);
    if let Some(found) = cache.lock().expect("Zwischenspeicher der Module").get(&key) {
        return Ok(Arc::clone(found));
    }
    let opened = Arc::new(open(info, cipher_key)?);
    let mut map = cache.lock().expect("Zwischenspeicher der Module");
    if map.len() >= 8 {
        map.clear();
    }
    map.insert(key, Arc::clone(&opened));
    Ok(opened)
}

/// A commentary or lexicon by name: ready or with an honest reason why not.
pub enum Source<T> {
    Ready(Arc<T>),
    Problem { module: String, info: Option<Box<BibleInfo>>, problem: String },
}

impl<T> Source<T> {
    fn problem(module: &str, info: Option<&BibleInfo>, problem: impl Into<String>) -> Self {
        Source::Problem { module: module.to_owned(), info: info.map(|i| Box::new(i.clone())), problem: problem.into() }
    }
}

async fn open_source<T: Send + Sync + 'static>(
    state: &AppState,
    kind: SourceKind,
    name: &str,
    cache: &'static ReferenceCache<T>,
    open: fn(&BibleInfo, Option<&str>) -> Result<T, BibleError>,
) -> Result<Source<T>, RpcError> {
    let (found, ..) = scan(state).await?;
    let Some(info) = found.find_kind(kind, name).cloned() else {
        return Ok(Source::problem(name, None, source_missing(kind, name)));
    };
    if let Some(why) = info.problem.clone() {
        return Ok(Source::problem(&info.name, Some(&info), why));
    }
    let key = stored_cipher_key(state, &info).await;
    let opened = {
        let info = info.clone();
        tokio::task::spawn_blocking(crate::i18n::carry(move || cached(cache, &info, key.as_deref(), open)))
            .await
            .map_err(|e| RpcError::new(INTERNAL, e.to_string()))?
    };
    Ok(match opened {
        Ok(source) => Source::Ready(source),
        Err(e) => Source::problem(&info.name, Some(&info), e.to_string()),
    })
}

/// „Kommentar/Lexikon <name> in keinem Modulordner gefunden“ in the language of the request.
fn source_missing(kind: SourceKind, name: &str) -> String {
    if kind == SourceKind::Commentary {
        tc("sources_msg.sword.commentary_missing", &[("name", name.into())])
    } else {
        tc("sources_msg.sword.lexicon_missing", &[("name", name.into())])
    }
}

/// Commentary by module name.
pub async fn commentary_by_name(state: &AppState, name: &str) -> Result<Source<Commentary>, RpcError> {
    open_source(state, SourceKind::Commentary, name, commentary_cache(), baruch_sword::open_commentary_with_key).await
}

/// Lexicon by module name.
pub async fn lexicon_by_name(state: &AppState, name: &str) -> Result<Source<Lexicon>, RpcError> {
    open_source(state, SourceKind::Lexicon, name, lexicon_cache(), baruch_sword::open_lexicon_with_key).await
}

/// Chosen commentaries and lexicons of the work.
pub async fn work_sources(state: &AppState, work: WorkId) -> Result<baruch_store::WorkSources, RpcError> {
    Ok(state.store.work_sword_sources(work).await?)
}

/// Modules for a request: `module` explicitly, otherwise the selection of the work.
async fn chosen(
    state: &AppState,
    work: WorkId,
    module: Option<&str>,
    kind: SourceKind,
) -> Result<Vec<String>, RpcError> {
    if let Some(name) = module.map(str::trim).filter(|m| !m.is_empty()) {
        return Ok(vec![name.to_owned()]);
    }
    let sources = work_sources(state, work).await?;
    Ok(if kind == SourceKind::Commentary { sources.commentaries } else { sources.lexicons })
}

/// German reference of a range (`Ps 139,23–24`, `Ps 139,23–140,2`).
pub fn range_label(book: &str, start: (u16, u16), end: (u16, u16)) -> String {
    let abbr = baruch_scripture::find_book(book).map_or_else(|| book.to_owned(), |b| b.abbr.to_owned());
    if start == end {
        format!("{abbr} {},{}", start.0, start.1)
    } else if start.0 == end.0 {
        format!("{abbr} {},{}–{}", start.0, start.1, end.1)
    } else {
        format!("{abbr} {},{}–{},{}", start.0, start.1, end.0, end.1)
    }
}

/// Shorten text to `max` characters (with „…“); `true` if shortened.
pub fn clipped(text: &str, max: usize) -> (String, bool) {
    baruch_sword::reference::clip(text, max)
}

fn source_json(info: &BibleInfo) -> Value {
    json!({ "module": info.name, "description": info.description, "versification": info.versification,
            "notice": info.notice })
}

/// `commentary.lookup {ref, module?, work?}` → `{ref, commentaries: [{module, description, versification,
/// notice, entries: [{ref, text, truncated}], problem?}]}`. Every item in `ref` is read with the reference check
/// (German versification), **mapped** into the versification of the commentary ([`read_commentary`], Round 23a) and
/// looked up there; `entries[].ref` names the commentary's reference, with the versification's name if it differs.
pub async fn commentary_lookup(
    state: &AppState,
    work: WorkId,
    reference: &str,
    module: Option<&str>,
) -> Result<Value, RpcError> {
    let found = baruch_scripture::parse(reference);
    let mut out = Vec::new();
    for name in chosen(state, work, module, SourceKind::Commentary).await? {
        match commentary_by_name(state, &name).await? {
            Source::Problem { module, info, problem } => {
                let mut v = info.as_deref().map(source_json).unwrap_or_else(|| json!({ "module": module }));
                v["entries"] = json!([]);
                v["problem"] = json!(problem);
                out.push(v);
            }
            Source::Ready(commentary) => {
                let mut entries = Vec::new();
                let mut problems: Vec<String> = Vec::new();
                for f in found.iter().take(20) {
                    let (Some(p), RefStatus::Valid) = (f.passage.as_ref(), &f.status) else {
                        problems.push(tc(
                            "sources_msg.sword.ref_problem",
                            &[("input", f.input.as_str().into()), ("problem", f.message().into())],
                        ));
                        continue;
                    };
                    let source = Arc::clone(&commentary);
                    let (book, start, end) = (p.book.id, p.start, p.end);
                    // Round 23a (code review quellen-2): map the German reference into the commentary's numbering.
                    let read = tokio::task::spawn_blocking(crate::i18n::carry(move || {
                        read_commentary(&source, book, start, end)
                    }))
                    .await
                    .map_err(|e| RpcError::new(INTERNAL, e.to_string()))?;
                    match read {
                        Ok(read) => {
                            for e in &read.entries {
                                let (text, cut) = clipped(&e.text, MAX_TEXT_CHARS);
                                entries.push(json!({ "ref": read.label(e), "text": text,
                                                     "truncated": cut || e.truncated }));
                            }
                        }
                        Err(e) => problems.push(e),
                    }
                }
                let mut v = source_json(&commentary.info);
                v["entries"] = json!(entries);
                if found.is_empty() {
                    problems.push(tc("sources_msg.sword.unreadable_ref", &[("reference", reference.into())]));
                }
                if entries.is_empty() && problems.is_empty() {
                    problems.push(tc("sources_msg.sword.no_entry", &[("name", commentary.info.name.as_str().into())]));
                }
                if !problems.is_empty() {
                    v["problem"] = json!(problems.join("; "));
                }
                out.push(v);
            }
        }
    }
    Ok(json!({ "ref": reference, "work": work, "commentaries": out }))
}

/// `commentary.search {query, module?, work?, limit?}` → `{query, commentaries: [{module, …, hits: [{ref,
/// text}], truncated, problem?}]}`; time limit [`REFERENCE_SEARCH_TIME`] in total.
pub async fn commentary_search(
    state: &AppState,
    work: WorkId,
    query: &str,
    module: Option<&str>,
    limit: usize,
    time: Duration,
) -> Result<Value, RpcError> {
    if query.trim().is_empty() || query.chars().count() > 200 {
        return Err(RpcError::new(INVALID_PARAMS, tc("sources_msg.sword.query_length", &[])));
    }
    let deadline = Instant::now() + time;
    let mut out = Vec::new();
    for name in chosen(state, work, module, SourceKind::Commentary).await? {
        match commentary_by_name(state, &name).await? {
            Source::Problem { module, info, problem } => {
                let mut v = info.as_deref().map(source_json).unwrap_or_else(|| json!({ "module": module }));
                v["hits"] = json!([]);
                v["problem"] = json!(problem);
                out.push(v);
            }
            Source::Ready(commentary) => {
                let source = Arc::clone(&commentary);
                let q = query.to_owned();
                let found = tokio::task::spawn_blocking(crate::i18n::carry(move || source.search(&q, limit, deadline)))
                    .await
                    .map_err(|e| RpcError::new(INTERNAL, e.to_string()))?;
                let mut v = source_json(&commentary.info);
                match found {
                    Ok((hits, truncated)) => {
                        let hits: Vec<Value> = hits
                            .iter()
                            .map(|h| {
                                let (text, cut) = clipped(&h.text, 600);
                                json!({ "ref": range_label(&h.book, (h.chapter, h.verse), (h.chapter, h.verse)),
                                        "text": text, "truncated": cut || h.truncated })
                            })
                            .collect();
                        v["hits"] = json!(hits);
                        v["truncated"] = json!(truncated);
                    }
                    Err(e) => {
                        v["hits"] = json!([]);
                        v["problem"] = json!(e.to_string());
                    }
                }
                out.push(v);
            }
        }
    }
    Ok(json!({ "query": query, "work": work, "commentaries": out }))
}

/// First chosen or named lexicon that is ready (otherwise error with reason).
async fn one_lexicon(state: &AppState, work: WorkId, module: Option<&str>) -> Result<Arc<Lexicon>, RpcError> {
    let names = chosen(state, work, module, SourceKind::Lexicon).await?;
    if names.is_empty() {
        return Err(RpcError::new(REFUSED, tc("sources_msg.sword.no_lexicon", &[])));
    }
    let mut problems = Vec::new();
    for name in names {
        match lexicon_by_name(state, &name).await? {
            Source::Ready(lexicon) => return Ok(lexicon),
            Source::Problem { module, problem, .. } => problems.push(format!("{module}: {problem}")),
        }
    }
    Err(RpcError::new(REFUSED, tc("sources_msg.sword.lexicon_unreadable", &[("problems", problems.join("; ").into())])))
}

/// `lexicon.lookup {key, module?, work?}` → `{module, key, found, text, notice, truncated}`.
pub async fn lexicon_lookup(
    state: &AppState,
    work: WorkId,
    key: &str,
    module: Option<&str>,
) -> Result<Value, RpcError> {
    if key.trim().is_empty() || key.chars().count() > 200 {
        return Err(RpcError::new(INVALID_PARAMS, tc("sources_msg.sword.key_length", &[])));
    }
    let lexicon = one_lexicon(state, work, module).await?;
    let source = Arc::clone(&lexicon);
    let wanted = key.to_owned();
    // Round 19c: with lemma probe (Strong spellings, endings, umlaut, compound), `probe` = tried form.
    let article = tokio::task::spawn_blocking(crate::i18n::carry(move || source.lookup_lemma(&wanted)))
        .await
        .map_err(|e| RpcError::new(INTERNAL, e.to_string()))?
        .map_err(|e| RpcError::new(REFUSED, e.to_string()))?;
    Ok(match article {
        Some((a, probe)) => {
            let (text, cut) = clipped(&a.text, MAX_TEXT_CHARS);
            json!({ "module": lexicon.info.name, "key": a.key, "found": true, "text": text,
                    "notice": lexicon.info.notice, "truncated": cut || a.truncated, "probe": probe,
                    "asked": key.trim() })
        }
        None => json!({ "module": lexicon.info.name, "key": key.trim(), "found": false, "text": null,
                        "notice": lexicon.info.notice, "truncated": false, "probe": null, "asked": key.trim() }),
    })
}

/// `lexicon.keys {module?, work?, prefix?, limit?}` → `{module, keys, truncated, total, notice}`.
pub async fn lexicon_keys(
    state: &AppState,
    work: WorkId,
    prefix: &str,
    module: Option<&str>,
    limit: usize,
) -> Result<Value, RpcError> {
    let lexicon = one_lexicon(state, work, module).await?;
    let source = Arc::clone(&lexicon);
    let p = prefix.to_owned();
    let list = tokio::task::spawn_blocking(crate::i18n::carry(move || source.keys(&p, limit)))
        .await
        .map_err(|e| RpcError::new(INTERNAL, e.to_string()))?
        .map_err(|e| RpcError::new(REFUSED, e.to_string()))?;
    Ok(json!({ "module": lexicon.info.name, "prefix": prefix, "keys": list.keys, "truncated": list.truncated,
               "total": list.total, "notice": lexicon.info.notice }))
}

/// `lexicon.search {query, module?, work?, limit?}` → `{module, query, hits: [{key, excerpt, in_key}],
/// truncated, notice}`.
pub async fn lexicon_search(
    state: &AppState,
    work: WorkId,
    query: &str,
    module: Option<&str>,
    limit: usize,
) -> Result<Value, RpcError> {
    if query.trim().is_empty() || query.chars().count() > 200 {
        return Err(RpcError::new(INVALID_PARAMS, tc("sources_msg.sword.query_length", &[])));
    }
    let lexicon = one_lexicon(state, work, module).await?;
    let source = Arc::clone(&lexicon);
    let q = query.to_owned();
    let deadline = Instant::now() + REFERENCE_SEARCH_TIME;
    let (hits, truncated) = tokio::task::spawn_blocking(crate::i18n::carry(move || source.search(&q, limit, deadline)))
        .await
        .map_err(|e| RpcError::new(INTERNAL, e.to_string()))?
        .map_err(|e| RpcError::new(REFUSED, e.to_string()))?;
    Ok(json!({ "module": lexicon.info.name, "query": query, "hits": hits, "truncated": truncated,
               "notice": lexicon.info.notice }))
}

/// `bible.work_sources {work}` → `{work, bible, commentaries, lexicons}`.
pub async fn work_sources_value(state: &AppState, work: WorkId) -> Result<Value, RpcError> {
    let bible = state.store.work_bible(work).await?;
    let sources = work_sources(state, work).await?;
    Ok(json!({ "work": work, "bible": bible, "commentaries": sources.commentaries, "lexicons": sources.lexicons }))
}

/// Names from a list of texts (`null`/missing = empty).
fn names_param(params: &Value, name: &str) -> Result<Vec<String>, RpcError> {
    match params.get(name) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(list)) => {
            list.iter().map(|v| v.as_str().map(str::to_owned)).collect::<Option<Vec<_>>>().ok_or_else(|| {
                RpcError::new(INVALID_PARAMS, tc("sources_msg.sword.name_list", &[("name", name.into())]))
            })
        }
        Some(_) => Err(RpcError::new(INVALID_PARAMS, tc("sources_msg.sword.name_list", &[("name", name.into())]))),
    }
}

/// `bible.set_work_sources {work, commentaries, lexicons}`: only found, readable modules of the right kind
/// (spelling as in the conf); otherwise -32602 with reason. The human only.
pub async fn set_work_sources(
    state: &AppState,
    work: WorkId,
    commentaries: &[String],
    lexicons: &[String],
) -> Result<Value, RpcError> {
    let (found, ..) = scan(state).await?;
    let check = |kind: SourceKind, names: &[String]| -> Result<Vec<String>, RpcError> {
        names
            .iter()
            .map(|n| n.trim())
            .filter(|n| !n.is_empty())
            .map(|n| {
                let info =
                    found.find_kind(kind, n).ok_or_else(|| RpcError::new(INVALID_PARAMS, source_missing(kind, n)))?;
                if let Some(why) = &info.problem {
                    let args = [("name", info.name.as_str().into()), ("problem", why.as_str().into())];
                    let message = if kind == SourceKind::Commentary {
                        tc("sources_msg.sword.commentary_problem", &args)
                    } else {
                        tc("sources_msg.sword.lexicon_problem", &args)
                    };
                    return Err(RpcError::new(INVALID_PARAMS, message));
                }
                Ok(info.name.clone())
            })
            .collect()
    };
    let commentaries = check(SourceKind::Commentary, commentaries)?;
    let lexicons = check(SourceKind::Lexicon, lexicons)?;
    state.store.set_work_sword_sources(work, &commentaries, &lexicons).await?;
    work_sources_value(state, work).await
}

/// Escape HTML (`&`, `<`, `>`, `"`, `'`).
pub fn html_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

/// Commentaries of the work's selection on `reference` as finished, escaped HTML:
/// `<details class="commentaries" data-widget="commentaries">…</details>` (opens/closes without JS, per module
/// name, text, licence notice). Empty = no commentaries chosen or no entries (then the caller shows nothing).
pub async fn commentary_panel_html(state: &AppState, work: WorkId, reference: &str) -> String {
    let Ok(value) = commentary_lookup(state, work, reference, None).await else { return String::new() };
    let groups: Vec<&Value> = value["commentaries"]
        .as_array()
        .map(|l| l.iter().filter(|c| c["entries"].as_array().is_some_and(|e| !e.is_empty())).collect())
        .unwrap_or_default();
    if groups.is_empty() {
        return String::new();
    }
    let count: usize = groups.iter().map(|g| g["entries"].as_array().map_or(0, Vec::len)).sum();
    let summary = tc("sources_msg.commentary.summary", &[("reference", reference.trim().into())]);
    let entries = if count == 1 {
        tc("sources_msg.commentary.one_entry", &[("count", count.into())])
    } else {
        tc("sources_msg.commentary.entries", &[("count", count.into())])
    };
    let mut html = format!(
        "<details class=\"commentaries\" data-widget=\"commentaries\"><summary>{} <span class=\"muted tiny\">({})</span></summary>",
        html_escape(&summary),
        html_escape(&entries)
    );
    for group in groups {
        let module = group["module"].as_str().unwrap_or("");
        let description = group["description"].as_str().unwrap_or("");
        // R17c: one <details> per module (opens/closes without JavaScript); the widget “commentaries”
        // (web/src/commentaries.ts) opens or closes all and remembers the state.
        html.push_str(&format!(
            "<details class=\"commentary\" data-commentary=\"{}\" open><summary class=\"hit-group-title\">{}{}</summary>",
            html_escape(module),
            html_escape(&tc("sources_msg.commentary.group", &[("module", module.into())])),
            if description.is_empty() {
                String::new()
            } else {
                format!(" <span class=\"muted tiny\">– {}</span>", html_escape(description))
            }
        ));
        for entry in group["entries"].as_array().into_iter().flatten() {
            html.push_str(&format!(
                "<div class=\"commentary-entry\"><strong class=\"small\">{}</strong><p>{}{}</p></div>",
                html_escape(entry["ref"].as_str().unwrap_or("")),
                html_escape(entry["text"].as_str().unwrap_or("")),
                if entry["truncated"] == json!(true) {
                    format!(
                        " <span class=\"muted tiny\">{}</span>",
                        html_escape(&tc("sources_msg.commentary.shortened", &[]))
                    )
                } else {
                    String::new()
                }
            ));
        }
        html.push_str(&format!(
            "<p class=\"muted tiny commentary-notice\">{}</p></details>",
            html_escape(&license(group["notice"].as_str()))
        ));
    }
    html.push_str("</details>");
    html
}

/// Hit groups „Kommentar“ (one group per module, with module name) for the shared literature search field as
/// escaped HTML. Links go to `/literatur/bibel?work=<work>&ref=<ref>&kom=1<tail>`; `tail` is a finished, already
/// escaped query rest that starts with „&“ (key, rev, lq, lmode) or is empty. Time limit 3 s in total, at most
/// 10 hits per group.
pub async fn commentary_hits_html(state: &AppState, work: WorkId, query: &str, tail: &str) -> String {
    if query.trim().is_empty() || query.chars().count() > 200 {
        return String::new();
    }
    let search = commentary_search(state, work, query, None, HITS_PER_GROUP, HITS_TIME);
    let Ok(Ok(value)) = tokio::time::timeout(HITS_TIME + Duration::from_millis(500), search).await else {
        return String::new();
    };
    let mut html = String::new();
    for group in value["commentaries"].as_array().into_iter().flatten() {
        let hits = group["hits"].as_array().cloned().unwrap_or_default();
        if hits.is_empty() {
            continue;
        }
        let module = group["module"].as_str().unwrap_or("");
        html.push_str(&format!(
            "<section class=\"hit-group commentary-hits\" data-hit-group=\"commentary\" data-module=\"{}\"><div class=\"muted tiny hit-group-title\">{}</div><ul>",
            html_escape(module),
            html_escape(&tc("sources_msg.commentary.group", &[("module", module.into())]))
        ));
        for hit in hits.iter().take(HITS_PER_GROUP) {
            let reference = hit["ref"].as_str().unwrap_or("");
            let href = format!(
                "/literatur/bibel?work={work}&amp;ref={}&amp;kom=1{tail}",
                crate::view::query_escape(reference)
            );
            let (excerpt, _) = clipped(hit["text"].as_str().unwrap_or(""), 200);
            html.push_str(&format!(
                "<li><a href=\"{href}\">{}</a> <span class=\"small\">{}</span></li>",
                html_escape(reference),
                html_escape(&excerpt)
            ));
        }
        html.push_str("</ul>");
        if group["truncated"] == json!(true) {
            html.push_str(&format!(
                "<p class=\"muted tiny\">{}</p>",
                html_escape(&tc("sources_msg.commentary.more_hits", &[]))
            ));
        }
        html.push_str(&format!(
            "<p class=\"muted tiny\">{}</p></section>",
            html_escape(&license(group["notice"].as_str()))
        ));
    }
    html
}

/// „Lizenz: <notice>“ (without a notice „keine Angabe“) in the language of the request.
fn license(notice: Option<&str>) -> String {
    let notice = notice.map(str::to_owned).unwrap_or_else(|| tc("sources_msg.commentary.no_notice", &[]));
    tc("sources_msg.commentary.license", &[("notice", notice.into())])
}

// ---- RPC -------------------------------------------------------------------------------------------

fn text_param<'a>(params: &'a Value, name: &str) -> Result<&'a str, RpcError> {
    params
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| RpcError::new(INVALID_PARAMS, tc("sources_msg.sword.param_missing", &[("name", name.into())])))
}

fn key_param(params: &Value) -> Result<i64, RpcError> {
    let wrong = || RpcError::new(INVALID_PARAMS, tc("sources_msg.sword.key_number", &[]));
    match params.get("key") {
        Some(Value::Number(n)) => n.as_i64().ok_or_else(wrong),
        Some(Value::String(s)) => s.trim().parse().map_err(|_| wrong()),
        _ => Err(wrong()),
    }
}

/// Check module folders: one folder per line, trimmed, empty lines dropped; absolute paths (or `~/…`).
pub fn parse_dirs(lines: &[String]) -> Result<Vec<String>, RpcError> {
    let mut out: Vec<String> = Vec::new();
    for line in lines {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.chars().count() > MAX_DIR_CHARS || line.chars().any(char::is_control) {
            return Err(RpcError::new(
                INVALID_PARAMS,
                tc("sources_msg.sword.dir_invalid", &[("max", MAX_DIR_CHARS.into())]),
            ));
        }
        let path = match line.strip_prefix("~/") {
            Some(rest) => match std::env::var_os("HOME").filter(|h| !h.is_empty()) {
                Some(home) => PathBuf::from(home).join(rest).display().to_string(),
                None => return Err(RpcError::new(INVALID_PARAMS, tc("sources_msg.sword.no_home", &[]))),
            },
            None => line.to_owned(),
        };
        if !PathBuf::from(&path).is_absolute() {
            return Err(RpcError::new(INVALID_PARAMS, tc("sources_msg.sword.dir_relative", &[("dir", line.into())])));
        }
        if !out.contains(&path) {
            out.push(path);
        }
    }
    if out.len() > MAX_DIRS {
        return Err(RpcError::new(INVALID_PARAMS, tc("sources_msg.sword.too_many_dirs", &[("max", MAX_DIRS.into())])));
    }
    Ok(out)
}

/// `bible.modules {work?}`: folders, modules (Bibles, since R15 also commentaries and lexicons, each with `kind`),
/// notes and the choice of the work (Bible, plus `work_sources` with commentaries and lexicons).
pub async fn modules_value(state: &AppState, work: WorkId) -> Result<Value, RpcError> {
    let (found, dirs, default) = scan(state).await?;
    let chosen = state.store.work_bible(work).await?;
    let mut modules: Vec<Value> = Vec::new();
    for info in found.all() {
        // R17c: encryption state with the keyring; the key itself never stands in the answer.
        let stored = stored_cipher_key(state, info).await.is_some();
        let mut entry = info_json_with(info, cipher_state(info, stored));
        entry["key_stored"] = json!(stored);
        modules.push(entry);
    }
    let sources = work_sources(state, work).await?;
    Ok(json!({
        "dirs": dirs.iter().map(|d| d.display().to_string()).collect::<Vec<_>>(),
        "default_dirs": default,
        "modules": modules,
        "problems": found.problems,
        "work": work,
        "work_bible": chosen,
        "work_sources": { "commentaries": sources.commentaries, "lexicons": sources.lexicons },
    }))
}

/// Read a reference (`bible.lookup`): the input is read with the reference check, the text looked up in the
/// versification of the module.
pub async fn lookup(state: &AppState, choice: &Choice, reference: &str) -> Result<Value, RpcError> {
    let _ = state;
    let Some(bible) = choice.bible() else {
        return Err(RpcError::new(REFUSED, no_bible(choice)));
    };
    let found = baruch_scripture::parse(reference);
    let mut out = Vec::new();
    for f in found.iter().take(20) {
        let Some(p) = f.passage.as_ref() else {
            out.push(json!({ "input": f.input, "status": f.status.code(), "message": f.message(), "verses": [] }));
            continue;
        };
        if f.status != RefStatus::Valid {
            out.push(json!({ "input": f.input, "normalized": f.normalized, "status": f.status.code(),
                             "message": f.message(), "verses": [] }));
            continue;
        }
        // R17c: through the mapping German → versification of the module (read_passage); truncated from PassageText.
        let read = match read_passage(bible, p.book.id, p.start, p.end) {
            Ok(read) => read,
            Err(why) => {
                let status = if why.starts_with(MISMATCH_PREFIX) { "versification_mismatch" } else { "module_error" };
                out.push(json!({ "input": f.input, "normalized": f.normalized, "status": status,
                                 "message": why, "verses": [] }));
                continue;
            }
        };
        // Round 19c: per verse the German reference (mapped back from the module's versification) or the reason why
        // it cannot be mapped.
        let list: Vec<Value> = read
            .verses
            .iter()
            .zip(&read.module_refs)
            .map(|(v, at)| {
                let (german_ref, german_problem) = german_of(bible, *at);
                json!({ "chapter": v.chapter, "verse": v.verse, "text": v.text, "german_ref": german_ref,
                        "german_problem": german_problem })
            })
            .collect();
        let missing = read.verses.iter().filter(|v| v.text.is_none()).count();
        out.push(json!({
            "input": f.input, "normalized": f.normalized, "book": p.book.name, "book_id": p.book.id,
            "status": f.status.code(), "message": f.message(), "verses": list, "missing": missing,
            "truncated": read.truncated,
            "module_ref": module_ref(bible, p.book.id, p.start, p.end, &read),
            "rules": read.rules,
        }));
    }
    Ok(json!({ "bible": choice.json(), "refs": out }))
}

/// Round 19c: mapping a verse back from the versification of `bible` into German (Schlachter 1951):
/// `(Some("Mal 3,20"), None)` or `(None, Some(reason))` if it cannot be mapped.
pub fn german_of(bible: &Bible, at: VerseRef) -> (Option<String>, Option<String>) {
    match mapping::map_verse(bible.v11n, german(), at) {
        mapping::Mapped::Same { to } | mapping::Mapped::Moved { to, .. } => (Some(to.label()), None),
        mapping::Mapped::Unmapped { reason } => (None, Some(reason)),
    }
}

fn no_bible(choice: &Choice) -> String {
    match choice {
        Choice::None => tc("sources_msg.sword.no_bible_chosen", &[]),
        Choice::Missing(name) => tc("sources_msg.sword.bible_missing", &[("name", name.into())]),
        Choice::Unusable(info, why) => bible_problem(&info.name, why),
        Choice::Ready(_) => String::new(),
    }
}

/// Linear search (`bible.search`).
/// „Bibelmodul <name>: <why>“ in the language of the request.
fn bible_problem(name: &str, why: &str) -> String {
    tc("sources_msg.sword.bible_problem", &[("name", name.into()), ("problem", why.into())])
}

pub async fn search(choice: &Choice, query: &str, limit: usize) -> Result<Value, RpcError> {
    let Choice::Ready(bible) = choice else { return Err(RpcError::new(REFUSED, no_bible(choice))) };
    if query.trim().is_empty() || query.chars().count() > 200 {
        return Err(RpcError::new(INVALID_PARAMS, tc("sources_msg.sword.query_length", &[])));
    }
    let bible = Arc::clone(bible);
    let query_owned = query.to_owned();
    let started = Instant::now();
    let result = tokio::task::spawn_blocking(crate::i18n::carry(move || {
        bible.search(&query_owned, limit, started + SEARCH_TIME)
    }))
    .await
    .map_err(|e| RpcError::new(INTERNAL, e.to_string()))?
    .map_err(|e| RpcError::new(REFUSED, e.to_string()))?;
    let hits: Vec<Value> = result
        .hits
        .iter()
        .map(|h| {
            let label = baruch_scripture::find_book(&h.book).map_or_else(|| h.book.clone(), |b| b.abbr.to_owned());
            json!({ "ref": format!("{label} {},{}", h.chapter, h.verse), "book_id": h.book, "chapter": h.chapter,
                    "verse": h.verse, "text": h.text })
        })
        .collect();
    Ok(json!({ "bible": choice.json(), "query": query, "hits": hits, "truncated": result.truncated,
               "searched_verses": result.searched_verses, "ms": started.elapsed().as_millis() as u64 }))
}

/// Wording check of a section (`bible.check {work, key}`): current revision of the section.
pub async fn check_section(state: &AppState, work: WorkId, key: i64) -> Result<Value, RpcError> {
    let seq = state.store.node_seq(work, key).await?;
    let loaded = state.store.load_work(work).await?;
    let node = loaded
        .nodes
        .get(seq)
        .ok_or_else(|| RpcError::new(INVALID_PARAMS, tc("sources_msg.sword.unknown_section", &[])))?;
    let body = node.current_index().map(|i| node.revisions[i].body.clone()).unwrap_or_default();
    let mut result = crate::scripture::check_for_work(state, Some(work), &body).await?;
    result["work"] = json!(work);
    result["key"] = json!(key);
    result["title"] = json!(node.title);
    Ok(result)
}

// ---- Check display (R17c) -------------------------------------------------------------------------

/// State of a reference for the display: `stimmt`, `weicht_ab`, `ohne_zitat`, `ungeprueft`, `fehler`.
pub fn mark_state(code: &str) -> &'static str {
    match code {
        "match" => "stimmt",
        "differs" => "weicht_ab",
        "no_quote" => "ohne_zitat",
        "unchecked" => "ungeprueft",
        _ => "fehler",
    }
}

/// German label of a state (for page and widget).
pub fn state_label(state: &str) -> &'static str {
    match state {
        "stimmt" => "stimmt",
        "weicht_ab" => "weicht ab",
        "ohne_zitat" => "ohne Zitat",
        "ungeprueft" => "ungeprüft",
        _ => "nicht prüfbar",
    }
}

/// Text of the shown revision of a section: `rev` (number of the revision) or the current one. Answer: (title,
/// number of the revision, text).
pub async fn shown_text(
    state: &AppState,
    work: WorkId,
    key: i64,
    rev: Option<u32>,
) -> Result<(String, u32, String), RpcError> {
    let seq = state.store.node_seq(work, key).await?;
    let loaded = state.store.load_work(work).await?;
    let node = loaded
        .nodes
        .get(seq)
        .ok_or_else(|| RpcError::new(INVALID_PARAMS, tc("sources_msg.sword.unknown_section", &[])))?;
    let revision = match rev {
        Some(number) => node.revision(number).ok_or_else(|| {
            RpcError::new(INVALID_PARAMS, tc("sources_msg.sword.no_revision", &[("number", number.into())]))
        })?,
        None => node
            .current_index()
            .and_then(|i| node.revisions.get(i))
            .or_else(|| node.revisions.last())
            .ok_or_else(|| RpcError::new(INVALID_PARAMS, tc("sources_msg.sword.section_without_revision", &[])))?,
    };
    Ok((node.title.clone(), revision.number, revision.body.clone()))
}

/// `bible.marks {work?, key, rev?}` (read-only): result of the wording check of the shown revision as
/// marks; spans are byte ranges in the text of this revision. Licence notice under `bible.license` and
/// `notice`.
pub async fn marks(state: &AppState, work: WorkId, key: i64, rev: Option<u32>) -> Result<Value, RpcError> {
    let (title, number, body) = shown_text(state, work, key, rev).await?;
    let choice = of_work(state, work).await?;
    marks_with(&choice, work, key, number, &title, &body).map_err(|m| RpcError::new(INVALID_PARAMS, m))
}

/// Like [`marks`] with a given Bible and a given text (for page and tests).
pub fn marks_with(choice: &Choice, work: WorkId, key: i64, rev: u32, title: &str, body: &str) -> Result<Value, String> {
    if body.chars().count() > crate::scripture::MAX_TEXT_CHARS {
        return Err(tc("sources_msg.sword.text_too_long", &[("max", crate::scripture::MAX_TEXT_CHARS.into())]));
    }
    let found: Vec<Found> = crate::scripture::refs(body).into_iter().take(crate::scripture::MAX_REFS).collect();
    let wording = check_wording(choice.bible(), body, &found);
    let (mut good, mut bad, mut other) = (0, 0, 0);
    let list: Vec<Value> = found
        .iter()
        .zip(&wording)
        .map(|(f, w)| {
            let state = mark_state(w.code);
            match state {
                "stimmt" => good += 1,
                "weicht_ab" => bad += 1,
                _ => other += 1,
            }
            json!({
                "ref": f.normalized.clone().unwrap_or_else(|| f.input.clone()),
                "ref_span": [f.span.0, f.span.1],
                "quote_span": w.quote_span.map(|(s, e)| [s, e]),
                "code": w.code,
                "state": state,
                "label": if w.code == "unchecked" { crate::scripture::UNCHECKED.to_owned() } else { w.label.clone() },
                "quote": w.quote,
                "bible_text": w.bible_text,
                "module_ref": w.module_ref,
                "diff_line": (!w.diff.is_empty()).then(|| wording::diff_line(&w.diff)),
                "module_spacing": w.module_spacing,
            })
        })
        .collect();
    let notice = match choice {
        Choice::Ready(b) => Some(b.info.notice.clone()),
        Choice::Unusable(info, _) => Some(info.notice.clone()),
        _ => None,
    };
    Ok(json!({
        "work": work, "key": key, "rev": rev, "title": title, "bible": choice.json(), "notice": notice,
        "marks": list, "counts": { "stimmt": good, "weicht_ab": bad, "andere": other },
        // Round 19c: note on strikingly many word fragments in the module (otherwise null).
        "module_note": choice_note(choice),
    }))
}

// ---- Keys of encrypted modules (R17c) ---------------------------------------------------------------

/// Check a key: 1 to [`MAX_CIPHER_KEY_CHARS`] characters, one line, no control characters.
fn checked_cipher_key(key: &str) -> Result<&str, RpcError> {
    let key = key.trim();
    if key.is_empty() || key.chars().count() > MAX_CIPHER_KEY_CHARS || key.chars().any(char::is_control) {
        return Err(RpcError::new(
            INVALID_PARAMS,
            tc("sources_msg.sword.cipher_key_length", &[("max", MAX_CIPHER_KEY_CHARS.into())]),
        ));
    }
    Ok(key)
}

/// Encrypted module by name (Bible, commentary or lexicon).
async fn enciphered_module(state: &AppState, module: &str) -> Result<BibleInfo, RpcError> {
    let (found, ..) = scan(state).await?;
    let name = module.trim();
    let info = found
        .all()
        .find(|m| m.name == name)
        .or_else(|| found.all().find(|m| m.name.eq_ignore_ascii_case(name)))
        .cloned()
        .ok_or_else(|| {
            RpcError::new(INVALID_PARAMS, tc("sources_msg.sword.module_missing", &[("name", name.into())]))
        })?;
    if !info.enciphered {
        return Err(RpcError::new(
            INVALID_PARAMS,
            tc("sources_msg.sword.not_enciphered", &[("name", info.name.as_str().into())]),
        ));
    }
    Ok(info)
}

/// Trial read with `key`: first existing entry decrypted, unpacked, valid UTF-8.
fn probe_with(info: &BibleInfo, key: &str) -> Result<(), BibleError> {
    match info.kind {
        SourceKind::Bible => baruch_sword::open_with_key(info, Some(key))?.probe(),
        SourceKind::Commentary => baruch_sword::open_commentary_with_key(info, Some(key))?.probe(),
        SourceKind::Lexicon => baruch_sword::open_lexicon_with_key(info, Some(key))?.probe(),
    }
}

/// `bible.cipher_key_set {module, key}` → `{module, cipher: "unlocked"}`. The key is trial-read first;
/// if it does not fit: -32000 [`CIPHER_KEY_WRONG`]. Stored only in the keyring (account [`cipher_account`]), never
/// in answers, messages or logs. Human only.
pub async fn cipher_key_set(state: &AppState, module: &str, key: &str) -> Result<Value, RpcError> {
    let key = checked_cipher_key(key)?.to_owned();
    let info = enciphered_module(state, module).await?;
    let probe = {
        let (info, key) = (info.clone(), key.clone());
        tokio::task::spawn_blocking(crate::i18n::carry(move || probe_with(&info, &key)))
            .await
            .map_err(|e| RpcError::new(INTERNAL, e.to_string()))?
    };
    if probe.is_err() {
        // No reason in its wording: it could carry fragments of the wrongly decrypted content.
        return Err(RpcError::new(REFUSED, CIPHER_KEY_WRONG));
    }
    crate::connections::secrets::set(&state.secrets, &cipher_account(&info.name), &key).await.map_err(|e| {
        RpcError::new(REFUSED, tc("sources_msg.sword.keyring_refused_set", &[("error", e.to_string().into())]))
    })?;
    Ok(json!({ "module": info.name, "cipher": "unlocked" }))
}

/// `bible.cipher_key_delete {module, confirm: true}` → `{module, cipher: "locked"}` (or `unlocked` if the
/// conf itself carries a key). Human only.
pub async fn cipher_key_delete(state: &AppState, module: &str, confirm: bool) -> Result<Value, RpcError> {
    if !confirm {
        return Err(RpcError::new(INVALID_PARAMS, tc("sources_msg.sword.confirm_delete", &[])));
    }
    let info = enciphered_module(state, module).await?;
    crate::connections::secrets::delete(&state.secrets, &cipher_account(&info.name)).await.map_err(|e| {
        RpcError::new(REFUSED, tc("sources_msg.sword.keyring_refused_delete", &[("error", e.to_string().into())]))
    })?;
    Ok(json!({ "module": info.name, "cipher": cipher_state(&info, false) }))
}

/// RPC `bible.*` (`docs/RPC.md`, „Bibeln“).
pub async fn call(state: &AppState, method: &str, params: Value) -> Result<Value, RpcError> {
    let work = work_param(state, &params).await?;
    let module = params.get("module").and_then(Value::as_str).map(str::trim).filter(|m| !m.is_empty());
    let choice = || async {
        match module {
            Some(name) => by_name(state, name).await,
            None => of_work(state, work).await,
        }
    };
    match method {
        "bible.modules" => modules_value(state, work).await,
        "bible.set_dirs" => {
            match params.get("dirs") {
                None | Some(Value::Null) => state.store.set_bible_dirs(None).await?,
                Some(Value::Array(list)) => {
                    let lines: Vec<String> = list.iter().map(|v| v.as_str().unwrap_or_default().to_owned()).collect();
                    if list.iter().any(|v| !v.is_string()) {
                        return Err(RpcError::new(INVALID_PARAMS, tc("sources_msg.sword.dirs_list", &[])));
                    }
                    let dirs = parse_dirs(&lines)?;
                    state.store.set_bible_dirs(Some(&dirs)).await?;
                }
                Some(_) => return Err(RpcError::new(INVALID_PARAMS, tc("sources_msg.sword.dirs_list_or_null", &[]))),
            }
            modules_value(state, work).await
        }
        "bible.set_work" => {
            let chosen = match params.get("module") {
                None | Some(Value::Null) => None,
                Some(Value::String(s)) if s.trim().is_empty() => None,
                Some(Value::String(s)) => Some(s.trim().to_owned()),
                Some(_) => return Err(RpcError::new(INVALID_PARAMS, tc("sources_msg.sword.module_or_null", &[]))),
            };
            if let Some(name) = &chosen {
                let (found, ..) = scan(state).await?;
                let info = found.find(name).ok_or_else(|| {
                    RpcError::new(
                        INVALID_PARAMS,
                        tc("sources_msg.sword.bible_missing", &[("name", name.as_str().into())]),
                    )
                })?;
                if let Some(why) = &info.problem {
                    return Err(RpcError::new(INVALID_PARAMS, bible_problem(&info.name, why)));
                }
                state.store.set_work_bible(work, Some(&info.name)).await?;
            } else {
                state.store.set_work_bible(work, None).await?;
            }
            modules_value(state, work).await
        }
        "bible.lookup" => {
            let reference = text_param(&params, "ref")?;
            if reference.chars().count() > 500 {
                return Err(RpcError::new(INVALID_PARAMS, tc("sources_msg.sword.ref_too_long", &[])));
            }
            lookup(state, &choice().await?, reference).await
        }
        "bible.search" => {
            let query = text_param(&params, "query")?;
            let limit =
                params.get("limit").and_then(Value::as_u64).map_or(SEARCH_LIMIT, |l| (l as usize).clamp(1, SEARCH_MAX));
            search(&choice().await?, query, limit).await
        }
        "bible.check" => check_section(state, work, key_param(&params)?).await,
        "bible.marks" => {
            let rev = match params.get("rev") {
                None | Some(Value::Null) => None,
                Some(v) => Some(
                    v.as_u64()
                        .and_then(|r| u32::try_from(r).ok())
                        .or_else(|| v.as_str().and_then(|s| s.trim().parse().ok()))
                        .ok_or_else(|| RpcError::new(INVALID_PARAMS, tc("sources_msg.sword.rev_number", &[])))?,
                ),
            };
            marks(state, work, key_param(&params)?, rev).await
        }
        "bible.cipher_key_set" => {
            cipher_key_set(state, text_param(&params, "module")?, text_param(&params, "key")?).await
        }
        "bible.cipher_key_delete" => {
            let confirm = params.get("confirm").and_then(Value::as_bool).unwrap_or(false);
            cipher_key_delete(state, text_param(&params, "module")?, confirm).await
        }
        "bible.work_sources" => work_sources_value(state, work).await,
        "bible.set_work_sources" => {
            let commentaries = names_param(&params, "commentaries")?;
            let lexicons = names_param(&params, "lexicons")?;
            set_work_sources(state, work, &commentaries, &lexicons).await
        }
        "commentary.lookup" => {
            let reference = text_param(&params, "ref")?;
            if reference.trim().is_empty() || reference.chars().count() > 500 {
                return Err(RpcError::new(INVALID_PARAMS, tc("sources_msg.sword.ref_length", &[])));
            }
            commentary_lookup(state, work, reference, module).await
        }
        "commentary.search" => {
            let query = text_param(&params, "query")?;
            let limit =
                params.get("limit").and_then(Value::as_u64).map_or(SEARCH_LIMIT, |l| (l as usize).clamp(1, SEARCH_MAX));
            commentary_search(state, work, query, module, limit, REFERENCE_SEARCH_TIME).await
        }
        "lexicon.lookup" => lexicon_lookup(state, work, text_param(&params, "key")?, module).await,
        "lexicon.keys" => {
            let prefix = params.get("prefix").and_then(Value::as_str).unwrap_or("");
            if prefix.chars().count() > 200 {
                return Err(RpcError::new(INVALID_PARAMS, tc("sources_msg.sword.prefix_too_long", &[])));
            }
            let limit = params.get("limit").and_then(Value::as_u64).map_or(100, |l| (l as usize).clamp(1, 500));
            lexicon_keys(state, work, prefix, module, limit).await
        }
        "lexicon.search" => {
            let query = text_param(&params, "query")?;
            let limit =
                params.get("limit").and_then(Value::as_u64).map_or(SEARCH_LIMIT, |l| (l as usize).clamp(1, SEARCH_MAX));
            lexicon_search(state, work, query, module, limit).await
        }
        _ => Err(RpcError::new(METHOD_NOT_FOUND, format!("unknown method {method}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directories_must_be_absolute_and_are_deduplicated() {
        let ok = parse_dirs(&["  /a/sword ".into(), String::new(), "/a/sword".into(), "/b".into()]).unwrap();
        assert_eq!(ok, vec!["/a/sword".to_owned(), "/b".to_owned()], "getrimmt, ohne Leerzeilen und Doppel");
        assert!(parse_dirs(&["relativ/ordner".into()]).is_err(), "relativer Pfad abgelehnt");
        assert!(parse_dirs(&["/a\u{7}".into()]).is_err(), "Steuerzeichen abgelehnt");
    }
}
