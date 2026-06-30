use super::super::{Schedule, Scheduler};
use crate::framework::console::{Args, BoxFuture, Command, ConsoleKernel};
use chrono::Utc;
use std::sync::Arc;

/// `schedule:run` — runs every task due in the current minute, then exits.
/// Designed to be invoked once per minute by the system cron.
pub struct ScheduleRunCommand {
    schedule: Arc<Schedule>,
    scheduler: Arc<Scheduler>,
    kernel: Arc<ConsoleKernel>,
}

impl ScheduleRunCommand {
    pub fn new(
        schedule: Arc<Schedule>,
        scheduler: Arc<Scheduler>,
        kernel: Arc<ConsoleKernel>,
    ) -> Self {
        Self {
            schedule,
            scheduler,
            kernel,
        }
    }
}

impl Command for ScheduleRunCommand {
    fn name(&self) -> &str {
        "schedule:run"
    }

    fn description(&self) -> &str {
        "Run scheduled tasks that are due now (single pass)"
    }

    fn execute(&self, _args: Args) -> BoxFuture<'_> {
        let schedule = Arc::clone(&self.schedule);
        let scheduler = Arc::clone(&self.scheduler);
        let kernel = Arc::clone(&self.kernel);

        Box::pin(async move {
            let errors = scheduler.run_once(&schedule, kernel, Utc::now()).await;

            if errors.is_empty() {
                Ok(())
            } else {
                Err(format!("{} scheduled task(s) failed", errors.len()).into())
            }
        })
    }
}
