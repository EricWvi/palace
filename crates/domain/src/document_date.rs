//! The date a document carries: a note's frontmatter `date` or an article's publish date.
//! Writers give either a bare day or an exact time, and the difference is kept so a bare day
//! is shown back as a day instead of as a midnight nobody wrote.
use chrono::{DateTime, NaiveDate, SecondsFormat, TimeZone};
use chrono_tz::Tz;

/// How exactly the writer gave the date.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DatePrecision {
    /// `YYYY-MM-DD`, read as the start of that day in the deployment's zone.
    Day,
    /// An RFC 3339 time with its own offset.
    Instant,
}

/// A document date resolved to a point in time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DocumentDate {
    /// Unix epoch milliseconds; finer digits of an RFC 3339 time are dropped.
    pub at: i64,
    pub precision: DatePrecision,
}

impl DocumentDate {
    /// Reads `YYYY-MM-DD` as midnight in `timezone`, or an RFC 3339 time with its own offset.
    pub fn parse(text: &str, timezone: Tz) -> Option<Self> {
        if text.len() == 10 {
            let day = NaiveDate::parse_from_str(text, "%Y-%m-%d").ok()?;
            let midnight = day.and_hms_opt(0, 0, 0)?;
            // Where midnight falls into a daylight-saving gap the day starts at its first instant.
            let start = timezone
                .from_local_datetime(&midnight)
                .earliest()
                .or_else(|| {
                    (1..=24).find_map(|hour| {
                        timezone
                            .from_local_datetime(&(midnight + chrono::Duration::hours(hour)))
                            .earliest()
                    })
                })?;
            return Some(Self {
                at: start.timestamp_millis(),
                precision: DatePrecision::Day,
            });
        }
        DateTime::parse_from_rfc3339(text).ok().map(|time| Self {
            at: time.timestamp_millis(),
            precision: DatePrecision::Instant,
        })
    }

    /// Writes the date back in the form [`DocumentDate::parse`] accepts: the day in `timezone`
    /// for a bare day, otherwise the time at `timezone`'s offset. The writer's original offset
    /// is not kept, so an instant comes back as the same moment in the deployment's zone.
    pub fn to_text(self, timezone: Tz) -> String {
        let Some(time) = timezone.timestamp_millis_opt(self.at).single() else {
            // Stored dates always came through `parse`, which only yields representable times.
            return self.at.to_string();
        };
        match self.precision {
            DatePrecision::Day => time.format("%Y-%m-%d").to_string(),
            DatePrecision::Instant => time.to_rfc3339_opts(SecondsFormat::AutoSi, false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    const SHANGHAI: Tz = chrono_tz::Asia::Shanghai;

    /// A bare day is midnight in the deployment's zone; a time keeps its own offset and loses
    /// digits below the millisecond; anything else is unreadable.
    /// Core test case:
    /// - `specs/test-cases/web/stars/articles.md#article-import-must-commit-the-article-and-its-media-together-or-not-at-all`
    #[test]
    fn dates_resolve_by_their_precision() {
        assert_eq!(
            DocumentDate::parse("2026-10-05", SHANGHAI),
            Some(DocumentDate {
                at: 1_791_129_600_000,
                precision: DatePrecision::Day,
            })
        );
        assert_eq!(
            DocumentDate::parse("2026-10-04T16:00:00.1239Z", SHANGHAI),
            Some(DocumentDate {
                at: 1_791_129_600_123,
                precision: DatePrecision::Instant,
            })
        );
        for unreadable in ["2026-02-30", "2026-10-05T08:00:00", "yesterday", ""] {
            assert_eq!(
                DocumentDate::parse(unreadable, SHANGHAI),
                None,
                "{unreadable}"
            );
        }
    }

    /// Text written back parses to the same date with the same precision.
    /// Core test cases:
    /// - `specs/test-cases/web/stars/articles.md#article-import-must-commit-the-article-and-its-media-together-or-not-at-all`
    /// - `specs/test-cases/web/stars/articles.md#editing-an-article-must-replace-only-its-metadata`
    #[test]
    fn dates_round_trip_through_their_text() {
        for (text, expected) in [
            ("2026-10-05", "2026-10-05"),
            ("2026-10-04T16:00:00Z", "2026-10-05T00:00:00+08:00"),
            ("2026-10-04T16:00:00.5Z", "2026-10-05T00:00:00.500+08:00"),
        ] {
            let date = DocumentDate::parse(text, SHANGHAI).unwrap();
            assert_eq!(date.to_text(SHANGHAI), expected);
            assert_eq!(DocumentDate::parse(expected, SHANGHAI), Some(date));
        }
    }

    /// Where midnight does not exist the day starts at its first instant.
    #[test]
    fn days_starting_in_a_daylight_saving_gap_begin_at_their_first_instant() {
        // Havana skips from 00:00 to 01:00 on 2026-03-08.
        let date = DocumentDate::parse("2026-03-08", chrono_tz::America::Havana).unwrap();
        assert_eq!(date.to_text(chrono_tz::America::Havana), "2026-03-08");
        assert_eq!(date.at, 1_772_946_000_000);
    }
}
