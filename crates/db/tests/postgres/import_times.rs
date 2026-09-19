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
        sqlx::query_scalar("SELECT (extract(epoch FROM clock_timestamp())*1000000)::bigint")
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
