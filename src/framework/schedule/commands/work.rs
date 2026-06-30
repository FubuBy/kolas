use std::sync::Arc;

use tokio::sync::watch;

use crate::framework::console::{Args, BoxFuture, Command, ConsoleKernel};

use super::super::{Schedule, Scheduler};

/// `schedule:work` — runs the scheduler in the foreground until interrupted
/// (Ctrl-C), dispatching tasks as they come due. Convenient for development;
/// production should prefer `schedule:run` driven by the system cron.
pub struct ScheduleWorkCommand {
    schedule: Arc<Schedule>,
    scheduler: Arc<Scheduler>,
    kernel: Arc<ConsoleKernel>,
}

impl ScheduleWorkCommand {
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

impl Command for ScheduleWorkCommand {
    fn name(&self) -> &str {
        "schedule:work"
    }

    fn description(&self) -> &str {
        "Run the scheduler in the foreground until interrupted"
    }

    fn execute(&self, _args: Args) -> BoxFuture<'_> {
        let schedule = Arc::clone(&self.schedule);
        let scheduler = Arc::clone(&self.scheduler);
        let kernel = Arc::clone(&self.kernel);

        Box::pin(async move {
            let (tx, rx) = watch::channel(false);
            tokio::spawn(async move {
                if tokio::signal::ctrl_c().await.is_ok() {
                    let _ = tx.send(true);
                }
            });
            scheduler.work(&schedule, kernel, rx).await?;
            Ok(())
        })
    }
}
