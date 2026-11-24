// SPDX-License-Identifier: AGPL-3.0-or-later
//! Farbe je Persona (Migration `0010_persona_color.sql`, Vertrag Runde 7, „proposals“).
//!
//! Die Spalte `persona.color` ist `NULL` (Vorgabe) oder `#rrggbb` in Kleinbuchstaben (CHECK). Die
//! Vorgabe kommt aus [`PERSONA_PALETTE`] nach der Reihenfolge der Personas (`position`, `id`), die
//! n-te Persona bekommt die Farbe `n mod 8`. Autoren ohne Persona-Eintrag (z. B. `@mcp:…`) zeigt die
//! Oberfläche in [`FALLBACK_COLOR`]. Personas sind global und kein Buchtext: Eine Farbänderung ist
//! eine Transaktion (`BEGIN IMMEDIATE`), aber kein Commit im Git-Spiegel.
//!
//! Runde 10 (personas): eigener Rollen-Prompt je Persona (`persona.prompt`, Migration 0015; `NULL` =
//! Vorgabe aus dem Code des Servers) und die Wahl des Lektors in `store_meta` (Schlüssel
//! [`LECTOR_META_KEY`], Wert `auto` oder ein Handle; fehlt die Zeile, gilt fest `@lektor`).

use baruch_core::LectorChoice;
use sqlx::SqliteConnection;

use crate::{Store, StoreError};

/// Vorgabefarben in Reihenfolge der Personas: gut unterscheidbar (Blau, Orange, Grün, Violett,
/// Magenta, Petrol, Ocker, Braun), kleingeschrieben `#rrggbb`.
pub const PERSONA_PALETTE: [&str; 8] =
    ["#1c7ed6", "#e8590c", "#2f9e44", "#7048e8", "#c2255c", "#0c8599", "#e0a800", "#8b5a2b"];

/// Farbe für Autoren ohne Persona-Eintrag (z. B. MCP-Aufrufer `@mcp:<client>`): Grau.
pub const FALLBACK_COLOR: &str = "#868e96";

/// Wirksame Farbe einer Persona.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PersonaColor {
    pub handle: String,
    pub name: String,
    /// Wirksame Farbe `#rrggbb` (gewählt oder Vorgabe aus der Palette).
    pub color: String,
    /// `true`: vom Menschen gewählt; `false`: Vorgabe aus [`PERSONA_PALETTE`].
    pub chosen: bool,
}

/// `#rgb` oder `#rrggbb` (Groß- oder Kleinbuchstaben, Rand-Leerraum erlaubt) → `#rrggbb` klein;
/// alles andere → `None`.
pub fn valid_color(input: &str) -> Option<String> {
    let hex = input.trim().strip_prefix('#')?;
    if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let full: String = match hex.len() {
        3 => hex.chars().flat_map(|c| [c, c]).collect(),
        6 => hex.to_owned(),
        _ => return None,
    };
    Some(format!("#{}", full.to_ascii_lowercase()))
}

/// Alle Personas mit wirksamer Farbe, nach `position`, `id`.
async fn colors(conn: &mut SqliteConnection) -> Result<Vec<PersonaColor>, StoreError> {
    let rows: Vec<(String, String, Option<String>)> =
        sqlx::query_as("SELECT handle, name, color FROM persona ORDER BY position, id").fetch_all(&mut *conn).await?;
    Ok(rows
        .into_iter()
        .enumerate()
        .map(|(index, (handle, name, color))| {
            // Die Spalte prüft das Format (CHECK); valid_color ist die zweite Wache.
            let chosen = color.as_deref().and_then(valid_color);
            PersonaColor {
                handle,
                name,
                chosen: chosen.is_some(),
                color: chosen.unwrap_or_else(|| PERSONA_PALETTE[index % PERSONA_PALETTE.len()].to_owned()),
            }
        })
        .collect())
}

/// Längste Rolle bzw. längstes Modell in Zeichen (nach dem Trimmen, Runde 9).
pub const MAX_PERSONA_TEXT: usize = 200;
/// Größte Kontextlänge in Tokens (Runde 9); die kleinste ist 1.
pub const MAX_CONTEXT_LENGTH: u32 = 10_000_000;

/// Längster eigener Rollen-Prompt in Zeichen (nach dem Trimmen, Runde 10; CHECK in Migration 0015).
pub const MAX_PROMPT_CHARS: usize = 8000;

/// Schlüssel der Lektor-Wahl in `store_meta` (Runde 10): Wert `auto` oder ein Handle.
pub const LECTOR_META_KEY: &str = "lector";

/// Schlüssel der Einstellung „Chat nur Abschnitt“ in `store_meta` (Runde 16a): `chapter` oder `section`.
pub const CHAT_SCOPE_META_KEY: &str = "chat_scope";

/// Was der Chat einer Persona zum gezeigten Abschnitt mitgibt (Runde 16a): das ganze Kapitel (Vorgabe, wie
/// bisher) oder nur den Abschnitt selbst (dazu Titel und Gliederung des Werks).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatScope {
    #[default]
    Chapter,
    Section,
}

impl ChatScope {
    /// `chapter` | `section`.
    pub fn word(self) -> &'static str {
        match self {
            ChatScope::Chapter => "chapter",
            ChatScope::Section => "section",
        }
    }

    /// Aus `chapter` | `section` (getrimmt); sonst `None`.
    pub fn parse(value: &str) -> Option<ChatScope> {
        match value.trim() {
            "chapter" => Some(ChatScope::Chapter),
            "section" => Some(ChatScope::Section),
            _ => None,
        }
    }
}

/// Eine Persona mit allen Feldern aus der Tabelle `persona` (Runde 9, Einstellungen „Assistenten“).
/// Personas sind global (für alle Werke); `color`/`chosen` wie [`PersonaColor`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PersonaRow {
    pub handle: String,
    pub name: String,
    pub role: String,
    pub model: String,
    /// Kontextlänge in Tokens.
    pub context_length: u32,
    pub active: bool,
    /// Wirksame Farbe `#rrggbb` (gewählt oder Vorgabe aus der Palette).
    pub color: String,
    /// `true`: vom Menschen gewählt.
    pub chosen: bool,
    /// Eigener Rollen-Prompt (Runde 10); `None` = Vorgabe aus dem Code.
    pub prompt: Option<String>,
}

/// Änderung einer Persona ([`Store::update_persona`]); `None` = bleibt. Rolle und Modell werden
/// getrimmt und müssen 1–[`MAX_PERSONA_TEXT`] Zeichen haben, die Kontextlänge 1–[`MAX_CONTEXT_LENGTH`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PersonaEdit {
    pub role: Option<String>,
    pub model: Option<String>,
    pub context_length: Option<u32>,
    pub active: Option<bool>,
}

/// Längster Name einer Persona in Zeichen (nach dem Trimmen, Runde 11).
const MAX_PERSONA_NAME: usize = 80;

impl PersonaEdit {
    /// Name einer Persona prüfen (Runde 11, Einstellungen → Assistenten): getrimmt 1–80 Zeichen, in
    /// einer Zeile. `Ok` = der getrimmte Name, `Err` = deutsche Meldung.
    pub fn check_name(name: &str) -> Result<String, &'static str> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Der Name darf nicht leer sein.");
        }
        if name.chars().count() > MAX_PERSONA_NAME {
            return Err("Der Name darf höchstens 80 Zeichen haben.");
        }
        if name.contains(['\n', '\r']) {
            return Err("Der Name muss in eine Zeile passen.");
        }
        Ok(name.to_owned())
    }

    /// Prüft die Grenzen; `Err` nennt das erste Problem auf Deutsch (die Server-Schicht meldet es als
    /// -32602 bzw. 422, der Store lehnt dasselbe als zweite Wache mit `Refused` ab).
    pub fn check(&self) -> Result<(), &'static str> {
        for (value, empty, long) in [
            (&self.role, "Die Rolle darf nicht leer sein.", "Die Rolle darf höchstens 200 Zeichen haben."),
            (&self.model, "Das Modell darf nicht leer sein.", "Das Modell darf höchstens 200 Zeichen haben."),
        ] {
            if let Some(text) = value {
                let text = text.trim();
                if text.is_empty() {
                    return Err(empty);
                }
                if text.chars().count() > MAX_PERSONA_TEXT {
                    return Err(long);
                }
            }
        }
        if let Some(n) = self.context_length
            && !(1..=MAX_CONTEXT_LENGTH).contains(&n)
        {
            return Err("Die Kontextlänge muss zwischen 1 und 10000000 Tokens liegen.");
        }
        Ok(())
    }
}

type RawPersona = (String, String, String, String, i64, i64, Option<String>, Option<String>);

/// Alle Personas mit allen Feldern, nach `position`, `id`.
async fn rows(conn: &mut SqliteConnection) -> Result<Vec<PersonaRow>, StoreError> {
    let raw: Vec<RawPersona> = sqlx::query_as(
        "SELECT handle, name, role, model, context_length, active, color, prompt FROM persona ORDER BY position, id",
    )
    .fetch_all(&mut *conn)
    .await?;
    raw.into_iter()
        .enumerate()
        .map(|(index, (handle, name, role, model, context_length, active, color, prompt))| {
            let chosen = color.as_deref().and_then(valid_color);
            let context_length = u32::try_from(context_length)
                .map_err(|_| crate::broken(format!("context_length {context_length} der Persona {handle}")))?;
            Ok(PersonaRow {
                handle,
                name,
                role,
                model,
                context_length,
                active: active != 0,
                chosen: chosen.is_some(),
                color: chosen.unwrap_or_else(|| PERSONA_PALETTE[index % PERSONA_PALETTE.len()].to_owned()),
                prompt,
            })
        })
        .collect()
}

/// Eigener Rollen-Prompt geprüft: `None` = Vorgabe; sonst getrimmt 1–[`MAX_PROMPT_CHARS`] Zeichen
/// mit `\n` als Zeilenende.
fn prompt_value(prompt: Option<&str>) -> Result<Option<String>, StoreError> {
    match prompt.map(str::trim) {
        None => Ok(None),
        Some("") => Err(StoreError::Refused("Der Rollen-Prompt darf nicht leer sein (Vorgabe: zurücksetzen).")),
        Some(text) if text.chars().count() > MAX_PROMPT_CHARS => {
            Err(StoreError::Refused("Der Rollen-Prompt darf höchstens 8000 Zeichen haben."))
        }
        Some(text) => Ok(Some(text.replace("\r\n", "\n"))),
    }
}

/// Lektor-Wahl aus `store_meta` lesen: `auto` → [`LectorChoice::Auto`], ein anderer Wert → fest diese
/// Persona, keine Zeile → Vorgabe (fest `@lektor`).
pub(crate) async fn read_lector_choice(conn: &mut SqliteConnection) -> Result<LectorChoice, StoreError> {
    let value: Option<String> = sqlx::query_scalar("SELECT value FROM store_meta WHERE key = ?")
        .bind(LECTOR_META_KEY)
        .fetch_optional(&mut *conn)
        .await?;
    Ok(match value.as_deref().map(str::trim) {
        None | Some("") => LectorChoice::default(),
        Some("auto") => LectorChoice::Auto,
        Some(handle) => LectorChoice::Fixed(handle.to_owned()),
    })
}

/// Handle in der Tabelle: wie eingegeben oder mit vorangestelltem `@`.
async fn find_handle(conn: &mut SqliteConnection, handle: &str) -> Result<Option<String>, StoreError> {
    let wanted = handle.trim();
    let mut candidates = vec![wanted.to_owned()];
    if !wanted.starts_with('@') {
        candidates.push(format!("@{wanted}"));
    }
    for candidate in candidates.iter().filter(|c| !c.is_empty()) {
        let hit: Option<String> = sqlx::query_scalar("SELECT handle FROM persona WHERE handle = ?")
            .bind(candidate)
            .fetch_optional(&mut *conn)
            .await?;
        if hit.is_some() {
            return Ok(hit);
        }
    }
    Ok(None)
}

impl Store {
    /// Alle Personas mit allen Feldern (Rolle, Modell, Kontext, aktiv, Farbe) in der Reihenfolge
    /// `position`, `id` – ohne Werk (Personas sind global, Runde 9).
    pub async fn persona_list(&self) -> Result<Vec<PersonaRow>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        rows(&mut conn).await
    }

    /// Rolle, Modell, Kontextlänge und aktiv einer Persona ändern, atomar (`BEGIN IMMEDIATE`).
    /// Unbekannte Persona → `UnknownPersona` (Handle auch ohne `@`); Grenzen verletzt
    /// ([`PersonaEdit::check`]) → `Refused`; in beiden Fällen bleibt alles, wie es ist. Kein Commit im
    /// Spiegel (Personas sind kein Buchtext).
    pub async fn update_persona(&self, handle: &str, edit: &PersonaEdit) -> Result<PersonaRow, StoreError> {
        edit.check().map_err(StoreError::Refused)?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let Some(handle) = find_handle(&mut tx, handle).await? else {
            return Err(StoreError::UnknownPersona(handle.trim().to_owned()));
        };
        sqlx::query(
            "UPDATE persona SET role = COALESCE(?, role), model = COALESCE(?, model), \
             context_length = COALESCE(?, context_length), active = COALESCE(?, active) WHERE handle = ?",
        )
        .bind(edit.role.as_deref().map(str::trim))
        .bind(edit.model.as_deref().map(str::trim))
        .bind(edit.context_length.map(i64::from))
        .bind(edit.active.map(i64::from))
        .bind(&handle)
        .execute(&mut *tx)
        .await?;
        let all = rows(&mut tx).await?;
        tx.commit().await?;
        all.into_iter()
            .find(|p| p.handle == handle)
            .ok_or_else(|| crate::broken(format!("Persona {handle} nach dem Ändern verschwunden")))
    }

    /// Alles einer Persona auf einmal speichern (Runde 11, ein Knopf „Speichern“ je Persona): Name
    /// (`Some`, [`PersonaEdit::check_name`]), Rolle, Modell, Kontext und aktiv ([`PersonaEdit`]), Farbe
    /// (`Some(Some(..))` setzen, `Some(None)` Vorgabe) und Rollen-Prompt (`Some(Some(..))` eigener,
    /// `Some(None)` Vorgabe); `None` = bleibt. Erst wird alles geprüft, dann in **einer** Transaktion
    /// geschrieben: Ein ungültiger Wert ändert nichts. Unbekannte Persona → `UnknownPersona`, ungültige
    /// Farbe → `Color`, sonst `Refused` mit deutscher Meldung. Kein Commit im Spiegel.
    pub async fn save_persona(
        &self,
        handle: &str,
        name: Option<&str>,
        edit: &PersonaEdit,
        color: Option<Option<&str>>,
        prompt: Option<Option<&str>>,
    ) -> Result<PersonaRow, StoreError> {
        let name = name.map(PersonaEdit::check_name).transpose().map_err(StoreError::Refused)?;
        edit.check().map_err(StoreError::Refused)?;
        let color = match color {
            None => None,
            Some(None) => Some(None),
            Some(Some(input)) => Some(Some(
                valid_color(input).ok_or_else(|| StoreError::Color("Farbe als #rrggbb angeben.".to_owned()))?,
            )),
        };
        let prompt = prompt.map(prompt_value).transpose()?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let Some(handle) = find_handle(&mut tx, handle).await? else {
            return Err(StoreError::UnknownPersona(handle.trim().to_owned()));
        };
        sqlx::query(
            "UPDATE persona SET name = COALESCE(?, name), role = COALESCE(?, role), model = COALESCE(?, model), \
             context_length = COALESCE(?, context_length), active = COALESCE(?, active) WHERE handle = ?",
        )
        .bind(name.as_deref())
        .bind(edit.role.as_deref().map(str::trim))
        .bind(edit.model.as_deref().map(str::trim))
        .bind(edit.context_length.map(i64::from))
        .bind(edit.active.map(i64::from))
        .bind(&handle)
        .execute(&mut *tx)
        .await?;
        if let Some(color) = &color {
            sqlx::query("UPDATE persona SET color = ? WHERE handle = ?")
                .bind(color.as_deref())
                .bind(&handle)
                .execute(&mut *tx)
                .await?;
        }
        if let Some(prompt) = &prompt {
            sqlx::query("UPDATE persona SET prompt = ? WHERE handle = ?")
                .bind(prompt.as_deref())
                .bind(&handle)
                .execute(&mut *tx)
                .await?;
        }
        let all = rows(&mut tx).await?;
        tx.commit().await?;
        all.into_iter()
            .find(|p| p.handle == handle)
            .ok_or_else(|| crate::broken(format!("Persona {handle} nach dem Speichern verschwunden")))
    }

    /// Wahl des Lektors (Runde 10): `store_meta.lector`, ohne Zeile fest `@lektor`.
    pub async fn lector_choice(&self) -> Result<LectorChoice, StoreError> {
        let mut conn = self.pool.acquire().await?;
        read_lector_choice(&mut conn).await
    }

    /// Wahl des Lektors setzen, atomar. `Fixed`: Handle mit oder ohne `@`, die Persona muss es geben
    /// (sonst `UnknownPersona`, nichts geändert); gespeichert wird der Handle aus der Tabelle. Ob sie
    /// aktiv ist, zählt erst beim Bestimmen des Lektors ([`LectorChoice::pick`]). Kein Commit im Spiegel.
    pub async fn set_lector_choice(&self, choice: &LectorChoice) -> Result<LectorChoice, StoreError> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let (value, stored) = match choice {
            LectorChoice::Auto => ("auto".to_owned(), LectorChoice::Auto),
            LectorChoice::Fixed(handle) => {
                let Some(found) = find_handle(&mut tx, handle).await? else {
                    return Err(StoreError::UnknownPersona(handle.trim().to_owned()));
                };
                (found.clone(), LectorChoice::Fixed(found))
            }
        };
        sqlx::query(
            "INSERT INTO store_meta (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(LECTOR_META_KEY)
        .bind(&value)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(stored)
    }

    /// Umfang des Chats (Runde 16a, `store_meta.chat_scope`): ohne Zeile oder mit unbekanntem Wert
    /// [`ChatScope::Chapter`] (wie bisher: das Kapitel des Abschnitts).
    pub async fn chat_scope(&self) -> Result<ChatScope, StoreError> {
        let value: Option<String> = sqlx::query_scalar("SELECT value FROM store_meta WHERE key = ?")
            .bind(CHAT_SCOPE_META_KEY)
            .fetch_optional(&self.pool)
            .await?;
        Ok(value.as_deref().and_then(ChatScope::parse).unwrap_or_default())
    }

    /// Umfang des Chats setzen (nur der Mensch; kein Commit im Spiegel, steht nur in SQLite).
    pub async fn set_chat_scope(&self, scope: ChatScope) -> Result<ChatScope, StoreError> {
        sqlx::query(
            "INSERT INTO store_meta (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        )
        .bind(CHAT_SCOPE_META_KEY)
        .bind(scope.word())
        .execute(&self.pool)
        .await?;
        Ok(scope)
    }

    /// Eigener Rollen-Prompt einer Persona (`None` = Vorgabe aus dem Code); unbekannt → `UnknownPersona`.
    pub async fn persona_prompt(&self, handle: &str) -> Result<Option<String>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        let Some(found) = find_handle(&mut conn, handle).await? else {
            return Err(StoreError::UnknownPersona(handle.trim().to_owned()));
        };
        Ok(sqlx::query_scalar("SELECT prompt FROM persona WHERE handle = ?").bind(found).fetch_one(&mut *conn).await?)
    }

    /// Eigenen Rollen-Prompt setzen (`Some`, getrimmt 1–[`MAX_PROMPT_CHARS`] Zeichen, sonst `Refused`)
    /// oder auf die Vorgabe zurücksetzen (`None`), atomar; unbekannte Persona → `UnknownPersona`. In
    /// jedem Fehlerfall bleibt alles, wie es ist. Kein Commit im Spiegel (kein Buchtext).
    pub async fn set_persona_prompt(&self, handle: &str, prompt: Option<&str>) -> Result<PersonaRow, StoreError> {
        let value = prompt_value(prompt)?;
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let Some(handle) = find_handle(&mut tx, handle).await? else {
            return Err(StoreError::UnknownPersona(handle.trim().to_owned()));
        };
        sqlx::query("UPDATE persona SET prompt = ? WHERE handle = ?")
            .bind(&value)
            .bind(&handle)
            .execute(&mut *tx)
            .await?;
        let all = rows(&mut tx).await?;
        tx.commit().await?;
        all.into_iter()
            .find(|p| p.handle == handle)
            .ok_or_else(|| crate::broken(format!("Persona {handle} nach dem Setzen des Prompts verschwunden")))
    }

    /// Alle Personas mit ihrer wirksamen Farbe in der Reihenfolge `position`, `id`.
    pub async fn persona_colors(&self) -> Result<Vec<PersonaColor>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        colors(&mut conn).await
    }

    /// Farbe einer Persona setzen (`Some`, `#rgb`/`#rrggbb`) oder auf die Vorgabe zurücksetzen
    /// (`None`), atomar. Unbekannte Persona → `UnknownPersona` (der Handle darf ohne `@` kommen),
    /// ungültige Farbe → `Color`; in beiden Fällen bleibt alles, wie es ist.
    pub async fn set_persona_color(&self, handle: &str, color: Option<&str>) -> Result<PersonaColor, StoreError> {
        let value = match color {
            Some(input) => {
                Some(valid_color(input).ok_or_else(|| StoreError::Color("Farbe als #rrggbb angeben.".to_owned()))?)
            }
            None => None,
        };
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let Some(handle) = find_handle(&mut tx, handle).await? else {
            return Err(StoreError::UnknownPersona(handle.trim().to_owned()));
        };
        sqlx::query("UPDATE persona SET color = ? WHERE handle = ?")
            .bind(&value)
            .bind(&handle)
            .execute(&mut *tx)
            .await?;
        let all = colors(&mut tx).await?;
        tx.commit().await?;
        all.into_iter()
            .find(|p| p.handle == handle)
            .ok_or_else(|| crate::broken(format!("Persona {handle} nach dem Setzen der Farbe verschwunden")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_colors_are_normalised_to_lowercase_rrggbb() {
        assert_eq!(valid_color("#ABC").as_deref(), Some("#aabbcc"), "#rgb wird verdoppelt und klein");
        assert_eq!(valid_color(" #1C7ED6 ").as_deref(), Some("#1c7ed6"), "Rand-Leerraum und Großbuchstaben");
        for bad in ["", "#", "1c7ed6", "#12", "#1234", "#12345g", "#1234567", "red", "#ab\"c", "#ab<cd>"] {
            assert_eq!(valid_color(bad), None, "{bad:?} ist keine Farbe");
        }
    }

    #[test]
    fn a_persona_name_is_trimmed_and_limited_to_one_line_of_80_chars() {
        assert_eq!(PersonaEdit::check_name("  Nathan ").as_deref(), Ok("Nathan"), "Rand-Leerraum entfällt");
        assert!(PersonaEdit::check_name(&"ä".repeat(80)).is_ok(), "80 Zeichen (auch Umlaute) sind gerade erlaubt");
        assert!(PersonaEdit::check_name("   ").is_err(), "ein leerer Name wird abgelehnt");
        assert!(PersonaEdit::check_name("a\nb").is_err(), "ein Name in zwei Zeilen wird abgelehnt");
        assert!(PersonaEdit::check_name(&"x".repeat(81)).is_err(), "81 Zeichen sind zu lang");
    }

    #[test]
    fn the_palette_holds_eight_distinct_valid_colors() {
        let mut seen = std::collections::HashSet::new();
        for color in PERSONA_PALETTE.iter().chain([&FALLBACK_COLOR]) {
            assert_eq!(valid_color(color).as_deref(), Some(*color), "{color} ist schon #rrggbb klein");
            assert!(seen.insert(*color), "{color} kommt zweimal vor");
        }
    }
}
