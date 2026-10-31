//! Owner-scoped 工夫 of 时刻: the owner's activity types, and activities, each an `activity`
//! moment at its start. Activities are outside the sync domain, so these writes take no
//! publication lock.
use crate::{ActivityCard, Database, DbError, OwnerScope};
use palace_domain::{ActivityInput, ActivityTypeName, InstantRange};
use sqlx::{Row, postgres::PgRow};
use std::collections::HashMap;
use uuid::Uuid;

/// A kind of activity the owner named.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActivityType {
    pub id: Uuid,
    pub name: String,
}

/// Whether adding a type made a new one or found one by the same name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TypeAdded {
    Created,
    Existing,
}

/// One activity; its start is its moment's place on the timeline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Activity {
    pub id: Uuid,
    pub type_id: Uuid,
    pub type_name: String,
    pub started_at: i64,
    pub minutes: i32,
    pub note: Option<String>,
}

const COLUMNS: &str = "a.id,a.type_id,t.name AS type_name,(extract(epoch FROM m.occurred_at)*1000)::bigint AS started_at,a.minutes,a.note";
const JOINS: &str = "moment m JOIN activity a ON a.owner_id=m.owner_id AND a.id=m.id JOIN activity_type t ON t.owner_id=a.owner_id AND t.id=a.type_id";
// Exact epoch-millisecond conversion; to_timestamp's float could round.
const START: &str = "(timestamptz 'epoch' + $3::bigint * interval '1 millisecond')";

impl Database {
    /// The owner's activity types in the order they were added.
    pub async fn activity_types(&self, owner: OwnerScope) -> Result<Vec<ActivityType>, DbError> {
        sqlx::query("SELECT id,name FROM activity_type WHERE owner_id=$1 ORDER BY created_at,id")
            .bind(owner.id())
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(activity_type)
            .collect()
    }

    /// Adds a type, or finds the one already named so; a repeated request never makes a second.
    pub async fn add_activity_type(
        &self,
        owner: OwnerScope,
        name: &ActivityTypeName,
    ) -> Result<(ActivityType, TypeAdded), DbError> {
        let created = sqlx::query("INSERT INTO activity_type(id,owner_id,name) VALUES($1,$2,$3) ON CONFLICT(owner_id,name) DO NOTHING RETURNING id,name")
            .bind(Uuid::now_v7())
            .bind(owner.id())
            .bind(name.as_str())
            .fetch_optional(&self.pool)
            .await?;
        if let Some(row) = created {
            return Ok((activity_type(&row)?, TypeAdded::Created));
        }
        let row = sqlx::query("SELECT id,name FROM activity_type WHERE owner_id=$1 AND name=$2")
            .bind(owner.id())
            .bind(name.as_str())
            .fetch_one(&self.pool)
            .await?;
        Ok((activity_type(&row)?, TypeAdded::Existing))
    }

    /// The activities starting in the range, earliest first, read through the timeline's index.
    pub async fn activities(
        &self,
        owner: OwnerScope,
        range: InstantRange,
    ) -> Result<Vec<Activity>, DbError> {
        sqlx::query(&format!(
            "SELECT {COLUMNS} FROM {JOINS} WHERE m.owner_id=$1 AND m.kind='activity' AND m.occurred_at>=(timestamptz 'epoch' + $2::bigint * interval '1 millisecond') AND m.occurred_at<{START} ORDER BY m.occurred_at,m.id"
        ))
        .bind(owner.id())
        .bind(range.start())
        .bind(range.end())
        .fetch_all(&self.pool)
        .await?
        .iter()
        .map(activity)
        .collect()
    }

    /// Records an activity as a moment at its start and its detail, in one transaction.
    pub async fn record_activity(
        &self,
        owner: OwnerScope,
        input: &ActivityInput,
    ) -> Result<Activity, DbError> {
        let id = Uuid::now_v7();
        let mut tx = self.pool.begin().await?;
        sqlx::query(&format!(
            "INSERT INTO moment(id,owner_id,kind,occurred_at) VALUES($1,$2,'activity',{START})"
        ))
        .bind(id)
        .bind(owner.id())
        .bind(input.started_at())
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO activity(id,owner_id,type_id,minutes,note) VALUES($1,$2,$3,$4,$5)",
        )
        .bind(id)
        .bind(owner.id())
        .bind(input.type_id())
        .bind(input.minutes())
        .bind(input.note())
        .execute(&mut *tx)
        .await
        .map_err(write_error)?;
        let recorded = read_activity(&mut tx, owner, id).await?;
        tx.commit().await?;
        Ok(recorded)
    }

    /// Corrects an activity; its moment moves with the start, keeping its identity.
    pub async fn update_activity(
        &self,
        owner: OwnerScope,
        id: Uuid,
        input: &ActivityInput,
    ) -> Result<Activity, DbError> {
        let mut tx = self.pool.begin().await?;
        let updated = sqlx::query(
            "UPDATE activity SET type_id=$3,minutes=$4,note=$5 WHERE owner_id=$1 AND id=$2",
        )
        .bind(owner.id())
        .bind(id)
        .bind(input.type_id())
        .bind(input.minutes())
        .bind(input.note())
        .execute(&mut *tx)
        .await
        .map_err(write_error)?;
        if updated.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }
        sqlx::query(&format!(
            "UPDATE moment SET occurred_at={START} WHERE owner_id=$1 AND id=$2 AND kind='activity'"
        ))
        .bind(owner.id())
        .bind(id)
        .bind(input.started_at())
        .execute(&mut *tx)
        .await?;
        let updated = read_activity(&mut tx, owner, id).await?;
        tx.commit().await?;
        Ok(updated)
    }

    /// Deletes an activity together with its moment.
    pub async fn delete_activity(&self, owner: OwnerScope, id: Uuid) -> Result<(), DbError> {
        let mut tx = self.pool.begin().await?;
        let deleted = sqlx::query("DELETE FROM activity WHERE owner_id=$1 AND id=$2")
            .bind(owner.id())
            .bind(id)
            .execute(&mut *tx)
            .await?;
        if deleted.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }
        sqlx::query("DELETE FROM moment WHERE owner_id=$1 AND id=$2 AND kind='activity'")
            .bind(owner.id())
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }
}

/// Reads the card fields of many activity moments in one query.
pub(crate) async fn activity_cards(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: OwnerScope,
    ids: &[Uuid],
) -> Result<HashMap<Uuid, ActivityCard>, DbError> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    sqlx::query("SELECT a.id,a.type_id,t.name AS type_name,a.minutes,a.note FROM activity a JOIN activity_type t ON t.owner_id=a.owner_id AND t.id=a.type_id WHERE a.owner_id=$1 AND a.id=ANY($2)")
        .bind(owner.id())
        .bind(ids)
        .fetch_all(&mut **tx)
        .await?
        .iter()
        .map(|row| {
            Ok((
                row.try_get("id")?,
                ActivityCard {
                    type_id: row.try_get("type_id")?,
                    type_name: row.try_get("type_name")?,
                    minutes: row.try_get("minutes")?,
                    note: row.try_get("note")?,
                },
            ))
        })
        .collect()
}

async fn read_activity(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    owner: OwnerScope,
    id: Uuid,
) -> Result<Activity, DbError> {
    let row = sqlx::query(&format!(
        "SELECT {COLUMNS} FROM {JOINS} WHERE m.owner_id=$1 AND m.id=$2"
    ))
    .bind(owner.id())
    .bind(id)
    .fetch_one(&mut **tx)
    .await?;
    activity(&row)
}

fn activity_type(row: &PgRow) -> Result<ActivityType, DbError> {
    Ok(ActivityType {
        id: row.try_get("id")?,
        name: row.try_get("name")?,
    })
}

fn activity(row: &PgRow) -> Result<Activity, DbError> {
    Ok(Activity {
        id: row.try_get("id")?,
        type_id: row.try_get("type_id")?,
        type_name: row.try_get("type_name")?,
        started_at: row.try_get("started_at")?,
        minutes: row.try_get("minutes")?,
        note: row.try_get("note")?,
    })
}

/// A type that is not the owner's breaks the composite key; to the caller it does not exist.
fn write_error(error: sqlx::Error) -> DbError {
    match error
        .as_database_error()
        .and_then(sqlx::error::DatabaseError::code)
        .as_deref()
    {
        Some("23503") => DbError::NotFound,
        _ => error.into(),
    }
}
