//! 时刻 sleep: the running start and finished nights.
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

/// The running sleep, if any.
#[derive(Serialize, ToSchema)]
pub(crate) struct SleepRunning {
    /// Unix epoch milliseconds of the start; null when nothing is running. A start the browser
    /// judges void (still running at 15:00 the next day) is still returned until it is replaced.
    #[schema(required = true)]
    pub started_at: Option<i64>,
}

/// Ends the start the caller saw.
#[derive(Deserialize, ToSchema)]
pub(crate) struct SleepEnd {
    /// `started_at` from `GET /api/sleep/running`.
    pub started_at: i64,
}

/// One finished night, `[started_at, ended_at)`.
#[derive(Serialize, ToSchema)]
pub(crate) struct SleepNight {
    /// Equal to the night's moment id.
    pub id: Uuid,
    /// Unix epoch milliseconds of the bedtime.
    pub started_at: i64,
    /// Unix epoch milliseconds of waking, the night's place on the timeline.
    pub ended_at: i64,
}

impl From<palace_db::SleepNight> for SleepNight {
    fn from(value: palace_db::SleepNight) -> Self {
        Self {
            id: value.id,
            started_at: value.started_at,
            ended_at: value.ended_at,
        }
    }
}

/// A night's span: after the bedtime, at most 24 hours later.
#[derive(Deserialize, ToSchema)]
pub(crate) struct SleepNightInput {
    /// Unix epoch milliseconds of the bedtime.
    pub started_at: i64,
    /// Unix epoch milliseconds of waking.
    pub ended_at: i64,
}

#[derive(Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct SleepNightRange {
    /// Inclusive start of the wake times, Unix epoch milliseconds in the caller's time zone.
    pub start: i64,
    /// Exclusive end of the wake times; after start and at most 400 days later.
    pub end: i64,
}
