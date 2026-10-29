//! Owner-scoped sleep of 时刻: the one running start per owner, and finished nights, each a
//! `sleep` moment on the day it ended. Sleep is outside the sync domain, so these writes take no
//! publication lock; the database itself keeps nights from overlapping.
use crate::{Database, DbError, OwnerScope};
use palace_domain::{SleepRange, SleepSpan};
use sqlx::{Row, postgres::PgRow};
use uuid::Uuid;

/// One finished night; its wake time is its moment's place on the timeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SleepNight {
    pub id: Uuid,
    pub started_at: i64,
    pub ended_at: i64,
}

// Exact epoch-millisecond conversions in both directions; to_timestamp's float could round.
const NIGHT_COLUMNS: &str = "s.id,(extract(epoch FROM lower(s.span))*1000)::bigint AS started_at,(extract(epoch FROM upper(s.span))*1000)::bigint AS ended_at";
/// The timestamp bind parameter `$n` holds as epoch milliseconds.
fn at(n: u8) -> String {
    format!("(timestamptz 'epoch' + ${n}::bigint * interval '1 millisecond')")
}

impl Database {
    /// The running start in epoch milliseconds, or none.
    pub async fn running_sleep(&self, owner: OwnerScope) -> Result<Option<i64>, DbError> {
        Ok(sqlx::query_scalar(
            "SELECT (extract(epoch FROM started_at)*1000)::bigint FROM sleep_start WHERE owner_id=$1",
        )
        .bind(owner.id())
        .fetch_optional(&self.pool)
        .await?)
    }

    /// Starts sleeping now by the server's clock. A start already running, ended or never, is
    /// replaced, which is how a forgotten start finally goes away.
    pub async fn start_sleep(&self, owner: OwnerScope) -> Result<i64, DbError> {
        Ok(sqlx::query_scalar("INSERT INTO sleep_start(owner_id,started_at) VALUES($1,date_trunc('milliseconds',now())) ON CONFLICT(owner_id) DO UPDATE SET started_at=EXCLUDED.started_at RETURNING (extract(epoch FROM started_at)*1000)::bigint")
            .bind(owner.id())
            .fetch_one(&self.pool)
            .await?)
    }

    /// Ends the running start the caller saw, at the server's now, and files the night under the
    /// day it ends. A start the caller did not see is left alone and reads as not found; ending
    /// the same start again returns the night it already became.
    pub async fn end_sleep(
        &self,
        owner: OwnerScope,
        started_at: i64,
    ) -> Result<SleepNight, DbError> {
        let mut tx = self.pool.begin().await?;
        let now: Option<i64> = sqlx::query_scalar(&format!(
            "DELETE FROM sleep_start WHERE owner_id=$1 AND started_at={} RETURNING (extract(epoch FROM date_trunc('milliseconds',now()))*1000)::bigint",
            at(2)
        ))
        .bind(owner.id())
        .bind(started_at)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(now) = now else {
            let night = sqlx::query(&format!(
                "SELECT {NIGHT_COLUMNS} FROM sleep s WHERE s.owner_id=$1 AND lower(s.span)={}",
                at(2)
            ))
            .bind(owner.id())
            .bind(started_at)
            .fetch_optional(&mut *tx)
            .await?;
            return night
                .as_ref()
                .map(night_from)
                .unwrap_or(Err(DbError::NotFound));
        };
        // A start over a day old is one someone forgot; refusing it keeps the start, which the
        // next start replaces.
        let span = SleepSpan::parse(started_at, now)?;
        let night = insert_night(&mut tx, owner, Uuid::now_v7(), span).await?;
        tx.commit().await.map_err(write_error)?;
        Ok(night)
    }

    /// Records a night that was never started, or one filled in from the history.
    pub async fn record_sleep_night(
        &self,
        owner: OwnerScope,
        span: SleepSpan,
    ) -> Result<SleepNight, DbError> {
        let mut tx = self.pool.begin().await?;
        let night = insert_night(&mut tx, owner, Uuid::now_v7(), span).await?;
        tx.commit().await.map_err(write_error)?;
        Ok(night)
    }

    /// Moves a night to a new span; its moment follows the wake time to its day, same identity.
    pub async fn update_sleep_night(
        &self,
        owner: OwnerScope,
        id: Uuid,
        span: SleepSpan,
    ) -> Result<SleepNight, DbError> {
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query(&format!(
            "UPDATE sleep s SET span=tstzrange({},{},'[)') WHERE s.owner_id=$1 AND s.id=$2 RETURNING {NIGHT_COLUMNS}",
            at(3),
            at(4)
        ))
        .bind(owner.id())
        .bind(id)
        .bind(span.started_at())
        .bind(span.ended_at())
        .fetch_optional(&mut *tx)
        .await
        .map_err(write_error)?
        .ok_or(DbError::NotFound)?;
        sqlx::query(&format!(
            "UPDATE moment SET occurred_at={} WHERE owner_id=$1 AND id=$2 AND kind='sleep'",
            at(3)
        ))
        .bind(owner.id())
        .bind(id)
        .bind(span.ended_at())
        .execute(&mut *tx)
        .await?;
        let night = night_from(&row)?;
        tx.commit().await.map_err(write_error)?;
        Ok(night)
    }

    /// Deletes a night together with its moment.
    pub async fn delete_sleep_night(&self, owner: OwnerScope, id: Uuid) -> Result<(), DbError> {
        let mut tx = self.pool.begin().await?;
        let deleted = sqlx::query("DELETE FROM sleep WHERE owner_id=$1 AND id=$2")
            .bind(owner.id())
            .bind(id)
            .execute(&mut *tx)
            .await?;
        if deleted.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }
        sqlx::query("DELETE FROM moment WHERE owner_id=$1 AND id=$2 AND kind='sleep'")
            .bind(owner.id())
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// The nights whose wake time falls in the range, earliest wake first. Reads through the
    /// timeline's own index, since a night's wake time is its moment's time.
    pub async fn sleep_nights(
        &self,
        owner: OwnerScope,
        range: SleepRange,
    ) -> Result<Vec<SleepNight>, DbError> {
        let rows = sqlx::query(&format!(
            "SELECT {NIGHT_COLUMNS} FROM moment m JOIN sleep s ON s.owner_id=m.owner_id AND s.id=m.id WHERE m.owner_id=$1 AND m.kind='sleep' AND m.occurred_at>={} AND m.occurred_at<{} ORDER BY m.occurred_at,m.id",
            at(2),
            at(3)
        ))
        .bind(owner.id())
        .bind(range.start())
        .bind(range.end())
        .fetch_all(&self.pool)
        .await?;
        rows.iter().map(night_from).collect()
    }
}

/// Writes a night's moment and detail; the commit check holds the two to the same wake time.
async fn insert_night(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: OwnerScope,
    id: Uuid,
    span: SleepSpan,
) -> Result<SleepNight, DbError> {
    sqlx::query(&format!(
        "INSERT INTO moment(id,owner_id,kind,occurred_at) VALUES($1,$2,'sleep',{})",
        at(3)
    ))
    .bind(id)
    .bind(owner.id())
    .bind(span.ended_at())
    .execute(&mut **tx)
    .await?;
    sqlx::query(&format!(
        "INSERT INTO sleep(id,owner_id,span) VALUES($1,$2,tstzrange({},{},'[)'))",
        at(3),
        at(4)
    ))
    .bind(id)
    .bind(owner.id())
    .bind(span.started_at())
    .bind(span.ended_at())
    .execute(&mut **tx)
    .await
    .map_err(write_error)?;
    Ok(SleepNight {
        id,
        started_at: span.started_at(),
        ended_at: span.ended_at(),
    })
}

fn night_from(row: &PgRow) -> Result<SleepNight, DbError> {
    Ok(SleepNight {
        id: row.try_get("id")?,
        started_at: row.try_get("started_at")?,
        ended_at: row.try_get("ended_at")?,
    })
}

/// An overlapping night is a conflict the reader can resolve, not a server failure.
fn write_error(error: sqlx::Error) -> DbError {
    match error
        .as_database_error()
        .and_then(sqlx::error::DatabaseError::code)
        .as_deref()
    {
        Some("23P01") => DbError::Conflict,
        _ => error.into(),
    }
}
