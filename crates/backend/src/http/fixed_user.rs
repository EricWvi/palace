use super::{BusinessServer, business_router, error::ApiError, security::check_origin};
use axum::{
    Router,
    extract::{Request, State},
    http::header,
    middleware::{self, Next},
    response::Response,
};
use palace_db::{Database, Owner};
use palace_domain::{ImportLimits, SourceLinks};

/// Builds the local single-user API without OIDC, sessions, cookies or login routes.
/// The stable identity pair reuses its Owner across restarts of the persistent database.
pub async fn fixed_user_router(
    database: Database,
    origin: String,
) -> Result<Router, Box<dyn std::error::Error>> {
    let owner = database
        .resolve_identity("palace:local-test", "single-user", "local-test@palace.test")
        .await?;
    Ok(business_router(BusinessServer {
        database,
        links: SourceLinks::standard()?,
        limits: ImportLimits::default(),
    })
    .route_layer(middleware::from_fn_with_state((owner, origin), identify))
    .route(
        "/",
        axum::routing::get(|| async { axum::response::Redirect::to("/api/me") }),
    ))
}

/// Ignores client credentials and assigns every request to the configured local owner.
async fn identify(
    State((owner, origin)): State<(Owner, String)>,
    mut request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    if !request.method().is_safe() {
        check_origin(request.headers(), &origin)?;
    }
    request.extensions_mut().insert(owner);
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-store"),
    );
    Ok(response)
}
