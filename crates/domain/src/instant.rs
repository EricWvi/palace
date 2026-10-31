//! Epoch-millisecond instants and the ranges records are read in, shared by every kind of
//! timed record the page writes (nights, activities).
use crate::{InputError, InputErrorKind};

const DAY_MS: i64 = 24 * 60 * 60 * 1000;
/// One request reads at most a year of history, plus room for time zones and DST.
const MAX_RANGE_MS: i64 = 400 * DAY_MS;
/// Instants the database can store and a browser can show: 1970 up to the year 10000.
pub(crate) const LATEST_MS: i64 = 253_402_300_800_000;

/// Accepts an epoch-millisecond instant within the storable years.
pub fn epoch_instant(value: i64, field: &str) -> Result<i64, InputError> {
    if !(0..LATEST_MS).contains(&value) {
        return Err(InputError::new(
            InputErrorKind::Field,
            field,
            "expected epoch milliseconds between 1970 and 9999",
        ));
    }
    Ok(value)
}

/// A half-open range `[start, end)` of epoch milliseconds, at most 400 days long.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InstantRange {
    start: i64,
    end: i64,
}

impl InstantRange {
    pub fn parse(start: i64, end: i64) -> Result<Self, InputError> {
        let start = epoch_instant(start, "start")?;
        let end = epoch_instant(end, "end")?;
        if start >= end || end - start > MAX_RANGE_MS {
            return Err(InputError::new(
                InputErrorKind::Field,
                "end",
                "expected start < end within 400 days",
            ));
        }
        Ok(Self { start, end })
    }

    pub fn start(self) -> i64 {
        self.start
    }

    pub fn end(self) -> i64 {
        self.end
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn a_range_must_be_nonempty_and_at_most_400_days() {
        assert_eq!(
            [
                InstantRange::parse(0, MAX_RANGE_MS).is_ok(),
                InstantRange::parse(0, MAX_RANGE_MS + 1).is_ok(),
                InstantRange::parse(5, 5).is_ok(),
                InstantRange::parse(-1, 5).is_ok(),
            ],
            [true, false, false, false]
        );
    }
}
