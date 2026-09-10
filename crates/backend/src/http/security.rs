use super::ApiError;
use axum::http::{HeaderMap, HeaderValue, header};

pub(super) const SESSION_COOKIE: &str = "__Host-palace-session";
pub(super) const LOGIN_COOKIE: &str = "__Host-palace-login";
/// Rejects missing, null, duplicate and foreign origins before any authenticated mutation.
pub(super) fn check_origin(headers: &HeaderMap, expected: &str) -> Result<(), ApiError> {
    let values: Vec<_> = headers.get_all(header::ORIGIN).iter().collect();
    if values.len() != 1 || values[0].to_str().ok() != Some(expected) {
        return Err(ApiError::Forbidden);
    }
    Ok(())
}
/// Rejects ambiguous duplicate credentials rather than choosing an attacker-controlled cookie order.
pub(super) fn cookie(headers: &HeaderMap, name: &str) -> Result<Option<String>, ApiError> {
    let mut found = None;
    for value in headers.get_all(header::COOKIE) {
        let value = value.to_str().map_err(|_| ApiError::Unauthorized)?;
        for pair in value.split(';') {
            if let Some((key, value)) = pair.trim().split_once('=')
                && key == name
            {
                if found.is_some()
                    || value.is_empty()
                    || !value
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
                {
                    return Err(ApiError::Unauthorized);
                }
                found = Some(value.to_owned());
            }
        }
    }
    Ok(found)
}
/// Uses a finite browser lifetime while server-side sessions remain explicitly revocable.
pub(super) fn set_cookie(name: &str, secret: &str, max_age: u32) -> Result<HeaderValue, ApiError> {
    HeaderValue::from_str(&format!(
        "{name}={secret}; Path=/; Secure; HttpOnly; SameSite=Lax; Max-Age={max_age}"
    ))
    .map_err(|_| ApiError::Internal)
}
