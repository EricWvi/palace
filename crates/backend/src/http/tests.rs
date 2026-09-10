use super::security::*;
use axum::http::{HeaderMap, HeaderValue, header};
use pretty_assertions::assert_eq;

/// Requires explicit same-origin writes independently of SameSite cookie behavior.
#[test]
fn origin_and_opaque_cookie_boundaries() {
    let mut headers = HeaderMap::new();
    assert!(check_origin(&headers, "https://palace.test").is_err());
    for origin in [
        "null",
        "https://evil.test",
        "https://palace.test.evil",
        "http://palace.test",
    ] {
        headers.insert(header::ORIGIN, HeaderValue::from_str(origin).unwrap());
        assert!(check_origin(&headers, "https://palace.test").is_err());
    }
    headers.insert(
        header::ORIGIN,
        HeaderValue::from_static("https://palace.test"),
    );
    assert!(check_origin(&headers, "https://palace.test").is_ok());
    headers.append(
        header::ORIGIN,
        HeaderValue::from_static("https://palace.test"),
    );
    assert!(check_origin(&headers, "https://palace.test").is_err());
    headers.insert(
        header::COOKIE,
        HeaderValue::from_static("__Host-palace-session=opaque_123-abc"),
    );
    assert_eq!(
        cookie(&headers, SESSION_COOKIE).ok(),
        Some(Some("opaque_123-abc".into()))
    );
    headers.append(
        header::COOKIE,
        HeaderValue::from_static("__Host-palace-session=other"),
    );
    assert!(cookie(&headers, SESSION_COOKIE).is_err());
    assert_eq!(
        set_cookie(SESSION_COOKIE, "opaque", 600).ok(),
        Some(HeaderValue::from_static(
            "__Host-palace-session=opaque; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age=600"
        ))
    );
}
