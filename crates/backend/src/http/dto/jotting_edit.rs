//! Web edits and deletions of jottings.
use super::Cursor;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

/// A jotting's new title, tags and body, as of the version the page read.
#[derive(Deserialize, ToSchema)]
pub(crate) struct JottingEdit {
    /// The `version` the page read; an older one is refused with `jotting_changed`.
    pub version: Cursor,
    /// Omitted, null or blank removes the title.
    #[serde(default)]
    pub title: Option<String>,
    /// Distinct non-blank tags.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Markdown, at most 1 MiB; leading blank lines are dropped.
    pub body: String,
}
impl From<JottingEdit> for palace_jottings::JottingEdit {
    fn from(value: JottingEdit) -> Self {
        Self {
            title: value.title,
            tags: value.tags,
            body: value.body,
        }
    }
}

/// The version an edit is published under; unchanged when the edit changed nothing.
#[derive(Serialize, ToSchema)]
pub(crate) struct JottingEdited {
    pub version: Cursor,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct JottingDeleteQuery {
    /// The `version` the page read; an older one is refused with `jotting_changed`.
    pub version: Cursor,
}
