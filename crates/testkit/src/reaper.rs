//! One Ryuk container per test process, registered through a connection that lives until exit.

use crate::{Error, docker, require_local};
use std::{
    io::{self, Read, Write},
    net::{SocketAddr, TcpStream},
    path::PathBuf,
    sync::LazyLock,
    thread,
    time::Duration,
};
use uuid::Uuid;

/// Must match an image already present locally; tests never pull it.
const RYUK_IMAGE: &str = "testcontainers/ryuk:0.14.0";
pub(crate) const SESSION_LABEL: &str = "org.testcontainers.session-id";
/// Bounds how long the published port may accept connections before Ryuk itself listens.
const HANDSHAKE_ATTEMPTS: u32 = 50;

/// The process-wide registration with Ryuk.
///
/// It lives in a static and is never dropped, so the connection closes only when the operating
/// system tears the process down; that close is the signal Ryuk waits for before reaping.
struct Session {
    id: Uuid,
    _connection: TcpStream,
}

static SESSION: LazyLock<Result<Session, Error>> = LazyLock::new(start);

/// Returns the label value every container of this process must carry to be reaped.
pub(crate) fn session() -> Result<Uuid, Error> {
    SESSION
        .as_ref()
        .map(|session| session.id)
        .map_err(Clone::clone)
}

/// Starts Ryuk and registers this process's session filter with it.
fn start() -> Result<Session, Error> {
    require_local(RYUK_IMAGE)?;
    let socket = docker_socket(std::env::var("DOCKER_HOST").ok().as_deref());
    // `--rm` removes Ryuk once it exits after reaping; testcontainers-rs cannot set AutoRemove.
    let volume = format!("{}:/var/run/docker.sock", socket.display());
    let container = docker(&[
        "run",
        "--detach",
        "--rm",
        "--publish",
        "127.0.0.1::8080",
        "--volume",
        &volume,
        "--label",
        "org.testcontainers.ryuk=true",
        RYUK_IMAGE,
    ])?;
    let address = published_address(&docker(&["port", &container, "8080/tcp"])?)?;
    let id = Uuid::now_v7();
    let filter = format!("label={SESSION_LABEL}={id}\n");
    let register = || -> io::Result<TcpStream> {
        let mut connection = TcpStream::connect_timeout(&address, Duration::from_secs(1))?;
        connection.set_read_timeout(Some(Duration::from_secs(10)))?;
        connection.write_all(filter.as_bytes())?;
        let mut reply = [0; 4];
        connection.read_exact(&mut reply)?;
        if &reply != b"ACK\n" {
            return Err(io::Error::other(format!("unexpected reply {reply:?}")));
        }
        connection.set_read_timeout(None)?;
        Ok(connection)
    };
    let mut last = String::new();
    for _ in 0..HANDSHAKE_ATTEMPTS {
        // Rootless port forwarders accept and then reset connections until Ryuk listens.
        match register() {
            Ok(connection) => {
                return Ok(Session {
                    id,
                    _connection: connection,
                });
            }
            Err(error) => last = error.to_string(),
        }
        thread::sleep(Duration::from_millis(100));
    }
    Err(Error::Handshake(last))
}

/// Locates the host socket Ryuk needs, which differs from the default under rootless Podman.
fn docker_socket(docker_host: Option<&str>) -> PathBuf {
    docker_host
        .and_then(|host| host.strip_prefix("unix://"))
        .map_or_else(|| PathBuf::from("/var/run/docker.sock"), PathBuf::from)
}

/// Picks the first binding `docker port` prints; the request publishes only one.
fn published_address(output: &str) -> Result<SocketAddr, Error> {
    output
        .lines()
        .next()
        .and_then(|line| line.trim().parse().ok())
        .ok_or_else(|| Error::PublishedPort(output.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn docker_socket_follows_unix_docker_host() {
        assert_eq!(
            docker_socket(Some("unix:///run/user/1000/podman/podman.sock")),
            PathBuf::from("/run/user/1000/podman/podman.sock")
        );
    }

    #[test]
    fn docker_socket_falls_back_to_default_path() {
        let default = PathBuf::from("/var/run/docker.sock");
        assert_eq!(docker_socket(/*docker_host*/ None), default);
        assert_eq!(docker_socket(Some("tcp://127.0.0.1:2375")), default);
    }

    #[test]
    fn published_address_reads_first_binding() {
        assert_eq!(
            published_address("127.0.0.1:43261\n[::1]:43261"),
            Ok("127.0.0.1:43261".parse().unwrap())
        );
    }

    #[test]
    fn published_address_rejects_garbage() {
        assert_eq!(
            published_address(""),
            Err(Error::PublishedPort(String::new()))
        );
    }
}
