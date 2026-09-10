use super::{
    ApiError, LoginProvider, Server,
    handlers::{authenticate, authenticated_response},
    security::check_origin,
};
use axum::{
    Json,
    extract::{Query, State},
    http::HeaderMap,
    response::Response,
};
use palace_domain::{Record, ServerVersion};
use serde::Deserialize;
use std::sync::Arc;

/// Returns an explicit committed result for every uploaded independent record.
pub(super) async fn upload<P: LoginProvider>(
    State(server): State<Arc<Server<P>>>,
    headers: HeaderMap,
    Json(records): Json<Vec<Record>>,
) -> Result<Response, ApiError> {
    check_origin(&headers, &server.origin)?;
    let session = authenticate(&server, &headers).await?;
    let results = server
        .database
        .upload_records(session.owner.scope(), &records)
        .await?;
    authenticated_response(&session, Json(results))
}
#[derive(Deserialize)]
pub(super) struct Pull {
    cursor: ServerVersion,
    limit: u32,
}
/// Supplies a scoped ordered page including tombstones and no inferred sequence high-water mark.
pub(super) async fn pull<P: LoginProvider>(
    State(server): State<Arc<Server<P>>>,
    headers: HeaderMap,
    Query(query): Query<Pull>,
) -> Result<Response, ApiError> {
    let session = authenticate(&server, &headers).await?;
    let page = server
        .database
        .pull_records(session.owner.scope(), query.cursor, query.limit)
        .await?;
    authenticated_response(&session, Json(page))
}
