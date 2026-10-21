//! Reader annotations on article bodies and conversation messages.
use super::{ApiError, BusinessServer, dto, path_management::json_error, routes};
use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
};
use palace_domain::{AnnotationAnchor, AnnotationContent, AnnotationTarget};
use std::sync::Arc;
use uuid::Uuid;

/// Lists an article's annotations, oldest first.
#[utoipa::path(
    get, path = routes::ARTICLE_ANNOTATIONS, operation_id = "article_annotations",
    params(("id" = Uuid, Path, description = "Owner-scoped article id")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "Successful operation", body = dto::AnnotationList, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
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
) -> Result<Json<dto::AnnotationList>, ApiError> {
    let annotations = server
        .database
        .article_annotations(owner.scope(), id)
        .await?;
    Ok(Json(annotations.into()))
}

/// Annotates the article body.
#[utoipa::path(
    post, path = routes::ARTICLE_ANNOTATIONS, operation_id = "create_article_annotation",
    request_body = dto::ArticleAnnotationCreate,
    params(("id" = Uuid, Path, description = "Owner-scoped article id"), ("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 201, description = "Created", body = dto::Annotation, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "Resource unavailable in owner scope", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 413, description = "Input capacity exceeded", body = dto::InputErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn article_create(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path(id): Path<Uuid>,
    body: Result<Json<dto::ArticleAnnotationCreate>, JsonRejection>,
) -> Result<(StatusCode, Json<dto::Annotation>), ApiError> {
    let Json(input) = body.map_err(json_error)?;
    let anchor = AnnotationAnchor::parse(input.start, input.end, input.quote)?;
    let content = AnnotationContent::parse(input.style.map(Into::into), input.note)?;
    let annotation = server
        .database
        .insert_annotation(
            owner.scope(),
            AnnotationTarget::Article(id),
            &anchor,
            &content,
        )
        .await?;
    Ok((StatusCode::CREATED, Json(annotation.into())))
}

/// Lists the annotations on every message of a conversation, whichever paths share them.
#[utoipa::path(
    get, path = routes::CONVERSATION_ANNOTATIONS, operation_id = "conversation_annotations",
    params(("id" = Uuid, Path, description = "Owner-scoped conversation id")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "Successful operation", body = dto::AnnotationList, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; native extractor text", body = String, content_type = "text/plain"),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "Resource unavailable in owner scope", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn conversation_list(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path(id): Path<Uuid>,
) -> Result<Json<dto::AnnotationList>, ApiError> {
    let annotations = server
        .database
        .conversation_annotations(owner.scope(), id)
        .await?;
    Ok(Json(annotations.into()))
}

/// Annotates one message of the conversation.
#[utoipa::path(
    post, path = routes::CONVERSATION_ANNOTATIONS, operation_id = "create_message_annotation",
    request_body = dto::MessageAnnotationCreate,
    params(("id" = Uuid, Path, description = "Owner-scoped conversation id"), ("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 201, description = "Created", body = dto::Annotation, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "Conversation or message unavailable in owner scope", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 413, description = "Input capacity exceeded", body = dto::InputErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn conversation_create(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path(id): Path<Uuid>,
    body: Result<Json<dto::MessageAnnotationCreate>, JsonRejection>,
) -> Result<(StatusCode, Json<dto::Annotation>), ApiError> {
    let Json(input) = body.map_err(json_error)?;
    let anchor = AnnotationAnchor::parse(input.start, input.end, input.quote)?;
    let content = AnnotationContent::parse(input.style.map(Into::into), input.note)?;
    let annotation = server
        .database
        .insert_annotation(
            owner.scope(),
            AnnotationTarget::Message {
                conversation: id,
                message: input.message_id,
            },
            &anchor,
            &content,
        )
        .await?;
    Ok((StatusCode::CREATED, Json(annotation.into())))
}

/// Replaces an annotation's style and note together.
#[utoipa::path(
    put, path = routes::ANNOTATION, operation_id = "update_annotation",
    request_body = dto::AnnotationUpdate,
    params(("id" = Uuid, Path, description = "Owner-scoped annotation id"), ("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "Updated", body = dto::Annotation, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "Resource unavailable in owner scope", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 413, description = "Input capacity exceeded", body = dto::InputErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn update(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path(id): Path<Uuid>,
    body: Result<Json<dto::AnnotationUpdate>, JsonRejection>,
) -> Result<Json<dto::Annotation>, ApiError> {
    let Json(input) = body.map_err(json_error)?;
    let content = AnnotationContent::parse(input.style.map(Into::into), input.note)?;
    let annotation = server
        .database
        .update_annotation(owner.scope(), id, &content)
        .await?;
    Ok(Json(annotation.into()))
}

/// Deletes an annotation.
#[utoipa::path(
    delete, path = routes::ANNOTATION, operation_id = "delete_annotation",
    params(("id" = Uuid, Path, description = "Owner-scoped annotation id"), ("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
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
    server.database.delete_annotation(owner.scope(), id).await?;
    Ok(StatusCode::NO_CONTENT)
}
