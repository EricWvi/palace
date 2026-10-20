// Integration-test helpers should fail immediately when setup fails.
#![allow(clippy::unwrap_used, clippy::expect_used)]

//! Real Git repositories driven by the `git` CLI, real PostgreSQL and real files: the jotting
//! index follows commits exactly as the agent makes them.
use palace_db::{Database, JottingChange, JottingChanges};
use palace_domain::{InputErrorKind, Jotting, JottingError, JottingProblem, ServerVersion};
use palace_jottings::{Jottings, JottingsError, OpenError, Reconciled, Uploaded, watch};
use pretty_assertions::assert_eq;
use sqlx::PgPool;
use std::{
    collections::BTreeMap,
    fs,
    future::Future,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
    time::Duration,
};
use testcontainers::{
    ContainerAsync, GenericImage, ImageExt,
    core::{IntoContainerPort, WaitFor},
    runners::AsyncRunner,
};
use uuid::Uuid;

#[path = "jottings/reading.rs"]
mod reading;

const SHANGHAI: chrono_tz::Tz = chrono_tz::Asia::Shanghai;
// 2026-10-19 08:30 in Shanghai.
const CREATED: i64 = 1_792_369_800_000;

/// Runs one test body on a fresh runtime under a scoped TRACE subscriber, so every logging
/// callsite the library reaches stays observable for later structured-log tests.
fn run(body: impl Future<Output = ()>) {
    palace_logging::with_trace_logging(|| {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(body);
    });
}

/// Starts an isolated database whose container Ryuk removes even if the test process is killed.
async fn database() -> (ContainerAsync<GenericImage>, Database, PgPool) {
    let container = palace_testkit::reaped(
        GenericImage::new("postgres", "17-alpine")
            .with_exposed_port(5432.tcp())
            .with_wait_for(WaitFor::message_on_stderr(
                "database system is ready to accept connections",
            )),
    )
    .unwrap()
    .with_env_var("POSTGRES_PASSWORD", "palace-test")
    .with_env_var("POSTGRES_DB", "palace")
    .start()
    .await
    .unwrap();
    let host = container.get_host().await.unwrap();
    let port = container
        .get_host_port_ipv4(/*internal_port*/ 5432)
        .await
        .unwrap();
    let url = format!("postgres://postgres:palace-test@{host}:{port}/palace");
    let db = Database::connect(&url).await.unwrap();
    let pool = PgPool::connect(&url).await.unwrap();
    (container, db, pool)
}

/// A repository whose jottings live in its `jottings/` subdirectory, as in a shared notes repo.
struct Repository {
    _dir: tempfile::TempDir,
    root: PathBuf,
}

impl Repository {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_owned();
        let repository = Self { _dir: dir, root };
        repository.git(&["init", "--quiet", "--initial-branch=main"]);
        fs::create_dir_all(repository.folder()).unwrap();
        repository
    }

    fn folder(&self) -> PathBuf {
        self.root.join("jottings")
    }

    /// Runs git with a fixed identity and no user or system configuration.
    fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(&self.root)
            .args([
                "-c",
                "user.name=Agent",
                "-c",
                "user.email=agent@palace.test",
            ])
            .args(["-c", "commit.gpgsign=false"])
            .args(args)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap();
        assert!(output.status.success(), "git {args:?}: {output:?}");
        String::from_utf8(output.stdout).unwrap()
    }

    /// Writes files below the jottings folder.
    fn write(&self, files: &[(&str, &str)]) {
        for (path, text) in files {
            let path = self.folder().join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
    }

    fn remove(&self, path: &str) {
        fs::remove_file(self.folder().join(path)).unwrap();
    }

    /// Stages everything and commits, allowing an empty commit.
    fn commit(&self, message: &str) {
        self.git(&["add", "--all"]);
        self.git(&["commit", "--quiet", "--allow-empty", "-m", message]);
    }

    /// Every file of the repository, `.git` included, with its bytes.
    fn snapshot(&self) -> BTreeMap<PathBuf, Vec<u8>> {
        let mut files = BTreeMap::new();
        let mut pending = vec![self.root.clone()];
        while let Some(directory) = pending.pop() {
            for entry in fs::read_dir(directory).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    pending.push(path);
                } else {
                    let bytes = fs::read(&path).unwrap();
                    files.insert(path.strip_prefix(&self.root).unwrap().to_owned(), bytes);
                }
            }
        }
        files
    }
}

fn file(id: Uuid, body: &str) -> String {
    format!("---\nid: {id}\ncreated: 2026-10-19T08:30:00+08:00\n---\n\n{body}")
}

fn jotting(id: Uuid, body: &str) -> Jotting {
    Jotting {
        id,
        created_at: CREATED,
        title: None,
        tags: vec![],
        body: body.into(),
    }
}

fn version(value: i64) -> ServerVersion {
    ServerVersion::new(value).unwrap()
}

fn live(value: i64, jotting: Jotting) -> JottingChange {
    JottingChange::Live {
        version: version(value),
        jotting,
    }
}

async fn changes(jottings: &Jottings, since: i64) -> JottingChanges {
    jottings.changes(version(since), 1000).await.unwrap()
}

fn indexed(writes: usize) -> impl Fn(Reconciled) {
    move |outcome| match outcome {
        Reconciled::Indexed { writes: actual, .. } => assert_eq!(actual, writes),
        other => panic!("expected an indexed commit, got {other:?}"),
    }
}

/// Moving a file keeps its id and publishes nothing; moving and editing publishes one change.
/// Core test case:
/// - `specs/test-cases/server/jotting/jotting-sync.md#moving-a-jotting-file-must-keep-its-identity`
#[test]
#[ignore = "requires local postgres:17-alpine and git"]
fn moving_a_file_keeps_the_jotting() {
    run(async {
        let (_container, database, _pool) = database().await;
        let repository = Repository::new();
        let id = Uuid::now_v7();
        repository.write(&[("inbox/a.md", &file(id, "first"))]);
        repository.commit("add");
        let jottings = Jottings::open(&repository.folder(), database, SHANGHAI).unwrap();
        indexed(1)(jottings.refresh().await.unwrap());
        assert_eq!(
            changes(&jottings, 0).await.changes,
            [live(1, jotting(id, "first"))]
        );

        repository.git(&["mv", "jottings/inbox/a.md", "jottings/a.md"]);
        repository.commit("move");
        indexed(1)(jottings.refresh().await.unwrap());
        assert_eq!(
            changes(&jottings, 1).await,
            JottingChanges {
                changes: vec![],
                next: version(1),
                has_more: false,
            }
        );

        repository.git(&["mv", "jottings/a.md", "jottings/b.md"]);
        repository.write(&[("b.md", &file(id, "edited"))]);
        repository.commit("move and edit");
        indexed(1)(jottings.refresh().await.unwrap());
        assert_eq!(
            changes(&jottings, 1).await.changes,
            [live(2, jotting(id, "edited"))]
        );
    });
}

/// Work-tree edits are invisible until committed, and an unmoved HEAD is not reconciled again.
/// Core test case:
/// - `specs/test-cases/server/jotting/jotting-sync.md#only-committed-jotting-states-may-be-published`
#[test]
#[ignore = "requires local postgres:17-alpine and git"]
fn only_commits_are_published() {
    run(async {
        let (_container, database, _pool) = database().await;
        let repository = Repository::new();
        let (a, b) = (Uuid::now_v7(), Uuid::now_v7());
        repository.write(&[("a.md", &file(a, "committed"))]);
        repository.commit("add");
        let jottings = Jottings::open(&repository.folder(), database, SHANGHAI).unwrap();
        indexed(1)(jottings.refresh().await.unwrap());
        repository.write(&[("a.md", &file(a, "half wri")), ("b.md", "---\nid: ")]);
        assert_eq!(jottings.refresh().await.unwrap(), Reconciled::Unchanged);
        assert_eq!(changes(&jottings, 1).await.changes, []);

        repository.write(&[("a.md", &file(a, "done")), ("b.md", &file(b, "new"))]);
        repository.commit("finish");
        indexed(2)(jottings.refresh().await.unwrap());
        assert_eq!(
            changes(&jottings, 1).await.changes,
            [live(2, jotting(a, "done")), live(3, jotting(b, "new"))]
        );
    });
}

/// A commit with any bad file leaves the index alone; the fix catches up in one reconciliation.
/// Core test case:
/// - `specs/test-cases/server/jotting/jotting-sync.md#an-invalid-commit-must-leave-the-index-untouched`
#[test]
#[ignore = "requires local postgres:17-alpine and git"]
fn an_invalid_commit_is_refused_whole() {
    run(async {
        let (_container, database, _pool) = database().await;
        let repository = Repository::new();
        let (a, b) = (Uuid::now_v7(), Uuid::now_v7());
        repository.write(&[("a.md", &file(a, "one"))]);
        repository.commit("add");
        let jottings = Jottings::open(&repository.folder(), database, SHANGHAI).unwrap();
        indexed(1)(jottings.refresh().await.unwrap());

        repository.write(&[
            ("a.md", &file(a, "two")),
            ("bad.md", "---\ncreated: 2026-10-19T08:30:00+08:00\n---\n"),
            ("copy.md", &file(a, "two")),
        ]);
        repository.commit("broken");
        let head = repository.git(&["rev-parse", "HEAD"]);
        match jottings.refresh().await.unwrap() {
            Reconciled::Refused { commit, problems } => {
                assert_eq!(commit.to_string(), head.trim());
                assert_eq!(
                    problems,
                    [
                        JottingProblem::Invalid {
                            path: "bad.md".into(),
                            error: JottingError::Missing("id"),
                        },
                        JottingProblem::Duplicate {
                            path: "copy.md".into(),
                            id: a,
                            first: "a.md".into(),
                        },
                    ]
                );
            }
            other => panic!("expected a refused commit, got {other:?}"),
        }
        assert_eq!(jottings.refresh().await.unwrap(), Reconciled::Unchanged);
        assert_eq!(changes(&jottings, 1).await.changes, []);

        repository.remove("copy.md");
        repository.write(&[("bad.md", &file(b, "fixed"))]);
        repository.commit("fix");
        indexed(2)(jottings.refresh().await.unwrap());
        assert_eq!(
            changes(&jottings, 1).await.changes,
            [live(2, jotting(a, "two")), live(3, jotting(b, "fixed"))]
        );
    });
}

/// Retries of one upload never write twice, revive a tombstone or undo the agent's edit; a
/// different upload under a used id is refused without side effects.
/// Core test cases:
/// - `specs/test-cases/server/jotting/jotting-sync.md#retrying-an-upload-must-never-create-a-second-jotting`
/// - `specs/test-cases/server/jotting/jotting-sync.md#uploading-a-different-jotting-under-an-existing-id-must-be-rejected`
#[test]
#[ignore = "requires local postgres:17-alpine and git"]
fn upload_retries_are_idempotent() {
    run(async {
        let (_container, database, pool) = database().await;
        let repository = Repository::new();
        let (agent, uploaded) = (Uuid::now_v7(), Uuid::now_v7());
        repository.write(&[("agent.md", &file(agent, "by agent"))]);
        repository.commit("add");
        let jottings = Jottings::open(&repository.folder(), database, SHANGHAI).unwrap();
        indexed(1)(jottings.refresh().await.unwrap());

        let upload = jotting(uploaded, "from phone");
        assert_eq!(
            jottings.upload(upload.clone()).await.unwrap(),
            Uploaded::Created(version(2))
        );
        let inbox = repository
            .folder()
            .join("inbox")
            .join(format!("{uploaded}.md"));
        let written = fs::read_to_string(&inbox).unwrap();
        assert_eq!(Jotting::parse(written.as_bytes()).unwrap(), upload);
        assert!(
            written.contains("created: \"2026-10-19T08:30:00+08:00\""),
            "{written}"
        );
        assert_eq!(
            jottings.upload(upload.clone()).await.unwrap(),
            Uploaded::Retried(version(2))
        );

        for conflicting in [jotting(agent, "by agent"), jotting(uploaded, "other")] {
            let before = repository.snapshot();
            assert!(matches!(
                jottings.upload(conflicting).await,
                Err(JottingsError::Conflict)
            ));
            assert_eq!(repository.snapshot(), before);
        }
        assert_eq!(changes(&jottings, 2).await.changes, []);

        repository.write(&[(&format!("inbox/{uploaded}.md"), &file(uploaded, "sorted"))]);
        repository.commit("agent edits the upload");
        indexed(1)(jottings.refresh().await.unwrap());
        assert_eq!(
            jottings.upload(upload.clone()).await.unwrap(),
            Uploaded::Retried(version(3))
        );
        repository.remove(&format!("inbox/{uploaded}.md"));
        repository.commit("agent deletes the upload");
        indexed(1)(jottings.refresh().await.unwrap());
        assert_eq!(
            jottings.upload(upload).await.unwrap(),
            Uploaded::Retried(version(4))
        );
        assert!(!inbox.exists());
        let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM jotting")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(rows, 2);
        let invalid = Jotting {
            title: Some(" ".into()),
            ..jotting(Uuid::now_v7(), "x")
        };
        match jottings.upload(invalid).await {
            Err(JottingsError::Input(error)) => assert_eq!(error.kind, InputErrorKind::Field),
            other => panic!("expected an input error, got {other:?}"),
        }
    });
}

/// A failure after the file is written and before the row is inserted leaves only the file; the
/// retry keeps it and ends with one file and one row.
/// Core test case:
/// - `specs/test-cases/server/jotting/jotting-sync.md#retrying-an-upload-must-never-create-a-second-jotting`
#[test]
#[ignore = "requires local postgres:17-alpine and git"]
fn a_failed_insert_converges_on_retry() {
    run(async {
        let (_container, database, pool) = database().await;
        let repository = Repository::new();
        repository.commit("empty");
        let jottings = Jottings::open(&repository.folder(), database, SHANGHAI).unwrap();
        sqlx::raw_sql(
            "CREATE FUNCTION fail_upload() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected'; END $$;
             CREATE TRIGGER fail_upload BEFORE INSERT ON jotting FOR EACH ROW EXECUTE FUNCTION fail_upload();",
        )
        .execute(&pool)
        .await
        .unwrap();
        let id = Uuid::now_v7();
        assert!(matches!(
            jottings.upload(jotting(id, "x")).await,
            Err(JottingsError::Database(_))
        ));
        let inbox = repository.folder().join("inbox");
        assert_eq!(
            fs::read_dir(&inbox).unwrap().count(),
            1,
            "only the upload's file, no temporary"
        );
        sqlx::raw_sql("DROP TRIGGER fail_upload ON jotting")
            .execute(&pool)
            .await
            .unwrap();
        // The injected trigger fires before the publication trigger, so no version was taken.
        assert_eq!(
            jottings.upload(jotting(id, "x")).await.unwrap(),
            Uploaded::Created(version(1))
        );
        assert_eq!(fs::read_dir(&inbox).unwrap().count(), 1);
        assert_eq!(
            changes(&jottings, 0).await.changes,
            [live(1, jotting(id, "x"))]
        );
    });
}

/// An upload survives unrelated commits, is committed as written without a new version, and
/// becomes a tombstone if its file is deleted before any commit has it.
/// Core test case:
/// - `specs/test-cases/server/jotting/jotting-sync.md#an-uploaded-jotting-must-survive-until-its-commit`
#[test]
#[ignore = "requires local postgres:17-alpine and git"]
fn uploads_wait_for_their_commit() {
    run(async {
        let (_container, database, _pool) = database().await;
        let repository = Repository::new();
        repository.commit("empty");
        let jottings = Jottings::open(&repository.folder(), database, SHANGHAI).unwrap();
        indexed(0)(jottings.refresh().await.unwrap());
        let (kept, dropped, other) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
        jottings.upload(jotting(kept, "kept")).await.unwrap();
        jottings.upload(jotting(dropped, "dropped")).await.unwrap();

        repository.write(&[("other.md", &file(other, "other"))]);
        repository.git(&["add", "jottings/other.md"]);
        repository.git(&["commit", "--quiet", "-m", "unrelated"]);
        indexed(1)(jottings.refresh().await.unwrap());
        assert_eq!(
            changes(&jottings, 2).await.changes,
            [live(3, jotting(other, "other"))]
        );

        repository.remove(&format!("inbox/{dropped}.md"));
        repository.commit("commit the kept upload as written");
        indexed(2)(jottings.refresh().await.unwrap());
        assert_eq!(
            changes(&jottings, 3).await.changes,
            [JottingChange::Deleted {
                id: dropped,
                version: version(4),
            }]
        );
        assert_eq!(
            changes(&jottings, 0).await.changes,
            [
                live(1, jotting(kept, "kept")),
                live(3, jotting(other, "other"))
            ]
        );
    });
}

/// The server's only file-system effect is the upload's inbox file; `.git` is never written.
/// Core test case:
/// - `specs/test-cases/server/jotting/jotting-sync.md#the-server-must-never-write-outside-inbox`
#[test]
#[ignore = "requires local postgres:17-alpine and git"]
fn the_server_writes_only_inbox_files() {
    run(async {
        let (_container, database, _pool) = database().await;
        let repository = Repository::new();
        repository.write(&[("a.md", &file(Uuid::now_v7(), "a"))]);
        repository.commit("add");
        let before = repository.snapshot();
        let jottings = Jottings::open(&repository.folder(), database, SHANGHAI).unwrap();
        indexed(1)(jottings.refresh().await.unwrap());
        let id = Uuid::now_v7();
        jottings.upload(jotting(id, "b")).await.unwrap();
        let mut after = repository.snapshot();
        let added = PathBuf::from("jottings")
            .join("inbox")
            .join(format!("{id}.md"));
        assert!(after.remove(&added).is_some());
        assert_eq!(after, before);
    });
}

/// A first sync leaves tombstones out but its cursor passes them; later pages carry them.
/// Versions follow commit order because a second writer waits for the publication lock.
/// Core test case:
/// - `specs/test-cases/server/jotting/jotting-sync.md#pulling-changes-by-cursor-must-not-skip-any-committed-change`
#[test]
#[ignore = "requires local postgres:17-alpine and git"]
fn the_feed_never_skips_a_committed_change() {
    run(async {
        let (_container, database, pool) = database().await;
        let repository = Repository::new();
        let (a, b, c) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
        repository.write(&[("a.md", &file(a, "a")), ("b.md", &file(b, "b"))]);
        repository.commit("add");
        let jottings =
            Arc::new(Jottings::open(&repository.folder(), database.clone(), SHANGHAI).unwrap());
        indexed(2)(jottings.refresh().await.unwrap());
        repository.remove("a.md");
        repository.commit("delete a");
        indexed(1)(jottings.refresh().await.unwrap());

        // a was published as 1, b as 2, and a's tombstone as 3.
        assert_eq!(
            jottings.changes(version(0), 1).await.unwrap(),
            JottingChanges {
                changes: vec![live(2, jotting(b, "b"))],
                next: version(2),
                has_more: true,
            }
        );
        assert_eq!(
            jottings.changes(version(0), 2).await.unwrap(),
            JottingChanges {
                changes: vec![live(2, jotting(b, "b"))],
                next: version(3),
                has_more: false,
            }
        );
        assert_eq!(
            jottings.changes(version(2), 2).await.unwrap(),
            JottingChanges {
                changes: vec![JottingChange::Deleted {
                    id: a,
                    version: version(3),
                }],
                next: version(3),
                has_more: false,
            }
        );

        // Hold the publication lock, then start an upload that must wait for it.
        let holder = database.begin_jottings().await.unwrap();
        let uploader = jottings.clone();
        let upload = tokio::spawn(async move { uploader.upload(jotting(c, "c")).await });
        loop {
            let waiting: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM pg_locks WHERE locktype='advisory' AND NOT granted",
            )
            .fetch_one(&pool)
            .await
            .unwrap();
            if waiting == 1 {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert_eq!(changes(&jottings, 3).await.changes, []);
        holder.commit().await.unwrap();
        assert_eq!(
            upload.await.unwrap().unwrap(),
            Uploaded::Created(version(4))
        );

        let removed = sqlx::query("DELETE FROM jotting WHERE id=$1")
            .bind(a)
            .execute(&pool)
            .await;
        assert!(removed.is_err(), "tombstones cannot be deleted");
    });
}

/// Folders outside a work tree, detached HEADs and unborn branches fail fast; a subdirectory of
/// a work tree is accepted.
/// Core test case:
/// - `specs/test-cases/server/jotting/jotting-sync.md#jotting-configuration-must-fail-fast-when-misconfigured`
#[test]
#[ignore = "requires local postgres:17-alpine and git"]
fn misconfigured_folders_fail_to_open() {
    run(async {
        let (_container, database, _pool) = database().await;
        let plain = tempfile::tempdir().unwrap();
        assert!(matches!(
            Jottings::open(plain.path(), database.clone(), SHANGHAI),
            Err(OpenError::NotInRepository { .. })
        ));
        let repository = Repository::new();
        assert!(matches!(
            Jottings::open(&repository.folder(), database.clone(), SHANGHAI),
            Err(OpenError::NoCommit { .. })
        ));
        repository.commit("first");
        assert!(Jottings::open(&repository.folder(), database.clone(), SHANGHAI).is_ok());
        repository.git(&["checkout", "--quiet", "--detach"]);
        assert!(matches!(
            Jottings::open(&repository.folder(), database, SHANGHAI),
            Err(OpenError::Detached(_))
        ));
        assert!(!Path::new(&repository.root.join("jottings").join("inbox")).exists());
    });
}

/// A commit wakes the watcher through the branch ref alone; work-tree edits do not.
/// Core test case:
/// - `specs/test-cases/server/jotting/jotting-sync.md#only-committed-jotting-states-may-be-published`
#[test]
#[ignore = "requires local postgres:17-alpine and git"]
fn a_commit_wakes_the_watcher() {
    run(async {
        let (_container, database, _pool) = database().await;
        let repository = Repository::new();
        repository.commit("empty");
        let jottings = Arc::new(Jottings::open(&repository.folder(), database, SHANGHAI).unwrap());
        let mut reconciliations = jottings.reconciliations();
        // The poll's first tick reconciles at once; after that only events can wake it.
        let _watch = watch(jottings.clone(), Duration::from_secs(/*secs*/ 3600)).unwrap();
        let next = |reconciliations: &mut tokio::sync::watch::Receiver<u64>| {
            let mut reconciliations = reconciliations.clone();
            async move {
                tokio::time::timeout(Duration::from_secs(/*secs*/ 10), reconciliations.changed())
                    .await
                    .unwrap()
                    .unwrap();
            }
        };
        next(&mut reconciliations).await;
        reconciliations.mark_unchanged();
        let id = Uuid::now_v7();
        repository.write(&[("a.md", &file(id, "a"))]);
        repository.commit("add");
        next(&mut reconciliations).await;
        assert_eq!(
            changes(&jottings, 0).await.changes,
            [live(1, jotting(id, "a"))]
        );
    });
}
