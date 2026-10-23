//! 行事 lists: create, rename and delete, and each list's finished tasks.
use super::{ApiError, BusinessServer, dto, path_management::json_error, routes};
use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
};
use palace_domain::TodoListName;
use std::sync::Arc;
use uuid::Uuid;

/// Adds a list after the existing ones.
#[utoipa::path(
    post, path = routes::TODO_LISTS, operation_id = "create_todo_list",
    request_body = dto::TodoListInput,
    params(("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 201, description = "Created", body = dto::TodoList, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 413, description = "Input capacity exceeded", body = dto::InputErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn create(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    body: Result<Json<dto::TodoListInput>, JsonRejection>,
) -> Result<(StatusCode, Json<dto::TodoList>), ApiError> {
    let Json(input) = body.map_err(json_error)?;
    let name = TodoListName::parse(&input.name)?;
    let list = server
        .database
        .insert_todo_list(owner.scope(), &name)
        .await?;
    Ok((StatusCode::CREATED, Json(list.into())))
}

/// Renames a list.
#[utoipa::path(
    put, path = routes::TODO_LIST, operation_id = "rename_todo_list",
    request_body = dto::TodoListInput,
    params(("id" = Uuid, Path, description = "Owner-scoped list id"), ("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "Renamed", body = dto::TodoList, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "Resource unavailable in owner scope", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 413, description = "Input capacity exceeded", body = dto::InputErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn rename(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path(id): Path<Uuid>,
    body: Result<Json<dto::TodoListInput>, JsonRejection>,
) -> Result<Json<dto::TodoList>, ApiError> {
    let Json(input) = body.map_err(json_error)?;
    let name = TodoListName::parse(&input.name)?;
    let list = server
        .database
        .rename_todo_list(owner.scope(), id, &name)
        .await?;
    Ok(Json(list.into()))
}

/// Deletes a list together with all its tasks.
#[utoipa::path(
    delete, path = routes::TODO_LIST, operation_id = "delete_todo_list",
    params(("id" = Uuid, Path, description = "Owner-scoped list id"), ("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
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
    server.database.delete_todo_list(owner.scope(), id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// A list's finished tasks, the most recently finished first.
#[utoipa::path(
    get, path = routes::TODO_LIST_COMPLETED, operation_id = "completed_todos",
    params(("id" = Uuid, Path, description = "Owner-scoped list id")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "Successful operation", body = dto::CompletedTodoList, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; native extractor text", body = String, content_type = "text/plain"),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "Resource unavailable in owner scope", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn completed(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path(id): Path<Uuid>,
) -> Result<Json<dto::CompletedTodoList>, ApiError> {
    let todos = server.database.completed_todos(owner.scope(), id).await?;
    Ok(Json(todos.into()))
}

/// Deletes every finished task of a list.
#[utoipa::path(
    delete, path = routes::TODO_LIST_COMPLETED, operation_id = "clear_completed_todos",
    params(("id" = Uuid, Path, description = "Owner-scoped list id"), ("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 204, description = "Cleared", headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; native extractor text", body = String, content_type = "text/plain"),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "Resource unavailable in owner scope", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn clear_completed(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    server
        .database
        .clear_completed_todos(owner.scope(), id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
