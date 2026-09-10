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
    let port = container.get_host_port_ipv4(5432).await.unwrap();
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
    let tree = db
        .conversation(owner.scope(), c.conversation_id)
        .await
        .unwrap();
    assert_eq!(tree.1.len(), 4);
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
    assert_eq!(counts, (1, 4, 3));
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
        db.authenticate(&session.secret, &key, &valid, 86500),
        db.authenticate(&session.secret, &key, &valid, 86500)
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
    assert!(matches!(
        db.authenticate(&session.secret, &key, &valid, 86531).await,
        Err(SessionError::Unauthorized)
    ));
    let transient = FakeProvider {
        calls: Default::default(),
        outcome: FakeOutcome::Unavailable,
    };
    assert!(matches!(
        db.authenticate(&a.secret, &key, &transient, 172900).await,
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
        .authenticate(&a.secret, &key, &valid, 172900)
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
        172901,
    )
    .await
    .unwrap();
    db.retry_revocations(&key, &transient).await.unwrap();
    assert!(matches!(
        db.authenticate(&recovered.secret, &key, &valid, 172901)
            .await,
        Err(SessionError::Unauthorized)
    ));
    assert!(
        db.authenticate(&second.secret, &key, &valid, 172901)
            .await
            .is_ok()
    );
    db.revoke_sessions(
        second.owner.scope(),
        second.id,
        RevokeScope::AllDevices,
        172902,
    )
    .await
    .unwrap();
    assert!(
        db.authenticate(&second.secret, &key, &valid, 172902)
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
            db.authenticate(&s.secret, &key, &provider, 286400)
                .await
                .is_err()
        );
        assert!(
            db.authenticate(&s.secret, &key, &valid, 286401)
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
        db.authenticate(&before.secret, &key, &valid, 300002)
            .await
            .is_err()
    );
    sqlx::query("UPDATE owner_identity SET disabled=true WHERE id=$1")
        .bind(after.owner.identity_id)
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        db.authenticate(&after.secret, &key, &valid, 300003)
            .await
            .is_err()
    );
}

/// State is single-use, browser-bound and persisted across independent database handles.
#[tokio::test]
#[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
async fn login_state_is_bound_expiring_and_single_use() {
    let (_container, db, _pool) = database().await;
    let key = palace_db::CredentialKey::new([9; 32]);
    db.begin_login("state", "browser", "pkce and nonce", &key, 100)
        .await
        .unwrap();
    assert!(
        db.consume_login("state", "attacker", &key, 101)
            .await
            .is_err()
    );
    assert_eq!(
        db.clone()
            .consume_login("state", "browser", &key, 101)
            .await
            .unwrap(),
        "pkce and nonce"
    );
    assert!(
        db.consume_login("state", "browser", &key, 102)
            .await
            .is_err()
    );
    db.begin_login("expired", "browser", "proof", &key, 100)
        .await
        .unwrap();
    assert!(
        db.consume_login("expired", "browser", &key, 700)
            .await
            .is_err()
    );
}
