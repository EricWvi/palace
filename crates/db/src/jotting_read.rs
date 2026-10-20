//! Live reads of the jotting index for the web 回响 page: a filtered list, an overview and the
//! day's recall pile. Tombstones never appear. The web reads here instead of the offline sync
//! feed, so the list can say which jottings still sit in `inbox/`.
//! See `specs/decisions/web/echo/0-jotting-wall-with-daily-recall-pile.md` D8.
use crate::{Database, DbError, SearchTerm};
use palace_domain::{InputError, InputErrorKind, Jotting};
use sqlx::{Postgres, Row, Transaction, postgres::PgRow};
use std::{fmt, str::FromStr};
use time::Date;
use uuid::Uuid;

/// Size of the recall pile.
pub const RECALL_SIZE: i64 = 5;

/// A live jotting as the web shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListedJotting {
    pub jotting: Jotting,
    /// The file sits under `inbox/`: uploaded or committed, the agent has not sorted it yet.
    pub pending: bool,
}

/// The position after the last jotting of a page. The list order is `(created_at, id)`
/// descending, so the next page starts strictly below it and jottings written meanwhile cannot
/// shift it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JottingCursor {
    pub created_at: i64,
    pub id: Uuid,
}

impl fmt::Display for JottingCursor {
    /// Writes the opaque token clients send back unchanged.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}_{}", self.created_at, self.id)
    }
}

impl FromStr for JottingCursor {
    type Err = InputError;

    /// Accepts only tokens this server could have written.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let invalid = || InputError::new(InputErrorKind::Field, "cursor", "expected a list cursor");
        let (created_at, id) = value.split_once('_').ok_or_else(invalid)?;
        Ok(Self {
            created_at: created_at.parse().map_err(|_| invalid())?,
            id: id.parse().map_err(|_| invalid())?,
        })
    }
}

/// Which jottings to list and from where.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JottingListRequest {
    /// Only jottings carrying exactly this tag.
    pub tag: Option<String>,
    /// Only jottings whose title, body or a tag contains the term.
    pub search: Option<SearchTerm>,
    pub after: Option<JottingCursor>,
    pub limit: u32,
}

/// One page of the list.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JottingPage {
    pub jottings: Vec<ListedJotting>,
    /// Every jotting matching the filters, across all pages.
    pub total: i64,
    /// Present only when more jottings follow this page.
    pub next_cursor: Option<JottingCursor>,
}

/// Counts behind the left column of the 回响 page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JottingOverview {
    pub total: i64,
    /// Local days with at least one jotting.
    pub days: i64,
    /// Every tag, most used first, then by name.
    pub tags: Vec<TagCount>,
    /// One entry per local day of the requested range, empty days included.
    pub daily: Vec<DayCount>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TagCount {
    pub name: String,
    pub count: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DayCount {
    pub date: Date,
    pub count: i64,
}

const LIVE: &str = "id,(extract(epoch FROM created_at)*1000)::bigint AS created_at,title,tags,body,starts_with(path,'inbox/') AS pending";

// `$1` tag, `$2` ILIKE pattern; a null parameter disables its filter.
const FILTER: &str = "state<>'deleted' AND ($1::text IS NULL OR $1=ANY(tags)) AND ($2::text IS NULL OR title ILIKE $2 OR body ILIKE $2 OR EXISTS(SELECT 1 FROM unnest(tags) t WHERE t ILIKE $2))";

// Local calendar day of a jotting in the deployment's zone, passed as `$1`.
const LOCAL_DAY: &str = "(created_at AT TIME ZONE $1)::date";
// How PostgreSQL writes a `date` as text, and how `Date` writes itself.
const DAY: &[time::format_description::BorrowedFormatItem<'static>] =
    time::macros::format_description!("[year]-[month]-[day]");

impl Database {
    /// One page of live jottings, newest first, with the total matching the same filters.
    pub async fn jotting_list(&self, request: &JottingListRequest) -> Result<JottingPage, DbError> {
        let mut tx = read_only(&self.pool).await?;
        let pattern = request.search.as_ref().map(SearchTerm::like_pattern);
        let total: i64 =
            sqlx::query_scalar(&format!("SELECT count(*) FROM jotting WHERE {FILTER}"))
                .bind(&request.tag)
                .bind(&pattern)
                .fetch_one(&mut *tx)
                .await?;
        let rows = sqlx::query(&format!(
            "SELECT {LIVE} FROM jotting WHERE {FILTER} AND ($3::bigint IS NULL OR (created_at,id)<(timestamptz 'epoch'+$3::bigint*interval '1 millisecond',$4)) ORDER BY created_at DESC,id DESC LIMIT $5"
        ))
        .bind(&request.tag)
        .bind(&pattern)
        .bind(request.after.map(|cursor| cursor.created_at))
        .bind(request.after.map(|cursor| cursor.id))
        .bind(i64::from(request.limit) + 1)
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;

        let mut jottings = rows.iter().map(decode).collect::<Result<Vec<_>, _>>()?;
        let next_cursor = if jottings.len() > request.limit as usize {
            jottings.truncate(request.limit as usize);
            jottings.last().map(|last| JottingCursor {
                created_at: last.jotting.created_at,
                id: last.jotting.id,
            })
        } else {
            None
        };
        Ok(JottingPage {
            jottings,
            total,
            next_cursor,
        })
    }

    /// Totals, tag counts and per-day counts from `from` to `today`, days taken in `timezone`.
    pub async fn jotting_overview(
        &self,
        timezone: &str,
        from: Date,
        today: Date,
    ) -> Result<JottingOverview, DbError> {
        let mut tx = read_only(&self.pool).await?;
        let row = sqlx::query(&format!(
            "SELECT count(*) AS total,count(DISTINCT {LOCAL_DAY}) AS days FROM jotting WHERE state<>'deleted'"
        ))
        .bind(timezone)
        .fetch_one(&mut *tx)
        .await?;
        let (total, days) = (row.try_get("total")?, row.try_get("days")?);
        // Byte order breaks ties, so the order never depends on the database's collation.
        let tags = sqlx::query("SELECT t AS name,count(*) AS count FROM jotting,unnest(tags) t WHERE state<>'deleted' GROUP BY t ORDER BY count(*) DESC,t COLLATE \"C\"")
            .fetch_all(&mut *tx)
            .await?
            .into_iter()
            .map(|row| {
                Ok(TagCount {
                    name: row.try_get("name")?,
                    count: row.try_get("count")?,
                })
            })
            .collect::<Result<Vec<_>, DbError>>()?;
        let daily = sqlx::query(
            "SELECT d::date::text AS date,count(j.id) AS count FROM generate_series($2::date,$3::date,interval '1 day') d LEFT JOIN jotting j ON j.state<>'deleted' AND (j.created_at AT TIME ZONE $1)::date=d::date GROUP BY d ORDER BY d"
        )
        .bind(timezone)
        .bind(from.to_string())
        .bind(today.to_string())
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .map(|row| {
            let date: String = row.try_get("date")?;
            Ok(DayCount {
                date: Date::parse(&date, DAY).map_err(|_| DbError::Conflict)?,
                count: row.try_get("count")?,
            })
        })
        .collect::<Result<Vec<_>, DbError>>()?;
        tx.commit().await?;
        Ok(JottingOverview {
            total,
            days,
            tags,
            daily,
        })
    }

    /// The recall pile of `today`: jottings created before `day_start` (epoch milliseconds of
    /// the local midnight), ordered by `SHA-256("YYYY-MM-DD" ‖ id)`. Nothing is stored, yet the
    /// same day and the same candidates always give the same pile in the same order.
    pub async fn jotting_recall(
        &self,
        today: Date,
        day_start: i64,
    ) -> Result<Vec<ListedJotting>, DbError> {
        let rows = sqlx::query(&format!(
            "SELECT {LIVE} FROM jotting WHERE state<>'deleted' AND created_at<timestamptz 'epoch'+$2::bigint*interval '1 millisecond' ORDER BY sha256(convert_to($1||id::text,'UTF8')) LIMIT $3"
        ))
        .bind(today.to_string())
        .bind(day_start)
        .bind(RECALL_SIZE)
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(decode).collect()
    }
}

/// Opens a read-only snapshot, so a count and the rows it describes see the same moment.
async fn read_only(pool: &sqlx::PgPool) -> Result<Transaction<'_, Postgres>, DbError> {
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
        .execute(&mut *tx)
        .await?;
    Ok(tx)
}

/// Reads a row selected with [`LIVE`].
fn decode(row: &PgRow) -> Result<ListedJotting, DbError> {
    Ok(ListedJotting {
        jotting: Jotting {
            id: row.try_get("id")?,
            created_at: row.try_get("created_at")?,
            title: row.try_get("title")?,
            tags: row.try_get("tags")?,
            body: row.try_get("body")?,
        },
        pending: row.try_get("pending")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    /// A cursor round-trips through its token, and anything else is a field error.
    #[test]
    fn cursors_round_trip_and_reject_foreign_tokens() {
        let cursor = JottingCursor {
            created_at: 1_792_369_800_000,
            id: Uuid::from_u128(7),
        };
        assert_eq!(cursor.to_string().parse::<JottingCursor>(), Ok(cursor));
        for token in [
            "",
            "123",
            "abc_00000000-0000-0000-0000-000000000007",
            "1_xyz",
        ] {
            assert_eq!(
                token.parse::<JottingCursor>(),
                Err(InputError::new(
                    InputErrorKind::Field,
                    "cursor",
                    "expected a list cursor"
                ))
            );
        }
    }
}
