//! Owner-scoped 日课 of 时刻: habits, and how many times each was done on a local day. Habits are
//! outside the sync domain, so these writes take no publication lock.
use crate::{Database, DbError, OwnerScope};
use palace_domain::{HabitCount, HabitDays, HabitInput, InputError, InputErrorKind, LocalDay};
use sqlx::{Row, postgres::PgRow};
use uuid::Uuid;

/// A habit as the page lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Habit {
    pub id: Uuid,
    pub name: String,
    pub mark: String,
    pub times_per_day: i16,
    /// Unix epoch milliseconds; the history counts its rate from this day on.
    pub created_at: i64,
    pub retired: bool,
}

/// How many times a habit was done on one local day; never zero.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HabitCheck {
    pub habit_id: Uuid,
    pub day: LocalDay,
    pub count: i16,
}

/// The habits a run of days shows and their checks on those days.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HabitBoard {
    /// Every habit still in use, and retired ones with checks in the run, in creation order.
    pub habits: Vec<Habit>,
    /// Ordered by day, then by habit.
    pub checks: Vec<HabitCheck>,
}

const COLUMNS: &str = "id,name,mark,times_per_day,(extract(epoch FROM created_at)*1000)::bigint AS created_at,retired_at IS NOT NULL AS retired";

impl Database {
    /// Reads the habits and checks of a run of local days from one snapshot, so no check names a
    /// habit the list leaves out.
    pub async fn habit_board(
        &self,
        owner: OwnerScope,
        days: HabitDays,
    ) -> Result<HabitBoard, DbError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
            .execute(&mut *tx)
            .await?;
        let habits = sqlx::query(&format!(
            "SELECT {COLUMNS} FROM habit h WHERE owner_id=$1 AND (retired_at IS NULL OR EXISTS(SELECT 1 FROM habit_check c WHERE c.owner_id=$1 AND c.habit_id=h.id AND c.day BETWEEN $2::date AND $3::date)) ORDER BY created_at,id"
        ))
        .bind(owner.id())
        .bind(days.from().text())
        .bind(days.to().text())
        .fetch_all(&mut *tx)
        .await?
        .iter()
        .map(habit)
        .collect::<Result<_, _>>()?;
        let checks = sqlx::query("SELECT habit_id,to_char(day,'YYYY-MM-DD') AS day,count FROM habit_check WHERE owner_id=$1 AND day BETWEEN $2::date AND $3::date ORDER BY day,habit_id")
            .bind(owner.id())
            .bind(days.from().text())
            .bind(days.to().text())
            .fetch_all(&mut *tx)
            .await?
            .iter()
            .map(|row| {
                Ok(HabitCheck {
                    habit_id: row.try_get("habit_id")?,
                    day: LocalDay::from_stored(&row.try_get::<String, _>("day")?)?,
                    count: row.try_get("count")?,
                })
            })
            .collect::<Result<_, DbError>>()?;
        tx.commit().await?;
        Ok(HabitBoard { habits, checks })
    }

    /// Adds a habit after the existing ones.
    pub async fn insert_habit(
        &self,
        owner: OwnerScope,
        input: &HabitInput,
    ) -> Result<Habit, DbError> {
        let row = sqlx::query(&format!(
            "INSERT INTO habit(id,owner_id,name,mark,times_per_day) VALUES($1,$2,$3,$4,$5) RETURNING {COLUMNS}"
        ))
        .bind(Uuid::now_v7())
        .bind(owner.id())
        .bind(input.name.as_str())
        .bind(input.mark.text())
        .bind(i16::from(input.times_per_day.get()))
        .fetch_one(&self.pool)
        .await?;
        habit(&row)
    }

    /// Renames a habit, changes its seal or its times a day. Existing checks stay as they were,
    /// even above a lowered times a day; a retired habit takes no edits.
    pub async fn update_habit(
        &self,
        owner: OwnerScope,
        id: Uuid,
        input: &HabitInput,
    ) -> Result<Habit, DbError> {
        let row = sqlx::query(&format!(
            "UPDATE habit SET name=$3,mark=$4,times_per_day=$5 WHERE owner_id=$1 AND id=$2 AND retired_at IS NULL RETURNING {COLUMNS}"
        ))
        .bind(owner.id())
        .bind(id)
        .bind(input.name.as_str())
        .bind(input.mark.text())
        .bind(i16::from(input.times_per_day.get()))
        .fetch_optional(&self.pool)
        .await?
        .ok_or(DbError::NotFound)?;
        habit(&row)
    }

    /// Retires a habit; its checks stay. Retiring it again changes nothing.
    pub async fn retire_habit(&self, owner: OwnerScope, id: Uuid) -> Result<(), DbError> {
        let found = sqlx::query("UPDATE habit SET retired_at=COALESCE(retired_at,date_trunc('milliseconds',now())) WHERE owner_id=$1 AND id=$2")
            .bind(owner.id())
            .bind(id)
            .execute(&self.pool)
            .await?;
        if found.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }
        Ok(())
    }

    /// Sets how many times a habit was done on `day`; zero clears the day. The value is set, not
    /// added, so a retried request leaves the same count.
    pub async fn set_habit_check(
        &self,
        owner: OwnerScope,
        id: Uuid,
        day: LocalDay,
        count: HabitCount,
    ) -> Result<Option<HabitCheck>, DbError> {
        let mut tx = self.pool.begin().await?;
        // Shared lock: an edit lowering the times a day waits until this check is written.
        let times: i16 = sqlx::query_scalar("SELECT times_per_day FROM habit WHERE owner_id=$1 AND id=$2 AND retired_at IS NULL FOR SHARE")
            .bind(owner.id())
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(DbError::NotFound)?;
        let count = i16::from(count.get());
        if count > times {
            return Err(InputError::new(
                InputErrorKind::Field,
                "count",
                format!("expected at most {times} times, the habit's times a day"),
            )
            .into());
        }
        let check = if count == 0 {
            sqlx::query(
                "DELETE FROM habit_check WHERE owner_id=$1 AND habit_id=$2 AND day=$3::date",
            )
            .bind(owner.id())
            .bind(id)
            .bind(day.text())
            .execute(&mut *tx)
            .await?;
            None
        } else {
            sqlx::query("INSERT INTO habit_check(owner_id,habit_id,day,count) VALUES($1,$2,$3::date,$4) ON CONFLICT(owner_id,habit_id,day) DO UPDATE SET count=EXCLUDED.count")
                .bind(owner.id())
                .bind(id)
                .bind(day.text())
                .bind(count)
                .execute(&mut *tx)
                .await?;
            Some(HabitCheck {
                habit_id: id,
                day,
                count,
            })
        };
        tx.commit().await?;
        Ok(check)
    }
}

fn habit(row: &PgRow) -> Result<Habit, DbError> {
    Ok(Habit {
        id: row.try_get("id")?,
        name: row.try_get("name")?,
        mark: row.try_get("mark")?,
        times_per_day: row.try_get("times_per_day")?,
        created_at: row.try_get("created_at")?,
        retired: row.try_get("retired")?,
    })
}
