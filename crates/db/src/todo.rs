//! Owner-scoped tasks of 行事. Tasks are outside the sync domain, so these writes take no
//! publication lock. Every statement keys on the owner, and a task's list is checked to be the
//! owner's in the same statement that writes it.
use crate::{Database, DbError, OwnerScope};
use palace_domain::{
    TodoBody, TodoDay, TodoLink, TodoPlacement, TodoPlan, TodoPosition, TodoTitle,
};
use sqlx::{Row, postgres::PgRow};
use uuid::Uuid;

/// Where a stored task sits: the inbox holds no plan, so only a list carries one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TodoPlace {
    Inbox,
    List { list: Uuid, plan: TodoPlan },
}

/// An open task as it may leave the server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Todo {
    pub id: Uuid,
    pub place: TodoPlace,
    pub title: String,
    pub position: TodoPosition,
    pub link: Option<String>,
    /// The body itself is read on its own; lists only say whether there is one.
    pub has_body: bool,
    pub done_days: i32,
}

/// What an edit replaces together.
pub struct TodoEdit {
    pub placement: TodoPlacement,
    pub title: TodoTitle,
    pub link: Option<TodoLink>,
    pub position: TodoPosition,
}

pub(crate) const COLUMNS: &str = "id,list_id,title,position,link,body IS NOT NULL AS has_body,to_char(planned_on,'YYYY-MM-DD') AS planned_on,shelved,done,done_days";

impl Database {
    /// Adds a task where the placement puts it. A list of another owner writes nothing and is
    /// not found.
    pub async fn insert_todo(
        &self,
        owner: OwnerScope,
        placement: &TodoPlacement,
        title: &TodoTitle,
        position: &TodoPosition,
    ) -> Result<Todo, DbError> {
        let row = sqlx::query(&format!(
            "INSERT INTO todo(id,owner_id,list_id,title,position,planned_on,shelved) SELECT $1,$2,$3,$4,$5,$6::date,$7 WHERE $3::uuid IS NULL OR EXISTS(SELECT 1 FROM todo_list WHERE owner_id=$2 AND id=$3) RETURNING {COLUMNS}"
        ))
        .bind(Uuid::now_v7())
        .bind(owner.id())
        .bind(placement.list())
        .bind(title.as_str())
        .bind(position.as_str())
        .bind(placement.day().map(TodoDay::as_str))
        .bind(placement.shelved())
        .fetch_optional(&self.pool)
        .await
        .map_err(write_error)?
        .ok_or(DbError::NotFound)?;
        todo(&row)
    }

    /// Replaces an open task's list, title, link, position and plan. Done survives only when the
    /// planned day stays the same; a finished task takes no edits.
    pub async fn update_todo(
        &self,
        owner: OwnerScope,
        id: Uuid,
        edit: &TodoEdit,
    ) -> Result<Todo, DbError> {
        let row = sqlx::query(&format!(
            "UPDATE todo SET list_id=$3,title=$4,link=$5,position=$6,planned_on=$7::date,shelved=$8,done=done AND planned_on IS NOT DISTINCT FROM $7::date,updated_at=date_trunc('milliseconds',now()) WHERE owner_id=$1 AND id=$2 AND completed_at IS NULL AND ($3::uuid IS NULL OR EXISTS(SELECT 1 FROM todo_list WHERE owner_id=$1 AND id=$3)) RETURNING {COLUMNS}"
        ))
        .bind(owner.id())
        .bind(id)
        .bind(edit.placement.list())
        .bind(edit.title.as_str())
        .bind(edit.link.as_ref().map(TodoLink::as_str))
        .bind(edit.position.as_str())
        .bind(edit.placement.day().map(TodoDay::as_str))
        .bind(edit.placement.shelved())
        .fetch_optional(&self.pool)
        .await
        .map_err(write_error)?;
        match row {
            Some(row) => todo(&row),
            // A finished task is there but refuses edits; a missing task or list is not found.
            None => Err(match self.todo_finished(owner, id).await? {
                Some(true) => DbError::Conflict,
                Some(false) | None => DbError::NotFound,
            }),
        }
    }

    /// Reads a task's body, finished or not.
    pub async fn todo_body(&self, owner: OwnerScope, id: Uuid) -> Result<Option<String>, DbError> {
        sqlx::query_scalar("SELECT body FROM todo WHERE owner_id=$1 AND id=$2")
            .bind(owner.id())
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(DbError::NotFound)
    }

    /// Replaces or clears a task's body.
    pub async fn set_todo_body(
        &self,
        owner: OwnerScope,
        id: Uuid,
        body: Option<&TodoBody>,
    ) -> Result<(), DbError> {
        let updated = sqlx::query("UPDATE todo SET body=$3,updated_at=date_trunc('milliseconds',now()) WHERE owner_id=$1 AND id=$2")
            .bind(owner.id())
            .bind(id)
            .bind(body.map(TodoBody::as_str))
            .execute(&self.pool)
            .await?;
        if updated.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }
        Ok(())
    }

    /// Marks a task done or not done for `day`. Only a task planned on that very day takes it, so
    /// a page left open past midnight cannot count a day the task is no longer on. Setting the
    /// value it already has changes nothing; otherwise the count moves by one.
    pub async fn set_todo_done(
        &self,
        owner: OwnerScope,
        id: Uuid,
        day: &TodoDay,
        done: bool,
    ) -> Result<Todo, DbError> {
        let row = sqlx::query(&format!(
            "UPDATE todo SET done=$4,done_days=done_days+CASE WHEN done=$4 THEN 0 WHEN $4 THEN 1 ELSE -1 END,updated_at=date_trunc('milliseconds',now()) WHERE owner_id=$1 AND id=$2 AND completed_at IS NULL AND planned_on=$3::date RETURNING {COLUMNS}"
        ))
        .bind(owner.id())
        .bind(id)
        .bind(day.as_str())
        .bind(done)
        .fetch_optional(&self.pool)
        .await?;
        match row {
            Some(row) => todo(&row),
            None => Err(self.missing_or_conflict(owner, id).await?),
        }
    }

    /// Puts the open, unscheduled tasks among `ids` on `day` in one statement. Tasks already on a
    /// day from `day` on, shelved, finished, in the inbox or not the owner's are left alone; a day
    /// in the past counts as unscheduled.
    pub async fn plan_todos(
        &self,
        owner: OwnerScope,
        day: &TodoDay,
        ids: &[Uuid],
    ) -> Result<(), DbError> {
        sqlx::query("UPDATE todo SET planned_on=$2::date,done=false,updated_at=date_trunc('milliseconds',now()) WHERE owner_id=$1 AND id=ANY($3) AND list_id IS NOT NULL AND completed_at IS NULL AND NOT shelved AND (planned_on IS NULL OR planned_on<$2::date)")
            .bind(owner.id())
            .bind(day.as_str())
            .bind(ids)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Finishes a task in a list: it leaves its plan behind and keeps its count. Inbox tasks are
    /// sorted into a list first, so finishing one is a conflict.
    pub async fn complete_todo(&self, owner: OwnerScope, id: Uuid) -> Result<(), DbError> {
        let updated = sqlx::query("UPDATE todo SET completed_at=date_trunc('milliseconds',now()),planned_on=NULL,shelved=false,done=false,updated_at=date_trunc('milliseconds',now()) WHERE owner_id=$1 AND id=$2 AND completed_at IS NULL AND list_id IS NOT NULL")
            .bind(owner.id())
            .bind(id)
            .execute(&self.pool)
            .await?;
        if updated.rows_affected() == 0 {
            return Err(self.missing_or_conflict(owner, id).await?);
        }
        Ok(())
    }

    /// Brings a finished task back to its list at `position`, unscheduled.
    pub async fn restore_todo(
        &self,
        owner: OwnerScope,
        id: Uuid,
        position: &TodoPosition,
    ) -> Result<Todo, DbError> {
        let row = sqlx::query(&format!(
            "UPDATE todo SET completed_at=NULL,position=$3,updated_at=date_trunc('milliseconds',now()) WHERE owner_id=$1 AND id=$2 AND completed_at IS NOT NULL RETURNING {COLUMNS}"
        ))
        .bind(owner.id())
        .bind(id)
        .bind(position.as_str())
        .fetch_optional(&self.pool)
        .await
        .map_err(write_error)?;
        match row {
            Some(row) => todo(&row),
            None => Err(self.missing_or_conflict(owner, id).await?),
        }
    }

    /// Removes one of the owner's tasks, finished or not.
    pub async fn delete_todo(&self, owner: OwnerScope, id: Uuid) -> Result<(), DbError> {
        let removed = sqlx::query("DELETE FROM todo WHERE owner_id=$1 AND id=$2")
            .bind(owner.id())
            .bind(id)
            .execute(&self.pool)
            .await?;
        if removed.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }
        Ok(())
    }

    /// Whether the owner's task is finished; `None` when there is no such task.
    async fn todo_finished(&self, owner: OwnerScope, id: Uuid) -> Result<Option<bool>, DbError> {
        Ok(sqlx::query_scalar(
            "SELECT completed_at IS NOT NULL FROM todo WHERE owner_id=$1 AND id=$2",
        )
        .bind(owner.id())
        .bind(id)
        .fetch_optional(&self.pool)
        .await?)
    }

    /// Explains a guarded write that matched nothing: a task that is there refused the change.
    async fn missing_or_conflict(&self, owner: OwnerScope, id: Uuid) -> Result<DbError, DbError> {
        Ok(match self.todo_finished(owner, id).await? {
            Some(_) => DbError::Conflict,
            None => DbError::NotFound,
        })
    }
}

/// Reads one row selected with [`COLUMNS`].
pub(crate) fn todo(row: &PgRow) -> Result<Todo, DbError> {
    let list: Option<Uuid> = row.try_get("list_id")?;
    let planned: Option<String> = row.try_get("planned_on")?;
    let shelved: bool = row.try_get("shelved")?;
    let done: bool = row.try_get("done")?;
    // The check constraints admit only these shapes: no plan in the inbox, a day or shelved.
    let place = match (list, planned, shelved) {
        (None, None, false) => TodoPlace::Inbox,
        (Some(list), None, false) => TodoPlace::List {
            list,
            plan: TodoPlan::Open,
        },
        (Some(list), None, true) => TodoPlace::List {
            list,
            plan: TodoPlan::Shelved,
        },
        (Some(list), Some(day), false) => TodoPlace::List {
            list,
            plan: TodoPlan::Day {
                day: TodoDay::from_stored(day),
                done,
            },
        },
        (None, Some(_), _) | (None, None, true) | (Some(_), Some(_), true) => {
            return Err(DbError::Conflict);
        }
    };
    Ok(Todo {
        id: row.try_get("id")?,
        place,
        title: row.try_get("title")?,
        position: TodoPosition::from_stored(row.try_get("position")?),
        link: row.try_get("link")?,
        has_body: row.try_get("has_body")?,
        done_days: row.try_get("done_days")?,
    })
}

/// Two tasks taking one key in the same place is a conflict the page resolves by reloading; a
/// list deleted between the check and the write is the same as never having existed.
pub(crate) fn write_error(error: sqlx::Error) -> DbError {
    match error
        .as_database_error()
        .and_then(sqlx::error::DatabaseError::code)
        .as_deref()
    {
        Some("23505") => DbError::Conflict,
        Some("23503") => DbError::NotFound,
        _ => error.into(),
    }
}
