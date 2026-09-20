//! Offline export; stdout contains only the deterministic JSON document.

/// Allows tools to generate the contract without deployment configuration or live services.
fn main() -> Result<(), serde_json::Error> {
    println!("{}", palace_backend::api_contract().to_pretty_json()?);
    Ok(())
}
