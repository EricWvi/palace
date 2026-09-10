use crate::{Database, DbError};
use serde::{Deserialize, Serialize};
use sqlx::Row;
use uuid::Uuid;

/// A scope returned by server-side identity resolution, never deserialized from a business request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OwnerScope(pub(crate) Uuid);
impl OwnerScope {
    /// Exposes the resolved owner for explicit scoped storage and response metadata.
    pub fn id(self) -> Uuid {
        self.0
    }
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Owner {
    pub id: Uuid,
    pub email: String,
    pub identity_id: Uuid,
}
impl Owner {
    /// Converts a persisted identity result into a business scope.
    pub fn scope(&self) -> OwnerScope {
        OwnerScope(self.id)
    }
}
impl Database {
    /// Atomically resolves only a verified issuer/subject pair; email cannot claim an existing owner.
    pub async fn resolve_identity(
        &self,
        issuer: &str,
        subject: &str,
        email: &str,
    ) -> Result<Owner, DbError> {
        let normalized = normalize_email(email)?;
        if issuer.is_empty() || subject.is_empty() {
            return Err(DbError::Conflict);
        }
        let mut tx = self.pool.begin().await?;
        // Serialize identity allocation and email changes to make conflicts deterministic.
        sqlx::query("SELECT pg_advisory_xact_lock(734281002)")
            .execute(&mut *tx)
            .await?;
        let existing = sqlx::query("SELECT o.id, i.id AS identity_id, o.disabled OR i.disabled AS disabled FROM owner_identity i JOIN owner o ON o.id=i.owner_id WHERE i.issuer=$1 AND i.subject=$2").bind(issuer).bind(subject).fetch_optional(&mut *tx).await?;
        let (id, identity_id) = if let Some(row) = existing {
            if row.try_get::<bool, _>("disabled")? {
                return Err(DbError::Disabled);
            }
            (
                row.try_get::<Uuid, _>("id")?,
                row.try_get::<Uuid, _>("identity_id")?,
            )
        } else {
            (Uuid::new_v4(), Uuid::new_v4())
        };
        let conflict: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM owner WHERE normalized_email=$1 AND id<>$2 AND NOT disabled)").bind(&normalized).bind(id).fetch_one(&mut *tx).await?;
        if conflict {
            return Err(DbError::Conflict);
        }
        sqlx::query("INSERT INTO owner(id,email,normalized_email) VALUES($1,$2,$3) ON CONFLICT(id) DO UPDATE SET email=$2,normalized_email=$3,updated_at=now()")
            .bind(id).bind(email).bind(normalized).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO owner_identity(id,owner_id,issuer,subject) VALUES($1,$2,$3,$4) ON CONFLICT(issuer,subject) DO UPDATE SET last_seen_at=now()")
            .bind(identity_id).bind(id).bind(issuer).bind(subject).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(Owner {
            id,
            email: email.into(),
            identity_id,
        })
    }
}
/// Validates current email without substituting a historical value; normalization only detects conflicts.
pub(crate) fn normalize_email(email: &str) -> Result<String, DbError> {
    let value = email.trim();
    if !email_address::EmailAddress::is_valid(value)
        || value.chars().any(|c| c.is_whitespace() || c.is_control())
    {
        return Err(DbError::Conflict);
    }
    Ok(value.to_lowercase())
}
#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    /// Prevents missing or ambiguous email from reaching owner allocation.
    #[test]
    fn email_is_current_validated_and_case_insensitive() {
        assert_eq!(
            normalize_email(" User@Example.COM ").unwrap(),
            "user@example.com"
        );
        for value in [
            "",
            " ",
            "a",
            "@host",
            "a@",
            "a@@b",
            "a b@host",
            "a@b\nc",
            "a<>@example.com",
            "a@example..com",
            "a..b@example.com",
        ] {
            assert!(normalize_email(value).is_err());
        }
    }
}
