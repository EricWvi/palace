//! Article import: one multipart request carries the article and every file it embeds, so the
//! article and its media appear together or not at all.
use super::{ApiError, BusinessServer, dto, media::read_field, routes};
use axum::{
    Json,
    extract::{Multipart, State},
    http::StatusCode,
};
use palace_db::ArticleImport;
use palace_domain::{
    ArticleBody, ArticleMetadata, ArticleMetadataInput, InputError, InputErrorKind, MediaLimits,
    MediaName, MediaUpload,
};
use palace_media::{ArticleStaging, MediaStorage, S3Bucket};
use std::sync::Arc;

/// The `article` part holds a body of at most 4 MiB as a JSON string; escaping can grow it
/// sixfold, and the other fields are small.
pub(super) const ARTICLE_PART_BYTES: usize = 4 * 1024 * 1024 * 6 + 64 * 1024;

/// Saves an article with its embedded files. A URL the owner already has returns that article
/// without reading the files or changing anything.
#[utoipa::path(
    post, path = routes::ARTICLES, operation_id = "import_article",
    request_body(content = dto::ArticleImportForm, content_type = "multipart/form-data"),
    params(("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "The owner already has this URL; the existing article is returned unchanged", body = dto::Article, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 201, description = "Imported", body = dto::Article, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 413, description = "A file, the body or the whole import exceeds its limit", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 500, description = "Internal persistence or response failure; nothing was saved", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider or media storage unavailable, including files sent without configured media storage; nothing was saved", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn import(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<dto::Article>), ApiError> {
    let first = multipart.next_field().await.map_err(multipart_error)?;
    let Some(mut field) = first.filter(|field| field.name() == Some("article")) else {
        return Err(InputError::new(
            InputErrorKind::Field,
            "article",
            "the first part must be article",
        )
        .into());
    };
    let fields: dto::ArticleImportFields =
        serde_json::from_slice(&read_field(&mut field, ARTICLE_PART_BYTES, "article").await?)
            .map_err(|error| {
                InputError::new(InputErrorKind::Field, "article", error.to_string())
            })?;
    drop(field);
    let metadata = ArticleMetadata::parse(
        ArticleMetadataInput {
            title: fields.title,
            url: fields.url,
            author: fields.author,
            date: fields.date,
        },
        server.timezone,
    )?;
    let body = ArticleBody::parse(fields.content)?;
    // A retry finds the article before any file is read, so nothing is stored twice.
    if let Some(existing) = server
        .database
        .article_by_url(owner.scope(), metadata.url())
        .await?
    {
        return Ok((
            StatusCode::OK,
            Json(dto::Article::new(existing, server.timezone)),
        ));
    }
    let (store, limits) = match &server.media {
        MediaStorage::Configured { store, limits, .. } => (Some(store), Some(*limits)),
        MediaStorage::Unconfigured => (None, None),
    };
    let mut staging = ArticleStaging::new(&server.database, store, owner.scope());
    if let Err(error) = stage_files(&mut multipart, &mut staging, limits).await {
        staging.discard().await;
        return Err(error);
    }
    Ok(match staging.commit(&metadata, body).await? {
        ArticleImport::Created(article) => (
            StatusCode::CREATED,
            Json(dto::Article::new(article, server.timezone)),
        ),
        ArticleImport::Existing(article) => (
            StatusCode::OK,
            Json(dto::Article::new(article, server.timezone)),
        ),
    })
}

/// Stores each `file` part as it arrives, so only one file is held in memory at a time.
async fn stage_files(
    multipart: &mut Multipart,
    staging: &mut ArticleStaging<'_, S3Bucket>,
    limits: Option<MediaLimits>,
) -> Result<(), ApiError> {
    while let Some(mut field) = multipart.next_field().await.map_err(multipart_error)? {
        if field.name() != Some("file") {
            let name = field.name().unwrap_or("multipart").to_owned();
            return Err(
                InputError::new(InputErrorKind::Field, name, "unknown or repeated field").into(),
            );
        }
        let limits = limits.ok_or(ApiError::MediaUnavailable)?;
        let name = MediaName::parse(field.file_name().unwrap_or_default())?;
        let content_type = field.content_type().map(str::to_owned);
        let bytes = read_field(&mut field, limits.max_bytes, "file").await?;
        let upload = MediaUpload::new(bytes, content_type.as_deref(), Some(name.as_str()), limits)?;
        staging.stage(name, upload).await?;
    }
    Ok(())
}

/// A malformed envelope, or one cut off by the import's body limit.
fn multipart_error(error: axum::extract::multipart::MultipartError) -> ApiError {
    let kind = if error.status() == StatusCode::PAYLOAD_TOO_LARGE {
        InputErrorKind::Limit
    } else {
        InputErrorKind::Field
    };
    InputError::new(kind, "multipart", error.body_text()).into()
}
