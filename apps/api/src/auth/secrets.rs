//! Tokens, password hashes and TOTP secrets. Nothing here logs or returns a secret except
//! to the caller that asked for it.

use std::sync::OnceLock;

use aes_gcm::{
    Aes256Gcm,
    aead::{Aead, KeyInit, Payload},
};
use argon2::{
    Argon2,
    password_hash::{PasswordHasher, PasswordVerifier, phc::PasswordHash},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use sha2::{Digest, Sha256};
use totp_rs::{Algorithm, Totp};
use uuid::Uuid;

use crate::config::SecretKey;

/// Shown as the account's issuer in authenticator apps.
pub const TOTP_ISSUER: &str = "Chitty Logistics";
pub const TOTP_DIGITS: u8 = 6;
pub const TOTP_STEP_SECONDS: u64 = 30;
/// Longest password accepted, so hashing cost stays bounded.
pub const PASSWORD_MAX_LENGTH: usize = 1024;

/// Fills `buf` from the operating system's random number generator.
pub fn random_bytes<const N: usize>() -> [u8; N] {
    let mut buf = [0u8; N];
    getrandom::fill(&mut buf).expect("the operating system's random number generator failed");
    buf
}

/// A new 256-bit token: the text given to the client once, and the hash that is stored.
pub fn new_token() -> (String, [u8; 32]) {
    let raw: [u8; 32] = random_bytes();
    (URL_SAFE_NO_PAD.encode(raw), Sha256::digest(raw).into())
}

/// The stored hash of a token the client sent; `None` if it is not one of ours.
pub fn token_hash(token: &str) -> Option<[u8; 32]> {
    let raw = URL_SAFE_NO_PAD.decode(token.trim()).ok()?;
    let raw: [u8; 32] = raw.try_into().ok()?;
    Some(Sha256::digest(raw).into())
}

/// Hashes a password as an argon2id PHC string (default parameters: 19 MiB, 2 passes).
/// CPU-heavy: call from `spawn_blocking`.
pub fn hash_password(password: &str) -> Result<String, String> {
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|h| h.to_string())
        .map_err(|e| format!("hashing password: {e}"))
}

/// Checks `password` against a PHC string. With `None` (unknown account), checks against a
/// fixed hash so the time taken does not reveal whether the account exists, and returns
/// false. CPU-heavy: call from `spawn_blocking`.
pub fn verify_password(password: &str, phc: Option<&str>) -> bool {
    let (phc, real) = match phc {
        Some(p) => (p, true),
        None => (dummy_hash(), false),
    };
    let Ok(parsed) = PasswordHash::new(phc) else {
        tracing::error!("a stored password hash could not be parsed");
        return false;
    };
    let ok = Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok();
    ok && real
}

fn dummy_hash() -> &'static str {
    static DUMMY: OnceLock<String> = OnceLock::new();
    DUMMY.get_or_init(|| {
        let password = URL_SAFE_NO_PAD.encode(random_bytes::<24>());
        hash_password(&password).expect("hashing the dummy password")
    })
}

/// Computes the dummy hash now, so the first sign-in with an unknown email takes no longer
/// than any other.
pub fn warm_up() {
    let _ = dummy_hash();
}

/// A TOTP secret encrypted for storage.
pub struct EncryptedSecret {
    pub nonce: [u8; 12],
    pub ciphertext: Vec<u8>,
}

/// Encrypts a TOTP secret with AES-256-GCM. The staff id is authenticated with it, so a
/// ciphertext copied to another staff row does not decrypt.
pub fn encrypt_totp_secret(key: &SecretKey, staff_id: Uuid, secret: &[u8]) -> EncryptedSecret {
    let cipher = Aes256Gcm::new_from_slice(&key.0).expect("a 32-byte key");
    let nonce: [u8; 12] = random_bytes();
    let ciphertext = cipher
        .encrypt(
            (&nonce).into(),
            Payload {
                msg: secret,
                aad: staff_id.as_bytes(),
            },
        )
        .expect("AES-GCM encryption does not fail for short messages");
    EncryptedSecret { nonce, ciphertext }
}

/// Decrypts a stored TOTP secret; `None` if the key, staff id or data do not match.
pub fn decrypt_totp_secret(
    key: &SecretKey,
    staff_id: Uuid,
    nonce: &[u8],
    ciphertext: &[u8],
) -> Option<Vec<u8>> {
    let cipher = Aes256Gcm::new_from_slice(&key.0).ok()?;
    let nonce: [u8; 12] = nonce.try_into().ok()?;
    cipher
        .decrypt(
            (&nonce).into(),
            Payload {
                msg: ciphertext,
                aad: staff_id.as_bytes(),
            },
        )
        .ok()
}

/// RFC 6238 with SHA-1, 6 digits and 30-second steps (what authenticator apps expect); a
/// code from the step before or after is accepted for clock drift.
pub fn totp(secret: Vec<u8>, account_name: &str) -> Result<Totp, String> {
    totp_rs::Builder::new()
        .with_algorithm(Algorithm::SHA1)
        .with_digits(TOTP_DIGITS)
        .with_step_duration(TOTP_STEP_SECONDS)
        .with_skew(1)
        .with_secret(secret)
        .with_issuer(Some(TOTP_ISSUER))
        .with_account_name(account_name.replace(':', ""))
        .build()
        .map_err(|e| format!("building TOTP: {e}"))
}

/// The step a valid `code` belongs to at `now` (Unix seconds), or `None`.
pub fn check_totp(totp: &Totp, code: &str, now: u64) -> Option<u64> {
    let code = code.trim();
    if code.len() != TOTP_DIGITS as usize || !code.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    totp.check(code, now)
}

pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("the clock is after 1970")
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_hash_to_what_was_stored() {
        let (token, hash) = new_token();
        assert_eq!(token.len(), 43);
        assert_eq!(token_hash(&token), Some(hash));
        assert_eq!(token_hash("short"), None);
        assert_eq!(token_hash("not base64 at all !!"), None);
    }

    #[test]
    fn passwords_verify_and_unknown_accounts_never_do() {
        let phc = hash_password("correct horse battery").unwrap();
        assert!(phc.starts_with("$argon2id$"));
        assert!(verify_password("correct horse battery", Some(&phc)));
        assert!(!verify_password("wrong horse battery", Some(&phc)));
        assert!(!verify_password("correct horse battery", None));
        assert!(!verify_password("x", Some("garbage")));
    }

    #[test]
    fn totp_secrets_round_trip_only_for_the_same_key_and_staff() {
        let key = SecretKey([3; 32]);
        let staff = Uuid::new_v4();
        let e = encrypt_totp_secret(&key, staff, b"0123456789abcdefghij");
        assert_ne!(e.ciphertext.as_slice(), b"0123456789abcdefghij");
        assert_eq!(
            decrypt_totp_secret(&key, staff, &e.nonce, &e.ciphertext).unwrap(),
            b"0123456789abcdefghij"
        );
        assert!(decrypt_totp_secret(&SecretKey([4; 32]), staff, &e.nonce, &e.ciphertext).is_none());
        assert!(decrypt_totp_secret(&key, Uuid::new_v4(), &e.nonce, &e.ciphertext).is_none());
    }

    #[test]
    fn totp_accepts_adjacent_steps_only() {
        let t = totp(random_bytes::<20>().to_vec(), "a@example.test").unwrap();
        let now = 1_800_000_000;
        let code = t.generate(now).to_string();
        assert_eq!(check_totp(&t, &code, now), Some(now / 30));
        assert!(check_totp(&t, &code, now + 30).is_some());
        assert_eq!(check_totp(&t, &code, now + 90), None);
        assert_eq!(check_totp(&t, "12345", now), None);
        assert_eq!(check_totp(&t, "abcdef", now), None);
    }
}
