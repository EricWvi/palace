use super::database;
use palace_db::{
    Article, ArticleCard, ArticleImport, ArticleListRequest, ArticleMedia, ArticlePage, Database,
    DbError, ListCursor, MomentDetail, OwnerScope, SearchTerm, StoredMedia,
};
use palace_domain::{ArticleMetadata, ArticleMetadataInput, MediaLimits, MediaUpload};
use pretty_assertions::assert_eq;
use sqlx::PgPool;
use uuid::Uuid;

const SHANGHAI: chrono_tz::Tz = chrono_tz::Asia::Shanghai;

fn metadata(title: &str, url: &str, author: Option<&str>, date: &str) -> ArticleMetadata {
    ArticleMetadata::parse(
        ArticleMetadataInput {
            title: title.into(),
            url: url.into(),
            author: author.map(str::to_owned),
            date: date.into(),
        },
        SHANGHAI,
    )
    .unwrap()
}

/// Saves an article without media and returns it; the URL must be new to the owner.
async fn import(db: &Database, owner: OwnerScope, metadata: &ArticleMetadata) -> Article {
    match db
        .insert_article(owner, metadata, "正文", &[])
        .await
        .unwrap()
    {
        ArticleImport::Created(article) => article,
        ArticleImport::Existing(article) => panic!("{} already existed", article.id),
    }
}

/// Reads every page of a list, from the start.
async fn all_pages(db: &Database, owner: OwnerScope, search: Option<&str>) -> Vec<ArticlePage> {
    let mut pages = Vec::new();
    let mut after = None;
    loop {
        let page = db
            .article_list(
                owner,
                &ArticleListRequest {
                    search: search.and_then(SearchTerm::parse),
                    after,
                },
            )
            .await
            .unwrap();
        after = page.next_cursor;
        pages.push(page);
        if after.is_none() {
            return pages;
        }
    }
}

fn ids(pages: &[ArticlePage]) -> Vec<Uuid> {
    pages
        .iter()
        .flat_map(|page| page.items.iter().map(|item| item.id))
        .collect()
}

/// The time of the article's moment, straight from the table.
async fn occurred_at(pool: &PgPool, id: Uuid) -> i64 {
    sqlx::query_scalar(
        "SELECT (extract(epoch FROM occurred_at)*1000)::bigint FROM moment WHERE id=$1",
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// Another owner reads, edits and deletes nothing, and the URL is unique only within an owner.
/// Core test cases:
/// - `specs/test-cases/web/stars/articles.md#articles-must-stay-within-the-owner`
/// - `specs/test-cases/web/stars/articles.md#importing-the-same-url-must-converge-on-one-article`
/// - `specs/test-cases/web/stars/articles.md#editing-an-article-must-replace-only-its-metadata`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn articles_must_stay_within_the_owner() {
    let (_container, db, pool) = database().await;
    let a = db
        .resolve_identity("https://idp", "a", "a@example.com")
        .await
        .unwrap()
        .scope();
    let b = db
        .resolve_identity("https://idp", "b", "b@example.com")
        .await
        .unwrap()
        .scope();
    let shared = metadata("文章", "https://example.com/a", Some("作者"), "2026-10-05");
    let article = import(&db, a, &shared).await;
    assert!(matches!(
        db.article(b, article.id).await,
        Err(DbError::NotFound)
    ));
    assert!(matches!(
        db.update_article(b, article.id, &shared).await,
        Err(DbError::NotFound)
    ));
    assert!(matches!(
        db.delete_article(b, article.id).await,
        Err(DbError::NotFound)
    ));
    assert!(matches!(
        db.article_object_keys(b, article.id).await,
        Err(DbError::NotFound)
    ));
    assert_eq!(all_pages(&db, b, None).await[0].total, 0);
    assert_eq!(
        db.article_by_url(b, "https://example.com/a").await.unwrap(),
        None
    );
    // The same URL is a different article for another owner.
    let theirs = import(&db, b, &shared).await;
    assert_ne!(theirs.id, article.id);
    assert_eq!(db.article(a, article.id).await.unwrap(), article.clone());
    for statement in [
        "UPDATE article SET owner_id=$2 WHERE id=$1",
        "UPDATE article SET content='改写' WHERE id=$1 AND $2::uuid IS NOT NULL",
    ] {
        assert!(
            sqlx::query(statement)
                .bind(article.id)
                .bind(b.id())
                .execute(&pool)
                .await
                .is_err(),
            "{statement}"
        );
    }
    assert_eq!(db.article(a, article.id).await.unwrap(), article);
}

/// A moment of kind article needs its article, and an article cannot hang off another kind.
/// Core test case:
/// - `specs/test-cases/web/stars/articles.md#article-import-must-commit-the-article-and-its-media-together-or-not-at-all`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn article_moments_and_details_lock_their_kind() {
    let (_container, db, pool) = database().await;
    let owner = db
        .resolve_identity("https://idp", "a", "a@example.com")
        .await
        .unwrap();
    let id = Uuid::now_v7();
    // A moment without its article fails when the transaction commits.
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO moment(id,owner_id,kind,occurred_at) VALUES($1,$2,'article',date_trunc('milliseconds',now()))")
        .bind(id)
        .bind(owner.id)
        .execute(&mut *tx)
        .await
        .unwrap();
    assert!(tx.commit().await.is_err());
    // An article cannot point at a conversation moment.
    let mut tx = pool.begin().await.unwrap();
    sqlx::query(
        "INSERT INTO moment(id,owner_id,kind,occurred_at) VALUES($1,$2,'conversation',date_trunc('milliseconds',now()))",
    )
    .bind(id)
    .bind(owner.id)
    .execute(&mut *tx)
    .await
    .unwrap();
    assert!(
        sqlx::query("INSERT INTO article(id,owner_id,title,content,url,published_at,published_precision) VALUES($1,$2,'t','c','https://example.com',date_trunc('milliseconds',now()),'day')")
            .bind(id)
            .bind(owner.id)
            .execute(&mut *tx)
            .await
            .is_err()
    );
    drop(tx);
    // Deleting the article alone leaves a moment with no detail, which fails at commit.
    let article = import(
        &db,
        owner.scope(),
        &metadata("t", "https://example.com/x", None, "2026-10-05"),
    )
    .await;
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("DELETE FROM article WHERE id=$1")
        .bind(article.id)
        .execute(&mut *tx)
        .await
        .unwrap();
    assert!(tx.commit().await.is_err());
    assert_eq!(
        db.article(owner.scope(), article.id).await.unwrap(),
        article
    );
}

/// A known URL hands back the first article untouched, whatever the second import carries.
/// Core test case:
/// - `specs/test-cases/web/stars/articles.md#importing-the-same-url-must-converge-on-one-article`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn importing_a_known_url_returns_the_existing_article() {
    let (_container, db, pool) = database().await;
    let owner = db
        .resolve_identity("https://idp", "a", "a@example.com")
        .await
        .unwrap()
        .scope();
    let first = import(
        &db,
        owner,
        &metadata("原题", "https://example.com/a", None, "2026-10-05"),
    )
    .await;
    let again = db
        .insert_article(
            owner,
            &metadata("新题", "https://example.com/a", Some("某人"), "2020-01-01"),
            "别的正文",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(again, ArticleImport::Existing(first));
    let counts: (i64, i64) =
        sqlx::query_as("SELECT (SELECT count(*) FROM article),(SELECT count(*) FROM moment)")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(counts, (1, 1));
}

/// Edits replace the four fields together, keep the body and the moment, refuse a URL another
/// article holds, and give a bare day back as a day.
/// Core test case:
/// - `specs/test-cases/web/stars/articles.md#editing-an-article-must-replace-only-its-metadata`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn editing_an_article_must_replace_only_its_metadata() {
    let (_container, db, pool) = database().await;
    let owner = db
        .resolve_identity("https://idp", "a", "a@example.com")
        .await
        .unwrap()
        .scope();
    let article = import(
        &db,
        owner,
        &metadata("原题", "https://example.com/a", Some("某人"), "2026-10-05"),
    )
    .await;
    let other = import(
        &db,
        owner,
        &metadata("另一篇", "https://example.com/b", None, "2026-10-04"),
    )
    .await;
    let moment = occurred_at(&pool, article.id).await;
    let edited = metadata(
        "新题",
        "https://example.com/c",
        None,
        "2025-01-02T03:04:05+08:00",
    );
    let updated = db.update_article(owner, article.id, &edited).await.unwrap();
    assert_eq!(
        updated,
        Article {
            metadata: edited,
            ..article.clone()
        }
    );
    assert_eq!(occurred_at(&pool, article.id).await, moment);
    // A bare day stays a day through the round trip.
    let day = metadata("新题", "https://example.com/c", None, "2025-01-02");
    let updated = db.update_article(owner, article.id, &day).await.unwrap();
    assert_eq!(updated.metadata.published().to_text(SHANGHAI), "2025-01-02");
    assert!(matches!(
        db.update_article(
            owner,
            article.id,
            &metadata("新题", "https://example.com/b", None, "2025-01-02"),
        )
        .await,
        Err(DbError::Conflict)
    ));
    assert_eq!(db.article(owner, article.id).await.unwrap(), updated);
    assert_eq!(db.article(owner, other.id).await.unwrap(), other);
}

/// Pages walk publish dates newest first however the articles were imported; search reads the
/// title, the author and the body literally; the timeline places each article on its import day
/// and edits never move it there.
/// Core test case:
/// - `specs/test-cases/web/stars/articles.md#article-list-must-order-by-published-date-while-the-timeline-uses-import-time`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn article_list_orders_by_publish_date_while_the_timeline_uses_import_time() {
    let (_container, db, pool) = database().await;
    let owner = db
        .resolve_identity("https://idp", "a", "a@example.com")
        .await
        .unwrap()
        .scope();
    // Imported oldest first, so import order is the reverse of the list; every fifth day is
    // shared by two articles whose order then comes from their ids.
    let mut imported = Vec::new();
    for index in 0..53 {
        let day = 1 + index / 2;
        let author = (index % 3 == 0).then_some("作者");
        let date = format!("2026-{:02}-{:02}", 1 + day / 28, 1 + day % 28);
        imported.push(
            import(
                &db,
                owner,
                &metadata(
                    &format!("文章 {index}"),
                    &format!("https://www.example.com/{index}"),
                    author,
                    &date,
                ),
            )
            .await,
        );
    }
    let mut expected = imported.clone();
    expected
        .sort_by_key(|article| std::cmp::Reverse((article.metadata.published().at, article.id)));
    let pages = all_pages(&db, owner, None).await;
    assert_eq!(pages.len(), 2);
    assert_eq!(
        ids(&pages),
        expected
            .iter()
            .map(|article| article.id)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        pages[0].next_cursor,
        Some(ListCursor {
            at: expected[49].metadata.published().at,
            id: expected[49].id,
        })
    );
    assert!(pages.iter().all(|page| page.total == 53));

    // Search is literal over title, author and body, and never changes the total.
    sqlx::query("UPDATE article SET title='100% 真实' WHERE id=$1")
        .bind(imported[7].id)
        .execute(&pool)
        .await
        .unwrap();
    let only_author = import(
        &db,
        owner,
        &metadata(
            "无关",
            "https://example.com/author",
            Some("独一的笔名"),
            "2026-06-01",
        ),
    )
    .await;
    let in_body = match db
        .insert_article(
            owner,
            &metadata("无关", "https://example.com/body", None, "2026-06-02"),
            "正文里有独一的笔名",
            &[],
        )
        .await
        .unwrap()
    {
        ArticleImport::Created(article) => article,
        ArticleImport::Existing(_) => unreachable!(),
    };
    let found = all_pages(&db, owner, Some(" 独一的笔名 ")).await;
    assert_eq!(ids(&found), vec![in_body.id, only_author.id]);
    assert_eq!(found[0].total, 55);
    assert_eq!(
        ids(&all_pages(&db, owner, Some("100%")).await),
        vec![imported[7].id]
    );
    assert_eq!(
        ids(&all_pages(&db, owner, Some("1_0")).await),
        Vec::<Uuid>::new()
    );

    // Every article lands on the timeline at its import time, named by author or site.
    let timeline = db
        .timeline(owner, imported[0].occurred_at, in_body.occurred_at + 1)
        .await
        .unwrap();
    assert_eq!(timeline.len(), 55);
    let first = timeline
        .iter()
        .find(|moment| moment.id == imported[1].id)
        .unwrap();
    assert_eq!(
        first.detail,
        MomentDetail::Article(ArticleCard {
            title: "文章 1".into(),
            author: None,
            site: "example.com".into(),
        })
    );
    assert_eq!(first.occurred_at, imported[1].occurred_at);
    let moment = occurred_at(&pool, imported[0].id).await;
    db.update_article(
        owner,
        imported[0].id,
        &metadata("文章 0", "https://www.example.com/0", None, "2030-01-01"),
    )
    .await
    .unwrap();
    assert_eq!(ids(&all_pages(&db, owner, None).await)[0], imported[0].id);
    assert_eq!(occurred_at(&pool, imported[0].id).await, moment);
}

/// Deleting removes the article, its moment and its media rows together.
/// Core test case:
/// - `specs/test-cases/web/stars/articles.md#deleting-an-article-must-remove-its-media-objects-before-reporting-success`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn deleting_an_article_removes_its_rows_together() {
    let (_container, db, pool) = database().await;
    let owner = db
        .resolve_identity("https://idp", "a", "a@example.com")
        .await
        .unwrap()
        .scope();
    let (_, image) = MediaUpload::new(
        b"png".to_vec(),
        Some("image/png"),
        Some("a.png"),
        MediaLimits::default(),
    )
    .unwrap()
    .into_parts();
    let media = Uuid::now_v7();
    let key = format!("{}/{media}", owner.id());
    let ArticleImport::Created(article) = db
        .insert_article(
            owner,
            &metadata("t", "https://example.com/a", None, "2026-10-05"),
            &format!("![a](/api/media/{media})"),
            &[StoredMedia {
                id: media,
                object_key: &key,
                metadata: &image,
            }],
        )
        .await
        .unwrap()
    else {
        panic!("the URL was new");
    };
    assert_eq!(
        article.media,
        vec![ArticleMedia {
            id: media,
            content_type: "image/png".into(),
        }]
    );
    assert_eq!(
        db.article_object_keys(owner, article.id).await.unwrap(),
        vec![key]
    );
    // Media an article embeds cannot be removed on its own.
    assert_eq!(
        db.media_for_delete(owner, media).await.unwrap(),
        palace_db::MediaForDelete::Embedded
    );
    db.delete_article(owner, article.id).await.unwrap();
    let counts: (i64, i64, i64, i64) = sqlx::query_as("SELECT (SELECT count(*) FROM article),(SELECT count(*) FROM article_media),(SELECT count(*) FROM media),(SELECT count(*) FROM moment)")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(counts, (0, 0, 0, 0));
    assert!(matches!(
        db.delete_article(owner, article.id).await,
        Err(DbError::NotFound)
    ));
}
