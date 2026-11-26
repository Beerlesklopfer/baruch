// SPDX-License-Identifier: AGPL-3.0-or-later
//! Passwörter (Runde 35, Vertrag §1.1): Argon2id als PHC-Text. Nie Klartext in Datenbank, Logs, Fehlermeldungen
//! oder Debug-Ausgaben.
//!
//! [`hash`] und [`verify`] rechnen (≈ 19 MiB Speicher, zwei Durchläufe) und gehören darum nicht auf einen Faden
//! der Laufzeit: Anfragen nehmen [`hash_blocking`] und [`verify_blocking`] (`spawn_blocking`). Die Parameter einer
//! Prüfung stehen im PHC-Text; ältere Hashes mit anderen Werten bleiben damit prüfbar, wenn die Vorgabe wächst.
//! Kein Typ dieses Moduls hält ein Passwort, keine Meldung nennt eines.

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};

/// Speicher in KiB (19 MiB), Durchläufe und Parallelität für Argon2id.
pub const ARGON2_M_KIB: u32 = 19456;
pub const ARGON2_T: u32 = 2;
pub const ARGON2_P: u32 = 1;
/// Länge eines Passworts in Zeichen; sonst gibt es keine Regel.
pub const MIN_PASSWORD_CHARS: usize = 10;
pub const MAX_PASSWORD_CHARS: usize = 200;

/// Länge von Salz und Ausgabe in Byte.
const SALT_BYTES: usize = 16;
const OUTPUT_BYTES: usize = 32;

/// Fester Schein-Hash mit den Parametern von [`hash`]: Gegen ihn wird geprüft, wenn es das Konto nicht gibt, damit
/// die Antwort gleich lange dauert wie bei einem vorhandenen Konto. Er gehört zu keinem Passwort, das jemand kennt.
pub const DUMMY_PHC: &str =
    "$argon2id$v=19$m=19456,t=2,p=1$YmFydWNoLXNjaGVpbi1oYQ$BwgJCgsMDQ4PEBESExQVFhcYGRobHB0eHyAhIiMkJSY";

/// Verstoß gegen die Regel der Länge, als Meldungscode der Oberfläche (`passwort_kurz`, `passwort_lang`).
pub fn policy(password: &str) -> Result<(), &'static str> {
    match password.chars().count() {
        n if n < MIN_PASSWORD_CHARS => Err("passwort_kurz"),
        n if n > MAX_PASSWORD_CHARS => Err("passwort_lang"),
        _ => Ok(()),
    }
}

/// Argon2id-PHC-Text des Passworts mit [`ARGON2_M_KIB`], [`ARGON2_T`], [`ARGON2_P`] und 16 Byte Zufallssalz.
/// Rechenintensiv: in `spawn_blocking` aufrufen ([`hash_blocking`]).
pub fn hash(password: &str) -> Result<String, String> {
    hash_with(password, ARGON2_M_KIB, ARGON2_T, ARGON2_P)
}

/// Wie [`hash`] mit eigenen Parametern (Tests nehmen kleine Werte). Fehlermeldungen nennen nie das Passwort.
pub fn hash_with(password: &str, m_kib: u32, t: u32, p: u32) -> Result<String, String> {
    let params = Params::new(m_kib, t, p, Some(OUTPUT_BYTES)).map_err(|e| format!("Argon2-Parameter: {e}"))?;
    let mut salt = [0u8; SALT_BYTES];
    getrandom::getrandom(&mut salt).map_err(|e| format!("Zufallszahlen des Systems: {e}"))?;
    let salt = SaltString::encode_b64(&salt).map_err(|e| format!("Salz: {e}"))?;
    let hash = Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| format!("Argon2: {e}"))?;
    Ok(hash.to_string())
}

/// Passt das Passwort zum PHC-Text? Die Parameter stehen im PHC-Text. Ein unlesbarer PHC-Text oder ein anderes
/// Verfahren als Argon2id ergibt `false`.
pub fn verify(password: &str, phc: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(phc) else { return false };
    if parsed.algorithm.as_str() != "argon2id" {
        return false;
    }
    Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok()
}

/// [`hash`] abseits der Laufzeit.
pub async fn hash_blocking(password: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || hash(&password))
        .await
        .unwrap_or_else(|_| Err("Passwort-Hash abgebrochen".to_owned()))
}

/// [`verify`] abseits der Laufzeit; ohne PHC-Text (unbekanntes Konto) gegen [`DUMMY_PHC`] – das dauert gleich lang
/// und ergibt immer `false`.
pub async fn verify_blocking(password: String, phc: Option<String>) -> bool {
    tokio::task::spawn_blocking(move || match phc {
        Some(phc) => verify(&password, &phc),
        None => {
            let _ = verify(&password, DUMMY_PHC);
            false
        }
    })
    .await
    .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stored_text_is_argon2id_with_the_parameters_of_the_contract() {
        let phc = hash("ein-langes-passwort").unwrap();
        assert!(
            phc.starts_with("$argon2id$v=19$m=19456,t=2,p=1$"),
            "PHC-Text mit Argon2id, m = 19456 KiB, t = 2, p = 1 (Vertrag §1.1): Anfang {:?}",
            &phc[..31.min(phc.len())]
        );
        let parsed = PasswordHash::new(&phc).unwrap();
        assert_eq!(parsed.salt.unwrap().as_str().len(), 22, "16 Byte Salz sind 22 Zeichen Base64 ohne Füllung");
        assert_eq!(parsed.hash.unwrap().len(), 32, "32 Byte Ausgabe");
        assert!(!phc.contains("ein-langes-passwort"), "der Klartext steht nicht im gespeicherten Text");
        assert!(verify("ein-langes-passwort", &phc), "das richtige Passwort passt");
        assert!(!verify("ein-langes-passworT", &phc), "ein anderes Passwort passt nicht");
        assert_ne!(hash("ein-langes-passwort").unwrap(), phc, "jedes Hashen nimmt ein neues Salz");
    }

    #[test]
    fn verify_reads_the_parameters_from_the_text() {
        let small = hash_with("klein-und-schnell", 8, 1, 1).unwrap();
        assert!(small.starts_with("$argon2id$v=19$m=8,t=1,p=1$"), "die eigenen Parameter stehen im Text");
        assert!(verify("klein-und-schnell", &small), "geprüft wird mit den Parametern aus dem Text, nicht der Vorgabe");
        assert!(!verify("klein-und-falsch", &small), "auch mit kleinen Parametern passt nur das richtige Passwort");
        let other = hash_with("klein-und-schnell", 16, 2, 1).unwrap();
        assert!(verify("klein-und-schnell", &other), "andere Parameter, gleiches Passwort: passt");
    }

    #[test]
    fn unreadable_or_foreign_hashes_never_match() {
        assert!(!verify("egal", ""), "leerer Text ist kein Hash");
        assert!(!verify("egal", "$argon2id$test"), "ein verstümmelter PHC-Text passt zu keinem Passwort");
        assert!(!verify("egal", "klartext"), "ein Klartext in der Spalte würde nie als Passwort gelten");
        let argon2i = "$argon2i$v=19$m=8,t=1,p=1$YmFydWNoLXNjaGVpbi1oYQ$BwgJCgsMDQ4PEBESExQVFhcYGRobHB0eHyAhIiMkJSY";
        assert!(!verify("egal", argon2i), "nur Argon2id gilt");
    }

    #[test]
    fn the_dummy_hash_is_well_formed_and_matches_nothing_known() {
        let parsed = PasswordHash::new(DUMMY_PHC).expect("der Schein-Hash ist ein lesbarer PHC-Text");
        let params = Params::try_from(&parsed).unwrap();
        assert_eq!(
            (params.m_cost(), params.t_cost(), params.p_cost()),
            (ARGON2_M_KIB, ARGON2_T, ARGON2_P),
            "gleiche Parameter wie ein echter Hash – sonst verriete die Dauer, ob es das Konto gibt"
        );
        assert_eq!(parsed.hash.unwrap().len(), 32, "gleiche Ausgabelänge wie ein echter Hash");
        for guess in ["", "passwort", "baruch-schein-ha"] {
            assert!(!verify(guess, DUMMY_PHC), "der Schein-Hash passt zu keinem naheliegenden Passwort");
        }
    }

    #[test]
    fn only_the_length_is_a_rule() {
        assert_eq!(policy("neun-zeic"), Err("passwort_kurz"), "9 Zeichen sind zu kurz");
        assert_eq!(policy("zehn-zeich"), Ok(()), "10 Zeichen genügen");
        assert_eq!(policy("ääääääääää"), Ok(()), "gezählt werden Zeichen, nicht Bytes");
        assert_eq!(policy(&"a".repeat(200)), Ok(()), "200 Zeichen gehen noch");
        assert_eq!(policy(&"a".repeat(201)), Err("passwort_lang"), "201 Zeichen sind zu lang");
        assert_eq!(policy("aaaaaaaaaa"), Ok(()), "keine Regel zu Ziffern oder Sonderzeichen");
    }

    #[test]
    fn errors_never_carry_the_password() {
        let error = hash_with("streng-geheimes-wort", 1, 0, 0).unwrap_err();
        assert!(!error.contains("streng-geheimes-wort"), "die Fehlermeldung nennt das Passwort nicht: {error}");
        assert!(error.starts_with("Argon2-Parameter"), "sie nennt die Ursache: {error}");
    }
}
