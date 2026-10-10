use super::database;
use super::paths::{branch, create, history};
use palace_db::{Database, DbError, ImportResult, OwnerScope};
use palace_domain::{
    ImportLimits, ImportRequest, ImportTarget, PathInput, UpdateHistory, read_path,
};
use pretty_assertions::assert_eq;
use uuid::Uuid;

/// Builds an update of one path in the given history mode, titled so the rename is observable.
fn update(
    path: &ImportResult,
    history_mode: UpdateHistory,
    key: &str,
    contents: &[&str],
    occurred_at: i64,
) -> ImportRequest {
    ImportRequest::parse_target(
        ImportTarget::Update {
            conversation_id: path.conversation_id,
            path_id: path.path_id,
            title: "delta".into(),
            history: history_mode,
        },
        PathInput {
            history: history(contents),
            idempotency_key: key.into(),
            occurred_at,
        },
        ImportLimits::default(),
    )
    .unwrap()
}
/// Anchors a delta on the head the caller believes the path has.
fn delta(base_head_message_id: Uuid) -> UpdateHistory {
    UpdateHistory::Delta {
        base_head_message_id,
    }
}
/// Reads the contents from root to the path head, which is what a reader of that path sees.
async fn path_contents(db: &Database, owner: OwnerScope, path: &ImportResult) -> Vec<String> {
    let detail = db
        .conversation_detail(owner, path.conversation_id)
        .await
        .unwrap();
    let head = detail
        .paths
        .iter()
        .find(|candidate| candidate.id == path.path_id)
        .unwrap()
        .head_message_id;
    read_path(&detail.conversation, &detail.messages, head)
        .unwrap()
        .into_iter()
        .map(|message| message.content)
        .collect()
}

/// A delta continues below the head, reuses a continuation another path already wrote, and leaves history alone.
/// Core test case:
/// - `specs/test-cases/server/import/delta-update.md#delta-updates-must-extend-the-current-head-without-touching-history`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn delta_extends_the_head_and_reuses_shared_continuations() {
    let (_container, db, _pool) = database().await;
    let owner = db
        .resolve_identity("https://idp", "delta", "delta@example.com")
        .await
        .unwrap();
    let first = db
        .import_path(owner.scope(), &create("s1", &["U1", "A1", "U2", "A2"]))
        .await
        .unwrap();
    let second = db
        .import_path(
            owner.scope(),
            &branch(
                first.conversation_id,
                "s2",
                &["U1", "A1", "U2", "A2", "U3", "A3"],
            ),
        )
        .await
        .unwrap();
    let before = db
        .conversation_detail(owner.scope(), first.conversation_id)
        .await
        .unwrap();
    // The source rewrote U1, so only the new turns are uploaded.
    let result = db
        .import_path(
            owner.scope(),
            &update(
                &first,
                delta(first.head_message_id),
                "delta",
                &["U3", "A3", "U4"],
                /*occurred_at*/ 5000,
            ),
        )
        .await
        .unwrap();
    assert_eq!((result.created, result.reused), (1, 2));
    assert_eq!(
        path_contents(&db, owner.scope(), &first).await,
        ["U1", "A1", "U2", "A2", "U3", "A3", "U4"]
    );
    let after = db
        .conversation_detail(owner.scope(), first.conversation_id)
        .await
        .unwrap();
    // Path order follows updated_at, so each path is located by identity rather than position.
    let take = |paths: &mut Vec<palace_db::ConversationPath>, id| {
        let index = paths.iter().position(|path| path.id == id).unwrap();
        paths.remove(index)
    };
    let (mut before_paths, mut after_paths) = (before.paths, after.paths);
    let mut expected_first = take(&mut before_paths, first.path_id);
    let updated = take(&mut after_paths, first.path_id);
    expected_first.title = "delta".into();
    expected_first.head_message_id = result.head_message_id;
    expected_first.message_count = 7;
    expected_first.occurred_at = 5000;
    expected_first.updated_at = updated.updated_at;
    assert_eq!(updated, expected_first);
    // Every earlier message and the other path are untouched; only U4 was added.
    assert_eq!(after.messages[..before.messages.len()], before.messages[..]);
    assert_eq!(after.messages.len(), before.messages.len() + 1);
    assert_eq!(after_paths, before_paths);
    assert_eq!(before_paths[0].id, second.path_id);
    // Roles need not alternate across the boundary: the head is a user message and so is the delta.
    let consecutive = db
        .import_path(
            owner.scope(),
            &update(
                &result,
                delta(result.head_message_id),
                "consecutive",
                &["U5"],
                /*occurred_at*/ 6000,
            ),
        )
        .await
        .unwrap();
    assert_eq!((consecutive.created, consecutive.reused), (1, 0));
    assert_eq!(
        path_contents(&db, owner.scope(), &first).await,
        ["U1", "A1", "U2", "A2", "U3", "A3", "U4", "U5"]
    );
}

/// A delta built on an old head never lands after messages its author did not see, and retries stay within one mode.
/// Core test cases:
/// - `specs/test-cases/server/import/delta-update.md#delta-updates-must-reject-a-stale-base-head-atomically`
/// - `specs/test-cases/server/import/delta-update.md#idempotency-keys-must-not-cross-history-modes`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn stale_delta_bases_are_rejected_without_writes() {
    let (_container, db, _pool) = database().await;
    let owner = db
        .resolve_identity("https://idp", "stale", "stale@example.com")
        .await
        .unwrap();
    let first = db
        .import_path(owner.scope(), &create("s1", &["U1", "A1"]))
        .await
        .unwrap();
    let base = delta(first.head_message_id);
    let submitted = update(
        &first,
        base,
        "first",
        &["U2", "A2"],
        /*occurred_at*/ 2000,
    );
    let appended = db.import_path(owner.scope(), &submitted).await.unwrap();
    let after_first = db
        .conversation_detail(owner.scope(), first.conversation_id)
        .await
        .unwrap();
    // A network retry returns the committed result; the same JSON under the other mode is a different request.
    assert_eq!(
        db.import_path(owner.scope(), &submitted).await.unwrap(),
        appended
    );
    assert!(matches!(
        db.import_path(
            owner.scope(),
            &update(
                &first,
                UpdateHistory::Full,
                "first",
                &["U2", "A2"],
                /*occurred_at*/ 2000
            )
        )
        .await,
        Err(DbError::Conflict)
    ));
    // Resubmitting the same delta under a new key would duplicate the tail, so the moved head rejects it.
    assert!(matches!(
        db.import_path(
            owner.scope(),
            &update(
                &first,
                base,
                "again",
                &["U2", "A2"],
                /*occurred_at*/ 2000
            )
        )
        .await,
        Err(DbError::StaleHead)
    ));
    assert_eq!(
        db.conversation_detail(owner.scope(), first.conversation_id)
            .await
            .unwrap(),
        after_first
    );
    // A full update moves the head just as a delta does.
    let full = db
        .import_path(
            owner.scope(),
            &update(
                &first,
                UpdateHistory::Full,
                "full",
                &["U1", "A1", "U2", "A2", "U3"],
                /*occurred_at*/ 3000,
            ),
        )
        .await
        .unwrap();
    let after_full = db
        .conversation_detail(owner.scope(), first.conversation_id)
        .await
        .unwrap();
    assert_ne!(after_full, after_first);
    assert!(matches!(
        db.import_path(
            owner.scope(),
            &update(
                &first,
                delta(appended.head_message_id),
                "behind-full",
                &["X"],
                /*occurred_at*/ 4000
            )
        )
        .await,
        Err(DbError::StaleHead)
    ));
    assert_eq!(
        db.conversation_detail(owner.scope(), first.conversation_id)
            .await
            .unwrap(),
        after_full
    );
    // Two windows appending from the same head: the write lock lets exactly one of them land.
    let left = update(
        &first,
        delta(full.head_message_id),
        "left",
        &["L"],
        /*occurred_at*/ 5000,
    );
    let right = update(
        &first,
        delta(full.head_message_id),
        "right",
        &["R"],
        /*occurred_at*/ 5000,
    );
    let (left, right) = tokio::join!(
        db.import_path(owner.scope(), &left),
        db.import_path(owner.scope(), &right)
    );
    assert_eq!(usize::from(left.is_ok()) + usize::from(right.is_ok()), 1);
    assert!(matches!(left, Err(DbError::StaleHead)) || matches!(right, Err(DbError::StaleHead)));
    let contents = path_contents(&db, owner.scope(), &first).await;
    assert_eq!(contents.len(), 6);
    assert!(matches!(contents[5].as_str(), "L" | "R"));
}
