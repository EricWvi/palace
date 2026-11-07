//! 总批: the one block of text on a whole article, conversation path or web page.
use super::{ApiError, BusinessServer, dto, path_management::json_error, routes};
use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
};
use palace_db::OverallNoteTarget;
use palace_domain::{OverallNoteText, WebPageTitle, WebPageUrl};
use std::sync::Arc;
use uuid::Uuid;

/// Opens an article's 总批.
#[utoipa::path(
    get, path = routes::ARTICLE_OVERALL_NOTE, operation_id = "article_overall_note",
    params(("id" = Uuid, Path, description = "Owner-scoped article id")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "Successful operation", body = dto::OverallNote, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
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
) -> Result<Json<dto::OverallNote>, ApiError> {
    let note = server
        .database
        .overall_note(owner.scope(), OverallNoteTarget::Article(id))
        .await?;
    Ok(Json(note.into()))
}

/// Replaces an article's 总批 if it is still at the version named.
#[utoipa::path(
    put, path = routes::ARTICLE_OVERALL_NOTE, operation_id = "save_article_overall_note",
    request_body = dto::OverallNoteSave,
    params(("id" = Uuid, Path, description = "Owner-scoped article id"), ("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "Saved", body = dto::OverallNote, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "Resource unavailable in owner scope", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 409, description = "The note changed since that version; nothing was written", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 413, description = "Input capacity exceeded", body = dto::InputErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn save_article(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path(id): Path<Uuid>,
    body: Result<Json<dto::OverallNoteSave>, JsonRejection>,
) -> Result<Json<dto::OverallNote>, ApiError> {
    save(&server, &owner, OverallNoteTarget::Article(id), body).await
}

/// Opens the 总批 of one path of a conversation.
#[utoipa::path(
    get, path = routes::PATH_OVERALL_NOTE, operation_id = "path_overall_note",
    params(("id" = Uuid, Path, description = "Owner-scoped conversation id"), ("path_id" = Uuid, Path, description = "Path of that conversation")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "Successful operation", body = dto::OverallNote, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; native extractor text", body = String, content_type = "text/plain"),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "Resource unavailable in owner scope", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn path(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path((conversation, path)): Path<(Uuid, Uuid)>,
) -> Result<Json<dto::OverallNote>, ApiError> {
    let note = server
        .database
        .overall_note(
            owner.scope(),
            OverallNoteTarget::Path { conversation, path },
        )
        .await?;
    Ok(Json(note.into()))
}

/// Replaces a path's 总批 if it is still at the version named.
#[utoipa::path(
    put, path = routes::PATH_OVERALL_NOTE, operation_id = "save_path_overall_note",
    request_body = dto::OverallNoteSave,
    params(("id" = Uuid, Path, description = "Owner-scoped conversation id"), ("path_id" = Uuid, Path, description = "Path of that conversation"), ("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "Saved", body = dto::OverallNote, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "Resource unavailable in owner scope", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 409, description = "The note changed since that version; nothing was written", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 413, description = "Input capacity exceeded", body = dto::InputErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn save_path(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path((conversation, path)): Path<(Uuid, Uuid)>,
    body: Result<Json<dto::OverallNoteSave>, JsonRejection>,
) -> Result<Json<dto::OverallNote>, ApiError> {
    let target = OverallNoteTarget::Path { conversation, path };
    save(&server, &owner, target, body).await
}

/// Replaces the 总批 of a web page, creating the page with its first note.
#[utoipa::path(
    put, path = routes::WEB_PAGE_OVERALL_NOTE, operation_id = "save_web_page_overall_note",
    request_body = dto::WebPageOverallNoteSave,
    params(("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "Saved", body = dto::OverallNote, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 409, description = "The page is bound to an article, or the note changed since that version; nothing was written", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 413, description = "Input capacity exceeded", body = dto::InputErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn save_web_page(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    body: Result<Json<dto::WebPageOverallNoteSave>, JsonRejection>,
) -> Result<Json<dto::OverallNote>, ApiError> {
    let Json(input) = body.map_err(json_error)?;
    let url = WebPageUrl::parse(input.url)?;
    let title = WebPageTitle::new(&input.title, &url);
    let text = OverallNoteText::parse(input.text)?;
    let note = server
        .database
        .save_web_page_overall_note(owner.scope(), &url, &title, input.version, text.as_ref())
        .await?;
    Ok(Json(note.into()))
}

/// Validates and applies a save to a document addressed directly.
async fn save(
    server: &BusinessServer,
    owner: &palace_db::Owner,
    target: OverallNoteTarget,
    body: Result<Json<dto::OverallNoteSave>, JsonRejection>,
) -> Result<Json<dto::OverallNote>, ApiError> {
    let Json(input) = body.map_err(json_error)?;
    let text = OverallNoteText::parse(input.text)?;
    let note = server
        .database
        .save_overall_note(owner.scope(), target, input.version, text.as_ref())
        .await?;
    Ok(Json(note.into()))
}
