use super::{database, request};
use pretty_assertions::assert_eq;

/// Creation uses the database clock, while retries preserve both independent timestamps.
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn creation_time_is_independent_of_occurrence_and_stable_on_retry() {
    let (_container, db, pool) = database().await;
    let owner = db
        .resolve_identity("https://idp", "times", "times@example.com")
        .await
        .unwrap();
    let input = request("times", &["hello"]);
    let before: i64 =
        // Stored times are truncated to milliseconds, so the lower bound is truncated as well.
        sqlx::query_scalar(
            "SELECT (extract(epoch FROM date_trunc('milliseconds',clock_timestamp()))*1000000)::bigint",
        )
            .fetch_one(&pool)
            .await
            .unwrap();
    let result = db.import_path(owner.scope(), &input).await.unwrap();
    let times: (i64, i64, bool) = sqlx::query_as(
        "SELECT (extract(epoch FROM occurred_at)*1000)::bigint, \
         (extract(epoch FROM created_at)*1000000)::bigint, \
         (extract(epoch FROM created_at)*1000000)::bigint >= $2 AND created_at <= clock_timestamp() \
         FROM conversation_import WHERE id=$1",
    )
    .bind(result.import_id)
    .bind(before)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(times, (input.occurred_at(), times.1, true));
    assert_eq!(db.import_path(owner.scope(), &input).await.unwrap(), result);
    let after: (i64, i64) = sqlx::query_as(
        "SELECT (extract(epoch FROM occurred_at)*1000)::bigint, \
         (extract(epoch FROM created_at)*1000000)::bigint \
         FROM conversation_import WHERE id=$1",
    )
    .bind(result.import_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(after, (times.0, times.1));
}

/// Upgrading preserves user dates and idempotency without inventing historical creation times.
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn migration_preserves_existing_occurrence_and_retry() {
    let (_container, db, pool) = database().await;
    let owner = db
        .resolve_identity("https://idp", "upgrade", "upgrade@example.com")
        .await
        .unwrap();
    let input = request("upgrade", &["hello"]);
    let result = db.import_path(owner.scope(), &input).await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    // Restore the previous table shape with a real record before applying the upgrade.
    sqlx::raw_sql(
        "ALTER TABLE conversation_import DROP COLUMN created_at; \
         ALTER TABLE conversation_import RENAME COLUMN occurred_at TO imported_at; \
         ALTER TABLE conversation_import ALTER COLUMN imported_at SET DEFAULT now();",
    )
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../../migrations/0006_conversation_import_times.sql"
    ))
    .execute(&mut *tx)
    .await
    .unwrap();
    let migrated: (i64, bool) = sqlx::query_as(
        "SELECT (extract(epoch FROM occurred_at)*1000)::bigint, created_at=now() \
         FROM conversation_import WHERE id=$1",
    )
    .bind(result.import_id)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    assert_eq!(migrated, (input.occurred_at(), true));
    tx.commit().await.unwrap();
    assert_eq!(db.import_path(owner.scope(), &input).await.unwrap(), result);
}

/// Epoch milliseconds from the API land on whole milliseconds, so the precision check never
/// rejects a write and a cursor built from a stored time compares equal to it.
/// Core test case:
/// - `specs/test-cases/web/stars/contents-page.md#conversation-list-must-page-every-path-exactly-once-in-order`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn epoch_milliseconds_convert_to_whole_milliseconds() {
    let (_container, _db, pool) = database().await;
    // Two windows of consecutive milliseconds: around now and around 2100, where a double carries
    // less fractional precision.
    let inexact: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM (SELECT generate_series(1759843443000, 1759844443000) AS ms \
         UNION ALL SELECT generate_series(4102444800000, 4102445800000)) s \
         WHERE to_timestamp(ms::double precision/1000.0) <> date_trunc('milliseconds', to_timestamp(ms::double precision/1000.0)) \
         OR (extract(epoch FROM to_timestamp(ms::double precision/1000.0))*1000)::bigint <> ms",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(inexact, 0);
}

/// Rows written while imports took `now()` carry microseconds; the upgrade truncates them, and
/// afterwards the database refuses sub-millisecond times.
/// Core test case:
/// - `specs/test-cases/web/stars/contents-page.md#conversation-list-must-page-every-path-exactly-once-in-order`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn timestamps_are_truncated_to_milliseconds_and_kept_there() {
    let (_container, db, pool) = database().await;
    let owner = db
        .resolve_identity("https://idp", "precision", "precision@example.com")
        .await
        .unwrap();
    let result = db
        .import_path(owner.scope(), &request("precision", &["hello"]))
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    // Recreate the legacy state: no precision checks and a microsecond occurrence time.
    sqlx::raw_sql(
        "ALTER TABLE moment DROP CONSTRAINT moment_occurred_at_check; \
         ALTER TABLE conversation_import DROP CONSTRAINT conversation_import_occurred_at_check;",
    )
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query("UPDATE moment SET occurred_at='2026-10-07 10:00:00.123456+00' WHERE id=$1")
        .bind(result.path_id)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE conversation_import SET occurred_at='2026-10-07 10:00:00.123456+00' WHERE id=$1",
    )
    .bind(result.import_id)
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::raw_sql(include_str!(
        "../../migrations/0012_millisecond_timestamps.sql"
    ))
    .execute(&mut *tx)
    .await
    .unwrap();
    let truncated: (String, String) = sqlx::query_as(
        "SELECT (SELECT occurred_at::text FROM moment WHERE id=$1), \
         (SELECT occurred_at::text FROM conversation_import WHERE id=$2)",
    )
    .bind(result.path_id)
    .bind(result.import_id)
    .fetch_one(&mut *tx)
    .await
    .unwrap();
    let expected = "2026-10-07 10:00:00.123+00".to_owned();
    assert_eq!(truncated, (expected.clone(), expected));
    let rejected =
        sqlx::query("UPDATE moment SET occurred_at='2026-10-07 10:00:00.123456+00' WHERE id=$1")
            .bind(result.path_id)
            .execute(&mut *tx)
            .await
            .unwrap_err();
    assert_eq!(
        rejected
            .as_database_error()
            .and_then(sqlx::error::DatabaseError::code)
            .as_deref(),
        Some("23514")
    );
}
