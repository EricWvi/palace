//! Owner access tokens for CLI and service callers. Tokens are security state outside the sync
//! domain, so these writes never take the publication lock.
use crate::{Database, DbError, Owner, OwnerScope};
use aes_gcm::aead::{OsRng, rand_core::RngCore};
use palace_domain::{AccessTokenNote, AccessTokenSecret};
use sqlx::Row;
use uuid::Uuid;

/// A token as it may be listed: the full value never leaves the database layer after issue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccessToken {
    pub id: Uuid,
    pub note: String,
    pub masked: String,
    /// Unix epoch milliseconds assigned by the database clock.
    pub created_at: i64,
    /// Unix epoch milliseconds of the last recorded use, at most a minute stale.
    pub last_used_at: Option<i64>,
}

/// A freshly created token together with its only copy of the full value.
pub struct IssuedAccessToken {
    pub token: AccessToken,
    pub secret: AccessTokenSecret,
}

const COLUMNS: &str = "id,note,masked,(extract(epoch FROM created_at)*1000)::bigint AS created_at,(extract(epoch FROM last_used_at)*1000)::bigint AS last_used_at";

impl Database {
    /// Issues a token bound to the owner and the identity that authenticated the request.
    pub async fn create_access_token(
        &self,
        owner: &Owner,
        note: &AccessTokenNote,
    ) -> Result<IssuedAccessToken, DbError> {
        let mut random = [0; 32];
        OsRng.fill_bytes(&mut random);
        let secret = AccessTokenSecret::from_random(random);
        let row = sqlx::query(&format!("INSERT INTO owner_access_token(id,owner_id,owner_identity_id,note,secret_hash,masked) VALUES($1,$2,$3,$4,$5,$6) RETURNING {COLUMNS}"))
            .bind(Uuid::now_v7())
            .bind(owner.id)
            .bind(owner.identity_id)
            .bind(note.as_str())
            .bind(secret.digest())
            .bind(secret.masked())
            .fetch_one(&self.pool)
            .await?;
        Ok(IssuedAccessToken {
            token: access_token(&row)?,
            secret,
        })
    }

    /// Lists the owner's tokens, newest first.
    pub async fn access_tokens(&self, owner: OwnerScope) -> Result<Vec<AccessToken>, DbError> {
        sqlx::query(&format!("SELECT {COLUMNS} FROM owner_access_token WHERE owner_id=$1 ORDER BY created_at DESC, id DESC"))
            .bind(owner.id())
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(access_token)
            .collect()
    }

    /// Revokes a token by forgetting it; ids of other owners are indistinguishable from absent ones.
    pub async fn delete_access_token(&self, owner: OwnerScope, id: Uuid) -> Result<(), DbError> {
        let deleted = sqlx::query("DELETE FROM owner_access_token WHERE owner_id=$1 AND id=$2")
            .bind(owner.id())
            .bind(id)
            .execute(&self.pool)
            .await?;
        if deleted.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }
        Ok(())
    }

    /// Resolves a token to its owner, reading disablement live so revocation has no cache window.
    /// Returns `None` for unknown tokens and disabled owners or identities alike, so callers
    /// cannot tell the holder why it was refused.
    pub async fn authenticate_access_token(
        &self,
        secret: &AccessTokenSecret,
    ) -> Result<Option<Owner>, DbError> {
        // Recording use at most once a minute keeps a burst of CLI calls from writing every time.
        let row = sqlx::query(
            "WITH hit AS (
                SELECT t.id,t.owner_id,t.owner_identity_id,o.email FROM owner_access_token t
                JOIN owner o ON o.id=t.owner_id
                JOIN owner_identity i ON i.id=t.owner_identity_id AND i.owner_id=t.owner_id
                WHERE t.secret_hash=$1 AND NOT o.disabled AND NOT i.disabled
            ), touch AS (
                UPDATE owner_access_token t SET last_used_at=date_trunc('milliseconds',now())
                FROM hit WHERE t.id=hit.id
                    AND (t.last_used_at IS NULL OR t.last_used_at < now() - interval '60 seconds')
            )
            SELECT owner_id,owner_identity_id,email FROM hit",
        )
        .bind(secret.digest())
        .fetch_optional(&self.pool)
        .await?;
        row.map(|row| {
            Ok(Owner {
                id: row.try_get("owner_id")?,
                email: row.try_get("email")?,
                identity_id: row.try_get("owner_identity_id")?,
            })
        })
        .transpose()
    }
}

/// Maps a row selected with [`COLUMNS`].
fn access_token(row: &sqlx::postgres::PgRow) -> Result<AccessToken, DbError> {
    Ok(AccessToken {
        id: row.try_get("id")?,
        note: row.try_get("note")?,
        masked: row.try_get("masked")?,
        created_at: row.try_get("created_at")?,
        last_used_at: row.try_get("last_used_at")?,
    })
}
