use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// A document's 总批. Without one, `text` is null and `version` is 0.
#[derive(Serialize, ToSchema)]
pub(crate) struct OverallNote {
    /// Markdown as written.
    #[schema(required = true)]
    pub text: Option<String>,
    /// The version a save must name. It only ever moves to numbers not used before.
    pub version: i64,
}

impl From<palace_db::OverallNote> for OverallNote {
    fn from(value: palace_db::OverallNote) -> Self {
        Self {
            text: value.text,
            version: value.version,
        }
    }
}

/// Replaces a document's 总批.
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct OverallNoteSave {
    /// At most 64 KiB; blank clears the note.
    pub text: String,
    /// The version last read; any other current version is a conflict.
    pub version: i64,
}

/// Replaces the 总批 of the web page at `url`.
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct WebPageOverallNoteSave {
    /// The page address without its fragment.
    pub url: String,
    /// The tab title.
    pub title: String,
    /// At most 64 KiB; blank clears the note.
    pub text: String,
    /// The version last read; any other current version is a conflict.
    pub version: i64,
}
