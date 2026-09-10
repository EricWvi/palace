use crate::session::SessionError;
use aes_gcm::{
    Aes256Gcm, KeyInit,
    aead::{Aead, AeadCore, OsRng, Payload},
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use uuid::Uuid;

/// Deployment-owned encryption key; its persistence across restarts preserves credential access.
#[derive(Clone)]
pub struct CredentialKey([u8; 32]);
impl CredentialKey {
    pub fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }
    /// Encrypts refresh material with per-write randomness and session-bound authentication.
    pub(crate) fn seal(&self, id: Uuid, secret: &str) -> Result<Vec<u8>, SessionError> {
        let cipher = Aes256Gcm::new((&self.0).into());
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng);
        let encrypted = cipher
            .encrypt(
                &nonce,
                Payload {
                    msg: secret.as_bytes(),
                    aad: id.as_bytes(),
                },
            )
            .map_err(|_| SessionError::Integrity)?;
        Ok([nonce.as_slice(), encrypted.as_slice()].concat())
    }
    /// Refuses ciphertext tampering or moving a credential into another session.
    pub(crate) fn open(&self, id: Uuid, bytes: &[u8]) -> Result<String, SessionError> {
        if bytes.len() < 12 {
            return Err(SessionError::Integrity);
        }
        let cipher = Aes256Gcm::new((&self.0).into());
        let plaintext = cipher
            .decrypt(
                bytes[..12].into(),
                Payload {
                    msg: &bytes[12..],
                    aad: id.as_bytes(),
                },
            )
            .map_err(|_| SessionError::Integrity)?;
        String::from_utf8(plaintext).map_err(|_| SessionError::Integrity)
    }
    /// Derives an opaque 256-bit secret so concurrent requests can receive the committed generation.
    pub(crate) fn secret(&self, id: Uuid, generation: i64) -> Result<String, SessionError> {
        let mut mac =
            <Hmac<Sha256> as Mac>::new_from_slice(&self.0).map_err(|_| SessionError::Integrity)?;
        mac.update(b"palace-session-v1\0");
        mac.update(id.as_bytes());
        mac.update(&generation.to_be_bytes());
        Ok(URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes()))
    }
}
/// Stores only a one-way browser credential digest in PostgreSQL.
pub(crate) fn secret_hash(secret: &str) -> Vec<u8> {
    Sha256::digest(secret.as_bytes()).to_vec()
}
#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    /// Protects persisted tokens against substitution and separates every opaque generation.
    #[test]
    fn credential_encryption_is_bound_to_session_and_rotation() {
        let key = CredentialKey::new([7; 32]);
        let id = Uuid::new_v4();
        let encrypted = key.seal(id, "refresh").unwrap();
        assert_eq!(key.open(id, &encrypted).unwrap(), "refresh");
        assert!(key.open(Uuid::new_v4(), &encrypted).is_err());
        let mut tampered = encrypted.clone();
        *tampered.last_mut().unwrap() ^= 1;
        assert!(key.open(id, &tampered).is_err());
        assert_ne!(
            key.secret(id, /*generation*/ 0).unwrap(),
            key.secret(id, /*generation*/ 1).unwrap()
        );
        assert_eq!(
            key.secret(id, /*generation*/ 1).unwrap(),
            key.secret(id, /*generation*/ 1).unwrap()
        );
        assert_eq!(key.secret(id, /*generation*/ 1).unwrap().len(), 43);
    }
}
