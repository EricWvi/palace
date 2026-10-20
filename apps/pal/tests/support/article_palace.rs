//! Assertions shared with the real PostgreSQL/MinIO Palace fixture in palace.rs.
use super::{article, pal};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use sqlx::PgPool;

/// Exercises the real CLI and database so URL idempotency and ownership are not inferred from mocks.
/// Core test case:
/// - `specs/test-cases/cli/article-upload.md#repeated-article-uploads-must-converge-within-each-owner`
pub(super) async fn exercise(pool: &PgPool, owner: &[(&str, &str)], foreign: &[(&str, &str)]) {
    let directory = article::directory("# 标题\n\n![](assets/a.jpg) ![](assets/b.mp4)\n");
    let output = pal(&article::args(directory.path()), owner).await;
    assert_eq!((output.code, output.stderr.as_str()), (Some(0), ""));
    let created: Value = serde_json::from_str(&output.stdout).unwrap();
    let id = uuid::Uuid::parse_str(created["id"].as_str().unwrap()).unwrap();
    let row: (String, String, String, i64) = sqlx::query_as("SELECT a.title,a.url,a.content,(extract(epoch FROM m.occurred_at)*1000)::bigint FROM article a JOIN moment m ON m.id=a.id AND m.owner_id=a.owner_id WHERE a.id=$1")
        .bind(id).fetch_one(pool).await.unwrap();
    let media: Vec<(uuid::Uuid, String, String, i64)> = sqlx::query_as("SELECT md.id,md.content_type,md.file_name,md.byte_size FROM media md JOIN article_media am ON am.media_id=md.id AND am.owner_id=md.owner_id JOIN article a ON a.id=am.article_id AND a.owner_id=am.owner_id WHERE a.id=$1 ORDER BY md.file_name")
        .bind(id).fetch_all(pool).await.unwrap();
    assert_eq!(media.len(), 2);
    let image_id = media[0].0;
    let video_id = media[1].0;
    assert_eq!(
        media,
        vec![
            (image_id, "image/jpeg".into(), "a.jpg".into(), 5),
            (video_id, "video/mp4".into(), "b.mp4".into(), 5),
        ]
    );
    let content = format!("# 标题\n\n![](/api/media/{image_id}) ![](/api/media/{video_id})\n");
    assert_eq!(
        row,
        (
            "标题".into(),
            "https://example.com/original?keep=1".into(),
            content.clone(),
            created["occurred_at"].as_i64().unwrap()
        )
    );
    let mut expected_media = vec![
        json!({"id":image_id,"content_type":"image/jpeg"}),
        json!({"id":video_id,"content_type":"video/mp4"}),
    ];
    expected_media.sort_by_key(|entry| entry["id"].as_str().unwrap().to_owned());
    assert_eq!(
        created,
        json!({
            "id":id,"title":"标题","url":"https://example.com/original?keep=1","author":"作者","site":"example.com",
            "date":"2026-07-03","published_at":1783008000000_i64,"occurred_at":row.3,
            "content":content,"media":expected_media
        })
    );
    // Re-running a changed local file must return the original complete article unchanged.
    std::fs::write(directory.path().join("post.md"), "changed body").unwrap();
    assert_eq!(pal(&article::args(directory.path()), owner).await, output);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM article_media WHERE article_id=$1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(count, 2);

    let other = pal(&article::args(directory.path()), foreign).await;
    assert_eq!((other.code, other.stderr.as_str()), (Some(0), ""));
    let other: Value = serde_json::from_str(&other.stdout).unwrap();
    assert_ne!(other["id"], created["id"]);
    let client = reqwest::Client::new();
    let get = client
        .get(format!("{}/api/articles/{id}", foreign[0].1))
        .bearer_auth(foreign[1].1)
        .send()
        .await
        .unwrap();
    assert_eq!(
        (get.status(), get.json::<Value>().await.unwrap()),
        (
            reqwest::StatusCode::NOT_FOUND,
            json!({"error":"not_found","login":""})
        )
    );
    // The redirect is Owner-checked and the stored bytes are fetched from real MinIO.
    for (media_id, bytes) in [(image_id, b"image"), (video_id, b"video")] {
        let response = client
            .get(format!("{}/api/media/{media_id}", owner[0].1))
            .bearer_auth(owner[1].1)
            .send()
            .await
            .unwrap();
        assert_eq!(response.bytes().await.unwrap().as_ref(), bytes);
    }
}
