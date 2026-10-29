//! 时刻 sleep: the span of one night and the ranges nights are read in. Every rule about the
//! reader's clock (18:00, 05:00, 15:00) lives in the browser; these are the structural ones.
use crate::{InputError, InputErrorKind};

const HOUR_MS: i64 = 60 * 60 * 1000;
/// A night longer than a day is a start someone forgot, not a night.
const MAX_NIGHT_MS: i64 = 24 * HOUR_MS;
/// One request reads at most the history page's year, plus room for time zones and DST.
const MAX_RANGE_MS: i64 = 400 * 24 * HOUR_MS;
/// Instants the database can store and a browser can show: 1970 up to the year 10000.
const LATEST_MS: i64 = 253_402_300_800_000;

/// Accepts an epoch-millisecond instant within the storable years, such as the bedtime an end
/// request names.
pub fn sleep_instant(value: i64, field: &str) -> Result<i64, InputError> {
    if !(0..LATEST_MS).contains(&value) {
        return Err(InputError::new(
            InputErrorKind::Field,
            field,
            "expected epoch milliseconds between 1970 and 9999",
        ));
    }
    Ok(value)
}

/// One night, `[started_at, ended_at)` in epoch milliseconds: it has length and lasts at most a day.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SleepSpan {
    started_at: i64,
    ended_at: i64,
}

impl SleepSpan {
    pub fn parse(started_at: i64, ended_at: i64) -> Result<Self, InputError> {
        let started_at = sleep_instant(started_at, "started_at")?;
        let ended_at = sleep_instant(ended_at, "ended_at")?;
        if ended_at <= started_at || ended_at - started_at > MAX_NIGHT_MS {
            return Err(InputError::new(
                InputErrorKind::Field,
                "ended_at",
                "expected a wake time after the bedtime and at most 24 hours later",
            ));
        }
        Ok(Self {
            started_at,
            ended_at,
        })
    }

    pub fn started_at(self) -> i64 {
        self.started_at
    }

    pub fn ended_at(self) -> i64 {
        self.ended_at
    }
}

/// The half-open range of wake times one list of nights covers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SleepRange {
    start: i64,
    end: i64,
}

impl SleepRange {
    pub fn parse(start: i64, end: i64) -> Result<Self, InputError> {
        let start = sleep_instant(start, "start")?;
        let end = sleep_instant(end, "end")?;
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

    const NIGHT_START: i64 = 1_761_694_000_000;

    #[test]
    fn a_night_must_have_length_and_last_at_most_a_day() {
        let field = |result: Result<SleepSpan, InputError>| result.map_err(|error| error.path);
        assert_eq!(
            [
                field(SleepSpan::parse(NIGHT_START, NIGHT_START + 7 * HOUR_MS)),
                field(SleepSpan::parse(NIGHT_START, NIGHT_START + MAX_NIGHT_MS)),
                field(SleepSpan::parse(NIGHT_START, NIGHT_START)),
                field(SleepSpan::parse(NIGHT_START, NIGHT_START - 1)),
                field(SleepSpan::parse(
                    NIGHT_START,
                    NIGHT_START + MAX_NIGHT_MS + 1
                )),
                field(SleepSpan::parse(-1, NIGHT_START)),
                field(SleepSpan::parse(NIGHT_START, LATEST_MS)),
            ],
            [
                Ok(SleepSpan {
                    started_at: NIGHT_START,
                    ended_at: NIGHT_START + 7 * HOUR_MS
                }),
                Ok(SleepSpan {
                    started_at: NIGHT_START,
                    ended_at: NIGHT_START + MAX_NIGHT_MS
                }),
                Err("ended_at".to_owned()),
                Err("ended_at".to_owned()),
                Err("ended_at".to_owned()),
                Err("started_at".to_owned()),
                Err("ended_at".to_owned()),
            ]
        );
    }

    #[test]
    fn a_range_must_be_nonempty_and_at_most_400_days() {
        assert_eq!(
            [
                SleepRange::parse(0, MAX_RANGE_MS).is_ok(),
                SleepRange::parse(0, MAX_RANGE_MS + 1).is_ok(),
                SleepRange::parse(5, 5).is_ok(),
            ],
            [true, false, false]
        );
    }
}
