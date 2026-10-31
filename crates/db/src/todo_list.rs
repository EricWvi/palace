//! Owner-scoped lists of 行事, the whole board the page loads, and each list's finished tasks.
use crate::todo::{COLUMNS, Todo, todo, write_error};
use crate::{Database, DbError, OwnerScope};
use palace_domain::{TodoListName, TodoPosition};
use sqlx::Row;
use uuid::Uuid;

/// A list as the page names it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TodoList {
    pub id: Uuid,
    pub name: String,
    pub position: TodoPosition,
}

/// Every list in key order, and every open task in the inbox and the lists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TodoBoard {
    pub lists: Vec<TodoList>,
    /// In key order within each place.
    pub todos: Vec<Todo>,
}

/// A finished task in its list's 已完成.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletedTodo {
    pub id: Uuid,
    pub title: String,
    pub link: Option<String>,
    pub has_body: bool,
    pub done_days: i32,
    /// Unix epoch milliseconds.
    pub completed_at: i64,
}

impl Database {
    /// Reads the whole board in one snapshot, so the counts and today agree with the lists.
    pub async fn todo_board(&self, owner: OwnerScope) -> Result<TodoBoard, DbError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY")
            .execute(&mut *tx)
            .await?;
        let lists = sqlx::query(
            "SELECT id,name,position FROM todo_list WHERE owner_id=$1 ORDER BY position,id",
        )
        .bind(owner.id())
        .fetch_all(&mut *tx)
        .await?
        .iter()
        .map(|row| {
            Ok(TodoList {
                id: row.try_get("id")?,
                name: row.try_get("name")?,
                position: TodoPosition::from_stored(row.try_get("position")?),
            })
        })
        .collect::<Result<_, DbError>>()?;
        let todos = sqlx::query(&format!(
            "SELECT {COLUMNS} FROM todo WHERE owner_id=$1 AND completed_at IS NULL ORDER BY position,id"
        ))
        .bind(owner.id())
        .fetch_all(&mut *tx)
        .await?
        .iter()
        .map(todo)
        .collect::<Result<_, _>>()?;
        tx.commit().await?;
        Ok(TodoBoard { lists, todos })
    }

    /// Adds a list at `position`; a key another of the owner's lists holds is a conflict.
    pub async fn insert_todo_list(
        &self,
        owner: OwnerScope,
        name: &TodoListName,
        position: &TodoPosition,
    ) -> Result<TodoList, DbError> {
        let id = Uuid::now_v7();
        sqlx::query("INSERT INTO todo_list(id,owner_id,name,position) VALUES($1,$2,$3,$4)")
            .bind(id)
            .bind(owner.id())
            .bind(name.as_str())
            .bind(position.as_str())
            .execute(&self.pool)
            .await
            .map_err(write_error)?;
        Ok(TodoList {
            id,
            name: name.as_str().to_owned(),
            position: position.clone(),
        })
    }

    /// Replaces a list's name and position together; moving writes only the moved list.
    pub async fn update_todo_list(
        &self,
        owner: OwnerScope,
        id: Uuid,
        name: &TodoListName,
        position: &TodoPosition,
    ) -> Result<TodoList, DbError> {
        let updated = sqlx::query("UPDATE todo_list SET name=$3,position=$4,updated_at=date_trunc('milliseconds',now()) WHERE owner_id=$1 AND id=$2")
            .bind(owner.id())
            .bind(id)
            .bind(name.as_str())
            .bind(position.as_str())
            .execute(&self.pool)
            .await
            .map_err(write_error)?;
        if updated.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }
        Ok(TodoList {
            id,
            name: name.as_str().to_owned(),
            position: position.clone(),
        })
    }

    /// Deletes a list; its tasks, finished or not, go with it through the cascading key.
    pub async fn delete_todo_list(&self, owner: OwnerScope, id: Uuid) -> Result<(), DbError> {
        let removed = sqlx::query("DELETE FROM todo_list WHERE owner_id=$1 AND id=$2")
            .bind(owner.id())
            .bind(id)
            .execute(&self.pool)
            .await?;
        if removed.rows_affected() == 0 {
            return Err(DbError::NotFound);
        }
        Ok(())
    }

    /// A list's finished tasks, the most recently finished first.
    pub async fn completed_todos(
        &self,
        owner: OwnerScope,
        list: Uuid,
    ) -> Result<Vec<CompletedTodo>, DbError> {
        self.require_todo_list(owner, list).await?;
        sqlx::query("SELECT id,title,link,body IS NOT NULL AS has_body,done_days,(extract(epoch FROM completed_at)*1000)::bigint AS completed_at FROM todo WHERE owner_id=$1 AND list_id=$2 AND completed_at IS NOT NULL ORDER BY completed_at DESC,id DESC")
            .bind(owner.id())
            .bind(list)
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(|row| {
                Ok(CompletedTodo {
                    id: row.try_get("id")?,
                    title: row.try_get("title")?,
                    link: row.try_get("link")?,
                    has_body: row.try_get("has_body")?,
                    done_days: row.try_get("done_days")?,
                    completed_at: row.try_get("completed_at")?,
                })
            })
            .collect()
    }

    /// Deletes every finished task of a list.
    pub async fn clear_completed_todos(
        &self,
        owner: OwnerScope,
        list: Uuid,
    ) -> Result<(), DbError> {
        self.require_todo_list(owner, list).await?;
        sqlx::query(
            "DELETE FROM todo WHERE owner_id=$1 AND list_id=$2 AND completed_at IS NOT NULL",
        )
        .bind(owner.id())
        .bind(list)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Fails with not found unless the list is the owner's.
    async fn require_todo_list(&self, owner: OwnerScope, list: Uuid) -> Result<(), DbError> {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM todo_list WHERE owner_id=$1 AND id=$2)",
        )
        .bind(owner.id())
        .bind(list)
        .fetch_one(&self.pool)
        .await?;
        if !exists {
            return Err(DbError::NotFound);
        }
        Ok(())
    }
}
