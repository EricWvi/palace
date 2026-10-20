//! Authenticated Palace requests. Each command sends at most one request, including article
//! uploads whose URL-based retry semantics differ from independent media uploads.
mod article;
use crate::{
    config::Config,
    failure::{Failure, Operation, Rejection},
};
use reqwest::{
    StatusCode,
    blocking::{self, Response, multipart},
    header::AUTHORIZATION,
    redirect,
};
use serde::de::IgnoredAny;
use std::{ffi::OsStr, fs::File, io, path::Path, time::Duration};
use uuid::Uuid;

pub(crate) struct Client {
    http: blocking::Client,
    config: Config,
}

impl Client {
    /// Bounds only connection setup: a total timeout would fail large uploads on slow links, and
    /// the user can always interrupt. Redirects are refused so the token never leaves `PAL_URL`.
    pub(crate) fn new(config: Config) -> Result<Self, Failure> {
        let http = blocking::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(None)
            .redirect(redirect::Policy::none())
            .user_agent(concat!("pal/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(Failure::Request)?;
        Ok(Self { http, config })
    }

    /// Streams one file as the `file` part and returns the stored media JSON exactly as Palace
    /// sent it, so scripts see every field and the CLI never reorders or drops one.
    pub(crate) fn upload(&self, path: &Path) -> Result<String, Failure> {
        let unreadable = |source| Failure::File {
            path: path.to_owned(),
            source,
        };
        let file = File::open(path).map_err(unreadable)?;
        let metadata = file.metadata().map_err(unreadable)?;
        if !metadata.is_file() {
            return Err(unreadable(io::Error::new(
                io::ErrorKind::InvalidInput,
                "not a regular file",
            )));
        }
        // Palace records the declared type for good and never guesses, so the CLI declares one
        // from the extension; without it every image would be stored as octet-stream.
        let content_type = mime_guess::from_path(path).first_or_octet_stream();
        let mut part = multipart::Part::reader_with_length(file, metadata.len())
            .mime_str(content_type.essence_str())
            .map_err(Failure::Request)?;
        // A name that is not UTF-8 is left out rather than lossily rewritten.
        if let Some(name) = path.file_name().and_then(OsStr::to_str) {
            part = part.file_name(name.to_owned());
        }
        let response = self
            .http
            .post(self.config.endpoint(&["api", "media"]))
            .header(AUTHORIZATION, self.config.authorization().clone())
            .multipart(multipart::Form::new().part("file", part))
            .send()
            .map_err(|source| Failure::Transport {
                operation: Operation::Upload,
                source,
            })?;
        let body = expect(response, StatusCode::CREATED, Operation::Upload)?;
        serde_json::from_str::<IgnoredAny>(&body).map_err(Failure::UnreadableMedia)?;
        Ok(body.trim_end().to_owned())
    }

    /// Succeeds only on 204, which Palace answers after the stored object is gone.
    pub(crate) fn delete(&self, id: Uuid) -> Result<(), Failure> {
        let id = id.to_string();
        let response = self
            .http
            .delete(self.config.endpoint(&["api", "media", &id]))
            .header(AUTHORIZATION, self.config.authorization().clone())
            .send()
            .map_err(|source| Failure::Transport {
                operation: Operation::Delete,
                source,
            })?;
        expect(response, StatusCode::NO_CONTENT, Operation::Delete)?;
        Ok(())
    }
}

/// Reads the body and accepts only the one status the operation promises; every other answer,
/// including redirects, becomes a rejection carrying Palace's error body.
fn expect(
    response: Response,
    success: StatusCode,
    operation: Operation,
) -> Result<String, Failure> {
    let status = response.status();
    let body = response
        .text()
        .map_err(|source| Failure::Transport { operation, source })?;
    if status == success {
        Ok(body)
    } else {
        Err(Failure::Rejected(Rejection { status, body }))
    }
}
