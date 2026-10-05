use super::database;
use palace_db::{ConversationCard, Database, DbError, ExcerptLine, Moment, MomentDetail, Owner};
use palace_domain::{
    ImportInput, ImportLimits, ImportRequest, ImportTarget, PathInput, Role, SessionId, Source,
};
use pretty_assertions::assert_eq;
use sqlx::PgPool;
use uuid::Uuid;

/// Restores the schema as it was before `0009_moments.sql` and `0010_path_titles.sql`, keeping
/// the data: occurrence times move back onto paths and the earliest path's title onto the
/// conversation. Migration tests replay from here to reach the current schema.
pub(super) const BEFORE_MOMENTS: &str = "
    ALTER TABLE conversation ADD COLUMN title text;
    UPDATE conversation c SET title=(SELECT p.title FROM conversation_path p WHERE p.conversation_id=c.id ORDER BY p.created_at,p.id LIMIT 1);
    ALTER TABLE conversation ALTER COLUMN title SET NOT NULL;
    ALTER TABLE conversation ADD CHECK(length(btrim(title)) > 0);
    ALTER TABLE conversation_path DROP COLUMN title;
    ALTER TABLE conversation_path ADD COLUMN occurred_at timestamptz;
    UPDATE conversation_path p SET occurred_at=m.occurred_at FROM moment m WHERE m.id=p.id;
    ALTER TABLE conversation_path ALTER COLUMN occurred_at SET NOT NULL;
    DROP TRIGGER path_detail_keeps_moment ON conversation_path;
    ALTER TABLE conversation_path DROP COLUMN kind;
    DROP TABLE moment;
    DROP FUNCTION require_moment_detail();";

/// Alternates user and assistant turns so every fixture has assistant prefixes to branch from.
fn history(contents: &[&str]) -> Vec<u8> {
    serde_json::to_vec(&contents.iter().enumerate().map(|(i, content)| {
        serde_json::json!({"role":if i % 2 == 0 {"user"} else {"assistant"},"content":content})
    }).collect::<Vec<_>>()).unwrap()
}
/// Creates a conversation whose first path has the given session, title and occurrence time.
async fn create(
    db: &Database,
    owner: &Owner,
    session: &str,
    contents: &[&str],
    occurred_at: i64,
) -> palace_db::ImportResult {
    let request = ImportRequest::parse(
        ImportInput {
            title: format!("title {session}"),
            source: Source::Chatgpt,
            session_id: session.into(),
            history: history(contents),
            idempotency_key: format!("create-{session}"),
            occurred_at,
        },
        ImportLimits::default(),
    )
    .unwrap();
    db.import_path(owner.scope(), &request).await.unwrap()
}
/// Imports another session into an existing tree.
fn branch(
    conversation_id: Uuid,
    session: &str,
    contents: &[&str],
    occurred_at: i64,
) -> ImportRequest {
    ImportRequest::parse_target(
        ImportTarget::Branch {
            conversation_id,
            session_id: SessionId::try_from(session.to_owned()).unwrap(),
            title: format!("title {session}"),
        },
        PathInput {
            history: history(contents),
            idempotency_key: format!("branch-{session}"),
            occurred_at,
        },
        ImportLimits::default(),
    )
    .unwrap()
}
/// Appends to a path under its current fixture title.
fn append(
    result: &palace_db::ImportResult,
    key: &str,
    contents: &[&str],
    occurred_at: i64,
) -> ImportRequest {
    ImportRequest::parse_target(
        ImportTarget::Update {
            conversation_id: result.conversation_id,
            path_id: result.path_id,
            title: "appended".into(),
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
/// Lists every stored moment as (id, occurred_at) in timeline order.
async fn moments(pool: &PgPool) -> Vec<(Uuid, i64)> {
    sqlx::query_as("SELECT id,(extract(epoch FROM occurred_at)*1000)::bigint FROM moment ORDER BY occurred_at,id")
        .fetch_all(pool)
        .await
        .unwrap()
}
/// Reduces a timeline to (id, occurred_at, title) for ordering assertions.
fn positions(timeline: Vec<Moment>) -> Vec<(Uuid, i64, String)> {
    timeline
        .into_iter()
        .map(|moment| match moment.detail {
            MomentDetail::Conversation(card) => (moment.id, moment.occurred_at, card.title),
        })
        .collect()
}

/// The database alone keeps moments and details paired, whatever order a transaction writes them.
/// Core test case:
/// - `specs/test-cases/server/moment/moment-timeline.md#every-moment-must-have-exactly-one-detail-of-its-own-kind`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn every_moment_must_have_exactly_one_detail_of_its_own_kind() {
    let (_container, db, pool) = database().await;
    let owner = db
        .resolve_identity("https://idp", "detail", "detail@example.com")
        .await
        .unwrap();
    let other = db
        .resolve_identity("https://idp", "other", "other@example.com")
        .await
        .unwrap();
    let first = create(&db, &owner, "s1", &["U1", "A1"], 1000).await;
    let foreign = create(&db, &other, "s2", &["U1", "A1"], 1000).await;
    let before = moments(&pool).await;

    // A moment without a detail cannot commit.
    let mut tx = pool.begin().await.unwrap();
    sqlx::query(
        "INSERT INTO moment(id,owner_id,kind,occurred_at) VALUES($1,$2,'conversation',now())",
    )
    .bind(Uuid::now_v7())
    .bind(owner.id)
    .execute(&mut *tx)
    .await
    .unwrap();
    assert!(tx.commit().await.is_err());
    // A detail cannot outlive its moment, and a moment cannot outlive its detail.
    assert!(
        sqlx::query("DELETE FROM moment WHERE id=$1")
            .bind(first.path_id)
            .execute(&pool)
            .await
            .is_err()
    );
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("DELETE FROM conversation_import WHERE path_id=$1")
        .bind(first.path_id)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("DELETE FROM conversation_path WHERE id=$1")
        .bind(first.path_id)
        .execute(&mut *tx)
        .await
        .unwrap();
    assert!(tx.commit().await.is_err());
    // No other kind exists yet, so neither side may claim one.
    for statement in [
        "UPDATE moment SET kind='sleep' WHERE id=$1",
        "UPDATE conversation_path SET kind='sleep' WHERE id=$1",
    ] {
        assert!(
            sqlx::query(statement)
                .bind(first.path_id)
                .execute(&pool)
                .await
                .is_err()
        );
    }
    // A detail of one owner cannot attach to another owner's moment.
    let stolen = Uuid::now_v7();
    let mut tx = pool.begin().await.unwrap();
    sqlx::query(
        "INSERT INTO moment(id,owner_id,kind,occurred_at) VALUES($1,$2,'conversation',now())",
    )
    .bind(stolen)
    .bind(owner.id)
    .execute(&mut *tx)
    .await
    .unwrap();
    let error = sqlx::query("INSERT INTO conversation_path(id,owner_id,conversation_id,source,session_id,head_message_id,message_count,title) VALUES($1,$2,$3,'chatgpt','stolen',$4,2,'t')")
        .bind(stolen)
        .bind(other.id)
        .bind(foreign.conversation_id)
        .bind(foreign.head_message_id)
        .execute(&mut *tx)
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("conversation_path_owner_id_id_kind_fkey"),
        "{error}"
    );
    drop(tx);
    assert_eq!(moments(&pool).await, before);
}

/// Imports, appends, retries and deletions move moments together with their paths.
/// Core test case:
/// - `specs/test-cases/server/moment/moment-timeline.md#conversation-path-writes-must-keep-their-moment-in-the-same-transaction`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn conversation_path_writes_keep_their_moment_in_the_same_transaction() {
    let (_container, db, pool) = database().await;
    let owner = db
        .resolve_identity("https://idp", "writes", "writes@example.com")
        .await
        .unwrap();
    let first = create(&db, &owner, "s1", &["U1", "A1"], 1000).await;
    let second = db
        .import_path(
            owner.scope(),
            &branch(first.conversation_id, "s2", &["U1", "A1", "U2", "A2"], 2000),
        )
        .await
        .unwrap();
    assert_eq!(
        moments(&pool).await,
        vec![(first.path_id, 1000), (second.path_id, 2000)]
    );
    // An append moves the moment to the latest continuation; a retry leaves it where it is.
    let appended = append(&first, "append", &["U1", "A1", "U3", "A3"], 5000);
    db.import_path(owner.scope(), &appended).await.unwrap();
    db.import_path(
        owner.scope(),
        &append(&first, "time-only", &["U1", "A1", "U3", "A3"], 6000),
    )
    .await
    .unwrap();
    db.import_path(owner.scope(), &appended).await.unwrap();
    assert_eq!(
        moments(&pool).await,
        vec![(second.path_id, 2000), (first.path_id, 6000)]
    );
    // A failed import leaves no moment behind.
    sqlx::raw_sql("CREATE FUNCTION fail_import() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected'; END $$; CREATE TRIGGER fail_import BEFORE INSERT ON conversation_import FOR EACH ROW EXECUTE FUNCTION fail_import();").execute(&pool).await.unwrap();
    assert!(
        db.import_path(
            owner.scope(),
            &branch(first.conversation_id, "s3", &["U1", "A1", "U4"], 3000),
        )
        .await
        .is_err()
    );
    sqlx::raw_sql("DROP TRIGGER fail_import ON conversation_import")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(moments(&pool).await.len(), 2);
    db.delete_path(owner.scope(), first.conversation_id, second.path_id)
        .await
        .unwrap();
    assert_eq!(moments(&pool).await, vec![(first.path_id, 6000)]);
    db.delete_conversation(owner.scope(), first.conversation_id)
        .await
        .unwrap();
    assert_eq!(moments(&pool).await, vec![]);
}

/// A day is the caller's half-open range; paths appear on their own days with their own titles.
/// Core test case:
/// - `specs/test-cases/server/moment/moment-timeline.md#day-timeline-must-return-moments-inside-the-callers-local-day-range`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn day_timeline_returns_moments_inside_the_callers_range() {
    let (_container, db, _pool) = database().await;
    let owner = db
        .resolve_identity("https://idp", "days", "days@example.com")
        .await
        .unwrap();
    let other = db
        .resolve_identity("https://idp", "elsewhere", "elsewhere@example.com")
        .await
        .unwrap();
    let day = 86_400_000_i64;
    let (start, end) = (10 * day, 11 * day);
    let before = create(&db, &owner, "before", &["x"], start - 1).await;
    let at_end = create(&db, &owner, "at-end", &["x"], end).await;
    let late = create(&db, &owner, "late", &["x"], end - 1).await;
    let early = create(&db, &owner, "early", &["U1", "A1"], start).await;
    create(&db, &other, "foreign", &["x"], start).await;
    let next_day = db
        .import_path(
            owner.scope(),
            &branch(early.conversation_id, "next", &["U1", "A1", "U2"], end + 5),
        )
        .await
        .unwrap();
    assert_eq!(
        positions(db.timeline(owner.scope(), start, end).await.unwrap()),
        vec![
            (early.path_id, start, "title early".into()),
            (late.path_id, end - 1, "title late".into()),
        ]
    );
    assert_eq!(
        positions(db.timeline(owner.scope(), end, end + day).await.unwrap()),
        vec![
            (at_end.path_id, end, "title at-end".into()),
            (next_day.path_id, end + 5, "title next".into()),
        ]
    );
    // An append moves the moment out of its old day and into the new one.
    db.import_path(owner.scope(), &append(&late, "move", &["x", "y"], end + 1))
        .await
        .unwrap();
    assert_eq!(
        positions(db.timeline(owner.scope(), start, end).await.unwrap()),
        vec![(early.path_id, start, "title early".into())]
    );
    assert_eq!(
        positions(db.timeline(owner.scope(), end, end + day).await.unwrap())[1],
        (late.path_id, end + 1, "appended".into())
    );
    assert_eq!(
        positions(
            db.timeline(owner.scope(), start - day, start)
                .await
                .unwrap()
        ),
        vec![(before.path_id, start - 1, "title before".into())]
    );
    for (from, to) in [(start, start), (end, start), (start, start + 2 * day + 1)] {
        assert!(matches!(
            db.timeline(owner.scope(), from, to).await,
            Err(DbError::Input(_))
        ));
    }
}

/// Each card counts and quotes only its own path, named by the path and sourced by the tree.
/// Core test case:
/// - `specs/test-cases/server/moment/moment-timeline.md#conversation-cards-must-summarize-their-own-path`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn conversation_cards_summarize_their_own_path() {
    let (_container, db, _pool) = database().await;
    let owner = db
        .resolve_identity("https://idp", "cards", "cards@example.com")
        .await
        .unwrap();
    let long_answer = format!("## 结论\n\n{}", "很长的回答 long answer ".repeat(20));
    let first = create(
        &db,
        &owner,
        "full",
        &["**你好**，周末去哪？", &long_answer, "U2", "A2"],
        1000,
    )
    .await;
    // A second session that stops inside the first one's path.
    let internal = db
        .import_path(
            owner.scope(),
            &branch(
                first.conversation_id,
                "internal",
                &["**你好**，周末去哪？", &long_answer],
                1001,
            ),
        )
        .await
        .unwrap();
    let single = create(&db, &owner, "single", &["只有一句"], 1002).await;
    db.update_path_metadata(
        owner.scope(),
        first.conversation_id,
        first.path_id,
        "renamed",
        Source::Gemini,
    )
    .await
    .unwrap();
    let opening = vec![
        ExcerptLine {
            role: Role::User,
            text: "你好，周末去哪？".into(),
        },
        ExcerptLine {
            role: Role::Assistant,
            text: format!(
                "{}…",
                format!("结论 {}", "很长的回答 long answer ".repeat(20))
                    .chars()
                    .take(120)
                    .collect::<String>()
                    .trim_end()
            ),
        },
    ];
    let card = |id: Uuid, occurred_at: i64, card: ConversationCard| Moment {
        id,
        occurred_at,
        detail: MomentDetail::Conversation(card),
    };
    assert_eq!(
        db.timeline(owner.scope(), 0, 2000).await.unwrap(),
        vec![
            card(
                first.path_id,
                1000,
                ConversationCard {
                    conversation_id: first.conversation_id,
                    title: "renamed".into(),
                    source: Source::Gemini,
                    message_count: 4,
                    excerpt: opening.clone(),
                }
            ),
            card(
                internal.path_id,
                1001,
                ConversationCard {
                    conversation_id: first.conversation_id,
                    title: "title internal".into(),
                    source: Source::Gemini,
                    message_count: 2,
                    excerpt: opening,
                }
            ),
            card(
                single.path_id,
                1002,
                ConversationCard {
                    conversation_id: single.conversation_id,
                    title: "title single".into(),
                    source: Source::Chatgpt,
                    message_count: 1,
                    excerpt: vec![ExcerptLine {
                        role: Role::User,
                        text: "只有一句".into(),
                    }],
                }
            ),
        ]
    );
}

/// Upgrading turns every path into a moment and names every path after its old conversation.
/// Core test cases:
/// - `specs/test-cases/server/moment/moment-timeline.md#migration-must-turn-existing-paths-into-moments-without-changing-identities`
/// - `specs/test-cases/server/conversation/message-tree.md#title-migration-must-copy-the-conversation-title-to-every-path`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn migration_turns_paths_into_moments_and_copies_titles() {
    let (_container, db, pool) = database().await;
    let owner = db
        .resolve_identity("https://idp", "upgrade", "upgrade@example.com")
        .await
        .unwrap();
    let first = create(&db, &owner, "s1", &["U1", "A1"], 1000).await;
    let second = db
        .import_path(
            owner.scope(),
            &branch(first.conversation_id, "s2", &["U1", "A1", "U2", "A2"], 2000),
        )
        .await
        .unwrap();
    let alone = create(&db, &owner, "alone", &["x"], 3000).await;
    let before_tree = db
        .conversation_detail(owner.scope(), first.conversation_id)
        .await
        .unwrap();
    let before_alone = db
        .conversation_detail(owner.scope(), alone.conversation_id)
        .await
        .unwrap();
    let before_moments = moments(&pool).await;
    let mut tx = pool.begin().await.unwrap();
    sqlx::raw_sql(BEFORE_MOMENTS)
        .execute(&mut *tx)
        .await
        .unwrap();
    for migration in [
        include_str!("../../migrations/0009_moments.sql"),
        include_str!("../../migrations/0010_path_titles.sql"),
    ] {
        sqlx::raw_sql(migration).execute(&mut *tx).await.unwrap();
    }
    tx.commit().await.unwrap();
    // Before the upgrade the tree had one title, the earliest path's; both paths inherit it.
    let mut expected = before_tree;
    for path in &mut expected.paths {
        path.title = "title s1".into();
    }
    assert_eq!(
        db.conversation_detail(owner.scope(), first.conversation_id)
            .await
            .unwrap(),
        expected
    );
    assert_eq!(
        db.conversation_detail(owner.scope(), alone.conversation_id)
            .await
            .unwrap(),
        before_alone
    );
    assert_eq!(moments(&pool).await, before_moments);
    assert_eq!(
        before_moments,
        vec![
            (first.path_id, 1000),
            (second.path_id, 2000),
            (alone.path_id, 3000)
        ]
    );
}
