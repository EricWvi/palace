//! Offline contract assembly; operation metadata lives beside the real handlers.
use super::{business, handlers, path_management, security, sync, timeline};
use utoipa::{
    OpenApi,
    openapi::security::{ApiKey, ApiKeyValue, SecurityScheme},
};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Palace HTTP API",
        version = "1.0.0",
        description = "Owner-scoped Palace API. Schema describes wire structure, not authorization or transactional guarantees. Body limits use injected ImportLimits (default raw history 8 MiB); HTTP limit is bytes * 6 + 64 KiB. UTF-8 byte limits, nonblank strings and canonical cursor upper bounds are enforced by the server. Native extractor errors may be text/plain. No deployment credentials or environment metadata are included."
    ),
    paths(
        business::me,
        business::import_text,
        business::import_file,
        timeline::timeline,
        business::conversation,
        business::path,
        path_management::create,
        path_management::update,
        path_management::delete,
        path_management::delete_conversation,
        path_management::update_metadata,
        sync::upload,
        sync::pull,
        handlers::login,
        handlers::callback,
        handlers::logout,
        handlers::logout_all,
    )
)]
struct Api;

/// Builds the public wire contract without creating a server or connecting to external services.
pub fn api_contract() -> utoipa::openapi::OpenApi {
    let mut document = Api::openapi();
    let components = document.components.get_or_insert_with(Default::default);
    components.add_security_scheme("session", SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::with_description(
        security::SESSION_COOKIE,
        "Production browser session; Secure, HttpOnly, SameSite=Lax, Path=/. Business handlers receive a server-resolved owner. Local fixed-user mode is a development-only boundary.",
    ))));
    components.add_security_scheme("login_flow", SecurityScheme::ApiKey(ApiKey::Cookie(ApiKeyValue::with_description(
        security::LOGIN_COOKIE,
        "Browser-bound one-time login flow; validated together with query state before exchanging code.",
    ))));
    document
}

#[cfg(test)]
mod tests;
