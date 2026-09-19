use crate::{CredentialKey, Database, SessionError, session_crypto::secret_hash};
use sqlx::Row;
use uuid::Uuid;

impl Database {
    /// Persists a short-lived, browser-bound OIDC attempt without storing state or PKCE in plaintext.
    pub async fn begin_login(
        &self,
        state: &str,
        browser: &str,
        proof: &str,
        key: &CredentialKey,
        now: i64,
    ) -> Result<(), SessionError> {
        let id = Uuid::now_v7();
        let encrypted = key.seal(id, proof)?;
        let mut tx = self.pool.begin().await?;
        sqlx::query("DELETE FROM oidc_login WHERE expires_at<=$1")
            .bind(now)
            .execute(&mut *tx)
            .await?;
        sqlx::query("INSERT INTO oidc_login(id,state_hash,browser_hash,encrypted_proof,expires_at) VALUES($1,$2,$3,$4,$5)").bind(id).bind(secret_hash(state)).bind(secret_hash(browser)).bind(encrypted).bind(now+600).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }
    /// Atomically consumes state only for its initiating browser; replay and expiry fail before exchange.
    pub async fn consume_login(
        &self,
        state: &str,
        browser: &str,
        key: &CredentialKey,
        now: i64,
    ) -> Result<String, SessionError> {
        let row=sqlx::query("DELETE FROM oidc_login WHERE state_hash=$1 AND browser_hash=$2 AND expires_at>$3 RETURNING id,encrypted_proof").bind(secret_hash(state)).bind(secret_hash(browser)).bind(now).fetch_optional(&self.pool).await?.ok_or(SessionError::Unauthorized)?;
        key.open(
            row.try_get::<Uuid, _>("id")?,
            &row.try_get::<Vec<u8>, _>("encrypted_proof")?,
        )
    }
}
