//! 今日回响's writes: completing a card of a day's pile, and taking a jotting out of recall or back
//! in. Reading the pile is in `jotting_reading`. Jottings belong to the deployment, so these
//! handlers take no owner scope. See
//! `specs/decisions/web/echo/20261101-recall-least-completed-first-from-a-stored-daily-pile.md`.
use super::{
    ApiError, BusinessServer, dto, jotting_reading::configured, path_management::json_error, routes,
};
use axum::{
    Json,
    extract::{Path, State, rejection::JsonRejection},
};
use palace_domain::{InputError, InputErrorKind};
use std::sync::Arc;
use time::{Date, macros::format_description};
use uuid::Uuid;

/// Completes a card of a stored pile, today's or the one before. Only the first completion of a
/// card counts; `retire` also takes the jotting out of recall, even when the card was done.
#[utoipa::path(
    post, path = routes::JOTTING_RECALL_DONE, operation_id = "complete_jotting_recall",
    request_body = dto::RecallDone,
    params(("date" = String, Path, description = "The pile's local day, `YYYY-MM-DD`, as the recall read gave it"), ("id" = Uuid, Path, description = "Jotting id"), ("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "Completed; whether the jotting is still in recall", body = dto::JottingRecallState, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "No stored pile of that day holds the jotting, or the jotting is deleted", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider unavailable, or no jottings folder configured", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn done(
    State(server): State<Arc<BusinessServer>>,
    Path((date, id)): Path<(String, Uuid)>,
    body: Result<Json<dto::RecallDone>, JsonRejection>,
) -> Result<Json<dto::JottingRecallState>, ApiError> {
    let Json(input) = body.map_err(json_error)?;
    let date = Date::parse(&date, format_description!("[year]-[month]-[day]"))
        .map_err(|_| InputError::new(InputErrorKind::Field, "date", "expected YYYY-MM-DD"))?;
    let state = configured(&server)?
        .complete_recall(date, id, input.next.into())
        .await?;
    Ok(Json(dto::JottingRecallState {
        recall: state.into(),
    }))
}

/// Takes a jotting out of recall, or back in. Back in, it starts at the least count of the
/// moment; a jotting already in keeps its count.
#[utoipa::path(
    put, path = routes::JOTTING_RECALL_STATE, operation_id = "set_jotting_recall",
    request_body = dto::JottingRecallState,
    params(("id" = Uuid, Path, description = "Jotting id"), ("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected for browser sessions; not checked for access tokens")),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "Set", body = dto::JottingRecallState, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "No live jotting has this id", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider unavailable, or no jottings folder configured", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn set(
    State(server): State<Arc<BusinessServer>>,
    Path(id): Path<Uuid>,
    body: Result<Json<dto::JottingRecallState>, JsonRejection>,
) -> Result<Json<dto::JottingRecallState>, ApiError> {
    let Json(input) = body.map_err(json_error)?;
    configured(&server)?
        .set_recall(id, input.recall.into())
        .await?;
    Ok(Json(input))
}
