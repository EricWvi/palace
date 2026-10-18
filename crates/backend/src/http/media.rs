//! Media upload and deletion. Without configured object storage both answer 503 before reading
//! the body, so a deployment without S3 serves everything else unchanged.
use super::{ApiError, BusinessServer, dto, routes};
use axum::{
    Json,
    extract::{Multipart, Path, State},
    http::StatusCode,
};
use palace_domain::{InputError, InputErrorKind, MediaUpload};
use palace_media::MediaStorage;
use std::sync::Arc;
use uuid::Uuid;

/// Stores one file and returns its server-generated identity; the object is written before the row.
#[utoipa::path(
    post, path = routes::MEDIA, operation_id = "upload_media",
    request_body(content = dto::MediaUploadForm, content_type = "multipart/form-data"),
    params(("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected")),
    security(("session" = [])),
    responses(
        (status = 201, description = "Stored", body = dto::Media, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 413, description = "File exceeds the deployment limit", body = dto::InputErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider or media storage unavailable, including media storage not configured", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn upload(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<dto::Media>), ApiError> {
    let MediaStorage::Configured { store, limits } = &server.media else {
        return Err(ApiError::MediaUnavailable);
    };
    let mut file = None;
    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|e| InputError::new(InputErrorKind::Field, "multipart", e.to_string()))?
    {
        if field.name() != Some("file") || file.is_some() {
            let name = field.name().unwrap_or("multipart").to_owned();
            return Err(
                InputError::new(InputErrorKind::Field, name, "unknown or repeated field").into(),
            );
        }
        let content_type = field.content_type().map(str::to_owned);
        let file_name = field.file_name().map(str::to_owned);
        // Stop buffering as soon as the limit is crossed instead of after the whole body arrives.
        let mut bytes = Vec::new();
        while let Some(chunk) = field
            .chunk()
            .await
            .map_err(|e| InputError::new(InputErrorKind::Limit, "file", e.to_string()))?
        {
            if bytes.len().saturating_add(chunk.len()) > limits.max_bytes {
                return Err(
                    InputError::new(InputErrorKind::Limit, "file", "file too large").into(),
                );
            }
            bytes.extend_from_slice(&chunk);
        }
        file = Some((bytes, content_type, file_name));
    }
    let (bytes, content_type, file_name) =
        file.ok_or_else(|| InputError::new(InputErrorKind::Field, "file", "missing field"))?;
    let form = dto::MediaUploadForm { file: bytes };
    let upload = MediaUpload::new(
        form.file,
        content_type.as_deref(),
        file_name.as_deref(),
        *limits,
    )?;
    let media = palace_media::upload(&server.database, store, owner.scope(), upload).await?;
    Ok((StatusCode::CREATED, Json(media.into())))
}

/// Deletes the object, then the row; success means the stored bytes are gone.
#[utoipa::path(
    delete, path = routes::MEDIA_ITEM, operation_id = "delete_media",
    params(("id" = Uuid, Path, description = "Owner-scoped media id"), ("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected")),
    security(("session" = [])),
    responses(
        (status = 204, description = "Object and record deleted", headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; native extractor text", body = String, content_type = "text/plain"),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "Resource unavailable in owner scope", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure; the media is kept and the delete can be retried", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider or media storage unavailable, including media storage not configured; the media is kept and the delete can be retried", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn delete(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let MediaStorage::Configured { store, .. } = &server.media else {
        return Err(ApiError::MediaUnavailable);
    };
    palace_media::delete(&server.database, store, owner.scope(), id).await?;
    Ok(StatusCode::NO_CONTENT)
}
