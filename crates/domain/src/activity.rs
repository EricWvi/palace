//! 工夫: time spent on a type of activity the owner names. Each one is a moment at its start; it
//! carries its length in minutes rather than a second instant.
use crate::{InputError, InputErrorKind, epoch_instant, todo::nonblank};
use uuid::Uuid;

const TYPE_NAME_BYTES: usize = 256;
const NOTE_BYTES: usize = 1024;
/// Longer than half a day is a typo, not an activity.
const MAX_MINUTES: i64 = 12 * 60;

/// An activity type's name, trimmed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActivityTypeName(String);

impl ActivityTypeName {
    pub fn parse(text: &str) -> Result<Self, InputError> {
        nonblank(text, TYPE_NAME_BYTES, "name", "name").map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One activity as recorded or corrected: its type, start, length and optional note.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActivityInput {
    type_id: Uuid,
    started_at: i64,
    minutes: i32,
    note: Option<String>,
}

impl ActivityInput {
    /// A blank note is no note, so a stored note always says something.
    pub fn parse(
        type_id: Uuid,
        started_at: i64,
        minutes: i64,
        note: Option<&str>,
    ) -> Result<Self, InputError> {
        let started_at = epoch_instant(started_at, "started_at")?;
        if !(1..=MAX_MINUTES).contains(&minutes) {
            return Err(InputError::new(
                InputErrorKind::Field,
                "minutes",
                "expected 1 to 720 minutes",
            ));
        }
        let note = match note.map(str::trim) {
            None | Some("") => None,
            Some(text) if text.len() > NOTE_BYTES => {
                return Err(InputError::new(
                    InputErrorKind::Field,
                    "note",
                    "expected a note of at most 1024 bytes",
                ));
            }
            Some(text) => Some(text.to_owned()),
        };
        Ok(Self {
            type_id,
            started_at,
            // Bounded to 720 just above.
            minutes: minutes as i32,
            note,
        })
    }

    pub fn type_id(&self) -> Uuid {
        self.type_id
    }

    pub fn started_at(&self) -> i64 {
        self.started_at
    }

    pub fn minutes(&self) -> i32 {
        self.minutes
    }

    pub fn note(&self) -> Option<&str> {
        self.note.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    /// Core test case:
    /// - `specs/test-cases/web/habits/habits.md#an-activity-must-be-one-moment-timed-at-its-start`
    #[test]
    fn an_activity_lasts_one_minute_to_twelve_hours() {
        let type_id = Uuid::nil();
        let parse = |minutes: i64, note: Option<&str>| {
            ActivityInput::parse(type_id, 1_761_694_000_000, minutes, note).map_err(|e| e.path)
        };
        assert_eq!(
            parse(35, Some("  晨跑 5 公里 ")),
            Ok(ActivityInput {
                type_id,
                started_at: 1_761_694_000_000,
                minutes: 35,
                note: Some("晨跑 5 公里".into()),
            })
        );
        assert_eq!(parse(720, Some("  ")).map(|a| a.note), Ok(None));
        assert_eq!(
            [
                parse(0, None),
                parse(721, None),
                parse(30, Some(&"字".repeat(342))),
                ActivityInput::parse(type_id, -1, 30, None).map_err(|e| e.path),
            ],
            [
                Err("minutes".to_owned()),
                Err("minutes".to_owned()),
                Err("note".to_owned()),
                Err("started_at".to_owned()),
            ]
        );
        assert_eq!(
            ActivityTypeName::parse("  ").map_err(|e| e.path),
            Err("name".to_owned())
        );
    }
}
