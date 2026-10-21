//! Reader annotations on texts that never change: an article body and a conversation message.
//! An annotation is an anchor into the rendered text, with a style, a note or both.
use crate::{InputError, InputErrorKind};
use uuid::Uuid;

// Room for a long marginal note or a highlight across several paragraphs, never a document.
const NOTE_BYTES: usize = 16 * 1024;
const QUOTE_BYTES: usize = 16 * 1024;

/// What an annotation marks: one article body, or one message of one conversation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnnotationTarget {
    Article(Uuid),
    Message { conversation: Uuid, message: Uuid },
}

/// How marked text is drawn: four highlighter grounds and two pen strokes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnnotationStyle {
    Yellow,
    Green,
    Blue,
    Rose,
    Underline,
    Wavy,
}

impl AnnotationStyle {
    /// Reads the wire and column spelling.
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "yellow" => Self::Yellow,
            "green" => Self::Green,
            "blue" => Self::Blue,
            "rose" => Self::Rose,
            "underline" => Self::Underline,
            "wavy" => Self::Wavy,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Yellow => "yellow",
            Self::Green => "green",
            Self::Blue => "blue",
            Self::Rose => "rose",
            Self::Underline => "underline",
            Self::Wavy => "wavy",
        }
    }
}

/// A marginal note: plain text, nonblank, kept as written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnnotationNote(String);

impl AnnotationNote {
    /// Accepts a nonblank note of at most 16 KiB.
    pub fn parse(text: String) -> Result<Self, InputError> {
        if text.trim().is_empty() || text.len() > NOTE_BYTES {
            return Err(InputError::new(
                InputErrorKind::Field,
                "note",
                "expected a nonblank note of at most 16 KiB",
            ));
        }
        Ok(Self(text))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// What an annotation shows. A mark with neither a style nor a note would show nothing, so that
/// state has no variant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AnnotationContent {
    Styled(AnnotationStyle),
    Noted(AnnotationNote),
    StyledNoted(AnnotationStyle, AnnotationNote),
}

impl AnnotationContent {
    /// Validates the optional parts of a create or an edit; at least one must be present.
    pub fn parse(style: Option<AnnotationStyle>, note: Option<String>) -> Result<Self, InputError> {
        let note = note.map(AnnotationNote::parse).transpose()?;
        match (style, note) {
            (Some(style), Some(note)) => Ok(Self::StyledNoted(style, note)),
            (Some(style), None) => Ok(Self::Styled(style)),
            (None, Some(note)) => Ok(Self::Noted(note)),
            (None, None) => Err(InputError::new(
                InputErrorKind::Field,
                "style",
                "an annotation needs a style or a note; delete it to remove both",
            )),
        }
    }

    /// Rebuilds content the database already holds; its check constraint keeps one part present,
    /// so `None` means the row was written outside these rules.
    pub fn from_stored(style: Option<AnnotationStyle>, note: Option<String>) -> Option<Self> {
        match (style, note.map(AnnotationNote)) {
            (Some(style), Some(note)) => Some(Self::StyledNoted(style, note)),
            (Some(style), None) => Some(Self::Styled(style)),
            (None, Some(note)) => Some(Self::Noted(note)),
            (None, None) => None,
        }
    }

    pub fn style(&self) -> Option<AnnotationStyle> {
        match self {
            Self::Styled(style) | Self::StyledNoted(style, _) => Some(*style),
            Self::Noted(_) => None,
        }
    }

    pub fn note(&self) -> Option<&str> {
        match self {
            Self::Noted(note) | Self::StyledNoted(_, note) => Some(note.as_str()),
            Self::Styled(_) => None,
        }
    }
}

/// Where an annotation sits: `[start, end)` in UTF-16 code units of the rendered text, which is
/// how the browser indexes strings, and the text found there when it was made. The server never
/// renders Markdown, so it can only check that the two agree in length; the page checks the text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AnnotationAnchor {
    start: i32,
    end: i32,
    quote: String,
}

impl AnnotationAnchor {
    /// Validates the offsets against the quote's own length.
    pub fn parse(start: i64, end: i64, quote: String) -> Result<Self, InputError> {
        let offset = |value: i64, name: &str| {
            i32::try_from(value)
                .ok()
                .filter(|value| *value >= 0)
                .ok_or_else(|| {
                    InputError::new(
                        InputErrorKind::Field,
                        name,
                        "expected a nonnegative 32-bit offset",
                    )
                })
        };
        let start = offset(start, "start")?;
        let end = offset(end, "end")?;
        if start >= end {
            return Err(InputError::new(
                InputErrorKind::Field,
                "end",
                "expected end after start",
            ));
        }
        if quote.trim().is_empty() || quote.len() > QUOTE_BYTES {
            return Err(InputError::new(
                InputErrorKind::Field,
                "quote",
                "expected nonblank marked text of at most 16 KiB",
            ));
        }
        if i64::try_from(quote.encode_utf16().count()) != Ok(i64::from(end - start)) {
            return Err(InputError::new(
                InputErrorKind::Field,
                "quote",
                "expected the marked text to span end - start UTF-16 code units",
            ));
        }
        Ok(Self { start, end, quote })
    }

    /// Rebuilds an anchor the database already holds; it was validated when it was written.
    pub fn from_stored(start: i32, end: i32, quote: String) -> Self {
        Self { start, end, quote }
    }

    pub fn start(&self) -> i32 {
        self.start
    }
    pub fn end(&self) -> i32 {
        self.end
    }
    pub fn quote(&self) -> &str {
        &self.quote
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    /// Content keeps whichever parts were given and refuses an annotation with neither.
    /// Core test case:
    /// - `specs/test-cases/web/stars/annotations.md#an-annotation-must-keep-a-style-or-a-note-and-an-immutable-anchor`
    #[test]
    fn content_needs_a_style_or_a_note() {
        let note = || AnnotationNote(" 想法 ".into());
        assert_eq!(
            AnnotationContent::parse(Some(AnnotationStyle::Rose), Some(" 想法 ".into())),
            Ok(AnnotationContent::StyledNoted(
                AnnotationStyle::Rose,
                note()
            ))
        );
        assert_eq!(
            AnnotationContent::parse(None, Some(" 想法 ".into())),
            Ok(AnnotationContent::Noted(note()))
        );
        assert_eq!(
            AnnotationContent::parse(Some(AnnotationStyle::Wavy), None),
            Ok(AnnotationContent::Styled(AnnotationStyle::Wavy))
        );
        for (style, note, field) in [
            (None, None, "style"),
            (None, Some(" \n"), "note"),
            (
                Some(AnnotationStyle::Yellow),
                Some(&*"字".repeat(5462)),
                "note",
            ),
        ] {
            assert_eq!(
                AnnotationContent::parse(style, note.map(str::to_owned))
                    .map_err(|error| error.path),
                Err(field.to_owned())
            );
        }
    }

    /// Offsets count UTF-16 code units, so a character outside the BMP spans two.
    /// Core test case:
    /// - `specs/test-cases/web/stars/annotations.md#an-annotation-must-keep-a-style-or-a-note-and-an-immutable-anchor`
    #[test]
    fn anchors_must_agree_with_their_quote() {
        assert_eq!(
            AnnotationAnchor::parse(/*start*/ 3, /*end*/ 7, "笔记😀".into()),
            Ok(AnnotationAnchor {
                start: 3,
                end: 7,
                quote: "笔记😀".into()
            })
        );
        for (start, end, quote, field) in [
            (-1, 2, "ab", "start"),
            (0, i64::from(i32::MAX) + 1, "ab", "end"),
            (2, 2, "ab", "end"),
            (0, 2, "  ", "quote"),
            (0, 3, "ab", "quote"),
            (0, 3, "笔记😀", "quote"),
        ] {
            assert_eq!(
                AnnotationAnchor::parse(start, end, quote.into()).map_err(|error| error.path),
                Err(field.to_owned())
            );
        }
    }

    /// Every style round-trips through its column spelling.
    #[test]
    fn styles_round_trip() {
        for style in [
            AnnotationStyle::Yellow,
            AnnotationStyle::Green,
            AnnotationStyle::Blue,
            AnnotationStyle::Rose,
            AnnotationStyle::Underline,
            AnnotationStyle::Wavy,
        ] {
            assert_eq!(AnnotationStyle::parse(style.as_str()), Some(style));
        }
    }
}
