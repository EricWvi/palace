//! Article directory validation and exact HTTP envelopes observed through the real CLI.
#![allow(clippy::unwrap_used, clippy::expect_used)]
#[path = "support/article.rs"]
mod article;
#[allow(dead_code)]
#[path = "support/pal.rs"]
mod support;

use axum::{
    Router,
    extract::Multipart,
    http::{HeaderMap, StatusCode, header::LOCATION},
    routing::post,
};
use pretty_assertions::assert_eq;
use serde_json::{Value, json};
use std::{
    fs,
    sync::{Arc, Mutex, atomic::Ordering},
};
use support::{Output, TOKEN, counting, pal, serve};

#[derive(Debug, PartialEq)]
struct Part {
    name: String,
    filename: Option<String>,
    content_type: Option<String>,
    bytes: Vec<u8>,
}

/// Only real image addresses change, and the wire representation matches the API exactly.
/// Core test case:
/// - `specs/test-cases/cli/article-upload.md#article-upload-must-send-one-complete-multipart-and-preserve-unrelated-markdown`
#[tokio::test]
async fn sends_one_complete_multipart_without_changing_source() {
    let body = "# 标题\r\n\r\n![assets/a.jpg](<assets/a.jpg> \"assets/a.jpg\")\n![](./assets/b.mp4) ![](./assets/a.jpg)\n`![](assets/not-there.jpg)`\n```md\n![](assets/missing.jpg)\n```\n[链接](assets/a.jpg) ![](https://example.com/a.png)\n![remote][r]\n\n[r]: data:image/png;base64,YQ==\n";
    let converted = "# 标题\r\n\r\n![assets/a.jpg](<media:a.jpg> \"assets/a.jpg\")\n![](media:b.mp4) ![](media:a.jpg)\n`![](assets/not-there.jpg)`\n```md\n![](assets/missing.jpg)\n```\n[链接](assets/a.jpg) ![](https://example.com/a.png)\n![remote][r]\n\n[r]: data:image/png;base64,YQ==\n";
    let directory = article::directory(body);
    let requests = Arc::new(Mutex::new(Vec::new()));
    let recorded = requests.clone();
    let app = Router::new().route(
        "/prefix/api/articles",
        post(move |headers: HeaderMap, mut form: Multipart| {
            let recorded = recorded.clone();
            async move {
                let mut parts = Vec::new();
                while let Some(field) = form.next_field().await.unwrap() {
                    let mut part = Part {
                        name: field.name().unwrap().into(),
                        filename: field.file_name().map(str::to_owned),
                        content_type: field.content_type().map(str::to_owned),
                        bytes: Vec::new(),
                    };
                    part.bytes = field.bytes().await.unwrap().to_vec();
                    parts.push(part);
                }
                recorded
                    .lock()
                    .unwrap()
                    .push((headers["authorization"].to_str().unwrap().to_owned(), parts));
                (StatusCode::CREATED, "{\"id\":\"saved\",\"future\":123}")
            }
        }),
    );
    let url = format!("{}/prefix", serve(app).await);
    let output = pal(
        &article::args(directory.path()),
        &[("PAL_URL", &url), ("PAL_TOKEN", TOKEN)],
    )
    .await;
    assert_eq!(
        output,
        Output {
            code: Some(0),
            stdout: "{\"id\":\"saved\",\"future\":123}\n".into(),
            stderr: String::new()
        }
    );
    let mut fields: Value = serde_json::from_str(article::METADATA).unwrap();
    fields["content"] = json!(converted);
    let mut requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 1);
    let (authorization, mut parts) = requests.pop().unwrap();
    let metadata: Value = serde_json::from_slice(&parts[0].bytes).unwrap();
    assert_eq!(metadata, fields);
    // JSON object field order is not part of the wire contract.
    parts[0].bytes = serde_json::to_vec(&metadata).unwrap();
    assert_eq!(
        (authorization, parts),
        (
            format!("Bearer {TOKEN}"),
            vec![
                Part {
                    name: "article".into(),
                    filename: None,
                    content_type: Some("application/json".into()),
                    bytes: serde_json::to_vec(&fields).unwrap()
                },
                Part {
                    name: "file".into(),
                    filename: Some("a.jpg".into()),
                    content_type: Some("image/jpeg".into()),
                    bytes: b"image".to_vec()
                },
                Part {
                    name: "file".into(),
                    filename: Some("b.mp4".into()),
                    content_type: Some("video/mp4".into()),
                    bytes: b"video".to_vec()
                },
            ]
        )
    );
    assert_eq!(
        fs::read_to_string(directory.path().join("post.md")).unwrap(),
        body
    );
    assert_eq!(
        fs::read_to_string(directory.path().join("article.json")).unwrap(),
        article::METADATA
    );
}

/// Invalid metadata, directory ambiguity and unsupported images never reach a server.
/// Core test case:
/// - `specs/test-cases/cli/article-upload.md#invalid-article-directories-must-fail-before-any-request`
#[tokio::test]
async fn invalid_inputs_send_no_requests() {
    let (url, hits) = counting(StatusCode::CREATED).await;
    let env = [("PAL_URL", url.as_str()), ("PAL_TOKEN", TOKEN)];
    for (field, value) in [
        ("title", json!(" ")),
        ("title", json!("a".repeat(1025))),
        ("url", json!("/relative")),
        ("url", json!(" https://example.com")),
        ("date", json!("2026-02-30")),
        ("date", json!("2026-07-03T12:00:00")),
        ("author", json!(true)),
        ("author", json!("a".repeat(257))),
        ("unknown", json!(1)),
    ] {
        let directory = article::directory("正文");
        let mut metadata: Value = serde_json::from_str(article::METADATA).unwrap();
        metadata[field] = value;
        fs::write(
            directory.path().join("article.json"),
            serde_json::to_vec(&metadata).unwrap(),
        )
        .unwrap();
        let output = pal(&article::args(directory.path()), &env).await;
        assert_eq!(
            (output.code, output.stdout.as_str()),
            (Some(1), ""),
            "{field}: {output:?}"
        );
        assert!(output.stderr.contains("article.json"), "{output:?}");
    }
    for metadata in [
        "{}",
        "[]",
        "{",
        "{\"title\":\"a\",\"title\":\"b\",\"url\":\"https://example.com\",\"date\":\"2026-07-03\"}",
    ] {
        let directory = article::directory("正文");
        fs::write(directory.path().join("article.json"), metadata).unwrap();
        let output = pal(&article::args(directory.path()), &env).await;
        assert_eq!(
            (output.code, output.stdout.as_str()),
            (Some(1), ""),
            "{output:?}"
        );
    }
    for body in [
        " ",
        "![](assets/missing.png)",
        "![](assets/../a.jpg)",
        "![](/etc/passwd)",
        "![](file:///etc/passwd)",
        "![](assets/sub/a.jpg)",
        "![](assets/a.jpg?q=1)",
        "![](assets/a.jpg#x)",
        "![](assets/%61.jpg)",
        "![](<assets/图.jpg>)",
        "![](<assets/a b.jpg>)",
        "![](media:a.jpg)",
        "![](/api/media/123)",
        "![local][r]\n\n[r]: assets/a.jpg\n",
        "![](ftp://example.com/image)",
    ] {
        let directory = article::directory(body);
        let output = pal(&article::args(directory.path()), &env).await;
        assert_eq!(
            (output.code, output.stdout.as_str()),
            (Some(1), ""),
            "{body}: {output:?}"
        );
        assert!(!output.stderr.contains(TOKEN));
    }
    for change in [
        "missing metadata",
        "missing body",
        "multiple bodies",
        "directory media",
        "non UTF-8",
        "oversized",
    ] {
        let directory = article::directory("![](assets/a.jpg)");
        let root = directory.path();
        match change {
            "missing metadata" => fs::remove_file(root.join("article.json")).unwrap(),
            "missing body" => fs::remove_file(root.join("post.md")).unwrap(),
            "multiple bodies" => fs::write(root.join("other.md"), "other").unwrap(),
            "directory media" => {
                fs::remove_file(root.join("assets").join("a.jpg")).unwrap();
                fs::create_dir(root.join("assets").join("a.jpg")).unwrap();
            }
            "non UTF-8" => fs::write(root.join("post.md"), [0xff]).unwrap(),
            "oversized" => {
                fs::write(root.join("post.md"), vec![b'x'; 4 * 1024 * 1024 + 1]).unwrap()
            }
            _ => unreachable!(),
        }
        let output = pal(&article::args(root), &env).await;
        assert_eq!(
            (output.code, output.stdout.as_str()),
            (Some(1), ""),
            "{change}: {output:?}"
        );
    }
    assert_eq!(hits.load(Ordering::SeqCst), 0);
}

/// Links at every selected input position must not turn a directory upload into arbitrary reads.
/// Core test case:
/// - `specs/test-cases/cli/article-upload.md#invalid-article-directories-must-fail-before-any-request`
#[cfg(unix)]
#[tokio::test]
async fn symbolic_links_send_no_requests() {
    let (url, hits) = counting(StatusCode::CREATED).await;
    for relative in ["article.json", "post.md", "assets", "assets/a.jpg"] {
        let source = article::directory("![](assets/a.jpg)");
        let directory = article::directory("![](assets/a.jpg)");
        let link = directory.path().join(relative);
        if link.is_dir() {
            fs::remove_dir_all(&link).unwrap();
        } else {
            fs::remove_file(&link).unwrap();
        }
        std::os::unix::fs::symlink(source.path().join(relative), link).unwrap();
        let output = pal(
            &article::args(directory.path()),
            &[("PAL_URL", &url), ("PAL_TOKEN", TOKEN)],
        )
        .await;
        assert_eq!(
            (output.code, output.stdout.as_str()),
            (Some(1), ""),
            "{relative}: {output:?}"
        );
        assert!(output.stderr.contains("symbolic link"), "{output:?}");
    }
    assert_eq!(hits.load(Ordering::SeqCst), 0);
}

/// All failures keep stdout empty and no request is replayed or redirected.
/// Core test case:
/// - `specs/test-cases/cli/article-upload.md#article-results-and-failures-must-preserve-retry-semantics`
#[tokio::test]
async fn responses_preserve_output_and_retry_semantics() {
    let directory = article::directory("正文");
    // A no-media article must work without an assets directory or author.
    fs::remove_dir_all(directory.path().join("assets")).unwrap();
    fs::write(
        directory.path().join("article.json"),
        r#"{"title":"标题","url":"https://example.com","date":"2026-07-03T12:00:00+08:00"}"#,
    )
    .unwrap();
    let (target, target_hits) = counting(StatusCode::CREATED).await;
    for (status, body, expected) in [
        (
            StatusCode::OK,
            "{\"content\":\"existing body\"}",
            Some("{\"content\":\"existing body\"}\n"),
        ),
        (StatusCode::CREATED, "{\"id\":123}", Some("{\"id\":123}\n")),
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "{\"error\":\"media_storage_unavailable\"}",
            None,
        ),
        (
            StatusCode::UNAUTHORIZED,
            "{\"error\":\"authentication_required\"}",
            None,
        ),
        (StatusCode::TEMPORARY_REDIRECT, "", None),
        (StatusCode::CREATED, "broken JSON", None),
        (StatusCode::NO_CONTENT, "", None),
    ] {
        let (url, hits) = counting((status, [(LOCATION, target.clone())], body)).await;
        let output = pal(
            &article::args(directory.path()),
            &[("PAL_URL", &url), ("PAL_TOKEN", TOKEN)],
        )
        .await;
        if let Some(stdout) = expected {
            assert_eq!(
                output,
                Output {
                    code: Some(0),
                    stdout: stdout.into(),
                    stderr: String::new()
                }
            );
        } else {
            assert_eq!(
                (output.code, output.stdout.as_str()),
                (Some(1), ""),
                "{output:?}"
            );
            assert!(!output.stderr.contains(TOKEN));
            if status == StatusCode::CREATED {
                assert!(
                    output.stderr.contains("rerun with the same original URL"),
                    "{output:?}"
                );
            }
        }
        assert_eq!(hits.load(Ordering::SeqCst), 1);
    }
    assert_eq!(target_hits.load(Ordering::SeqCst), 0);
    let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", closed.local_addr().unwrap());
    drop(closed);
    let output = pal(
        &article::args(directory.path()),
        &[("PAL_URL", &url), ("PAL_TOKEN", TOKEN)],
    )
    .await;
    assert_eq!((output.code, output.stdout.as_str()), (Some(1), ""));
    assert!(
        output.stderr.contains("rerun with the same original URL"),
        "{output:?}"
    );
}

/// A lost success body cannot be reported as either a definite failure or an independent media retry.
/// Core test case:
/// - `specs/test-cases/cli/article-upload.md#article-results-and-failures-must-preserve-retry-semantics`
#[tokio::test]
async fn interrupted_success_body_reports_uncertain_commit_without_retrying() {
    use std::io::{BufRead, Read, Write};
    let directory = article::directory("正文");
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::task::spawn_blocking(move || {
        let (mut stream, _) = listener.accept().unwrap();
        let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
        let mut length = None;
        loop {
            let mut line = String::new();
            reader.read_line(&mut line).unwrap();
            if line == "\r\n" {
                break;
            }
            if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                length = Some(value.trim().parse::<usize>().unwrap());
            }
        }
        let mut body = vec![0; length.unwrap()];
        reader.read_exact(&mut body).unwrap();
        // Consume the entire upload first; only the confirmed-success response is truncated.
        stream
            .write_all(
                b"HTTP/1.1 201 Created\r\nContent-Length: 100\r\nConnection: close\r\n\r\n{\"id\":",
            )
            .unwrap();
    });
    let output = pal(
        &article::args(directory.path()),
        &[("PAL_URL", &url), ("PAL_TOKEN", TOKEN)],
    )
    .await;
    server.await.unwrap();
    assert_eq!((output.code, output.stdout.as_str()), (Some(1), ""));
    assert!(
        output.stderr.contains("rerun with the same original URL"),
        "{output:?}"
    );
    assert!(!output.stderr.contains(TOKEN));
}
