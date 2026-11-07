//! 总批: one block of Markdown on a whole document, where an annotation belongs to a run of words.
//! It has no anchor, so unlike an annotation it survives anything that happens to the text.
use crate::{InputError, InputErrorKind};

// Long enough for pages of reading notes; an annotation's note stops at 16 KiB.
const TEXT_BYTES: usize = 64 * 1024;
const FIELD: &str = "text";
// Between two notes merged by a binding: a thematic break, so neither reads as the other's end.
const SEPARATOR: &str = "\n\n---\n\n";

/// The text of an overall note: nonblank, at most 64 KiB, kept as written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OverallNoteText(String);

impl OverallNoteText {
    /// Reads what a reader submitted. Blank text means no note, which is how one is cleared.
    pub fn parse(text: String) -> Result<Option<Self>, InputError> {
        if text.trim().is_empty() {
            return Ok(None);
        }
        Self::bounded(text).map(Some)
    }

    /// Rebuilds a note the database already holds; it was validated when it was written.
    pub fn from_stored(text: String) -> Self {
        Self(text)
    }

    /// Appends a web page's note to its article's. Nothing is ever overwritten, so a note too
    /// long to merge fails the binding, with every part left as it was.
    pub fn merged(&self, incoming: &Self) -> Result<Self, InputError> {
        let Self(kept) = self;
        let Self(added) = incoming;
        Self::bounded(format!("{kept}{SEPARATOR}{added}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Accepts text within the limit.
    fn bounded(text: String) -> Result<Self, InputError> {
        if text.len() > TEXT_BYTES {
            return Err(InputError::new(
                InputErrorKind::Field,
                FIELD,
                "expected an overall note of at most 64 KiB",
            ));
        }
        Ok(Self(text))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    /// Text is kept as written, blank text clears, and the limit counts bytes.
    /// Core test case:
    /// - `specs/test-cases/web/stars/overall-notes.md#an-overall-note-must-belong-to-exactly-one-document-and-go-with-it`
    #[test]
    fn text_is_bounded_and_blank_means_none() {
        assert_eq!(
            OverallNoteText::parse("  想到一件事\n".into()),
            Ok(Some(OverallNoteText("  想到一件事\n".into())))
        );
        assert_eq!(OverallNoteText::parse(" \n\t".into()), Ok(None));
        assert!(OverallNoteText::parse("a".repeat(TEXT_BYTES)).is_ok());
        assert_eq!(
            OverallNoteText::parse("a".repeat(TEXT_BYTES + 1)).map_err(|error| error.path),
            Err(FIELD.to_owned())
        );
    }

    /// A merge keeps both notes in order, and refuses to outgrow the limit.
    /// Core test case:
    /// - `specs/test-cases/web/stars/overall-notes.md#binding-a-web-page-must-carry-its-overall-note-into-the-article-whole`
    #[test]
    fn merging_appends_and_stays_within_the_limit() {
        let kept = OverallNoteText("文章的".into());
        assert_eq!(
            kept.merged(&OverallNoteText("网页的".into())),
            Ok(OverallNoteText("文章的\n\n---\n\n网页的".into()))
        );
        assert!(
            kept.merged(&OverallNoteText("a".repeat(TEXT_BYTES)))
                .is_err()
        );
    }
}
