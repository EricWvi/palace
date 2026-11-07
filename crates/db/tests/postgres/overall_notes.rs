use super::{
    database,
    paths::{branch, create},
    web_pages::{article, mark, owner, title, url},
};
use palace_db::{Database, DbError, OverallNote, OverallNoteTarget, OwnerScope};
use palace_domain::{OverallNoteText, WebPageState};
use pretty_assertions::assert_eq;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::Barrier;
use uuid::Uuid;

fn text(words: &str) -> OverallNoteText {
    OverallNoteText::parse(words.into()).unwrap().unwrap()
}

/// Saves `words` on a document with no note yet and returns the new version.
async fn first_save(
    db: &Database,
    owner: OwnerScope,
    target: OverallNoteTarget,
    words: &str,
) -> i64 {
    db.save_overall_note(owner, target, /*expected*/ 0, Some(&text(words)))
        .await
        .unwrap()
        .version
}

/// Saves `words` on a web page with no note yet.
async fn page_note(db: &Database, owner: OwnerScope, page: &str, words: &str) -> OverallNote {
    db.save_web_page_overall_note(
        owner,
        &url(page),
        &title("标签页"),
        /*expected*/ 0,
        Some(&text(words)),
    )
    .await
    .unwrap()
}

async fn rows(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM overall_note")
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Two paths keep their own notes, no owner reaches another's, and a note goes with its document.
/// Core test case:
/// - `specs/test-cases/web/stars/overall-notes.md#an-overall-note-must-belong-to-exactly-one-document-and-go-with-it`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn overall_notes_belong_to_one_document_and_go_with_it() {
    let (_container, db, pool) = database().await;
    let a = owner(&db, "a").await;
    let b = owner(&db, "b").await;
    let first = db
        .import_path(a, &create("s1", &["U1", "A1", "U2", "A2"]))
        .await
        .unwrap();
    let conversation = first.conversation_id;
    let second = db
        .import_path(a, &branch(conversation, "s2", &["U1", "A1", "U9", "A9"]))
        .await
        .unwrap();
    let path = |id: Uuid| OverallNoteTarget::Path {
        conversation,
        path: id,
    };
    let (one, two) = (path(first.path_id), path(second.path_id));

    assert_eq!(
        db.overall_note(a, one).await.unwrap(),
        OverallNote {
            text: None,
            version: 0
        }
    );
    let version = first_save(&db, a, one, "第一支").await;
    first_save(&db, a, two, "第二支").await;
    assert_eq!(
        db.overall_note(a, one).await.unwrap(),
        OverallNote {
            text: Some("第一支".into()),
            version
        }
    );
    assert_eq!(
        db.overall_note(a, two).await.unwrap().text.as_deref(),
        Some("第二支")
    );

    assert!(matches!(
        db.overall_note(b, one).await,
        Err(DbError::NotFound)
    ));
    assert!(matches!(
        db.save_overall_note(b, one, version, None).await,
        Err(DbError::NotFound)
    ));
    // A path is named with its conversation; another conversation's id does not reach it.
    assert!(matches!(
        db.overall_note(
            a,
            OverallNoteTarget::Path {
                conversation: Uuid::now_v7(),
                path: first.path_id
            }
        )
        .await,
        Err(DbError::NotFound)
    ));

    // Blank text is no note, and no row is left behind.
    db.save_overall_note(a, one, version, None).await.unwrap();
    assert_eq!(rows(&pool).await, 1);
    first_save(&db, a, one, "再写").await;

    db.delete_path(a, conversation, second.path_id)
        .await
        .unwrap();
    assert_eq!(rows(&pool).await, 1);
    db.delete_conversation(a, conversation).await.unwrap();
    assert_eq!(rows(&pool).await, 0);

    let id = article(&db, a, "https://example.com/a", "文章").await;
    first_save(&db, a, OverallNoteTarget::Article(id), "文章的").await;
    db.delete_article(a, id).await.unwrap();
    assert_eq!(rows(&pool).await, 0);
}

/// A save names what it replaces, and a version once used is never used again.
/// Core test case:
/// - `specs/test-cases/web/stars/overall-notes.md#a-stale-write-must-never-replace-an-overall-note`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn a_stale_version_never_replaces_an_overall_note() {
    let (_container, db, _pool) = database().await;
    let a = owner(&db, "a").await;
    let target = OverallNoteTarget::Article(article(&db, a, "https://example.com/a", "文章").await);
    let old = first_save(&db, a, target, "第一版").await;
    let new = db
        .save_overall_note(a, target, old, Some(&text("第二版")))
        .await
        .unwrap()
        .version;
    assert!(new > old);
    for stale in [0, old] {
        assert!(matches!(
            db.save_overall_note(a, target, stale, Some(&text("过期")))
                .await,
            Err(DbError::OverallNoteChanged)
        ));
        assert!(matches!(
            db.save_overall_note(a, target, stale, None).await,
            Err(DbError::OverallNoteChanged)
        ));
    }
    // Clearing and writing again must not bring an old number back.
    db.save_overall_note(a, target, new, None).await.unwrap();
    let again = first_save(&db, a, target, "重写").await;
    assert!(again > new);
    assert!(matches!(
        db.save_overall_note(a, target, new, Some(&text("过期")))
            .await,
        Err(DbError::OverallNoteChanged)
    ));
    assert_eq!(
        db.overall_note(a, target).await.unwrap(),
        OverallNote {
            text: Some("重写".into()),
            version: again
        }
    );
}

/// Two first saves meet on the document's lock; the second finds a note and writes nothing.
/// Core test case:
/// - `specs/test-cases/web/stars/overall-notes.md#a-stale-write-must-never-replace-an-overall-note`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn concurrent_first_writes_leave_one_overall_note() {
    let (_container, db, pool) = database().await;
    let a = owner(&db, "a").await;
    let target = OverallNoteTarget::Article(article(&db, a, "https://example.com/a", "文章").await);
    let barrier = Arc::new(Barrier::new(2));
    let racer = |words: &'static str| {
        let (db, barrier) = (db.clone(), barrier.clone());
        tokio::spawn(async move {
            barrier.wait().await;
            db.save_overall_note(a, target, /*expected*/ 0, Some(&text(words)))
                .await
        })
    };
    let (left, right) = (racer("左"), racer("右"));
    let results = [left.await.unwrap(), right.await.unwrap()];
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert!(
        results
            .iter()
            .any(|result| matches!(result, Err(DbError::OverallNoteChanged)))
    );
    assert_eq!(rows(&pool).await, 1);
}

/// A note keeps a page alive like an annotation, and binding appends it to the article's, whole
/// or not at all.
/// Core test case:
/// - `specs/test-cases/web/stars/overall-notes.md#binding-a-web-page-must-carry-its-overall-note-into-the-article-whole`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn binding_carries_the_overall_note_into_the_article_whole() {
    let (_container, db, pool) = database().await;
    let a = owner(&db, "a").await;
    let id = article(&db, a, "https://example.com/a", "文章").await;
    let target = OverallNoteTarget::Article(id);
    let page = |n: u8| format!("https://example.com/page-{n}");

    // A page with only a note is a page; clearing that note removes it.
    let note = page_note(&db, a, &page(1), "空网页").await;
    assert_eq!(
        db.web_page_by_url(a, &url(&page(1)))
            .await
            .unwrap()
            .overall_note,
        note
    );
    db.save_web_page_overall_note(a, &url(&page(1)), &title("标签页"), note.version, None)
        .await
        .unwrap();
    assert!(matches!(
        db.web_page_by_url(a, &url(&page(1))).await,
        Err(DbError::NotFound)
    ));

    // The last annotation goes without taking a page that still holds a note.
    let marked = mark(&db, a, &page(2), "引文").await.unwrap();
    let note = page_note(&db, a, &page(2), "第一条").await;
    db.delete_web_annotation(a, marked.id).await.unwrap();
    assert_eq!(
        db.web_page_by_url(a, &url(&page(2)))
            .await
            .unwrap()
            .overall_note,
        note
    );

    // Into an article with no note the text moves as it is, then a second page appends.
    db.bind_web_page(a, &url(&page(2)), &title("标签页"), id)
        .await
        .unwrap();
    assert_eq!(
        db.overall_note(a, target).await.unwrap().text.as_deref(),
        Some("第一条")
    );
    page_note(&db, a, &page(3), "第二条").await;
    db.bind_web_page(a, &url(&page(3)), &title("标签页"), id)
        .await
        .unwrap();
    let merged = db.overall_note(a, target).await.unwrap();
    assert_eq!(merged.text.as_deref(), Some("第一条\n\n---\n\n第二条"));
    assert_eq!(rows(&pool).await, 1);

    // A bound page reads its article's note, and has none of its own to write.
    let bound = db.web_page_by_url(a, &url(&page(3))).await.unwrap();
    assert_eq!(bound.page.state, WebPageState::Bound(id));
    assert_eq!(bound.overall_note, merged);
    assert!(matches!(
        db.save_web_page_overall_note(
            a,
            &url(&page(3)),
            &title("标签页"),
            merged.version,
            Some(&text("迟到"))
        )
        .await,
        Err(DbError::WebPageBound)
    ));

    // A note too long to merge fails the binding, leaving the page and the article as they were.
    let long = page_note(&db, a, &page(4), &"长".repeat(21_845)).await;
    assert!(matches!(
        db.bind_web_page(a, &url(&page(4)), &title("标签页"), id)
            .await,
        Err(DbError::Input(_))
    ));
    let kept = db.web_page_by_url(a, &url(&page(4))).await.unwrap();
    assert_eq!(
        (kept.page.state, kept.overall_note),
        (WebPageState::Unbound, long)
    );
    assert_eq!(db.overall_note(a, target).await.unwrap(), merged);
}
