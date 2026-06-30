//! Error types for the scheduler.

/// Errors raised while building or evaluating a schedule.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ScheduleError {
    /// A cron expression failed to parse.
    #[error("invalid cron expression '{expr}': {message}")]
    InvalidCron { expr: String, message: String },

    /// A `HH:MM` time string could not be parsed.
    #[error("invalid time format '{value}': expected HH:MM (00:00–23:59)")]
    InvalidTimeFormat { value: String },

    /// A day-of-month value was out of range.
    #[error("invalid day value {day}: must be 1–31")]
    InvalidDay { day: u32 },

    /// A timezone name could not be resolved.
    #[error("invalid timezone '{tz}'")]
    InvalidTimezone { tz: String },

    /// Computing the next occurrence of a cron pattern failed.
    #[error("failed to compute next occurrence for '{expr}': {message}")]
    NextOccurrence { expr: String, message: String },

    /// A scheduled command name is not registered with the console kernel.
    #[error("unknown command '{name}' registered in schedule")]
    UnknownCommand { name: String },
}

/// Error returned by a scheduled task that failed to run.
#[derive(Debug, Clone, thiserror::Error)]
#[error("scheduled task '{id}' failed: {message}")]
pub struct TaskError {
    pub id: String,
    pub message: String,
}
