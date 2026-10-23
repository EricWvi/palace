use palace_db::TodoPlace;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// A list of 行事.
#[derive(Serialize, ToSchema)]
pub(crate) struct TodoList {
    pub id: Uuid,
    pub name: String,
}

/// A task's plan. A `day` before the reader's today means the plan lapsed; the page reads it as
/// `open`. Inbox tasks are always `open`.
#[derive(Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum TodoPlan {
    Open,
    /// Set aside: kept in its list, never offered when planning a day.
    Shelved,
    Day {
        /// `YYYY-MM-DD`.
        day: String,
        /// Done for that day.
        done: bool,
    },
}

/// The plan an edit writes; done is not written by an edit and resets whenever the day changes.
#[derive(Deserialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum TodoPlanInput {
    Open,
    Shelved,
    Day {
        /// `YYYY-MM-DD`.
        day: String,
    },
}

/// An open task.
#[derive(Serialize, ToSchema)]
pub(crate) struct Todo {
    pub id: Uuid,
    /// The list; null is the inbox.
    #[schema(required = true)]
    pub list_id: Option<Uuid>,
    pub title: String,
    /// fractional-indexing key; tasks in one place sort by it as bytes, smallest first.
    pub position: String,
    #[schema(required = true)]
    pub link: Option<String>,
    /// Read the body itself from `/api/todos/{id}/body`.
    pub has_body: bool,
    pub plan: TodoPlan,
    /// Days the task was marked done.
    pub done_days: i32,
}

/// Every list in creation order and every open task, in key order within each place.
#[derive(Serialize, ToSchema)]
pub(crate) struct TodoBoard {
    pub lists: Vec<TodoList>,
    pub todos: Vec<Todo>,
}

/// A new task, unscheduled.
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TodoCreate {
    /// The list; null puts the task in the inbox.
    pub list_id: Option<Uuid>,
    /// Trimmed; nonblank, at most 1024 bytes.
    pub title: String,
    /// fractional-indexing key, at most 1024 bytes, not taken by another open task in the place.
    pub position: String,
}

/// Everything an edit replaces together on an open task.
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TodoUpdate {
    /// The list; null is the inbox, which takes only an `open` plan.
    pub list_id: Option<Uuid>,
    pub title: String,
    /// Absolute http or https URL of at most 2048 bytes; null removes it.
    pub link: Option<String>,
    pub position: String,
    pub plan: TodoPlanInput,
}

/// A task's body: Markdown, nonblank, at most 256 KiB; null when there is none.
#[derive(Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TodoBodyContent {
    #[schema(required = true)]
    pub body: Option<String>,
}

/// Done or not done for the day the task is planned on.
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TodoDone {
    /// The reader's today, `YYYY-MM-DD`; it must be the task's planned day.
    pub day: String,
    pub done: bool,
}

/// Where a finished task returns in its list.
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TodoRestore {
    pub position: String,
}

/// Tasks to put on a day.
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TodoPlanRequest {
    /// `YYYY-MM-DD`.
    pub day: String,
    /// At most 1000. Tasks that are not open, unscheduled and in a list are left alone.
    pub ids: Vec<Uuid>,
}

/// A list's name: trimmed, nonblank, at most 256 bytes.
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct TodoListInput {
    pub name: String,
}

/// A finished task.
#[derive(Serialize, ToSchema)]
pub(crate) struct CompletedTodo {
    pub id: Uuid,
    pub title: String,
    #[schema(required = true)]
    pub link: Option<String>,
    pub has_body: bool,
    pub done_days: i32,
    /// Unix epoch milliseconds.
    pub completed_at: i64,
}

/// A list's finished tasks, the most recently finished first.
#[derive(Serialize, ToSchema)]
pub(crate) struct CompletedTodoList {
    pub items: Vec<CompletedTodo>,
}

impl From<palace_db::TodoList> for TodoList {
    fn from(value: palace_db::TodoList) -> Self {
        Self {
            id: value.id,
            name: value.name,
        }
    }
}

impl From<palace_db::Todo> for Todo {
    fn from(value: palace_db::Todo) -> Self {
        let (list_id, plan) = match value.place {
            TodoPlace::Inbox => (None, TodoPlan::Open),
            TodoPlace::List { list, plan } => (
                Some(list),
                match plan {
                    palace_domain::TodoPlan::Open => TodoPlan::Open,
                    palace_domain::TodoPlan::Shelved => TodoPlan::Shelved,
                    palace_domain::TodoPlan::Day { day, done } => TodoPlan::Day {
                        day: day.as_str().to_owned(),
                        done,
                    },
                },
            ),
        };
        Self {
            id: value.id,
            list_id,
            title: value.title,
            position: value.position.as_str().to_owned(),
            link: value.link,
            has_body: value.has_body,
            plan,
            done_days: value.done_days,
        }
    }
}

impl From<palace_db::TodoBoard> for TodoBoard {
    fn from(value: palace_db::TodoBoard) -> Self {
        Self {
            lists: value.lists.into_iter().map(TodoList::from).collect(),
            todos: value.todos.into_iter().map(Todo::from).collect(),
        }
    }
}

impl From<Vec<palace_db::CompletedTodo>> for CompletedTodoList {
    fn from(value: Vec<palace_db::CompletedTodo>) -> Self {
        Self {
            items: value
                .into_iter()
                .map(|todo| CompletedTodo {
                    id: todo.id,
                    title: todo.title,
                    link: todo.link,
                    has_body: todo.has_body,
                    done_days: todo.done_days,
                    completed_at: todo.completed_at,
                })
                .collect(),
        }
    }
}
