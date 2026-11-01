//! Live reads for the web 回响 page. The index answers them directly; what this layer adds is the
//! deployment's calendar: which local day it is, and when that day began.
use super::{Jottings, JottingsError};
use chrono::{Datelike, NaiveDate, NaiveTime, TimeZone};
use palace_db::{
    JottingListRequest, JottingOverview, JottingPage, RecallCard, RecallNext, RecallState,
};
use palace_domain::{InputError, InputErrorKind};
use std::{fmt, str::FromStr};
use time::{Date, Month};
use uuid::Uuid;

/// The longest overview range: a year, leap day included. A heatmap needs twelve weeks; the
/// bound only keeps one request from asking for every day since year 1.
const MAX_OVERVIEW_DAYS: i64 = 366;

/// The day's recall pile and the local day it belongs to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recall {
    pub date: Date,
    pub cards: Vec<RecallCard>,
}

/// How many cards a day's pile draws: 1 to 18, laid out as up to three rows of six. Read only
/// when a pile is drawn, so a new size takes effect from the next day that draws one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecallSize(u8);

impl RecallSize {
    pub const MAX: u8 = 18;
}

impl Default for RecallSize {
    fn default() -> Self {
        Self(5)
    }
}

impl FromStr for RecallSize {
    type Err = RecallSizeError;

    /// Accepts a whole number of cards within 1..=18.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().parse::<u8>() {
            Ok(size @ 1..=Self::MAX) => Ok(Self(size)),
            _ => Err(RecallSizeError(value.to_owned())),
        }
    }
}

/// A recall size outside 1..=18 or not a whole number.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
#[error("expected a whole number of cards from 1 to 18, got {0:?}")]
pub struct RecallSizeError(String);

impl fmt::Display for RecallSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Jottings {
    /// One page of live jottings, newest first.
    pub async fn list(&self, request: &JottingListRequest) -> Result<JottingPage, JottingsError> {
        Ok(self.database.jotting_list(request).await?)
    }

    /// Totals and tag counts, with per-day counts from `from` through the local day of `now`
    /// (epoch milliseconds).
    pub async fn overview(&self, from: Date, now: i64) -> Result<JottingOverview, JottingsError> {
        let today = self.local_day(now);
        let span = (today - from).whole_days();
        if !(0..MAX_OVERVIEW_DAYS).contains(&span) {
            return Err(InputError::new(
                InputErrorKind::Field,
                "from",
                "expected a day within the last year",
            )
            .into());
        }
        Ok(self
            .database
            .jotting_overview(self.timezone.name(), from, today)
            .await?)
    }

    /// The recall pile of the local day of `now` (epoch milliseconds), drawn on the day's first
    /// request. Jottings created that day are not candidates.
    pub async fn recall(&self, now: i64) -> Result<Recall, JottingsError> {
        let date = self.local_day(now);
        let cards = self
            .database
            .jotting_recall(date, self.day_start(date), i64::from(self.recall_size.0))
            .await?;
        Ok(Recall { date, cards })
    }

    /// Completes the card of `id` in the pile of `date`; only the first completion counts.
    pub async fn complete_recall(
        &self,
        date: Date,
        id: Uuid,
        next: RecallNext,
    ) -> Result<RecallState, JottingsError> {
        Ok(self.database.jotting_recall_done(date, id, next).await?)
    }

    /// Takes a jotting out of recall, or back in at the least count.
    pub async fn set_recall(&self, id: Uuid, state: RecallState) -> Result<(), JottingsError> {
        Ok(self.database.set_jotting_recall(id, state).await?)
    }

    /// The deployment's calendar day at `now`.
    fn local_day(&self, now: i64) -> Date {
        let day = self
            .timezone
            .timestamp_millis_opt(now)
            .single()
            .map(|time| time.date_naive())
            .unwrap_or_default();
        to_date(day)
    }

    /// Epoch milliseconds of the first instant of `date`. Where a transition skips midnight, the
    /// day begins at the first local time that exists.
    fn day_start(&self, date: Date) -> i64 {
        let day = NaiveDate::from_ymd_opt(
            date.year(),
            u32::from(u8::from(date.month())),
            u32::from(date.day()),
        )
        .unwrap_or_default();
        (0..24)
            .find_map(|hour| {
                let time = NaiveTime::from_hms_opt(hour, /*min*/ 0, /*sec*/ 0)?;
                self.timezone
                    .from_local_datetime(&day.and_time(time))
                    .earliest()
            })
            .map_or(0, |start| start.timestamp_millis())
    }
}

/// Converts a chrono calendar day to the `time` day the database layer takes.
fn to_date(day: NaiveDate) -> Date {
    let month = u8::try_from(day.month())
        .ok()
        .and_then(|month| Month::try_from(month).ok())
        .unwrap_or(Month::January);
    let date = u8::try_from(day.day()).unwrap_or(1);
    Date::from_calendar_date(day.year(), month, date).unwrap_or(Date::MIN)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use time::macros::date;

    /// A recall size is a whole number of cards from 1 to 18; anything else is refused.
    #[test]
    fn recall_sizes_stay_within_three_rows_of_six() {
        assert_eq!("1".parse(), Ok(RecallSize(1)));
        assert_eq!(" 18 ".parse(), Ok(RecallSize(18)));
        assert_eq!(RecallSize::default(), RecallSize(5));
        for value in ["0", "19", "-1", "5.5", "", "five"] {
            assert_eq!(
                value.parse::<RecallSize>(),
                Err(RecallSizeError(value.to_owned()))
            );
        }
    }

    /// Calendar days convert without drifting, leap day included.
    #[test]
    fn chrono_days_convert_exactly() {
        assert_eq!(
            to_date(NaiveDate::from_ymd_opt(2028, 2, 29).unwrap_or_default()),
            date!(2028 - 02 - 29)
        );
        assert_eq!(
            to_date(NaiveDate::from_ymd_opt(2026, 12, 31).unwrap_or_default()),
            date!(2026 - 12 - 31)
        );
    }
}
