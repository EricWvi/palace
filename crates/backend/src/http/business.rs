use super::{ApiError, BusinessServer, dto, routes};
use axum::{
    Json,
    extract::{Multipart, Path, State, rejection::JsonRejection},
};
use palace_domain::{ImportInput, ImportRequest, InputError, InputErrorKind};
use std::sync::Arc;
use uuid::Uuid;

/// Returns the owner supplied by the selected server authentication boundary.
#[utoipa::path(
    get, path = routes::ME, operation_id = "me",
    security(("session" = [])),
    responses(
        (status = 200, description = "Successful operation", body = dto::Owner, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 409, description = "Identity, source session or idempotency conflict", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn me(
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
) -> Json<dto::Owner> {
    Json(owner.into())
}
/// Adapts pasted JSON to the shared byte parser; owner fields are not part of this protocol.
#[utoipa::path(
    post, path = routes::IMPORT, operation_id = "import_text",
    request_body = dto::TextImport,
    params(("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected")),
    security(("session" = [])),
    responses(
        (status = 200, description = "Successful operation", body = dto::ImportResult, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", body = dto::InputErrorResponse, content_type = "application/json"),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "Resource unavailable in owner scope", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 409, description = "Identity, source session or idempotency conflict", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 413, description = "Input capacity exceeded", body = dto::InputErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn import_text(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    body: Result<Json<dto::TextImport>, JsonRejection>,
) -> Result<Json<dto::ImportResult>, ApiError> {
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
            source: body.source.into(),
            session_id: body.session_id,
            history: body.history.into_bytes(),
            idempotency_key: body.idempotency_key,
        },
        server.limits,
    )?;
    let result = server.database.import_path(owner.scope(), &request).await?;
    Ok(Json(result.into()))
}
/// Accumulates a bounded file body and sends precisely the same bytes through the common parser.
#[utoipa::path(
    post, path = routes::IMPORT_FILE, operation_id = "import_file",
    request_body(content = dto::FileImport, content_type = "multipart/form-data"),
    params(("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected")),
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
pub(super) async fn import_file(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    mut multipart: Multipart,
) -> Result<Json<dto::ImportResult>, ApiError> {
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
    let source: dto::Source = serde_json::from_value(serde_json::Value::String(text("source")?))
        .map_err(|_| InputError::new(InputErrorKind::Field, "source", "unsupported source"))?;
    let form = dto::FileImport {
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
    };
    let request = ImportRequest::parse(
        ImportInput {
            occurred_at: form.occurred_at,
            title: form.title,
            source: form.source.into(),
            session_id: form.session_id,
            history: form.history,
            idempotency_key: form.idempotency_key,
        },
        server.limits,
    )?;
    let result = server.database.import_path(owner.scope(), &request).await?;
    Ok(Json(result.into()))
}
/// Returns a stable tree and controlled external link only for the authenticated owner.
#[utoipa::path(
    get, path = routes::CONVERSATION, operation_id = "conversation",
    params(("id" = Uuid, Path, description = "Owner-scoped UUID")),
    security(("session" = [])),
    responses(
        (status = 200, description = "Successful operation", body = dto::ConversationDetail, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "Resource unavailable in owner scope", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 409, description = "Identity, source session or idempotency conflict", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn conversation(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path(id): Path<Uuid>,
) -> Result<Json<dto::ConversationDetail>, ApiError> {
    let detail = server
        .database
        .conversation_detail(owner.scope(), id)
        .await?;
    let paths = detail
        .paths
        .into_iter()
        .map(|path| {
            let original_link = server
                .links
                .original_link(detail.conversation.source, &path.session_id)?;
            Ok(dto::ConversationPath {
                id: path.id,
                title: path.title,
                session_id: path.session_id.into(),
                head_message_id: path.head_message_id,
                occurred_at: path.occurred_at,
                created_at: path.created_at,
                updated_at: path.updated_at,
                message_count: path.message_count,
                original_link: original_link.into(),
            })
        })
        .collect::<Result<Vec<_>, ApiError>>()?;
    Ok(Json(dto::ConversationDetail {
        conversation: detail.conversation.into(),
        messages: detail.messages.into_iter().map(Into::into).collect(),
        paths,
    }))
}
/// Exposes one validated ancestor path without inferring turns or role alternation.
#[utoipa::path(
    get, path = routes::PATH, operation_id = "path",
    params(("id" = Uuid, Path, description = "Owner-scoped UUID"), ("path_id" = Uuid, Path, description = "Owner-scoped UUID")),
    security(("session" = [])),
    responses(
        (status = 200, description = "Successful operation", body = [dto::Message], headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "Resource unavailable in owner scope", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 409, description = "Identity, source session or idempotency conflict", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn path(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path((id, path_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Vec<dto::Message>>, ApiError> {
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
    Ok(Json(messages.into_iter().map(Into::into).collect()))
}
