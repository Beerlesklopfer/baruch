// SPDX-License-Identifier: AGPL-3.0-or-later
//! Passwörter (Runde 35, Vertrag §1.1): Argon2id als PHC-Text. Nie Klartext in Datenbank, Logs, Fehlermeldungen
//! oder Debug-Ausgaben.
//!
//! Schnittstelle R35-S1: Konstanten und Signaturen; „konten“ setzt Hashen und Prüfen um (Crate `argon2`).

/// Speicher in KiB (19 MiB), Durchläufe und Parallelität für Argon2id.
pub const ARGON2_M_KIB: u32 = 19456;
pub const ARGON2_T: u32 = 2;
pub const ARGON2_P: u32 = 1;
/// Länge eines Passworts in Zeichen; sonst gibt es keine Regel.
pub const MIN_PASSWORD_CHARS: usize = 10;
pub const MAX_PASSWORD_CHARS: usize = 200;

/// Argon2id-PHC-Text des Passworts mit [`ARGON2_M_KIB`], [`ARGON2_T`], [`ARGON2_P`] und 16 Byte Zufallssalz.
/// Rechenintensiv: in `spawn_blocking` aufrufen.
pub fn hash(password: &str) -> Result<String, String> {
    hash_with(password, ARGON2_M_KIB, ARGON2_T, ARGON2_P)
}

/// Wie [`hash`] mit eigenen Parametern (Tests nehmen kleine Werte).
pub fn hash_with(password: &str, m_kib: u32, t: u32, p: u32) -> Result<String, String> {
    // STUB: Argon2id-Hash als PHC-Text mit 16 Byte Salz aus getrandom (Runde 35)
    let _ = (password, m_kib, t, p);
    Err("Passwörter: noch nicht umgesetzt (Runde 35)".to_owned())
}

/// Passt das Passwort zum PHC-Text? Die Parameter stehen im PHC-Text. Ein unlesbarer PHC-Text ergibt `false`.
pub fn verify(password: &str, phc: &str) -> bool {
    // STUB: Argon2id-Prüfung gegen den PHC-Text (Runde 35) – bis dahin passt kein Passwort.
    let _ = (password, phc);
    false
}
