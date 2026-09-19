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
    let (conversation, messages) = server.database.conversation(owner.scope(), id).await?;
    let link = server
        .links
        .original_link(conversation.source, &conversation.session_id)?;
    Ok(Json(
            serde_json::json!({"conversation":conversation,"messages":messages,"original_link":link.as_str()}),
        ).into_response())
}
/// Exposes one validated ancestor path without inferring turns or role alternation.
pub(super) async fn path(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path((id, head)): Path<(Uuid, Uuid)>,
) -> Result<Response, ApiError> {
    let path = server.database.path(owner.scope(), id, head).await?;
    Ok(Json(path).into_response())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Rename {
    title: String,
}
/// Updates a title without exposing source, parent or owner reassignment through the request body.
pub(super) async fn rename(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path(id): Path<Uuid>,
    Json(input): Json<Rename>,
) -> Result<Response, ApiError> {
    server
        .database
        .rename_conversation(owner.scope(), id, &input.title)
        .await?;
    Ok(Json(serde_json::json!({"id":id,"title":input.title})).into_response())
}

/// Lists each conversation once, ordered by its newest user-selected conversation occurrence time.
pub(super) async fn conversations(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
) -> Result<Response, ApiError> {
    Ok(Json(server.database.list_conversations(owner.scope()).await?).into_response())
}
