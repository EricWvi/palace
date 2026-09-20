use super::{ApiError, BusinessServer, dto, routes};
use axum::{
    Json,
    extract::{Query, State},
};
use std::sync::Arc;

/// Returns an explicit committed result for every uploaded independent record.
#[utoipa::path(
    post, path = routes::SYNC, operation_id = "upload",
    request_body = [dto::Record],
    params(("Origin" = String, Header, description = "Exactly one header matching the configured external origin; null, duplicates and omissions are rejected")),
    security(("session" = [])),
    responses(
        (status = 200, description = "Successful operation", body = [dto::UploadResult], headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", body = String, content_type = "text/plain"),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 403, description = "Origin rejected", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 404, description = "Resource unavailable in owner scope", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 409, description = "Identity, source session or idempotency conflict", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 413, description = "Input capacity exceeded: extractor text or structured batch/record limit", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 415, description = "JSON Content-Type required", body = String, content_type = "text/plain"),
        (status = 422, description = "JSON does not match the target type", body = String, content_type = "text/plain"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn upload(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Json(records): Json<Vec<dto::Record>>,
) -> Result<Json<Vec<dto::UploadResult>>, ApiError> {
    let records: Vec<palace_domain::Record> = records.into_iter().map(Into::into).collect();
    let results = server
        .database
        .upload_records(owner.scope(), &records)
        .await?;
    Ok(Json(results.into_iter().map(Into::into).collect()))
}
/// Supplies a scoped ordered page including tombstones and no inferred sequence high-water mark.
#[utoipa::path(
    get, path = routes::SYNC, operation_id = "pull",
    params(dto::Pull),
    security(("session" = [])),
    responses(
        (status = 200, description = "Successful operation", body = dto::SyncPage, headers(("Set-Cookie" = String, description = "Refreshed production session; absent in fixed-user mode"), ("Cache-Control" = String, description = "no-store"))),
        (status = 400, description = "Invalid input; JSON domain error or native extractor text as declared", content((dto::InputErrorResponse = "application/json"), (String = "text/plain"))),
        (status = 401, description = "Authentication required", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 409, description = "Identity, source session or idempotency conflict", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 500, description = "Internal persistence or response failure", body = dto::ErrorResponse, content_type = "application/json"),
        (status = 503, description = "Identity provider temporarily unavailable", body = dto::ErrorResponse, content_type = "application/json"),
    )
)]
pub(super) async fn pull(
    State(server): State<Arc<BusinessServer>>,
    axum::Extension(owner): axum::Extension<palace_db::Owner>,
    Query(query): Query<dto::Pull>,
) -> Result<Json<dto::SyncPage>, ApiError> {
    let page = server
        .database
        .pull_records(owner.scope(), query.cursor.0, query.limit)
        .await?;
    Ok(Json(page.into()))
}
