use super::{ApiError, BusinessServer};
use axum::{
    Json,
    extract::{Multipart, Path, State, rejection::JsonRejection},
    response::{IntoResponse, Response},
};
use palace_domain::{ImportInput, ImportRequest, InputError, InputErrorKind, Source};
use serde::Deserialize;
use std::sync::Arc;
use uuid::Uuid;

/// Returns the owner supplied by the selected server authentication boundary.
pub(super) async fn me(
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
) -> Json<palace_db::Owner> {
    Json(owner)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TextImport {
    title: String,
    occurred_at: i64,
    source: Source,
    session_id: String,
    history: String,
    idempotency_key: String,
}
/// Adapts pasted JSON to the shared byte parser; owner fields are not part of this protocol.
pub(super) async fn import_text(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    body: Result<Json<TextImport>, JsonRejection>,
) -> Result<Response, ApiError> {
    let Json(body) = body.map_err(|e| {
        InputError::new(
            if e.status() == axum::http::StatusCode::PAYLOAD_TOO_LARGE {
                InputErrorKind::Limit
            } else {
                InputErrorKind::Field
            },
            "request",
            e.body_text(),
        )
    })?;
    let request = ImportRequest::parse(
        ImportInput {
            occurred_at: body.occurred_at,
            title: body.title,
            source: body.source,
            session_id: body.session_id,
            history: body.history.into_bytes(),
            idempotency_key: body.idempotency_key,
        },
        server.limits,
    )?;
    let result = server.database.import_path(owner.scope(), &request).await?;
    Ok(Json(result).into_response())
}
/// Accumulates a bounded file body and sends precisely the same bytes through the common parser.
pub(super) async fn import_file(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    mut multipart: Multipart,
) -> Result<Response, ApiError> {
    let mut fields = std::collections::HashMap::new();
    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|e| InputError::new(InputErrorKind::Field, "multipart", e.to_string()))?
    {
        let name = field
            .name()
            .ok_or_else(|| InputError::new(InputErrorKind::Field, "multipart", "unnamed field"))?
            .to_owned();
        if ![
            "occurred_at",
            "title",
            "source",
            "session_id",
            "history",
            "idempotency_key",
        ]
        .contains(&name.as_str())
            || fields.contains_key(&name)
        {
            return Err(
                InputError::new(InputErrorKind::Field, name, "unknown or repeated field").into(),
            );
        }
        let max = if name == "history" {
            server.limits.bytes
        } else {
            1024
        };
        let mut bytes = Vec::new();
        while let Some(chunk) = field
            .chunk()
            .await
            .map_err(|e| InputError::new(InputErrorKind::Limit, &name, e.to_string()))?
        {
            if bytes.len().saturating_add(chunk.len()) > max {
                return Err(
                    InputError::new(InputErrorKind::Limit, &name, "field too large").into(),
                );
            }
            bytes.extend_from_slice(&chunk);
        }
        fields.insert(name, bytes);
    }
    let history = fields
        .remove("history")
        .ok_or_else(|| InputError::new(InputErrorKind::Field, "history", "missing field"))?;
    let mut text = |name: &str| -> Result<String, ApiError> {
        String::from_utf8(
            fields
                .remove(name)
                .ok_or_else(|| InputError::new(InputErrorKind::Field, name, "missing field"))?,
        )
        .map_err(|_| InputError::new(InputErrorKind::Field, name, "expected UTF-8").into())
    };
    let source = serde_json::from_value(serde_json::Value::String(text("source")?))
        .map_err(|_| InputError::new(InputErrorKind::Field, "source", "unsupported source"))?;
    let request = ImportRequest::parse(
        ImportInput {
            occurred_at: text("occurred_at")?.parse().map_err(|_| {
                InputError::new(
                    InputErrorKind::Field,
                    "occurred_at",
                    "expected epoch milliseconds",
                )
            })?,
            title: text("title")?,
            source,
            session_id: text("session_id")?,
            history,
            idempotency_key: text("idempotency_key")?,
        },
        server.limits,
    )?;
    let result = server.database.import_path(owner.scope(), &request).await?;
    Ok(Json(result).into_response())
}
/// Returns a stable tree and controlled external link only for the authenticated owner.
pub(super) async fn conversation(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path(id): Path<Uuid>,
) -> Result<Response, ApiError> {
    let detail = server
        .database
        .conversation_detail(owner.scope(), id)
        .await?;
    let paths = detail
        .paths
        .iter()
        .map(|path| {
            let mut value = serde_json::to_value(path).map_err(|_| ApiError::Internal)?;
            value["original_link"] = serde_json::json!(
                server
                    .links
                    .original_link(detail.conversation.source, &path.session_id)?
                    .as_str()
            );
            Ok(value)
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    Ok(Json(serde_json::json!({"conversation":detail.conversation,"messages":detail.messages,"paths":paths})).into_response())
}
/// Exposes one validated ancestor path without inferring turns or role alternation.
pub(super) async fn path(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path((id, path_id)): Path<(Uuid, Uuid)>,
) -> Result<Response, ApiError> {
    let detail = server
        .database
        .conversation_detail(owner.scope(), id)
        .await?;
    let path = detail
        .paths
        .iter()
        .find(|path| path.id == path_id)
        .ok_or(ApiError::NotFound)?;
    let messages =
        palace_domain::read_path(&detail.conversation, &detail.messages, path.head_message_id)?;
    Ok(Json(messages).into_response())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ConversationMetadata {
    title: String,
    source: Source,
}
/// Replaces editable card metadata without exposing tree or owner reassignment.
pub(super) async fn update_conversation(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path(id): Path<Uuid>,
    body: Result<Json<ConversationMetadata>, JsonRejection>,
) -> Result<Response, ApiError> {
    let Json(input) =
        body.map_err(|error| InputError::new(InputErrorKind::Field, "request", error.body_text()))?;
    server
        .database
        .update_conversation_metadata(owner.scope(), id, &input.title, input.source)
        .await?;
    Ok(
        Json(serde_json::json!({"id":id,"title":input.title,"source":input.source}))
            .into_response(),
    )
}

/// Lists each conversation once, ordered by its newest user-selected conversation occurrence time.
pub(super) async fn conversations(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
) -> Result<Response, ApiError> {
    Ok(Json(server.database.list_conversations(owner.scope()).await?).into_response())
}
