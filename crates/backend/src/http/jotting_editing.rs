//! Web edits and deletions of jottings: the server rewrites or removes the jotting's file in the
//! work tree and publishes at once; the agent commits it later. See
//! `specs/decisions/server/jotting/20261101-edit-and-delete-jottings-from-the-web-in-the-work-tree-until-committed.md`.
use super::{
    ApiError, BusinessServer, dto, jotting_reading::configured, path_management::json_error, routes,
};
use axum::{
    Json,
    extract::{Path, Query, State, rejection::JsonRejection, rejection::QueryRejection},
    http::StatusCode,
};
use palace_domain::{InputError, InputErrorKind};
use std::sync::Arc;
use uuid::Uuid;

/// Sets a jotting's title, tags and body, keeping every other byte of its file.
#[utoipa::path(
    put, path = routes::JOTTING, operation_id = "edit_jotting",
    request_body = dto::JottingEdit,
    params(("id" = Uuid, Path, description = "Jotting id"), ("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "Written and published, or unchanged", body = dto::JottingEdited, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "No jotting ever had this id", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 409, description = "`jotting_changed`: changed or deleted since read; `jotting_moved`: its file is not where it was", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 413, description = "Input capacity exceeded: extractor text or body limit", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 500, description = "Internal persistence or file failure; the edit can be retried", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider unavailable, or no jottings folder configured", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn edit(
    State(server): State<Arc<BusinessServer>>,
    Path(id): Path<Uuid>,
    body: Result<Json<dto::JottingEdit>, JsonRejection>,
) -> Result<Json<dto::JottingEdited>, ApiError> {
    let Json(input) = body.map_err(json_error)?;
    let version = input.version.0;
    let version = configured(&server)?.edit(id, version, input.into()).await?;
    Ok(Json(dto::JottingEdited {
        version: dto::Cursor(version),
    }))
}

/// Deletes a jotting's file and publishes the deletion.
#[utoipa::path(
    delete, path = routes::JOTTING, operation_id = "delete_jotting",
    params(("id" = Uuid, Path, description = "Jotting id"), dto::JottingDeleteQuery, ("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 204, description = "Deleted and published", headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "No jotting ever had this id", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 409, description = "`jotting_changed`: changed or deleted since read; `jotting_moved`: its file is not where it was", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or file failure; the deletion can be retried", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider unavailable, or no jottings folder configured", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn delete(
    State(server): State<Arc<BusinessServer>>,
    Path(id): Path<Uuid>,
    query: Result<Query<dto::JottingDeleteQuery>, QueryRejection>,
) -> Result<StatusCode, ApiError> {
    let Query(query) = query
        .map_err(|error| InputError::new(InputErrorKind::Field, "query", error.body_text()))?;
    configured(&server)?.delete(id, query.version.0).await?;
    Ok(StatusCode::NO_CONTENT)
}
