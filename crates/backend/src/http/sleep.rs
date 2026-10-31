//! 时刻 sleep: starting and ending the running sleep, and recording, correcting, deleting and
//! listing finished nights. Rules about the reader's clock are the browser's; see the ADR
//! `specs/decisions/web/sleep/0-sleep-started-from-the-day-menu-pinned-as-a-dial.md`.
use super::{ApiError, BusinessServer, dto, path_management::json_error, routes};
use axum::{
    Json,
    extract::{Path, Query, State, rejection::JsonRejection, rejection::QueryRejection},
    http::StatusCode,
};
use palace_domain::{InputError, InputErrorKind, InstantRange, SleepSpan, epoch_instant};
use std::sync::Arc;
use uuid::Uuid;

/// The running sleep, if any.
#[utoipa::path(
    get, path = routes::SLEEP_RUNNING, operation_id = "sleep_running",
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "Successful operation", body = dto::SleepRunning, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn running(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
) -> Result<Json<dto::SleepRunning>, ApiError> {
    let started_at = server.database.running_sleep(owner.scope()).await?;
    Ok(Json(dto::SleepRunning { started_at }))
}

/// Starts sleeping now by the server's clock, replacing any start still on record.
#[utoipa::path(
    put, path = routes::SLEEP_RUNNING, operation_id = "start_sleep",
    params(("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "Started", body = dto::SleepRunning, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn start(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
) -> Result<Json<dto::SleepRunning>, ApiError> {
    let started_at = server.database.start_sleep(owner.scope()).await?;
    Ok(Json(dto::SleepRunning {
        started_at: Some(started_at),
    }))
}

/// Ends the start the caller saw, waking now by the server's clock. Repeating the request
/// returns the same night.
#[utoipa::path(
    post, path = routes::SLEEP_END, operation_id = "end_sleep",
    request_body = dto::SleepEnd,
    params(("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "The night the start became", body = dto::SleepNight, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input, or the start is more than 24 hours old; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "No running start at this time, and no night that began then", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 409, description = "The night would overlap another", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 413, description = "Input capacity exceeded", body = dto::InputErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn end(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    body: Result<Json<dto::SleepEnd>, JsonRejection>,
) -> Result<Json<dto::SleepNight>, ApiError> {
    let Json(input) = body.map_err(json_error)?;
    let started_at = epoch_instant(input.started_at, "started_at")?;
    let night = server.database.end_sleep(owner.scope(), started_at).await?;
    Ok(Json(night.into()))
}

/// The nights whose wake time falls in the caller's range, earliest first.
#[utoipa::path(
    get, path = routes::SLEEP_NIGHTS, operation_id = "sleep_nights",
    params(dto::SleepNightRange),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "Nights ordered by wake time, then id", body = [dto::SleepNight], headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn nights(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    range: Result<Query<dto::SleepNightRange>, QueryRejection>,
) -> Result<Json<Vec<dto::SleepNight>>, ApiError> {
    let Query(range) = range
        .map_err(|error| InputError::new(InputErrorKind::Field, "query", error.body_text()))?;
    let range = InstantRange::parse(range.start, range.end)?;
    Ok(Json(
        server
            .database
            .sleep_nights(owner.scope(), range)
            .await?
            .into_iter()
            .map(Into::into)
            .collect(),
    ))
}

/// Records a night that was never started.
#[utoipa::path(
    post, path = routes::SLEEP_NIGHTS, operation_id = "record_sleep_night",
    request_body = dto::SleepNightInput,
    params(("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 201, description = "Created", body = dto::SleepNight, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 409, description = "The night would overlap another", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 413, description = "Input capacity exceeded", body = dto::InputErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn record(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    body: Result<Json<dto::SleepNightInput>, JsonRejection>,
) -> Result<(StatusCode, Json<dto::SleepNight>), ApiError> {
    let Json(input) = body.map_err(json_error)?;
    let span = SleepSpan::parse(input.started_at, input.ended_at)?;
    let night = server
        .database
        .record_sleep_night(owner.scope(), span)
        .await?;
    Ok((StatusCode::CREATED, Json(night.into())))
}

/// Moves a night to a new span; it keeps its identity and moves to the day of its new wake time.
#[utoipa::path(
    put, path = routes::SLEEP_NIGHT, operation_id = "update_sleep_night",
    request_body = dto::SleepNightInput,
    params(("id" = Uuid, Path, description = "Owner-scoped night id"), ("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "Updated", body = dto::SleepNight, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "Night unavailable in owner scope", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 409, description = "The night would overlap another", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 413, description = "Input capacity exceeded", body = dto::InputErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn update(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path(id): Path<Uuid>,
    body: Result<Json<dto::SleepNightInput>, JsonRejection>,
) -> Result<Json<dto::SleepNight>, ApiError> {
    let Json(input) = body.map_err(json_error)?;
    let span = SleepSpan::parse(input.started_at, input.ended_at)?;
    let night = server
        .database
        .update_sleep_night(owner.scope(), id, span)
        .await?;
    Ok(Json(night.into()))
}

/// Deletes a night together with its moment.
#[utoipa::path(
    delete, path = routes::SLEEP_NIGHT, operation_id = "delete_sleep_night",
    params(("id" = Uuid, Path, description = "Owner-scoped night id"), ("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 204, description = "Deleted", headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; native extractor text", body = String, content_type = "text/plain"),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "Night unavailable in owner scope", body = dto::ErrorResponse, content_type = "application/json"),
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
        .delete_sleep_night(owner.scope(), id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
