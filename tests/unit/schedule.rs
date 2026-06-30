//! Unit tests for the scheduler: frequency→cron mapping, due calculation,
//! lock store semantics, and a single-pass `run_once`. All tests drive the
//! public `kolas::*` API only and pass `now` explicitly for determinism.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use chrono::{DateTime, TimeZone, Utc};
use kolas::framework::console::{Args, BoxFuture, Command, ConsoleKernel};
use kolas::framework::schedule::{
    EventId, InMemoryLockStore, LockStore, NoOpLockStore, Schedule, Scheduler,
};

// --- helpers ---------------------------------------------------------------

/// A command that counts how many times it ran.
struct CounterCommand {
    name: String,
    hits: Arc<AtomicUsize>,
}

impl Command for CounterCommand {
    fn name(&self) -> &str {
        &self.name
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

/// A lock store that never grants a lock — models a task already running.
struct AlwaysLocked;

impl LockStore for AlwaysLocked {
    fn try_lock(&self, _id: &EventId, _ttl: Duration) -> bool {
        false
    }
    fn release(&self, _id: &EventId) {}
}

fn at(h: u32, m: u32, s: u32) -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 6, 29, h, m, s).unwrap()
}

// --- frequency → cron expression ------------------------------------------

#[test]
fn frequency_methods_map_to_expected_cron() {
    let mut s = Schedule::new();
    assert_eq!(
        s.command("c").every_minute().expression().unwrap(),
        "0 * * * * *"
    );
    assert_eq!(
        s.command("c").every_second().expression().unwrap(),
        "* * * * * *"
    );
    assert_eq!(
        s.command("c").every_five_minutes().expression().unwrap(),
        "0 */5 * * * *"
    );
    assert_eq!(
        s.command("c").every_fifteen_minutes().expression().unwrap(),
        "0 */15 * * * *"
    );
    assert_eq!(
        s.command("c").every_thirty_minutes().expression().unwrap(),
        "0 0,30 * * * *"
    );
    assert_eq!(s.command("c").hourly().expression().unwrap(), "0 0 * * * *");
    assert_eq!(
        s.command("c").hourly_at(15).expression().unwrap(),
        "0 15 * * * *"
    );
    assert_eq!(s.command("c").daily().expression().unwrap(), "0 0 0 * * *");
    assert_eq!(
        s.command("c").daily_at("09:30").expression().unwrap(),
        "0 30 9 * * *"
    );
    assert_eq!(
        s.command("c").twice_daily(1, 13).expression().unwrap(),
        "0 0 1,13 * * *"
    );
    assert_eq!(s.command("c").weekly().expression().unwrap(), "0 0 0 * * 0");
    assert_eq!(
        s.command("c").monthly().expression().unwrap(),
        "0 0 0 1 * *"
    );
    assert_eq!(
        s.command("c").quarterly().expression().unwrap(),
        "0 0 0 1 1,4,7,10 *"
    );
    assert_eq!(s.command("c").yearly().expression().unwrap(), "0 0 0 1 1 *");
    assert_eq!(
        s.command("c").cron("*/5 * * * *").expression().unwrap(),
        "*/5 * * * *"
    );
}

#[test]
fn day_of_week_and_hour_modifiers_apply() {
    let mut s = Schedule::new();
    assert_eq!(
        s.command("c").daily().weekdays().expression().unwrap(),
        "0 0 0 * * 1-5"
    );
    assert_eq!(
        s.command("c").daily().weekends().expression().unwrap(),
        "0 0 0 * * 0,6"
    );
    assert_eq!(
        s.command("c").daily().mondays().expression().unwrap(),
        "0 0 0 * * 1"
    );
    assert_eq!(
        s.command("c")
            .daily()
            .between("09:00", "17:00")
            .expression()
            .unwrap(),
        "0 0 9-17 * * *"
    );
}

#[test]
fn invalid_builder_inputs_surface_as_errors() {
    let mut s = Schedule::new();
    assert!(s.command("c").daily_at("25:00").expression().is_err());
    assert!(s.command("c").daily_at("noon").expression().is_err());
    assert!(s.command("c").monthly_on(40, "09:00").expression().is_err());
    assert!(
        s.command("c")
            .timezone("Nowhere/Land")
            .expression()
            .is_err()
    );
    assert!(s.command("c").hourly_at(60).expression().is_err());
    assert!(s.command("c").twice_daily(25, 1).expression().is_err());
    assert!(
        s.command("c")
            .daily()
            .between("17:00", "09:00")
            .expression()
            .is_err()
    );
}

// --- due calculation -------------------------------------------------------

#[test]
fn every_minute_is_always_due() {
    let mut s = Schedule::new();
    assert!(s.command("c").every_minute().is_due(at(12, 30, 0)).unwrap());
    assert!(s.command("c").every_minute().is_due(at(3, 7, 42)).unwrap());
}

#[test]
fn daily_task_is_due_only_within_its_minute() {
    let mut s = Schedule::new();
    assert!(
        s.command("c")
            .daily_at("09:00")
            .is_due(at(9, 0, 0))
            .unwrap()
    );
    assert!(
        s.command("c")
            .daily_at("09:00")
            .is_due(at(9, 0, 45))
            .unwrap()
    );
    assert!(
        !s.command("c")
            .daily_at("09:00")
            .is_due(at(10, 30, 0))
            .unwrap()
    );
}

// --- lock store ------------------------------------------------------------

#[test]
fn noop_lock_store_always_grants() {
    let store = NoOpLockStore;
    let id: EventId = "task".into();
    assert!(store.try_lock(&id, Duration::from_secs(60)));
    assert!(store.try_lock(&id, Duration::from_secs(60)));
}

#[test]
fn in_memory_lock_store_blocks_until_released() {
    let store = InMemoryLockStore::new();
    let id: EventId = "task".into();
    assert!(store.try_lock(&id, Duration::from_secs(60)));
    assert!(!store.try_lock(&id, Duration::from_secs(60)));
    store.release(&id);
    assert!(store.try_lock(&id, Duration::from_secs(60)));
}

#[tokio::test]
async fn in_memory_lock_store_expires_after_ttl() {
    let store = InMemoryLockStore::new();
    let id: EventId = "task".into();
    assert!(store.try_lock(&id, Duration::from_millis(10)));
    assert!(!store.try_lock(&id, Duration::from_millis(10)));
    tokio::time::sleep(Duration::from_millis(25)).await;
    assert!(store.try_lock(&id, Duration::from_millis(10)));
}

// --- run_once --------------------------------------------------------------

#[tokio::test]
async fn run_once_executes_due_command() {
    let hits = Arc::new(AtomicUsize::new(0));
    let kernel = Arc::new(ConsoleKernel::new().register(CounterCommand {
        name: "counter".into(),
        hits: Arc::clone(&hits),
    }));
    let mut schedule = Schedule::new();
    schedule.command("counter").every_minute();

    let errors = Scheduler::new()
        .run_once(&schedule, kernel, at(12, 30, 0))
        .await;

    assert!(errors.is_empty());
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn run_once_skips_non_due_command() {
    let hits = Arc::new(AtomicUsize::new(0));
    let kernel = Arc::new(ConsoleKernel::new().register(CounterCommand {
        name: "counter".into(),
        hits: Arc::clone(&hits),
    }));
    let mut schedule = Schedule::new();
    schedule.command("counter").daily_at("09:00");

    let errors = Scheduler::new()
        .run_once(&schedule, kernel, at(10, 30, 0))
        .await;

    assert!(errors.is_empty());
    assert_eq!(hits.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn run_once_skips_overlapping_when_locked() {
    let hits = Arc::new(AtomicUsize::new(0));
    let kernel = Arc::new(ConsoleKernel::new().register(CounterCommand {
        name: "counter".into(),
        hits: Arc::clone(&hits),
    }));
    let mut schedule = Schedule::new();
    schedule
        .command("counter")
        .every_minute()
        .without_overlapping();

    let errors = Scheduler::new()
        .with_lock_store(AlwaysLocked)
        .run_once(&schedule, kernel, at(12, 30, 0))
        .await;

    assert!(errors.is_empty());
    assert_eq!(hits.load(Ordering::SeqCst), 0);
}
