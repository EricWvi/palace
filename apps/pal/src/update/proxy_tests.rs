//! Child processes isolate proxy environment settings from every other test thread.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use super::*;
use pretty_assertions::assert_eq;
use std::process::Command;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

const CHILD: &str = "PAL_UPDATE_PROXY_TEST_CHILD";

/// Observes CONNECT traffic and bypass behavior without reaching GitHub or replacing a CLI.
/// Core test: specs/test-cases/cli/update.md#updates-must-honor-standard-proxy-environment-variables
#[tokio::test]
async fn standard_proxy_environment_is_used_and_no_proxy_is_respected() {
    if std::env::var_os(CHILD).is_some() {
        tokio::task::spawn_blocking(|| {
            let client = download_client_builder()
                .resolve("pal-update.invalid", "127.0.0.1:1".parse().unwrap())
                .timeout(Duration::from_secs(/*secs*/ 3))
                .build()
                .unwrap();
            assert!(
                Download::get(
                    &client,
                    "https://pal-update.invalid:1/latest.json",
                    /*limit*/ 1024,
                )
                .is_err()
            );
        })
        .await
        .unwrap();
        return;
    }
    for (variable, bypass) in [
        ("HTTPS_PROXY", None),
        ("https_proxy", None),
        ("ALL_PROXY", None),
        ("all_proxy", None),
        ("HTTPS_PROXY", Some("NO_PROXY")),
        ("https_proxy", Some("no_proxy")),
    ] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let proxy_url = format!("http://{}", listener.local_addr().unwrap());
        let child = tokio::task::spawn_blocking(move || {
            let mut command = Command::new(std::env::current_exe().unwrap());
            command
                .args(["--exact", "update::proxy_tests::standard_proxy_environment_is_used_and_no_proxy_is_respected"])
                .env_clear()
                .env(CHILD, "1")
                .env(variable, proxy_url);
            if let Some(variable) = bypass {
                command.env(variable, "pal-update.invalid");
            }
            command.output().unwrap()
        });
        let output = if bypass.is_some() {
            let output = tokio::time::timeout(Duration::from_secs(/*secs*/ 10), child)
                .await
                .unwrap()
                .unwrap();
            let error = listener.into_std().unwrap().accept().unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
            output
        } else {
            let proxy = async {
                let (socket, _) = listener.accept().await.unwrap();
                let mut reader = BufReader::new(socket);
                let mut request = String::new();
                loop {
                    let mut line = String::new();
                    assert!(reader.read_line(&mut line).await.unwrap() > 0);
                    if line == "\r\n" {
                        break;
                    }
                    request.push_str(&line);
                }
                reader.get_mut().write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await.unwrap();
                request
            };
            let (output, request) = tokio::time::timeout(Duration::from_secs(/*secs*/ 10), async {
                tokio::join!(child, proxy)
            })
            .await
            .unwrap();
            assert_eq!(
                request.lines().next(),
                Some("CONNECT pal-update.invalid:1 HTTP/1.1")
            );
            output.unwrap()
        };
        assert!(output.status.success(), "{variable} {bypass:?}: {output:?}");
    }
}
