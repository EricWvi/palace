use base64::{Engine, engine::general_purpose::STANDARD};
use palace_backend::{OidcProvider, Server, router};
use palace_db::{CredentialKey, Database};
use palace_domain::{ImportLimits, SourceLinks};
use palace_logging::{LogLevel, LogOutput, LoggingConfig, init_logging, palace_info, palace_warn};

/// Composes the HTTP server and durable revocation worker from explicit deployment configuration.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let timezone = std::env::var("PALACE_TIMEZONE")
        .unwrap_or_else(|_| "Asia/Shanghai".into())
        .parse()?;
    let _logging = init_logging(LoggingConfig::new(
        LogLevel::Info,
        LogOutput::Stdout,
        timezone,
    ))?;
    let origin = validate_origin(&std::env::var("PALACE_ORIGIN")?)?;
    let database = Database::connect(&std::env::var("PALACE_DATABASE_URL")?).await?;
    let key = decode_key(&std::env::var("PALACE_SESSION_KEY")?)?;
    let provider = OidcProvider::discover(
        std::env::var("PALACE_OIDC_ISSUER")?,
        std::env::var("PALACE_OIDC_CLIENT_ID")?,
        std::env::var("PALACE_OIDC_CLIENT_SECRET")?,
        format!("{origin}/auth/callback"),
    )
    .await?;
    let worker_db = database.clone();
    let worker_key = key.clone();
    let worker_provider = provider.clone();
    let worker = tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
        loop {
            interval.tick().await;
            if worker_db
                .retry_revocations(&worker_key, &worker_provider)
                .await
                .is_err()
            {
                palace_warn!(
                    message = "Session revocation retry failed; durable work remains pending"
                );
            }
        }
    });
    let app = router(Server {
        database,
        provider,
        credential_key: key,
        origin,
        links: SourceLinks::standard()?,
        limits: ImportLimits::default(),
        now: now_seconds,
    });
    let address = std::env::var("PALACE_LISTEN").unwrap_or_else(|_| "127.0.0.1:8080".into());
    let listener = tokio::net::TcpListener::bind(&address).await?;
    palace_info!(message="Palace server listening",address=%address);
    let result = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown())
        .await;
    worker.abort();
    result?;
    Ok(())
}
/// Supplies the initialized local clock while keeping persisted epoch arithmetic timezone-independent.
fn now_seconds() -> i64 {
    palace_logging::clock::now_local().unix_timestamp()
}
/// Requires a canonical HTTPS origin because host-only Secure cookies define the browser boundary.
fn validate_origin(value: &str) -> Result<String, Box<dyn std::error::Error>> {
    let url = url::Url::parse(value)?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || url.username() != ""
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("PALACE_ORIGIN must be an HTTPS origin without path or credentials".into());
    }
    Ok(url.origin().ascii_serialization())
}
/// Accepts only a complete 256-bit deployment key, never a hard-coded fallback.
fn decode_key(value: &str) -> Result<CredentialKey, Box<dyn std::error::Error>> {
    let bytes = STANDARD
        .decode(value)
        .map_err(|_| "PALACE_SESSION_KEY must be base64")?;
    let key: [u8; 32] = bytes
        .try_into()
        .map_err(|_| "PALACE_SESSION_KEY must decode to 32 bytes")?;
    Ok(CredentialKey::new(key))
}
/// Allows the HTTP server to finish in-flight work before its process exits.
async fn shutdown() {
    let _ = tokio::signal::ctrl_c().await;
}
#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    /// Rejects ambiguous browser origins and missing or truncated encryption keys at startup.
    #[test]
    fn validates_security_configuration() {
        assert_eq!(
            validate_origin("https://palace.test/").unwrap(),
            "https://palace.test"
        );
        for value in [
            "http://palace.test",
            "https://a:b@palace.test",
            "https://palace.test/path",
            "https://palace.test/?query",
            "https://palace.test/#fragment",
        ] {
            assert!(validate_origin(value).is_err());
        }
        assert!(decode_key(&STANDARD.encode([1; 32])).is_ok());
        assert!(decode_key(&STANDARD.encode([1; 31])).is_err());
        assert!(decode_key("not-base64").is_err());
    }
}
