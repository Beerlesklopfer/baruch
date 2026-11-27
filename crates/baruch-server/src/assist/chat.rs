// SPDX-License-Identifier: AGPL-3.0-or-later
//! Abschnitts-Chat mit den Personas (Vertrag Runde 10; seit Runde 15, Schnittstelle S2, eigene Datei):
//! erwähnte Personas ([`mentions`]), eine Chat-Nachricht ausführen ([`chat`]) und dasselbe aus RPC-Parametern
//! ([`run_chat`], [`chat_json`]). Der Beitrag des Absenders ist sofort gespeichert und geht als
//! [`Progress::Posted`] an den Aufrufer.
//!
//! Runde 28 (chat-auftrag), Aufträge im Chat:
//! - **RFC** ([`is_rfc`]): „RFC“ am Anfang der Nachricht oder „@alle“ ruft jede aktive Persona genau einmal, der
//!   Lektor (nach Einstellung) zuletzt mit den Antworten der anderen gerahmt als Daten
//!   ([`super::OrderFrame::rfc_answers`]) und dem Auftrag zur Sammlung und Gewichtung; seine Befunde werden
//!   Sammelbefunde (`rfc_summary`). [`MAX_CHAT_PERSONAS`] gilt dafür nicht, die Warteschlange der Modelle schon
//!   (die Personas antworten nacheinander).
//! - **Schreibauftrag** ([`is_write_order`] oder `as_order`): Die erwähnte Persona (mit `as_order` ohne Erwähnung der
//!   Lektor) schreibt einen **Vorschlag** des Abschnitts ([`Mode::Propose`], nie übernommen) und dazu steht ein kurzer
//!   Beitrag im Chat ([`order_note`]). Löschen und neue Abschnitte bleiben beim Menschen (Gliederung). Nennt die
//!   Nachricht Befunde mit „[B<id>]“, schlägt Baruch für jeden „übernommen in Fassung n“ vor
//!   ([`crate::findings::rpc::propose`], Herkunft die Persona); der Mensch bestätigt. Seit Runde 23 zählen nur
//!   Verweise aus der Nachricht, nicht mehr aus dem Modelltext des Vorschlags.
//! - Ohne beides bleibt es beim Beitrag je erwähnter Persona (höchstens [`MAX_CHAT_PERSONAS`]).
//!
//! Runde 30 (Haken der Schnittstelle R30-S1): Alle Personas eines Aufrufs prüfen dieselbe Fassung
//! ([`super::basis::choose`], [`ChatOrder::basis`]) und antworten auf den Beitrag des Absenders (`links.reply_to`);
//! ein Varianten-Auftrag ([`super::variants::is_variants_order`]) geht an [`super::variants::run`]; nach dem Lektor
//! beim RFC legt [`super::solutions::rfc_proposal`] wahlweise einen Sammel-Vorschlag an.
//!
//! Runde 30c (chat-bedienung):
//! - **Anrede** ([`addressed`]): außer `@handle` (überall, wie bisher) zählt der Name oder das Handle ohne „@“ **am
//!   Satz- bzw. Beitragsanfang**, gefolgt von „,“ oder „:“ („Lektor, …“, „Exeget:“), Groß/klein egal; im Fließtext
//!   nicht („der Lektor meint“ ruft niemanden).
//! - **Relay** ([`line_addresses`]): Beginnt eine Zeile einer Persona-Antwort mit `@handle:` bzw. einer Anrede, antwortet
//!   die angesprochene Persona einmal – eine Stufe (Relay-Antworten rufen niemanden), höchstens
//!   [`MAX_RELAY_ANSWERS`] Relay-Antworten je Ausgangsnachricht, jede Persona höchstens einmal je Nachricht (wer schon
//!   geantwortet hat oder scheiterte, wird nicht erneut gerufen), nicht beim RFC und nicht beim Schreibauftrag. Der
//!   Relay-Beitrag trägt `relay_from` (die anredende Persona) und `reply_to` (deren Beitrag), der Auftrag die Antwort
//!   gerahmt ([`OrderFrame::relay`]); sichtbar „antwortet auf Beitrag n von @…“.
//! - **Fehler** jeder Persona merkt [`crate::chat_live::record_failure`] für die Live-Anzeige (`post.since`).
//! - **Prüfgrundlage** aus RPC und Formular: `basis` = `shown` | `current` | `<n>` ([`run_chat`]).
//!
//! Runde 23 (mcp-namen):
//! - **Zitatzeilen** („> …“, [`unquoted`]) rufen niemanden und machen weder RFC noch Schreibauftrag.
//! - **Ein Relay-Weg** ([`relay_targets`]): Anrede am Zeilenanfang für alle, mit [`ChatOrder::relay`] (MCP `chat`) dazu
//!   jede Erwähnung; `relay_section` in `mcp/tools.rs` gibt es nicht mehr.
//! - `persona.chat` gibt `done_proposals`, `done_notes`, `all_done` aus ([`chat_json`]), wenn eine Persona eigene
//!   Befunde als erledigt meldete.

use baruch_store::{IdeaOrigin, PersonaRow, WorkId};
use serde::Serialize;
use serde_json::{Value, json};
use tokio::sync::mpsc::UnboundedSender;

use super::{
    FramedAnswer, MAX_CHAT_PERSONAS, Mode, Order, OrderFrame, Outcome, PostLinks, Progress, SavedPost, Scope,
    check_text, context, post, run, send,
};
use crate::AppState;
use crate::rpc::{INVALID_PARAMS, RpcError};

/// Erwähnung, die beim RFC alle aktiven Personas ruft.
pub const ALL_MENTION: &str = "@alle";

/// Höchstens so viele Relay-Antworten je Ausgangsnachricht (Runde 30c).
pub const MAX_RELAY_ANSWERS: usize = 3;

/// Ist `c` ein Zeichen eines Handles?
fn handle_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '-'
}

/// Beginnt `text` (ohne Groß/klein) mit `word` als ganzem Wort? Dann die Bytes danach.
fn starts_with_word<'a>(text: &'a str, word: &str) -> Option<&'a str> {
    let n = word.chars().count();
    if n == 0 {
        return None;
    }
    let head: String = text.chars().take(n).collect();
    if head.chars().count() < n || head.to_lowercase() != word.to_lowercase() {
        return None;
    }
    let rest = &text[head.len()..];
    rest.chars().next().is_none_or(|c| !handle_char(c)).then_some(rest)
}

/// Folgt (nach Leerraum und `**`) ein „,“ oder „:“?
fn followed_by_address_mark(rest: &str) -> bool {
    let rest = rest.trim_start().trim_start_matches("**").trim_start();
    rest.starts_with(',') || rest.starts_with(':')
}

/// Anrede am Anfang von `text`: Name oder Handle ohne „@“ einer aktiven Persona, gefolgt von „,“ oder „:“.
fn address_at(text: &str, personas: &[PersonaRow]) -> Option<String> {
    let text = text.trim_start_matches("**");
    personas.iter().filter(|p| p.active).find_map(|p| {
        let bare = p.handle.trim_start_matches('@');
        [p.name.trim(), bare]
            .into_iter()
            .filter(|w| !w.is_empty())
            .any(|w| starts_with_word(text, w).is_some_and(followed_by_address_mark))
            .then(|| p.handle.clone())
    })
}

/// Anfänge der Sätze in `message` (Bytes): Beitragsanfang, nach Zeilenumbruch und nach „.“, „!“, „?“ mit Leerraum.
fn sentence_starts(message: &str) -> Vec<usize> {
    let mut out = Vec::new();
    let mut expect = true;
    let mut after_mark = false;
    for (i, c) in message.char_indices() {
        if c == '\n' {
            expect = true;
            after_mark = false;
            continue;
        }
        if c.is_whitespace() {
            if after_mark {
                expect = true;
                after_mark = false;
            }
            continue;
        }
        if expect {
            out.push(i);
            expect = false;
        }
        after_mark = matches!(c, '.' | '!' | '?');
    }
    out
}

/// Die Nachricht ohne Zitatzeilen (Runde 23): Zeilen, die – nach Leerraum – mit „>“ beginnen, sind fremder Text
/// („Zitieren“ setzt den Beitrag eines anderen als „> …“ an den Anfang der eigenen Nachricht). Sie rufen niemanden,
/// machen keinen RFC und keinen Schreibauftrag: [`addressed`], [`mentions`], [`is_rfc`] und [`is_write_order`] lesen
/// nur den Rest. Ohne Zitatzeile bleibt der Text unverändert (geliehen).
pub fn unquoted(message: &str) -> std::borrow::Cow<'_, str> {
    if !message.lines().any(|line| line.trim_start().starts_with('>')) {
        return std::borrow::Cow::Borrowed(message);
    }
    let kept: Vec<&str> = message.lines().filter(|line| !line.trim_start().starts_with('>')).collect();
    std::borrow::Cow::Owned(kept.join("\n"))
}

/// Angesprochene Personas (Runde 30c): `@handle` überall (wie [`mentions`]) und dazu Name oder Handle ohne „@“ am
/// Satz- bzw. Beitragsanfang mit „,“ oder „:“ („Lektor, …“, „Exeget:“). Nur aktive, ohne Doppelte, in der Reihenfolge
/// der Nachricht, höchstens [`MAX_CHAT_PERSONAS`]. „der Lektor meint“ im Fließtext zählt nicht. Zitatzeilen („> …“)
/// zählen nicht ([`unquoted`], Runde 23).
pub fn addressed(message: &str, personas: &[PersonaRow]) -> Vec<String> {
    let message = unquoted(message);
    let message = message.as_ref();
    let mut hits: Vec<(usize, String)> = Vec::new();
    let mut offset = 0;
    while let Some(at) = message[offset..].find('@') {
        let start = offset + at;
        let after = &message[start + 1..];
        let end = after.find(|c: char| !handle_char(c)).unwrap_or(after.len());
        let word = after[..end].to_lowercase();
        offset = start + 1 + end;
        if let Some(p) = personas.iter().find(|p| p.active && p.handle.trim_start_matches('@').to_lowercase() == word)
            && !word.is_empty()
        {
            hits.push((start, p.handle.clone()));
        }
    }
    for start in sentence_starts(message) {
        if let Some(handle) = address_at(&message[start..], personas) {
            hits.push((start, handle));
        }
    }
    hits.sort_by_key(|(at, _)| *at);
    let mut found: Vec<String> = Vec::new();
    for (_, handle) in hits {
        if !found.contains(&handle) {
            found.push(handle);
            if found.len() == MAX_CHAT_PERSONAS {
                break;
            }
        }
    }
    found
}

/// Personas, die eine Antwort am **Zeilenanfang** anspricht (Relay, Runde 30c): `@handle:` bzw. `@handle,` oder eine
/// Anrede (Name bzw. Handle ohne „@“ mit „,“/„:“), auch hinter Aufzählungszeichen und `**`. Zitatzeilen („> …“)
/// zählen nicht. Nur aktive, ohne Doppelte, in der Reihenfolge der Antwort.
pub fn line_addresses(answer: &str, personas: &[PersonaRow]) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for line in answer.lines() {
        let mut rest = line.trim_start();
        if rest.starts_with('>') {
            continue;
        }
        loop {
            let before = rest;
            for prefix in ["- ", "* ", "• ", "– ", "+ ", "**"] {
                rest = rest.strip_prefix(prefix).unwrap_or(rest).trim_start();
            }
            if rest == before {
                break;
            }
        }
        let handle = if let Some(after) = rest.strip_prefix('@') {
            let end = after.find(|c: char| !handle_char(c)).unwrap_or(after.len());
            let word = after[..end].to_lowercase();
            personas
                .iter()
                .find(|p| p.active && !word.is_empty() && p.handle.trim_start_matches('@').to_lowercase() == word)
                .filter(|_| followed_by_address_mark(&after[end..]))
                .map(|p| p.handle.clone())
        } else {
            address_at(rest, personas)
        };
        if let Some(handle) = handle
            && !found.contains(&handle)
        {
            found.push(handle);
        }
    }
    found
}

/// Auftrag der angesprochenen Persona beim Relay (von Baruch gebaut, nicht vom Modell; die Antwort selbst steht
/// gerahmt in [`OrderFrame::relay`]).
pub fn relay_text(from: &str, post: i64, message: &str) -> String {
    let text = format!(
        "{from} hat dich in Beitrag {post} direkt angesprochen. Gehe auf diese Anrede ein (die Antwort von {from} steht \
         als Daten im Chat). Die Nachricht des Autors war: {message}"
    );
    text.chars().take(super::MAX_QUESTION_CHARS).collect()
}

/// Erwähnte Personas in einer Nachricht (`@handle`, Groß-/Kleinschreibung egal): nur aktive, ohne
/// Doppelte, in der Reihenfolge der Nachricht, höchstens [`MAX_CHAT_PERSONAS`]. Zitatzeilen („> …“) zählen nicht
/// ([`unquoted`], Runde 23).
pub fn mentions(message: &str, personas: &[PersonaRow]) -> Vec<String> {
    let message = unquoted(message);
    let mut found: Vec<String> = Vec::new();
    let mut rest = message.as_ref();
    while let Some(at) = rest.find('@') {
        let after = &rest[at + 1..];
        let end = after.find(|c: char| !(c.is_alphanumeric() || c == '_' || c == '-')).unwrap_or(after.len());
        let word = after[..end].to_lowercase();
        rest = &after[end..];
        if word.is_empty() {
            continue;
        }
        if let Some(p) = personas.iter().find(|p| p.active && p.handle.trim_start_matches('@').to_lowercase() == word)
            && !found.contains(&p.handle)
        {
            found.push(p.handle.clone());
            if found.len() == MAX_CHAT_PERSONAS {
                break;
            }
        }
    }
    found
}

/// Ruft die Nachricht alle aktiven Personas (RFC)? „RFC“ als erstes Wort (Groß-/Kleinschreibung egal, auch „RFC:“)
/// oder „@alle“ als Erwähnung irgendwo (ganzes Wort: „@allerlei“ zählt nicht). Zitatzeilen („> …“) zählen nicht
/// ([`unquoted`], Runde 23): „> @alle …“ aus einem zitierten Beitrag ruft niemanden.
pub fn is_rfc(message: &str) -> bool {
    let message = unquoted(message);
    let message = message.as_ref();
    let first: String = message.trim_start().chars().take_while(|c| c.is_alphanumeric()).collect();
    if first.eq_ignore_ascii_case("rfc") {
        return true;
    }
    let lower = message.to_lowercase();
    lower.match_indices(ALL_MENTION).any(|(at, m)| {
        lower[at + m.len()..].chars().next().is_none_or(|c| !(c.is_alphanumeric() || c == '_' || c == '-'))
    })
}

/// Imperative eines Schreibauftrags (klein, ganzes erstes Wort nach Erwähnungen und „bitte“).
const WRITE_VERBS: &[&str] = &[
    "schreib",
    "schreibe",
    "formulier",
    "formuliere",
    "erstell",
    "erstelle",
    "verfass",
    "verfasse",
    "überarbeite",
    "überarbeit",
    "kürze",
    "kürz",
    "ergänze",
    "ergänz",
    "entwirf",
    "entwerfe",
    "glätte",
    "straffe",
    "redigiere",
    "übersetze",
];

/// Ist die Nachricht ein Schreibauftrag („@lektor Schreibe …“, „Bitte formuliere …“)? Erkannt am ersten Wort nach
/// Erwähnungen (`@…`), Satzzeichen und „bitte“ (Imperativ eines Schreibverbs, [`WRITE_VERBS`]). Fragen („Kannst du
/// schreiben, …?“) oder Nachrichten, in denen das Verb erst später steht, sind keine Aufträge – dafür gibt es
/// „als Auftrag“ (`as_order`). Zitatzeilen („> …“) zählen nicht ([`unquoted`], Runde 23).
pub fn is_write_order(message: &str) -> bool {
    let message = unquoted(message);
    let words = message
        .split(|c: char| c.is_whitespace() || matches!(c, ',' | ':' | ';' | '!' | '.'))
        .filter(|w| !w.is_empty());
    for word in words {
        let lower = word.to_lowercase();
        if lower.starts_with('@') || lower == "bitte" {
            continue;
        }
        return WRITE_VERBS.contains(&lower.as_str());
    }
    false
}

/// Wortanfänge eines Prüfauftrags (klein): „prüf den Vorschlag“, „bitte überprüfen“, „check das“, „Review bitte“.
const CHECK_STEMS: &[&str] =
    &["prüf", "überprüf", "nachprüf", "gegenprüf", "kontrollier", "check", "review", "verif", "examin", "proofread"];

/// Ist die Nachricht an eine direkt angesprochene Persona ein Prüfauftrag (Runde 36)? Ein Wort beginnt mit einem der
/// [`CHECK_STEMS`] (irgendwo in der Nachricht, auch als Frage „Kannst du das prüfen?“), oder die Nachricht nennt
/// einen Befund („[B12]“). Nur dann sieht die Persona ihre eigenen offenen Befunde ([`context::is_recheck`]); bei
/// einer schlichten Frage („@nathan wie alt war Mose?“) nicht – sonst zöge jede Frage die alten Befunde in die
/// Antwort. Erwähnungen (`@…`) und Zitatzeilen („> …“, [`unquoted`]) zählen nicht.
pub fn is_check_order(message: &str) -> bool {
    let message = unquoted(message);
    if !finding_refs(&message).is_empty() {
        return true;
    }
    message
        .split(|c: char| !(c.is_alphanumeric() || c == '@' || c == '_' || c == '-'))
        .filter(|word| !word.is_empty() && !word.starts_with('@'))
        .any(|word| {
            let lower = word.to_lowercase();
            CHECK_STEMS.iter().any(|stem| lower.starts_with(stem))
        })
}

/// „[B<id>]“ in einem Text (ohne Doppelte, in der Reihenfolge des Texts).
pub fn finding_refs(text: &str) -> Vec<i64> {
    let mut out: Vec<i64> = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("[B") {
        let after = &rest[at + 2..];
        let digits: String = after.chars().take_while(char::is_ascii_digit).collect();
        rest = &after[digits.len()..];
        if !digits.is_empty()
            && rest.starts_with(']')
            && let Ok(id) = digits.parse::<i64>()
            && !out.contains(&id)
        {
            out.push(id);
        }
    }
    out
}

/// Kurzer Beitrag der Persona zu ihrem Vorschlag (Runde 28). Baruch schreibt ihn (kein zweiter Modellaufruf); er
/// beginnt mit keiner Befund-Marke, damit er beim Zerlegen keinen Befund ergibt.
pub fn order_note(rev: u32) -> String {
    format!(
        "Fassung {rev} dieses Abschnitts liegt als Vorschlag bereit (noch nicht übernommen). Übernehmen oder verwerfen \
         kann nur der Mensch; neue Abschnitte anlegen und löschen bleiben bei ihm (Gliederung)."
    )
}

/// Ein Vorschlag, den ein Schreibauftrag im Chat angelegt hat.
#[derive(Debug, Clone, Serialize)]
pub struct ChatProposal {
    pub persona: String,
    pub rev: u32,
    pub url: String,
}

/// Ergebnis einer Chat-Nachricht.
#[derive(Debug, Clone, Serialize)]
pub struct ChatOutcome {
    /// Beitrag des Absenders, dann je Persona ihre Antwort.
    pub posts: Vec<SavedPost>,
    pub answers: Vec<Outcome>,
    /// Personas, deren Antwort scheiterte, mit Meldung (die übrigen Beiträge bleiben).
    pub errors: Vec<ChatError>,
    pub url: String,
    /// Runde 28: RFC bzw. „@alle“ (alle aktiven Personas, der Lektor zuletzt).
    pub rfc: bool,
    /// Runde 28: Vorschläge aus Schreibaufträgen.
    pub proposals: Vec<ChatProposal>,
    /// Runde 28: Befunde aus den Antworten (ids, `[B<id>]`).
    pub findings: Vec<i64>,
    /// Runde 28: Hinweise, die keine Persona betreffen (z. B. ein Statusvorschlag zu einem Befund scheiterte).
    pub notes: Vec<String>,
    /// Personas, die die Relay-Stufe gerufen hat (auch gescheiterte). Ihre Antworten rufen niemanden mehr – das
    /// MCP-Werkzeug `chat` ergänzt Erwähnungen nur aus den übrigen Antworten (eine Stufe).
    #[serde(skip)]
    pub relayed: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatError {
    pub persona: String,
    pub message: String,
}

/// Eine Chat-Nachricht: Werk, Abschnitt (`key`), Text, gezeigte Fassung, Absender und wahlweise eine
/// Persona, die auch ohne Erwähnung antworten soll (MCP `chat {persona}`).
#[derive(Debug, Clone)]
pub struct ChatOrder<'a> {
    pub work: WorkId,
    pub key: i64,
    pub message: &'a str,
    pub rev: Option<u32>,
    pub author: &'a str,
    pub extra: Option<&'a str>,
    /// Wie [`Order::web`]: Websuche zulassen (Oberfläche ja, MCP nie).
    pub web: bool,
    /// Runde 28: Die Nachricht ist ein Auftrag (Schreibaufträge werden Vorschläge); MCP und Vorgabe false.
    pub as_order: bool,
    /// Runde 30b: Prüfgrundlage aller Personas dieses Aufrufs (Vorgabe: die gezeigte Fassung).
    pub basis: super::basis::BasisChoice,
    /// Runde 23: Relay auch bei bloßer Erwähnung (`@handle` irgendwo in einer Antwort) – so ruft MCP `chat` seit
    /// Runde 15. Ohne (Oberfläche) ruft nur die Anrede am Zeilenanfang ([`line_addresses`]). Beides läuft durch
    /// dieselbe Relay-Stufe ([`relay_targets`]).
    pub relay: bool,
}

/// Wer beim RFC antwortet: alle aktiven Personas in ihrer Reihenfolge, der Lektor (nach Einstellung, falls aktiv)
/// herausgenommen und als zweites Ergebnis.
pub async fn rfc_personas(state: &AppState) -> Result<(Vec<String>, Option<String>), RpcError> {
    let (rows, _, lector) = crate::personas::rows_and_lector(state).await?;
    let lector = lector.filter(|l| rows.iter().any(|p| p.active && &p.handle == l));
    let others =
        rows.iter().filter(|p| p.active && Some(&p.handle) != lector.as_ref()).map(|p| p.handle.clone()).collect();
    Ok((others, lector))
}

/// Chat-Nachricht: erst der Beitrag von `author` (sofort gespeichert), dann nacheinander je erwähnter
/// aktiver Persona (dazu `extra`, falls genannt) ihre Antwort als Beitrag – beim RFC alle aktiven, beim
/// Schreibauftrag ein Vorschlag mit kurzem Beitrag (siehe Moduldoku). Antworten der Personas rufen keine weiteren
/// Personas auf.
pub async fn chat(
    state: &AppState,
    order: &ChatOrder<'_>,
    progress: Option<&UnboundedSender<Progress>>,
) -> Result<ChatOutcome, RpcError> {
    let ChatOrder { work, key, message, rev, author, extra, web, as_order, basis, relay } = order.clone();
    check_text(message, "Die Nachricht")?;
    let message = message.trim();
    if message.is_empty() {
        return Err(RpcError::new(INVALID_PARAMS, "Die Nachricht ist leer."));
    }
    let rfc = is_rfc(message);
    let write = !rfc && (as_order || is_write_order(message));
    let rows = state.store.persona_list().await?;
    let mut addressed = Vec::new();
    let mut lector_last: Option<String> = None;
    if rfc {
        let (others, lector) = rfc_personas(state).await?;
        addressed = others;
        lector_last = lector;
    } else {
        if let Some(extra) = extra.filter(|e| !e.trim().is_empty()) {
            addressed.push(context::active_persona(state, extra).await?.handle);
        }
        // Runde 30c: `@handle` überall und Anrede mit Namen am Satzanfang.
        for handle in self::addressed(message, &rows) {
            if !addressed.contains(&handle) && addressed.len() < MAX_CHAT_PERSONAS {
                addressed.push(handle);
            }
        }
        // „als Auftrag“ ohne Erwähnung: der Lektor schreibt.
        if write && as_order && addressed.is_empty() {
            addressed.extend(rfc_personas(state).await?.1);
        }
    }
    // Runde 30b: eine Prüfgrundlage für alle Personas dieses Aufrufs.
    let basis = super::basis::choose(state, work, key, rev, basis).await?;
    let first = {
        let _held = state.write_lock.lock().await;
        let seq = state.store.node_seq(work, key).await?;
        let id = state.store.add_post(work, seq, &post(author, message, vec![])).await?;
        SavedPost { id, author: author.to_owned(), body: message.to_owned() }
    };
    send(progress, Progress::Posted(first.clone()));
    let mut result = ChatOutcome {
        posts: vec![first],
        answers: Vec::new(),
        errors: Vec::new(),
        url: crate::view::node_url(work, key, rev, false, "", ""),
        rfc,
        proposals: Vec::new(),
        findings: Vec::new(),
        notes: Vec::new(),
        relayed: Vec::new(),
    };
    let first_id = result.posts[0].id;
    let base = |handle: &str, mode: Mode, frame: OrderFrame| Order {
        work,
        key,
        persona: handle.to_owned(),
        mode,
        text: message.to_owned(),
        rev: Some(basis.rev),
        asker: Some(author.to_owned()),
        post_question: false,
        sources: vec![],
        web,
        scope: Scope::Section,
        frame,
        links: PostLinks { reply_to: Some(first_id), ..Default::default() },
        basis: Some(basis.clone()),
    };
    // Runde 30b: Varianten-Auftrag („Varianten …“) statt des Personen-Durchlaufs.
    if super::variants::is_variants_order(message) {
        let variant_order = super::variants::VariantOrder {
            work,
            key,
            rev: Some(basis.rev),
            findings: finding_refs(message),
            count: None,
            asker: author.to_owned(),
            from_post: Some(first_id),
        };
        match super::variants::run(state, &variant_order, progress).await {
            Ok(outcome) => {
                for variant in &outcome.variants {
                    result.proposals.push(ChatProposal {
                        persona: outcome.persona.clone(),
                        rev: variant.rev,
                        url: variant.url.clone(),
                    });
                }
                result.notes.extend(outcome.notes);
            }
            Err(e) => {
                let persona = rfc_personas(state).await.ok().and_then(|(_, lector)| lector).unwrap_or_default();
                fail(&mut result, work, key, persona, e.message);
            }
        }
        return Ok(result);
    }
    let mut framed: Vec<FramedAnswer> = Vec::new();
    // Runde 30c: Antworten auf die Nachricht (für das Relay) und wer schon dran war.
    let mut answered: Vec<FramedAnswer> = Vec::new();
    let mut called: Vec<String> = addressed.clone();
    for handle in addressed {
        if write {
            let order = base(&handle, Mode::Propose, OrderFrame { as_order: true, ..Default::default() });
            match write_order(state, &order, message, progress, &mut result).await {
                Ok(()) => {}
                Err(e) => fail(&mut result, work, key, handle, e.message),
            }
            continue;
        }
        // Runde 22: Beim RFC prüft jede Persona erneut und sieht ihre eigenen offenen Befunde des Abschnitts. Fehler
        // 0.9.0 (Kritik-Zyklus): auch eine direkt angesprochene Persona („@nathan prüf den Vorschlag“) – sonst meldete
        // der Kritiker am Vorschlag seine alten Befunde erneut, statt sie nachzuprüfen.
        // Runde 36 (Hinweis 0.9.0): nur beim RFC und bei einem Prüfauftrag ([`is_check_order`]). Eine schlichte Frage
        // an die Persona zieht ihre offenen Befunde nicht mehr in den Kontext.
        let recheck = rfc || is_check_order(message);
        let order = base(&handle, Mode::Chat, OrderFrame { recheck, ..OrderFrame::default() });
        match run(state, &order, progress).await {
            Ok(outcome) => {
                let answer = FramedAnswer {
                    persona: outcome.persona.clone(),
                    post: outcome.post.as_ref().map(|p| p.id),
                    text: outcome.text.clone(),
                    findings: outcome.findings.clone(),
                };
                if rfc {
                    framed.push(answer);
                } else {
                    answered.push(answer);
                }
                keep(&mut result, outcome);
            }
            Err(e) => fail(&mut result, work, key, handle, e.message),
        }
    }
    // Runde 30c: eine Relay-Stufe (nicht beim RFC, nicht beim Schreibauftrag). Runde 23: der einzige Relay-Weg des
    // Abschnitts-Chats – Anrede am Zeilenanfang für alle, mit `relay` (MCP) dazu jede Erwähnung.
    if !rfc && !write {
        for (target, from) in relay_targets(&answered, &rows, &mut called, relay) {
            result.relayed.push(target.clone());
            let post_id = from.post.unwrap_or_default();
            let frame = OrderFrame { relay: Some(from.clone()), ..Default::default() };
            let mut order = base(&target, Mode::Chat, frame);
            order.text = relay_text(&from.persona, post_id, message);
            order.asker = Some(from.persona.clone());
            order.links =
                PostLinks { reply_to: Some(post_id), repeat_of: None, relay_from: Some(from.persona.clone()) };
            match run(state, &order, progress).await {
                Ok(outcome) => keep(&mut result, outcome),
                Err(e) => fail(&mut result, work, key, target, e.message),
            }
        }
    }
    if let Some(lector) = lector_last {
        let frame = OrderFrame { as_order: false, rfc_answers: framed, rfc_summary: true, relay: None, recheck: true };
        let lector_order = base(&lector, Mode::Chat, frame);
        match run(state, &lector_order, progress).await {
            Ok(outcome) => {
                // Runde 30b: „Mein Vorschlag:“ des Lektors als ein Sammel-Vorschlag.
                match super::solutions::rfc_proposal(state, &lector_order, &outcome, progress).await {
                    Ok(Some(p)) => result.proposals.push(p),
                    Ok(None) => {}
                    Err(e) => result.notes.push(e.message),
                }
                keep(&mut result, outcome);
            }
            Err(e) => fail(&mut result, work, key, lector, e.message),
        }
    }
    Ok(result)
}

/// Wen die Relay-Stufe ruft (Runde 23: ein Weg für die Oberfläche und MCP, vorher zusätzlich `relay_section` in
/// `mcp/tools.rs`): je Antwort auf die Nachricht (`answered`, nur gespeicherte) erst die Anreden am Zeilenanfang
/// ([`line_addresses`], zusammen höchstens [`MAX_RELAY_ANSWERS`]), mit `mentions_too` (MCP `chat`) danach jede weitere
/// Erwähnung ([`mentions`]), solange insgesamt höchstens [`MAX_CHAT_PERSONAS`] Personas gerufen sind. Niemand wird
/// zweimal gerufen (`called` = wer schon dran war; die Gewählten kommen dazu), keine Persona ruft sich selbst.
/// Relay-Antworten rufen niemanden mehr – sie stehen nicht in `answered`.
pub fn relay_targets(
    answered: &[FramedAnswer],
    personas: &[PersonaRow],
    called: &mut Vec<String>,
    mentions_too: bool,
) -> Vec<(String, FramedAnswer)> {
    let mut queue: Vec<(String, FramedAnswer)> = Vec::new();
    let mut direct_count = 0usize;
    for answer in answered {
        if answer.post.is_none() {
            continue;
        }
        let direct = line_addresses(&answer.text, personas);
        let mut candidates: Vec<(String, bool)> = direct.iter().map(|h| (h.clone(), true)).collect();
        if mentions_too {
            for m in mentions(&answer.text, personas) {
                if !direct.contains(&m) {
                    candidates.push((m, false));
                }
            }
        }
        for (target, is_direct) in candidates {
            if target == answer.persona || called.contains(&target) {
                continue;
            }
            let allowed = if is_direct { direct_count < MAX_RELAY_ANSWERS } else { called.len() < MAX_CHAT_PERSONAS };
            if !allowed {
                continue;
            }
            if is_direct {
                direct_count += 1;
            }
            called.push(target.clone());
            queue.push((target, answer.clone()));
        }
    }
    queue
}

/// Fehler einer Persona ins Ergebnis und in die Live-Anzeige ([`crate::chat_live::record_failure`]).
fn fail(result: &mut ChatOutcome, work: WorkId, key: i64, persona: String, message: String) {
    crate::chat_live::record_failure(work, key, &persona, &message);
    result.errors.push(ChatError { persona, message });
}

/// Antwort einer Persona ins Ergebnis übernehmen.
fn keep(result: &mut ChatOutcome, outcome: Outcome) {
    if let Some(post) = &outcome.post {
        result.posts.push(post.clone());
    }
    result.findings.extend(&outcome.findings);
    result.answers.push(outcome);
}

/// Schreibauftrag ausführen: Vorschlag des Abschnitts (nie übernommen), kurzer Beitrag der Persona, Statusvorschläge
/// „übernommen in Fassung n“ für die in der **Nachricht** genannten Befunde (`[B<id>]`; seit Runde 23 nicht mehr aus
/// dem Text des Vorschlags).
async fn write_order(
    state: &AppState,
    order: &Order,
    message: &str,
    progress: Option<&UnboundedSender<Progress>>,
    result: &mut ChatOutcome,
) -> Result<(), RpcError> {
    let outcome = run(state, order, progress).await?;
    let Some(rev) = outcome.rev else {
        return Err(RpcError::new(crate::rpc::INTERNAL, "Der Vorschlag hat keine Fassungsnummer."));
    };
    let handle = outcome.persona.clone();
    let body = order_note(rev);
    let saved = {
        let _held = state.write_lock.lock().await;
        let seq = state.store.node_seq(order.work, order.key).await?;
        let id = state
            .store
            .add_post(order.work, seq, &post(&handle, &body, vec![format!("Vorschlag Fassung {rev}")]))
            .await?;
        SavedPost { id, author: handle.clone(), body }
    };
    // Runde 23: Verweise nur aus der Nachricht des Auftraggebers. `[B<id>]` im Modelltext des Vorschlags erzeugt
    // keinen Statusvorschlag mehr – das Modell könnte sonst Befunde als „übernommen“ melden, die der Auftrag nie
    // nannte (auch über eingeschleusten Text im Abschnitt).
    let refs = finding_refs(message);
    let by = IdeaOrigin::Persona { handle: handle.clone() };
    for id in refs {
        // Die Fassungsnummer gilt nur im Abschnitt des Auftrags: ein Befund aus einem anderen Abschnitt bekäme sonst
        // eine fremde Fassung mit derselben Nummer als „übernommen“ vorgeschlagen.
        match state.store.finding_get(order.work, id).await {
            Ok(finding) if finding.key == Some(order.key) => {}
            Ok(_) => {
                result.notes.push(format!(
                    "[B{id}]: Der Befund gehört nicht zu diesem Abschnitt; kein Statusvorschlag zu Fassung {rev}."
                ));
                continue;
            }
            Err(e) => {
                result.notes.push(format!("[B{id}]: {}", RpcError::from(e).message));
                continue;
            }
        }
        let change = baruch_store::StatusChange::Adopted { rev };
        let note = format!("Vorschlag Fassung {rev} aus dem Chat");
        if let Err(e) = crate::findings::rpc::propose(state, order.work, id, &change, &by, &note).await {
            result.notes.push(format!("[B{id}]: {}", e.message));
        }
    }
    result.proposals.push(ChatProposal { persona: handle, rev, url: outcome.url.clone() });
    result.posts.push(saved);
    result.answers.push(outcome);
    Ok(())
}

/// Antwort einer Chat-Nachricht als JSON; `compressed` (Runde 15) ist die erste Komprimierung einer Antwort
/// (`null`, wenn keine Persona gestückelt sah). Runde 28: `rfc`, `proposals` (`{persona, rev, url}`), `findings`
/// (ids) und `notes`.
pub fn chat_json(outcome: &ChatOutcome) -> Value {
    let compressed = outcome.answers.iter().find_map(|a| a.compressed.as_ref());
    let mut value = json!({ "posts": outcome.posts, "errors": outcome.errors, "url": outcome.url,
            "truncated": outcome.answers.iter().any(|a| a.truncated), "cut": outcome.answers.iter().any(|a| a.cut),
            "compressed": compressed, "rfc": outcome.rfc, "proposals": outcome.proposals,
            "findings": outcome.findings, "notes": outcome.notes });
    add_done(&mut value, &outcome.answers);
    value
}

/// Runde 23: Rückmeldungen „Erledigt: Befund n“ aus den Antworten in die Ausgabe (Namen wie bei `persona.ask`):
/// `done_proposals` (Befundnummern mit Statusvorschlag „erledigt“, ohne Doppelte), `done_notes` (übergangene
/// Rückmeldungen mit Grund) und `all_done` (mindestens eine Persona sagt, alle ihre Befunde scheinen erledigt). Jedes
/// Feld nur, wenn es nicht leer bzw. `true` ist.
pub fn add_done(value: &mut Value, answers: &[Outcome]) {
    let mut proposals: Vec<i64> = Vec::new();
    for id in answers.iter().flat_map(|a| a.done_proposals.iter().copied()) {
        if !proposals.contains(&id) {
            proposals.push(id);
        }
    }
    let notes: Vec<&String> = answers.iter().flat_map(|a| a.done_notes.iter()).collect();
    if !proposals.is_empty() {
        value["done_proposals"] = json!(proposals);
    }
    if !notes.is_empty() {
        value["done_notes"] = json!(notes);
    }
    if answers.iter().any(|a| a.all_done) {
        value["all_done"] = json!(true);
    }
}

/// Chat-Nachricht des Menschen aus RPC-Parametern ausführen.
pub async fn run_chat(
    state: &AppState,
    params: &Value,
    progress: Option<&UnboundedSender<Progress>>,
) -> Result<ChatOutcome, RpcError> {
    let (work, key, rev) = super::rpc::address(state, params).await?;
    let message = super::rpc::text(params, "message")?.unwrap_or_default();
    // Runde 28: `as_order` (Vorgabe false) – die Nachricht als Auftrag; aus Formularen auch „1“ bzw. „on“.
    let as_order = match params.get("as_order") {
        Some(Value::Bool(b)) => *b,
        Some(Value::String(s)) => matches!(s.trim(), "1" | "on" | "true"),
        _ => false,
    };
    let basis = basis_param(params)?;
    let order = ChatOrder {
        work,
        key,
        message: &message,
        rev,
        author: crate::mirror::HUMAN_NAME,
        extra: None,
        web: true,
        as_order,
        basis,
        relay: false,
    };
    chat(state, &order, progress).await
}

/// Runde 30c: Prüfgrundlage aus RPC bzw. Formular (`basis` = `shown` | `current` | `<n>`, Zahl oder Text); fehlt oder
/// leer → die gezeigte.
pub fn basis_param(params: &Value) -> Result<super::basis::BasisChoice, RpcError> {
    let text = match params.get("basis") {
        None | Some(Value::Null) => return Ok(super::basis::BasisChoice::Shown),
        Some(Value::Number(n)) => n.to_string(),
        Some(Value::String(s)) if s.trim().is_empty() => return Ok(super::basis::BasisChoice::Shown),
        Some(Value::String(s)) => s.clone(),
        Some(_) => return Err(RpcError::new(INVALID_PARAMS, "basis: shown, current oder eine Fassungsnummer")),
    };
    super::basis::BasisChoice::parse(&text)
        .ok_or_else(|| RpcError::new(INVALID_PARAMS, "basis: shown, current oder eine Fassungsnummer"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(handle: &str, active: bool) -> PersonaRow {
        PersonaRow {
            handle: handle.into(),
            name: handle.trim_start_matches('@').into(),
            role: String::new(),
            model: "m".into(),
            context_length: 1,
            active,
            color: "#000000".into(),
            chosen: false,
            prompt: None,
        }
    }

    #[test]
    fn mentions_take_active_personas_once_and_at_most_three() {
        let rows = [
            row("@lektor", true),
            row("@exeget", true),
            row("@nathan", false),
            row("@hoerer", true),
            row("@schrift", true),
        ];
        assert_eq!(mentions("@Lektor und @lektor, bitte", &rows), vec!["@lektor"], "ohne Doppelte, Groß/klein egal");
        assert_eq!(
            mentions("@nathan? @unbekannt", &rows),
            Vec::<String>::new(),
            "inaktive und unbekannte zählen nicht"
        );
        assert_eq!(
            mentions("@schrift @hoerer @exeget @lektor", &rows),
            vec!["@schrift", "@hoerer", "@exeget"],
            "höchstens drei, in der Reihenfolge der Nachricht"
        );
        assert_eq!(mentions("mail@lektor.de", &rows), vec!["@lektor"], "auch mitten im Wort – wie im Chat üblich");
    }

    #[test]
    fn rfc_means_rfc_as_first_word_or_the_mention_alle() {
        for yes in ["RFC für den Abschnitt", "rfc: bitte alle", "  RFC", "Was meint ihr, @alle?", "@Alle bitte"] {
            assert!(is_rfc(yes), "{yes:?}: RFC am Anfang bzw. @alle ruft alle (Fund am echten Buch, Beitrag 41)");
        }
        for no in ["Ist das ein RFC?", "RFCs sind gut", "@allerlei Fragen", "@lektor alle Absätze", "alle zusammen"] {
            assert!(!is_rfc(no), "{no:?}: RFC nur als erstes Wort, @alle nur als ganze Erwähnung");
        }
    }

    #[test]
    fn a_write_order_is_an_imperative_writing_verb_after_mentions_and_bitte() {
        for yes in [
            "@lektor Schreibe ein Vorwort",
            "@lektor, bitte formuliere den Schluss neu",
            "Bitte überarbeite den zweiten Absatz",
            "@exeget kürze den Abschnitt auf die Hälfte",
            "Erstelle eine Fassung ohne Fremdwörter",
        ] {
            assert!(is_write_order(yes), "{yes:?}: Imperativ eines Schreibverbs nach Erwähnung/„bitte“");
        }
        for no in [
            "@lektor Wie wirkt der Anfang?",
            "Kannst du das schreiben?",
            "So falle ich mit der Tür ins Haus",
            "@lektor",
            "Schriftlich ist das schwierig",
            "Ergänzend: der Schluss trägt",
        ] {
            assert!(!is_write_order(no), "{no:?}: Frage, Bemerkung oder anderes Wort – kein Auftrag");
        }
    }

    #[test]
    fn finding_references_are_read_once_in_order() {
        assert_eq!(finding_refs("Setzt [B12] und [B3] um, dazu nochmals [B12]."), vec![12, 3], "ohne Doppelte");
        assert_eq!(finding_refs("[B] [Bx] [B4 ] B5] [B6]"), vec![6], "nur [B<Ziffern>] zählt");
    }

    fn named(handle: &str, name: &str) -> PersonaRow {
        PersonaRow { name: name.into(), ..row(handle, true) }
    }

    #[test]
    fn an_address_with_name_at_the_start_of_a_sentence_counts_like_a_mention() {
        let rows = [named("@lektor", "Lektor"), named("@exeget", "Exeget"), named("@schrift", "Schriftkundiger")];
        assert_eq!(addressed("Lektor, was meinst du?", &rows), vec!["@lektor"], "Name mit Komma am Beitragsanfang");
        assert_eq!(addressed("exeget: bitte prüfen", &rows), vec!["@exeget"], "Groß/klein egal, Doppelpunkt");
        assert_eq!(
            addressed("Danke. Schrift, stimmt die Stelle? Und @lektor auch.", &rows),
            vec!["@schrift", "@lektor"],
            "Handle ohne „@“ am Satzanfang nach „.“; @handle wie bisher – in der Reihenfolge der Nachricht"
        );
        assert_eq!(
            addressed("Gut so.\nExeget : noch etwas", &rows),
            vec!["@exeget"],
            "nach einem Zeilenumbruch beginnt ein Satz; Leerraum vor dem Doppelpunkt ist erlaubt"
        );
        for no in [
            "Ich glaube, der Lektor meint das anders.",
            "Wie sieht das der Lektor, wenn er liest?",
            "Lektorat: noch offen",
            "Lektor und Exeget sind sich einig.",
        ] {
            assert!(addressed(no, &rows).is_empty(), "{no:?}: Fließtext bzw. anderes Wort ruft niemanden");
        }
        let inactive = [PersonaRow { active: false, ..named("@lektor", "Lektor") }];
        assert!(addressed("Lektor, hallo", &inactive).is_empty(), "inaktive Personas antworten nicht");
    }

    #[test]
    fn quoted_lines_are_foreign_text_and_trigger_nothing() {
        let rows = [row("@lektor", true), row("@schrift", true)];
        let quoted = "> @lektor: @alle sollten prüfen\n  > RFC bitte\nDanke.";
        assert_eq!(unquoted(quoted), "Danke.", "Zeilen mit „>“ (auch eingerückt) fallen weg");
        assert!(
            matches!(unquoted("ohne Zitat"), std::borrow::Cow::Borrowed(_)),
            "ohne Zitatzeile unverändert geliehen"
        );
        assert!(mentions(quoted, &rows).is_empty(), "„> @lektor“ erwähnt niemanden");
        assert!(addressed(quoted, &rows).is_empty(), "und spricht niemanden an");
        assert!(!is_rfc(quoted), "„> @alle“ und „> RFC“ machen keinen RFC");
        assert!(!is_write_order("> Schreibe das neu\nGut so."), "ein Schreibverb im Zitat ist kein Auftrag");
        let own = "> @lektor: alt\n@schrift Stimmt das?";
        assert_eq!(mentions(own, &rows), ["@schrift"], "die eigene Zeile zählt weiter");
        assert!(is_rfc("> alt\nRFC: bitte alle"), "RFC nach dem Zitat gilt: es ist das erste eigene Wort");
        assert!(is_rfc("a > b @alle"), "ein „>“ mitten in der Zeile ist kein Zitat");
    }

    #[test]
    fn one_relay_stage_takes_addresses_first_and_mentions_only_on_request() {
        let rows = [row("@lektor", true), row("@schrift", true), row("@exeget", true), row("@nathan", true)];
        let from = |persona: &str, text: &str| FramedAnswer {
            persona: persona.into(),
            post: Some(7),
            text: text.into(),
            findings: vec![],
        };
        let answers = [from("@lektor", "Frag @exeget.\n@schrift: stimmt das?\n@lektor: ich selbst")];
        let mut called = vec!["@lektor".to_owned()];
        let targets: Vec<String> =
            relay_targets(&answers, &rows, &mut called, false).into_iter().map(|t| t.0).collect();
        assert_eq!(targets, ["@schrift"], "ohne Erwähnungen (Oberfläche): nur die Anrede, nie an sich selbst");
        let mut called = vec!["@lektor".to_owned()];
        let targets: Vec<String> = relay_targets(&answers, &rows, &mut called, true).into_iter().map(|t| t.0).collect();
        assert_eq!(targets, ["@schrift", "@exeget"], "mit Erwähnungen (MCP): Anrede zuerst, dann die Erwähnung");
        assert_eq!(called, ["@lektor", "@schrift", "@exeget"], "wer gewählt ist, gilt als gerufen");
        let more = [from("@lektor", "@nathan soll auch")];
        assert!(
            relay_targets(&more, &rows, &mut called, true).is_empty(),
            "Erwähnungen nur, solange höchstens drei Personas gerufen sind"
        );
        let unsaved = [FramedAnswer { post: None, ..from("@lektor", "@schrift: hallo") }];
        assert!(relay_targets(&unsaved, &rows, &mut vec![], true).is_empty(), "ohne gespeicherten Beitrag kein Relay");
    }

    #[test]
    fn relay_targets_are_addresses_at_the_start_of_a_line_only() {
        let rows = [named("@lektor", "Lektor"), named("@exeget", "Exeget"), named("@schrift", "Schrift")];
        assert_eq!(
            line_addresses("Was trägt: gut.\n@schrift: prüfe Joh 3,16.\n- **Lektor:** einverstanden?", &rows),
            vec!["@schrift", "@lektor"],
            "„@handle:“ und Anrede am Zeilenanfang, auch hinter Aufzählung und fett"
        );
        for no in [
            "Wie @schrift schon sagte, passt das.",
            "> @lektor: zitiert",
            "Der Lektor: meint etwas",
            "@schrift hat recht.",
        ] {
            assert!(line_addresses(no, &rows).is_empty(), "{no:?}: keine Anrede am Zeilenanfang");
        }
    }

    #[test]
    fn the_relay_task_is_built_by_baruch_and_names_the_post() {
        let text = relay_text("@lektor", 61, "Wie wirkt der Anfang?");
        assert!(text.starts_with("@lektor hat dich in Beitrag 61 direkt angesprochen."), "{text}");
        assert!(text.ends_with("Wie wirkt der Anfang?"), "die Nachricht des Autors bleibt der Auftrag: {text}");
        assert!(
            relay_text("@x", 1, &"a".repeat(5000)).chars().count() <= crate::assist::MAX_QUESTION_CHARS,
            "nie länger als ein Auftrag sein darf"
        );
    }

    #[test]
    fn the_basis_parameter_comes_as_word_number_or_form_text() {
        use crate::assist::basis::BasisChoice;
        assert_eq!(basis_param(&json!({})).unwrap(), BasisChoice::Shown, "ohne Angabe die gezeigte Fassung");
        assert_eq!(basis_param(&json!({ "basis": "" })).unwrap(), BasisChoice::Shown, "leeres Formularfeld ebenso");
        assert_eq!(basis_param(&json!({ "basis": "current" })).unwrap(), BasisChoice::Current, "aktuelle Fassung");
        assert_eq!(basis_param(&json!({ "basis": 3 })).unwrap(), BasisChoice::Rev(3), "Zahl aus JSON");
        assert_eq!(basis_param(&json!({ "basis": "2" })).unwrap(), BasisChoice::Rev(2), "Ziffern aus dem Formular");
        assert!(basis_param(&json!({ "basis": "neueste" })).is_err(), "Unbekanntes wird abgelehnt, nicht geraten");
    }

    #[test]
    fn the_note_of_a_write_order_carries_no_finding_mark() {
        let note = order_note(4);
        assert!(note.contains("Fassung 4") && note.contains("nur der Mensch"), "Fassung und Regel genannt: {note}");
        for mark in ["Kritisch:", "Vorschlag:", "Hinweis:", "Lob:"] {
            assert!(!note.starts_with(mark), "{mark}: der Hinweis ergibt beim Zerlegen keinen Befund");
        }
    }
}
