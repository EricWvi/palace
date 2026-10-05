//! Serves the built web app with cache headers that follow how each file is named.
use axum::{
    Router,
    http::{HeaderValue, StatusCode, header::CACHE_CONTROL},
    middleware::map_response,
    response::Response,
};
use std::path::Path;
use tower_http::services::{ServeDir, ServeFile};

/// Routes `assets/` and every other path of the built web app.
///
/// Vite puts a content hash in every name under `assets/`, so those files never change and
/// may be cached for a year. Everything else, including `index.html` and the client-route
/// fallback, keeps a stable name and must be revalidated to pick up a release.
pub(crate) fn web_app(dist: &Path) -> Router {
    // No client-route fallback here: a missing hashed file must be a 404, never an index.html
    // that the browser would then keep for a year under that name.
    let assets = Router::new()
        .fallback_service(ServeDir::new(dist.join("assets")))
        .layer(map_response(cache_forever));
    let pages = Router::new()
        .fallback_service(
            // `fallback`, not `not_found_service`: a client route is a page, so it answers 200.
            ServeDir::new(dist).fallback(ServeFile::new(dist.join("index.html"))),
        )
        .layer(map_response(revalidate));
    Router::new()
        .nest_service("/assets", assets)
        .fallback_service(pages)
}
/// Marks a found hashed file as immutable; errors stay uncached.
async fn cache_forever(mut response: Response) -> Response {
    if response.status().is_success() || response.status() == StatusCode::NOT_MODIFIED {
        response.headers_mut().insert(
            CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=31536000, immutable"),
        );
    }
    response
}
/// Lets the browser keep a copy but makes it ask first, using `ServeDir`'s `Last-Modified`.
async fn revalidate(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-cache"));
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use pretty_assertions::assert_eq;
    use tower::ServiceExt;

    /// Hashed assets are immutable, missing assets are plain 404s, and every page revalidates.
    #[tokio::test]
    async fn cache_headers_follow_file_naming() {
        let dist = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dist.path().join("assets")).unwrap();
        std::fs::create_dir_all(dist.path().join("r")).unwrap();
        std::fs::write(dist.path().join("index.html"), "index").unwrap();
        std::fs::write(dist.path().join("assets").join("app-1a2b.js"), "app").unwrap();
        std::fs::write(dist.path().join("r").join("style.json"), "{}").unwrap();
        let app = web_app(dist.path());
        let mut observed = Vec::new();
        for path in [
            "/assets/app-1a2b.js",
            "/assets/missing-0000.js",
            "/",
            "/conversations/some-id?path=p&date=2025-09-30",
            "/r/style.json",
        ] {
            let response = app
                .clone()
                .oneshot(Request::get(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            let status = response.status().as_u16();
            let cache = response
                .headers()
                .get(CACHE_CONTROL)
                .map(|value| value.to_str().unwrap().to_owned());
            let body = response.into_body().collect().await.unwrap().to_bytes();
            observed.push((
                path,
                status,
                cache,
                String::from_utf8(body.to_vec()).unwrap(),
            ));
        }
        let immutable = Some("public, max-age=31536000, immutable".to_owned());
        let no_cache = Some("no-cache".to_owned());
        assert_eq!(
            observed,
            vec![
                ("/assets/app-1a2b.js", 200, immutable, "app".to_owned()),
                ("/assets/missing-0000.js", 404, None, String::new()),
                ("/", 200, no_cache.clone(), "index".to_owned()),
                (
                    "/conversations/some-id?path=p&date=2025-09-30",
                    200,
                    no_cache.clone(),
                    "index".to_owned()
                ),
                ("/r/style.json", 200, no_cache, "{}".to_owned()),
            ]
        );
    }
}
