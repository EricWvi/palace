mod business;
mod error;
#[cfg(feature = "test-server")]
mod fixed_user;
#[cfg(feature = "test-server")]
pub use fixed_user::fixed_user_router;
mod handlers;
mod security;
mod sync;
#[cfg(test)]
mod tests;

use crate::{LoginRedirect, OidcProvider};
use axum::{
    Router,
    extract::DefaultBodyLimit,
    routing::{get, post, put},
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
    let business = business_router(BusinessServer {
        database: server.database.clone(),
        links: server.links.clone(),
        limits: server.limits,
    });
    let server = Arc::new(server);
    let business = business.route_layer(axum::middleware::from_fn_with_state(
        server.clone(),
        authenticate_request::<P>,
    ));
    Router::new()
        .route("/auth/login", get(login::<P>))
        .route("/auth/callback", get(callback::<P>))
        .route("/auth/logout", post(logout::<P>))
        .route("/auth/logout-all", post(logout_all::<P>))
        .with_state(server)
        .merge(business)
}

/// Business handlers depend on an already resolved owner, never on a login provider.
struct BusinessServer {
    database: Database,
    links: SourceLinks,
    limits: ImportLimits,
}

/// Shares the complete business API between authenticated deployment and local single-user testing.
fn business_router(server: BusinessServer) -> Router {
    let limit = server.limits.bytes.saturating_add(64 * 1024);
    Router::new()
        .route("/api/me", get(business::me))
        .route("/api/sync", get(sync::pull).post(sync::upload))
        .route("/api/import", post(business::import_text))
        .route("/api/import/file", post(business::import_file))
        .route("/api/conversations", get(business::conversations))
        .route("/api/conversations/{id}", get(business::conversation))
        .route("/api/conversations/{id}/title", put(business::rename))
        .route("/api/conversations/{id}/paths/{head}", get(business::path))
        .layer(DefaultBodyLimit::max(limit))
        .with_state(Arc::new(server))
}
