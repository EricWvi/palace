mod runtime;
mod server;

/// Runs the production server against explicitly configured database and identity services.
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let origin = server::validate_origin(&std::env::var("PALACE_ORIGIN")?)?;
    let key = server::decode_key(&std::env::var("PALACE_SESSION_KEY")?)?;
    let provider = palace_backend::OidcProvider::discover(
        std::env::var("PALACE_OIDC_ISSUER")?,
        std::env::var("PALACE_OIDC_CLIENT_ID")?,
        std::env::var("PALACE_OIDC_CLIENT_SECRET")?,
        format!("{origin}/auth/callback"),
    )
    .await?;
    server::run(
        &std::env::var("PALACE_DATABASE_URL")?,
        origin,
        key,
        provider,
    )
    .await
}
