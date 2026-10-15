//! 摘星 notes. Notes belong to the deployment, not to an owner: every signed-in user reads the
//! same library, so these handlers take no owner scope. Authentication still applies through the
//! business router's layer.
use super::{ApiError, BusinessServer, dto, routes};
use axum::{
    Json,
    extract::{Path, Query, State, rejection::QueryRejection},
};
use palace_domain::{InputError, InputErrorKind};
use palace_notes::{NoteCursor, NoteListRequest, NoteSearch};
use std::sync::Arc;

/// Lists the notes for 摘星, latest date first, one page at a time.
#[utoipa::path(
    get, path = routes::NOTES, operation_id = "notes",
    params(dto::NoteListQuery),
    security(("session" = [])),
    responses(
        (status = 200, description = "Up to 50 notes ordered by date, then id, both descending", body = dto::NotePage, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 409, description = "Identity, source session or idempotency conflict", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider or note search temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn list(
    State(server): State<Arc<BusinessServer>>,
    query: Result<Query<dto::NoteListQuery>, QueryRejection>,
) -> Result<Json<dto::NotePage>, ApiError> {
    let Query(query) = query
        .map_err(|error| InputError::new(InputErrorKind::Field, "query", error.body_text()))?;
    let request = NoteListRequest {
        search: query.q.as_deref().and_then(NoteSearch::parse),
        after: query
            .cursor
            .as_deref()
            .map(str::parse::<NoteCursor>)
            .transpose()?,
    };
    Ok(Json(server.notes.list(&request).await?.into()))
}

/// Opens one note listed in the library.
#[utoipa::path(
    get, path = routes::NOTE, operation_id = "note",
    params(("id" = String, Path, description = "Note id; its `/` separators are percent-encoded as one segment")),
    security(("session" = [])),
    responses(
        (status = 200, description = "Successful operation", body = dto::NoteDetail, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "No such note in the library", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 409, description = "Identity, source session or idempotency conflict", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn note(
    State(server): State<Arc<BusinessServer>>,
    Path(id): Path<String>,
) -> Result<Json<dto::NoteDetail>, ApiError> {
    Ok(Json(server.notes.read(&id).await?.into()))
}
