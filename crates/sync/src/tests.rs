use super::*;
use palace_domain::{PublishedRecord, Record, ServerVersion, SyncPage, UploadResult};
use pretty_assertions::assert_eq;
use uuid::Uuid;

/// Builds a complete business record for temporal race scenarios.
fn record(id: Uuid, timestamp: i64, title: &str) -> Record {
    Record {
        id,
        updated_at: timestamp,
        is_deleted: false,
        body: serde_json::json!({"title":title}),
    }
}
/// Publishes a complete authoritative state with an independent sequence cursor.
fn published(owner: Uuid, version: i64, record: Record) -> PublishedRecord {
    PublishedRecord {
        owner_id: owner,
        server_version: ServerVersion::new(version).unwrap(),
        record,
    }
}
/// Protects both same-millisecond edits and clock rollback/recreation from stale upload confirmation.
#[test]
fn acknowledgements_preserve_new_mutations_and_tombstones_prevent_resurrection() {
    let owner = Uuid::new_v4();
    let id = Uuid::new_v4();
    let mut replica =
        Replica::from_connection(rusqlite::Connection::open_in_memory().unwrap(), owner).unwrap();
    let old = replica.edit(record(id, /*timestamp*/ 1000, "old")).unwrap();
    let newer = replica
        .edit(record(id, /*timestamp*/ 1000, "same millisecond"))
        .unwrap();
    replica
        .acknowledge(
            &old,
            &UploadResult::Accepted(published(owner, /*version*/ 1, old.record.clone())),
        )
        .unwrap();
    assert_eq!(
        replica.record(id).unwrap(),
        Some(LocalRecord {
            mutation: newer.clone(),
            pending: true
        })
    );
    let tombstone = published(
        owner,
        /*version*/ 2,
        Record {
            updated_at: 999,
            is_deleted: true,
            ..old.record.clone()
        },
    );
    replica
        .apply_page(&SyncPage {
            records: vec![tombstone.clone()],
            cursor: tombstone.server_version,
        })
        .unwrap();
    assert_eq!(replica.record(id).unwrap(), None);
    assert_eq!(replica.pending().unwrap(), Vec::new());
    assert_eq!(replica.derived_jobs().unwrap(), Vec::<String>::new());
    replica
        .acknowledge(
            &old,
            &UploadResult::Accepted(published(owner, /*version*/ 1, old.record.clone())),
        )
        .unwrap();
    assert_eq!(replica.record(id).unwrap(), None);
    let recreated = replica
        .edit(record(id, /*timestamp*/ 1000, "recreated"))
        .unwrap();
    assert!(recreated.generation > newer.generation);
    replica
        .acknowledge(
            &old,
            &UploadResult::Accepted(published(owner, /*version*/ 1, old.record.clone())),
        )
        .unwrap();
    assert_eq!(
        replica.record(id).unwrap(),
        Some(LocalRecord {
            mutation: recreated,
            pending: true
        })
    );
    let newer = replica
        .edit(record(id, /*timestamp*/ 1001, "newer"))
        .unwrap();
    replica
        .acknowledge(
            &old,
            &UploadResult::Retained(published(owner, /*version*/ 1, old.record.clone())),
        )
        .unwrap();
    assert_eq!(replica.pending().unwrap(), vec![newer]);
}
/// Reopens disk storage after an injected derived-job failure and proves cursor/data atomicity and owner isolation.
#[test]
fn pages_commit_records_jobs_and_cursor_together_and_survive_restart() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("replica.sqlite");
    let owner = Uuid::new_v4();
    let other = Uuid::new_v4();
    let id = Uuid::new_v4();
    let page = SyncPage {
        records: vec![published(
            owner,
            /*version*/ 7,
            record(id, /*timestamp*/ 1000, "remote"),
        )],
        cursor: ServerVersion::new(/*value*/ 7).unwrap(),
    };
    {
        let mut replica = Replica::open(&path, owner).unwrap();
        replica.connection.execute_batch("CREATE TRIGGER fail_job BEFORE INSERT ON derived_job BEGIN SELECT RAISE(ABORT,'injected'); END;").unwrap();
        assert!(replica.apply_page(&page).is_err());
        assert!(
            replica
                .edit(record(id, /*timestamp*/ 900, "failed edit"))
                .is_err()
        );
        assert_eq!(replica.pending().unwrap(), Vec::<Mutation>::new());
        assert_eq!(
            (
                replica.record(id).unwrap(),
                replica.cursor().unwrap(),
                replica.derived_jobs().unwrap()
            ),
            (None, ServerVersion::default(), Vec::new())
        );
        replica
            .connection
            .execute_batch("DROP TRIGGER fail_job")
            .unwrap();
        replica.apply_page(&page).unwrap();
    }
    let mut replica = Replica::open(&path, owner).unwrap();
    replica.apply_page(&page).unwrap();
    assert_eq!(replica.cursor().unwrap(), page.cursor);
    assert_eq!(
        replica.record(id).unwrap().unwrap().mutation.record,
        page.records[0].record
    );
    assert_eq!(replica.derived_jobs().unwrap(), vec![id.to_string()]);
    let mut another = Replica::open(&path, other).unwrap();
    assert!(
        another
            .connection
            .execute(
                "INSERT INTO derived_job(owner_id,id) VALUES(?1,?2)",
                rusqlite::params![other.to_string(), id.to_string()]
            )
            .is_err()
    );
    assert_eq!(another.derived_jobs().unwrap(), Vec::<String>::new());
    assert_eq!(
        (another.record(id).unwrap(), another.cursor().unwrap()),
        (None, ServerVersion::default())
    );
    replica
        .apply_page(&SyncPage {
            records: Vec::new(),
            cursor: page.cursor,
        })
        .unwrap();
    assert_eq!(replica.cursor().unwrap(), page.cursor);
    let ignored = published(
        owner,
        /*version*/ 8,
        record(id, /*timestamp*/ 999, "older"),
    );
    replica
        .apply_page(&SyncPage {
            records: vec![ignored],
            cursor: ServerVersion::new(/*value*/ 8).unwrap(),
        })
        .unwrap();
    assert_eq!(
        replica.record(id).unwrap().unwrap().mutation.record,
        page.records[0].record
    );
    let foreign = SyncPage {
        records: vec![published(
            other,
            /*version*/ 9,
            record(id, /*timestamp*/ 1001, "foreign"),
        )],
        cursor: ServerVersion::new(/*value*/ 9).unwrap(),
    };
    assert!(replica.apply_page(&foreign).is_err());
    assert_eq!(replica.cursor().unwrap().value(), 8);
    let independent = another
        .edit(record(id, /*timestamp*/ 2000, "other owner pending"))
        .unwrap();
    let tombstone = published(
        owner,
        /*version*/ 9,
        Record {
            is_deleted: true,
            ..record(id, /*timestamp*/ 1001, "deleted")
        },
    );
    replica
        .apply_page(&SyncPage {
            records: vec![tombstone],
            cursor: ServerVersion::new(9).unwrap(),
        })
        .unwrap();
    assert_eq!(replica.record(id).unwrap(), None);
    assert_eq!(
        (
            another.pending().unwrap(),
            another.cursor().unwrap(),
            another.derived_jobs().unwrap()
        ),
        (
            vec![independent],
            ServerVersion::default(),
            vec![id.to_string()]
        )
    );
}
struct OfflineUpload {
    calls: std::sync::Mutex<Vec<&'static str>>,
}
impl SyncTransport for OfflineUpload {
    /// Leaves pending data untouched to simulate an indeterminate upload outcome.
    async fn upload(&self, _: Vec<Record>) -> Result<Vec<UploadResult>, SyncError> {
        self.calls.lock().unwrap().push("upload");
        Err(SyncError::Transport)
    }
    /// Proves a failed upload cannot starve the consumption phase.
    async fn pull(&self, cursor: ServerVersion) -> Result<SyncPage, SyncError> {
        self.calls.lock().unwrap().push("pull");
        Ok(SyncPage {
            records: Vec::new(),
            cursor,
        })
    }
}
/// A finite failed upload still reaches pull in the same round.
#[tokio::test]
async fn failed_upload_does_not_block_pull() {
    let owner = Uuid::new_v4();
    let replica =
        Replica::from_connection(rusqlite::Connection::open_in_memory().unwrap(), owner).unwrap();
    let client = SyncClient::new(replica);
    client
        .edit(record(Uuid::new_v4(), /*timestamp*/ 1000, "local"))
        .unwrap();
    let transport = OfflineUpload {
        calls: Default::default(),
    };
    assert_eq!(
        client.synchronize(&transport).await.unwrap(),
        RoundResult {
            acknowledged: 0,
            pulled: 0,
            upload_failed: true
        }
    );
    assert_eq!(*transport.calls.lock().unwrap(), vec!["upload", "pull"]);
}
