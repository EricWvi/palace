use super::database;
use palace_db::{
    CONVERSATION_PAGE_SIZE, ConversationListItem, ConversationListRequest, ConversationPage,
    Database, Owner, SearchTerm,
};
use palace_domain::{
    ImportInput, ImportLimits, ImportRequest, ImportTarget, PathInput, SessionId, Source,
};
use pretty_assertions::assert_eq;
use sqlx::PgPool;
use uuid::Uuid;

/// Alternates user and assistant turns so a fixture can branch after any assistant message.
fn history(contents: &[&str]) -> Vec<u8> {
    serde_json::to_vec(&contents.iter().enumerate().map(|(i, content)| {
        serde_json::json!({"role":if i % 2 == 0 {"user"} else {"assistant"},"content":content})
    }).collect::<Vec<_>>()).unwrap()
}
/// Creates a conversation whose only path has the given session and title.
async fn create(
    db: &Database,
    owner: &Owner,
    session: &str,
    title: &str,
    contents: &[&str],
) -> palace_db::ImportResult {
    let request = ImportRequest::parse(
        ImportInput {
            title: title.into(),
            source: Source::Gemini,
            session_id: session.into(),
            history: history(contents),
            idempotency_key: format!("create-{session}"),
            occurred_at: 1_000,
        },
        ImportLimits::default(),
    )
    .unwrap();
    db.import_path(owner.scope(), &request).await.unwrap()
}
/// Adds a branch to an existing conversation.
async fn branch(
    db: &Database,
    owner: &Owner,
    conversation_id: Uuid,
    session: &str,
    title: &str,
    contents: &[&str],
) -> palace_db::ImportResult {
    let request = ImportRequest::parse_target(
        ImportTarget::Branch {
            conversation_id,
            session_id: SessionId::try_from(session.to_owned()).unwrap(),
            title: title.into(),
        },
        PathInput {
            history: history(contents),
            idempotency_key: format!("branch-{session}"),
            occurred_at: 1_000,
        },
        ImportLimits::default(),
    )
    .unwrap();
    db.import_path(owner.scope(), &request).await.unwrap()
}
/// Lists every page of one request, following cursors to the end.
async fn all_pages(db: &Database, owner: &Owner, search: Option<&str>) -> Vec<ConversationPage> {
    let mut request = ConversationListRequest {
        search: search.and_then(SearchTerm::parse),
        after: None,
    };
    let mut pages = Vec::new();
    loop {
        let page = db.conversation_list(owner.scope(), &request).await.unwrap();
        request.after = page.next_cursor;
        let done = page.next_cursor.is_none();
        pages.push(page);
        if done {
            return pages;
        }
    }
}
/// The ids of a single-page search, in list order.
async fn search(db: &Database, owner: &Owner, term: &str) -> Vec<Uuid> {
    let page = db
        .conversation_list(
            owner.scope(),
            &ConversationListRequest {
                search: SearchTerm::parse(term),
                after: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(page.next_cursor, None);
    page.items.into_iter().map(|item| item.id).collect()
}
/// Every path id of an owner in the order the list promises.
async fn expected_order(pool: &PgPool, owner: &Owner) -> Vec<Uuid> {
    sqlx::query_scalar(
        "SELECT id FROM conversation_path WHERE owner_id=$1 ORDER BY updated_at DESC,id DESC",
    )
    .bind(owner.id)
    .fetch_all(pool)
    .await
    .unwrap()
}

/// Keyset pages cover every path exactly once, in `(updated_at, id)` order, ties included, even
/// when a path is imported between two page requests.
/// Core test case:
/// - `specs/test-cases/web/stars/contents-page.md#conversation-list-must-page-every-path-exactly-once-in-order`
/// - `specs/test-cases/web/stars/contents-page.md#every-row-must-open-the-path-whose-title-and-date-it-shows`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn conversation_list_must_page_every_path_exactly_once_in_order() {
    let (_container, db, pool) = database().await;
    let owner = db
        .resolve_identity("https://idp", "pages", "pages@example.com")
        .await
        .unwrap();
    let mut first = None;
    for i in 0..CONVERSATION_PAGE_SIZE + 10 {
        let result = create(
            &db,
            &owner,
            &format!("s{i}"),
            &format!("title {i}"),
            &["U", "A"],
        )
        .await;
        first.get_or_insert(result);
    }
    let first = first.unwrap();
    let forked = branch(
        &db,
        &owner,
        first.conversation_id,
        "fork",
        "fork",
        &["U", "A", "U2"],
    )
    .await;
    // Ties on updated_at straddle the page boundary, so only the id can order them.
    // Copying the 46th row's time onto the next nine keeps them at positions 46..55.
    sqlx::query("WITH ordered AS (SELECT id,updated_at FROM conversation_path WHERE owner_id=$1 ORDER BY updated_at DESC,id DESC OFFSET 45 LIMIT 10) UPDATE conversation_path SET updated_at=(SELECT max(updated_at) FROM ordered) WHERE id IN (SELECT id FROM ordered)")
        .bind(owner.id)
        .execute(&pool)
        .await
        .unwrap();
    let expected = expected_order(&pool, &owner).await;
    let tied: Vec<i64> = sqlx::query_scalar("SELECT count(*) FROM conversation_path WHERE owner_id=$1 GROUP BY updated_at HAVING count(*)>1")
        .bind(owner.id)
        .fetch_all(&pool)
        .await
        .unwrap();
    // Imports landing in the same millisecond may add incidental ties; the forced group must exist.
    assert!(tied.contains(&10), "{tied:?}");

    let page = db
        .conversation_list(owner.scope(), &ConversationListRequest::default())
        .await
        .unwrap();
    assert_eq!(page.items.len() as i64, CONVERSATION_PAGE_SIZE);
    assert_eq!(page.total, expected.len() as i64);
    // A path imported between two requests sorts before the cursor, so later pages neither
    // repeat nor skip the rows that existed when browsing started.
    create(&db, &owner, "late", "late", &["U", "A"]).await;
    let mut seen: Vec<Uuid> = page.items.iter().map(|item| item.id).collect();
    let mut request = ConversationListRequest {
        search: None,
        after: page.next_cursor,
    };
    while request.after.is_some() {
        let page = db.conversation_list(owner.scope(), &request).await.unwrap();
        seen.extend(page.items.iter().map(|item| item.id));
        request.after = page.next_cursor;
    }
    assert_eq!(seen, expected);

    // A fresh listing includes the late path, and every row carries its own path's fields.
    let pages = all_pages(&db, &owner, None).await;
    assert_eq!(pages[0].total, expected.len() as i64 + 1);
    let rows: Vec<ConversationListItem> = pages.into_iter().flat_map(|page| page.items).collect();
    assert_eq!(rows.len(), expected.len() + 1);
    let fork_row = rows
        .into_iter()
        .find(|row| row.id == forked.path_id)
        .unwrap();
    let updated_at: i64 = sqlx::query_scalar(
        "SELECT (extract(epoch FROM updated_at)*1000)::bigint FROM conversation_path WHERE id=$1",
    )
    .bind(forked.path_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        fork_row,
        ConversationListItem {
            id: forked.path_id,
            conversation_id: first.conversation_id,
            title: "fork".into(),
            source: Source::Gemini,
            updated_at,
        }
    );
}

/// Neither the list, its total nor a search reaches another owner's paths.
/// Core test case:
/// - `specs/test-cases/web/stars/contents-page.md#conversation-list-must-stay-within-the-owner`
/// - `specs/test-cases/web/stars/contents-page.md#conversation-list-must-page-every-path-exactly-once-in-order`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn conversation_list_must_stay_within_the_owner() {
    let (_container, db, _pool) = database().await;
    let owner = db
        .resolve_identity("https://idp", "mine", "mine@example.com")
        .await
        .unwrap();
    let other = db
        .resolve_identity("https://idp", "theirs", "theirs@example.com")
        .await
        .unwrap();
    let mine = create(&db, &owner, "mine", "plain", &["hello", "world"]).await;
    create(
        &db,
        &other,
        "theirs",
        "secret title",
        &["secret body", "reply"],
    )
    .await;

    let pages = all_pages(&db, &owner, None).await;
    assert_eq!(pages.len(), 1);
    assert_eq!(pages[0].total, 1);
    assert_eq!(
        pages[0]
            .items
            .iter()
            .map(|item| item.id)
            .collect::<Vec<_>>(),
        vec![mine.path_id]
    );
    assert_eq!(search(&db, &owner, "secret").await, Vec::<Uuid>::new());
    let searched = &all_pages(&db, &owner, Some("secret")).await[0];
    assert_eq!(searched.total, 1);
}

/// A search matches a path's own title or a message from its root to its end, literally and
/// case-insensitively, including keywords too short for a trigram.
/// Core test case:
/// - `specs/test-cases/web/stars/contents-page.md#search-must-run-on-enter-and-match-only-the-paths-own-title-and-messages`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn search_must_match_only_the_paths_own_title_and_messages() {
    let (_container, db, pool) = database().await;
    let owner = db
        .resolve_identity("https://idp", "search", "search@example.com")
        .await
        .unwrap();
    let trunk = create(
        &db,
        &owner,
        "trunk",
        "周末出行",
        &["想去海边 Shared", "好的", "去 Mendocino"],
    )
    .await;
    let fork = branch(
        &db,
        &owner,
        trunk.conversation_id,
        "fork",
        "改去 Point Reyes",
        &["想去海边 Shared", "好的", "改去 Reyes 看象海豹"],
    )
    .await;
    let literal = create(
        &db,
        &owner,
        "literal",
        "折扣",
        &["打 50% 折", "snake_case 名字", r"路径 a\b"],
    )
    .await;
    let plain = create(
        &db,
        &owner,
        "plain",
        "别的",
        &["50 折", "snakecase", "路径 ab"],
    )
    .await;
    // Fix the order so expectations can be written as lists.
    for (id, at) in [
        (trunk.path_id, "04"),
        (fork.path_id, "03"),
        (literal.path_id, "02"),
        (plain.path_id, "01"),
    ] {
        sqlx::query("UPDATE conversation_path SET updated_at=('2026-10-'||$2||' 00:00:00+00')::timestamptz WHERE id=$1")
            .bind(id)
            .bind(at)
            .execute(&pool)
            .await
            .unwrap();
    }

    // Shared messages belong to every path through them; a branch's own tail only to that path.
    assert_eq!(
        search(&db, &owner, "shared").await,
        vec![trunk.path_id, fork.path_id]
    );
    assert_eq!(search(&db, &owner, "mendocino").await, vec![trunk.path_id]);
    assert_eq!(search(&db, &owner, "象海豹").await, vec![fork.path_id]);
    // Titles match on their own path only, even when another path shares the messages.
    assert_eq!(search(&db, &owner, "point reyes").await, vec![fork.path_id]);
    assert_eq!(search(&db, &owner, "周末").await, vec![trunk.path_id]);
    // Wildcards are literal: `50%` is not `50` followed by anything, `_` is not any character.
    assert_eq!(search(&db, &owner, "50%").await, vec![literal.path_id]);
    assert_eq!(search(&db, &owner, "e_c").await, vec![literal.path_id]);
    // `\` is ILIKE's escape character; left unescaped, `a\b` would also match `ab`.
    assert_eq!(search(&db, &owner, r"a\b").await, vec![literal.path_id]);
    // Two characters yield no trigram; the index cannot help, the result must still be right.
    assert_eq!(
        search(&db, &owner, "海边").await,
        vec![trunk.path_id, fork.path_id]
    );
    assert_eq!(search(&db, &owner, "没有的词").await, Vec::<Uuid>::new());
}
