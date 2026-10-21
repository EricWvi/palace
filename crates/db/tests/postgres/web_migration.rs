use super::database;
use super::web_pages::{article, counts, mark, owner, title, url};
use palace_db::{
    Annotation, ArticleCursor, ArticleListRequest, ArticleOrder, Database, DbError, OwnerScope,
    SearchTerm,
};
use palace_domain::{AnnotationAnchor, AnnotationContent, AnnotationNote, AnnotationTarget};
use pretty_assertions::assert_eq;
use uuid::Uuid;

fn anchor(start: i32, quote: &str) -> AnnotationAnchor {
    let end = start + i32::try_from(quote.encode_utf16().count()).unwrap();
    AnnotationAnchor::parse(start.into(), end.into(), quote.into()).unwrap()
}

/// Migrating replaces the listed web annotations with article annotations that keep their
/// style, note and creation time; a failing insert keeps every web annotation, and items not on
/// a page bound to this article are skipped.
/// Core test case:
/// - `specs/test-cases/extension/web-annotations.md#migrating-a-web-annotation-must-atomically-replace-it-with-exactly-one-article-annotation`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn migration_must_atomically_replace_web_annotations() {
    let (_container, db, pool) = database().await;
    let a = owner(&db, "a").await;
    let b = owner(&db, "b").await;
    let target = article(&db, a, "https://example.com/a", "目标").await;
    let other = article(&db, a, "https://example.com/o", "别的").await;
    let page = "https://example.com/page";
    let one = mark(&db, a, page, "第一处").await.unwrap();
    let noted = AnnotationContent::StyledNoted(
        palace_domain::AnnotationStyle::Wavy,
        AnnotationNote::parse("想法".into()).unwrap(),
    );
    let two = db
        .update_web_annotation(
            a,
            mark(&db, a, page, "第二处").await.unwrap().id,
            &noted,
            "",
        )
        .await
        .unwrap();
    let three = mark(&db, a, page, "第三处").await.unwrap();
    db.bind_web_page(a, &url(page), &title("页"), target)
        .await
        .unwrap();
    // Annotations on an unbound page and on a page bound elsewhere.
    let loose = mark(&db, a, "https://example.com/loose", "散落")
        .await
        .unwrap();
    let elsewhere = mark(&db, a, "https://example.com/else", "别处")
        .await
        .unwrap();
    db.bind_web_page(a, &url("https://example.com/else"), &title("别"), other)
        .await
        .unwrap();
    let b_article = article(&db, b, "https://example.com/b", "B").await;

    // One broken anchor rolls back the whole statement, deletions included.
    let broken = AnnotationAnchor::from_stored(9, 3, "错位".into());
    assert!(matches!(
        db.migrate_web_annotations(
            a,
            target,
            &[(one.id, anchor(0, "第一处")), (three.id, broken)]
        )
        .await,
        Err(DbError::Storage(_))
    ));
    assert_eq!(
        db.article_web_annotations(a, target).await.unwrap(),
        vec![one.clone(), two.clone(), three.clone()]
    );
    assert_eq!(db.article_annotations(a, target).await.unwrap(), vec![]);

    let created = db
        .migrate_web_annotations(
            a,
            target,
            &[
                (one.id, anchor(0, "第一处")),
                (two.id, anchor(10, "第二处")),
                (loose.id, anchor(20, "散落")),
                (elsewhere.id, anchor(30, "别处")),
                (Uuid::now_v7(), anchor(40, "不存在")),
            ],
        )
        .await
        .unwrap();
    let expected = vec![
        Annotation {
            id: created[0].id,
            target: AnnotationTarget::Article(target),
            anchor: anchor(0, "第一处"),
            content: one.content.clone(),
            created_at: one.created_at,
            updated_at: created[0].updated_at,
        },
        Annotation {
            id: created[1].id,
            target: AnnotationTarget::Article(target),
            anchor: anchor(10, "第二处"),
            content: noted,
            created_at: two.created_at,
            updated_at: created[1].updated_at,
        },
    ];
    assert_eq!(created, expected);
    assert_eq!(db.article_annotations(a, target).await.unwrap(), expected);
    assert_eq!(
        db.article_web_annotations(a, target).await.unwrap(),
        vec![three.clone()]
    );
    assert_eq!(
        db.article_web_annotations(a, other).await.unwrap(),
        vec![elsewhere.clone()]
    );
    assert_eq!(
        db.web_page_by_url(a, &url("https://example.com/loose"))
            .await
            .unwrap()
            .annotations,
        vec![loose]
    );

    // Another owner's article, or A's annotations named from B, move nothing.
    assert!(matches!(
        db.migrate_web_annotations(b, target, &[(three.id, anchor(0, "第三处"))])
            .await,
        Err(DbError::NotFound)
    ));
    assert_eq!(
        db.migrate_web_annotations(b, b_article, &[(three.id, anchor(0, "第三处"))])
            .await
            .unwrap(),
        vec![]
    );
    assert_eq!(counts(&pool).await, (3, 3));
}

/// Two article pages migrating the same annotations at once end with one article annotation
/// each: both statements wait at the delete until a third transaction releases the rows.
/// Core test case:
/// - `specs/test-cases/extension/web-annotations.md#migrating-a-web-annotation-must-atomically-replace-it-with-exactly-one-article-annotation`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn concurrent_migrations_create_one_annotation_each() {
    let (_container, db, pool) = database().await;
    let owner = owner(&db, "a").await;
    let target = article(&db, owner, "https://example.com/a", "目标").await;
    let page = "https://example.com/page";
    let one = mark(&db, owner, page, "第一处").await.unwrap();
    let two = mark(&db, owner, page, "第二处").await.unwrap();
    db.bind_web_page(owner, &url(page), &title("页"), target)
        .await
        .unwrap();
    let items = vec![
        (one.id, anchor(0, "第一处")),
        (two.id, anchor(10, "第二处")),
    ];

    let mut blocker = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM web_annotation WHERE id=ANY($1) FOR UPDATE")
        .bind(vec![one.id, two.id])
        .execute(&mut *blocker)
        .await
        .unwrap();
    let migrate = |db: Database, owner: OwnerScope, items: Vec<(Uuid, AnnotationAnchor)>| {
        tokio::spawn(async move { db.migrate_web_annotations(owner, target, &items).await })
    };
    let first = migrate(db.clone(), owner, items.clone());
    let second = migrate(db.clone(), owner, items);
    // Release the rows only once both deletes are queued on them.
    loop {
        let waiting: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM pg_stat_activity WHERE wait_event_type='Lock' AND query LIKE 'WITH item%'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        if waiting == 2 {
            break;
        }
        tokio::task::yield_now().await;
    }
    blocker.commit().await.unwrap();
    let first = first.await.unwrap().unwrap();
    let second = second.await.unwrap().unwrap();
    let mut lengths = [first.len(), second.len()];
    lengths.sort_unstable();
    assert_eq!(lengths, [0, 2]);
    assert_eq!(
        db.article_annotations(owner, target).await.unwrap(),
        [first, second].concat()
    );
    assert_eq!(counts(&pool).await, (1, 0));
}

/// Reads every page of the near-ordered list.
async fn near_pages(
    db: &Database,
    owner: OwnerScope,
    near: &str,
    search: Option<&str>,
) -> Vec<palace_db::ArticlePage> {
    let mut pages = Vec::new();
    let mut after = None;
    loop {
        let page = db
            .article_list(
                owner,
                &ArticleListRequest {
                    search: search.and_then(SearchTerm::parse),
                    order: ArticleOrder::Near {
                        url: near.into(),
                        after,
                    },
                },
            )
            .await
            .unwrap();
        after = page.next_cursor.map(|cursor| match cursor {
            ArticleCursor::Near(cursor) => cursor,
            ArticleCursor::Published(cursor) => panic!("near list returned {cursor}"),
        });
        pages.push(page);
        if after.is_none() {
            return pages;
        }
    }
}

fn titles(pages: &[palace_db::ArticlePage]) -> Vec<String> {
    pages
        .iter()
        .flat_map(|page| {
            page.items
                .iter()
                .map(|item| item.metadata.title().to_owned())
        })
        .collect()
}

/// Same host first, then the longer shared path, then the latest import, across pages and
/// under a search that only filters.
/// Core test case:
/// - `specs/test-cases/extension/web-annotations.md#candidate-articles-must-rank-the-nearest-address-first`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn candidate_articles_must_rank_the_nearest_address_first() {
    let (_container, db, _pool) = database().await;
    let owner = owner(&db, "a").await;
    // Imported oldest first: fifty fillers elsewhere, then the near ones in scrambled order.
    for n in 0..50 {
        article(
            &db,
            owner,
            &format!("https://other.com/{n}"),
            &format!("填充 {n}"),
        )
        .await;
    }
    for (path, name) in [
        ("https://www.example.com/blog/2026/post", "同一篇"),
        ("https://EXAMPLE.com/news/x", "同站别栏"),
        ("https://example.com/blog/2025/old", "同栏旧年"),
        ("https://example.com/blog/2026/next", "同栏同年"),
        ("https://sub.example.com/blog/2026/post", "子域名"),
    ] {
        article(&db, owner, path, name).await;
    }
    let near = "https://example.com/blog/2026/post/?utm_source=feed";
    let pages = near_pages(&db, owner, near, None).await;
    assert_eq!(pages.len(), 2);
    assert!(pages.iter().all(|page| page.total == 55));
    let mut expected: Vec<String> = ["同一篇", "同栏同年", "同栏旧年", "同站别栏", "子域名"]
        .map(str::to_owned)
        .into();
    expected.extend((0..50).rev().map(|n| format!("填充 {n}")));
    assert_eq!(titles(&pages), expected);

    // The search filters bodies too and leaves the order and the total alone.
    let searched = near_pages(&db, owner, near, Some("同栏")).await;
    assert_eq!(titles(&searched), ["同栏同年", "同栏旧年"]);
    assert_eq!(searched[0].total, 55);
}
