//! 时刻 sleep: the span of one night. Every rule about the
//! reader's clock (18:00, 05:00, 15:00) lives in the browser; these are the structural ones.
use crate::{InputError, InputErrorKind, epoch_instant};

const HOUR_MS: i64 = 60 * 60 * 1000;
/// A night longer than a day is a start someone forgot, not a night.
const MAX_NIGHT_MS: i64 = 24 * HOUR_MS;

/// One night, `[started_at, ended_at)` in epoch milliseconds: it has length and lasts at most a day.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SleepSpan {
    started_at: i64,
    ended_at: i64,
}

impl SleepSpan {
    pub fn parse(started_at: i64, ended_at: i64) -> Result<Self, InputError> {
        let started_at = epoch_instant(started_at, "started_at")?;
        let ended_at = epoch_instant(ended_at, "ended_at")?;
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
                field(SleepSpan::parse(NIGHT_START, crate::instant::LATEST_MS)),
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
}
