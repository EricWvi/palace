//! 行事: tasks, the lists they sit in, and the plan that puts a task on a day. A task without a
//! list sits in the inbox, which is never planned from.
use crate::{InputError, InputErrorKind, article::validate_web_url};
use uuid::Uuid;

mod position;
pub use position::TodoPosition;

const TITLE_BYTES: usize = 1024;
const LIST_NAME_BYTES: usize = 256;
// A working note beside a task, never a document.
const BODY_BYTES: usize = 256 * 1024;

/// Trims the text and accepts it when it is nonblank and fits `limit` bytes.
fn nonblank(text: &str, limit: usize, field: &str, what: &str) -> Result<String, InputError> {
    let text = text.trim();
    if text.is_empty() || text.len() > limit {
        return Err(InputError::new(
            InputErrorKind::Field,
            field,
            format!("expected a nonblank {what} of at most {limit} bytes"),
        ));
    }
    Ok(text.to_owned())
}

/// A task's title, trimmed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TodoTitle(String);

impl TodoTitle {
    pub fn parse(text: &str) -> Result<Self, InputError> {
        nonblank(text, TITLE_BYTES, "title", "title").map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A list's name, trimmed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TodoListName(String);

impl TodoListName {
    pub fn parse(text: &str) -> Result<Self, InputError> {
        nonblank(text, LIST_NAME_BYTES, "name", "name").map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Where a task's title leads: an absolute web address, as an article's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TodoLink(String);

impl TodoLink {
    pub fn parse(text: String) -> Result<Self, InputError> {
        validate_web_url(&text, "link")?;
        Ok(Self(text))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A task's working note: Markdown kept exactly as written. Clearing it removes it, so a stored
/// body is never blank.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TodoBody(String);

impl TodoBody {
    pub fn parse(text: String) -> Result<Self, InputError> {
        if text.trim().is_empty() || text.len() > BODY_BYTES {
            return Err(InputError::new(
                InputErrorKind::Field,
                "body",
                "expected a nonblank body of at most 256 KiB; send null to clear it",
            ));
        }
        Ok(Self(text))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A calendar day, `YYYY-MM-DD`. The page names its own local day; the server never decides
/// what "today" is, since the reader and the deployment may sit in different zones.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TodoDay(String);

impl TodoDay {
    pub fn parse(text: &str, field: &str) -> Result<Self, InputError> {
        let format = time::macros::format_description!("[year]-[month]-[day]");
        let day = time::Date::parse(text, format).map_err(|_| {
            InputError::new(InputErrorKind::Field, field, "expected a YYYY-MM-DD day")
        })?;
        // Years past 9999 would format with a sign; the column and the page expect four digits.
        if !(1..=9999).contains(&day.year()) {
            return Err(InputError::new(
                InputErrorKind::Field,
                field,
                "expected a YYYY-MM-DD day",
            ));
        }
        Ok(Self(text.to_owned()))
    }

    /// Rebuilds a day the database formatted itself.
    pub fn from_stored(text: String) -> Self {
        Self(text)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A task's plan as stored. A day in the past is still `Day`; the page reads it as unscheduled.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TodoPlan {
    Open,
    /// Set aside: kept in its list, never offered when planning a day.
    Shelved,
    /// On a day, and whether it was done for that day.
    Day {
        day: TodoDay,
        done: bool,
    },
}

/// The plan an edit writes. Done is never written by an edit: it belongs to the day it was
/// done on, so any change of plan starts the day afresh.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TodoPlanInput {
    Open,
    Shelved,
    Day(TodoDay),
}

/// Where an edit puts a task. The inbox holds no plan, so only a list carries one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TodoPlacement {
    Inbox,
    List { list: Uuid, plan: TodoPlanInput },
}

impl TodoPlacement {
    /// Joins the wire's separate list and plan, refusing a plan for the inbox.
    pub fn parse(list: Option<Uuid>, plan: TodoPlanInput) -> Result<Self, InputError> {
        match (list, plan) {
            (Some(list), plan) => Ok(Self::List { list, plan }),
            (None, TodoPlanInput::Open) => Ok(Self::Inbox),
            (None, TodoPlanInput::Shelved | TodoPlanInput::Day(_)) => Err(InputError::new(
                InputErrorKind::Field,
                "plan",
                "the inbox is never planned; move the task to a list first",
            )),
        }
    }

    pub fn list(&self) -> Option<Uuid> {
        match self {
            Self::Inbox => None,
            Self::List { list, .. } => Some(*list),
        }
    }

    /// The day this placement plans the task on, if any.
    pub fn day(&self) -> Option<&TodoDay> {
        match self {
            Self::List {
                plan: TodoPlanInput::Day(day),
                ..
            } => Some(day),
            Self::Inbox
            | Self::List {
                plan: TodoPlanInput::Open | TodoPlanInput::Shelved,
                ..
            } => None,
        }
    }

    pub fn shelved(&self) -> bool {
        matches!(
            self,
            Self::List {
                plan: TodoPlanInput::Shelved,
                ..
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    /// Texts are trimmed and bounded; each refusal names its field.
    /// Core test case:
    /// - `specs/test-cases/web/todo/todos.md#a-task-must-keep-one-plan-and-the-inbox-none`
    #[test]
    fn texts_are_trimmed_and_bounded() {
        assert_eq!(TodoTitle::parse(" 读书 ").map(|t| t.0), Ok("读书".into()));
        assert_eq!(
            TodoListName::parse(" 工作 ").map(|t| t.0),
            Ok("工作".into())
        );
        assert_eq!(
            TodoBody::parse(" 原样 \n".into()).map(|b| b.0),
            Ok(" 原样 \n".into())
        );
        let refused = [
            TodoTitle::parse("  ").map(|_| ()),
            TodoTitle::parse(&"字".repeat(342)).map(|_| ()),
            TodoListName::parse(&"a".repeat(257)).map(|_| ()),
            TodoLink::parse("ftp://example.com".into()).map(|_| ()),
            TodoBody::parse(" \n".into()).map(|_| ()),
            TodoDay::parse("2026-2-30", "day").map(|_| ()),
            TodoDay::parse("2026-02-30", "day").map(|_| ()),
        ]
        .map(|result| result.map_err(|error| error.path));
        assert_eq!(
            refused,
            ["title", "title", "name", "link", "body", "day", "day"].map(|path| Err(path.into()))
        );
        assert_eq!(
            TodoDay::parse("2026-10-23", "day").map(|d| d.0),
            Ok("2026-10-23".into())
        );
    }

    /// Only a list takes a plan; the inbox is open or refused.
    /// Core test case:
    /// - `specs/test-cases/web/todo/todos.md#a-task-must-keep-one-plan-and-the-inbox-none`
    #[test]
    fn the_inbox_takes_no_plan() {
        let list = Uuid::now_v7();
        let day = TodoDay::parse("2026-10-23", "day").unwrap();
        assert_eq!(
            TodoPlacement::parse(None, TodoPlanInput::Open),
            Ok(TodoPlacement::Inbox)
        );
        assert_eq!(
            TodoPlacement::parse(Some(list), TodoPlanInput::Day(day.clone())),
            Ok(TodoPlacement::List {
                list,
                plan: TodoPlanInput::Day(day.clone())
            })
        );
        for plan in [TodoPlanInput::Shelved, TodoPlanInput::Day(day)] {
            assert_eq!(
                TodoPlacement::parse(None, plan).map_err(|error| error.path),
                Err("plan".into())
            );
        }
    }
}
