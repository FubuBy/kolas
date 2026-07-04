use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{debug, error, info, warn};
use crate::framework::console::ConsoleKernel;

use super::error::{ScheduleError, TaskError};
use super::lock::{EventId, InMemoryLockStore, LockStore};
use super::registry::Schedule;
use super::server::{NoOpServerSelector, ServerSelector};

/// Time-to-live for an overlapping-protection lock. Conservative so a lock is
/// never dropped while a long task is still running; the runner also releases
/// the lock explicitly once the task finishes.
const LOCK_TTL: Duration = Duration::from_secs(24 * 60 * 60);

/// Result carried out of a spawned task so the runner can release locks and log.
type TaskOutcome = (EventId, Result<(), TaskError>, bool, Arc<dyn LockStore>);

/// Executes [`Schedule`] events, either as a single pass (`run_once`) or as a
/// long-running loop (`work`).
pub struct Scheduler {
    pub(crate) lock_store: Arc<dyn LockStore>,
    pub(crate) server_selector: Arc<dyn ServerSelector>,
    log_runs: bool,
    log_overlapping: bool,
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl Scheduler {
    pub fn new() -> Self {
        Self {
            // In-memory by default so `without_overlapping` works within a
            // single process. Swap in a persistent store later via
            // `with_lock_store` without touching this API.
            lock_store: Arc::new(InMemoryLockStore::new()),
            server_selector: Arc::new(NoOpServerSelector),
            log_runs: true,
            log_overlapping: true,
        }
    }

    pub fn with_lock_store(mut self, store: impl LockStore + 'static) -> Self {
        self.lock_store = Arc::new(store);
        self
    }

    pub fn with_server_selector(mut self, selector: impl ServerSelector + 'static) -> Self {
        self.server_selector = Arc::new(selector);
        self
    }

    pub fn with_log_runs(mut self, enabled: bool) -> Self {
        self.log_runs = enabled;
        self
    }

    pub fn with_log_overlapping(mut self, enabled: bool) -> Self {
        self.log_overlapping = enabled;
        self
    }

    /// Runs every event due within the minute ending at `now`, awaiting each in
    /// turn. Intended to be invoked once per minute by the system cron via the
    /// `schedule:run` command. Returns the errors of any tasks that failed.
    ///
    /// Unlike [`work`](Self::work), which dispatches due tasks concurrently,
    /// `run_once` executes them sequentially: a slow task delays the remaining
    /// due tasks in the same pass.
    pub async fn run_once(
        &self,
        schedule: &Schedule,
        kernel: Arc<ConsoleKernel>,
        now: chrono::DateTime<Utc>,
    ) -> Vec<TaskError> {
        let events = schedule.events();

        if self.log_runs {
            info!(count = events.len(), "schedule:run evaluating events");
        }

        if events.is_empty() {
            info!("no scheduled tasks registered (see app/console/schedule.rs)");
            return Vec::new();
        }

        let mut errors = Vec::new();

        for event in events {
            match event.is_due_with(now, schedule.default_timezone) {
                Ok(true) => {}
                Ok(false) => continue,
                Err(e) => {
                    errors.push(TaskError {
                        id: event.id().to_string(),
                        message: e.to_string(),
                    });
                    continue;
                }
            }

            let id: EventId = event.id().to_string();

            if !self.server_selector.is_leader(&id) {
                continue;
            }

            if event.without_overlapping && !self.lock_store.try_lock(&id, LOCK_TTL) {
                if self.log_overlapping {
                    warn!(task = %id, "skipping scheduled task: previous run still in progress");
                }
                continue;
            }

            if self.log_runs {
                debug!(task = %id, "running scheduled task");
            }

            let outcome = event.task.execute(Arc::clone(&kernel)).await;

            if event.without_overlapping {
                self.lock_store.release(&id);
            }

            if let Err(e) = outcome {
                error!(task = %id, error = %e.message, "scheduled task failed");
                errors.push(e);
            }
        }
        errors
    }

    /// Runs the schedule continuously until `shutdown` flips to `true`,
    /// sleeping until the next due event and dispatching due tasks concurrently.
    pub async fn work(
        &self,
        schedule: &Schedule,
        kernel: Arc<ConsoleKernel>,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<(), ScheduleError> {
        info!(count = schedule.events().len(), "scheduler started");

        let mut tasks: JoinSet<TaskOutcome> = JoinSet::new();

        loop {
            let sleep = Duration::from_millis(self.millis_until_next(schedule, Utc::now()));
            let deadline = tokio::time::Instant::now() + sleep;

            tokio::select! {
                _ = tokio::time::sleep_until(deadline) => {
                    // Reap first so locks released by tasks that finished during
                    // the sleep are cleared before we decide whether to skip a
                    // new run of a `without_overlapping` task.
                    reap(&mut tasks);
                    self.dispatch_due(schedule, &kernel, &mut tasks, Utc::now());
                }
                _ = shutdown.changed() => {
                    info!("scheduler received shutdown signal");
                    break;
                }
            }
        }

        // Drain in-flight tasks before returning so locks are released and
        // failures/panics are logged.
        while let Some(joined) = tasks.join_next().await {
            handle_join(joined);
        }
        info!("scheduler stopped");
        Ok(())
    }

    /// Milliseconds from `now` until the earliest next occurrence across all
    /// events. Falls back to 60s when nothing is scheduled or all events are
    /// invalid. Takes `now` as a parameter so the loop uses one clock reading
    /// per tick.
    fn millis_until_next(&self, schedule: &Schedule, now: chrono::DateTime<Utc>) -> u64 {
        let mut earliest: Option<u64> = None;

        for event in schedule.events() {
            let tz = event.timezone.unwrap_or(schedule.default_timezone);
            let Ok(cron) = event.build_cron() else {
                continue;
            };

            if let Ok(next) = cron.find_next_occurrence(&now.with_timezone(&tz), false) {
                let delta = (next.with_timezone(&Utc) - now).num_milliseconds().max(0) as u64;
                earliest = Some(earliest.map_or(delta, |e| e.min(delta)));
            }
        }
        earliest.unwrap_or(60_000)
    }

    /// Spawns every event due right now, respecting leader election and
    /// overlapping locks.
    fn dispatch_due(
        &self,
        schedule: &Schedule,
        kernel: &Arc<ConsoleKernel>,
        tasks: &mut JoinSet<TaskOutcome>,
        now: chrono::DateTime<Utc>,
    ) {
        for event in schedule.events() {
            match event.is_due_with(now, schedule.default_timezone) {
                Ok(true) => {}
                Ok(false) => continue,
                Err(e) => {
                    error!(task = %event.id(), error = %e, "invalid scheduled task, skipping");
                    continue;
                }
            }

            let id: EventId = event.id().to_string();

            if !self.server_selector.is_leader(&id) {
                continue;
            }

            if event.without_overlapping && !self.lock_store.try_lock(&id, LOCK_TTL) {
                if self.log_overlapping {
                    warn!(task = %id, "skipping scheduled task: previous run still in progress");
                }
                continue;
            }

            if self.log_runs {
                debug!(task = %id, "running scheduled task");
            }

            let future = event.task.execute(Arc::clone(kernel));
            let lock_store = Arc::clone(&self.lock_store);
            let without_overlapping = event.without_overlapping;
            tasks.spawn(async move {
                let result = future.await;
                (id, result, without_overlapping, lock_store)
            });
        }
    }
}

/// Drains finished tasks without blocking.
fn reap(tasks: &mut JoinSet<TaskOutcome>) {
    while let Some(joined) = tasks.try_join_next() {
        handle_join(joined);
    }
}

/// Releases the overlapping lock and logs the result of a finished task. A
/// panicked task surfaces as a `JoinError`, which is logged but never
/// propagated, so the runner keeps going.
fn handle_join(joined: Result<TaskOutcome, tokio::task::JoinError>) {
    match joined {
        Ok((id, result, without_overlapping, lock_store)) => {
            if without_overlapping {
                lock_store.release(&id);
            }

            if let Err(e) = result {
                error!(task = %id, error = %e.message, "scheduled task failed");
            }
        }
        Err(join_err) => {
            error!(error = %join_err, "scheduled task panicked");
        }
    }
}
