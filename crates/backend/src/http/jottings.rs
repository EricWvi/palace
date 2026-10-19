//! 札记 sync for offline clients: a feed of changes and uploads of new jottings. Jottings belong
//! to the deployment, so these handlers take no owner scope; authentication still applies
//! through the business router's layer.
use super::{ApiError, BusinessServer, dto, routes};
use axum::{
    Json,
    extract::{Query, State, rejection::QueryRejection},
    http::StatusCode,
};
use palace_domain::{InputError, InputErrorKind};
use palace_jottings::{JottingService, Uploaded};
use std::sync::Arc;

// Rows per page when the client does not ask for a size.
const DEFAULT_LIMIT: u32 = 200;

/// Accepts a jotting created offline: writes it to the inbox and publishes it, or recognizes a
/// retry of the same upload.
#[utoipa::path(
    post, path = routes::JOTTINGS, operation_id = "upload_jotting",
    request_body = dto::JottingUpload,
    params(("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 201, description = "Written to the inbox and published", body = dto::UploadedJotting, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 200, description = "The same upload was already accepted; nothing was written", body = dto::UploadedJotting, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 409, description = "The id already belongs to a different jotting", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 413, description = "Input capacity exceeded: extractor text or body limit", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 415, description = "JSON Content-Type required", body = String, content_type = "text/plain"),
        (status = 422, description = "JSON does not match the target type", body = String, content_type = "text/plain"),
        (status = 500, description = "Internal persistence or file failure; the upload can be retried", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider unavailable, or no jottings folder configured", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn upload(
    State(server): State<Arc<BusinessServer>>,
    Json(upload): Json<dto::JottingUpload>,
) -> Result<(StatusCode, Json<dto::UploadedJotting>), ApiError> {
    let JottingService::Configured(jottings) = &server.jottings else {
        return Err(ApiError::JottingsUnavailable);
    };
    let id = upload.id;
    let (status, version) = match jottings.upload(upload.into()).await? {
        Uploaded::Created(version) => (StatusCode::CREATED, version),
        Uploaded::Retried(version) => (StatusCode::OK, version),
    };
    Ok((
        status,
        Json(dto::UploadedJotting {
            id,
            version: dto::Cursor(version),
        }),
    ))
}

/// Returns the current state of every jotting changed after `since`, in version order.
#[utoipa::path(
    get, path = routes::JOTTING_CHANGES, operation_id = "jotting_changes",
    params(dto::JottingChangesQuery),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "Changes in version order; empty with `next` equal to `since` when nothing changed or no folder is configured", body = dto::JottingChanges, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn changes(
    State(server): State<Arc<BusinessServer>>,
    query: Result<Query<dto::JottingChangesQuery>, QueryRejection>,
) -> Result<Json<dto::JottingChanges>, ApiError> {
    let Query(query) = query
        .map_err(|error| InputError::new(InputErrorKind::Field, "query", error.body_text()))?;
    let since = query.since.0;
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT);
    let page = match &server.jottings {
        JottingService::Configured(jottings) => jottings.changes(since, limit).await?,
        JottingService::Unconfigured => palace_db::JottingChanges {
            changes: vec![],
            next: since,
            has_more: false,
        },
    };
    Ok(Json(page.into()))
}
