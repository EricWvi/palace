//! 摘星 articles after import: the list, the reading page, edits and deletion.
use super::{ApiError, BusinessServer, dto, path_management::json_error, routes};
use axum::{
    Json,
    extract::{Path, Query, State, rejection::JsonRejection, rejection::QueryRejection},
    http::StatusCode,
};
use palace_db::{ArticleListRequest, ListCursor, SearchTerm};
use palace_domain::{ArticleMetadata, ArticleMetadataInput, InputError, InputErrorKind};
use palace_media::MediaStorage;
use std::sync::Arc;
use uuid::Uuid;

/// Lists the owner's articles for 摘星, latest publish date first, one page at a time.
#[utoipa::path(
    get, path = routes::ARTICLES, operation_id = "articles",
    params(dto::ArticleListQuery),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "Up to 50 articles ordered by publish date, then id, both descending", body = dto::ArticlePage, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn list(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    query: Result<Query<dto::ArticleListQuery>, QueryRejection>,
) -> Result<Json<dto::ArticlePage>, ApiError> {
    let Query(query) = query
        .map_err(|error| InputError::new(InputErrorKind::Field, "query", error.body_text()))?;
    let request = ArticleListRequest {
        search: query.q.as_deref().and_then(SearchTerm::parse),
        after: query
            .cursor
            .as_deref()
            .map(str::parse::<ListCursor>)
            .transpose()?,
    };
    let page = server
        .database
        .article_list(owner.scope(), &request)
        .await?;
    Ok(Json(dto::ArticlePage::new(page, server.timezone)))
}

/// Opens one of the owner's articles.
#[utoipa::path(
    get, path = routes::ARTICLE, operation_id = "article",
    params(("id" = Uuid, Path, description = "Owner-scoped article id")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "Successful operation", body = dto::Article, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; native extractor text", body = String, content_type = "text/plain"),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "Resource unavailable in owner scope", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn article(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path(id): Path<Uuid>,
) -> Result<Json<dto::Article>, ApiError> {
    let article = server.database.article(owner.scope(), id).await?;
    Ok(Json(dto::Article::new(article, server.timezone)))
}

/// Replaces everything but the body; the moment keeps its import time.
#[utoipa::path(
    put, path = routes::ARTICLE, operation_id = "update_article",
    request_body = dto::ArticleUpdate,
    params(("id" = Uuid, Path, description = "Owner-scoped article id"), ("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "Updated", body = dto::Article, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "Resource unavailable in owner scope", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 409, description = "Another article of the owner has this URL; nothing changed", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 413, description = "Input capacity exceeded", body = dto::InputErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn update(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path(id): Path<Uuid>,
    body: Result<Json<dto::ArticleUpdate>, JsonRejection>,
) -> Result<Json<dto::Article>, ApiError> {
    let Json(input) = body.map_err(json_error)?;
    let metadata = ArticleMetadata::parse(
        ArticleMetadataInput {
            title: input.title,
            url: input.url,
            author: input.author,
            date: input.date,
        },
        server.timezone,
    )?;
    let article = server
        .database
        .update_article(owner.scope(), id, &metadata)
        .await?;
    Ok(Json(dto::Article::new(article, server.timezone)))
}

/// Deletes the article with its media: objects first, then every row in one transaction.
#[utoipa::path(
    delete, path = routes::ARTICLE, operation_id = "delete_article",
    params(("id" = Uuid, Path, description = "Owner-scoped article id"), ("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 204, description = "Article, moment and media deleted", headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; native extractor text", body = String, content_type = "text/plain"),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "Resource unavailable in owner scope", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure; the article is kept and the delete can be retried", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider or media storage unavailable; the article is kept and the delete can be retried", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn delete(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let store = match &server.media {
        MediaStorage::Configured { store, .. } => Some(store),
        MediaStorage::Unconfigured => None,
    };
    palace_media::delete_article(&server.database, store, owner.scope(), id).await?;
    Ok(StatusCode::NO_CONTENT)
}
