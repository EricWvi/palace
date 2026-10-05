use super::{ApiError, BusinessServer, dto, routes};
use axum::{
    Json,
    extract::{Query, State, rejection::QueryRejection},
};
use palace_domain::{InputError, InputErrorKind};
use std::sync::Arc;

/// Lists one local day of moments; the browser supplies the day's bounds in its own time zone.
#[utoipa::path(
    get, path = routes::TIMELINE, operation_id = "timeline",
    params(dto::TimelineRange),
    security(("session" = [])),
    responses(
        (status = 200, description = "Moments ordered by occurrence time, then id", body = [dto::Moment], headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 409, description = "Identity, source session or idempotency conflict", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn timeline(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    range: Result<Query<dto::TimelineRange>, QueryRejection>,
) -> Result<Json<Vec<dto::Moment>>, ApiError> {
    let Query(range) = range
        .map_err(|error| InputError::new(InputErrorKind::Field, "query", error.body_text()))?;
    Ok(Json(
        server
            .database
            .timeline(owner.scope(), range.start, range.end)
            .await?
            .into_iter()
            .map(Into::into)
            .collect(),
    ))
}
