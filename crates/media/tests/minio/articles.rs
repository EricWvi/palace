use super::{Services, fail_media, heal_media, keys, rows, services};
use bytes::Bytes;
use palace_db::{ArticleImport, DbError, OwnerScope};
use palace_domain::{
    ArticleBody, ArticleMetadata, ArticleMetadataInput, MediaLimits, MediaName, MediaUpload,
};
use palace_media::{
    ArticleStaging, Disposition, MediaError, MediaStore, S3Bucket, StoreError, object_key,
};
use pretty_assertions::assert_eq;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use tokio::sync::Barrier;

fn metadata(url: &str) -> ArticleMetadata {
    ArticleMetadata::parse(
        ArticleMetadataInput {
            title: "文章".into(),
            url: url.into(),
            author: None,
            date: "2026-10-05".into(),
        },
        chrono_tz::Asia::Shanghai,
    )
    .unwrap()
}

fn file(name: &str, content_type: &str, bytes: &[u8]) -> (MediaName, MediaUpload) {
    (
        MediaName::parse(name).unwrap(),
        MediaUpload::new(
            bytes.to_vec(),
            Some(content_type),
            Some(name),
            MediaLimits::default(),
        )
        .unwrap(),
    )
}

const BODY: &str = "![封面](media:cover.png)\n\n![演示](media:demo.mp4)\n";

/// Stages both files of [`BODY`] and commits the article.
async fn import<S: MediaStore>(
    s: &Services,
    store: &S,
    owner: OwnerScope,
    url: &str,
) -> Result<ArticleImport, MediaError> {
    let mut staging = ArticleStaging::new(&s.db, Some(store), owner);
    for (name, upload) in [
        file("cover.png", "image/png", b"png"),
        file("demo.mp4", "video/mp4", b"mp4"),
    ] {
        if let Err(error) = staging.stage(name, upload).await {
            staging.discard().await;
            return Err(error);
        }
    }
    staging
        .commit(&metadata(url), ArticleBody::parse(BODY.into()).unwrap())
        .await
}

/// A committed import holds exactly its files, addressed by id; any failure after the objects
/// were written leaves neither rows nor objects.
/// Core test case:
/// - `specs/test-cases/web/stars/articles.md#article-import-must-commit-the-article-and-its-media-together-or-not-at-all`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine and minio/minio images and Docker/Podman socket"]
async fn article_import_must_commit_the_article_and_its_media_together() {
    let s = services().await;
    let ArticleImport::Created(article) = import(&s, &s.bucket, s.a, "https://example.com/a")
        .await
        .unwrap()
    else {
        panic!("the URL was new");
    };
    let [cover, demo] = [article.media[0].id, article.media[1].id];
    assert_eq!(
        article.content,
        format!("![封面](/api/media/{cover})\n\n![演示](/api/media/{demo})\n")
    );
    assert_eq!(
        article
            .media
            .iter()
            .map(|media| media.content_type.as_str())
            .collect::<Vec<_>>(),
        vec!["image/png", "video/mp4"]
    );
    let committed = vec![object_key(s.a.id(), cover), object_key(s.a.id(), demo)];
    assert_eq!(keys(&s.raw).await, committed);

    // The database refuses the media rows after both objects were written.
    fail_media(&s.pool, "insert").await;
    assert!(matches!(
        import(&s, &s.bucket, s.a, "https://example.com/b").await,
        Err(MediaError::Database(DbError::Storage(_)))
    ));
    heal_media(&s.pool, "insert").await;
    // A body that embeds a file the import did not send.
    let mut staging = ArticleStaging::new(&s.db, Some(&s.bucket), s.a);
    let (name, upload) = file("cover.png", "image/png", b"png");
    staging.stage(name, upload).await.unwrap();
    assert!(matches!(
        staging
            .commit(
                &metadata("https://example.com/c"),
                ArticleBody::parse(BODY.into()).unwrap()
            )
            .await,
        Err(MediaError::Input(_))
    ));
    // A name sent twice is refused before its second object is written.
    let mut staging = ArticleStaging::new(&s.db, Some(&s.bucket), s.a);
    let (name, upload) = file("cover.png", "image/png", b"png");
    staging.stage(name.clone(), upload.clone()).await.unwrap();
    assert!(matches!(
        staging.stage(name, upload).await,
        Err(MediaError::Input(_))
    ));
    staging.discard().await;
    // Without storage the files cannot be kept at all.
    let mut staging = ArticleStaging::<S3Bucket>::new(&s.db, None, s.a);
    let (name, upload) = file("cover.png", "image/png", b"png");
    assert!(matches!(
        staging.stage(name, upload).await,
        Err(MediaError::Unconfigured)
    ));

    assert_eq!(keys(&s.raw).await, committed);
    assert_eq!(rows(&s.pool).await.len(), 2);
    let articles: i64 = sqlx::query_scalar("SELECT count(*) FROM article")
        .fetch_one(&s.pool)
        .await
        .unwrap();
    assert_eq!(articles, 1);
}

/// Holds every write until both imports have stored their objects, so both reach the commit
/// with the same URL and the database decides between them.
struct Gate<'a> {
    inner: &'a S3Bucket,
    barrier: Arc<Barrier>,
}
impl MediaStore for Gate<'_> {
    async fn put(&self, key: &str, content: Bytes, content_type: &str) -> Result<(), StoreError> {
        self.inner.put(key, content, content_type).await?;
        self.barrier.wait().await;
        Ok(())
    }
    async fn delete(&self, key: &str) -> Result<(), StoreError> {
        self.inner.delete(key).await
    }
}

/// Two imports of one URL that both stored their files end with one article; the other import
/// gets that article and removes its own objects.
/// Core test case:
/// - `specs/test-cases/web/stars/articles.md#importing-the-same-url-must-converge-on-one-article`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine and minio/minio images and Docker/Podman socket"]
async fn concurrent_imports_of_one_url_converge_on_one_article() {
    let s = services().await;
    // Each import writes two objects; both imports pass each write together.
    let barrier = Arc::new(Barrier::new(2));
    let first = Gate {
        inner: &s.bucket,
        barrier: barrier.clone(),
    };
    let second = Gate {
        inner: &s.bucket,
        barrier,
    };
    let (one, two) = tokio::join!(
        import(&s, &first, s.a, "https://example.com/same"),
        import(&s, &second, s.a, "https://example.com/same"),
    );
    let (created, existing) = match (one.unwrap(), two.unwrap()) {
        (ArticleImport::Created(created), ArticleImport::Existing(existing))
        | (ArticleImport::Existing(existing), ArticleImport::Created(created)) => {
            (created, existing)
        }
        outcome => panic!("expected one article, got {outcome:?}"),
    };
    assert_eq!(existing, created);
    let mut expected: Vec<String> = created
        .media
        .iter()
        .map(|media| object_key(s.a.id(), media.id))
        .collect();
    expected.sort();
    assert_eq!(keys(&s.raw).await, expected);
    assert_eq!(rows(&s.pool).await.len(), 2);

    // The URL is unique only within an owner.
    assert!(matches!(
        import(&s, &s.bucket, s.b, "https://example.com/same").await,
        Ok(ArticleImport::Created(_))
    ));
}

/// Fails the `n`th object delete (counting from 1) and passes every other request through.
struct FailNthDelete<'a> {
    inner: &'a S3Bucket,
    n: usize,
    seen: AtomicUsize,
}
impl MediaStore for FailNthDelete<'_> {
    async fn put(&self, key: &str, content: Bytes, content_type: &str) -> Result<(), StoreError> {
        self.inner.put(key, content, content_type).await
    }
    async fn delete(&self, key: &str) -> Result<(), StoreError> {
        if self.seen.fetch_add(1, Ordering::SeqCst) + 1 == self.n {
            return Err(StoreError::new("injected delete failure"));
        }
        self.inner.delete(key).await
    }
}

/// Media an article embeds goes only with the article; a failure on the second object keeps
/// the article readable, and the retry removes everything.
/// Core test cases:
/// - `specs/test-cases/web/stars/articles.md#deleting-an-article-must-remove-its-media-objects-before-reporting-success`
/// - `specs/test-cases/server/media/media-reading.md#referenced-media-must-not-be-deletable-on-its-own`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine and minio/minio images and Docker/Podman socket"]
async fn deleting_an_article_removes_its_objects_before_its_rows() {
    let s = services().await;
    let ArticleImport::Created(article) = import(&s, &s.bucket, s.a, "https://example.com/a")
        .await
        .unwrap()
    else {
        panic!("the URL was new");
    };
    let embedded = article.media[0].id;
    assert!(matches!(
        palace_media::delete(&s.db, &s.bucket, s.a, embedded).await,
        Err(MediaError::Embedded)
    ));
    assert_eq!(keys(&s.raw).await.len(), 2);
    // A file uploaded on its own is still deleted as before.
    let free = palace_media::upload(
        &s.db,
        &s.bucket,
        s.a,
        file("free.png", "image/png", b"free").1,
    )
    .await
    .unwrap();
    palace_media::delete(&s.db, &s.bucket, s.a, free.id)
        .await
        .unwrap();

    let failing = FailNthDelete {
        inner: &s.bucket,
        n: 2,
        seen: AtomicUsize::new(0),
    };
    assert!(matches!(
        palace_media::delete_article(&s.db, Some(&failing), s.a, article.id).await,
        Err(MediaError::Store(_))
    ));
    // One object is gone, but the article and every row remain for the retry.
    assert_eq!(keys(&s.raw).await.len(), 1);
    assert_eq!(s.db.article(s.a, article.id).await.unwrap(), article);
    assert_eq!(rows(&s.pool).await.len(), 2);
    assert!(matches!(
        palace_media::delete_article(&s.db, Some(&s.bucket), s.b, article.id).await,
        Err(MediaError::Database(DbError::NotFound))
    ));
    palace_media::delete_article(&s.db, Some(&s.bucket), s.a, article.id)
        .await
        .unwrap();
    assert_eq!(keys(&s.raw).await, Vec::<String>::new());
    assert_eq!(rows(&s.pool).await, Vec::new());
    assert!(matches!(
        s.db.media_object(s.a, embedded).await,
        Err(DbError::NotFound)
    ));
    let moments: i64 = sqlx::query_scalar("SELECT count(*) FROM moment")
        .fetch_one(&s.pool)
        .await
        .unwrap();
    assert_eq!(moments, 0);
}

/// A URL signed for the bucket fetches the stored bytes with the recorded type; anything a page
/// does not embed downloads instead of opening.
/// Core test case:
/// - `specs/test-cases/server/media/media-reading.md#presigned-media-urls-must-be-stable-within-the-hour-and-safe-to-open`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine and minio/minio images and Docker/Podman socket"]
async fn presigned_urls_fetch_the_stored_bytes() {
    let s = services().await;
    let client = reqwest::Client::new();
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    for (name, content_type, disposition, expected) in [
        ("a.png", "image/png", Disposition::Inline, "inline"),
        (
            "page.html",
            "text/html",
            Disposition::Attachment("page.html"),
            "attachment; filename*=UTF-8''page.html",
        ),
    ] {
        let media =
            palace_media::upload(&s.db, &s.bucket, s.a, file(name, content_type, b"bytes").1)
                .await
                .unwrap();
        let object = s.db.media_object(s.a, media.id).await.unwrap();
        let signed =
            s.signer
                .presign_get(&object.object_key, &object.content_type, disposition, now);
        let response = client.get(&signed.url).send().await.unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        let headers = response.headers().clone();
        assert_eq!(
            (
                headers["content-type"].to_str().unwrap(),
                headers["content-disposition"].to_str().unwrap(),
            ),
            (content_type, expected)
        );
        assert_eq!(
            response.bytes().await.unwrap(),
            Bytes::from_static(b"bytes")
        );
    }
}
