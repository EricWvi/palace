//! 时刻 日课 and 工夫: habits with their checks per local day, activity types and activities.
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

/// A habit of 日课.
#[derive(Serialize, ToSchema)]
pub(crate) struct Habit {
    pub id: Uuid,
    pub name: String,
    /// The one character its seal shows.
    pub mark: String,
    /// 1 to 10.
    pub times_per_day: i16,
    /// Unix epoch milliseconds.
    pub created_at: i64,
    /// Retired habits take no checks and are listed only where they have checks.
    pub retired: bool,
}

impl From<palace_db::Habit> for Habit {
    fn from(value: palace_db::Habit) -> Self {
        Self {
            id: value.id,
            name: value.name,
            mark: value.mark,
            times_per_day: value.times_per_day,
            created_at: value.created_at,
            retired: value.retired,
        }
    }
}

/// How many times a habit was done on a local day.
#[derive(Serialize, ToSchema)]
pub(crate) struct HabitCheck {
    pub habit_id: Uuid,
    /// `YYYY-MM-DD`, the reader's local day.
    pub day: String,
    /// 1 to 10; it may exceed a habit's times a day that was lowered later.
    pub count: i16,
}

impl From<palace_db::HabitCheck> for HabitCheck {
    fn from(value: palace_db::HabitCheck) -> Self {
        Self {
            habit_id: value.habit_id,
            day: value.day.text(),
            count: value.count,
        }
    }
}

/// The habits a run of days shows, and their checks on those days.
#[derive(Serialize, ToSchema)]
pub(crate) struct HabitBoard {
    /// Habits in use, and retired ones with checks in the run, in creation order.
    pub habits: Vec<Habit>,
    /// Ordered by day, then habit; days without a check are absent.
    pub checks: Vec<HabitCheck>,
}

impl From<palace_db::HabitBoard> for HabitBoard {
    fn from(value: palace_db::HabitBoard) -> Self {
        Self {
            habits: value.habits.into_iter().map(Into::into).collect(),
            checks: value.checks.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct HabitDaysQuery {
    /// First local day, `YYYY-MM-DD`.
    pub from: String,
    /// Last local day, included; at most 99 days after `from`.
    pub to: String,
}

/// A habit as created or edited.
#[derive(Deserialize, ToSchema)]
pub(crate) struct HabitInput {
    pub name: String,
    /// Exactly one character.
    pub mark: String,
    /// 1 to 10.
    pub times_per_day: i64,
}

/// The times a day was done, set rather than added.
#[derive(Deserialize, ToSchema)]
pub(crate) struct HabitCountInput {
    /// 0 to the habit's times a day; 0 clears the day.
    pub count: i64,
}

/// A day's check after it was set; `count` 0 means the day has none.
#[derive(Serialize, ToSchema)]
pub(crate) struct HabitCheckResult {
    pub habit_id: Uuid,
    /// `YYYY-MM-DD`.
    pub day: String,
    pub count: i16,
}

/// A kind of activity the owner named.
#[derive(Serialize, ToSchema)]
pub(crate) struct ActivityType {
    pub id: Uuid,
    pub name: String,
}

impl From<palace_db::ActivityType> for ActivityType {
    fn from(value: palace_db::ActivityType) -> Self {
        Self {
            id: value.id,
            name: value.name,
        }
    }
}

/// A type to add; an existing name gives back that type.
#[derive(Deserialize, ToSchema)]
pub(crate) struct ActivityTypeInput {
    pub name: String,
}

/// One activity of 工夫.
#[derive(Serialize, ToSchema)]
pub(crate) struct Activity {
    /// Equal to the activity's moment id.
    pub id: Uuid,
    pub type_id: Uuid,
    pub type_name: String,
    /// Unix epoch milliseconds of the start, the activity's place on the timeline.
    pub started_at: i64,
    /// 1 to 720.
    pub minutes: i32,
    #[schema(required = true)]
    pub note: Option<String>,
}

impl From<palace_db::Activity> for Activity {
    fn from(value: palace_db::Activity) -> Self {
        Self {
            id: value.id,
            type_id: value.type_id,
            type_name: value.type_name,
            started_at: value.started_at,
            minutes: value.minutes,
            note: value.note,
        }
    }
}

/// An activity as recorded or corrected.
#[derive(Deserialize, ToSchema)]
pub(crate) struct ActivityInput {
    /// One of the owner's activity types.
    pub type_id: Uuid,
    /// Unix epoch milliseconds of the start.
    pub started_at: i64,
    /// 1 to 720.
    pub minutes: i64,
    /// At most 1024 bytes; blank or null is no note.
    pub note: Option<String>,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct ActivityRange {
    /// Inclusive start of the starts, Unix epoch milliseconds in the caller's time zone.
    pub start: i64,
    /// Exclusive end; after start and at most 400 days later.
    pub end: i64,
}
