//! 时刻 日课: reading habits with their checks, arranging habits and setting a day's count. The
//! day is the reader's; see the ADR
//! `specs/decisions/web/habits/0-habits-as-seals-and-activities-as-moments.md`.
use super::{ApiError, BusinessServer, dto, path_management::json_error, routes};
use axum::{
    Json,
    extract::{Path, Query, State, rejection::JsonRejection, rejection::QueryRejection},
    http::StatusCode,
};
use palace_domain::{
    HabitCount, HabitDays, HabitInput, HabitMark, HabitName, HabitTimes, InputError,
    InputErrorKind, LocalDay,
};
use std::sync::Arc;
use uuid::Uuid;

/// The habits a run of local days shows and their checks on those days.
#[utoipa::path(
    get, path = routes::HABITS, operation_id = "habits",
    params(dto::HabitDaysQuery),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "Successful operation", body = dto::HabitBoard, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn board(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    query: Result<Query<dto::HabitDaysQuery>, QueryRejection>,
) -> Result<Json<dto::HabitBoard>, ApiError> {
    let Query(query) = query
        .map_err(|error| InputError::new(InputErrorKind::Field, "query", error.body_text()))?;
    let days = HabitDays::parse(&query.from, &query.to)?;
    Ok(Json(
        server
            .database
            .habit_board(owner.scope(), days)
            .await?
            .into(),
    ))
}

/// Adds a habit after the existing ones.
#[utoipa::path(
    post, path = routes::HABITS, operation_id = "create_habit",
    request_body = dto::HabitInput,
    params(("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 201, description = "Created", body = dto::Habit, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
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
    body: Result<Json<dto::HabitInput>, JsonRejection>,
) -> Result<(StatusCode, Json<dto::Habit>), ApiError> {
    let Json(input) = body.map_err(json_error)?;
    let habit = server
        .database
        .insert_habit(owner.scope(), &habit_input(&input)?)
        .await?;
    Ok((StatusCode::CREATED, Json(habit.into())))
}

/// Renames a habit, changes its seal or its times a day; existing checks stay as they were.
#[utoipa::path(
    put, path = routes::HABIT, operation_id = "update_habit",
    request_body = dto::HabitInput,
    params(("id" = Uuid, Path, description = "Owner-scoped habit id"), ("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "Updated", body = dto::Habit, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "Habit unavailable in owner scope, or retired", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 413, description = "Input capacity exceeded", body = dto::InputErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn update(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path(id): Path<Uuid>,
    body: Result<Json<dto::HabitInput>, JsonRejection>,
) -> Result<Json<dto::Habit>, ApiError> {
    let Json(input) = body.map_err(json_error)?;
    let habit = server
        .database
        .update_habit(owner.scope(), id, &habit_input(&input)?)
        .await?;
    Ok(Json(habit.into()))
}

/// Retires a habit: it leaves the day page and takes no checks; its checks stay.
#[utoipa::path(
    post, path = routes::HABIT_RETIRE, operation_id = "retire_habit",
    params(("id" = Uuid, Path, description = "Owner-scoped habit id"), ("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 204, description = "Retired, now or before", headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; native extractor text", body = String, content_type = "text/plain"),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "Habit unavailable in owner scope", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn retire(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    server.database.retire_habit(owner.scope(), id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Sets how many times a habit was done on a local day. Repeating the request changes nothing.
#[utoipa::path(
    put, path = routes::HABIT_CHECK, operation_id = "set_habit_check",
    request_body = dto::HabitCountInput,
    params(("id" = Uuid, Path, description = "Owner-scoped habit id"), ("day" = String, Path, description = "Local day, `YYYY-MM-DD`"), ("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "Set", body = dto::HabitCheckResult, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input, or more than the habit's times a day; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "Habit unavailable in owner scope, or retired", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 413, description = "Input capacity exceeded", body = dto::InputErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn check(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path((id, day)): Path<(Uuid, String)>,
    body: Result<Json<dto::HabitCountInput>, JsonRejection>,
) -> Result<Json<dto::HabitCheckResult>, ApiError> {
    let Json(input) = body.map_err(json_error)?;
    let day = LocalDay::parse(&day, "day")?;
    let count = HabitCount::parse(input.count)?;
    let check = server
        .database
        .set_habit_check(owner.scope(), id, day, count)
        .await?;
    Ok(Json(dto::HabitCheckResult {
        habit_id: id,
        day: day.text(),
        count: check.map_or(0, |check| check.count),
    }))
}

fn habit_input(input: &dto::HabitInput) -> Result<HabitInput, InputError> {
    Ok(HabitInput {
        name: HabitName::parse(&input.name)?,
        mark: HabitMark::parse(&input.mark)?,
        times_per_day: HabitTimes::parse(input.times_per_day)?,
    })
}
