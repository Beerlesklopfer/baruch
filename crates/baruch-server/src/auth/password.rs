// SPDX-License-Identifier: AGPL-3.0-or-later
//! Passwords (Round 35, contract §1.1): Argon2id as PHC text. Never plain text in database, logs, error messages
//! or debug output.
//!
//! [`hash`] and [`verify`] compute (≈ 19 MiB memory, two passes) and therefore do not belong on a thread
//! of the runtime: requests take [`hash_blocking`] and [`verify_blocking`] (`spawn_blocking`). The parameters of a
//! check stand in the PHC text; older hashes with other values thus stay checkable when the default grows.
//! No type of this module holds a password, no message names one.

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};

/// Memory in KiB (19 MiB), passes and parallelism for Argon2id.
pub const ARGON2_M_KIB: u32 = 19456;
pub const ARGON2_T: u32 = 2;
pub const ARGON2_P: u32 = 1;
/// Length of a password in characters; there is no other rule.
pub const MIN_PASSWORD_CHARS: usize = 10;
pub const MAX_PASSWORD_CHARS: usize = 200;

/// Length of salt and output in bytes.
const SALT_BYTES: usize = 16;
const OUTPUT_BYTES: usize = 32;

/// Fixed dummy hash with the parameters of [`hash`]: it is checked against when the account does not exist, so that
/// the answer takes as long as for an existing account. It belongs to no password anybody knows.
pub const DUMMY_PHC: &str =
    "$argon2id$v=19$m=19456,t=2,p=1$YmFydWNoLXNjaGVpbi1oYQ$BwgJCgsMDQ4PEBESExQVFhcYGRobHB0eHyAhIiMkJSY";

/// Violation of the length rule, as a message code of the UI (`passwort_kurz`, `passwort_lang`).
pub fn policy(password: &str) -> Result<(), &'static str> {
    match password.chars().count() {
        n if n < MIN_PASSWORD_CHARS => Err("passwort_kurz"),
        n if n > MAX_PASSWORD_CHARS => Err("passwort_lang"),
        _ => Ok(()),
    }
}

/// Argon2id PHC text of the password with [`ARGON2_M_KIB`], [`ARGON2_T`], [`ARGON2_P`] and 16 bytes of random salt.
/// Compute-intensive: call in `spawn_blocking` ([`hash_blocking`]).
pub fn hash(password: &str) -> Result<String, String> {
    hash_with(password, ARGON2_M_KIB, ARGON2_T, ARGON2_P)
}

/// Like [`hash`] with own parameters (tests take small values). Error messages never name the password.
pub fn hash_with(password: &str, m_kib: u32, t: u32, p: u32) -> Result<String, String> {
    let params = Params::new(m_kib, t, p, Some(OUTPUT_BYTES)).map_err(|e| format!("Argon2-Parameter: {e}"))?;
    let mut salt = [0u8; SALT_BYTES];
    getrandom::getrandom(&mut salt)
        .map_err(|e| crate::i18n::tc("accounts_msg.login.random", &[("detail", e.to_string().into())]))?;
    let salt = SaltString::encode_b64(&salt).map_err(|e| format!("Salz: {e}"))?;
    let hash = Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| format!("Argon2: {e}"))?;
    Ok(hash.to_string())
}

/// Does the password match the PHC text? The parameters stand in the PHC text. An unreadable PHC text or a
/// method other than Argon2id yields `false`.
pub fn verify(password: &str, phc: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(phc) else { return false };
    if parsed.algorithm.as_str() != "argon2id" {
        return false;
    }
    Argon2::default().verify_password(password.as_bytes(), &parsed).is_ok()
}

/// [`hash`] off the runtime.
pub async fn hash_blocking(password: String) -> Result<String, String> {
    tokio::task::spawn_blocking(move || hash(&password))
        .await
        .unwrap_or_else(|_| Err(crate::i18n::tc("accounts_msg.login.hash_aborted", &[])))
}

/// [`verify`] off the runtime; without PHC text (unknown account) against [`DUMMY_PHC`] – that takes equally long
/// and always yields `false`.
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
