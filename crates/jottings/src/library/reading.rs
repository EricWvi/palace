//! Live reads for the web 回响 page. The index answers them directly; what this layer adds is the
//! deployment's calendar: which local day it is, and when that day began.
use super::{Jottings, JottingsError};
use chrono::{Datelike, NaiveDate, NaiveTime, TimeZone};
use palace_db::{JottingListRequest, JottingOverview, JottingPage, ListedJotting};
use palace_domain::{InputError, InputErrorKind};
use time::{Date, Month};

/// The longest overview range: a year, leap day included. A heatmap needs twelve weeks; the
/// bound only keeps one request from asking for every day since year 1.
const MAX_OVERVIEW_DAYS: i64 = 366;

/// The day's recall pile and the local day it belongs to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Recall {
    pub date: Date,
    pub jottings: Vec<ListedJotting>,
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

    /// The recall pile of the local day of `now` (epoch milliseconds). Jottings created that day
    /// are not candidates, so writing one never reshuffles the pile.
    pub async fn recall(&self, now: i64) -> Result<Recall, JottingsError> {
        let date = self.local_day(now);
        let jottings = self
            .database
            .jotting_recall(date, self.day_start(date))
            .await?;
        Ok(Recall { date, jottings })
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
