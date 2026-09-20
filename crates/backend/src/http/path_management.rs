use super::{ApiError, BusinessServer, dto, routes};
use axum::{
    Json,
    extract::{Path, State},
};
use palace_domain::{
    ImportRequest, ImportTarget, InputError, InputErrorKind, PathInput, SessionId,
};
use std::sync::Arc;
use uuid::Uuid;

/// Branch creation derives title and source exclusively from the owner-scoped conversation.
#[utoipa::path(
    post, path = routes::PATHS, operation_id = "create",
    request_body = dto::NewPath,
    params(("id" = Uuid, Path, description = "Owner-scoped UUID"), ("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected")),
    security(("session" = [])),
    responses(
        (status = 200, description = "Successful operation", body = dto::ImportResult, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "Resource unavailable in owner scope", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 409, description = "Identity, source session or idempotency conflict", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 413, description = "Input capacity exceeded", body = dto::InputErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn create(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path(conversation_id): Path<Uuid>,
    body: Result<Json<dto::NewPath>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<dto::ImportResult>, ApiError> {
    let Json(body) = body.map_err(json_error)?;
    let input = ImportRequest::parse_target(
        ImportTarget::Branch {
            conversation_id,
            session_id: SessionId::try_from(body.session_id)?,
        },
        PathInput {
            history: body.history.into_bytes(),
            occurred_at: body.occurred_at,
            idempotency_key: body.idempotency_key,
        },
        server.limits,
    )?;
    Ok(Json(
        server
            .database
            .import_path(owner.scope(), &input)
            .await?
            .into(),
    ))
}
/// Updates keep the session identity fixed and validate the full historical prefix in the transaction.
#[utoipa::path(
    put, path = routes::PATH, operation_id = "update",
    request_body = dto::UpdatePath,
    params(("id" = Uuid, Path, description = "Owner-scoped UUID"), ("path_id" = Uuid, Path, description = "Owner-scoped UUID"), ("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected")),
    security(("session" = [])),
    responses(
        (status = 200, description = "Successful operation", body = dto::ImportResult, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "Resource unavailable in owner scope", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 409, description = "Identity, source session or idempotency conflict", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 413, description = "Input capacity exceeded", body = dto::InputErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn update(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path((conversation_id, path_id)): Path<(Uuid, Uuid)>,
    body: Result<Json<dto::UpdatePath>, axum::extract::rejection::JsonRejection>,
) -> Result<Json<dto::ImportResult>, ApiError> {
    let Json(body) = body.map_err(json_error)?;
    let input = ImportRequest::parse_target(
        ImportTarget::Update {
            conversation_id,
            path_id,
        },
        PathInput {
            history: body.history.into_bytes(),
            occurred_at: body.occurred_at,
            idempotency_key: body.idempotency_key,
        },
        server.limits,
    )?;
    Ok(Json(
        server
            .database
            .import_path(owner.scope(), &input)
            .await?
            .into(),
    ))
}
/// Deletes only one source path while preserving shared ancestors and other paths.
#[utoipa::path(
    delete, path = routes::PATH, operation_id = "delete",
    params(("id" = Uuid, Path, description = "Owner-scoped UUID"), ("path_id" = Uuid, Path, description = "Owner-scoped UUID"), ("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected")),
    security(("session" = [])),
    responses(
        (status = 200, description = "Successful operation", body = dto::Deleted, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", body = String, content_type = "text/plain"),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "Resource unavailable in owner scope", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 409, description = "Identity, source session or idempotency conflict", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn delete(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path((conversation_id, path_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<dto::Deleted>, ApiError> {
    server
        .database
        .delete_path(owner.scope(), conversation_id, path_id)
        .await?;
    Ok(Json(dto::Deleted { id: path_id }))
}
/// Deletes the card and all branches atomically inside the authenticated owner's scope.
#[utoipa::path(
    delete, path = routes::CONVERSATION, operation_id = "delete_conversation",
    params(("id" = Uuid, Path, description = "Owner-scoped UUID"), ("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected")),
    security(("session" = [])),
    responses(
        (status = 200, description = "Successful operation", body = dto::Deleted, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", body = String, content_type = "text/plain"),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "Resource unavailable in owner scope", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 409, description = "Identity, source session or idempotency conflict", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn delete_conversation(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path(id): Path<Uuid>,
) -> Result<Json<dto::Deleted>, ApiError> {
    server
        .database
        .delete_conversation(owner.scope(), id)
        .await?;
    Ok(Json(dto::Deleted { id }))
}
/// Gives unknown metadata fields the same structured rejection as other import validation errors.
fn json_error(error: axum::extract::rejection::JsonRejection) -> InputError {
    InputError::new(
        if error.status() == axum::http::StatusCode::PAYLOAD_TOO_LARGE {
            InputErrorKind::Limit
        } else {
            InputErrorKind::Field
        },
        "request",
        error.body_text(),
    )
}
