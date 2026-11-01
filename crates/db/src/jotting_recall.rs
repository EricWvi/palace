//! 今日回响: the daily pile, drawn once from the jottings completed least often and stored for the
//! day, and the completions that move a jotting up. Palace's own state beside the Git index: it
//! never takes the publication lock and never changes a jotting's version.
//! See `specs/decisions/web/echo/20261101-recall-least-completed-first-from-a-stored-daily-pile.md`.
use crate::{Database, DbError, ListedJotting, jotting_read::LIVE};
use sqlx::Row;
use time::Date;
use uuid::Uuid;

// Serializes drawing, so two first requests of a day cannot both draw a pile.
const RECALL_LOCK: i64 = 734_281_003;

/// Whether a jotting is drawn into recall piles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecallState {
    Included,
    Excluded,
}

/// What completing a recall card goes on to do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecallNext {
    /// The jotting stays in recall.
    Keep,
    /// The jotting is not drawn again: 束阁.
    Retire,
}

/// One card of a day's pile.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecallCard {
    pub listed: ListedJotting,
    /// Completed on its day; the page no longer shows it.
    pub done: bool,
}

// The least count among jottings in recall. Only live jottings have a row (see the migration's
// trigger), and `min` skips the jottings out of recall.
const LEAST: &str = "coalesce((SELECT min(completions) FROM jotting_recall),0)";

impl Database {
    /// The pile of `day`, drawn on the first request of the day: up to `size` jottings created
    /// before `day_start` (epoch milliseconds), in recall, and not left unfinished in the
    /// previous pile, the least completed first and ties by `SHA-256("YYYY-MM-DD" ‖ id)`. A
    /// stored pile is returned as drawn, without the jottings deleted since. Drawing keeps only
    /// the new pile and the one before it.
    pub async fn jotting_recall(
        &self,
        day: Date,
        day_start: i64,
        size: i64,
    ) -> Result<Vec<RecallCard>, DbError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(RECALL_LOCK)
            .execute(&mut *tx)
            .await?;
        let drawn = sqlx::query(
            "INSERT INTO jotting_recall_pile(day) VALUES($1::date) ON CONFLICT DO NOTHING",
        )
        .bind(day.to_string())
        .execute(&mut *tx)
        .await?
        .rows_affected()
            == 1;
        if drawn {
            sqlx::query(
                "WITH previous AS (SELECT max(day) AS day FROM jotting_recall_pile WHERE day<$1::date),
                 skipped AS (SELECT c.jotting_id FROM jotting_recall_card c,previous p WHERE c.day=p.day AND c.done_at IS NULL),
                 ranked AS (SELECT j.id,row_number() OVER (ORDER BY r.completions,sha256(convert_to($1||j.id::text,'UTF8'))) AS n
                     FROM jotting j JOIN jotting_recall r ON r.jotting_id=j.id
                     WHERE r.completions IS NOT NULL
                     AND j.created_at<timestamptz 'epoch'+$2::bigint*interval '1 millisecond'
                     AND j.id NOT IN (SELECT jotting_id FROM skipped))
                 INSERT INTO jotting_recall_card(day,position,jotting_id)
                 SELECT $1::date,n-1,id FROM ranked WHERE n<=$3",
            )
            .bind(day.to_string())
            .bind(day_start)
            .bind(size)
            .execute(&mut *tx)
            .await?;
            // The previous pile is kept for the skip rule above and for cards completed after
            // midnight; anything older has no further use.
            sqlx::query("DELETE FROM jotting_recall_pile WHERE day<(SELECT max(day) FROM jotting_recall_pile WHERE day<$1::date)")
                .bind(day.to_string())
                .execute(&mut *tx)
                .await?;
        }
        let rows = sqlx::query(&format!(
            "SELECT {LIVE},c.done_at IS NOT NULL AS done FROM jotting_recall_card c JOIN jotting ON jotting.id=c.jotting_id WHERE c.day=$1::date AND jotting.state<>'deleted' ORDER BY c.position"
        ))
        .bind(day.to_string())
        .fetch_all(&mut *tx)
        .await?;
        tx.commit().await?;
        rows.iter()
            .map(|row| {
                Ok(RecallCard {
                    listed: crate::jotting_read::decode(row)?,
                    done: row.try_get("done")?,
                })
            })
            .collect()
    }

    /// Completes the card of `id` in the pile of `day`. Only the first completion counts; a
    /// retiring one takes the jotting out of recall even when the card was already done.
    /// `NotFound` when the pile no longer holds the card or the jotting is deleted.
    pub async fn jotting_recall_done(
        &self,
        day: Date,
        id: Uuid,
        next: RecallNext,
    ) -> Result<RecallState, DbError> {
        let mut tx = self.pool.begin().await?;
        // The row lock makes a concurrent completion wait and then find the card done.
        let first = sqlx::query("UPDATE jotting_recall_card c SET done_at=date_trunc('milliseconds',now()) FROM jotting j WHERE c.day=$1::date AND c.jotting_id=$2 AND j.id=c.jotting_id AND j.state<>'deleted' AND c.done_at IS NULL")
            .bind(day.to_string())
            .bind(id)
            .execute(&mut *tx)
            .await?
            .rows_affected()
            == 1;
        if !first {
            let held: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM jotting_recall_card c JOIN jotting j ON j.id=c.jotting_id WHERE c.day=$1::date AND c.jotting_id=$2 AND j.state<>'deleted')")
                .bind(day.to_string())
                .bind(id)
                .fetch_one(&mut *tx)
                .await?;
            if !held {
                return Err(DbError::NotFound);
            }
        }
        match next {
            RecallNext::Retire => {
                sqlx::query("UPDATE jotting_recall SET completions=NULL WHERE jotting_id=$1")
                    .bind(id)
                    .execute(&mut *tx)
                    .await?;
            }
            // A jotting taken out of recall from the wall while its card was in today's pile
            // stays out; completing the card does not count it back in.
            RecallNext::Keep if first => {
                sqlx::query("UPDATE jotting_recall SET completions=completions+1 WHERE jotting_id=$1 AND completions IS NOT NULL")
                    .bind(id)
                    .execute(&mut *tx)
                    .await?;
            }
            RecallNext::Keep => {}
        }
        let excluded: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM jotting_recall WHERE jotting_id=$1 AND completions IS NULL)",
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(if excluded {
            RecallState::Excluded
        } else {
            RecallState::Included
        })
    }

    /// Takes a live jotting out of recall, or back in at the least count. Taking part again
    /// only resets a jotting that was out; one already in keeps its count.
    pub async fn set_jotting_recall(&self, id: Uuid, state: RecallState) -> Result<(), DbError> {
        let statement = match state {
            RecallState::Excluded => {
                "UPDATE jotting_recall SET completions=NULL WHERE jotting_id=$1 RETURNING 1"
            }
            // The least is read before this row joins it, so the jotting lands on it exactly.
            RecallState::Included => &format!(
                "UPDATE jotting_recall SET completions=coalesce(completions,{LEAST}) WHERE jotting_id=$1 RETURNING 1"
            ),
        };
        // Only live jottings have a row, so no row means no such jotting.
        sqlx::query(statement)
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .map(|_| ())
            .ok_or(DbError::NotFound)
    }
}
