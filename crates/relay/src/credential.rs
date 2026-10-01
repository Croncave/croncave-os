//! Short-lived credentials that work for one computer only.

use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use uuid::Uuid;

#[derive(Clone)]
pub struct Credentials {
    secret: Vec<u8>,
    ttl: Duration,
}

impl Credentials {
    pub fn new(secret: &[u8], ttl: Duration) -> Self {
        Self { secret: secret.to_vec(), ttl }
    }

    /// A credential for `computer`, and when it expires (unix seconds).
    pub fn issue(&self, computer: Uuid) -> (String, i64) {
        let expires_at = chrono::Utc::now().timestamp() + self.ttl.as_secs() as i64;
        let body = URL_SAFE_NO_PAD.encode(format!("{computer}:{expires_at}"));
        let sig = hex::encode(self.sign(body.as_bytes()));
        (format!("{body}.{sig}"), expires_at)
    }

    /// The computer a credential is for, if it is genuine and unexpired.
    pub fn verify(&self, credential: &str) -> Option<Uuid> {
        let (body, sig) = credential.split_once('.')?;
        let sig = hex::decode(sig).ok()?;
        let mut mac = self.mac();
        mac.update(body.as_bytes());
        mac.verify_slice(&sig).ok()?;
        let decoded = String::from_utf8(URL_SAFE_NO_PAD.decode(body).ok()?).ok()?;
        let (id, exp) = decoded.split_once(':')?;
        if exp.parse::<i64>().ok()? < chrono::Utc::now().timestamp() {
            return None;
        }
        id.parse().ok()
    }

    fn mac(&self) -> Hmac<Sha256> {
        <Hmac<Sha256> as KeyInit>::new_from_slice(&self.secret).expect("hmac takes any key length")
    }

    fn sign(&self, data: &[u8]) -> Vec<u8> {
        let mut mac = self.mac();
        mac.update(data);
        mac.finalize().into_bytes().to_vec()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issued_credentials_verify_for_their_computer_only() {
        let c = Credentials::new(b"secret", Duration::from_secs(60));
        let id = Uuid::new_v4();
        let (cred, _) = c.issue(id);
        assert_eq!(c.verify(&cred), Some(id));
        let other = Credentials::new(b"other", Duration::from_secs(60));
        assert_eq!(other.verify(&cred), None);
    }

    #[test]
    fn tampered_and_expired_credentials_fail() {
        let c = Credentials::new(b"secret", Duration::from_secs(60));
        let (cred, _) = c.issue(Uuid::new_v4());
        let (body, sig) = cred.split_once('.').unwrap();
        let forged_body = URL_SAFE_NO_PAD.encode(format!("{}:{}", Uuid::new_v4(), i64::MAX));
        assert_eq!(c.verify(&format!("{forged_body}.{sig}")), None);
        assert_eq!(c.verify(body), None);
        let expired = Credentials::new(b"secret", Duration::from_secs(0));
        let (cred, _) = expired.issue(Uuid::new_v4());
        std::thread::sleep(Duration::from_millis(1100));
        assert_eq!(expired.verify(&cred), None);
    }
}
