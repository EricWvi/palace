use super::database;
use palace_db::{AccessToken, DbError, Owner};
use palace_domain::{AccessTokenNote, AccessTokenSecret};
use pretty_assertions::assert_eq;
use sqlx::PgPool;
use uuid::Uuid;

/// Reads a token's recorded last use straight from the table.
async fn last_used(pool: &PgPool, id: Uuid) -> Option<i64> {
    sqlx::query_scalar(
        "SELECT (extract(epoch FROM last_used_at)*1000)::bigint FROM owner_access_token WHERE id=$1",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// Only the digest and the masked form are stored, and listing, deleting and ownership stay
/// inside one owner.
/// Core test cases:
/// - `specs/test-cases/server/owner/access-tokens.md#the-full-access-token-must-leave-the-server-only-in-its-creation-response`
/// - `specs/test-cases/server/owner/access-tokens.md#access-tokens-must-stay-within-their-owner`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn access_tokens_store_only_digests_and_stay_within_their_owner() {
    let (_container, db, pool) = database().await;
    let a = db
        .resolve_identity("https://idp", "token-a", "a@example.com")
        .await
        .unwrap();
    let b = db
        .resolve_identity("https://idp", "token-b", "b@example.com")
        .await
        .unwrap();
    let note = |text: &str| AccessTokenNote::parse(text).unwrap();
    let first = db.create_access_token(&a, &note(" cli ")).await.unwrap();
    let second = db.create_access_token(&a, &note("sync")).await.unwrap();
    let foreign = db.create_access_token(&b, &note("other")).await.unwrap();

    assert_eq!(
        first.token,
        AccessToken {
            id: first.token.id,
            note: "cli".into(),
            masked: first.secret.masked(),
            created_at: first.token.created_at,
            last_used_at: None,
        }
    );
    // The stored row holds the digest and masked form, and no column carries the full value.
    let (hash, masked, row): (Vec<u8>, String, String) = sqlx::query_as(
        "SELECT secret_hash,masked,row_to_json(t)::text FROM owner_access_token t WHERE id=$1",
    )
    .bind(first.token.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        (hash, masked),
        (first.secret.digest(), first.secret.masked())
    );
    let random = &first.secret.expose()["palace_pat_".len()..];
    assert!(!row.contains(&random[4..random.len() - 4]));

    assert_eq!(
        db.access_tokens(a.scope()).await.unwrap(),
        vec![second.token.clone(), first.token.clone()]
    );
    assert_eq!(
        db.access_tokens(b.scope()).await.unwrap(),
        vec![foreign.token.clone()]
    );
    for id in [foreign.token.id, Uuid::now_v7()] {
        assert!(matches!(
            db.delete_access_token(a.scope(), id).await,
            Err(DbError::NotFound)
        ));
    }
    assert_eq!(
        db.authenticate_access_token(&foreign.secret).await.unwrap(),
        Some(b.clone())
    );
    assert!(
        sqlx::query("UPDATE owner_access_token SET owner_id=$2 WHERE id=$1")
            .bind(foreign.token.id)
            .bind(a.id)
            .execute(&pool)
            .await
            .is_err()
    );
    // A token cannot be issued under an identity that belongs to another owner.
    let crossed = Owner {
        identity_id: b.identity_id,
        ..a.clone()
    };
    assert!(db.create_access_token(&crossed, &note("x")).await.is_err());
}

/// Deletion and disablement take effect on the very next authentication, and use is recorded at
/// most once a minute.
/// Core test case:
/// - `specs/test-cases/server/owner/access-tokens.md#an-access-token-must-stop-working-as-soon-as-it-is-deleted-or-its-owner-is-disabled`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn access_tokens_stop_working_once_deleted_or_disabled() {
    let (_container, db, pool) = database().await;
    let note = AccessTokenNote::parse("cli").unwrap();
    let owner = db
        .resolve_identity("https://idp", "revoke", "revoke@example.com")
        .await
        .unwrap();
    let kept = db.create_access_token(&owner, &note).await.unwrap();
    let deleted = db.create_access_token(&owner, &note).await.unwrap();
    for issued in [&kept, &deleted] {
        assert_eq!(
            db.authenticate_access_token(&issued.secret).await.unwrap(),
            Some(owner.clone())
        );
    }
    let unknown = AccessTokenSecret::from_random([1; 32]);
    assert_eq!(db.authenticate_access_token(&unknown).await.unwrap(), None);

    db.delete_access_token(owner.scope(), deleted.token.id)
        .await
        .unwrap();
    assert_eq!(
        db.authenticate_access_token(&deleted.secret).await.unwrap(),
        None
    );
    assert_eq!(
        db.authenticate_access_token(&kept.secret).await.unwrap(),
        Some(owner.clone())
    );

    // A use within a minute of the last recorded one leaves it alone; an older one moves it.
    let recorded = last_used(&pool, kept.token.id).await.unwrap();
    db.authenticate_access_token(&kept.secret).await.unwrap();
    assert_eq!(last_used(&pool, kept.token.id).await, Some(recorded));
    sqlx::query(
        "UPDATE owner_access_token SET last_used_at=last_used_at - interval '61 seconds' WHERE id=$1",
    )
    .bind(kept.token.id)
    .execute(&pool)
    .await
    .unwrap();
    db.authenticate_access_token(&kept.secret).await.unwrap();
    assert!(last_used(&pool, kept.token.id).await.unwrap() >= recorded);

    sqlx::query("UPDATE owner_identity SET disabled=true WHERE id=$1")
        .bind(owner.identity_id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        db.authenticate_access_token(&kept.secret).await.unwrap(),
        None
    );

    let disabled = db
        .resolve_identity("https://idp", "disabled", "disabled@example.com")
        .await
        .unwrap();
    let issued = db.create_access_token(&disabled, &note).await.unwrap();
    sqlx::query("UPDATE owner SET disabled=true WHERE id=$1")
        .bind(disabled.id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        db.authenticate_access_token(&issued.secret).await.unwrap(),
        None
    );
}
