use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{Event, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;

use super::super::config::{DatabaseSinkConfig, LevelFilter};
use super::super::error::LoggingError;
use super::super::format::{LogEntry, event_to_entry};
use super::level_passes;
use crate::framework::database::Database;

/// Hard-coded target prefixes always excluded from the database sink.
/// These prevent a recursive loop: sqlx events → DB sink → sqlx → ∞.
const EXCLUDED_TARGETS: &[&str] = &[
    "sqlx",
    "kolas::framework::logging",
    "kolas::framework::database",
];

/// The subscriber layer. Converts events to `LogEntry` values synchronously
/// and forwards them to the background writer via a bounded mpsc channel.
struct DatabaseSink {
    sender: mpsc::Sender<LogEntry>,
    level: LevelFilter,
    dropped_count: Arc<AtomicU64>,
}

impl<S> Layer<S> for DatabaseSink
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let meta = event.metadata();

        // 1. Level gate
        if !level_passes(meta.level(), self.level) {
            return;
        }

        // 2. Anti-recursion: drop events from sqlx / the logging subsystem itself.
        let target = meta.target();
        if EXCLUDED_TARGETS
            .iter()
            .any(|prefix| target.starts_with(prefix))
        {
            return;
        }

        // 3. Serialize into an owned struct (no allocation of Subscriber internals).
        let entry = event_to_entry(event, &ctx);

        // 4. Non-blocking send — prefer dropping over blocking the caller.
        if self.sender.try_send(entry).is_err() {
            self.dropped_count.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// Validates that a table name contains only `[A-Za-z0-9_]` and is non-empty.
fn validate_table_name(table: &str) -> bool {
    !table.is_empty() && table.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Builds a database sink layer and starts the background batch-writer task.
///
/// `Database::install_global()` must have been called before this function,
/// though the background task tolerates a temporarily unavailable connection
/// by backing off and retrying on the next flush cycle.
#[allow(clippy::type_complexity)]
pub fn build_database_layer<S>(
    cfg: &DatabaseSinkConfig,
) -> Result<(Box<dyn Layer<S> + Send + Sync>, JoinHandle<()>), LoggingError>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    // M-1: Validate the table name before spawning to catch misconfiguration early.
    if !validate_table_name(&cfg.table) {
        return Err(LoggingError::InvalidTableName {
            table: cfg.table.clone(),
        });
    }

    let (sender, receiver) = mpsc::channel(cfg.channel_capacity);
    let dropped_count = Arc::new(AtomicU64::new(0));

    let sink = DatabaseSink {
        sender,
        level: cfg.level,
        dropped_count: Arc::clone(&dropped_count),
    };

    let handle = spawn_db_worker(receiver, dropped_count, cfg.clone());

    Ok((Box::new(sink), handle))
}

fn spawn_db_worker(
    mut receiver: mpsc::Receiver<LogEntry>,
    dropped_count: Arc<AtomicU64>,
    cfg: DatabaseSinkConfig,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let batch_size = cfg.batch_size;
        let flush_interval = Duration::from_millis(cfg.flush_interval_ms);
        let mut batch: Vec<LogEntry> = Vec::with_capacity(batch_size);
        let mut backoff_secs: u64 = 1;

        loop {
            let deadline = tokio::time::Instant::now() + flush_interval;

            // Fill the batch up to `batch_size` or until the flush deadline.
            loop {
                match tokio::time::timeout_at(deadline, receiver.recv()).await {
                    Ok(Some(entry)) => {
                        batch.push(entry);
                        if batch.len() >= batch_size {
                            break;
                        }
                    }
                    Ok(None) => {
                        // Channel closed — flush what remains and exit.
                        if !batch.is_empty() {
                            flush_batch(&batch, &cfg, &dropped_count, &mut backoff_secs).await;
                        }
                        return;
                    }
                    Err(_timeout) => break, // flush_interval elapsed
                }
            }

            if batch.is_empty() {
                // Nothing to flush; still report any accumulated drop counter.
                let dropped = dropped_count.swap(0, Ordering::Relaxed);
                if dropped > 0 {
                    eprintln!("[kolas/log] {dropped} log events dropped (channel full)");
                }
            } else {
                flush_batch(&batch, &cfg, &dropped_count, &mut backoff_secs).await;
                batch.clear();
            }
        }
    })
}

async fn flush_batch(
    batch: &[LogEntry],
    cfg: &DatabaseSinkConfig,
    dropped_count: &Arc<AtomicU64>,
    backoff_secs: &mut u64,
) {
    // Surface any dropped-event count first.
    let dropped = dropped_count.swap(0, Ordering::Relaxed);
    if dropped > 0 {
        eprintln!("[kolas/log] {dropped} log events dropped (channel full)");
    }

    // Acquire the AnyPool — Database may not be initialized in test environments.
    let pool = match Database::try_global() {
        Some(db) => match db.any_pool_for(&cfg.connection).await {
            Ok(p) => p,
            Err(err) => {
                // S-5: report the number of entries being lost on early return.
                eprintln!(
                    "[kolas/log] DatabaseSink: connection '{}' unavailable: {err} — {} entries lost",
                    cfg.connection,
                    batch.len()
                );
                let wait = Duration::from_secs((*backoff_secs).min(30));
                tokio::time::sleep(wait).await;
                *backoff_secs = (*backoff_secs * 2).min(30);
                return;
            }
        },
        None => {
            eprintln!(
                "[kolas/log] DatabaseSink: Database not initialized, skipping flush ({} entries lost)",
                batch.len()
            );
            return;
        }
    };

    // Individual inserts via AnyPool — avoids dynamic SQL construction for bulk VALUES.
    // Table name is validated in build_database_layer, so format! is safe here.
    let sql = format!(
        "INSERT INTO {} (level, target, message, span_name, context, logged_at) \
         VALUES (?, ?, ?, ?, ?, ?)",
        cfg.table
    );

    for entry in batch {
        let context_json = entry.fields.to_string();
        let result = sqlx::query(&sql)
            .bind(&entry.level)
            .bind(&entry.target)
            .bind(&entry.message)
            .bind(entry.span_name.as_deref())
            .bind(&context_json)
            .bind(entry.timestamp.to_rfc3339())
            .execute(&pool)
            .await;

        if let Err(err) = result {
            eprintln!("[kolas/log] DatabaseSink: INSERT failed: {err}");
            // Continue with remaining entries rather than aborting the batch.
        }
    }

    *backoff_secs = 1;
}
