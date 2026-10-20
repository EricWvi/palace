use super::{
    ApiError, LoginProvider, Server, dto, routes,
    security::{LOGIN_COOKIE, SESSION_COOKIE, bearer, check_origin, cookie, set_cookie},
};
use axum::{
    Json,
    extract::{Query, Request, State},
    http::{HeaderMap, header},
    response::{IntoResponse, Redirect, Response},
};
use palace_db::{AuthenticatedSession, RevokeScope};
use std::sync::Arc;

/// Reads the persisted session before a handler can establish its owner scope.
pub(super) async fn authenticate<P: LoginProvider>(
    server: &Server<P>,
    headers: &HeaderMap,
) -> Result<AuthenticatedSession, ApiError> {
    let secret = cookie(headers, SESSION_COOKIE)?.ok_or(ApiError::Unauthorized)?;
    Ok(server
        .database
        .authenticate(
            &secret,
            &server.credential_key,
            &server.provider,
            (server.now)(),
        )
        .await?)
}
/// Adds rolling persistence and prevents intermediaries from caching authenticated data.
pub(super) fn authenticated_response(
    session: &AuthenticatedSession,
    body: impl IntoResponse,
) -> Result<Response, ApiError> {
    let mut response = body.into_response();
    response.headers_mut().append(
        header::SET_COOKIE,
        set_cookie(SESSION_COOKIE, &session.secret, 180 * 24 * 60 * 60)?,
    );
    // A handler that chose its own caching (a media redirect) keeps it; nothing else is cached.
    response
        .headers_mut()
        .entry(header::CACHE_CONTROL)
        .or_insert(header::HeaderValue::from_static("no-store"));
    Ok(response)
}
/// Starts a persisted, browser-bound Authorization Code Flow.
#[utoipa::path(
    get, path = routes::LOGIN, operation_id = "login",
    security(),
    responses(
        (status = 303, description = "Browser redirect; empty body", headers(("Location" = String, description = "Identity provider authorization URL"), ("Set-Cookie" = String, description = "Secure HttpOnly browser credential"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 409, description = "Identity, source session or idempotency conflict", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn login<P: LoginProvider>(
    State(server): State<Arc<Server<P>>>,
) -> Result<Response, ApiError> {
    let login = server.provider.authorize()?;
    let browser = openidconnect::CsrfToken::new_random().secret().clone();
    server
        .database
        .begin_login(
            &login.state,
            &browser,
            &login.proof,
            &server.credential_key,
            (server.now)(),
        )
        .await?;
    let mut response = Redirect::to(login.url.as_str()).into_response();
    response.headers_mut().append(
        header::SET_COOKIE,
        set_cookie(LOGIN_COOKIE, &browser, /*max_age*/ 600)?,
    );
    Ok(response)
}
/// Consumes state before exchanging the code; successful login replaces any previous browser session.
#[utoipa::path(
    get, path = routes::CALLBACK, operation_id = "callback",
    params(dto::Callback),
    security(("login_flow" = [])),
    responses(
        (status = 303, description = "Login completed; empty body", headers(("Location" = String, description = "/"), ("Set-Cookie" = String, description = "Sets session cookie and clears login cookie"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", body = String, content_type = "text/plain"),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 409, description = "Identity, source session or idempotency conflict", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn callback<P: LoginProvider>(
    State(server): State<Arc<Server<P>>>,
    headers: HeaderMap,
    Query(query): Query<dto::Callback>,
) -> Result<Response, ApiError> {
    let browser = cookie(&headers, LOGIN_COOKIE)?.ok_or(ApiError::Unauthorized)?;
    let proof = server
        .database
        .consume_login(
            &query.state,
            &browser,
            &server.credential_key,
            (server.now)(),
        )
        .await?;
    let tokens = server.provider.callback(query.code, &proof).await?;
    let previous = cookie(&headers, SESSION_COOKIE)?;
    let session = server
        .database
        .create_session(
            tokens,
            &server.credential_key,
            (server.now)(),
            previous.as_deref(),
        )
        .await?;
    let mut response = authenticated_response(&session, Redirect::to("/"))?;
    response.headers_mut().append(
        header::SET_COOKIE,
        set_cookie(LOGIN_COOKIE, "", /*max_age*/ 0)?,
    );
    Ok(response)
}
/// Persists current-device logout before attempting external provider revocation.
#[utoipa::path(
    post, path = routes::LOGOUT, operation_id = "logout",
    params(("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected")),
    security(("session" = [])),
    responses(
        (status = 200, description = "Successful operation", body = dto::LogoutResponse, headers(("Set-Cookie" = String, description = "Clears the browser session cookie"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn logout<P: LoginProvider>(
    State(server): State<Arc<Server<P>>>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    revoke(&server, headers, RevokeScope::Current).await
}
/// Uses the same authenticated boundary to revoke all devices owned by this user.
#[utoipa::path(
    post, path = routes::LOGOUT_ALL, operation_id = "logout_all",
    params(("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected")),
    security(("session" = [])),
    responses(
        (status = 200, description = "Successful operation", body = dto::LogoutResponse, headers(("Set-Cookie" = String, description = "Clears the browser session cookie"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn logout_all<P: LoginProvider>(
    State(server): State<Arc<Server<P>>>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    revoke(&server, headers, RevokeScope::AllDevices).await
}
/// Clears the browser credential even if the external revocation endpoint is unavailable.
async fn revoke<P: LoginProvider>(
    server: &Server<P>,
    headers: HeaderMap,
    scope: RevokeScope,
) -> Result<Response, ApiError> {
    check_origin(&headers, &server.origin)?;
    let secret = cookie(&headers, SESSION_COOKIE)?.ok_or(ApiError::Unauthorized)?;
    server
        .database
        .logout_browser(&secret, scope, (server.now)())
        .await?;
    let _ = server
        .database
        .retry_revocations(&server.credential_key, &server.provider)
        .await;
    let mut response = Json(dto::LogoutResponse { logged_out: true }).into_response();
    response.headers_mut().append(
        header::SET_COOKIE,
        set_cookie(SESSION_COOKIE, "", /*max_age*/ 0)?,
    );
    Ok(response)
}

/// Resolves an access token or, without an Authorization header, the browser session before
/// business handlers receive a trusted owner.
pub(super) async fn authenticate_request<P: LoginProvider + 'static>(
    State(server): State<Arc<Server<P>>>,
    mut request: Request,
    next: axum::middleware::Next,
) -> Result<Response, ApiError> {
    let Some(secret) = bearer(request.headers())? else {
        return session_request(&server, request, next).await;
    };
    // Origin checks defend the ambient cookie against CSRF; browsers never attach Authorization
    // on their own, and CLI callers send no Origin at all.
    let owner = server
        .database
        .authenticate_access_token(&secret)
        .await?
        .ok_or(ApiError::Unauthorized)?;
    request.extensions_mut().insert(owner);
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .entry(header::CACHE_CONTROL)
        .or_insert(header::HeaderValue::from_static("no-store"));
    Ok(response)
}
/// Admits only the browser session, for routes a leaked access token must never reach.
pub(super) async fn authenticate_session_request<P: LoginProvider + 'static>(
    State(server): State<Arc<Server<P>>>,
    request: Request,
    next: axum::middleware::Next,
) -> Result<Response, ApiError> {
    if request.headers().contains_key(header::AUTHORIZATION) {
        return Err(ApiError::SessionRequired);
    }
    session_request(&server, request, next).await
}
/// Applies the session's CSRF check, rolling cookie and owner scope to one request.
async fn session_request<P: LoginProvider>(
    server: &Server<P>,
    mut request: Request,
    next: axum::middleware::Next,
) -> Result<Response, ApiError> {
    if !request.method().is_safe() {
        check_origin(request.headers(), &server.origin)?;
    }
    let session = authenticate(server, request.headers()).await?;
    request.extensions_mut().insert(session.owner.clone());
    authenticated_response(&session, next.run(request).await)
}
