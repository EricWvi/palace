//! Access token management. These routes are mounted behind browser-session authentication
//! only, so a leaked token can neither mint another token nor delete the owner's others.
use super::{ApiError, dto, routes};
use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
};
use palace_db::Database;
use palace_domain::{AccessTokenNote, InputError, InputErrorKind};
use uuid::Uuid;

/// Lists the owner's tokens newest first, each only in its masked form.
#[utoipa::path(
    get, path = routes::ACCESS_TOKENS, operation_id = "list_access_tokens",
    security(("session" = [])),
    responses(
        (status = 200, description = "Successful operation", body = Vec<dto::AccessToken>, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Called with an access token instead of a browser session", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn list(
    State(database): State<Database>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
) -> Result<Json<Vec<dto::AccessToken>>, ApiError> {
    let tokens = database.access_tokens(owner.scope()).await?;
    Ok(Json(tokens.into_iter().map(Into::into).collect()))
}

/// Issues a token under the session's identity; the response is the only place its full value appears.
#[utoipa::path(
    post, path = routes::ACCESS_TOKENS, operation_id = "create_access_token",
    request_body = dto::AccessTokenRequest,
    params(("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected")),
    security(("session" = [])),
    responses(
        (status = 201, description = "Created", body = dto::CreatedAccessToken, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error", body = dto::InputErrorResponse, content_type = "application/json"),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected, or called with an access token instead of a browser session", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 413, description = "Note longer than 100 characters", body = dto::InputErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn create(
    State(database): State<Database>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    body: Result<Json<dto::AccessTokenRequest>, JsonRejection>,
) -> Result<(StatusCode, Json<dto::CreatedAccessToken>), ApiError> {
    let Json(body) =
        body.map_err(|e| InputError::new(InputErrorKind::Field, "request", e.body_text()))?;
    let note = AccessTokenNote::parse(&body.note)?;
    let issued = database.create_access_token(&owner, &note).await?;
    Ok((
        StatusCode::CREATED,
        Json(dto::CreatedAccessToken {
            access_token: issued.token.into(),
            token: issued.secret.expose().to_owned(),
        }),
    ))
}

/// Revokes a token immediately by deleting it.
#[utoipa::path(
    delete, path = routes::ACCESS_TOKEN, operation_id = "delete_access_token",
    params(("id" = Uuid, Path, description = "Owner-scoped access token id"), ("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected")),
    security(("session" = [])),
    responses(
        (status = 204, description = "Deleted; requests with the token now fail", headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; native extractor text", body = String, content_type = "text/plain"),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected, or called with an access token instead of a browser session", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "Resource unavailable in owner scope", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn delete(
    State(database): State<Database>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    database.delete_access_token(owner.scope(), id).await?;
    Ok(StatusCode::NO_CONTENT)
}
