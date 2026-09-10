mod error;
mod handlers;
mod security;
#[cfg(test)]
mod tests;

use crate::{LoginRedirect, OidcProvider};
use axum::{
    Router,
    extract::DefaultBodyLimit,
    routing::{get, post},
};
use error::ApiError;
use handlers::*;
use palace_db::{CredentialKey, Database, IdentityProvider, IdentityTokens, ProviderError};
use palace_domain::{ImportLimits, SourceLinks};
use std::{future::Future, sync::Arc};

/// Separates HTTP authentication orchestration from actual Authelia transport for focused tests.
pub trait LoginProvider: IdentityProvider {
    /// Creates state, nonce and PKCE challenge for an OIDC redirect.
    fn authorize(&self) -> Result<LoginRedirect, ProviderError>;
    /// Exchanges a code after the server has consumed its browser-bound proof.
    fn callback(
        &self,
        code: String,
        proof: &str,
    ) -> impl Future<Output = Result<IdentityTokens, ProviderError>> + Send;
}
impl LoginProvider for OidcProvider {
    /// Delegates authorization parameters to the verified OIDC client.
    fn authorize(&self) -> Result<LoginRedirect, ProviderError> {
        self.authorize()
    }
    /// Delegates token and claim verification to the OIDC client.
    async fn callback(&self, code: String, proof: &str) -> Result<IdentityTokens, ProviderError> {
        self.callback(code, proof).await
    }
}
/// Explicit process dependencies; the server clock and provider can be replaced without environment mutation.
pub struct Server<P> {
    pub database: Database,
    pub provider: P,
    pub credential_key: CredentialKey,
    pub origin: String,
    pub links: SourceLinks,
    pub limits: ImportLimits,
    pub now: fn() -> i64,
}
/// Builds authenticated JSON/file endpoints with bounded bodies and no client-supplied owner scope.
pub fn router<P: LoginProvider + 'static>(server: Server<P>) -> Router {
    let limit = server.limits.bytes.saturating_add(64 * 1024);
    Router::new()
        .route(
            "/",
            get(|| async { axum::response::Redirect::to("/api/me") }),
        )
        .route("/auth/login", get(login::<P>))
        .route("/auth/callback", get(callback::<P>))
        .route("/auth/logout", post(logout::<P>))
        .route("/auth/logout-all", post(logout_all::<P>))
        .route("/api/me", get(me::<P>))
        .route("/api/import", post(import_text::<P>))
        .route("/api/import/file", post(import_file::<P>))
        .route("/api/conversations/{id}", get(conversation::<P>))
        .route("/api/conversations/{id}/paths/{head}", get(path::<P>))
        .layer(DefaultBodyLimit::max(limit))
        .with_state(Arc::new(server))
}
