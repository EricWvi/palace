//! Web pages the browser extension annotates, their binding to articles, and the migration of
//! their annotations into those articles.
use super::{ApiError, BusinessServer, dto, path_management::json_error, routes};
use axum::{
    Json,
    extract::{Path, Query, State, rejection::JsonRejection, rejection::QueryRejection},
    http::StatusCode,
};
use palace_domain::{
    AnnotationAnchor, AnnotationContent, InputError, InputErrorKind, WebAnchor, WebPageTitle,
    WebPageUrl,
};
use std::sync::Arc;
use uuid::Uuid;

/// Lists every web page of the owner, most recently changed first.
#[utoipa::path(
    get, path = routes::WEB_PAGES, operation_id = "web_pages",
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "Successful operation", body = dto::WebPageList, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn list(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
) -> Result<Json<dto::WebPageList>, ApiError> {
    let pages = server.database.web_pages(owner.scope()).await?;
    Ok(Json(pages.into()))
}

/// Opens the web page at exactly this address, with the annotations still on it.
#[utoipa::path(
    get, path = routes::WEB_PAGE_LOOKUP, operation_id = "web_page",
    params(dto::WebPageLookup),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "Successful operation", body = dto::WebPageDetail, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error", body = dto::InputErrorResponse, content_type = "application/json"),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "No page at this address in owner scope", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn lookup(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    query: Result<Query<dto::WebPageLookup>, QueryRejection>,
) -> Result<Json<dto::WebPageDetail>, ApiError> {
    let Query(query) = query
        .map_err(|error| InputError::new(InputErrorKind::Field, "query", error.body_text()))?;
    let url = WebPageUrl::parse(query.url)?;
    let page = server.database.web_page_by_url(owner.scope(), &url).await?;
    Ok(Json(page.into()))
}

/// Annotates a web page, creating the page with its first annotation.
#[utoipa::path(
    post, path = routes::WEB_PAGE_ANNOTATIONS, operation_id = "create_web_annotation",
    request_body = dto::WebAnnotationCreate,
    params(("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 201, description = "Created", body = dto::WebAnnotation, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 409, description = "The page is bound to an article", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 413, description = "Input capacity exceeded", body = dto::InputErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn create(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    body: Result<Json<dto::WebAnnotationCreate>, JsonRejection>,
) -> Result<(StatusCode, Json<dto::WebAnnotation>), ApiError> {
    let Json(input) = body.map_err(json_error)?;
    let url = WebPageUrl::parse(input.url)?;
    let title = WebPageTitle::new(&input.title, &url);
    let anchor = WebAnchor::parse(input.quote, input.prefix, input.suffix, input.position)?;
    let content = AnnotationContent::parse(input.style.map(Into::into), input.note)?;
    let annotation = server
        .database
        .insert_web_annotation(owner.scope(), &url, &title, &anchor, &content)
        .await?;
    Ok((StatusCode::CREATED, Json(annotation.into())))
}

/// Replaces a web annotation's style and note together; the last write wins.
#[utoipa::path(
    put, path = routes::WEB_ANNOTATION, operation_id = "update_web_annotation",
    request_body = dto::WebAnnotationUpdate,
    params(("id" = Uuid, Path, description = "Owner-scoped web annotation id"), ("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "Updated", body = dto::WebAnnotation, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "Resource unavailable in owner scope", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 409, description = "The page is bound to an article", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 413, description = "Input capacity exceeded", body = dto::InputErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn update(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path(id): Path<Uuid>,
    body: Result<Json<dto::WebAnnotationUpdate>, JsonRejection>,
) -> Result<Json<dto::WebAnnotation>, ApiError> {
    let Json(input) = body.map_err(json_error)?;
    let content = AnnotationContent::parse(input.style.map(Into::into), input.note)?;
    let annotation = server
        .database
        .update_web_annotation(owner.scope(), id, &content, &input.title)
        .await?;
    Ok(Json(annotation.into()))
}

/// Deletes a web annotation; an unbound page goes with its last one.
#[utoipa::path(
    delete, path = routes::WEB_ANNOTATION, operation_id = "delete_web_annotation",
    params(("id" = Uuid, Path, description = "Owner-scoped web annotation id"), ("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 204, description = "Deleted", headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; native extractor text", body = String, content_type = "text/plain"),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "Resource unavailable in owner scope", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn delete(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    server
        .database
        .delete_web_annotation(owner.scope(), id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Binds a web page to an article or moves its binding; the page stops taking annotations.
#[utoipa::path(
    put, path = routes::WEB_PAGE_BINDING, operation_id = "bind_web_page",
    request_body = dto::WebPageBinding,
    params(("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "Bound", body = dto::WebPage, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "Article unavailable in owner scope", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 413, description = "Input capacity exceeded", body = dto::InputErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn bind(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    body: Result<Json<dto::WebPageBinding>, JsonRejection>,
) -> Result<Json<dto::WebPage>, ApiError> {
    let Json(input) = body.map_err(json_error)?;
    let url = WebPageUrl::parse(input.url)?;
    let title = WebPageTitle::new(&input.title, &url);
    let page = server
        .database
        .bind_web_page(owner.scope(), &url, &title, input.article_id)
        .await?;
    Ok(Json(page.into()))
}

/// Lists the web annotations left on every page bound to the article.
#[utoipa::path(
    get, path = routes::ARTICLE_WEB_ANNOTATIONS, operation_id = "article_web_annotations",
    params(("id" = Uuid, Path, description = "Owner-scoped article id")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "Successful operation", body = dto::WebAnnotationList, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; native extractor text", body = String, content_type = "text/plain"),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "Resource unavailable in owner scope", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn article_list(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path(id): Path<Uuid>,
) -> Result<Json<dto::WebAnnotationList>, ApiError> {
    let annotations = server
        .database
        .article_web_annotations(owner.scope(), id)
        .await?;
    Ok(Json(annotations.into()))
}

/// Replaces located web annotations with article annotations in one transaction.
#[utoipa::path(
    post, path = routes::ARTICLE_WEB_ANNOTATION_MIGRATION, operation_id = "migrate_web_annotations",
    request_body = dto::WebAnnotationMigration,
    params(("id" = Uuid, Path, description = "Owner-scoped article id"), ("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "The article annotations created; items already migrated or not on a page bound to this article are skipped", body = dto::MigratedAnnotations, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared; nothing migrated", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "Resource unavailable in owner scope", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 413, description = "Input capacity exceeded", body = dto::InputErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn migrate(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path(id): Path<Uuid>,
    body: Result<Json<dto::WebAnnotationMigration>, JsonRejection>,
) -> Result<Json<dto::MigratedAnnotations>, ApiError> {
    let Json(input) = body.map_err(json_error)?;
    // Any invalid anchor refuses the whole request before anything is deleted.
    let items = input
        .items
        .into_iter()
        .map(|item| {
            AnnotationAnchor::parse(item.start, item.end, item.quote)
                .map(|anchor| (item.web_annotation_id, anchor))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let created = server
        .database
        .migrate_web_annotations(owner.scope(), id, &items)
        .await?;
    Ok(Json(created.into()))
}
