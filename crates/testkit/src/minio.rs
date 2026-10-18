//! A MinIO server with an empty bucket, for tests that need real S3 semantics such as idempotent
//! deletes and stored content types. In-memory stores only stand in for lower-level rules.
use crate::reaped;
use testcontainers::{
    ContainerAsync, GenericImage, ImageExt,
    core::{IntoContainerPort, WaitFor},
    runners::AsyncRunner,
};

pub const MINIO_ACCESS_KEY: &str = "palace-test";
pub const MINIO_SECRET_KEY: &str = "palace-test-secret";
pub const MINIO_BUCKET: &str = "palace-media";
pub const MINIO_REGION: &str = "us-east-1";
const READY: &str = "palace test bucket ready";

/// A running MinIO; dropping it stops the container.
pub struct Minio {
    pub endpoint: String,
    _container: ContainerAsync<GenericImage>,
}

/// Starts MinIO and creates the test bucket inside the container before reporting ready, so
/// tests never race the server's startup or need an S3 client that can create buckets.
pub async fn minio() -> Result<Minio, Box<dyn std::error::Error>> {
    let script = format!(
        "minio server /data --quiet & \
         until mc alias set local http://127.0.0.1:9000 {MINIO_ACCESS_KEY} {MINIO_SECRET_KEY} >/dev/null 2>&1; do sleep 0.2; done; \
         mc mb --quiet local/{MINIO_BUCKET} >/dev/null && echo '{READY}'; wait"
    );
    let container = reaped(
        GenericImage::new("minio/minio", "RELEASE.2025-02-28T09-55-16Z")
            .with_entrypoint("sh")
            .with_exposed_port(9000.tcp())
            .with_wait_for(WaitFor::message_on_stdout(READY)),
    )?
    .with_cmd(["-c", script.as_str()])
    .with_env_var("MINIO_ROOT_USER", MINIO_ACCESS_KEY)
    .with_env_var("MINIO_ROOT_PASSWORD", MINIO_SECRET_KEY)
    .start()
    .await?;
    let host = container.get_host().await?;
    let port = container.get_host_port_ipv4(/*internal_port*/ 9000).await?;
    Ok(Minio {
        endpoint: format!("http://{host}:{port}"),
        _container: container,
    })
}
