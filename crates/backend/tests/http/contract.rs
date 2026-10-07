//! Captures real Router responses before callers consume them; Node validates the current contract.
use axum::{
    Router,
    body::{Body, to_bytes},
    extract::{MatchedPath, Request},
    http::header::CONTENT_ENCODING,
    middleware::{Next, from_fn},
    response::Response,
};
use flate2::read::GzDecoder;
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    path::Path,
    process::{Command, Stdio},
    sync::{Arc, Mutex},
};

#[derive(Clone, Default)]
pub(super) struct Capture(Arc<Mutex<Vec<Value>>>);

impl Capture {
    /// Observes authentication, extractors and handlers without replacing any production behavior.
    pub(super) fn attach(&self, app: Router) -> Router {
        let capture = self.clone();
        app.layer(from_fn(move |request: Request, next: Next| {
            let capture = capture.clone();
            async move {
                let method = request.method().to_string();
                let path = request
                    .extensions()
                    .get::<MatchedPath>()
                    .map(|path| path.as_str().to_owned());
                let response = next.run(request).await;
                let (parts, body) = response.into_parts();
                let bytes = to_bytes(body, usize::MAX).await.unwrap();
                // Existing regressions deliberately probe removed URLs; fallback 404s are not operations.
                let Some(path) = path else {
                    pretty_assertions::assert_eq!((parts.status.as_u16(), bytes.len()), (404, 0));
                    return Response::from_parts(parts, Body::from(bytes));
                };
                let headers: serde_json::Map<String, Value> = parts
                    .headers
                    .iter()
                    .map(|(name, value)| (name.to_string(), json!(value.to_str().unwrap())))
                    .collect();
                // The contract describes the JSON payload, so a gzip-encoded body is validated decoded.
                let mut body = String::new();
                if parts
                    .headers
                    .get(CONTENT_ENCODING)
                    .is_some_and(|value| value == "gzip")
                {
                    GzDecoder::new(&bytes[..])
                        .read_to_string(&mut body)
                        .unwrap();
                } else {
                    body = String::from_utf8(bytes.to_vec()).unwrap();
                }
                capture.0.lock().unwrap().push(json!({
                    "method": method, "path": path, "status": parts.status.as_u16(),
                    "headers": headers, "body": body,
                }));
                Response::from_parts(parts, Body::from(bytes))
            }
        }))
    }

    /// Validates observed wire data against fresh handler schemas, independently of committed artifacts.
    pub(super) fn verify(&self, coverage: Coverage) {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        let mut child = Command::new("node")
            .arg(root.join("scripts").join("check-api-responses.mjs"))
            .stdin(Stdio::piped())
            .spawn()
            .expect("Node and npm ci are required for HTTP contract tests");
        let input = json!({
            "document": palace_backend::api_contract(),
            "samples": *self.0.lock().unwrap(),
            "requireAllOperations": matches!(coverage, Coverage::AllOperations),
        });
        child
            .stdin
            .take()
            .unwrap()
            .write_all(input.to_string().as_bytes())
            .unwrap();
        assert!(
            child.wait().unwrap().success(),
            "real HTTP responses violated the OpenAPI contract"
        );
    }
}

pub(super) enum Coverage {
    AllOperations,
    ObservedOperations,
}
