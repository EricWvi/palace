//! Web edits and deletions against real Git, real files and real PostgreSQL: what the server
//! writes to the work tree, what it publishes, and how later commits treat it.
use super::{Repository, SHANGHAI, changes, database, file, indexed, jotting, run};
use crate::recall::until_blocked;
use palace_db::{DbError, JottingChange, JottingListRequest, ListedJotting};
use palace_domain::{Jotting, ServerVersion};
use palace_jottings::{JottingEdit, Jottings, JottingsError, Uploaded};
use pretty_assertions::assert_eq;
use std::{fs, path::PathBuf, sync::Arc};
use uuid::Uuid;

/// Which refusal a web write must meet.
type Refusal = fn(&JottingsError) -> bool;

/// Every live jotting as the web lists it, newest first.
async fn listed(jottings: &Jottings) -> Vec<ListedJotting> {
    jottings
        .list(&JottingListRequest {
            tag: None,
            search: None,
            after: None,
            limit: 200,
        })
        .await
        .unwrap()
        .jottings
}

/// The web's view of one jotting, or `None` once it is gone.
async fn one(jottings: &Jottings, id: Uuid) -> Option<ListedJotting> {
    listed(jottings)
        .await
        .into_iter()
        .find(|listed| listed.jotting.id == id)
}

fn edit(title: Option<&str>, tags: &[&str], body: &str) -> JottingEdit {
    JottingEdit {
        title: title.map(str::to_owned),
        tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
        body: body.to_owned(),
    }
}

/// Commits only the given paths below the jottings folder, as an agent committing other work.
fn commit_only(repository: &Repository, paths: &[&str], message: &str) {
    for path in paths {
        repository.git(&["add", "--all", &format!("jottings/{path}")]);
    }
    repository.git(&["commit", "--quiet", "--allow-empty", "-m", message]);
}

/// An edit replaces only title, tags and body in the file and publishes at once; an edit that
/// changes nothing writes nothing; a deletion removes the file and publishes a tombstone; an
/// edited upload still answers its retry. Nothing else in the repository, `.git` included,
/// changes.
/// Core test cases:
/// - `specs/test-cases/server/jotting/jotting-sync.md#a-web-edit-must-rewrite-only-its-fields-and-publish-at-once`
/// - `specs/test-cases/server/jotting/jotting-sync.md#the-server-must-write-only-inbox-files-and-the-jotting-the-web-names`
#[test]
#[ignore = "requires local postgres:17-alpine and git"]
fn a_web_edit_rewrites_only_its_fields_and_publishes_at_once() {
    run(async {
        let (_container, database, _pool) = database().await;
        let repository = Repository::new();
        let (a, b) = (Uuid::now_v7(), Uuid::now_v7());
        let original = format!(
            "---\n# kept by the agent\nsource: flomo\nid: {a}\ntags:\n  - 旧\ncreated: 2026-10-19T08:30:00+08:00\n---\n\n\n旧正文\n"
        );
        repository.write(&[("2026/a.md", &original), ("b.md", &file(b, "b"))]);
        repository.commit("add");
        let jottings = Jottings::open(&repository.folder(), database, SHANGHAI).unwrap();
        indexed(2)(jottings.refresh().await.unwrap());
        let before = repository.snapshot();
        let read = one(&jottings, a).await.unwrap();
        assert!(!read.pending);
        let since = changes(&jottings, 0).await.next;

        let version = jottings
            .edit(
                a,
                read.version,
                edit(Some("标题"), &["新", "rust"], "\n新正文"),
            )
            .await
            .unwrap();
        let edited = Jotting {
            id: a,
            created_at: super::CREATED,
            title: Some("标题".into()),
            tags: vec!["新".into(), "rust".into()],
            body: "新正文".into(),
        };
        assert_eq!(
            fs::read_to_string(repository.folder().join("2026").join("a.md")).unwrap(),
            format!(
                "---\n# kept by the agent\nsource: flomo\nid: {a}\ntags: [新, rust]\ncreated: 2026-10-19T08:30:00+08:00\ntitle: 标题\n---\n\n\n新正文"
            )
        );
        assert_eq!(
            changes(&jottings, since.value()).await.changes,
            [JottingChange::Live {
                version,
                jotting: edited.clone(),
            }]
        );
        assert_eq!(
            one(&jottings, a)
                .await
                .map(|listed| (listed.jotting, listed.version, listed.pending)),
            Some((edited.clone(), version, true))
        );
        // The same content again writes nothing and keeps the version.
        let file_after = repository.snapshot();
        assert_eq!(
            jottings
                .edit(a, version, edit(Some("标题"), &["新", "rust"], "新正文"))
                .await
                .unwrap(),
            version
        );
        assert_eq!(repository.snapshot(), file_after);

        let b_version = one(&jottings, b).await.unwrap().version;
        jottings.delete(b, b_version).await.unwrap();
        assert!(!repository.folder().join("b.md").exists());
        assert!(matches!(
            changes(&jottings, version.value()).await.changes.as_slice(),
            [JottingChange::Deleted { id, .. }] if *id == b
        ));
        assert_eq!(one(&jottings, b).await, None);

        // An upload edited before its commit still answers a retry of the upload, as created.
        let upload = Jotting {
            id: Uuid::now_v7(),
            ..jotting(Uuid::nil(), "上传")
        };
        let Uploaded::Created(uploaded) = jottings.upload(upload.clone()).await.unwrap() else {
            panic!("a first upload is created");
        };
        let changed = jottings
            .edit(upload.id, uploaded, edit(None, &[], "改过的上传"))
            .await
            .unwrap();
        assert_eq!(
            jottings.upload(upload.clone()).await.unwrap(),
            Uploaded::Retried(changed)
        );
        jottings.delete(upload.id, changed).await.unwrap();
        assert!(matches!(
            jottings.upload(upload.clone()).await.unwrap(),
            Uploaded::Retried(_)
        ));
        assert_eq!(one(&jottings, upload.id).await, None);

        // Only the two named files changed; `.git` and the rest are as they were.
        let after = repository.snapshot();
        let mut expected = before;
        let a_path = PathBuf::from("jottings").join("2026").join("a.md");
        expected.insert(a_path.clone(), after[&a_path].clone());
        expected.remove(&PathBuf::from("jottings").join("b.md"));
        assert_eq!(after, expected);
    });
}

/// A stale version, a deleted jotting, or a file that is not where the index has it, refuses
/// the write and changes nothing; a write waiting for the publication lock sees the state the
/// writer before it left.
/// Core test case:
/// - `specs/test-cases/server/jotting/jotting-sync.md#a-stale-or-misplaced-web-write-must-change-nothing`
#[test]
#[ignore = "requires local postgres:17-alpine and git"]
fn a_stale_or_misplaced_web_write_changes_nothing() {
    run(async {
        let (_container, database, pool) = database().await;
        let repository = Repository::new();
        let (a, b, c, other) = (
            Uuid::now_v7(),
            Uuid::now_v7(),
            Uuid::now_v7(),
            Uuid::now_v7(),
        );
        repository.write(&[
            ("a.md", &file(a, "a")),
            ("b.md", &file(b, "b")),
            ("c.md", &file(c, "c")),
        ]);
        repository.commit("add");
        let jottings = Arc::new(Jottings::open(&repository.folder(), database, SHANGHAI).unwrap());
        indexed(3)(jottings.refresh().await.unwrap());
        let read = |id| {
            let jottings = jottings.clone();
            async move { one(&jottings, id).await.unwrap().version }
        };
        let (a_read, b_read, c_read) = (read(a).await, read(b).await, read(c).await);

        // a is changed by a commit after the page read it.
        repository.write(&[("a.md", &file(a, "agent"))]);
        repository.commit("agent edits a");
        indexed(1)(jottings.refresh().await.unwrap());
        // b is being moved, c's path now holds another jotting; neither is committed.
        fs::rename(
            repository.folder().join("b.md"),
            repository.folder().join("b-moved.md"),
        )
        .unwrap();
        repository.write(&[("c.md", &file(other, "other"))]);

        let published = changes(&jottings, 0).await;
        let files = repository.snapshot();
        let attempts: [(Uuid, ServerVersion, Refusal); 3] = [
            (a, a_read, |error| matches!(error, JottingsError::Changed)),
            (b, b_read, |error| matches!(error, JottingsError::Moved)),
            (c, c_read, |error| matches!(error, JottingsError::Moved)),
        ];
        for (id, version, expected) in attempts {
            let edited = jottings.edit(id, version, edit(None, &[], "web")).await;
            assert!(edited.as_ref().is_err_and(expected), "{id}: {edited:?}");
            let deleted = jottings.delete(id, version).await;
            assert!(deleted.as_ref().is_err_and(expected), "{id}: {deleted:?}");
        }
        assert!(matches!(
            jottings
                .edit(Uuid::now_v7(), a_read, edit(None, &[], "x"))
                .await,
            Err(JottingsError::Database(DbError::NotFound))
        ));
        assert_eq!(changes(&jottings, 0).await, published);
        assert_eq!(repository.snapshot(), files);
        let recorded: i64 = sqlx::query_scalar("SELECT count(*) FROM jotting_web_write")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(recorded, 0);

        // Another writer holds the publication lock and moves a past the version the page has.
        let current = read(a).await;
        let mut writer = pool.begin().await.unwrap();
        sqlx::query("UPDATE jotting SET body='locked' WHERE id=$1")
            .bind(a)
            .execute(&mut *writer)
            .await
            .unwrap();
        let waiting = tokio::spawn({
            let jottings = jottings.clone();
            async move { jottings.edit(a, current, edit(None, &[], "web")).await }
        });
        until_blocked(&pool).await;
        writer.commit().await.unwrap();
        assert!(matches!(
            waiting.await.unwrap(),
            Err(JottingsError::Changed)
        ));
        assert_eq!(
            fs::read_to_string(repository.folder().join("a.md")).unwrap(),
            file(a, "agent")
        );
    });
}

/// A web edit and a web deletion stand through commits that leave them out, follow a `git mv`,
/// come undone by `git restore`, and give way to commits that carry or change them.
/// Core test cases:
/// - `specs/test-cases/server/jotting/jotting-sync.md#a-web-write-must-survive-commits-that-leave-it-out`
/// - `specs/test-cases/web/echo/jotting-reading.md#pending-must-mean-the-jotting-sits-under-inbox-or-holds-an-uncommitted-web-edit`
#[test]
#[ignore = "requires local postgres:17-alpine and git"]
fn web_writes_survive_commits_that_leave_them_out() {
    run(async {
        let (_container, database, pool) = database().await;
        let repository = Repository::new();
        let (a, b) = (Uuid::now_v7(), Uuid::now_v7());
        repository.write(&[("a.md", &file(a, "a")), ("b.md", &file(b, "b"))]);
        repository.commit("add");
        let jottings = Jottings::open(&repository.folder(), database, SHANGHAI).unwrap();
        indexed(2)(jottings.refresh().await.unwrap());
        let a_read = one(&jottings, a).await.unwrap().version;
        let b_read = one(&jottings, b).await.unwrap().version;
        let web_a = jottings
            .edit(a, a_read, edit(None, &[], "web"))
            .await
            .unwrap();
        jottings.delete(b, b_read).await.unwrap();
        let state = |listed: Option<ListedJotting>| {
            listed.map(|listed| (listed.jotting.body, listed.version, listed.pending))
        };

        // Other work is committed; the web's results stand and nothing is published.
        repository.write(&[("other.md", &file(Uuid::now_v7(), "other"))]);
        commit_only(&repository, &["other.md"], "other work");
        indexed(1)(jottings.refresh().await.unwrap());
        assert_eq!(
            state(one(&jottings, a).await),
            Some(("web".into(), web_a, true))
        );
        assert_eq!(one(&jottings, b).await, None);
        let quiet = changes(&jottings, 0).await.next;

        // `git mv` moves the web's file and stages the old bytes at the new path.
        fs::create_dir_all(repository.folder().join("2026")).unwrap();
        repository.git(&["mv", "jottings/a.md", "jottings/2026/a.md"]);
        repository.git(&["commit", "--quiet", "-m", "sort a"]);
        jottings.refresh().await.unwrap();
        assert_eq!(
            state(one(&jottings, a).await),
            Some(("web".into(), web_a, true))
        );
        assert_eq!(changes(&jottings, 0).await.next, quiet);
        // The next web edit finds the file at its new place.
        let web_a = jottings
            .edit(a, web_a, edit(None, &[], "web again"))
            .await
            .unwrap();
        assert_eq!(
            fs::read_to_string(repository.folder().join("2026").join("a.md")).unwrap(),
            file(a, "web again")
        );

        // `git restore` undoes the deletion; the next commit brings b back, in recall at the
        // least count.
        repository.git(&["restore", "jottings/b.md"]);
        repository.write(&[("more.md", &file(Uuid::now_v7(), "more"))]);
        commit_only(&repository, &["more.md"], "more work");
        jottings.refresh().await.unwrap();
        assert_eq!(
            one(&jottings, b)
                .await
                .map(|listed| (listed.jotting, listed.pending)),
            Some((jotting(b, "b"), false))
        );
        let least: Option<i32> =
            sqlx::query_scalar("SELECT completions FROM jotting_recall WHERE jotting_id=$1")
                .bind(b)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(least, Some(0));

        // The agent commits the web's edit as it is: it is no longer pending, nothing is
        // published.
        commit_only(&repository, &["2026/a.md"], "commit the web edit");
        jottings.refresh().await.unwrap();
        assert_eq!(
            state(one(&jottings, a).await),
            Some(("web again".into(), web_a, false))
        );

        // Edited on the web again, then changed by the agent in a commit: the commit decides.
        let web_a = jottings
            .edit(a, web_a, edit(None, &[], "web third"))
            .await
            .unwrap();
        repository.write(&[("2026/a.md", &file(a, "agent"))]);
        commit_only(&repository, &["2026/a.md"], "agent rewrites a");
        jottings.refresh().await.unwrap();
        let after = one(&jottings, a).await.unwrap();
        assert_eq!(
            (after.jotting.body.as_str(), after.pending),
            ("agent", false)
        );
        assert!(after.version > web_a);
        let recorded: i64 = sqlx::query_scalar("SELECT count(*) FROM jotting_web_write")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(recorded, 0);
    });
}
