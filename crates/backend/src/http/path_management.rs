use super::{ApiError, BusinessServer};
use axum::{
    Json,
    extract::{Path, State},
    response::{IntoResponse, Response},
};
use palace_domain::{
    ImportRequest, ImportTarget, InputError, InputErrorKind, PathInput, SessionId,
};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct NewPath {
    session_id: String,
    history: String,
    occurred_at: i64,
    idempotency_key: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct UpdatePath {
    history: String,
    occurred_at: i64,
    idempotency_key: String,
}
/// Branch creation derives title and source exclusively from the owner-scoped conversation.
pub(super) async fn create(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path(conversation_id): Path<Uuid>,
    body: Result<Json<NewPath>, axum::extract::rejection::JsonRejection>,
) -> Result<Response, ApiError> {
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
    Ok(Json(server.database.import_path(owner.scope(), &input).await?).into_response())
}
/// Updates keep the session identity fixed and validate the full historical prefix in the transaction.
pub(super) async fn update(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path((conversation_id, path_id)): Path<(Uuid, Uuid)>,
    body: Result<Json<UpdatePath>, axum::extract::rejection::JsonRejection>,
) -> Result<Response, ApiError> {
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
    Ok(Json(server.database.import_path(owner.scope(), &input).await?).into_response())
}
/// Deletes only one source path while preserving shared ancestors and other paths.
pub(super) async fn delete(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path((conversation_id, path_id)): Path<(Uuid, Uuid)>,
) -> Result<Response, ApiError> {
    server
        .database
        .delete_path(owner.scope(), conversation_id, path_id)
        .await?;
    Ok(Json(serde_json::json!({"id":path_id})).into_response())
}
/// Deletes the card and all branches atomically inside the authenticated owner's scope.
pub(super) async fn delete_conversation(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path(id): Path<Uuid>,
) -> Result<Response, ApiError> {
    server
        .database
        .delete_conversation(owner.scope(), id)
        .await?;
    Ok(Json(serde_json::json!({"id":id})).into_response())
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
