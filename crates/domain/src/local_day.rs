//! A calendar day as the reader's browser names it. The server never decides what "today" is,
//! since the reader and the deployment may sit in different zones.
use crate::{InputError, InputErrorKind};

/// A day, `YYYY-MM-DD`, kept as the text the page sent and as a date for arithmetic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct LocalDay(time::Date);

const FORMAT: &[time::format_description::BorrowedFormatItem<'static>] =
    time::macros::format_description!("[year]-[month]-[day]");

impl LocalDay {
    pub fn parse(text: &str, field: &str) -> Result<Self, InputError> {
        let refused = || InputError::new(InputErrorKind::Field, field, "expected a YYYY-MM-DD day");
        let day = time::Date::parse(text, FORMAT).map_err(|_| refused())?;
        // Years past 9999 would format with a sign; the columns and the page expect four digits.
        if !(1..=9999).contains(&day.year()) {
            return Err(refused());
        }
        Ok(Self(day))
    }

    /// Rebuilds a day the database formatted itself.
    pub fn from_stored(text: &str) -> Result<Self, InputError> {
        Self::parse(text, "day")
    }

    /// The `YYYY-MM-DD` text the database column and the page both read.
    pub fn text(self) -> String {
        // Four-digit years are guaranteed by `parse`, so formatting cannot fail.
        self.0.format(FORMAT).unwrap_or_default()
    }

    /// Whole days from `self` to `later`; negative when `later` is earlier.
    pub fn days_until(self, later: Self) -> i64 {
        (later.0 - self.0).whole_days()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn only_real_four_digit_days_parse() {
        let path =
            |result: Result<LocalDay, InputError>| result.map(LocalDay::text).map_err(|e| e.path);
        assert_eq!(
            [
                path(LocalDay::parse("2026-10-23", "day")),
                path(LocalDay::parse("2026-2-30", "day")),
                path(LocalDay::parse("2026-02-30", "on")),
            ],
            [
                Ok("2026-10-23".to_owned()),
                Err("day".to_owned()),
                Err("on".to_owned())
            ]
        );
        let first = LocalDay::parse("2026-10-31", "day").unwrap();
        let later = LocalDay::parse("2026-11-02", "day").unwrap();
        assert_eq!([first.days_until(later), later.days_until(first)], [2, -2]);
    }
}
