//! 日课: daily practices and how many times one was done on a local day. A check carries no
//! time; the day is the reader's, named by the page.
use crate::{InputError, InputErrorKind, LocalDay, todo::nonblank};

const NAME_BYTES: usize = 256;
/// More than ten a day stops reading as a seal filling up and starts being a counter.
const MAX_TIMES: u8 = 10;
/// Days one read may cover: the history page's nine weeks with room to spare.
const MAX_RANGE_DAYS: i64 = 100;

/// A habit's name, trimmed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HabitName(String);

impl HabitName {
    pub fn parse(text: &str) -> Result<Self, InputError> {
        nonblank(text, NAME_BYTES, "name", "name").map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The one character a habit's seal shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HabitMark(char);

impl HabitMark {
    pub fn parse(text: &str) -> Result<Self, InputError> {
        let mut chars = text.trim().chars();
        match (chars.next(), chars.next()) {
            (Some(mark), None) => Ok(Self(mark)),
            (None, _) | (Some(_), Some(_)) => Err(InputError::new(
                InputErrorKind::Field,
                "mark",
                "expected exactly one character",
            )),
        }
    }

    pub fn text(self) -> String {
        self.0.to_string()
    }
}

/// How many times a day a habit asks for, one to ten.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct HabitTimes(u8);

impl HabitTimes {
    pub fn parse(value: i64) -> Result<Self, InputError> {
        match u8::try_from(value) {
            Ok(times) if (1..=MAX_TIMES).contains(&times) => Ok(Self(times)),
            Ok(_) | Err(_) => Err(InputError::new(
                InputErrorKind::Field,
                "times_per_day",
                "expected 1 to 10 times a day",
            )),
        }
    }

    pub fn get(self) -> u8 {
        self.0
    }
}

/// What a habit is: its name, seal and times a day, as created or edited together.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HabitInput {
    pub name: HabitName,
    pub mark: HabitMark,
    pub times_per_day: HabitTimes,
}

/// The times a day was done, as a check sets it. Zero clears the day. Whether it fits the
/// habit's times a day is checked against the habit as stored, in the same statement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HabitCount(u8);

impl HabitCount {
    pub fn parse(value: i64) -> Result<Self, InputError> {
        match u8::try_from(value) {
            Ok(count) if count <= MAX_TIMES => Ok(Self(count)),
            Ok(_) | Err(_) => Err(InputError::new(
                InputErrorKind::Field,
                "count",
                "expected 0 to 10 times",
            )),
        }
    }

    pub fn get(self) -> u8 {
        self.0
    }
}

/// The local days one read covers, both ends included, at most 100 days.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HabitDays {
    from: LocalDay,
    to: LocalDay,
}

impl HabitDays {
    pub fn parse(from: &str, to: &str) -> Result<Self, InputError> {
        let from = LocalDay::parse(from, "from")?;
        let to = LocalDay::parse(to, "to")?;
        if !(0..MAX_RANGE_DAYS).contains(&from.days_until(to)) {
            return Err(InputError::new(
                InputErrorKind::Field,
                "to",
                "expected from <= to within 100 days",
            ));
        }
        Ok(Self { from, to })
    }

    pub fn from(self) -> LocalDay {
        self.from
    }

    pub fn to(self) -> LocalDay {
        self.to
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    /// Core test case:
    /// - `specs/test-cases/web/habits/habits.md#a-check-must-be-set-never-accumulated`
    #[test]
    fn times_and_counts_stay_within_one_to_ten() {
        let path = |result: Result<(), InputError>| result.map_err(|error| error.path);
        assert_eq!(
            [
                path(HabitTimes::parse(1).map(|_| ())),
                path(HabitTimes::parse(10).map(|_| ())),
                path(HabitTimes::parse(0).map(|_| ())),
                path(HabitTimes::parse(11).map(|_| ())),
                path(HabitCount::parse(0).map(|_| ())),
                path(HabitCount::parse(10).map(|_| ())),
                path(HabitCount::parse(-1).map(|_| ())),
                path(HabitCount::parse(11).map(|_| ())),
            ],
            [
                Ok(()),
                Ok(()),
                Err("times_per_day".to_owned()),
                Err("times_per_day".to_owned()),
                Ok(()),
                Ok(()),
                Err("count".to_owned()),
                Err("count".to_owned()),
            ]
        );
    }

    #[test]
    fn a_seal_takes_one_character_and_a_name_some_text() {
        assert_eq!(
            HabitMark::parse(" 锻 ").map(HabitMark::text),
            Ok("锻".into())
        );
        let refused = [
            HabitMark::parse("锻炼").map(|_| ()),
            HabitMark::parse(" ").map(|_| ()),
            HabitName::parse("  ").map(|_| ()),
            HabitName::parse(&"a".repeat(257)).map(|_| ()),
        ]
        .map(|result| result.map_err(|error| error.path));
        assert_eq!(
            refused,
            ["mark", "mark", "name", "name"].map(|p| Err(p.into()))
        );
    }

    #[test]
    fn a_read_covers_one_to_a_hundred_days() {
        let path =
            |from: &str, to: &str| HabitDays::parse(from, to).map(|_| ()).map_err(|e| e.path);
        assert_eq!(
            [
                path("2026-10-31", "2026-10-31"),
                path("2026-08-01", "2026-11-08"),
                path("2026-08-01", "2026-11-09"),
                path("2026-10-31", "2026-10-30"),
                path("2026-10-31", "tomorrow"),
            ],
            [
                Ok(()),
                Ok(()),
                Err("to".to_owned()),
                Err("to".to_owned()),
                Err("to".to_owned()),
            ]
        );
    }
}
