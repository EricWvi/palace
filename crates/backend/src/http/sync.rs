use super::{ApiError, BusinessServer};
use axum::{
    Json,
    extract::{Query, State},
    response::{IntoResponse, Response},
};
use palace_domain::{Record, ServerVersion};
use serde::Deserialize;
use std::sync::Arc;

/// Returns an explicit committed result for every uploaded independent record.
pub(super) async fn upload(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Json(records): Json<Vec<Record>>,
) -> Result<Response, ApiError> {
    let results = server
        .database
        .upload_records(owner.scope(), &records)
        .await?;
    Ok(Json(results).into_response())
}
#[derive(Deserialize)]
pub(super) struct Pull {
    cursor: ServerVersion,
    limit: u32,
}
/// Supplies a scoped ordered page including tombstones and no inferred sequence high-water mark.
pub(super) async fn pull(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Query(query): Query<Pull>,
) -> Result<Response, ApiError> {
    let page = server
        .database
        .pull_records(owner.scope(), query.cursor, query.limit)
        .await?;
    Ok(Json(page).into_response())
}
