//! 时刻 工夫: activity types, and activities as moments at their start. See the ADR
//! `specs/decisions/web/habits/0-habits-as-seals-and-activities-as-moments.md`.
use super::{ApiError, BusinessServer, dto, path_management::json_error, routes};
use axum::{
    Json,
    extract::{Path, Query, State, rejection::JsonRejection, rejection::QueryRejection},
    http::StatusCode,
};
use palace_db::TypeAdded;
use palace_domain::{ActivityInput, ActivityTypeName, InputError, InputErrorKind, InstantRange};
use std::sync::Arc;
use uuid::Uuid;

/// The owner's activity types in the order they were added.
#[utoipa::path(
    get, path = routes::ACTIVITY_TYPES, operation_id = "activity_types",
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "Successful operation", body = [dto::ActivityType], headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn types(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
) -> Result<Json<Vec<dto::ActivityType>>, ApiError> {
    Ok(Json(
        server
            .database
            .activity_types(owner.scope())
            .await?
            .into_iter()
            .map(Into::into)
            .collect(),
    ))
}

/// Adds a type, or gives back the one already named so.
#[utoipa::path(
    post, path = routes::ACTIVITY_TYPES, operation_id = "add_activity_type",
    request_body = dto::ActivityTypeInput,
    params(("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "A type by this name already existed", body = dto::ActivityType, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 201, description = "Created", body = dto::ActivityType, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 413, description = "Input capacity exceeded", body = dto::InputErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn add_type(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    body: Result<Json<dto::ActivityTypeInput>, JsonRejection>,
) -> Result<(StatusCode, Json<dto::ActivityType>), ApiError> {
    let Json(input) = body.map_err(json_error)?;
    let name = ActivityTypeName::parse(&input.name)?;
    let (added, how) = server
        .database
        .add_activity_type(owner.scope(), &name)
        .await?;
    let status = match how {
        TypeAdded::Created => StatusCode::CREATED,
        TypeAdded::Existing => StatusCode::OK,
    };
    Ok((status, Json(added.into())))
}

/// The activities starting in the caller's range, earliest first.
#[utoipa::path(
    get, path = routes::ACTIVITIES, operation_id = "activities",
    params(dto::ActivityRange),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "Activities ordered by start, then id", body = [dto::Activity], headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn list(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    range: Result<Query<dto::ActivityRange>, QueryRejection>,
) -> Result<Json<Vec<dto::Activity>>, ApiError> {
    let Query(range) = range
        .map_err(|error| InputError::new(InputErrorKind::Field, "query", error.body_text()))?;
    let range = InstantRange::parse(range.start, range.end)?;
    Ok(Json(
        server
            .database
            .activities(owner.scope(), range)
            .await?
            .into_iter()
            .map(Into::into)
            .collect(),
    ))
}

/// Records an activity as a moment at its start.
#[utoipa::path(
    post, path = routes::ACTIVITIES, operation_id = "record_activity",
    request_body = dto::ActivityInput,
    params(("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 201, description = "Created", body = dto::Activity, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "Activity type unavailable in owner scope", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 413, description = "Input capacity exceeded", body = dto::InputErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn record(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    body: Result<Json<dto::ActivityInput>, JsonRejection>,
) -> Result<(StatusCode, Json<dto::Activity>), ApiError> {
    let Json(input) = body.map_err(json_error)?;
    let activity = server
        .database
        .record_activity(owner.scope(), &activity_input(&input)?)
        .await?;
    Ok((StatusCode::CREATED, Json(activity.into())))
}

/// Corrects an activity; it keeps its identity and moves to the day of its new start.
#[utoipa::path(
    put, path = routes::ACTIVITY, operation_id = "update_activity",
    request_body = dto::ActivityInput,
    params(("id" = Uuid, Path, description = "Owner-scoped activity id"), ("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "Updated", body = dto::Activity, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "Activity or its type unavailable in owner scope", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 413, description = "Input capacity exceeded", body = dto::InputErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn update(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path(id): Path<Uuid>,
    body: Result<Json<dto::ActivityInput>, JsonRejection>,
) -> Result<Json<dto::Activity>, ApiError> {
    let Json(input) = body.map_err(json_error)?;
    let activity = server
        .database
        .update_activity(owner.scope(), id, &activity_input(&input)?)
        .await?;
    Ok(Json(activity.into()))
}

/// Deletes an activity together with its moment.
#[utoipa::path(
    delete, path = routes::ACTIVITY, operation_id = "delete_activity",
    params(("id" = Uuid, Path, description = "Owner-scoped activity id"), ("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 204, description = "Deleted", headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; native extractor text", body = String, content_type = "text/plain"),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "Activity unavailable in owner scope", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn delete(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    server.database.delete_activity(owner.scope(), id).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn activity_input(input: &dto::ActivityInput) -> Result<ActivityInput, InputError> {
    ActivityInput::parse(
        input.type_id,
        input.started_at,
        input.minutes,
        input.note.as_deref(),
    )
}
