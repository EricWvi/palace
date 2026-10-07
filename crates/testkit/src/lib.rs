//! Container support shared by Palace integration and contract tests.
//!
//! testcontainers-rs has no resource reaper: a container is removed only when its handle is
//! dropped, so a test process killed by a signal leaks every container it started. This crate
//! labels each test container with a per-process session that a Ryuk container watches, and
//! Ryuk removes them once the process's connection closes, however the process ends.
//!
//! Tests never download images. Every image, including Ryuk itself, must already exist in the
//! local Docker/Podman engine selected by `DOCKER_HOST`.

mod reaper;

use std::process::Command;
use testcontainers::{ContainerRequest, GenericImage, Image, ImageExt};

/// Setup failures surfaced to the test that requested a container.
///
/// Cloneable because the process-wide Ryuk session caches its startup result for every test.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Error {
    #[error("{image} must already exist locally; image downloads are not allowed: {stderr}")]
    MissingImage { image: String, stderr: String },
    #[error("`docker {args}` failed: {stderr}")]
    Docker { args: String, stderr: String },
    #[error("Ryuk returned an unusable published port: {0:?}")]
    PublishedPort(String),
    #[error("Ryuk did not acknowledge the session filter: {0}")]
    Handshake(String),
}

/// Prepares a locally present image so Ryuk removes its container after this test process exits.
pub fn reaped(image: GenericImage) -> Result<ContainerRequest<GenericImage>, Error> {
    require_local(&format!("{}:{}", image.name(), image.tag()))?;
    let session = reaper::session()?;
    Ok(image.with_label(reaper::SESSION_LABEL, session.to_string()))
}

/// Fails before testcontainers or `docker run` can fall back to pulling a missing image.
fn require_local(image: &str) -> Result<(), Error> {
    docker(&["image", "inspect", "--format", "{{.Id}}", image])
        .map(drop)
        .map_err(|error| Error::MissingImage {
            image: image.into(),
            stderr: error.to_string(),
        })
}

/// Runs the Docker CLI against the engine named by `DOCKER_HOST` and returns its trimmed stdout.
fn docker(args: &[&str]) -> Result<String, Error> {
    let failed = |stderr: String| Error::Docker {
        args: args.join(" "),
        stderr,
    };
    let output = Command::new("docker")
        .args(args)
        .output()
        .map_err(|error| failed(error.to_string()))?;
    if !output.status.success() {
        return Err(failed(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}
