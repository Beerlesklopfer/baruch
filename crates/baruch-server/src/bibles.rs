// SPDX-License-Identifier: AGPL-3.0-or-later
//! SWORD-Bibeln (Wunsch 2026-11-06): Modulordner, Bibel je Werk und Wortlautprüfung. RPC `bible.*`
//! (`docs/RPC.md`, „Bibeln“), MCP `bible_modules`/`bible_lookup`/`bible_search` ([`crate::mcp::bibles`]),
//! Formular auf der Seite Einstellungen → Quellen ([`crate::connections`]).
//!
//! - Modulordner: Liste im Store (Migration 0030), ohne Eintrag die Vorgaben von [`baruch_sword::default_dirs`].
//! - Bibel je Werk: Modulname im Store; gelesen über [`baruch_sword`] (Format-Leser `sword-modules`, CC0).
//! - [`check_wording`]: je Stelle Text der Bibel und Vergleich mit einem Zitat beim Verweis
//!   ([`baruch_scripture::wording`]). Ohne Bibel bleibt der Wortlaut „ungeprüft“.
//!
//! Antworten nennen immer Modul, Versifikation und den Lizenzvermerk der conf. Nur lesend außer
//! `bible.set_dirs`/`bible.set_work`/`bible.set_work_sources` (nur Mensch: nicht über MCP, nicht über `/act`).
//!
//! Runde 15 (Punkt 5a/5b): **Kommentare und Lexika** als Recherchewerkzeug. `bible.modules` meldet je Modul
//! `kind` (`bible`, `commentary`, `lexicon`); `commentary.lookup/search`, `lexicon.lookup/keys/search`,
//! `bible.work_sources`/`bible.set_work_sources` (Auswahl je Werk, mehrere, Migration 0037); MCP
//! `commentary_lookup`, `lexicon_lookup`, `lexicon_search` (nur lesend). Für die Literatur fertige, maskierte
//! HTML-Bausteine: [`commentary_panel_html`] (`<details>`, ohne JavaScript) und [`commentary_hits_html`]
//! (Treffergruppen „Kommentar“ im Suchfeld, Zeitgrenze 3 s).

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
use crate::rpc::{INTERNAL, INVALID_PARAMS, METHOD_NOT_FOUND, REFUSED, RpcError, work_param};

/// Höchstzahl der Modulordner.
pub const MAX_DIRS: usize = 20;
/// Höchstlänge eines Ordners (Zeichen).
pub const MAX_DIR_CHARS: usize = 1000;
/// Vorgabe und Höchstwert der Treffer von `bible.search`.
pub const SEARCH_LIMIT: usize = 50;
pub const SEARCH_MAX: usize = 200;
/// Zeitgrenze der linearen Suche.
pub const SEARCH_TIME: Duration = Duration::from_secs(5);
/// Höchstlänge des Bibeltexts einer Stelle in Antworten (Zeichen); längere Stellen werden gekürzt.
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

/// Methoden, die etwas ändern (nur Mensch: nicht über `/act`, nicht über MCP).
pub const WRITES: &[&str] =
    &["bible.set_dirs", "bible.set_work", "bible.set_work_sources", "bible.cipher_key_set", "bible.cipher_key_delete"];

/// Höchstlänge eines Modulschlüssels (Zeichen).
pub const MAX_CIPHER_KEY_CHARS: usize = 255;

/// Antwort auf einen Schlüssel, der das Modul nicht entschlüsselt.
pub const CIPHER_KEY_WRONG: &str = "Schlüssel passt nicht zum Modul; nichts gespeichert.";

/// Bedient dieses Modul die RPC-Methode?
pub fn handles(method: &str) -> bool {
    METHODS.contains(&method)
}

// ---- Ordner, Module, Wahl --------------------------------------------------------------------------

/// Modulordner und ob es die Vorgaben sind.
pub async fn dirs(state: &AppState) -> Result<(Vec<PathBuf>, bool), RpcError> {
    Ok(match state.store.bible_dirs().await? {
        Some(dirs) => (dirs.into_iter().map(PathBuf::from).collect(), false),
        None => (baruch_sword::default_dirs(), true),
    })
}

/// Alle Bibelmodule der Modulordner (liest nur `mods.d`).
pub async fn scan(state: &AppState) -> Result<(Scan, Vec<PathBuf>, bool), RpcError> {
    let (dirs, default) = dirs(state).await?;
    let list = dirs.clone();
    let found = tokio::task::spawn_blocking(move || baruch_sword::scan(&list))
        .await
        .map_err(|e| RpcError::new(INTERNAL, e.to_string()))?;
    Ok((found, dirs, default))
}

/// Konto im Schlüsselbund (Dienst „baruch“) für den Schlüssel eines verschlüsselten Moduls (R17c).
pub fn cipher_account(module: &str) -> String {
    format!("sword-cipher:{}", module.to_lowercase())
}

/// Hinterlegter Schlüssel eines verschlüsselten Moduls (nur über `AppState::secrets`); `None` bei einem
/// unverschlüsselten Modul (dann fragt Baruch den Schlüsselbund gar nicht), ohne Eintrag oder wenn der
/// Schlüsselbund nicht antwortet. Der Schlüssel verlässt diese Funktion nur Richtung SWORD-Leser.
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

/// `none` (nicht verschlüsselt), `locked` (verschlüsselt, Schlüssel fehlt), `unlocked` (Schlüssel im
/// Schlüsselbund oder schon in der conf).
pub fn cipher_state(info: &BibleInfo, key_stored: bool) -> &'static str {
    if !info.enciphered {
        "none"
    } else if key_stored || !info.locked() {
        "unlocked"
    } else {
        "locked"
    }
}

/// Kurzangaben eines Moduls für Antworten: Modul, Beschreibung, Versifikation, Lizenzvermerk; seit R17c auch
/// `cipher` (ohne Blick in den Schlüsselbund: `locked`, wenn die conf keinen Schlüssel trägt) und `notice`.
pub fn info_json(info: &BibleInfo) -> Value {
    info_json_with(info, cipher_state(info, false))
}

/// Wie [`info_json`] mit dem Stand der Verschlüsselung `cipher` (`none`, `locked`, `unlocked`).
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

/// Geöffnete Module, je conf-Datei, Name, Änderungszeit (eine geänderte conf wird neu geöffnet) und Prüfsumme
/// des Schlüssels (0 ohne Schlüssel; nie der Schlüssel selbst).
type CacheKey = (PathBuf, String, Option<SystemTime>, u64);

/// Prüfsumme eines Schlüssels für den Zwischenspeicher (der Schlüssel steht nie im Klartext darin).
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

/// Öffnet ein Modul (mit Zwischenspeicher, höchstens 8 offene Module) ohne Schlüssel.
pub fn open_cached(info: &BibleInfo) -> Result<Arc<Bible>, BibleError> {
    open_cached_with_key(info, None)
}

/// Öffnet ein Modul mit dem Schlüssel eines verschlüsselten Moduls (R17c; Zwischenspeicher wie
/// [`open_cached`], der Schlüssel zählt nur als Prüfsumme).
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

/// Die Bibel eines Werks.
pub enum Choice {
    /// Keine gewählt.
    None,
    /// Gewählt, aber in keinem Modulordner gefunden.
    Missing(String),
    /// Gefunden, aber nicht lesbar oder nicht unterstützt.
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

    /// Angaben für Antworten (`null` ohne Wahl).
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

/// Bibel nach Modulname (aus den Modulordnern).
pub async fn by_name(state: &AppState, name: &str) -> Result<Choice, RpcError> {
    let (found, ..) = scan(state).await?;
    let Some(info) = found.find(name).cloned() else { return Ok(Choice::Missing(name.to_owned())) };
    if let Some(why) = info.problem.clone() {
        return Ok(Choice::Unusable(Box::new(info), why));
    }
    // R17c: Schlüssel eines verschlüsselten Moduls holt Baruch selbst aus dem Schlüsselbund.
    let key = stored_cipher_key(state, &info).await;
    let opened = {
        let info = info.clone();
        tokio::task::spawn_blocking(move || open_cached_with_key(&info, key.as_deref()))
            .await
            .map_err(|e| RpcError::new(INTERNAL, e.to_string()))?
    };
    Ok(match opened {
        Ok(bible) => Choice::Ready(bible),
        Err(e) => Choice::Unusable(Box::new(info), e.to_string()),
    })
}

/// Die Bibel des Werks `work`.
pub async fn of_work(state: &AppState, work: WorkId) -> Result<Choice, RpcError> {
    match state.store.work_bible(work).await? {
        None => Ok(Choice::None),
        Some(name) => by_name(state, &name).await,
    }
}

// ---- Wortlaut --------------------------------------------------------------------------------------

/// Ergebnis der Wortlautprüfung einer Stelle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WordingResult {
    /// `unchecked`, `invalid_ref`, `versification_mismatch`, `missing_verse`, `module_error`, `no_quote`,
    /// `match`, `differs`.
    pub code: &'static str,
    /// Deutsch, z. B. „Wortlaut stimmt“, „weicht ab“, „kein Zitat gefunden“.
    pub label: String,
    /// Text der Bibel zur Stelle (nur mit Bibel und lesbarer Stelle).
    pub bible_text: Option<String>,
    /// Das Zitat im Text bei der Stelle.
    pub quote: Option<String>,
    pub partial: bool,
    pub diff: Vec<DiffPart>,
    /// R17c: Byte-Bereich des Zitatinhalts (ohne Anführungszeichen) im geprüften Text, wenn ein Zitat zugeordnet ist.
    pub quote_span: Option<(usize, usize)>,
    /// R17c: gelesene Stelle in der Zählung des Moduls („Joel 2,28–32 (KJV)“), nur wenn sie von der deutschen abweicht.
    pub module_ref: Option<String>,
    /// Runde 19c: Der Wortlaut stimmt nur ohne Leerraum (Trennreste im Modul wie „die ses“); `code` bleibt `match`.
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

/// R17c: Text einer Stelle, gelesen in der Zählung des Moduls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PassageText {
    /// Verse in der Zählung des Moduls (Kapitel, Vers wie in `module_refs`); `text: None` = Vers fehlt im Modul.
    pub verses: Vec<baruch_sword::Verse>,
    /// Die gelesenen Verse in der Zählung des Moduls (gleiche Reihenfolge wie `verses`).
    pub module_refs: Vec<VerseRef>,
    /// Die Stelle hat in deutscher Zählung mehr als [`baruch_sword::MAX_PASSAGE_VERSES`] Verse.
    pub truncated: bool,
    /// Angewandte Regeln der Abbildung (deutscher Kurztext, z. B. „Psalmüberschrift als Vers 1“).
    pub rules: Vec<&'static str>,
}

/// Anfang des Grunds, wenn sich eine Stelle nicht in die Zählung des Moduls abbilden lässt.
pub const MISMATCH_PREFIX: &str = "Versifikation passt nicht";

/// Stelle in deutscher Zählung aus `bible` lesen: Abbildung german() → bible.v11n (map_range, limit
/// baruch_sword::MAX_PASSAGE_VERSES), dann je Vers bible.verse(…). Err(Grund, Deutsch).
pub fn read_passage(bible: &Bible, book: &str, start: (u16, u16), end: (u16, u16)) -> Result<PassageText, String> {
    let mapped = mapping::map_range(german(), bible.v11n, book, start, end, baruch_sword::MAX_PASSAGE_VERSES)?;
    let mut verses = Vec::with_capacity(mapped.verses.len());
    for at in &mapped.verses {
        let text = bible.verse(at.book, at.chapter, at.verse).map_err(|e| e.to_string())?;
        verses.push(baruch_sword::Verse { chapter: at.chapter, verse: at.verse, text });
    }
    Ok(PassageText { verses, module_refs: mapped.verses, truncated: mapped.truncated, rules: mapped.rules })
}

/// Einträge eines Kommentars zu einer Stelle in **deutscher** Zählung (Runde 23a, Codereview quellen-2).
pub struct CommentaryRead {
    /// Einträge, Kapitel und Vers in der Zählung des Kommentars.
    pub entries: Vec<baruch_sword::CommentaryEntry>,
    /// Buch (OSIS) in der Zählung des Kommentars.
    pub book: &'static str,
    /// Name der Zählung des Kommentars, wenn sie von der deutschen abweicht (für die Beschriftung „… (KJV)“).
    pub v11n: Option<&'static str>,
}

impl CommentaryRead {
    /// Beschriftung eines Eintrags: Stelle in der Zählung des Kommentars, bei abweichender Zählung mit ihrem Namen
    /// („Joel 2,28–32 (KJV)“), damit niemand sie für die deutsche Stelle hält.
    pub fn label(&self, entry: &baruch_sword::CommentaryEntry) -> String {
        let range = range_label(self.book, entry.start, entry.end);
        match self.v11n {
            Some(name) => format!("{range} ({name})"),
            None => range,
        }
    }
}

/// Kommentar zu einer Stelle in deutscher Zählung lesen: wie bei Bibeln ([`read_passage`]) erst german() →
/// Zählung des Kommentars abbilden, dann den abgebildeten Bereich nachschlagen. Vorher wurden die deutschen Nummern
/// unverändert als Nummern des Kommentars benutzt; bei einem Kommentar in KJV-Zählung kam so zu „Joel 3,1“ der
/// Kommentar zu KJV Joel 3,1 (deutsch 4,1) zurück, beschriftet mit der deutschen Stelle. `Err` = Grund (Deutsch).
/// Blockierend.
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

/// „Mal 4,1–6 (KJV)“: die gelesene Stelle in der Zählung des Moduls, nur wenn sie von der deutschen Angabe
/// abweicht (andere Zählung und andere Verse oder eine angewandte Regel); sonst `None`.
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

/// Wortlaut je Stelle in `refs` (aus `text`) gegen `bible`. Ohne Bibel: alle „ungeprüft“. Zitate kommen nur aus
/// [`crate::quotes::quote_spans`] (R17).
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
    // Andere Zählung im Modul: über die Abbildung lesen (R17); was sich nicht abbilden lässt, bleibt
    // „Versifikation passt nicht“ mit Grund.
    let read = match read_passage(bible, book.id, passage.start, passage.end) {
        Ok(read) => read,
        Err(why) if why.starts_with(MISMATCH_PREFIX) => {
            return WordingResult::plain("versification_mismatch", format!("{why} (Modul {})", bible.info.name));
        }
        Err(why) => return WordingResult::plain("module_error", why),
    };
    let module_ref = module_ref(bible, book.id, passage.start, passage.end, &read);
    // Runde 23a (Codereview quellen-6): `read.verses` trägt Kapitel und Vers in der Zählung des Moduls. Die Meldung
    // nennt den fehlenden Vers darum in deutscher Zählung (zurück abgebildet); lässt er sich nicht abbilden, steht
    // die Modulstelle mit dem Namen ihrer Zählung da – nie eine Modulnummer unter deutscher Flagge.
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

// ---- Wortfragmente im Modul (Runde 19c) ---------------------------------------------------------------

/// Bezeichnung einer Stelle, deren Zitat nur ohne Leerraum stimmt (Trennreste im Modul).
pub const SPACING_LABEL: &str = "Wortlaut stimmt (Leerraum im Modul)";
/// Höchstens so viele Verse liest die Stichprobe für die Schätzung der Wortfragmente.
pub const FRAGMENT_SAMPLE_VERSES: usize = 4000;
/// Zeitgrenze der Stichprobe.
pub const FRAGMENT_SAMPLE_TIME: Duration = Duration::from_secs(2);

type NoteKey = (PathBuf, String, Option<SystemTime>);

fn note_cache() -> &'static Mutex<HashMap<NoteKey, Option<String>>> {
    static CACHE: OnceLock<Mutex<HashMap<NoteKey, Option<String>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Hinweis zu einem Modul mit auffällig vielen Wortfragmenten (vermutlich Trennreste aus einem PDF), sonst `None`.
/// Geschätzt über [`baruch_scripture::wording::fragment_survey`] auf einer Stichprobe der ersten
/// [`FRAGMENT_SAMPLE_VERSES`] Verse (höchstens [`FRAGMENT_SAMPLE_TIME`]); das Ergebnis bleibt je conf-Datei und
/// Änderungszeit im Speicher. Blockierend (liest das Modul); ein Lesefehler ergibt keinen Hinweis.
pub fn module_note(bible: &Bible) -> Option<String> {
    let modified = std::fs::metadata(&bible.info.conf_file).and_then(|m| m.modified()).ok();
    let key = (bible.info.conf_file.clone(), bible.info.name.clone(), modified);
    if let Some(note) = note_cache().lock().expect("Zwischenspeicher der Modulhinweise").get(&key) {
        return note.clone();
    }
    // Runde 23a (Gutachten R19): ein Lauf je Modul. Wer zugleich nach demselben Modul fragt, wartet auf den ersten
    // Lauf und nimmt dann dessen Ergebnis aus dem Zwischenspeicher, statt die Stichprobe (bis 2 s) selbst zu lesen.
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
        Ok((sample, _)) => fragment_note(&bible.info.name, &sample),
        Err(_) => None,
    };
    let mut map = note_cache().lock().expect("Zwischenspeicher der Modulhinweise");
    if map.len() >= 32 {
        map.clear();
    }
    map.insert(key, note.clone());
    note
}

/// Zahl der Stichproben-Läufe von [`module_note`] in diesem Prozess (für Tests des „ein Lauf je Modul“).
pub fn module_note_runs() -> usize {
    MODULE_NOTE_RUNS.load(std::sync::atomic::Ordering::SeqCst)
}

static MODULE_NOTE_RUNS: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// Hinweistext aus einer Stichprobe (öffentlich für Tests): `None`, wenn das Modul unauffällig ist.
pub fn fragment_note(module: &str, sample: &[String]) -> Option<String> {
    let survey = wording::fragment_survey(sample.iter().map(String::as_str));
    if !survey.fragmented() {
        return None;
    }
    let examples = survey.examples.iter().map(|e| format!("„{e}“")).collect::<Vec<_>>().join(", ");
    Some(format!(
        "Modul {module} hat auffällig viele Wortfragmente ({} von {} Wortpaaren, z. B. {examples}) – vermutlich \
         Silbentrennreste; Baruch vergleicht den Wortlaut darum auch ohne Leerraum.",
        survey.suspects, survey.pairs
    ))
}

/// Hinweis zur Bibel einer Wahl (nur bei einer lesbaren Bibel).
pub fn choice_note(choice: &Choice) -> Option<String> {
    choice.bible().and_then(module_note)
}

// ---- Kommentare und Lexika (R15, Punkt 5a) ----------------------------------------------------------

/// Zeitgrenze der Kommentar- und Lexikonsuche über RPC/MCP.
pub const REFERENCE_SEARCH_TIME: Duration = Duration::from_secs(5);
/// Zeitgrenze aller Treffergruppen „Kommentar“ im Suchfeld der Literatur zusammen.
pub const HITS_TIME: Duration = Duration::from_secs(3);
/// Höchstzahl der Treffer je Gruppe im Suchfeld.
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

/// Ein Kommentar bzw. Lexikon nach Name: bereit oder mit ehrlichem Grund, warum nicht.
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
    let word = if kind == SourceKind::Commentary { "Kommentar" } else { "Lexikon" };
    let Some(info) = found.find_kind(kind, name).cloned() else {
        return Ok(Source::problem(name, None, format!("{word} {name} in keinem Modulordner gefunden")));
    };
    if let Some(why) = info.problem.clone() {
        return Ok(Source::problem(&info.name, Some(&info), why));
    }
    let key = stored_cipher_key(state, &info).await;
    let opened = {
        let info = info.clone();
        tokio::task::spawn_blocking(move || cached(cache, &info, key.as_deref(), open))
            .await
            .map_err(|e| RpcError::new(INTERNAL, e.to_string()))?
    };
    Ok(match opened {
        Ok(source) => Source::Ready(source),
        Err(e) => Source::problem(&info.name, Some(&info), e.to_string()),
    })
}

/// Kommentar nach Modulname.
pub async fn commentary_by_name(state: &AppState, name: &str) -> Result<Source<Commentary>, RpcError> {
    open_source(state, SourceKind::Commentary, name, commentary_cache(), baruch_sword::open_commentary_with_key).await
}

/// Lexikon nach Modulname.
pub async fn lexicon_by_name(state: &AppState, name: &str) -> Result<Source<Lexicon>, RpcError> {
    open_source(state, SourceKind::Lexicon, name, lexicon_cache(), baruch_sword::open_lexicon_with_key).await
}

/// Gewählte Kommentare und Lexika des Werks.
pub async fn work_sources(state: &AppState, work: WorkId) -> Result<baruch_store::WorkSources, RpcError> {
    Ok(state.store.work_sword_sources(work).await?)
}

/// Module für eine Anfrage: `module` ausdrücklich, sonst die Auswahl des Werks.
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

/// Deutsche Stellenangabe eines Bereichs (`Ps 139,23–24`, `Ps 139,23–140,2`).
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

/// Text auf `max` Zeichen kürzen (mit „…“); `true`, wenn gekürzt.
pub fn clipped(text: &str, max: usize) -> (String, bool) {
    baruch_sword::reference::clip(text, max)
}

fn source_json(info: &BibleInfo) -> Value {
    json!({ "module": info.name, "description": info.description, "versification": info.versification,
            "notice": info.notice })
}

/// `commentary.lookup {ref, module?, work?}` → `{ref, commentaries: [{module, description, versification,
/// notice, entries: [{ref, text, truncated}], problem?}]}`. Jede Angabe in `ref` wird mit der Stellenprüfung
/// (deutsche Zählung) gelesen, in die Verszählung des Kommentars **abgebildet** ([`read_commentary`], Runde 23a) und
/// dort nachgeschlagen; `entries[].ref` nennt die Stelle des Kommentars, bei abweichender Zählung mit deren Namen.
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
                        problems.push(format!("„{}“: {}", f.input, f.message()));
                        continue;
                    };
                    let source = Arc::clone(&commentary);
                    let (book, start, end) = (p.book.id, p.start, p.end);
                    // Runde 23a (Codereview quellen-2): deutsche Stelle erst in die Zählung des Kommentars abbilden.
                    let read = tokio::task::spawn_blocking(move || read_commentary(&source, book, start, end))
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
                    problems.push(format!("„{reference}“ ist keine lesbare Stellenangabe"));
                }
                if entries.is_empty() && problems.is_empty() {
                    problems.push(format!("{} hat zu dieser Stelle keinen Eintrag", commentary.info.name));
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
/// text}], truncated, problem?}]}`; Zeitgrenze [`REFERENCE_SEARCH_TIME`] gesamt.
pub async fn commentary_search(
    state: &AppState,
    work: WorkId,
    query: &str,
    module: Option<&str>,
    limit: usize,
    time: Duration,
) -> Result<Value, RpcError> {
    if query.trim().is_empty() || query.chars().count() > 200 {
        return Err(RpcError::new(INVALID_PARAMS, "query: 1 bis 200 Zeichen"));
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
                let found = tokio::task::spawn_blocking(move || source.search(&q, limit, deadline))
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

/// Erstes gewähltes bzw. genanntes Lexikon, das bereit ist (sonst Fehler mit Grund).
async fn one_lexicon(state: &AppState, work: WorkId, module: Option<&str>) -> Result<Arc<Lexicon>, RpcError> {
    let names = chosen(state, work, module, SourceKind::Lexicon).await?;
    if names.is_empty() {
        return Err(RpcError::new(
            REFUSED,
            "Kein Lexikon gewählt: Einstellungen → Werk → Kommentare und Lexika oder module angeben.",
        ));
    }
    let mut problems = Vec::new();
    for name in names {
        match lexicon_by_name(state, &name).await? {
            Source::Ready(lexicon) => return Ok(lexicon),
            Source::Problem { module, problem, .. } => problems.push(format!("{module}: {problem}")),
        }
    }
    Err(RpcError::new(REFUSED, format!("Lexikon nicht lesbar: {}", problems.join("; "))))
}

/// `lexicon.lookup {key, module?, work?}` → `{module, key, found, text, notice, truncated}`.
pub async fn lexicon_lookup(
    state: &AppState,
    work: WorkId,
    key: &str,
    module: Option<&str>,
) -> Result<Value, RpcError> {
    if key.trim().is_empty() || key.chars().count() > 200 {
        return Err(RpcError::new(INVALID_PARAMS, "key: 1 bis 200 Zeichen"));
    }
    let lexicon = one_lexicon(state, work, module).await?;
    let source = Arc::clone(&lexicon);
    let wanted = key.to_owned();
    // Runde 19c: mit Lemma-Probe (Strong-Schreibweisen, Endungen, Umlaut, Kompositum), `probe` = probierte Form.
    let article = tokio::task::spawn_blocking(move || source.lookup_lemma(&wanted))
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
    let list = tokio::task::spawn_blocking(move || source.keys(&p, limit))
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
        return Err(RpcError::new(INVALID_PARAMS, "query: 1 bis 200 Zeichen"));
    }
    let lexicon = one_lexicon(state, work, module).await?;
    let source = Arc::clone(&lexicon);
    let q = query.to_owned();
    let deadline = Instant::now() + REFERENCE_SEARCH_TIME;
    let (hits, truncated) = tokio::task::spawn_blocking(move || source.search(&q, limit, deadline))
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

/// Namen aus einer Liste von Texten (`null`/fehlt = leer).
fn names_param(params: &Value, name: &str) -> Result<Vec<String>, RpcError> {
    match params.get(name) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(list)) => list
            .iter()
            .map(|v| v.as_str().map(str::to_owned))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| RpcError::new(INVALID_PARAMS, format!("{name}: Liste von Modulnamen erwartet"))),
        Some(_) => Err(RpcError::new(INVALID_PARAMS, format!("{name}: Liste von Modulnamen erwartet"))),
    }
}

/// `bible.set_work_sources {work, commentaries, lexicons}`: nur gefundene, lesbare Module der richtigen Art
/// (Schreibweise wie in der conf); sonst -32602 mit Grund. Nur der Mensch.
pub async fn set_work_sources(
    state: &AppState,
    work: WorkId,
    commentaries: &[String],
    lexicons: &[String],
) -> Result<Value, RpcError> {
    let (found, ..) = scan(state).await?;
    let check = |kind: SourceKind, names: &[String]| -> Result<Vec<String>, RpcError> {
        let word = if kind == SourceKind::Commentary { "Kommentar" } else { "Lexikon" };
        names
            .iter()
            .map(|n| n.trim())
            .filter(|n| !n.is_empty())
            .map(|n| {
                let info = found.find_kind(kind, n).ok_or_else(|| {
                    RpcError::new(INVALID_PARAMS, format!("{word} {n} in keinem Modulordner gefunden"))
                })?;
                if let Some(why) = &info.problem {
                    return Err(RpcError::new(INVALID_PARAMS, format!("{word} {}: {why}", info.name)));
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

/// HTML maskieren (`&`, `<`, `>`, `"`, `'`).
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

/// Kommentare der Werk-Auswahl zur Stelle `reference` als fertiges, maskiertes HTML: `<details class="commentaries" data-widget="commentaries">…</details>`
/// (ein-/ausklappbar ohne JS, je Modul Name, Text, Lizenzvermerk). Leer = keine Kommentare gewählt bzw. keine Einträge (dann zeigt der Aufrufer nichts).
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
    let mut html = format!(
        "<details class=\"commentaries\" data-widget=\"commentaries\"><summary>Kommentare zu {} <span class=\"muted tiny\">({} {})</span></summary>",
        html_escape(reference.trim()),
        count,
        if count == 1 { "Eintrag" } else { "Einträge" }
    );
    for group in groups {
        let module = group["module"].as_str().unwrap_or("");
        let description = group["description"].as_str().unwrap_or("");
        // R17c: je Modul ein eigenes <details> (ohne JavaScript auf-/zuklappbar); das Widget „commentaries“
        // (web/src/commentaries.ts) klappt alle auf bzw. zu und merkt sich den Zustand.
        html.push_str(&format!(
            "<details class=\"commentary\" data-commentary=\"{}\" open><summary class=\"hit-group-title\">Kommentar · {}{}</summary>",
            html_escape(module),
            html_escape(module),
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
                if entry["truncated"] == json!(true) { " <span class=\"muted tiny\">(gekürzt)</span>" } else { "" }
            ));
        }
        html.push_str(&format!(
            "<p class=\"muted tiny commentary-notice\">Lizenz: {}</p></details>",
            html_escape(group["notice"].as_str().unwrap_or("keine Angabe"))
        ));
    }
    html.push_str("</details>");
    html
}

/// Treffergruppen „Kommentar“ (je Modul eine Gruppe mit Modulname) für das gemeinsame Suchfeld der Literatur als maskiertes HTML.
/// Links zeigen auf `/literatur/bibel?work=<work>&ref=<Stelle>&kom=1<tail>`; `tail` ist ein fertiger, schon maskierter Anfragerest, der mit „&“ beginnt (key, rev, lq, lmode) oder leer ist. Zeitgrenze gesamt 3 s, höchstens 10 Treffer je Gruppe.
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
            "<section class=\"hit-group commentary-hits\" data-hit-group=\"commentary\" data-module=\"{}\"><div class=\"muted tiny hit-group-title\">Kommentar · {}</div><ul>",
            html_escape(module),
            html_escape(module)
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
            html.push_str("<p class=\"muted tiny\">weitere Treffer nicht gezeigt (Grenze erreicht)</p>");
        }
        html.push_str(&format!(
            "<p class=\"muted tiny\">Lizenz: {}</p></section>",
            html_escape(group["notice"].as_str().unwrap_or("keine Angabe"))
        ));
    }
    html
}

// ---- RPC -------------------------------------------------------------------------------------------

fn text_param<'a>(params: &'a Value, name: &str) -> Result<&'a str, RpcError> {
    params.get(name).and_then(Value::as_str).ok_or_else(|| RpcError::new(INVALID_PARAMS, format!("{name} fehlt")))
}

fn key_param(params: &Value) -> Result<i64, RpcError> {
    let wrong = || RpcError::new(INVALID_PARAMS, "key: fester Schlüssel (Zahl) erwartet");
    match params.get("key") {
        Some(Value::Number(n)) => n.as_i64().ok_or_else(wrong),
        Some(Value::String(s)) => s.trim().parse().map_err(|_| wrong()),
        _ => Err(wrong()),
    }
}

/// Modulordner prüfen: je Zeile ein Ordner, getrimmt, leere Zeilen fallen weg; absolute Pfade (oder `~/…`).
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
                format!("Ordner zu lang oder mit Steuerzeichen (höchstens {MAX_DIR_CHARS} Zeichen)"),
            ));
        }
        let path = match line.strip_prefix("~/") {
            Some(rest) => match std::env::var_os("HOME").filter(|h| !h.is_empty()) {
                Some(home) => PathBuf::from(home).join(rest).display().to_string(),
                None => return Err(RpcError::new(INVALID_PARAMS, "~ lässt sich nicht auflösen (HOME fehlt)")),
            },
            None => line.to_owned(),
        };
        if !PathBuf::from(&path).is_absolute() {
            return Err(RpcError::new(INVALID_PARAMS, format!("Ordner „{line}“: bitte einen absoluten Pfad angeben")));
        }
        if !out.contains(&path) {
            out.push(path);
        }
    }
    if out.len() > MAX_DIRS {
        return Err(RpcError::new(INVALID_PARAMS, format!("höchstens {MAX_DIRS} Modulordner")));
    }
    Ok(out)
}

/// `bible.modules {work?}`: Ordner, Module (Bibeln, seit R15 auch Kommentare und Lexika, je mit `kind`),
/// Hinweise und die Wahl des Werks (Bibel, dazu `work_sources` mit Kommentaren und Lexika).
pub async fn modules_value(state: &AppState, work: WorkId) -> Result<Value, RpcError> {
    let (found, dirs, default) = scan(state).await?;
    let chosen = state.store.work_bible(work).await?;
    let mut modules: Vec<Value> = Vec::new();
    for info in found.all() {
        // R17c: Stand der Verschlüsselung mit dem Schlüsselbund; der Schlüssel selbst steht nie in der Antwort.
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

/// Stelle lesen (`bible.lookup`): die Angabe wird mit der Stellenprüfung gelesen, der Text in der Zählung des
/// Moduls nachgeschlagen.
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
        // R17c: über die Abbildung deutsch → Zählung des Moduls (read_passage); truncated aus PassageText.
        let read = match read_passage(bible, p.book.id, p.start, p.end) {
            Ok(read) => read,
            Err(why) => {
                let status = if why.starts_with(MISMATCH_PREFIX) { "versification_mismatch" } else { "module_error" };
                out.push(json!({ "input": f.input, "normalized": f.normalized, "status": status,
                                 "message": why, "verses": [] }));
                continue;
            }
        };
        // Runde 19c: je Vers die deutsche Stelle (Rückabbildung aus der Zählung des Moduls) oder der Grund, warum
        // sie sich nicht abbilden lässt.
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

/// Runde 19c: Rückabbildung eines Verses aus der Zählung von `bible` ins Deutsche (Schlachter 1951):
/// `(Some("Mal 3,20"), None)` oder `(None, Some(Grund))`, wenn er sich nicht abbilden lässt.
pub fn german_of(bible: &Bible, at: VerseRef) -> (Option<String>, Option<String>) {
    match mapping::map_verse(bible.v11n, german(), at) {
        mapping::Mapped::Same { to } | mapping::Mapped::Moved { to, .. } => (Some(to.label()), None),
        mapping::Mapped::Unmapped { reason } => (None, Some(reason)),
    }
}

fn no_bible(choice: &Choice) -> String {
    match choice {
        Choice::None => "Keine Bibel gewählt: Einstellungen → Quellen → Bibeln (SWORD) oder module angeben.".to_owned(),
        Choice::Missing(name) => format!("Bibelmodul {name} in keinem Modulordner gefunden."),
        Choice::Unusable(info, why) => format!("Bibelmodul {}: {why}", info.name),
        Choice::Ready(_) => String::new(),
    }
}

/// Lineare Suche (`bible.search`).
pub async fn search(choice: &Choice, query: &str, limit: usize) -> Result<Value, RpcError> {
    let Choice::Ready(bible) = choice else { return Err(RpcError::new(REFUSED, no_bible(choice))) };
    if query.trim().is_empty() || query.chars().count() > 200 {
        return Err(RpcError::new(INVALID_PARAMS, "query: 1 bis 200 Zeichen"));
    }
    let bible = Arc::clone(bible);
    let query_owned = query.to_owned();
    let started = Instant::now();
    let result = tokio::task::spawn_blocking(move || bible.search(&query_owned, limit, started + SEARCH_TIME))
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

/// Wortlautprüfung eines Abschnitts (`bible.check {work, key}`): aktuelle Fassung des Abschnitts.
pub async fn check_section(state: &AppState, work: WorkId, key: i64) -> Result<Value, RpcError> {
    let seq = state.store.node_seq(work, key).await?;
    let loaded = state.store.load_work(work).await?;
    let node = loaded.nodes.get(seq).ok_or_else(|| RpcError::new(INVALID_PARAMS, "Abschnitt unbekannt"))?;
    let body = node.current_index().map(|i| node.revisions[i].body.clone()).unwrap_or_default();
    let mut result = crate::scripture::check_for_work(state, Some(work), &body).await?;
    result["work"] = json!(work);
    result["key"] = json!(key);
    result["title"] = json!(node.title);
    Ok(result)
}

// ---- Prüfanzeige (R17c) ---------------------------------------------------------------------------

/// Zustand einer Stelle für die Anzeige: `stimmt`, `weicht_ab`, `ohne_zitat`, `ungeprueft`, `fehler`.
pub fn mark_state(code: &str) -> &'static str {
    match code {
        "match" => "stimmt",
        "differs" => "weicht_ab",
        "no_quote" => "ohne_zitat",
        "unchecked" => "ungeprueft",
        _ => "fehler",
    }
}

/// Deutsche Bezeichnung eines Zustands (für Seite und Widget).
pub fn state_label(state: &str) -> &'static str {
    match state {
        "stimmt" => "stimmt",
        "weicht_ab" => "weicht ab",
        "ohne_zitat" => "ohne Zitat",
        "ungeprueft" => "ungeprüft",
        _ => "nicht prüfbar",
    }
}

/// Text der gezeigten Fassung eines Abschnitts: `rev` (Nummer der Fassung) oder die aktuelle. Antwort: (Titel,
/// Nummer der Fassung, Text).
pub async fn shown_text(
    state: &AppState,
    work: WorkId,
    key: i64,
    rev: Option<u32>,
) -> Result<(String, u32, String), RpcError> {
    let seq = state.store.node_seq(work, key).await?;
    let loaded = state.store.load_work(work).await?;
    let node = loaded.nodes.get(seq).ok_or_else(|| RpcError::new(INVALID_PARAMS, "Abschnitt unbekannt"))?;
    let revision = match rev {
        Some(number) => node
            .revision(number)
            .ok_or_else(|| RpcError::new(INVALID_PARAMS, format!("Fassung {number} gibt es nicht")))?,
        None => node
            .current_index()
            .and_then(|i| node.revisions.get(i))
            .or_else(|| node.revisions.last())
            .ok_or_else(|| RpcError::new(INVALID_PARAMS, "Abschnitt ohne Fassung"))?,
    };
    Ok((node.title.clone(), revision.number, revision.body.clone()))
}

/// `bible.marks {work?, key, rev?}` (nur lesend): Ergebnis der Wortlautprüfung der gezeigten Fassung als
/// Markierungen; Spans sind Byte-Bereiche im Text dieser Fassung. Lizenzvermerk unter `bible.license` und
/// `notice`.
pub async fn marks(state: &AppState, work: WorkId, key: i64, rev: Option<u32>) -> Result<Value, RpcError> {
    let (title, number, body) = shown_text(state, work, key, rev).await?;
    let choice = of_work(state, work).await?;
    marks_with(&choice, work, key, number, &title, &body).map_err(|m| RpcError::new(INVALID_PARAMS, m))
}

/// Wie [`marks`] mit gegebener Bibel und gegebenem Text (für Seite und Tests).
pub fn marks_with(choice: &Choice, work: WorkId, key: i64, rev: u32, title: &str, body: &str) -> Result<Value, String> {
    if body.chars().count() > crate::scripture::MAX_TEXT_CHARS {
        return Err(format!("text: höchstens {} Zeichen", crate::scripture::MAX_TEXT_CHARS));
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
        // Runde 19c: Hinweis auf auffällig viele Wortfragmente im Modul (sonst null).
        "module_note": choice_note(choice),
    }))
}

// ---- Schlüssel verschlüsselter Module (R17c) --------------------------------------------------------

/// Schlüssel prüfen: 1 bis [`MAX_CIPHER_KEY_CHARS`] Zeichen, eine Zeile, ohne Steuerzeichen.
fn checked_cipher_key(key: &str) -> Result<&str, RpcError> {
    let key = key.trim();
    if key.is_empty() || key.chars().count() > MAX_CIPHER_KEY_CHARS || key.chars().any(char::is_control) {
        return Err(RpcError::new(
            INVALID_PARAMS,
            format!("key: 1 bis {MAX_CIPHER_KEY_CHARS} Zeichen in einer Zeile; nichts gespeichert."),
        ));
    }
    Ok(key)
}

/// Verschlüsseltes Modul nach Name (Bibel, Kommentar oder Lexikon).
async fn enciphered_module(state: &AppState, module: &str) -> Result<BibleInfo, RpcError> {
    let (found, ..) = scan(state).await?;
    let name = module.trim();
    let info = found
        .all()
        .find(|m| m.name == name)
        .or_else(|| found.all().find(|m| m.name.eq_ignore_ascii_case(name)))
        .cloned()
        .ok_or_else(|| RpcError::new(INVALID_PARAMS, format!("Modul {name} in keinem Modulordner gefunden")))?;
    if !info.enciphered {
        return Err(RpcError::new(
            INVALID_PARAMS,
            format!("Modul {} ist nicht verschlüsselt; kein Schlüssel nötig", info.name),
        ));
    }
    Ok(info)
}

/// Probelesen mit `key`: erster vorhandener Eintrag entschlüsselt, entpackt, gültiges UTF-8.
fn probe_with(info: &BibleInfo, key: &str) -> Result<(), BibleError> {
    match info.kind {
        SourceKind::Bible => baruch_sword::open_with_key(info, Some(key))?.probe(),
        SourceKind::Commentary => baruch_sword::open_commentary_with_key(info, Some(key))?.probe(),
        SourceKind::Lexicon => baruch_sword::open_lexicon_with_key(info, Some(key))?.probe(),
    }
}

/// `bible.cipher_key_set {module, key}` → `{module, cipher: "unlocked"}`. Der Schlüssel wird vorher probegelesen;
/// passt er nicht: -32000 [`CIPHER_KEY_WRONG`]. Gespeichert nur im Schlüsselbund (Konto [`cipher_account`]), nie
/// in Antworten, Meldungen oder Protokollen. Nur Mensch.
pub async fn cipher_key_set(state: &AppState, module: &str, key: &str) -> Result<Value, RpcError> {
    let key = checked_cipher_key(key)?.to_owned();
    let info = enciphered_module(state, module).await?;
    let probe = {
        let (info, key) = (info.clone(), key.clone());
        tokio::task::spawn_blocking(move || probe_with(&info, &key))
            .await
            .map_err(|e| RpcError::new(INTERNAL, e.to_string()))?
    };
    if probe.is_err() {
        // Kein Grund im Wortlaut: Er könnte Bruchstücke des falsch entschlüsselten Inhalts tragen.
        return Err(RpcError::new(REFUSED, CIPHER_KEY_WRONG));
    }
    crate::connections::secrets::set(&state.secrets, &cipher_account(&info.name), &key)
        .await
        .map_err(|e| RpcError::new(REFUSED, format!("Schlüsselbund lehnt ab ({e}); nichts gespeichert.")))?;
    Ok(json!({ "module": info.name, "cipher": "unlocked" }))
}

/// `bible.cipher_key_delete {module, confirm: true}` → `{module, cipher: "locked"}` (bzw. `unlocked`, wenn die
/// conf selbst einen Schlüssel trägt). Nur Mensch.
pub async fn cipher_key_delete(state: &AppState, module: &str, confirm: bool) -> Result<Value, RpcError> {
    if !confirm {
        return Err(RpcError::new(INVALID_PARAMS, "Löschen bitte bestätigen (confirm: true); nichts geändert."));
    }
    let info = enciphered_module(state, module).await?;
    crate::connections::secrets::delete(&state.secrets, &cipher_account(&info.name))
        .await
        .map_err(|e| RpcError::new(REFUSED, format!("Schlüsselbund lehnt ab ({e}); nichts geändert.")))?;
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
                        return Err(RpcError::new(INVALID_PARAMS, "dirs: Liste von Texten erwartet"));
                    }
                    let dirs = parse_dirs(&lines)?;
                    state.store.set_bible_dirs(Some(&dirs)).await?;
                }
                Some(_) => return Err(RpcError::new(INVALID_PARAMS, "dirs: Liste von Texten oder null (Vorgaben)")),
            }
            modules_value(state, work).await
        }
        "bible.set_work" => {
            let chosen = match params.get("module") {
                None | Some(Value::Null) => None,
                Some(Value::String(s)) if s.trim().is_empty() => None,
                Some(Value::String(s)) => Some(s.trim().to_owned()),
                Some(_) => return Err(RpcError::new(INVALID_PARAMS, "module: Modulname oder null")),
            };
            if let Some(name) = &chosen {
                let (found, ..) = scan(state).await?;
                let info = found.find(name).ok_or_else(|| {
                    RpcError::new(INVALID_PARAMS, format!("Bibelmodul {name} in keinem Modulordner gefunden"))
                })?;
                if let Some(why) = &info.problem {
                    return Err(RpcError::new(INVALID_PARAMS, format!("Bibelmodul {}: {why}", info.name)));
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
                return Err(RpcError::new(INVALID_PARAMS, "ref: höchstens 500 Zeichen"));
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
                        .ok_or_else(|| RpcError::new(INVALID_PARAMS, "rev: Nummer der Fassung erwartet"))?,
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
                return Err(RpcError::new(INVALID_PARAMS, "ref: 1 bis 500 Zeichen"));
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
                return Err(RpcError::new(INVALID_PARAMS, "prefix: höchstens 200 Zeichen"));
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
