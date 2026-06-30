//! Feature tests for the scheduler: the `schedule:run` command end-to-end,
//! the `schedule:work` loop with graceful shutdown, and timezone-aware due
//! calculation. Async timing uses `tokio::time` only.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use chrono::{TimeZone, Utc};
use kolas::framework::console::{Args, BoxFuture, Command, ConsoleKernel};
use kolas::framework::schedule::commands::ScheduleRunCommand;
use kolas::framework::schedule::{Schedule, Scheduler};
use tokio::sync::watch;

struct CounterCommand {
    hits: Arc<AtomicUsize>,
}

impl Command for CounterCommand {
    fn name(&self) -> &str {
        "counter"
    }

    fn description(&self) -> &str {
        "increments a counter (test only)"
    }

    fn execute(&self, _args: Args) -> BoxFuture<'_> {
        let hits = Arc::clone(&self.hits);

        Box::pin(async move {
            hits.fetch_add(1, Ordering::SeqCst);
            Ok(())
        })
    }
}

#[tokio::test]
async fn schedule_run_command_executes_due_tasks() {
    let hits = Arc::new(AtomicUsize::new(0));
    let dispatch_kernel = Arc::new(ConsoleKernel::new().register(CounterCommand {
        hits: Arc::clone(&hits),
    }));

    let mut schedule = Schedule::new();

    schedule.command("counter").every_minute();

    let schedule = Arc::new(schedule);
    let scheduler = Arc::new(Scheduler::new());

    let command = ScheduleRunCommand::new(schedule, scheduler, dispatch_kernel);

    command
        .execute(Args::parse(vec![]))
        .await
        .expect("schedule:run should succeed");

    assert_eq!(hits.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn schedule_work_runs_tasks_then_shuts_down_cleanly() {
    let hits = Arc::new(AtomicUsize::new(0));
    let kernel = Arc::new(ConsoleKernel::new());

    let mut schedule = Schedule::new();
    let counter = Arc::clone(&hits);

    schedule
        .call(move || {
            let counter = Arc::clone(&counter);
            async move {
                counter.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        })
        .every_second();

    let schedule = Arc::new(schedule);
    let scheduler = Arc::new(Scheduler::new());

    let (tx, rx) = watch::channel(false);
    let handle = {
        let schedule = Arc::clone(&schedule);
        let scheduler = Arc::clone(&scheduler);
        let kernel = Arc::clone(&kernel);
        tokio::spawn(async move { scheduler.work(&schedule, kernel, rx).await })
    };

    // Let the runner fire at least once, then signal shutdown.
    tokio::time::sleep(Duration::from_millis(1_200)).await;
    tx.send(true).expect("shutdown signal should send");

    let joined = tokio::time::timeout(Duration::from_secs(5), handle).await;

    assert!(joined.is_ok(), "scheduler did not shut down within timeout");

    joined
        .unwrap()
        .expect("work task should not panic")
        .expect("work should return Ok");

    assert!(
        hits.load(Ordering::SeqCst) >= 1,
        "every-second task should have run at least once"
    );
}

#[tokio::test]
async fn event_respects_per_event_timezone() {
    // 14:00 UTC on 2026-01-01 == 09:00 in America/New_York (EST, UTC-5).
    let moment = Utc.with_ymd_and_hms(2026, 1, 1, 14, 0, 0).unwrap();

    let mut schedule = Schedule::new();
    let due_in_ny = schedule
        .command("counter")
        .daily_at("09:00")
        .timezone("America/New_York")
        .is_due(moment)
        .unwrap();
    assert!(due_in_ny, "09:00 New York should be due at 14:00 UTC");

    // The same wall-clock time is not 09:00 in UTC, so a UTC-bound task is not due.
    let due_in_utc = schedule
        .command("counter")
        .daily_at("09:00")
        .is_due(moment)
        .unwrap();
    assert!(!due_in_utc, "09:00 UTC should not be due at 14:00 UTC");
}
