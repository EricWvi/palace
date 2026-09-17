mod postgres;
mod runtime;

use palace_logging::{LogLevel, LogOutput, LoggingConfig, init_logging, palace_info};

/// Serves a fixed local owner over HTTP while retaining PostgreSQL data between runs.
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
    let address = std::env::var("PALACE_LISTEN").unwrap_or_else(|_| "127.0.0.1:8080".into());
    let listener = tokio::net::TcpListener::bind(&address).await?;
    let bound_address = listener.local_addr()?;
    let origin =
        std::env::var("PALACE_ORIGIN").unwrap_or_else(|_| format!("http://{bound_address}"));
    let url = url::Url::parse(&origin)?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("PALACE_ORIGIN must be an HTTP(S) origin without path or credentials".into());
    }
    let origin = url.origin().ascii_serialization();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .ok_or("server package must be inside the workspace apps directory")?;
    let postgres = postgres::Postgres::start(
        &root.join(".data").join("postgres"),
        postgres::PortBinding::Debug,
    )
    .await?;
    let result = async {
        let database = palace_db::Database::connect(&postgres.url).await?;
        palace_info!(message="PostgreSQL debug port published",port=15432,database="palace",username="postgres");
        let app = palace_backend::fixed_user_router(database, origin.clone()).await?;
        palace_info!(message="Palace single-user test server listening",address=%address,origin=%origin);
        axum::serve(listener, app).with_graceful_shutdown(runtime::shutdown()).await?;
        Ok::<(), Box<dyn std::error::Error>>(())
    }.await;
    let cleanup = postgres.shutdown().await;
    result?;
    cleanup
}

#[cfg(test)]
mod single_user_tests;
