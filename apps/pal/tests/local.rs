//! `pal` against loopback servers that count requests: what the CLI refuses to send, and that it
//! never sends twice. No containers are needed, so these run in every `task test`.
#![allow(clippy::unwrap_used, clippy::expect_used)]

#[allow(dead_code)]
#[path = "support/pal.rs"]
mod support;

use axum::http::{StatusCode, header::LOCATION};
use pretty_assertions::assert_eq;
use std::sync::atomic::Ordering;
use support::{Output, TOKEN, counting, pal};

const ID: &str = "0199f0a0-0000-7000-8000-000000000000";

/// Core test cases:
/// - `specs/test-cases/cli/media-commands.md#the-access-token-must-reach-only-the-configured-palace-host`
/// - `specs/test-cases/cli/media-commands.md#each-command-must-send-at-most-one-request`
/// - `specs/test-cases/cli/media-commands.md#upload-must-print-exactly-the-stored-media-and-nothing-on-failure`
/// - `specs/test-cases/cli/media-commands.md#delete-must-succeed-only-when-palace-confirms-removal`
#[tokio::test]
async fn local_failures_send_nothing_and_keep_stdout_empty() {
    let (url, hits) = counting(StatusCode::SERVICE_UNAVAILABLE).await;
    let folder = tempfile::tempdir().unwrap();
    let file = folder.path().join("a.png");
    std::fs::write(&file, b"abc").unwrap();
    let file = file.to_str().unwrap();
    let missing = folder.path().join("missing.png");
    let missing = missing.to_str().unwrap();
    let directory = folder.path().to_str().unwrap();
    let valid = [("PAL_URL", url.as_str()), ("PAL_TOKEN", TOKEN)];
    let failed = |stderr: String| Output {
        code: Some(1),
        stdout: String::new(),
        stderr,
    };
    let cases = [
        (
            vec!["s3", "upload", file],
            vec![("PAL_URL", url.as_str())],
            failed("pal: PAL_TOKEN is not set\n".into()),
        ),
        (
            vec!["s3", "upload", file],
            vec![("PAL_URL", url.as_str()), ("PAL_TOKEN", &TOKEN[..20])],
            failed(
                "pal: PAL_TOKEN is not a Palace access token (palace_pat_ followed by 43 characters)\n"
                    .into(),
            ),
        ),
        (
            vec!["s3", "delete", ID],
            vec![("PAL_URL", "http://palace.test"), ("PAL_TOKEN", TOKEN)],
            failed(
                "pal: PAL_URL must use https; plain http is allowed only for loopback hosts\n"
                    .into(),
            ),
        ),
        (
            vec!["s3", "upload", missing],
            valid.to_vec(),
            failed(format!(
                "pal: cannot read {missing}: No such file or directory (os error 2)\n"
            )),
        ),
        (
            vec!["s3", "upload", directory],
            valid.to_vec(),
            failed(format!("pal: cannot read {directory}: not a regular file\n")),
        ),
    ];
    for (args, env, expected) in cases {
        assert_eq!(pal(&args, &env).await, expected, "{args:?}");
    }
    // A malformed id is a usage error: exit 2, before the environment is even consulted.
    let usage = pal(&["s3", "delete", "not-a-uuid"], &valid).await;
    assert_eq!(
        (usage.code, usage.stdout.as_str()),
        (Some(2), ""),
        "{}",
        usage.stderr
    );
    assert!(
        usage.stderr.contains("invalid value 'not-a-uuid'"),
        "{}",
        usage.stderr
    );
    assert_eq!(hits.load(Ordering::SeqCst), 0);

    // An unreachable server: only the upload warns that it may have landed anyway.
    let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let closed_url = format!("http://{}", closed.local_addr().unwrap());
    drop(closed);
    let env = [("PAL_URL", closed_url.as_str()), ("PAL_TOKEN", TOKEN)];
    for (args, hint) in [
        (vec!["s3", "upload", file], true),
        (vec!["s3", "delete", ID], false),
    ] {
        let output = pal(&args, &env).await;
        assert_eq!(
            (output.code, output.stdout.as_str()),
            (Some(1), ""),
            "{args:?}"
        );
        assert!(
            output.stderr.starts_with("pal: cannot reach Palace: "),
            "{}",
            output.stderr
        );
        assert_eq!(
            output
                .stderr
                .contains("the upload may still have completed on the server"),
            hint,
            "{}",
            output.stderr
        );
        assert!(!output.stderr.contains(TOKEN));
    }
}

/// Core test case:
/// - `specs/test-cases/cli/media-commands.md#each-command-must-send-at-most-one-request`
#[tokio::test]
async fn server_failures_are_reported_after_a_single_request() {
    let (url, hits) = counting((
        StatusCode::SERVICE_UNAVAILABLE,
        [("content-type", "application/json")],
        r#"{"error":"media_storage_unavailable","login":""}"#,
    ))
    .await;
    let folder = tempfile::tempdir().unwrap();
    let file = folder.path().join("a.png");
    std::fs::write(&file, b"abc").unwrap();
    let env = [("PAL_URL", url.as_str()), ("PAL_TOKEN", TOKEN)];
    let unavailable = Output {
        code: Some(1),
        stdout: String::new(),
        stderr: "pal: Palace answered 503 Service Unavailable: media_storage_unavailable\n".into(),
    };
    assert_eq!(
        pal(&["s3", "upload", file.to_str().unwrap()], &env).await,
        unavailable
    );
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    assert_eq!(pal(&["s3", "delete", ID], &env).await, unavailable);
    assert_eq!(hits.load(Ordering::SeqCst), 2);
}

/// Core test case:
/// - `specs/test-cases/cli/media-commands.md#the-access-token-must-reach-only-the-configured-palace-host`
#[tokio::test]
async fn redirects_are_reported_and_never_followed() {
    let (target, target_hits) = counting(StatusCode::NO_CONTENT).await;
    let location = format!("{target}/api/media/{ID}");
    let (url, hits) = counting((StatusCode::TEMPORARY_REDIRECT, [(LOCATION, location)])).await;
    let folder = tempfile::tempdir().unwrap();
    let file = folder.path().join("a.png");
    std::fs::write(&file, b"abc").unwrap();
    let env = [("PAL_URL", url.as_str()), ("PAL_TOKEN", TOKEN)];
    let redirected = Output {
        code: Some(1),
        stdout: String::new(),
        stderr: "pal: Palace answered 307 Temporary Redirect\n".into(),
    };
    for args in [
        vec!["s3", "upload", file.to_str().unwrap()],
        vec!["s3", "delete", ID],
    ] {
        assert_eq!(pal(&args, &env).await, redirected, "{args:?}");
    }
    assert_eq!(
        (
            hits.load(Ordering::SeqCst),
            target_hits.load(Ordering::SeqCst)
        ),
        (2, 0)
    );
}
