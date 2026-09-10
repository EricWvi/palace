use palace_db::{Database, DbError};
use palace_domain::{ImportInput, ImportLimits, ImportRequest, Source};
use pretty_assertions::assert_eq;
use sqlx::PgPool;
use testcontainers::{
    ContainerAsync, GenericImage, ImageExt,
    core::{IntoContainerPort, WaitFor},
    runners::AsyncRunner,
};
use uuid::Uuid;

/// Requires the prepared image before testcontainers can attempt its pull-on-missing fallback.
async fn database() -> (ContainerAsync<GenericImage>, Database, PgPool) {
    let present = std::process::Command::new("docker")
        .args(["image", "inspect", "postgres:17-alpine"])
        .output()
        .unwrap();
    assert!(
        present.status.success(),
        "postgres:17-alpine must already exist; image downloads are not allowed"
    );
    let container = GenericImage::new("postgres", "17-alpine")
        .with_exposed_port(5432.tcp())
        .with_wait_for(WaitFor::message_on_stderr(
            "database system is ready to accept connections",
        ))
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
/// Builds independently keyed requests sharing a source identity.
fn request(key: &str, messages: &[&str]) -> ImportRequest {
    let history = serde_json::to_vec(
        &messages
            .iter()
            .map(|content| serde_json::json!({"role":"user","content":content}))
            .collect::<Vec<_>>(),
    )
    .unwrap();
    ImportRequest::parse(
        ImportInput {
            title: key.into(),
            source: Source::Chatgpt,
            session_id: "same".into(),
            history,
            idempotency_key: key.into(),
        },
        ImportLimits::default(),
    )
    .unwrap()
}
/// Verifies real constraints, stable identity and cross-owner queries, including email takeover attempts.
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn identity_and_database_constraints_isolate_owners() {
    let (_container, db, pool) = database().await;
    let a = db
        .resolve_identity("https://idp", "a", "A@example.com")
        .await
        .unwrap();
    let mut changed = a.clone();
    changed.email = "new@example.com".into();
    assert_eq!(
        db.resolve_identity("https://idp", "a", &changed.email)
            .await
            .unwrap(),
        changed
    );
    assert!(matches!(
        db.resolve_identity("https://idp", "other", " NEW@example.com ")
            .await,
        Err(DbError::Conflict)
    ));
    assert!(matches!(
        db.resolve_identity("https://other", "a", "new@example.com")
            .await,
        Err(DbError::Conflict)
    ));
    assert!(db.resolve_identity("https://idp", "a", "").await.is_err());
    assert_eq!(
        db.resolve_identity("https://idp", "a", &changed.email)
            .await
            .unwrap(),
        changed
    );
    let identity_count: i64 = sqlx::query_scalar("SELECT count(*) FROM owner_identity")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(identity_count, 1);
    let b = db
        .resolve_identity("https://idp", "b", "b@example.com")
        .await
        .unwrap();
    let ar = db
        .import_path(a.scope(), &request("a", &["A", "B"]))
        .await
        .unwrap();
    let br = db
        .import_path(b.scope(), &request("b", &["A", "B"]))
        .await
        .unwrap();
    let (mut renamed, messages) = db
        .conversation(a.scope(), ar.conversation_id)
        .await
        .unwrap();
    renamed.title = "新标题".into();
    db.rename_conversation(a.scope(), ar.conversation_id, &renamed.title)
        .await
        .unwrap();
    assert_eq!(
        db.conversation(a.scope(), ar.conversation_id)
            .await
            .unwrap(),
        (renamed, messages)
    );
    assert!(
        db.rename_conversation(b.scope(), ar.conversation_id, "foreign")
            .await
            .is_err()
    );
    assert_ne!(ar.conversation_id, br.conversation_id);
    assert!(matches!(
        db.conversation(a.scope(), br.conversation_id).await,
        Err(DbError::NotFound)
    ));
    assert!(sqlx::query("INSERT INTO message(id,owner_id,conversation_id,role,content) VALUES($1,$2,$3,'user','bad')").bind(Uuid::new_v4()).bind(a.id).bind(br.conversation_id).execute(&pool).await.is_err());
    assert!(
        sqlx::query("UPDATE message SET parent_message_id=id WHERE owner_id=$1")
            .bind(a.id)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE message SET owner_id=$1 WHERE owner_id=$2")
            .bind(b.id)
            .bind(a.id)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("UPDATE message SET parent_message_id=$1 WHERE owner_id=$2")
            .bind(br.head_message_id)
            .bind(a.id)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        sqlx::query("DELETE FROM owner WHERE id=$1")
            .bind(a.id)
            .execute(&pool)
            .await
            .is_err()
    );
    let counts: (i64,i64,i64) = sqlx::query_as("SELECT (SELECT count(*) FROM conversation),(SELECT count(*) FROM message),(SELECT count(*) FROM conversation_import)").fetch_one(&pool).await.unwrap();
    assert_eq!(counts, (2, 4, 2));
}
/// Simultaneous submissions share one prefix; failures after writes roll back every affected table.
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn concurrent_imports_reuse_prefix_and_failures_roll_back() {
    let (_container, db, pool) = database().await;
    let owner = db
        .resolve_identity("https://idp", "a", "a@example.com")
        .await
        .unwrap();
    let abc = request("abc", &["A", "B", "C"]);
    let abd = request("abd", &["A", "B", "D"]);
    let (c, d) = tokio::join!(
        db.import_path(owner.scope(), &abc),
        db.import_path(owner.scope(), &abd)
    );
    let c = c.unwrap();
    let d = d.unwrap();
    assert_eq!(c.conversation_id, d.conversation_id);
    assert_eq!(c.created + d.created, 4);
    assert_eq!(db.import_path(owner.scope(), &abc).await.unwrap(), c);
    assert!(matches!(
        db.import_path(owner.scope(), &request("abc", &["different"]))
            .await,
        Err(DbError::Conflict)
    ));
    let duplicate = db
        .import_path(owner.scope(), &request("duplicate", &["A", "B", "C"]))
        .await
        .unwrap();
    assert_eq!(
        (
            duplicate.head_message_id,
            duplicate.created,
            duplicate.reused
        ),
        (c.head_message_id, 0, 3)
    );
    let another_root = db
        .import_path(owner.scope(), &request("root", &["X", "Y"]))
        .await
        .unwrap();
    assert_eq!(
        (
            another_root.conversation_id,
            another_root.created,
            another_root.reused
        ),
        (c.conversation_id, 2, 0)
    );
    assert_eq!(
        db.path(
            owner.scope(),
            c.conversation_id,
            another_root.head_message_id
        )
        .await
        .unwrap()
        .iter()
        .map(|m| m.content.as_str())
        .collect::<Vec<_>>(),
        vec!["X", "Y"]
    );
    let tree = db
        .conversation(owner.scope(), c.conversation_id)
        .await
        .unwrap();
    assert_eq!(tree.1.len(), 6);
    for (head, content) in [
        (c.head_message_id, vec!["A", "B", "C"]),
        (d.head_message_id, vec!["A", "B", "D"]),
    ] {
        assert_eq!(
            db.path(owner.scope(), c.conversation_id, head)
                .await
                .unwrap()
                .iter()
                .map(|m| m.content.as_str())
                .collect::<Vec<_>>(),
            content
        );
    }
    sqlx::raw_sql("CREATE FUNCTION fail_import() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected'; END $$; CREATE TRIGGER fail_import BEFORE INSERT ON conversation_import FOR EACH ROW EXECUTE FUNCTION fail_import();").execute(&pool).await.unwrap();
    assert!(
        db.import_path(
            owner.scope(),
            &request("failure", &["new-root", "new-child"])
        )
        .await
        .is_err()
    );
    assert_eq!(
        db.conversation(owner.scope(), c.conversation_id)
            .await
            .unwrap(),
        tree
    );
    let counts:(i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM conversation),(SELECT count(*) FROM message),(SELECT count(*) FROM conversation_import)").fetch_one(&pool).await.unwrap();
    assert_eq!(counts, (1, 6, 4));
    let fresh = ImportRequest::parse(
        ImportInput {
            title: "fresh".into(),
            source: Source::Gemini,
            session_id: "fresh".into(),
            idempotency_key: "fresh".into(),
            history: br#"[{"role":"assistant","content":"first"}]"#.to_vec(),
        },
        ImportLimits::default(),
    )
    .unwrap();
    assert!(db.import_path(owner.scope(), &fresh).await.is_err());
    let after:(i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM conversation),(SELECT count(*) FROM message),(SELECT count(*) FROM conversation_import)").fetch_one(&pool).await.unwrap();
    assert_eq!(after, counts);
}

struct FakeProvider {
    calls: std::sync::atomic::AtomicUsize,
    outcome: FakeOutcome,
}
enum FakeOutcome {
    Valid,
    Unavailable,
    Rejected,
    MissingEmail,
}
impl palace_db::IdentityProvider for FakeProvider {
    /// Counts actual refresh exchanges to detect concurrent reuse of a rotating credential.
    async fn refresh(
        &self,
        _credential: &str,
    ) -> Result<palace_db::IdentityTokens, palace_db::ProviderError> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        match self.outcome {
            FakeOutcome::Valid => Ok(palace_db::IdentityTokens {
                issuer: "https://idp".into(),
                subject: "a".into(),
                email: "new@example.com".into(),
                refresh: "rotated".into(),
            }),
            FakeOutcome::MissingEmail => Ok(palace_db::IdentityTokens {
                issuer: "https://idp".into(),
                subject: "a".into(),
                email: String::new(),
                refresh: "rotated".into(),
            }),
            FakeOutcome::Unavailable => Err(palace_db::ProviderError::Unavailable),
            FakeOutcome::Rejected => Err(palace_db::ProviderError::Rejected),
        }
    }
    /// Simulates an outage so the test proves local invalidation survives external failure.
    async fn revoke(&self, _credential: &str) -> Result<(), palace_db::ProviderError> {
        Err(palace_db::ProviderError::Unavailable)
    }
}
/// Creates provider results without involving live accounts or production credentials.
fn tokens() -> palace_db::IdentityTokens {
    palace_db::IdentityTokens {
        issuer: "https://idp".into(),
        subject: "a".into(),
        email: "a@example.com".into(),
        refresh: "initial".into(),
    }
}
/// Tests persisted restart recovery, serialized refresh, bounded rotation and local-first revocation.
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn persistent_sessions_revalidate_rotate_and_revoke() {
    use palace_db::{CredentialKey, RevokeScope, SessionError};
    let (_container, db, pool) = database().await;
    let key = CredentialKey::new([1; 32]);
    let valid = FakeProvider {
        calls: Default::default(),
        outcome: FakeOutcome::Valid,
    };
    let session = db
        .create_session(tokens(), &key, /*now*/ 100, /*previous*/ None)
        .await
        .unwrap();
    let stored: (Vec<u8>, Vec<u8>) =
        sqlx::query_as("SELECT secret_hash,refresh_credential FROM owner_session WHERE id=$1")
            .bind(session.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_ne!(stored.0, session.secret.as_bytes());
    assert_ne!(stored.1, b"initial");
    let (a, b) = tokio::join!(
        db.authenticate(&session.secret, &key, &valid, /*now*/ 86500),
        db.authenticate(&session.secret, &key, &valid, /*now*/ 86500)
    );
    let a = a.unwrap();
    let b = b.unwrap();
    assert_eq!(
        (a.id, a.owner.clone(), a.secret.clone()),
        (b.id, b.owner, b.secret)
    );
    assert_eq!(valid.calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    assert_eq!(a.id, session.id);
    assert_eq!(a.owner.id, session.owner.id);
    assert_ne!(a.secret, session.secret);
    let transient = FakeProvider {
        calls: Default::default(),
        outcome: FakeOutcome::Unavailable,
    };
    assert!(matches!(
        db.authenticate(&a.secret, &key, &transient, /*now*/ 172900)
            .await,
        Err(SessionError::Unavailable)
    ));
    let revoked: Option<i64> =
        sqlx::query_scalar("SELECT revoked_at FROM owner_session WHERE id=$1")
            .bind(a.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(revoked, None);
    let recovered = db
        .authenticate(&a.secret, &key, &valid, /*now*/ 172900)
        .await
        .unwrap();
    let second = db
        .create_session(tokens(), &key, /*now*/ 172900, /*previous*/ None)
        .await
        .unwrap();
    db.revoke_sessions(
        recovered.owner.scope(),
        recovered.id,
        RevokeScope::Current,
        /*now*/ 172901,
    )
    .await
    .unwrap();
    db.retry_revocations(&key, &transient).await.unwrap();
    assert!(matches!(
        db.authenticate(&recovered.secret, &key, &valid, /*now*/ 172901)
            .await,
        Err(SessionError::Unauthorized)
    ));
    assert!(
        db.authenticate(&second.secret, &key, &valid, /*now*/ 172901)
            .await
            .is_ok()
    );
    db.revoke_sessions(
        second.owner.scope(),
        second.id,
        RevokeScope::AllDevices,
        /*now*/ 172902,
    )
    .await
    .unwrap();
    assert!(
        db.authenticate(&second.secret, &key, &valid, /*now*/ 172902)
            .await
            .is_err()
    );
    let pending: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM owner_session WHERE revocation_pending AND revoked_at IS NOT NULL",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(pending, 2);
    for outcome in [FakeOutcome::Rejected, FakeOutcome::MissingEmail] {
        let s = db
            .create_session(tokens(), &key, /*now*/ 200000, /*previous*/ None)
            .await
            .unwrap();
        let provider = FakeProvider {
            calls: Default::default(),
            outcome,
        };
        assert!(
            db.authenticate(&s.secret, &key, &provider, /*now*/ 286400)
                .await
                .is_err()
        );
        assert!(
            db.authenticate(&s.secret, &key, &valid, /*now*/ 286401)
                .await
                .is_err()
        );
    }
    let before = db
        .create_session(tokens(), &key, /*now*/ 300000, /*previous*/ None)
        .await
        .unwrap();
    let after = db
        .create_session(tokens(), &key, /*now*/ 300001, Some(&before.secret))
        .await
        .unwrap();
    assert!(
        db.authenticate(&before.secret, &key, &valid, /*now*/ 300002)
            .await
            .is_err()
    );
    sqlx::query("UPDATE owner_identity SET disabled=true WHERE id=$1")
        .bind(after.owner.identity_id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        db.authenticate(&after.secret, &key, &valid, /*now*/ 300003)
            .await
            .is_err()
    );
    let sequence: (i64, bool) =
        sqlx::query_as("SELECT last_value,is_called FROM business_server_version")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(sequence, (1, false));
}

/// State is single-use, browser-bound and persisted across independent database handles.
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn login_state_is_bound_expiring_and_single_use() {
    let (_container, db, _pool) = database().await;
    let key = palace_db::CredentialKey::new([9; 32]);
    db.begin_login("state", "browser", "pkce and nonce", &key, /*now*/ 100)
        .await
        .unwrap();
    assert!(
        db.consume_login("state", "attacker", &key, /*now*/ 101)
            .await
            .is_err()
    );
    assert_eq!(
        db.clone()
            .consume_login("state", "browser", &key, /*now*/ 101)
            .await
            .unwrap(),
        "pkce and nonce"
    );
    assert!(
        db.consume_login("state", "browser", &key, /*now*/ 102)
            .await
            .is_err()
    );
    db.begin_login("expired", "browser", "proof", &key, /*now*/ 100)
        .await
        .unwrap();
    assert!(
        db.consume_login("expired", "browser", &key, /*now*/ 700)
            .await
            .is_err()
    );
}

/// Uses the database lock graph to order concurrent commits without sleeping or assuming scheduler timing.
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn sync_publication_preserves_commit_order_lww_and_owner_scope() {
    use palace_domain::{Record, ServerVersion, UploadResult};
    let (_container, db, pool) = database().await;
    let a = db
        .resolve_identity("https://idp", "a", "a@example.com")
        .await
        .unwrap();
    let b = db
        .resolve_identity("https://idp", "b", "b@example.com")
        .await
        .unwrap();
    let record = Record {
        id: Uuid::new_v4(),
        updated_at: 1000,
        is_deleted: false,
        body: serde_json::json!({"title":"first","text":"body"}),
    };
    let first = db
        .upload_records(a.scope(), std::slice::from_ref(&record))
        .await
        .unwrap();
    let UploadResult::Accepted(first) = &first[0] else {
        panic!("new record must be accepted")
    };
    for timestamp in [999, 1000] {
        let stale = Record {
            updated_at: timestamp,
            body: serde_json::json!({"title":"other"}),
            ..record.clone()
        };
        assert_eq!(
            db.upload_records(a.scope(), &[stale]).await.unwrap(),
            vec![UploadResult::Retained(first.clone())]
        );
    }
    let sequence: i64 = sqlx::query_scalar("SELECT last_value FROM business_server_version")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(sequence, first.server_version.value());
    let foreign = Record {
        id: Uuid::new_v4(),
        ..record.clone()
    };
    db.upload_records(b.scope(), std::slice::from_ref(&foreign))
        .await
        .unwrap();
    let own_new = Record {
        id: Uuid::new_v4(),
        ..record.clone()
    };
    assert!(
        db.upload_records(
            a.scope(),
            &[
                own_new,
                Record {
                    updated_at: 999999,
                    ..foreign
                }
            ]
        )
        .await
        .is_err()
    );
    assert_eq!(
        db.pull_records(a.scope(), ServerVersion::default(), /*limit*/ 100)
            .await
            .unwrap()
            .records,
        vec![first.clone()]
    );
    let mut transaction = pool.begin().await.unwrap();
    let pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(&mut *transaction)
        .await
        .unwrap();
    let held_version:i64=sqlx::query_scalar("INSERT INTO sync_record(id,owner_id,updated_at,is_deleted,body) VALUES($1,$2,1000,false,'{}') RETURNING server_version").bind(Uuid::new_v4()).bind(a.id).fetch_one(&mut *transaction).await.unwrap();
    let next = Record {
        id: Uuid::new_v4(),
        ..record.clone()
    };
    let writer = db.clone();
    let scope = a.scope();
    let task = tokio::spawn(async move { writer.upload_records(scope, &[next]).await.unwrap() });
    tokio::time::timeout(std::time::Duration::from_secs(/*secs*/ 10), async {
        loop {
            let blocked: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE $1=ANY(pg_blocking_pids(pid)))",
            )
            .bind(pid)
            .fetch_one(&pool)
            .await
            .unwrap();
            if blocked {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let before = db
        .pull_records(a.scope(), first.server_version, /*limit*/ 100)
        .await
        .unwrap();
    assert!(before.records.is_empty());
    assert_eq!(before.cursor, first.server_version);
    transaction.commit().await.unwrap();
    let results = task.await.unwrap();
    let UploadResult::Accepted(next) = &results[0] else {
        panic!("expected accepted")
    };
    assert!(next.server_version.value() > held_version);
    let page = db
        .pull_records(a.scope(), first.server_version, /*limit*/ 1)
        .await
        .unwrap();
    assert_eq!(page.cursor.value(), held_version);
    let following = db
        .pull_records(a.scope(), page.cursor, /*limit*/ 1)
        .await
        .unwrap();
    assert_eq!(following.records, vec![next.clone()]);
    let mut rolled_back = pool.begin().await.unwrap();
    let gap:i64=sqlx::query_scalar("INSERT INTO sync_record(id,owner_id,updated_at,is_deleted,body) VALUES($1,$2,1000,false,'{}') RETURNING server_version").bind(Uuid::new_v4()).bind(a.id).fetch_one(&mut *rolled_back).await.unwrap();
    rolled_back.rollback().await.unwrap();
    let tombstone = Record {
        updated_at: 1001,
        is_deleted: true,
        body: serde_json::Value::Null,
        ..record.clone()
    };
    db.upload_records(a.scope(), std::slice::from_ref(&tombstone))
        .await
        .unwrap();
    let page = db
        .pull_records(a.scope(), following.cursor, /*limit*/ 100)
        .await
        .unwrap();
    assert_eq!(page.records[0].record, tombstone);
    assert!(page.cursor.value() > gap);
    assert!(
        sqlx::query("DELETE FROM sync_record WHERE id=$1")
            .bind(record.id)
            .execute(&pool)
            .await
            .is_err()
    );
    let empty = db
        .pull_records(a.scope(), page.cursor, /*limit*/ 100)
        .await
        .unwrap();
    assert_eq!(empty.records, Vec::new());
    assert_eq!(empty.cursor, page.cursor);
}

/// Revocation remains durable through direct writes, damaged rotation state and provider outages.
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn session_integrity_and_revocation_cannot_be_bypassed() {
    let (_container, db, pool) = database().await;
    let key = palace_db::CredentialKey::new([8; 32]);
    let provider = FakeProvider {
        calls: Default::default(),
        outcome: FakeOutcome::Unavailable,
    };
    let mut empty = tokens();
    empty.refresh.clear();
    assert!(
        db.create_session(empty, &key, /*now*/ 100, /*previous*/ None)
            .await
            .is_err()
    );
    let session = db
        .create_session(tokens(), &key, /*now*/ 100, /*previous*/ None)
        .await
        .unwrap();
    // Local logout intentionally succeeds beyond the verification deadline while Authelia is unavailable.
    db.logout_browser(
        &session.secret,
        palace_db::RevokeScope::Current,
        /*now*/ 100000,
    )
    .await
    .unwrap();
    assert!(
        sqlx::query("UPDATE owner_session SET revoked_at=NULL WHERE id=$1")
            .bind(session.id)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(
        db.authenticate(&session.secret, &key, &provider, /*now*/ 100001)
            .await
            .is_err()
    );
    let damaged = db
        .create_session(tokens(), &key, /*now*/ 100, /*previous*/ None)
        .await
        .unwrap();
    sqlx::query("UPDATE owner_session SET generation=generation+1 WHERE id=$1")
        .bind(damaged.id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        db.authenticate(&damaged.secret, &key, &provider, /*now*/ 101)
            .await
            .is_err()
    );
    let revoked: Option<i64> =
        sqlx::query_scalar("SELECT revoked_at FROM owner_session WHERE id=$1")
            .bind(damaged.id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(revoked, Some(101));
}

/// Recognized retired secrets revoke their session after the explicit concurrency grace ends.
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn retired_session_secret_replay_is_revoked() {
    let (_container, db, pool) = database().await;
    let key = palace_db::CredentialKey::new([8; 32]);
    let provider = FakeProvider {
        calls: Default::default(),
        outcome: FakeOutcome::Valid,
    };
    let session = db
        .create_session(tokens(), &key, /*now*/ 100, /*previous*/ None)
        .await
        .unwrap();
    let rotated = db
        .authenticate(&session.secret, &key, &provider, /*now*/ 86500)
        .await
        .unwrap();
    assert!(
        db.authenticate(&session.secret, &key, &provider, /*now*/ 86530)
            .await
            .is_err()
    );
    assert!(
        db.authenticate(&rotated.secret, &key, &provider, /*now*/ 86531)
            .await
            .is_err()
    );
    let reason: String = sqlx::query_scalar("SELECT revoke_reason FROM owner_session WHERE id=$1")
        .bind(session.id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(reason, "retired secret replay");
}

/// Rejects owner/conversation/head mismatches and multi-row cycles independently of application checks.
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn all_scoped_references_and_multirow_cycles_are_rejected() {
    let (_container, db, pool) = database().await;
    let a = db
        .resolve_identity("https://idp", "a", "a@example.com")
        .await
        .unwrap();
    let b = db
        .resolve_identity("https://idp", "b", "b@example.com")
        .await
        .unwrap();
    let ar = db
        .import_path(a.scope(), &request("a", &["A", "B"]))
        .await
        .unwrap();
    let br = db
        .import_path(b.scope(), &request("b", &["A"]))
        .await
        .unwrap();
    assert!(
        sqlx::query("UPDATE conversation_import SET head_message_id=$1 WHERE id=$2")
            .bind(br.head_message_id)
            .bind(ar.import_id)
            .execute(&pool)
            .await
            .is_err()
    );
    let other = Uuid::new_v4();
    sqlx::query("INSERT INTO conversation(id,owner_id,title,source,session_id) VALUES($1,$2,'other','grok','other')").bind(other).bind(a.id).execute(&pool).await.unwrap();
    assert!(sqlx::query("INSERT INTO message(id,owner_id,conversation_id,parent_message_id,role,content) VALUES($1,$2,$3,$4,'user','bad')").bind(Uuid::new_v4()).bind(a.id).bind(other).bind(ar.head_message_id).execute(&pool).await.is_err());
    let x = Uuid::new_v4();
    let y = Uuid::new_v4();
    assert!(sqlx::query("INSERT INTO message(id,owner_id,conversation_id,parent_message_id,role,content) VALUES($1,$3,$4,$2,'user','x'),($2,$3,$4,$1,'assistant','y')").bind(x).bind(y).bind(a.id).bind(other).execute(&pool).await.is_err());
    let nullable:Vec<String>=sqlx::query_scalar("SELECT table_name FROM information_schema.columns WHERE table_schema='public' AND column_name='owner_id' AND is_nullable='YES'").fetch_all(&pool).await.unwrap();
    assert_eq!(nullable, Vec::<String>::new());
    // Identity removal has no cascade to knowledge records; sessions would separately restrict an unsafe unlink.
    sqlx::query("DELETE FROM owner_identity WHERE id=$1")
        .bind(a.identity_id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        db.path(a.scope(), ar.conversation_id, ar.head_message_id)
            .await
            .unwrap()
            .iter()
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>(),
        vec!["A", "B"]
    );
}

/// A failing identity insert cannot leave an orphan Owner; concurrent first login resolves one pair.
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn owner_allocation_is_atomic_and_conflicts_preserve_existing_knowledge() {
    let (_container, db, pool) = database().await;
    sqlx::raw_sql("CREATE FUNCTION fail_identity() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected'; END $$; CREATE TRIGGER fail_identity BEFORE INSERT ON owner_identity FOR EACH ROW EXECUTE FUNCTION fail_identity();").execute(&pool).await.unwrap();
    assert!(
        db.resolve_identity("https://idp", "a", "a@example.com")
            .await
            .is_err()
    );
    let counts: (i64, i64) =
        sqlx::query_as("SELECT (SELECT count(*) FROM owner),(SELECT count(*) FROM owner_identity)")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(counts, (0, 0));
    sqlx::query("DROP TRIGGER fail_identity ON owner_identity")
        .execute(&pool)
        .await
        .unwrap();
    let (first, second) = tokio::join!(
        db.resolve_identity("https://idp", "a", "a@example.com"),
        db.resolve_identity("https://idp", "a", "a@example.com")
    );
    let owner = first.unwrap();
    assert_eq!(second.unwrap(), owner);
    let imported = db
        .import_path(owner.scope(), &request("first", &["A", "B"]))
        .await
        .unwrap();
    let before = db
        .conversation(owner.scope(), imported.conversation_id)
        .await
        .unwrap();
    assert!(
        db.resolve_identity("https://idp", "unknown", "a@example.com")
            .await
            .is_err()
    );
    assert_eq!(
        db.conversation(owner.scope(), imported.conversation_id)
            .await
            .unwrap(),
        before
    );
    assert_eq!(
        db.resolve_identity("https://idp", "a", "a@example.com")
            .await
            .unwrap(),
        owner
    );
}
