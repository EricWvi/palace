//! Offline contract assembly; operation metadata lives beside the real handlers.
use super::{
    access_tokens, annotations, article_import, articles, business, conversation_list, handlers,
    jotting_reading, jottings, media, notes, path_management, security, sleep, sync, timeline,
    todo_lists, todos, web_pages,
};
use utoipa::{
    OpenApi,
    openapi::security::{ApiKey, ApiKeyValue, HttpAuthScheme, HttpBuilder, SecurityScheme},
};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Palace HTTP API",
        version = "1.0.0",
        description = "Owner-scoped Palace API. Schema describes wire structure, not authorization or transactional guarantees. Body limits use injected ImportLimits (default raw history 8 MiB); HTTP limit is bytes * 6 + 64 KiB. Media uploads instead use the deployment media limit (default 50 MiB) + 64 KiB; article imports use the deployment article limit (default 256 MiB). UTF-8 byte limits, nonblank strings and canonical cursor upper bounds are enforced by the server. Native extractor errors may be text/plain. No deployment credentials or environment metadata are included."
    ),
    paths(
        business::me,
        business::import_text,
        business::import_file,
        timeline::timeline,
        timeline::outline,
        conversation_list::list,
        business::conversation,
        business::path,
        path_management::create,
        path_management::update,
        path_management::delete,
        path_management::delete_conversation,
        path_management::update_metadata,
        notes::list,
        notes::note,
        jottings::changes,
        jottings::upload,
        jotting_reading::list,
        jotting_reading::overview,
        jotting_reading::recall,
        articles::list,
        article_import::import,
        articles::article,
        articles::update,
        articles::delete,
        annotations::article_list,
        annotations::article_create,
        annotations::conversation_list,
        annotations::conversation_create,
        annotations::update,
        annotations::delete,
        web_pages::list,
        web_pages::lookup,
        web_pages::create,
        web_pages::update,
        web_pages::delete,
        web_pages::bind,
        web_pages::article_list,
        web_pages::migrate,
        sleep::running,
        sleep::start,
        sleep::end,
        sleep::nights,
        sleep::record,
        sleep::update,
        sleep::delete,
        todos::board,
        todos::create,
        todos::plan,
        todos::update,
        todos::delete,
        todos::body,
        todos::set_body,
        todos::done,
        todos::complete,
        todos::restore,
        todo_lists::create,
        todo_lists::update,
        todo_lists::delete,
        todo_lists::completed,
        todo_lists::clear_completed,
        media::upload,
        media::read,
        media::delete,
        access_tokens::list,
        access_tokens::create,
        access_tokens::delete,
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
    components.add_security_scheme("access_token", SecurityScheme::Http(HttpBuilder::new().scheme(HttpAuthScheme::Bearer).description(Some(
        "Owner access token `palace_pat_…` for CLI and service callers; created and deleted only through a browser session. Business operations only; sends no cookie and needs no Origin. Any Authorization header disables the session cookie for that request.",
    )).build()));
    document
}

#[cfg(test)]
mod tests;
