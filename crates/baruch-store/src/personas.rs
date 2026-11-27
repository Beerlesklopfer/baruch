// SPDX-License-Identifier: AGPL-3.0-or-later
//! Colour per persona (migration `0010_persona_color.sql`, contract Round 7, "proposals").
//!
//! The column `persona.color` is `NULL` (default) or `#rrggbb` in lower-case letters (CHECK). The default comes from
//! [`PERSONA_PALETTE`] after the order of the personas (`position`, `id`), the n-th persona gets the colour `n mod 8`.
//! Authors without a persona entry (for example `@mcp:…`) are shown by the user interface in [`FALLBACK_COLOR`].
//! Personas are global and not book text: a colour change is one transaction (`BEGIN IMMEDIATE`), but no commit in the
//! Git mirror.
//!
//! Round 10 (personas): a custom role prompt per persona (`persona.prompt`, migration 0015; `NULL` = default from the
//! code of the server) and the choice of the lector in `store_meta` (key [`LECTOR_META_KEY`], value `auto` or a handle;
//! if the row is missing, `@lektor` applies as fixed).

use baruch_core::LectorChoice;
use sqlx::SqliteConnection;

use crate::{Store, StoreError};

/// Default colours in the order of the personas: easy to tell apart (blue, orange, green, violet, magenta, teal, ochre,
/// brown), lower-case `#rrggbb`.
pub const PERSONA_PALETTE: [&str; 8] =
    ["#1c7ed6", "#e8590c", "#2f9e44", "#7048e8", "#c2255c", "#0c8599", "#e0a800", "#8b5a2b"];

/// Colour for authors without a persona entry (for example MCP callers `@mcp:<client>`): grey.
pub const FALLBACK_COLOR: &str = "#868e96";

/// Effective colour of a persona.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PersonaColor {
    pub handle: String,
    pub name: String,
    /// Effective colour `#rrggbb` (chosen or default from the palette).
    pub color: String,
    /// `true`: chosen by the human; `false`: default from [`PERSONA_PALETTE`].
    pub chosen: bool,
}

/// `#rgb` or `#rrggbb` (upper- or lower-case letters, whitespace at the edges allowed) → `#rrggbb` in lower case;
/// everything else → `None`.
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

/// All personas with their effective colour, by `position`, `id`.
async fn colors(conn: &mut SqliteConnection) -> Result<Vec<PersonaColor>, StoreError> {
    let rows: Vec<(String, String, Option<String>)> =
        sqlx::query_as("SELECT handle, name, color FROM persona ORDER BY position, id").fetch_all(&mut *conn).await?;
    Ok(rows
        .into_iter()
        .enumerate()
        .map(|(index, (handle, name, color))| {
            // The column checks the format (CHECK); valid_color is the second guard.
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

/// Longest role or longest model in characters (after trimming, Round 9).
pub const MAX_PERSONA_TEXT: usize = 200;
/// Largest context length in tokens (Round 9); the smallest is 1.
pub const MAX_CONTEXT_LENGTH: u32 = 10_000_000;

/// Longest custom role prompt in characters (after trimming, Round 10; CHECK in migration 0015).
pub const MAX_PROMPT_CHARS: usize = 8000;

/// Key of the lector choice in `store_meta` (Round 10): value `auto` or a handle.
pub const LECTOR_META_KEY: &str = "lector";

/// Key of the setting „Chat nur Abschnitt“ in `store_meta` (Round 16a): `chapter` or `section`.
pub const CHAT_SCOPE_META_KEY: &str = "chat_scope";

/// What the chat of a persona passes along for the section shown (Round 16a): the whole chapter (default, as before) or
/// only the section itself (plus title and outline of the work).
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

    /// From `chapter` | `section` (trimmed); otherwise `None`.
    pub fn parse(value: &str) -> Option<ChatScope> {
        match value.trim() {
            "chapter" => Some(ChatScope::Chapter),
            "section" => Some(ChatScope::Section),
            _ => None,
        }
    }
}

/// A persona with all fields from the table `persona` (Round 9, settings „Assistenten“). Personas are global (for all
/// works); `color`/`chosen` like [`PersonaColor`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct PersonaRow {
    pub handle: String,
    pub name: String,
    pub role: String,
    pub model: String,
    /// Context length in tokens.
    pub context_length: u32,
    pub active: bool,
    /// Effective colour `#rrggbb` (chosen or default from the palette).
    pub color: String,
    /// `true`: chosen by the human.
    pub chosen: bool,
    /// Custom role prompt (Round 10); `None` = default from the code.
    pub prompt: Option<String>,
}

/// Change of a persona ([`Store::update_persona`]); `None` = stays. Role and model are trimmed and must have
/// 1–[`MAX_PERSONA_TEXT`] characters, the context length 1–[`MAX_CONTEXT_LENGTH`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PersonaEdit {
    pub role: Option<String>,
    pub model: Option<String>,
    pub context_length: Option<u32>,
    pub active: Option<bool>,
}

/// Longest name of a persona in characters (after trimming, Round 11).
const MAX_PERSONA_NAME: usize = 80;

impl PersonaEdit {
    /// Check the name of a persona (Round 11, Einstellungen → Assistenten): trimmed 1–80 characters, on one line. `Ok`
    /// = the trimmed name, `Err` = German message.
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

    /// Checks the limits; `Err` names the first problem in German (the server layer reports it as -32602 or 422, the
    /// store rejects the same as the second guard with `Refused`).
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

/// All personas with all fields, by `position`, `id`.
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

/// Custom role prompt checked: `None` = default; otherwise trimmed 1–[`MAX_PROMPT_CHARS`] characters with `\n` as the
/// line ending.
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

/// Read the lector choice from `store_meta`: `auto` → [`LectorChoice::Auto`], another value → fixed to this persona, no
/// row → default (fixed `@lektor`).
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

/// Handle in the table: as entered or with a leading `@`.
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
    /// All personas with all fields (role, model, context, active, colour) in the order `position`, `id` – without a
    /// work (personas are global, Round 9).
    pub async fn persona_list(&self) -> Result<Vec<PersonaRow>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        rows(&mut conn).await
    }

    /// Change role, model, context length and active of a persona, atomically (`BEGIN IMMEDIATE`). Unknown persona →
    /// `UnknownPersona` (handle also without `@`); limits violated ([`PersonaEdit::check`]) → `Refused`; in both cases
    /// everything stays as it is. No commit in the mirror (personas are not book text).
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

    /// Save everything of a persona at once (Round 11, one button „Speichern“ per persona): name (`Some`,
    /// [`PersonaEdit::check_name`]), role, model, context and active ([`PersonaEdit`]), colour (`Some(Some(..))` set,
    /// `Some(None)` default) and role prompt (`Some(Some(..))` custom, `Some(None)` default); `None` = stays. First
    /// everything is checked, then written in **one** transaction: an invalid value changes nothing. Unknown persona →
    /// `UnknownPersona`, invalid colour → `Color`, otherwise `Refused` with a German message. No commit in the mirror.
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

    /// Choice of the lector (Round 10): `store_meta.lector`, without a row fixed `@lektor`.
    pub async fn lector_choice(&self) -> Result<LectorChoice, StoreError> {
        let mut conn = self.pool.acquire().await?;
        read_lector_choice(&mut conn).await
    }

    /// Set the choice of the lector, atomically. `Fixed`: handle with or without `@`, the persona must exist (otherwise
    /// `UnknownPersona`, nothing changed); the handle from the table is stored. Whether it is active counts only when
    /// the lector is determined ([`LectorChoice::pick`]). No commit in the mirror.
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

    /// Scope of the chat (Round 16a, `store_meta.chat_scope`): without a row or with an unknown value
    /// [`ChatScope::Chapter`] (as before: the chapter of the section).
    pub async fn chat_scope(&self) -> Result<ChatScope, StoreError> {
        let value: Option<String> = sqlx::query_scalar("SELECT value FROM store_meta WHERE key = ?")
            .bind(CHAT_SCOPE_META_KEY)
            .fetch_optional(&self.pool)
            .await?;
        Ok(value.as_deref().and_then(ChatScope::parse).unwrap_or_default())
    }

    /// Set the scope of the chat (only the human; no commit in the mirror, it is only in SQLite).
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

    /// Custom role prompt of a persona (`None` = default from the code); unknown → `UnknownPersona`.
    pub async fn persona_prompt(&self, handle: &str) -> Result<Option<String>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        let Some(found) = find_handle(&mut conn, handle).await? else {
            return Err(StoreError::UnknownPersona(handle.trim().to_owned()));
        };
        Ok(sqlx::query_scalar("SELECT prompt FROM persona WHERE handle = ?").bind(found).fetch_one(&mut *conn).await?)
    }

    /// Set a custom role prompt (`Some`, trimmed 1–[`MAX_PROMPT_CHARS`] characters, otherwise `Refused`) or reset to
    /// the default (`None`), atomically; unknown persona → `UnknownPersona`. In every error case everything stays as it
    /// is. No commit in the mirror (not book text).
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

    /// All personas with their effective colour in the order `position`, `id`.
    pub async fn persona_colors(&self) -> Result<Vec<PersonaColor>, StoreError> {
        let mut conn = self.pool.acquire().await?;
        colors(&mut conn).await
    }

    /// Set the colour of a persona (`Some`, `#rgb`/`#rrggbb`) or reset it to the default (`None`), atomically. Unknown
    /// persona → `UnknownPersona` (the handle may come without `@`), invalid colour → `Color`; in both cases everything
    /// stays as it is.
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
