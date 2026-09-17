use std::{fs::File, path::Path};
use testcontainers::{
    ContainerAsync, GenericImage, ImageExt,
    core::{IntoContainerPort, Mount, WaitFor},
    runners::AsyncRunner,
};

/// Keeps the developer endpoint stable while allowing isolated tests to run concurrently.
pub(crate) enum PortBinding {
    Debug,
    #[cfg(test)]
    Random,
}

/// Owns both the container and exclusive access to its persistent development directory.
pub(crate) struct Postgres {
    container: ContainerAsync<GenericImage>,
    pub(crate) url: String,
    _lock: File,
}

impl Postgres {
    /// Reuses database files and publishes the port selected for the calling entry point.
    pub(crate) async fn start(
        directory: &Path,
        binding: PortBinding,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        std::fs::create_dir_all(directory)?;
        let directory = directory.canonicalize()?;
        let lock = File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(directory.join("server.lock"))?;
        lock.try_lock()
            .map_err(|error| format!("PostgreSQL data directory is already in use: {error}"))?;
        let present = std::process::Command::new("docker")
            .args(["image", "inspect", "postgres:17-alpine"])
            .output()?;
        if !present.status.success() {
            return Err(
                "postgres:17-alpine must already exist in the local Docker/Podman engine".into(),
            );
        }
        let request = GenericImage::new("postgres", "17-alpine")
            .with_exposed_port(5432.tcp())
            // The initialization server only listens on a Unix socket. TCP listening identifies
            // the final server on both fresh and previously initialized data directories.
            .with_wait_for(WaitFor::message_on_stderr("listening on IPv4 address"))
            .with_env_var("POSTGRES_PASSWORD", "palace-test")
            .with_env_var("POSTGRES_DB", "palace")
            // A child directory keeps initdb separate from the lock file and leaves the host
            // mount root accessible even after PostgreSQL tightens PGDATA permissions.
            .with_env_var("PGDATA", "/var/lib/postgresql/data/pgdata")
            .with_mount(Mount::bind_mount(
                directory
                    .to_str()
                    .ok_or("PostgreSQL data path must be UTF-8")?,
                "/var/lib/postgresql/data",
            ));
        let request = match binding {
            PortBinding::Debug => request.with_mapped_port(/*host_port*/ 15432, 5432.tcp()),
            #[cfg(test)]
            PortBinding::Random => request,
        };
        let container = request.start().await?;
        let host = container.get_host().await?;
        let port = container.get_host_port_ipv4(/*internal_port*/ 5432).await?;
        let url = format!("postgres://postgres:palace-test@{host}:{port}/palace");
        Ok(Self {
            container,
            url,
            _lock: lock,
        })
    }

    /// Stops PostgreSQL cleanly before removing its container, without deleting the bind mount.
    pub(crate) async fn shutdown(self) -> Result<(), Box<dyn std::error::Error>> {
        let stopped = self.container.stop().await;
        // Even an engine-side stop failure must not skip an explicit removal attempt.
        let removed = self.container.rm().await;
        stopped?;
        removed?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use palace_db::Database;
    use pretty_assertions::assert_eq;

    /// Exercises migrations and durable domain data across entirely different container instances.
    #[tokio::test]
    #[ignore = "requires the existing postgres:17-alpine image and Docker/Podman socket"]
    async fn preserves_data_across_restarts_and_rejects_concurrent_use() {
        let directory = tempfile::tempdir().unwrap();
        let first = Postgres::start(directory.path(), PortBinding::Random)
            .await
            .unwrap();
        assert!(
            Postgres::start(directory.path(), PortBinding::Random)
                .await
                .is_err()
        );
        let database = Database::connect(&first.url).await.unwrap();
        let owner = database
            .resolve_identity("https://test.invalid", "test-owner", "test@example.com")
            .await
            .unwrap();
        drop(database);
        first.shutdown().await.unwrap();

        let second = Postgres::start(directory.path(), PortBinding::Random)
            .await
            .unwrap();
        let database = Database::connect(&second.url).await.unwrap();
        assert_eq!(
            database
                .resolve_identity("https://test.invalid", "test-owner", "test@example.com")
                .await
                .unwrap(),
            owner
        );
        drop(database);
        second.shutdown().await.unwrap();
    }
}
