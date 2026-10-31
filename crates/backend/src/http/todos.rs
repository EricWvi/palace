//! 行事 tasks: the board, edits, the body, done for the day, finishing and planning a day.
use super::{ApiError, BusinessServer, dto, path_management::json_error, routes};
use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
    http::StatusCode,
};
use palace_db::TodoEdit;
use palace_domain::{
    InputError, InputErrorKind, LocalDay, TodoBody, TodoLink, TodoPlacement, TodoPlanInput,
    TodoPosition, TodoTitle,
};
use std::sync::Arc;
use uuid::Uuid;

// One day's plan comes from a reader ticking boxes; a thousand is far beyond any day.
const PLAN_IDS: usize = 1000;

/// Every list and every open task.
#[utoipa::path(
    get, path = routes::TODOS, operation_id = "todo_board",
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "Successful operation", body = dto::TodoBoard, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn board(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
) -> Result<Json<dto::TodoBoard>, ApiError> {
    Ok(Json(
        server.database.todo_board(owner.scope()).await?.into(),
    ))
}

/// Adds an unscheduled task to a list or the inbox.
#[utoipa::path(
    post, path = routes::TODOS, operation_id = "create_todo",
    request_body = dto::TodoCreate,
    params(("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 201, description = "Created", body = dto::Todo, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "List unavailable in owner scope", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 409, description = "Another open task in the place has this position", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 413, description = "Input capacity exceeded", body = dto::InputErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn create(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    body: Result<Json<dto::TodoCreate>, JsonRejection>,
) -> Result<(StatusCode, Json<dto::Todo>), ApiError> {
    let Json(input) = body.map_err(json_error)?;
    let placement = TodoPlacement::parse(input.list_id, TodoPlanInput::Open)?;
    let title = TodoTitle::parse(&input.title)?;
    let position = TodoPosition::parse(input.position)?;
    let todo = server
        .database
        .insert_todo(owner.scope(), &placement, &title, &position)
        .await?;
    Ok((StatusCode::CREATED, Json(todo.into())))
}

/// Replaces an open task's list, title, link, position and plan.
#[utoipa::path(
    put, path = routes::TODO, operation_id = "update_todo",
    request_body = dto::TodoUpdate,
    params(("id" = Uuid, Path, description = "Owner-scoped task id"), ("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "Updated", body = dto::Todo, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "Task or list unavailable in owner scope", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 409, description = "The task is finished, or another open task in the place has this position", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 413, description = "Input capacity exceeded", body = dto::InputErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn update(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path(id): Path<Uuid>,
    body: Result<Json<dto::TodoUpdate>, JsonRejection>,
) -> Result<Json<dto::Todo>, ApiError> {
    let Json(input) = body.map_err(json_error)?;
    let plan = match input.plan {
        dto::TodoPlanInput::Open => TodoPlanInput::Open,
        dto::TodoPlanInput::Shelved => TodoPlanInput::Shelved,
        dto::TodoPlanInput::Day { day } => TodoPlanInput::Day(LocalDay::parse(&day, "plan.day")?),
    };
    let edit = TodoEdit {
        placement: TodoPlacement::parse(input.list_id, plan)?,
        title: TodoTitle::parse(&input.title)?,
        link: input.link.map(TodoLink::parse).transpose()?,
        position: TodoPosition::parse(input.position)?,
    };
    let todo = server
        .database
        .update_todo(owner.scope(), id, &edit)
        .await?;
    Ok(Json(todo.into()))
}

/// Deletes a task, finished or not.
#[utoipa::path(
    delete, path = routes::TODO, operation_id = "delete_todo",
    params(("id" = Uuid, Path, description = "Owner-scoped task id"), ("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
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
    server.database.delete_todo(owner.scope(), id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Reads a task's body.
#[utoipa::path(
    get, path = routes::TODO_BODY, operation_id = "todo_body",
    params(("id" = Uuid, Path, description = "Owner-scoped task id")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "Successful operation", body = dto::TodoBodyContent, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; native extractor text", body = String, content_type = "text/plain"),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "Resource unavailable in owner scope", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn body(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path(id): Path<Uuid>,
) -> Result<Json<dto::TodoBodyContent>, ApiError> {
    let body = server.database.todo_body(owner.scope(), id).await?;
    Ok(Json(dto::TodoBodyContent { body }))
}

/// Replaces or clears a task's body.
#[utoipa::path(
    put, path = routes::TODO_BODY, operation_id = "set_todo_body",
    request_body = dto::TodoBodyContent,
    params(("id" = Uuid, Path, description = "Owner-scoped task id"), ("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 204, description = "Saved", headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "Resource unavailable in owner scope", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 413, description = "Input capacity exceeded", body = dto::InputErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn set_body(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path(id): Path<Uuid>,
    body: Result<Json<dto::TodoBodyContent>, JsonRejection>,
) -> Result<StatusCode, ApiError> {
    let Json(input) = body.map_err(json_error)?;
    let body = input.body.map(TodoBody::parse).transpose()?;
    server
        .database
        .set_todo_body(owner.scope(), id, body.as_ref())
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Marks a task done or not done for the day it is planned on.
#[utoipa::path(
    put, path = routes::TODO_DONE, operation_id = "set_todo_done",
    request_body = dto::TodoDone,
    params(("id" = Uuid, Path, description = "Owner-scoped task id"), ("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "Updated", body = dto::Todo, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "Resource unavailable in owner scope", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 409, description = "The task is not planned on this day, or is finished", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 413, description = "Input capacity exceeded", body = dto::InputErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn done(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path(id): Path<Uuid>,
    body: Result<Json<dto::TodoDone>, JsonRejection>,
) -> Result<Json<dto::Todo>, ApiError> {
    let Json(input) = body.map_err(json_error)?;
    let day = LocalDay::parse(&input.day, "day")?;
    let todo = server
        .database
        .set_todo_done(owner.scope(), id, day, input.done)
        .await?;
    Ok(Json(todo.into()))
}

/// Finishes a task in a list; it leaves its plan and keeps its count.
#[utoipa::path(
    post, path = routes::TODO_COMPLETE, operation_id = "complete_todo",
    params(("id" = Uuid, Path, description = "Owner-scoped task id"), ("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 204, description = "Finished", headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; native extractor text", body = String, content_type = "text/plain"),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "Resource unavailable in owner scope", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 409, description = "The task is in the inbox or already finished", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn complete(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    server.database.complete_todo(owner.scope(), id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Brings a finished task back to its list, unscheduled.
#[utoipa::path(
    post, path = routes::TODO_RESTORE, operation_id = "restore_todo",
    request_body = dto::TodoRestore,
    params(("id" = Uuid, Path, description = "Owner-scoped task id"), ("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "Restored", body = dto::Todo, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "Resource unavailable in owner scope", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 409, description = "The task is not finished, or another open task in the list has this position", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 413, description = "Input capacity exceeded", body = dto::InputErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn restore(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path(id): Path<Uuid>,
    body: Result<Json<dto::TodoRestore>, JsonRejection>,
) -> Result<Json<dto::Todo>, ApiError> {
    let Json(input) = body.map_err(json_error)?;
    let position = TodoPosition::parse(input.position)?;
    let todo = server
        .database
        .restore_todo(owner.scope(), id, &position)
        .await?;
    Ok(Json(todo.into()))
}

/// Puts the unscheduled tasks among `ids` on a day.
#[utoipa::path(
    post, path = routes::TODO_PLAN, operation_id = "plan_todos",
    request_body = dto::TodoPlanRequest,
    params(("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 204, description = "Planned", headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 413, description = "Input capacity exceeded", body = dto::InputErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn plan(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    body: Result<Json<dto::TodoPlanRequest>, JsonRejection>,
) -> Result<StatusCode, ApiError> {
    let Json(input) = body.map_err(json_error)?;
    let day = LocalDay::parse(&input.day, "day")?;
    if input.ids.len() > PLAN_IDS {
        return Err(
            InputError::new(InputErrorKind::Limit, "ids", "expected at most 1000 ids").into(),
        );
    }
    server
        .database
        .plan_todos(owner.scope(), day, &input.ids)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
