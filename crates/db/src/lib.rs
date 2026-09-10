//! PostgreSQL transactions enforce owner boundaries independently of HTTP inputs.
mod import;
mod login;
mod owner;
mod session;
mod session_crypto;
mod sync;
pub use session::{
    AuthenticatedSession, IdentityProvider, IdentityTokens, ProviderError, RevokeScope,
    SessionError,
};
pub use session_crypto::CredentialKey;

pub use import::ImportResult;
pub use owner::{Owner, OwnerScope};
use sqlx::{PgPool, Postgres, Transaction};

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("database operation failed")]
    Storage(#[from] sqlx::Error),
    #[error("database migration failed")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("identity or source conflicts with existing state")]
    Conflict,
    #[error("record is unavailable in this owner scope")]
    NotFound,
    #[error("owner or identity is disabled")]
    Disabled,
    #[error(transparent)]
    Input(#[from] palace_domain::InputError),
}
#[derive(Clone)]
pub struct Database {
    pool: PgPool,
}
impl Database {
    /// Opens the shared PostgreSQL pool and applies versioned schema migrations.
    pub async fn connect(url: &str) -> Result<Self, DbError> {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(/*max*/ 12)
            .connect(url)
            .await?;
        sqlx::migrate!().run(&pool).await?;
        Ok(Self { pool })
    }
    /// Begins the sole business publication transaction before reading or assigning versions.
    async fn begin_write(&self) -> Result<Transaction<'_, Postgres>, DbError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(734281001)")
            .execute(&mut *tx)
            .await?;
        Ok(tx)
    }
}
