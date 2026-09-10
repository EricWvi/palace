use crate::{SyncError, SyncTransport};
use palace_domain::{Record, ServerVersion, SyncPage, UploadResult};

/// Uses a separately authenticated HTTP client; session credentials never enter the replica database.
pub struct HttpTransport {
    client: reqwest::Client,
    endpoint: url::Url,
    origin: String,
}
impl HttpTransport {
    /// Requires an origin rather than arbitrary paths so upload and pull target one authenticated scope.
    pub fn new(client: reqwest::Client, origin: &str) -> Result<Self, SyncError> {
        let url = url::Url::parse(origin).map_err(|_| SyncError::Protocol)?;
        if !matches!(url.scheme(), "https" | "http")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.path() != "/"
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(SyncError::Protocol);
        }
        Ok(Self {
            client,
            endpoint: url.join("api/sync").map_err(|_| SyncError::Protocol)?,
            origin: url.origin().ascii_serialization(),
        })
    }
}
impl SyncTransport for HttpTransport {
    /// Preserves per-record committed results; unsuccessful HTTP exchanges leave the snapshot pending.
    async fn upload(&self, records: Vec<Record>) -> Result<Vec<UploadResult>, SyncError> {
        self.client
            .post(self.endpoint.clone())
            .header("Origin", &self.origin)
            .json(&records)
            .send()
            .await
            .map_err(|_| SyncError::Transport)?
            .error_for_status()
            .map_err(|_| SyncError::Transport)?
            .json()
            .await
            .map_err(|_| SyncError::Protocol)
    }
    /// Sends the exact decimal cursor and lets the replica atomically commit the returned page.
    async fn pull(&self, cursor: ServerVersion) -> Result<SyncPage, SyncError> {
        self.client
            .get(self.endpoint.clone())
            .query(&[("cursor", String::from(cursor)), ("limit", "100".into())])
            .send()
            .await
            .map_err(|_| SyncError::Transport)?
            .error_for_status()
            .map_err(|_| SyncError::Transport)?
            .json()
            .await
            .map_err(|_| SyncError::Protocol)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    /// Prevents endpoint paths and credentials from accidentally changing the authenticated sync origin.
    #[test]
    fn transport_requires_one_unambiguous_origin() {
        let transport = HttpTransport::new(reqwest::Client::new(), "https://palace.test/").unwrap();
        assert_eq!(
            (transport.endpoint.as_str(), transport.origin.as_str()),
            ("https://palace.test/api/sync", "https://palace.test")
        );
        for origin in [
            "file:///tmp/file",
            "https://user:pass@palace.test",
            "https://palace.test/path",
            "https://palace.test?query",
            "https://palace.test/#fragment",
        ] {
            assert!(HttpTransport::new(reqwest::Client::new(), origin).is_err());
        }
    }
}
