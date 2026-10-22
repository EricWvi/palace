//! Every way a `pal` command can fail, rendered for stderr. Server answers are reduced to their
//! status and Palace error code so the message is actionable without dumping response bodies.
use crate::config::{ConfigError, TOKEN_VAR};
use palace_domain::JottingProblem;
use reqwest::StatusCode;
use serde::Deserialize;
use std::{error::Error as _, fmt, io, path::PathBuf};

/// Which request a transport failure interrupted; an interrupted upload may still have landed.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Operation {
    Upload,
    ArticleUpload,
    Delete,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum Failure {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error("update failed: {0}")]
    Update(#[from] crate::update::Error),
    #[error("cannot read {}: {source}", path.display())]
    File { path: PathBuf, source: io::Error },
    #[error("cannot prepare the request: {0}")]
    Request(reqwest::Error),
    #[error("{}", transport(*operation, source))]
    Transport {
        operation: Operation,
        source: reqwest::Error,
    },
    #[error("{0}")]
    Rejected(Rejection),
    #[error("Palace stored the file but answered with a body that is not JSON: {0}")]
    UnreadableMedia(serde_json::Error),
    #[error("{0}")]
    Article(#[from] crate::article::Error),
    #[error(
        "Palace may have saved the article but answered with invalid JSON: {0}; rerun with the same original URL; an existing article will not be overwritten"
    )]
    UnreadableArticle(serde_json::Error),
    #[error("{}", jottings(.0))]
    Jottings(Vec<JottingProblem>),
    #[error("{}: {error}", path.display())]
    Tags {
        path: PathBuf,
        error: crate::tags::Error,
    },
}

/// A non-success answer from Palace.
#[derive(Debug, PartialEq)]
pub(crate) struct Rejection {
    pub status: StatusCode,
    pub body: String,
}

/// The two error shapes Palace answers with; anything else is shown as text.
#[derive(Deserialize)]
#[serde(untagged)]
enum ErrorBody {
    Coded {
        error: String,
    },
    Input {
        kind: String,
        path: String,
        message: String,
    },
}

/// One line per problem, so a pre-commit hook shows every file to fix at once.
fn jottings(problems: &[JottingProblem]) -> String {
    let mut message = format!(
        "the server would refuse this commit ({} problems):",
        problems.len()
    );
    for problem in problems {
        message.push_str(&format!("\n  {problem}"));
    }
    message
}

/// Includes the source chain, because reqwest's top-level message omits the cause such as
/// "connection refused".
fn transport(operation: Operation, error: &reqwest::Error) -> String {
    let mut message = format!("cannot reach Palace: {error}");
    let mut source = error.source();
    while let Some(cause) = source {
        message.push_str(&format!(": {cause}"));
        source = cause.source();
    }
    match operation {
        Operation::Upload => {
            message.push_str("; the upload may still have completed on the server");
        }
        Operation::ArticleUpload => {
            message.push_str("; the article may have been saved; rerun with the same original URL; an existing article will not be overwritten");
        }
        Operation::Delete => {}
    }
    message
}

impl fmt::Display for Rejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let status = self.status;
        match serde_json::from_str::<ErrorBody>(&self.body) {
            Ok(ErrorBody::Coded { error }) => {
                write!(f, "Palace answered {status}: {error}")?;
                match error.as_str() {
                    "authentication_required" => write!(
                        f,
                        " (check that {TOKEN_VAR} is a token that has not been deleted)"
                    ),
                    // Palace answers the same for missing and foreign media; so does the hint.
                    "not_found" => write!(f, " (no such media for this token's owner)"),
                    _ => Ok(()),
                }
            }
            Ok(ErrorBody::Input {
                kind,
                path,
                message,
            }) => write!(
                f,
                "Palace answered {status}: {kind} error at {path}: {message}"
            ),
            Err(_) if self.body.trim().is_empty() => write!(f, "Palace answered {status}"),
            Err(_) => write!(f, "Palace answered {status}: {}", self.body.trim()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn rejections_show_status_and_palace_error() {
        let cases = [
            (
                StatusCode::UNAUTHORIZED,
                r#"{"error":"authentication_required","login":"/auth/login"}"#,
                "Palace answered 401 Unauthorized: authentication_required (check that PAL_TOKEN is a token that has not been deleted)",
            ),
            (
                StatusCode::NOT_FOUND,
                r#"{"error":"not_found","login":""}"#,
                "Palace answered 404 Not Found: not_found (no such media for this token's owner)",
            ),
            (
                StatusCode::SERVICE_UNAVAILABLE,
                r#"{"error":"media_storage_unavailable","login":""}"#,
                "Palace answered 503 Service Unavailable: media_storage_unavailable",
            ),
            (
                StatusCode::PAYLOAD_TOO_LARGE,
                r#"{"kind":"limit","path":"file","message":"file too large"}"#,
                "Palace answered 413 Payload Too Large: limit error at file: file too large",
            ),
            (
                StatusCode::BAD_REQUEST,
                "Invalid URL\n",
                "Palace answered 400 Bad Request: Invalid URL",
            ),
            (StatusCode::FOUND, "", "Palace answered 302 Found"),
        ];
        let rendered: Vec<_> = cases
            .iter()
            .map(|(status, body, _)| {
                Rejection {
                    status: *status,
                    body: (*body).to_owned(),
                }
                .to_string()
            })
            .collect();
        let expected: Vec<_> = cases
            .iter()
            .map(|(_, _, text)| (*text).to_owned())
            .collect();
        assert_eq!(rendered, expected);
    }
}
