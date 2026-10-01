//! Tokens, hashes and secret encryption.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Key, Nonce};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// A random token for links, sessions and bootstraps (244 random bits).
pub fn token() -> String {
    format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple())
}

/// A six-digit code for text messages.
pub fn code() -> String {
    let n = u128::from_le_bytes(*Uuid::new_v4().as_bytes()) % 1_000_000;
    format!("{n:06}")
}

/// Tokens are stored only as hashes.
pub fn hash(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

/// Secrets are encrypted with a per-account key derived from the master key. The master
/// key stands in for a key management service.
pub fn encrypt(master: &[u8; 32], account: Uuid, plaintext: &str) -> String {
    let cipher = ChaCha20Poly1305::new(&account_key(master, account));
    let nonce_bytes: [u8; 12] = Uuid::new_v4().as_bytes()[..12].try_into().expect("12 bytes");
    let nonce = Nonce::from(nonce_bytes);
    let ct = cipher.encrypt(&nonce, plaintext.as_bytes()).expect("encryption cannot fail");
    let mut out = nonce_bytes.to_vec();
    out.extend_from_slice(&ct);
    STANDARD.encode(out)
}

pub fn decrypt(master: &[u8; 32], account: Uuid, stored: &str) -> Option<String> {
    let raw = STANDARD.decode(stored).ok()?;
    if raw.len() < 12 {
        return None;
    }
    let (n, ct) = raw.split_at(12);
    let nonce_bytes: [u8; 12] = n.try_into().ok()?;
    let cipher = ChaCha20Poly1305::new(&account_key(master, account));
    String::from_utf8(cipher.decrypt(&Nonce::from(nonce_bytes), ct).ok()?).ok()
}

fn account_key(master: &[u8; 32], account: Uuid) -> Key {
    let mut mac = <Hmac<Sha256> as hmac::KeyInit>::new_from_slice(master).expect("any key length");
    mac.update(b"croncave-secrets:");
    mac.update(account.as_bytes());
    let bytes: [u8; 32] = mac.finalize().into_bytes().into();
    Key::from(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_round_trip_only_for_their_account() {
        let master = [3u8; 32];
        let a = Uuid::new_v4();
        let enc = encrypt(&master, a, "hunter2");
        assert!(!enc.contains("hunter2"));
        assert_eq!(decrypt(&master, a, &enc).as_deref(), Some("hunter2"));
        assert_eq!(decrypt(&master, Uuid::new_v4(), &enc), None);
    }

    #[test]
    fn codes_are_six_digits_and_tokens_differ() {
        assert_eq!(code().len(), 6);
        assert_ne!(token(), token());
        assert_eq!(hash("x").len(), 64);
    }
}
