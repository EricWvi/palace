use super::database;
use palace_db::{
    ArticleImport, Database, DbError, OwnerScope, WebAnnotation, WebPage, WebPageDetail,
    WebPageSummary,
};
use palace_domain::{
    AnnotationContent, AnnotationNote, AnnotationStyle, ArticleMetadata, ArticleMetadataInput,
    WebAnchor, WebPageState, WebPageTitle, WebPageUrl,
};
use pretty_assertions::assert_eq;
use sqlx::PgPool;
use uuid::Uuid;

pub(super) async fn owner(db: &Database, subject: &str) -> OwnerScope {
    db.resolve_identity("https://idp", subject, &format!("{subject}@example.com"))
        .await
        .unwrap()
        .scope()
}

/// Saves an article at `url` with this title and body and returns its id.
pub(super) async fn article(db: &Database, owner: OwnerScope, url: &str, title: &str) -> Uuid {
    let metadata = ArticleMetadata::parse(
        ArticleMetadataInput {
            title: title.into(),
            url: url.into(),
            author: None,
            date: "2026-10-05".into(),
        },
        chrono_tz::Asia::Shanghai,
    )
    .unwrap();
    match db
        .insert_article(owner, &metadata, &format!("{title} 正文"), &[])
        .await
        .unwrap()
    {
        ArticleImport::Created(article) => article.id,
        ArticleImport::Existing(article) => panic!("{} already existed", article.id),
    }
}

pub(super) fn url(text: &str) -> WebPageUrl {
    WebPageUrl::parse(text.into()).unwrap()
}

pub(super) fn title(text: &str) -> WebPageTitle {
    WebPageTitle::new(text, &url("https://example.com/"))
}

pub(super) fn anchor(quote: &str) -> WebAnchor {
    WebAnchor::parse(quote.into(), "前文".into(), "后文".into(), 0.25).unwrap()
}

pub(super) fn styled(style: &str) -> AnnotationContent {
    AnnotationContent::parse(AnnotationStyle::parse(style), /*note*/ None).unwrap()
}

/// Annotates the page at `page` with a yellow mark on `quote`.
pub(super) async fn mark(
    db: &Database,
    owner: OwnerScope,
    page: &str,
    quote: &str,
) -> Result<WebAnnotation, DbError> {
    db.insert_web_annotation(
        owner,
        &url(page),
        &title("标签页"),
        &anchor(quote),
        &styled("yellow"),
    )
    .await
}

/// Counts pages and web annotations, whoever owns them.
pub(super) async fn counts(pool: &PgPool) -> (i64, i64) {
    sqlx::query_as("SELECT (SELECT count(*) FROM web_page),(SELECT count(*) FROM web_annotation)")
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Another owner can neither find, change nor bind A's pages, nor bind a page to A's article;
/// the same address is a separate page per owner.
/// Core test case:
/// - `specs/test-cases/extension/web-annotations.md#web-pages-and-web-annotations-must-stay-within-the-owner-scope`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn web_pages_must_stay_within_the_owner_scope() {
    let (_container, db, pool) = database().await;
    let a = owner(&db, "a").await;
    let b = owner(&db, "b").await;
    let a_article = article(&db, a, "https://example.com/a", "A").await;
    let b_article = article(&db, b, "https://example.com/b", "B").await;
    let page = "https://example.com/page";
    let created = mark(&db, a, page, "引文").await.unwrap();
    assert_eq!(
        created,
        WebAnnotation {
            id: created.id,
            page_id: created.page_id,
            anchor: anchor("引文"),
            content: styled("yellow"),
            created_at: created.created_at,
            updated_at: created.created_at,
        }
    );

    assert!(matches!(
        db.web_page_by_url(b, &url(page)).await,
        Err(DbError::NotFound)
    ));
    assert_eq!(db.web_pages(b).await.unwrap(), vec![]);
    assert!(matches!(
        db.update_web_annotation(b, created.id, &styled("rose"), "B")
            .await,
        Err(DbError::NotFound)
    ));
    assert!(matches!(
        db.delete_web_annotation(b, created.id).await,
        Err(DbError::NotFound)
    ));
    assert!(matches!(
        db.bind_web_page(b, &url(page), &title("B"), a_article)
            .await,
        Err(DbError::NotFound)
    ));
    assert!(matches!(
        db.article_web_annotations(b, a_article).await,
        Err(DbError::NotFound)
    ));
    assert_eq!(counts(&pool).await, (1, 1));

    // B annotating the same address gets a page of its own.
    let b_mark = mark(&db, b, page, "另一段").await.unwrap();
    assert_ne!(b_mark.page_id, created.page_id);
    assert_eq!(
        db.web_page_by_url(a, &url(page)).await.unwrap().annotations,
        vec![created.clone()]
    );
    // B binding its page to its own article leaves A's page unbound.
    db.bind_web_page(b, &url(page), &title("B"), b_article)
        .await
        .unwrap();
    assert_eq!(
        db.web_page_by_url(a, &url(page)).await.unwrap().page.state,
        WebPageState::Unbound
    );
    assert_eq!(
        db.article_web_annotations(b, b_article).await.unwrap(),
        vec![b_mark]
    );
}

/// An unbound page lives exactly as long as its annotations; a bound one refuses new and changed
/// annotations but lets the reader discard what is left, and binding moves between articles.
/// Core test case:
/// - `specs/test-cases/extension/web-annotations.md#a-bound-web-page-must-reject-new-and-changed-web-annotations`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn a_bound_web_page_must_reject_new_and_changed_web_annotations() {
    let (_container, db, pool) = database().await;
    let owner = owner(&db, "a").await;
    let first_article = article(&db, owner, "https://example.com/1", "一").await;
    let second_article = article(&db, owner, "https://example.com/2", "二").await;
    let page = "https://example.com/page";

    let one = mark(&db, owner, page, "第一处").await.unwrap();
    let two = mark(&db, owner, page, "第二处").await.unwrap();
    assert_eq!(one.page_id, two.page_id);
    db.delete_web_annotation(owner, one.id).await.unwrap();
    assert_eq!(counts(&pool).await, (1, 1));
    db.delete_web_annotation(owner, two.id).await.unwrap();
    assert_eq!(counts(&pool).await, (0, 0));
    assert!(matches!(
        db.web_page_by_url(owner, &url(page)).await,
        Err(DbError::NotFound)
    ));

    // Editing records the tab title; a blank one falls back to the address.
    let kept = mark(&db, owner, page, "留下").await.unwrap();
    let edited = db
        .update_web_annotation(
            owner,
            kept.id,
            &AnnotationContent::Noted(AnnotationNote::parse("想法".into()).unwrap()),
            " ",
        )
        .await
        .unwrap();
    assert_eq!(
        edited,
        WebAnnotation {
            content: AnnotationContent::Noted(AnnotationNote::parse("想法".into()).unwrap()),
            updated_at: edited.updated_at,
            ..kept.clone()
        }
    );
    assert_eq!(
        db.web_page_by_url(owner, &url(page))
            .await
            .unwrap()
            .page
            .title,
        page
    );

    let bound = db
        .bind_web_page(owner, &url(page), &title("绑定时的标题"), first_article)
        .await
        .unwrap();
    assert_eq!(
        bound,
        WebPage {
            id: kept.page_id,
            url: page.into(),
            title: "绑定时的标题".into(),
            state: WebPageState::Bound(first_article),
            updated_at: bound.updated_at,
        }
    );
    assert!(matches!(
        mark(&db, owner, page, "绑定后").await,
        Err(DbError::WebPageBound)
    ));
    assert!(matches!(
        db.update_web_annotation(owner, kept.id, &styled("rose"), "标题")
            .await,
        Err(DbError::WebPageBound)
    ));
    // Refused writes leave the title as the binding set it.
    assert_eq!(
        db.web_pages(owner).await.unwrap(),
        vec![WebPageSummary {
            page: bound.clone(),
            annotation_count: 1,
        }]
    );

    // Moving the binding changes only the article.
    let moved = db
        .bind_web_page(owner, &url(page), &title("绑定时的标题"), second_article)
        .await
        .unwrap();
    assert_eq!(
        moved,
        WebPage {
            state: WebPageState::Bound(second_article),
            updated_at: moved.updated_at,
            ..bound.clone()
        }
    );
    assert_eq!(
        db.article_web_annotations(owner, first_article)
            .await
            .unwrap(),
        vec![]
    );
    assert_eq!(
        db.article_web_annotations(owner, second_article)
            .await
            .unwrap(),
        vec![edited]
    );

    // The last annotation of a bound page can be discarded; the page stays bound.
    db.delete_web_annotation(owner, kept.id).await.unwrap();
    assert_eq!(
        db.web_page_by_url(owner, &url(page)).await.unwrap(),
        WebPageDetail {
            page: WebPage {
                updated_at: db.web_pages(owner).await.unwrap()[0].page.updated_at,
                ..moved
            },
            annotations: vec![],
        }
    );

    // A page never annotated is created bound.
    let fresh = "https://example.com/fresh";
    let created = db
        .bind_web_page(owner, &url(fresh), &title("新页"), first_article)
        .await
        .unwrap();
    assert_eq!(
        created,
        WebPage {
            id: created.id,
            url: fresh.into(),
            title: "新页".into(),
            state: WebPageState::Bound(first_article),
            updated_at: created.updated_at,
        }
    );
    assert!(matches!(
        db.bind_web_page(owner, &url(fresh), &title("x"), Uuid::now_v7())
            .await,
        Err(DbError::NotFound)
    ));
}

/// The table refuses an unbound page with no annotation and any change of a web anchor.
/// Core test case:
/// - `specs/test-cases/extension/web-annotations.md#a-bound-web-page-must-reject-new-and-changed-web-annotations`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn web_page_rows_must_keep_annotations_and_anchors() {
    let (_container, db, pool) = database().await;
    let owner = owner(&db, "a").await;
    let kept = mark(&db, owner, "https://example.com/page", "引文")
        .await
        .unwrap();
    let empty = sqlx::query("INSERT INTO web_page(id,owner_id,url,title) VALUES($1,$2,'https://example.com/empty','空')")
        .bind(Uuid::now_v7())
        .bind(owner.id())
        .execute(&pool)
        .await;
    assert!(empty.is_err());
    let orphaned = sqlx::query("DELETE FROM web_annotation WHERE id=$1")
        .bind(kept.id)
        .execute(&pool)
        .await;
    assert!(orphaned.is_err());
    let moved = sqlx::query("UPDATE web_annotation SET quote='别处' WHERE id=$1")
        .bind(kept.id)
        .execute(&pool)
        .await;
    assert!(moved.is_err());
    assert_eq!(counts(&pool).await, (1, 1));
}

/// Deleting an article takes the pages bound to it and what is left on them, in its own
/// transaction; a failed delete keeps all of it, and unbound pages are never touched.
/// Core test case:
/// - `specs/test-cases/extension/web-annotations.md#deleting-an-article-must-take-its-bound-web-pages-with-it`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn deleting_an_article_must_take_its_bound_web_pages_with_it() {
    let (_container, db, pool) = database().await;
    let owner = owner(&db, "a").await;
    let id = article(&db, owner, "https://example.com/a", "文章").await;
    mark(&db, owner, "https://example.com/left", "剩下")
        .await
        .unwrap();
    db.bind_web_page(owner, &url("https://example.com/left"), &title("左"), id)
        .await
        .unwrap();
    db.bind_web_page(owner, &url("https://example.com/bare"), &title("空"), id)
        .await
        .unwrap();
    let unbound = mark(&db, owner, "https://example.com/other", "别的")
        .await
        .unwrap();
    assert_eq!(counts(&pool).await, (3, 2));

    // A check that fails at commit makes the article delete roll back.
    sqlx::raw_sql("CREATE FUNCTION fail_article_delete() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected'; END $$; CREATE CONSTRAINT TRIGGER fail_article_delete AFTER DELETE ON article DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION fail_article_delete();")
        .execute(&pool)
        .await
        .unwrap();
    assert!(matches!(
        db.delete_article(owner, id).await,
        Err(DbError::Storage(_))
    ));
    assert_eq!(counts(&pool).await, (3, 2));
    assert_eq!(
        db.article_web_annotations(owner, id).await.unwrap().len(),
        1
    );

    sqlx::raw_sql("DROP TRIGGER fail_article_delete ON article")
        .execute(&pool)
        .await
        .unwrap();
    db.delete_article(owner, id).await.unwrap();
    assert_eq!(counts(&pool).await, (1, 1));
    assert_eq!(
        db.web_page_by_url(owner, &url("https://example.com/other"))
            .await
            .unwrap()
            .annotations,
        vec![unbound]
    );
}
