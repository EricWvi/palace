use super::{ApiError, BusinessServer, dto, routes};
use axum::{
    Json,
    extract::{Query, State, rejection::QueryRejection},
};
use palace_db::{ConversationListRequest, PathCursor, SearchTerm};
use palace_domain::{InputError, InputErrorKind};
use std::sync::Arc;

/// Lists the owner's conversation paths for 摘星, newest change first, one page at a time.
#[utoipa::path(
    get, path = routes::CONVERSATIONS, operation_id = "conversations",
    params(dto::ConversationListQuery),
    security(("session" = [])),
    responses(
        (status = 200, description = "Up to 50 paths ordered by last update, then id, both descending", body = dto::ConversationPage, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 409, description = "Identity, source session or idempotency conflict", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn list(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    query: Result<Query<dto::ConversationListQuery>, QueryRejection>,
) -> Result<Json<dto::ConversationPage>, ApiError> {
    let Query(query) = query
        .map_err(|error| InputError::new(InputErrorKind::Field, "query", error.body_text()))?;
    let request = ConversationListRequest {
        search: query.q.as_deref().and_then(SearchTerm::parse),
        after: query
            .cursor
            .as_deref()
            .map(str::parse::<PathCursor>)
            .transpose()?,
    };
    Ok(Json(
        server
            .database
            .conversation_list(owner.scope(), &request)
            .await?
            .into(),
    ))
}
