//! Live reads of 札记 for the web 回响 page. Unlike the offline sync feed, these answer each view
//! directly: a filtered list, the left column's counts and the day's recall pile. Jottings
//! belong to the deployment, so these handlers take no owner scope.
//! See `specs/decisions/web/echo/0-jotting-wall-with-daily-recall-pile.md` D8.
use super::{ApiError, BusinessServer, dto, routes};
use axum::{
    Json,
    extract::{Query, State, rejection::QueryRejection},
};
use palace_db::{JottingCursor, JottingListRequest, SearchTerm};
use palace_domain::{InputError, InputErrorKind};
use palace_jottings::{JottingService, Jottings};
use std::sync::Arc;
use time::{Date, macros::format_description};

// Jottings per page when the client does not ask for a size, and the most it may ask for.
const DEFAULT_LIMIT: u32 = 60;
const MAX_LIMIT: u32 = 200;

/// Lists live jottings, newest first, optionally by tag and search term.
#[utoipa::path(
    get, path = routes::JOTTINGS, operation_id = "jottings",
    params(dto::JottingListQuery),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "Jottings ordered by `created_at`, then id, both descending", body = dto::JottingPage, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider unavailable, or no jottings folder configured", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn list(
    State(server): State<Arc<BusinessServer>>,
    query: Result<Query<dto::JottingListQuery>, QueryRejection>,
) -> Result<Json<dto::JottingPage>, ApiError> {
    let Query(query) = query
        .map_err(|error| InputError::new(InputErrorKind::Field, "query", error.body_text()))?;
    let limit = query.limit.unwrap_or(DEFAULT_LIMIT);
    if !(1..=MAX_LIMIT).contains(&limit) {
        return Err(InputError::new(InputErrorKind::Field, "limit", "expected 1..200").into());
    }
    let request = JottingListRequest {
        tag: query
            .tag
            .map(|tag| tag.trim().to_owned())
            .filter(|tag| !tag.is_empty()),
        search: query.q.as_deref().and_then(SearchTerm::parse),
        after: query
            .cursor
            .as_deref()
            .map(str::parse::<JottingCursor>)
            .transpose()?,
        limit,
    };
    Ok(Json(configured(&server)?.list(&request).await?.into()))
}

/// Counts for the left column: totals, every tag and per-day counts through today.
#[utoipa::path(
    get, path = routes::JOTTING_OVERVIEW, operation_id = "jotting_overview",
    params(dto::JottingOverviewQuery),
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "Counts of live jottings; days are the deployment's local days", body = dto::JottingOverview, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider unavailable, or no jottings folder configured", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn overview(
    State(server): State<Arc<BusinessServer>>,
    query: Result<Query<dto::JottingOverviewQuery>, QueryRejection>,
) -> Result<Json<dto::JottingOverview>, ApiError> {
    let Query(query) = query
        .map_err(|error| InputError::new(InputErrorKind::Field, "query", error.body_text()))?;
    let from = Date::parse(&query.from, format_description!("[year]-[month]-[day]"))
        .map_err(|_| InputError::new(InputErrorKind::Field, "from", "expected YYYY-MM-DD"))?;
    let overview = configured(&server)?
        .overview(from, palace_logging::clock::now_millis())
        .await?;
    Ok(Json(overview.into()))
}

/// Today's recall pile, fixed for the day.
#[utoipa::path(
    get, path = routes::JOTTING_RECALL, operation_id = "jotting_recall",
    security(("session" = []), ("access_token" = [])),
    responses(
        (status = 200, description = "At most five jottings created before today, in the day's order", body = dto::JottingRecall, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider unavailable, or no jottings folder configured", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn recall(
    State(server): State<Arc<BusinessServer>>,
) -> Result<Json<dto::JottingRecall>, ApiError> {
    let recall = configured(&server)?
        .recall(palace_logging::clock::now_millis())
        .await?;
    Ok(Json(recall.into()))
}

/// The configured jottings; without a folder there is nothing to read.
fn configured(server: &BusinessServer) -> Result<&Jottings, ApiError> {
    match &server.jottings {
        JottingService::Configured(jottings) => Ok(jottings),
        JottingService::Unconfigured => Err(ApiError::JottingsUnavailable),
    }
}
