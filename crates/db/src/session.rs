use crate::{
    Database, DbError, Owner, OwnerScope,
    owner::normalize_email,
    session_crypto::{CredentialKey, secret_hash},
};
use sqlx::Row;
use std::future::Future;
use uuid::Uuid;

/// A fresh protocol-verified result, including the current identity and a revocable refresh credential.
pub struct IdentityTokens {
    pub issuer: String,
    pub subject: String,
    pub email: String,
    pub refresh: String,
}
#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("identity provider temporarily unavailable")]
    Unavailable,
    #[error("identity provider rejected credential or claims")]
    Rejected,
}
/// Implements actual provider refresh/UserInfo and revocation; fakes exercise orchestration deterministically.
pub trait IdentityProvider: Send + Sync {
    /// Must validate refreshed protocol data and obtain current email without historical fallback.
    fn refresh(
        &self,
        credential: &str,
    ) -> impl Future<Output = Result<IdentityTokens, ProviderError>> + Send;
    /// Attempts external revocation after Palace has already committed local invalidation.
    fn revoke(&self, credential: &str) -> impl Future<Output = Result<(), ProviderError>> + Send;
}
#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error(transparent)]
    Database(#[from] DbError),
    #[error("authentication required")]
    Unauthorized,
    #[error("identity revalidation temporarily unavailable")]
    Unavailable,
    #[error("session credential integrity failure")]
    Integrity,
}
impl From<sqlx::Error> for SessionError {
    /// Keeps database details out of authentication responses.
    fn from(error: sqlx::Error) -> Self {
        Self::Database(error.into())
    }
}
/// Contains the new opaque cookie only for the HTTP composition boundary; never serialize or log it.
pub struct AuthenticatedSession {
    pub id: Uuid,
    pub owner: Owner,
    pub secret: String,
}
#[derive(Clone, Copy)]
pub enum RevokeScope {
    Current,
    AllDevices,
}
impl Database {
    /// Establishes a fresh session after OIDC login and invalidates a supplied prior browser session.
    pub async fn create_session(
        &self,
        tokens: IdentityTokens,
        key: &CredentialKey,
        now: i64,
        previous: Option<&str>,
    ) -> Result<AuthenticatedSession, SessionError> {
        if tokens.refresh.trim().is_empty() {
            return Err(SessionError::Unauthorized);
        }
        let owner = self
            .resolve_identity(&tokens.issuer, &tokens.subject, &tokens.email)
            .await?;
        let id = Uuid::new_v4();
        let secret = key.secret(id, /*generation*/ 0)?;
        let encrypted = key.seal(id, &tokens.refresh)?;
        let mut tx = self.pool.begin().await?;
        // Serialize creation against disablement, then verify the binding still exists.
        let active:Option<Uuid>=sqlx::query_scalar("SELECT i.id FROM owner_identity i JOIN owner o ON o.id=i.owner_id WHERE i.id=$1 AND NOT i.disabled AND NOT o.disabled FOR UPDATE OF i,o").bind(owner.identity_id).fetch_optional(&mut *tx).await?;
        if active.is_none() {
            return Err(SessionError::Unauthorized);
        }
        if let Some(previous) = previous {
            sqlx::query("UPDATE owner_session SET revoked_at=$2,revoke_reason='login rotation',revocation_pending=true WHERE (secret_hash=$1 OR previous_secret_hash=$1) AND revoked_at IS NULL").bind(secret_hash(previous)).bind(now).execute(&mut *tx).await?;
        }
        sqlx::query("INSERT INTO owner_session(id,owner_id,owner_identity_id,secret_hash,created_at,last_seen_at,last_identity_verified_at,refresh_credential,rotated_at) VALUES($1,$2,$3,$4,$5,$5,$5,$6,$5)")
            .bind(id).bind(owner.id).bind(owner.identity_id).bind(secret_hash(&secret)).bind(now).bind(encrypted).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(AuthenticatedSession { id, owner, secret })
    }
    /// Refreshes expired verification under a database row lock, without imposing session lifetime limits.
    pub async fn authenticate<P: IdentityProvider>(
        &self,
        secret: &str,
        key: &CredentialKey,
        provider: &P,
        now: i64,
    ) -> Result<AuthenticatedSession, SessionError> {
        let hash = secret_hash(secret);
        let mut tx = self.pool.begin().await?;
        let row=sqlx::query("SELECT s.*,i.issuer,i.subject,i.disabled AS identity_disabled,o.disabled AS owner_disabled,o.email FROM owner_session s JOIN owner_identity i ON i.id=s.owner_identity_id AND i.owner_id=s.owner_id JOIN owner o ON o.id=s.owner_id WHERE s.secret_hash=$1 OR (s.previous_secret_hash=$1 AND $2::bigint IS NOT NULL) FOR UPDATE OF s")
            .bind(&hash).bind(now).fetch_optional(&mut *tx).await?.ok_or(SessionError::Unauthorized)?;
        let id: Uuid = row.try_get("id")?;
        let owner_id: Uuid = row.try_get("owner_id")?;
        let identity_id: Uuid = row.try_get("owner_identity_id")?;
        if row.try_get::<Option<i64>, _>("revoked_at")?.is_some()
            || row.try_get::<bool, _>("identity_disabled")?
            || row.try_get::<bool, _>("owner_disabled")?
        {
            return Err(SessionError::Unauthorized);
        }
        if hash != row.try_get::<Vec<u8>, _>("secret_hash")?
            && row
                .try_get::<Option<i64>, _>("previous_valid_until")?
                .is_none_or(|until| now >= until)
        {
            sqlx::query("UPDATE owner_session SET revoked_at=$2,revoke_reason='retired secret replay',revocation_pending=true WHERE id=$1").bind(id).bind(now).execute(&mut *tx).await?;
            tx.commit().await?;
            return Err(SessionError::Unauthorized);
        }
        // READ COMMITTED may refresh the locked session tuple but retain an older joined owner tuple.
        // Read owner state again after acquiring the session lock so waiters observe the published email.
        let current = sqlx::query("SELECT o.email,o.disabled OR i.disabled AS disabled FROM owner o JOIN owner_identity i ON i.owner_id=o.id WHERE o.id=$1 AND i.id=$2")
            .bind(owner_id).bind(identity_id).fetch_one(&mut *tx).await?;
        if current.try_get::<bool, _>("disabled")? {
            return Err(SessionError::Unauthorized);
        }
        let mut generation: i64 = row.try_get("generation")?;
        let mut email: String = current.try_get("email")?;
        if secret_hash(&key.secret(id, generation)?) != row.try_get::<Vec<u8>, _>("secret_hash")? {
            sqlx::query("UPDATE owner_session SET revoked_at=$2,revoke_reason='secret integrity',revocation_pending=true WHERE id=$1").bind(id).bind(now).execute(&mut *tx).await?;
            tx.commit().await?;
            return Err(SessionError::Unauthorized);
        }
        let last_verified: i64 = row.try_get("last_identity_verified_at")?;
        if needs_revalidation(last_verified, now) {
            let refresh = key.open(id, &row.try_get::<Vec<u8>, _>("refresh_credential")?);
            let result = match refresh {
                Ok(refresh) => provider.refresh(&refresh).await,
                Err(_) => Err(ProviderError::Rejected),
            };
            let tokens = match result {
                Ok(tokens)
                    if tokens.issuer == row.try_get::<String, _>("issuer")?
                        && tokens.subject == row.try_get::<String, _>("subject")?
                        && normalize_email(&tokens.email).is_ok()
                        && !tokens.refresh.trim().is_empty() =>
                {
                    tokens
                }
                Err(ProviderError::Unavailable) => return Err(SessionError::Unavailable),
                Ok(_) | Err(ProviderError::Rejected) => {
                    sqlx::query("UPDATE owner_session SET revoked_at=$2,revoke_reason='identity rejected',revocation_pending=true WHERE id=$1").bind(id).bind(now).execute(&mut *tx).await?;
                    tx.commit().await?;
                    return Err(SessionError::Unauthorized);
                }
            };
            // A savepoint allows email uniqueness failures to commit deterministic session revocation.
            sqlx::query("SAVEPOINT email_update")
                .execute(&mut *tx)
                .await?;
            let updated=sqlx::query("UPDATE owner SET email=$2,normalized_email=$3,updated_at=now() WHERE id=$1 AND NOT disabled")
                .bind(owner_id).bind(&tokens.email).bind(normalize_email(&tokens.email)?).execute(&mut *tx).await;
            if updated.as_ref().is_err_and(|e| {
                e.as_database_error()
                    .is_some_and(sqlx::error::DatabaseError::is_unique_violation)
            }) {
                sqlx::query("ROLLBACK TO SAVEPOINT email_update")
                    .execute(&mut *tx)
                    .await?;
                sqlx::query("UPDATE owner_session SET revoked_at=$2,revoke_reason='email conflict',revocation_pending=true WHERE id=$1").bind(id).bind(now).execute(&mut *tx).await?;
                tx.commit().await?;
                return Err(SessionError::Unauthorized);
            }
            if updated?.rows_affected() != 1 {
                return Err(SessionError::Unauthorized);
            }
            email = tokens.email;
            generation = generation.checked_add(1).ok_or(SessionError::Integrity)?;
            let next = key.secret(id, generation)?;
            sqlx::query("UPDATE owner_session SET previous_secret_hash=secret_hash,previous_valid_until=$2+30,secret_hash=$3,generation=$4,refresh_credential=$5,last_identity_verified_at=$2,rotated_at=$2 WHERE id=$1")
                .bind(id).bind(now).bind(secret_hash(&next)).bind(generation).bind(key.seal(id,&tokens.refresh)?).execute(&mut *tx).await?;
        }
        sqlx::query("UPDATE owner_session SET last_seen_at=$2 WHERE id=$1")
            .bind(id)
            .bind(now)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(AuthenticatedSession {
            id,
            owner: Owner {
                id: owner_id,
                email,
                identity_id,
            },
            secret: key.secret(id, generation)?,
        })
    }
    /// Allows logout during provider outages without using stale identity to access business data.
    pub async fn logout_browser(
        &self,
        secret: &str,
        scope: RevokeScope,
        now: i64,
    ) -> Result<(), SessionError> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query("SELECT id,owner_id FROM owner_session WHERE (secret_hash=$1 OR (previous_secret_hash=$1 AND previous_valid_until>$2)) AND revoked_at IS NULL FOR UPDATE")
            .bind(secret_hash(secret)).bind(now).fetch_optional(&mut *tx).await?.ok_or(SessionError::Unauthorized)?;
        let id: Uuid = row.try_get("id")?;
        let owner: Uuid = row.try_get("owner_id")?;
        let query = match scope {
            RevokeScope::Current => {
                "UPDATE owner_session SET revoked_at=$3,revoke_reason='logout',revocation_pending=true WHERE owner_id=$1 AND id=$2 AND revoked_at IS NULL"
            }
            RevokeScope::AllDevices => {
                "UPDATE owner_session SET revoked_at=$3,revoke_reason='logout all',revocation_pending=true WHERE owner_id=$1 AND $2::uuid IS NOT NULL AND revoked_at IS NULL"
            }
        };
        sqlx::query(query)
            .bind(owner)
            .bind(id)
            .bind(now)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }
    /// Commits local logout first; the retry worker owns external revocation independently.
    pub async fn revoke_sessions(
        &self,
        owner: OwnerScope,
        session: Uuid,
        scope: RevokeScope,
        now: i64,
    ) -> Result<(), SessionError> {
        let query = match scope {
            RevokeScope::Current => {
                "UPDATE owner_session SET revoked_at=$3,revoke_reason='logout',revocation_pending=true WHERE owner_id=$1 AND id=$2 AND revoked_at IS NULL"
            }
            RevokeScope::AllDevices => {
                "UPDATE owner_session SET revoked_at=$3,revoke_reason='logout all',revocation_pending=true WHERE owner_id=$1 AND $2::uuid IS NOT NULL AND revoked_at IS NULL"
            }
        };
        sqlx::query(query)
            .bind(owner.id())
            .bind(session)
            .bind(now)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
    /// Retries durable external revocations; endpoint failure never restores a Palace session.
    pub async fn retry_revocations<P: IdentityProvider>(
        &self,
        key: &CredentialKey,
        provider: &P,
    ) -> Result<(), SessionError> {
        let rows=sqlx::query("SELECT id,refresh_credential FROM owner_session WHERE revocation_pending ORDER BY revoked_at,id LIMIT 100").fetch_all(&self.pool).await?;
        for row in rows {
            let id: Uuid = row.try_get("id")?;
            let credential = key.open(id, &row.try_get::<Vec<u8>, _>("refresh_credential")?)?;
            if provider.revoke(&credential).await.is_ok() {
                sqlx::query("UPDATE owner_session SET revocation_pending=false WHERE id=$1")
                    .bind(id)
                    .execute(&self.pool)
                    .await?;
            }
        }
        Ok(())
    }
}
/// Clock rollback fails closed rather than silently granting more than one verification window.
fn needs_revalidation(last_verified: i64, now: i64) -> bool {
    now < last_verified || now.saturating_sub(last_verified) >= 24 * 60 * 60
}
#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    /// Distinguishes the verification deadline from idle or absolute session expiry.
    #[test]
    fn revalidation_has_a_strict_24_hour_boundary() {
        assert_eq!(
            [
                needs_revalidation(/*last_verified*/ 100, /*now*/ 100),
                needs_revalidation(/*last_verified*/ 100, /*now*/ 86499),
                needs_revalidation(/*last_verified*/ 100, /*now*/ 86500),
                needs_revalidation(/*last_verified*/ 100, i64::MAX),
                needs_revalidation(/*last_verified*/ 100, /*now*/ 99)
            ],
            [false, false, true, true, true]
        );
    }
}
