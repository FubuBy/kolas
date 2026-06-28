use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{Event, Subscriber};
use tracing_subscriber::Layer;
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;

use super::super::config::{LevelFilter, QueueSinkConfig};
use super::super::driver::{NullQueueDriver, QueueDriver};
use super::super::error::LoggingError;
use super::super::format::{LogEntry, event_to_entry};
use super::level_passes;

/// The subscriber layer. Converts events to `LogEntry` values and forwards them
/// to the background publisher via a bounded mpsc channel. Drops silently when
/// the channel is full rather than blocking the caller.
struct QueueSink {
    sender: mpsc::Sender<LogEntry>,
    level: LevelFilter,
}

impl<S> Layer<S> for QueueSink
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        let meta = event.metadata();

        if !level_passes(meta.level(), self.level) {
            return;
        }

        let entry = event_to_entry(event, &ctx);

        // Non-blocking send; silently drop on a full channel.
        let _ = self.sender.try_send(entry);
    }
}

/// Builds a queue sink layer and starts the background publisher task.
///
/// Supported driver names: `"null"`. The `"null"` driver discards all entries —
/// useful for tests and development environments. Additional drivers (Redis,
/// RabbitMQ, etc.) can be wired in here by matching on `cfg.driver`.
#[allow(clippy::type_complexity)]
pub fn build_queue_layer<S>(
    cfg: &QueueSinkConfig,
) -> Result<(Box<dyn Layer<S> + Send + Sync>, JoinHandle<()>), LoggingError>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    let driver: Arc<dyn QueueDriver> = match cfg.driver.as_str() {
        "null" => Arc::new(NullQueueDriver),
        other => {
            return Err(LoggingError::UnknownQueueDriver {
                driver: other.to_string(),
            });
        }
    };

    let (sender, receiver) = mpsc::channel(cfg.channel_capacity);

    let sink = QueueSink {
        sender,
        level: cfg.level,
    };

    let handle = spawn_queue_worker(receiver, driver);

    Ok((Box::new(sink), handle))
}

fn spawn_queue_worker(
    mut receiver: mpsc::Receiver<LogEntry>,
    driver: Arc<dyn QueueDriver>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            match receiver.recv().await {
                Some(entry) => {
                    let payload = match serde_json::to_value(&entry) {
                        Ok(v) => v,
                        Err(err) => {
                            eprintln!("[kolas/log] QueueSink: serialization error: {err}");
                            continue;
                        }
                    };

                    if let Err(err) = driver.publish(payload).await {
                        eprintln!(
                            "[kolas/log] QueueSink: publish to '{}' failed: {err}",
                            driver.name()
                        );
                    }
                }
                None => {
                    // Channel closed — exit cleanly.
                    return;
                }
            }
        }
    })
}
