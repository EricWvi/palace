use super::database;
use super::paths::{branch, create};
use palace_db::{Annotation, ArticleImport, Database, DbError, OwnerScope};
use palace_domain::{
    AnnotationAnchor, AnnotationContent, AnnotationNote, AnnotationStyle, AnnotationTarget,
    ArticleMetadata, ArticleMetadataInput,
};
use pretty_assertions::assert_eq;
use sqlx::PgPool;
use uuid::Uuid;

/// Saves an article at `url` and returns its id.
async fn article(db: &Database, owner: OwnerScope, url: &str) -> Uuid {
    let metadata = ArticleMetadata::parse(
        ArticleMetadataInput {
            title: "文章".into(),
            url: url.into(),
            author: None,
            date: "2026-10-05".into(),
        },
        chrono_tz::Asia::Shanghai,
    )
    .unwrap();
    match db
        .insert_article(owner, &metadata, "正文", &[])
        .await
        .unwrap()
    {
        ArticleImport::Created(article) => article.id,
        ArticleImport::Existing(article) => panic!("{} already existed", article.id),
    }
}

fn anchor(quote: &str) -> AnnotationAnchor {
    let length = i64::try_from(quote.encode_utf16().count()).unwrap();
    AnnotationAnchor::parse(/*start*/ 2, 2 + length, quote.into()).unwrap()
}

fn noted(style: Option<&str>, note: &str) -> AnnotationContent {
    let style = style.map(|style| AnnotationStyle::parse(style).unwrap());
    AnnotationContent::parse(style, Some(note.into())).unwrap()
}

fn styled(style: &str) -> AnnotationContent {
    AnnotationContent::parse(AnnotationStyle::parse(style), /*note*/ None).unwrap()
}

/// The id of the conversation's message with this content.
async fn message(db: &Database, owner: OwnerScope, conversation: Uuid, content: &str) -> Uuid {
    db.conversation_detail(owner, conversation)
        .await
        .unwrap()
        .messages
        .into_iter()
        .find(|message| message.content == content)
        .unwrap()
        .id
}

/// Counts every annotation row, whoever owns it.
async fn count(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM annotation")
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Another owner can neither see nor change an annotation, nor annotate someone else's text, and
/// a message is only reachable through its own conversation.
/// Core test case:
/// - `specs/test-cases/web/stars/annotations.md#annotations-must-stay-within-the-owner-scope`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn annotations_must_stay_within_the_owner_scope() {
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
    let a_article = article(&db, a, "https://example.com/a").await;
    let b_article = article(&db, b, "https://example.com/a").await;
    let a_tree = db
        .import_path(a, &create("a", &["U1", "A1"]))
        .await
        .unwrap();
    let a_other = db.import_path(a, &create("a2", &["X"])).await.unwrap();
    let b_tree = db.import_path(b, &create("b", &["U1"])).await.unwrap();
    let a_message = message(&db, a, a_tree.conversation_id, "A1").await;

    let on_article = db
        .insert_annotation(
            a,
            AnnotationTarget::Article(a_article),
            &anchor("正文"),
            &styled("yellow"),
        )
        .await
        .unwrap();
    let on_message = db
        .insert_annotation(
            a,
            AnnotationTarget::Message {
                conversation: a_tree.conversation_id,
                message: a_message,
            },
            &anchor("A1"),
            &noted(/*style*/ None, "想法"),
        )
        .await
        .unwrap();
    assert_eq!(
        on_message,
        Annotation {
            id: on_message.id,
            target: AnnotationTarget::Message {
                conversation: a_tree.conversation_id,
                message: a_message,
            },
            anchor: anchor("A1"),
            content: AnnotationContent::Noted(AnnotationNote::parse("想法".into()).unwrap()),
            created_at: on_message.created_at,
            updated_at: on_message.created_at,
        }
    );
    assert_eq!(
        db.article_annotations(a, a_article).await.unwrap(),
        vec![on_article.clone()]
    );
    assert_eq!(
        db.conversation_annotations(a, a_tree.conversation_id)
            .await
            .unwrap(),
        vec![on_message.clone()]
    );

    // Every way B can name A's text or A's annotations looks like nothing is there.
    assert!(matches!(
        db.article_annotations(b, a_article).await,
        Err(DbError::NotFound)
    ));
    assert!(matches!(
        db.conversation_annotations(b, a_tree.conversation_id).await,
        Err(DbError::NotFound)
    ));
    for target in [
        AnnotationTarget::Article(a_article),
        AnnotationTarget::Message {
            conversation: a_tree.conversation_id,
            message: a_message,
        },
        // B's own conversation, A's message.
        AnnotationTarget::Message {
            conversation: b_tree.conversation_id,
            message: a_message,
        },
    ] {
        assert!(matches!(
            db.insert_annotation(b, target, &anchor("正文"), &styled("blue"))
                .await,
            Err(DbError::NotFound)
        ));
    }
    // A's own message under A's other conversation.
    assert!(matches!(
        db.insert_annotation(
            a,
            AnnotationTarget::Message {
                conversation: a_other.conversation_id,
                message: a_message,
            },
            &anchor("A1"),
            &styled("blue"),
        )
        .await,
        Err(DbError::NotFound)
    ));
    assert!(matches!(
        db.update_annotation(b, on_article.id, &styled("rose"))
            .await,
        Err(DbError::NotFound)
    ));
    assert!(matches!(
        db.delete_annotation(b, on_article.id).await,
        Err(DbError::NotFound)
    ));
    assert_eq!(db.article_annotations(b, b_article).await.unwrap(), vec![]);
    assert_eq!(count(&pool).await, 2);

    // Editing replaces the style and the note together and keeps the anchor.
    let edited = db
        .update_annotation(a, on_article.id, &noted(Some("wavy"), "再想想"))
        .await
        .unwrap();
    assert_eq!(
        edited,
        Annotation {
            content: AnnotationContent::StyledNoted(
                AnnotationStyle::Wavy,
                AnnotationNote::parse("再想想".into()).unwrap()
            ),
            updated_at: edited.updated_at,
            ..on_article.clone()
        }
    );
    assert!(edited.updated_at >= on_article.updated_at);
    db.delete_annotation(a, on_article.id).await.unwrap();
    assert!(matches!(
        db.delete_annotation(a, on_article.id).await,
        Err(DbError::NotFound)
    ));
    assert_eq!(db.article_annotations(a, a_article).await.unwrap(), vec![]);
}

/// The table itself refuses an empty annotation, two targets at once and any change of anchor.
/// Core test case:
/// - `specs/test-cases/web/stars/annotations.md#an-annotation-must-keep-a-style-or-a-note-and-an-immutable-anchor`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn annotation_rows_must_keep_content_and_anchor() {
    let (_container, db, pool) = database().await;
    let owner = db
        .resolve_identity("https://idp", "a", "a@example.com")
        .await
        .unwrap()
        .scope();
    let id = article(&db, owner, "https://example.com/a").await;
    let tree = db.import_path(owner, &create("s", &["U1"])).await.unwrap();
    let message = message(&db, owner, tree.conversation_id, "U1").await;
    let annotation = db
        .insert_annotation(
            owner,
            AnnotationTarget::Article(id),
            &anchor("正文"),
            &styled("green"),
        )
        .await
        .unwrap();
    for statement in [
        "UPDATE annotation SET style=NULL WHERE id=$1",
        "UPDATE annotation SET start_offset=0 WHERE id=$1",
        "UPDATE annotation SET end_offset=9 WHERE id=$1",
        "UPDATE annotation SET quote='别处' WHERE id=$1",
        "UPDATE annotation SET style='red' WHERE id=$1",
        "UPDATE annotation SET note=' ' WHERE id=$1",
    ] {
        assert!(
            sqlx::query(statement)
                .bind(annotation.id)
                .execute(&pool)
                .await
                .is_err(),
            "{statement}"
        );
    }
    let moved = sqlx::query(
        "UPDATE annotation SET article_id=NULL,conversation_id=$2,message_id=$3 WHERE id=$1",
    )
    .bind(annotation.id)
    .bind(tree.conversation_id)
    .bind(message)
    .execute(&pool)
    .await;
    assert!(moved.is_err());
    let both = sqlx::query("INSERT INTO annotation(id,owner_id,article_id,conversation_id,message_id,start_offset,end_offset,quote,style) VALUES($1,$2,$3,$4,$5,0,2,'正文','blue')")
        .bind(Uuid::now_v7())
        .bind(owner.id())
        .bind(id)
        .bind(tree.conversation_id)
        .bind(message)
        .execute(&pool)
        .await;
    assert!(both.is_err());
    assert_eq!(
        db.article_annotations(owner, id).await.unwrap(),
        vec![annotation]
    );
}

/// Deleting an article, a path or a conversation takes the annotations on the deleted text with
/// it, and only those.
/// Core test case:
/// - `specs/test-cases/web/stars/annotations.md#annotations-must-disappear-with-the-text-they-mark`
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn annotations_must_disappear_with_the_text_they_mark() {
    let (_container, db, pool) = database().await;
    let owner = db
        .resolve_identity("https://idp", "a", "a@example.com")
        .await
        .unwrap()
        .scope();
    let id = article(&db, owner, "https://example.com/a").await;
    db.insert_annotation(
        owner,
        AnnotationTarget::Article(id),
        &anchor("正文"),
        &styled("yellow"),
    )
    .await
    .unwrap();
    db.delete_article(owner, id).await.unwrap();
    assert_eq!(count(&pool).await, 0);

    let first = db
        .import_path(owner, &create("s1", &["U1", "A1", "U2", "A2"]))
        .await
        .unwrap();
    let conversation = first.conversation_id;
    let side = db
        .import_path(owner, &branch(conversation, "s2", &["U1", "A1", "X"]))
        .await
        .unwrap();
    let mut marks = Vec::new();
    for content in ["A1", "A2", "X"] {
        let message = message(&db, owner, conversation, content).await;
        marks.push(
            db.insert_annotation(
                owner,
                AnnotationTarget::Message {
                    conversation,
                    message,
                },
                &anchor(content),
                &styled("underline"),
            )
            .await
            .unwrap(),
        );
    }
    let [shared, only_first, only_side] = <[Annotation; 3]>::try_from(marks).unwrap();
    db.delete_path(owner, conversation, first.path_id)
        .await
        .unwrap();
    assert_eq!(
        db.conversation_annotations(owner, conversation)
            .await
            .unwrap(),
        vec![shared, only_side]
    );
    assert!(matches!(
        db.update_annotation(owner, only_first.id, &styled("rose"))
            .await,
        Err(DbError::NotFound)
    ));
    assert_ne!(side.path_id, first.path_id);
    db.delete_conversation(owner, conversation).await.unwrap();
    assert_eq!(count(&pool).await, 0);
}
