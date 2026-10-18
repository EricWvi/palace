//! Runs the real `pal` binary and observes it the way a script would: exit code, stdout, stderr.
use axum::{Router, body::Bytes, response::IntoResponse};
use std::{
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

/// A well-formed token; servers that do not issue it reject it as unknown.
pub const TOKEN: &str = "palace_pat_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";

/// What one `pal` invocation left behind.
#[derive(Debug, PartialEq)]
pub struct Output {
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// Runs `pal` with exactly the given environment, so proxies or a developer's own PAL_* never
/// leak in. Runs off the async runtime so in-process servers keep answering meanwhile.
pub async fn pal(args: &[&str], env: &[(&str, &str)]) -> Output {
    let args: Vec<String> = args.iter().map(|arg| (*arg).to_owned()).collect();
    let env: Vec<(String, String)> = env
        .iter()
        .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
        .collect();
    tokio::task::spawn_blocking(move || {
        let output = Command::new(env!("CARGO_BIN_EXE_pal"))
            .args(args)
            .env_clear()
            .envs(env)
            .output()
            .unwrap();
        Output {
            code: output.status.code(),
            stdout: String::from_utf8(output.stdout).unwrap(),
            stderr: String::from_utf8(output.stderr).unwrap(),
        }
    })
    .await
    .unwrap()
}

/// Serves `app` on a loopback port and returns its base URL.
pub async fn serve(app: Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    url
}

/// A server that counts requests and gives every one the same answer. It reads the whole body
/// first so the client sees the answer rather than a reset mid-upload.
pub async fn counting(
    answer: impl IntoResponse + Clone + Send + Sync + 'static,
) -> (String, Arc<AtomicUsize>) {
    let hits = Arc::new(AtomicUsize::new(0));
    let counter = hits.clone();
    let app = Router::new().fallback(move |_body: Bytes| {
        let answer = answer.clone();
        counter.fetch_add(1, Ordering::SeqCst);
        async move { answer }
    });
    (serve(app).await, hits)
}
