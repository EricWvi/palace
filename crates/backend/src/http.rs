mod access_tokens;
mod annotations;
mod article_import;
mod articles;
mod business;
mod contract;
mod conversation_list;
mod dto;
mod routes;
pub use contract::api_contract;
mod error;
#[cfg(feature = "test-server")]
mod fixed_user;
mod path_management;
#[cfg(feature = "test-server")]
pub use fixed_user::fixed_user_router;
mod handlers;
mod jotting_reading;
mod jottings;
mod media;
mod notes;
mod security;
mod sleep;
mod sync;
#[cfg(test)]
mod tests;
mod timeline;
mod todo_lists;
mod todos;
mod web_pages;

use crate::{LoginRedirect, OidcProvider};
use axum::{Router, extract::DefaultBodyLimit};
use chrono_tz::Tz;
use error::ApiError;
use handlers::*;
use palace_db::{CredentialKey, Database, IdentityProvider, IdentityTokens, ProviderError};
use palace_domain::{ImportLimits, SourceLinks};
use palace_jottings::JottingService;
use palace_media::MediaStorage;
use palace_notes::NoteLibrary;
use std::{future::Future, sync::Arc};
use tower_http::compression::CompressionLayer;

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
    pub notes: Arc<NoteLibrary>,
    pub media: MediaStorage,
    pub jottings: JottingService,
    /// The deployment's zone: a bare article publish day starts at its midnight.
    pub timezone: Tz,
    pub now: fn() -> i64,
}
/// Builds authenticated JSON/file endpoints with bounded bodies and no client-supplied owner scope.
pub fn router<P: LoginProvider + 'static>(mut server: Server<P>) -> Router {
    // Only business handlers use object storage; authentication keeps no handle to it.
    let media = std::mem::replace(&mut server.media, MediaStorage::Unconfigured);
    let jottings = std::mem::replace(&mut server.jottings, JottingService::Unconfigured);
    let business = business_router(BusinessServer {
        database: server.database.clone(),
        links: server.links.clone(),
        limits: server.limits,
        notes: server.notes.clone(),
        media,
        jottings,
        timezone: server.timezone,
    });
    let tokens = access_token_router(server.database.clone());
    let server = Arc::new(server);
    let business = business.route_layer(axum::middleware::from_fn_with_state(
        server.clone(),
        authenticate_request::<P>,
    ));
    let tokens = tokens.route_layer(axum::middleware::from_fn_with_state(
        server.clone(),
        authenticate_session_request::<P>,
    ));
    macro_rules! mount {
        ($( $method:ident $path:ident => $module:ident::$handler:ident, )*) => {
            Router::new()$(.route(routes::$path, axum::routing::$method($module::$handler::<P>)))*
        };
    }
    routes::auth_routes!(mount)
        .with_state(server)
        .merge(business)
        .merge(tokens)
}

/// Builds token management without authentication; each deployment mode layers its own on top.
fn access_token_router(database: Database) -> Router {
    macro_rules! mount {
        ($( $method:ident $path:ident => $module:ident::$handler:ident, )*) => {
            Router::new()$(.route(routes::$path, axum::routing::$method($module::$handler)))*
        };
    }
    routes::access_token_routes!(mount).with_state(database)
}

/// Business handlers depend on an already resolved owner, never on a login provider.
struct BusinessServer {
    database: Database,
    links: SourceLinks,
    limits: ImportLimits,
    notes: Arc<NoteLibrary>,
    media: MediaStorage,
    jottings: JottingService,
    timezone: Tz,
}

/// Shares the complete business API between authenticated deployment and local single-user testing.
fn business_router(server: BusinessServer) -> Router {
    // JSON requests wrap the original history text in another string; escaping can expand it sixfold.
    // The shared parser and multipart reader still enforce the original history byte limit.
    let limit = server
        .limits
        .bytes
        .saturating_mul(6)
        .saturating_add(64 * 1024);
    macro_rules! mount {
        ($( $method:ident $path:ident => $module:ident::$handler:ident, )*) => {
            Router::new()$(.route(routes::$path, axum::routing::$method($module::$handler)))*
        };
    }
    // A multipart envelope around one file stays well within 64 KiB of overhead. Without
    // storage the handlers answer before reading any body, so the envelope alone is enough.
    let media_limit = match &server.media {
        MediaStorage::Configured { limits, .. } => limits.max_bytes.saturating_add(64 * 1024),
        MediaStorage::Unconfigured => 64 * 1024,
    };
    let media = routes::media_routes!(mount).layer(DefaultBodyLimit::max(media_limit));
    // Without storage an import can only carry its `article` part.
    let article_limit = match &server.media {
        MediaStorage::Configured { limits, .. } => limits.article_max_bytes,
        MediaStorage::Unconfigured => article_import::ARTICLE_PART_BYTES.saturating_add(64 * 1024),
    };
    let article_import =
        routes::article_import_routes!(mount).layer(DefaultBodyLimit::max(article_limit));
    // Timeline and conversation JSON carries whole chat histories and compresses well; the
    // layer only answers gzip when the client offers it and skips tiny bodies.
    routes::business_routes!(mount)
        .layer(DefaultBodyLimit::max(limit))
        .layer(CompressionLayer::new())
        .merge(media)
        .merge(article_import)
        .with_state(Arc::new(server))
}
