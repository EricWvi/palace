use super::database;
use palace_db::DbError;
use palace_domain::{
    ImportInput, ImportLimits, ImportRequest, ImportTarget, PathInput, SessionId, Source,
};
use pretty_assertions::assert_eq;
use uuid::Uuid;

/// The single-path upgrade preserves message identities and recovers source metadata from legacy rows.
/// Core test case:
/// - `specs/test-cases/server/conversation/message-tree.md#migration-must-preserve-existing-linear-conversation-identities`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn migration_retains_existing_linear_conversations() {
    let (_container, db, pool) = database().await;
    let owner = db
        .resolve_identity("https://idp", "migration", "migration@example.com")
        .await
        .unwrap();
    let first = db
        .import_path(owner.scope(), &create("legacy", &["U1", "A1"]))
        .await
        .unwrap();
    let before = db
        .conversation_detail(owner.scope(), first.conversation_id)
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    sqlx::raw_sql("ALTER TABLE conversation_import DROP COLUMN path_id;
        ALTER TABLE conversation ADD COLUMN session_id text;
        UPDATE conversation c SET session_id=p.session_id FROM conversation_path p WHERE p.conversation_id=c.id;
        DROP TABLE conversation_path;
        ALTER TABLE conversation DROP CONSTRAINT conversation_owner_id_id_source_key;
        UPDATE conversation_import SET result=result-'path_id';")
        .execute(&mut *tx).await.unwrap();
    sqlx::raw_sql(include_str!("../../migrations/0007_conversation_paths.sql"))
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let after = db
        .conversation_detail(owner.scope(), first.conversation_id)
        .await
        .unwrap();
    assert_eq!(
        (after.conversation, after.messages),
        (before.conversation, before.messages)
    );
    let mut expected_path = before.paths.into_iter().next().unwrap();
    expected_path.id = first.conversation_id;
    // Migration uses the import receipt's creation time, not a fabricated occurrence time.
    let receipt: (i64,i64) = sqlx::query_as("SELECT (extract(epoch FROM created_at)*1000)::bigint,(extract(epoch FROM date_trunc('milliseconds',created_at))*1000)::bigint FROM conversation_import WHERE id=$1")
        .bind(first.import_id).fetch_one(&pool).await.unwrap();
    expected_path.created_at = receipt.0;
    expected_path.updated_at = receipt.1;
    assert_eq!(after.paths, vec![expected_path]);
}

/// Alternating messages make the shared assistant boundary explicit in each fixture.
fn history(contents: &[&str]) -> Vec<u8> {
    serde_json::to_vec(&contents.iter().enumerate().map(|(i, content)| {
        serde_json::json!({"role":if i % 2 == 0 {"user"} else {"assistant"},"content":content})
    }).collect::<Vec<_>>()).unwrap()
}

/// Card chronology follows occurrence time while opening follows the most recently updated session.
/// Core test case:
/// - `specs/test-cases/server/conversation/message-tree.md#fork-selection-must-resolve-to-one-real-source-session`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn library_separates_occurrence_order_from_default_path_selection() {
    let (_container, db, _pool) = database().await;
    let owner = db
        .resolve_identity("https://idp", "library", "library@example.com")
        .await
        .unwrap();
    let first = db
        .import_path(owner.scope(), &create("s1", &["U1", "A1"]))
        .await
        .unwrap();
    db.import_path(
        owner.scope(),
        &branch(first.conversation_id, "s2", &["U1", "A1", "U2", "A2"]),
    )
    .await
    .unwrap();
    db.import_path(
        owner.scope(),
        &change(
            ImportTarget::Update {
                conversation_id: first.conversation_id,
                path_id: first.path_id,
            },
            "older-occurrence",
            &["U1", "A1"],
            /*occurred_at*/ 500,
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        db.list_conversations(owner.scope()).await.unwrap(),
        vec![palace_db::ConversationSummary {
            id: first.conversation_id,
            title: "tree".into(),
            source: Source::Chatgpt,
            session_ids: vec!["s1".into(), "s2".into()],
            path_count: 2,
            path_id: first.path_id,
            occurred_at: 2000,
            head_message_id: first.head_message_id,
            message_count: 4,
        }]
    );
}
/// Creates a root import without sharing identity between separate test conversations.
fn create(session: &str, contents: &[&str]) -> ImportRequest {
    ImportRequest::parse(
        ImportInput {
            title: "tree".into(),
            source: Source::Chatgpt,
            session_id: session.into(),
            history: history(contents),
            idempotency_key: format!("create-{session}"),
            occurred_at: 1000,
        },
        ImportLimits::default(),
    )
    .unwrap()
}
/// Explicit path targets ensure retries cannot cross from creation into an update operation.
fn change(target: ImportTarget, key: &str, contents: &[&str], occurred_at: i64) -> ImportRequest {
    ImportRequest::parse_target(
        target,
        PathInput {
            history: history(contents),
            idempotency_key: key.into(),
            occurred_at,
        },
        ImportLimits::default(),
    )
    .unwrap()
}
/// Builds a fresh external identity for a branch of one existing tree.
fn branch(id: Uuid, session: &str, contents: &[&str]) -> ImportRequest {
    change(
        ImportTarget::Branch {
            conversation_id: id,
            session_id: SessionId::try_from(session.to_owned()).unwrap(),
        },
        session,
        contents,
        /*occurred_at*/ 2000,
    )
}

/// Source correction cascades to every path without changing tree or receipt identity.
/// Core test case:
/// - `specs/test-cases/server/conversation/message-tree.md#metadata-correction-must-atomically-preserve-conversation-tree-identities`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn metadata_correction_is_atomic_and_preserves_tree_identities() {
    let (_container, db, pool) = database().await;
    let owner = db
        .resolve_identity("https://idp", "metadata", "metadata@example.com")
        .await
        .unwrap();
    let foreign = db
        .resolve_identity("https://idp", "foreign", "foreign@example.com")
        .await
        .unwrap();
    let first = db
        .import_path(owner.scope(), &create("s1", &["U1", "A1"]))
        .await
        .unwrap();
    db.import_path(
        owner.scope(),
        &branch(first.conversation_id, "s2", &["U1", "A1", "U2", "A2"]),
    )
    .await
    .unwrap();
    let blocker = ImportRequest::parse(
        ImportInput {
            title: "blocker".into(),
            source: Source::Gemini,
            session_id: "s2".into(),
            history: history(&["other"]),
            idempotency_key: "blocker".into(),
            occurred_at: 3000,
        },
        ImportLimits::default(),
    )
    .unwrap();
    db.import_path(owner.scope(), &blocker).await.unwrap();
    let before = db
        .conversation_detail(owner.scope(), first.conversation_id)
        .await
        .unwrap();
    let receipts_before: Vec<(Uuid, Uuid, sqlx::types::Json<serde_json::Value>)> =
        sqlx::query_as("SELECT id,path_id,result FROM conversation_import WHERE owner_id=$1 AND conversation_id=$2 ORDER BY id")
            .bind(owner.id).bind(first.conversation_id).fetch_all(&pool).await.unwrap();

    assert!(matches!(
        db.update_conversation_metadata(
            owner.scope(),
            first.conversation_id,
            "conflicting",
            Source::Gemini,
        )
        .await,
        Err(DbError::DuplicateSession)
    ));
    assert_eq!(
        db.conversation_detail(owner.scope(), first.conversation_id)
            .await
            .unwrap(),
        before
    );
    assert!(matches!(
        db.update_conversation_metadata(
            foreign.scope(),
            first.conversation_id,
            "foreign",
            Source::Grok,
        )
        .await,
        Err(DbError::NotFound)
    ));
    assert!(matches!(
        db.update_conversation_metadata(owner.scope(), first.conversation_id, " ", Source::Grok,)
            .await,
        Err(DbError::Input(_))
    ));

    db.update_conversation_metadata(
        owner.scope(),
        first.conversation_id,
        "corrected",
        Source::Grok,
    )
    .await
    .unwrap();
    let after = db
        .conversation_detail(owner.scope(), first.conversation_id)
        .await
        .unwrap();
    let mut expected = before;
    expected.conversation.title = "corrected".into();
    expected.conversation.source = Source::Grok;
    assert_eq!(after, expected);
    let path_sources: Vec<String> = sqlx::query_scalar(
        "SELECT source FROM conversation_path WHERE owner_id=$1 AND conversation_id=$2 ORDER BY id",
    )
    .bind(owner.id)
    .bind(first.conversation_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(path_sources, vec!["grok", "grok"]);
    let receipts_after: Vec<(Uuid, Uuid, sqlx::types::Json<serde_json::Value>)> =
        sqlx::query_as("SELECT id,path_id,result FROM conversation_import WHERE owner_id=$1 AND conversation_id=$2 ORDER BY id")
            .bind(owner.id).bind(first.conversation_id).fetch_all(&pool).await.unwrap();
    assert_eq!(receipts_after, receipts_before);

    let released = ImportRequest::parse(
        ImportInput {
            title: "released".into(),
            source: Source::Chatgpt,
            session_id: "s1".into(),
            history: history(&["new"]),
            idempotency_key: "released".into(),
            occurred_at: 4000,
        },
        ImportLimits::default(),
    )
    .unwrap();
    db.import_path(owner.scope(), &released).await.unwrap();
}

/// Concurrent branches share ancestors; failed receipts roll back all newly allocated state.
/// Core test cases:
/// - `specs/test-cases/server/conversation/message-tree.md#source-sessions-must-uniquely-identify-owned-paths`
/// - `specs/test-cases/server/import/linear-path-import.md#new-branches-must-share-an-assistant-prefix`
/// - `specs/test-cases/server/import/linear-path-import.md#import-receipts-and-tree-mutations-must-commit-atomically`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn branches_share_prefix_and_failures_roll_back() {
    let (_container, db, pool) = database().await;
    let owner = db
        .resolve_identity("https://idp", "a", "a@example.com")
        .await
        .unwrap();
    let first = db
        .import_path(owner.scope(), &create("s1", &["U1", "A1", "U2", "A2"]))
        .await
        .unwrap();
    let left = branch(first.conversation_id, "s2", &["U1", "A1", "U3", "A3"]);
    let right = branch(
        first.conversation_id,
        "s3",
        &["U1", "A1", "U3", "A3", "U4", "A4"],
    );
    let (left, right) = tokio::join!(
        db.import_path(owner.scope(), &left),
        db.import_path(owner.scope(), &right)
    );
    let left = left.unwrap();
    let right = right.unwrap();
    assert_eq!(
        (left.created + right.created, left.reused + right.reused),
        (4, 6)
    );
    let before = db
        .conversation_detail(owner.scope(), first.conversation_id)
        .await
        .unwrap();
    assert_eq!((before.messages.len(), before.paths.len()), (8, 3));
    for (session, content) in [
        ("unrelated", vec!["other", "answer"]),
        ("user-only", vec!["U1", "different answer"]),
    ] {
        assert!(matches!(
            db.import_path(
                owner.scope(),
                &branch(first.conversation_id, session, &content)
            )
            .await,
            Err(DbError::Input(_))
        ));
        assert_eq!(
            db.conversation_detail(owner.scope(), first.conversation_id)
                .await
                .unwrap(),
            before
        );
    }
    assert!(matches!(
        db.import_path(owner.scope(), &create("s2", &["x"])).await,
        Err(DbError::DuplicateSession)
    ));
    assert!(matches!(
        db.import_path(
            owner.scope(),
            &branch(first.conversation_id, "s1", &["U1", "A1"])
        )
        .await,
        Err(DbError::DuplicateSession)
    ));
    let other = db
        .resolve_identity("https://idp", "b", "b@example.com")
        .await
        .unwrap();
    assert!(matches!(
        db.import_path(
            other.scope(),
            &branch(first.conversation_id, "foreign", &["U1", "A1"])
        )
        .await,
        Err(DbError::NotFound)
    ));
    assert!(
        sqlx::query("UPDATE conversation_path SET head_message_id=$1 WHERE id=$2")
            .bind(Uuid::now_v7())
            .bind(left.path_id)
            .execute(&pool)
            .await
            .is_err()
    );
    sqlx::raw_sql("CREATE FUNCTION fail_import() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected'; END $$; CREATE TRIGGER fail_import BEFORE INSERT ON conversation_import FOR EACH ROW EXECUTE FUNCTION fail_import();").execute(&pool).await.unwrap();
    assert!(
        db.import_path(
            owner.scope(),
            &branch(first.conversation_id, "failed", &["U1", "A1", "new"])
        )
        .await
        .is_err()
    );
    assert!(
        db.import_path(owner.scope(), &create("fresh", &["fresh"]))
            .await
            .is_err()
    );
    assert_eq!(
        db.conversation_detail(owner.scope(), first.conversation_id)
            .await
            .unwrap(),
        before
    );
    let counts: (i64,i64,i64,i64) = sqlx::query_as("SELECT (SELECT count(*) FROM conversation),(SELECT count(*) FROM message),(SELECT count(*) FROM conversation_path),(SELECT count(*) FROM conversation_import)").fetch_one(&pool).await.unwrap();
    assert_eq!(counts, (1, 8, 3, 3));
}

/// Updates retain the full historical prefix, preserve timestamps on retries and reject stale histories.
/// Core test cases:
/// - `specs/test-cases/server/import/linear-path-import.md#path-updates-must-retain-the-entire-historical-prefix`
/// - `specs/test-cases/server/import/linear-path-import.md#import-receipts-and-tree-mutations-must-commit-atomically`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn updates_are_append_only_and_retries_preserve_path_metadata() {
    let (_container, db, _pool) = database().await;
    let owner = db
        .resolve_identity("https://idp", "a", "a@example.com")
        .await
        .unwrap();
    let first = db
        .import_path(owner.scope(), &create("s1", &["U1", "A1"]))
        .await
        .unwrap();
    let target = ImportTarget::Update {
        conversation_id: first.conversation_id,
        path_id: first.path_id,
    };
    let before = db
        .conversation_detail(owner.scope(), first.conversation_id)
        .await
        .unwrap();
    for (key, content) in [
        ("short", vec!["U1"]),
        ("edit", vec!["U1", "edited"]),
        ("root", vec!["changed", "A1"]),
    ] {
        assert!(matches!(
            db.import_path(
                owner.scope(),
                &change(target.clone(), key, &content, /*occurred_at*/ 9000)
            )
            .await,
            Err(DbError::Input(_))
        ));
        assert_eq!(
            db.conversation_detail(owner.scope(), first.conversation_id)
                .await
                .unwrap(),
            before
        );
    }
    let input = change(
        target.clone(),
        "append",
        &["U1", "A1", "U2", "A2"],
        /*occurred_at*/ 3000,
    );
    let result = db.import_path(owner.scope(), &input).await.unwrap();
    let after = db
        .conversation_detail(owner.scope(), first.conversation_id)
        .await
        .unwrap();
    assert_eq!(
        (result.path_id, result.created, result.reused),
        (first.path_id, 2, 2)
    );
    assert_eq!(
        (after.paths[0].occurred_at, after.paths[0].created_at),
        (3000, before.paths[0].created_at)
    );
    assert!(after.paths[0].updated_at > before.paths[0].updated_at);
    assert_eq!(db.import_path(owner.scope(), &input).await.unwrap(), result);
    assert_eq!(
        db.conversation_detail(owner.scope(), first.conversation_id)
            .await
            .unwrap(),
        after
    );
    assert!(matches!(
        db.import_path(
            owner.scope(),
            &change(
                target.clone(),
                "stale",
                &["U1", "A1", "other"],
                /*occurred_at*/ 4000
            )
        )
        .await,
        Err(DbError::Input(_))
    ));
    let time_only = change(
        target,
        "time-only",
        &["U1", "A1", "U2", "A2"],
        /*occurred_at*/ 5000,
    );
    assert_eq!(
        db.import_path(owner.scope(), &time_only)
            .await
            .unwrap()
            .created,
        0
    );
    assert_eq!(
        db.conversation_detail(owner.scope(), first.conversation_id)
            .await
            .unwrap()
            .paths[0]
            .occurred_at,
        5000
    );
}

/// Identical and internal-endpoint paths survive deletion of siblings without losing shared messages.
/// Core test cases:
/// - `specs/test-cases/server/conversation/message-tree.md#shared-and-internal-endpoint-paths-must-remain-independently-manageable`
/// - `specs/test-cases/server/import/linear-path-import.md#new-branches-must-share-an-assistant-prefix`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn deletion_preserves_shared_messages_and_owner_boundaries() {
    let (_container, db, pool) = database().await;
    let owner = db
        .resolve_identity("https://idp", "a", "a@example.com")
        .await
        .unwrap();
    let foreign = db
        .resolve_identity("https://idp", "b", "b@example.com")
        .await
        .unwrap();
    let first = db
        .import_path(owner.scope(), &create("s1", &["U1", "A1", "U2", "A2"]))
        .await
        .unwrap();
    let short = db
        .import_path(
            owner.scope(),
            &branch(first.conversation_id, "s2", &["U1", "A1"]),
        )
        .await
        .unwrap();
    let same = db
        .import_path(
            owner.scope(),
            &branch(first.conversation_id, "s3", &["U1", "A1"]),
        )
        .await
        .unwrap();
    assert_eq!(
        (short.head_message_id, short.created, same.created),
        (same.head_message_id, 0, 0)
    );
    assert!(matches!(
        db.delete_path(foreign.scope(), first.conversation_id, first.path_id)
            .await,
        Err(DbError::NotFound)
    ));
    assert!(matches!(
        db.delete_conversation(foreign.scope(), first.conversation_id)
            .await,
        Err(DbError::NotFound)
    ));
    db.delete_path(owner.scope(), first.conversation_id, first.path_id)
        .await
        .unwrap();
    let tree = db
        .conversation_detail(owner.scope(), first.conversation_id)
        .await
        .unwrap();
    assert_eq!((tree.messages.len(), tree.paths.len()), (2, 2));
    db.delete_path(owner.scope(), first.conversation_id, short.path_id)
        .await
        .unwrap();
    assert!(matches!(
        db.delete_path(owner.scope(), first.conversation_id, same.path_id)
            .await,
        Err(DbError::Input(_))
    ));
    db.delete_conversation(owner.scope(), first.conversation_id)
        .await
        .unwrap();
    let counts: (i64,i64,i64,i64) = sqlx::query_as("SELECT (SELECT count(*) FROM conversation),(SELECT count(*) FROM message),(SELECT count(*) FROM conversation_path),(SELECT count(*) FROM conversation_import)").fetch_one(&pool).await.unwrap();
    assert_eq!(counts, (0, 0, 0, 0));
}

/// Competing initial imports cannot create duplicate session identities or orphan cards.
/// Core test cases:
/// - `specs/test-cases/server/conversation/message-tree.md#source-sessions-must-uniquely-identify-owned-paths`
/// - `specs/test-cases/server/import/linear-path-import.md#import-receipts-and-tree-mutations-must-commit-atomically`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn concurrent_duplicate_sessions_create_only_one_card() {
    let (_container, db, pool) = database().await;
    let owner = db
        .resolve_identity("https://idp", "a", "a@example.com")
        .await
        .unwrap();
    let a = create("same", &["U1", "A1"]);
    let b = ImportRequest::parse(
        ImportInput {
            title: "other".into(),
            source: Source::Chatgpt,
            session_id: "same".into(),
            history: history(&["U1", "A1"]),
            idempotency_key: "other-key".into(),
            occurred_at: 2000,
        },
        ImportLimits::default(),
    )
    .unwrap();
    let (a, b) = tokio::join!(
        db.import_path(owner.scope(), &a),
        db.import_path(owner.scope(), &b)
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert!(
        matches!(a, Err(DbError::DuplicateSession)) || matches!(b, Err(DbError::DuplicateSession))
    );
    let counts: (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM conversation),(SELECT count(*) FROM conversation_path)",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(counts, (1, 1));
}
