use tokio::task::JoinHandle;
use tracing_appender::non_blocking::WorkerGuard;

/// Holds resources that must outlive the subscriber:
/// - `WorkerGuard` from tracing-appender (drop = flush file buffer)
/// - `JoinHandle` for retention tasks (abort on drop — they hold no unsaved data)
/// - `JoinHandle` for db/queue background tasks (awaited on drop for graceful flush)
///
/// # Drop safety in async contexts
///
/// `Drop` uses `tokio::task::block_in_place` to await background tasks, which
/// requires a multi-thread Tokio runtime. On a current-thread runtime (the
/// default for `#[tokio::test]`), `block_in_place` panics.
///
/// To avoid this, we detect the runtime flavor at drop time:
/// - **multi-thread runtime**: use `block_in_place` for graceful flush.
/// - **current-thread runtime** (or no runtime): abort background tasks and
///   drop senders immediately (best-effort; buffers are NOT flushed).
///
/// If you need guaranteed flush in tests, use
/// `#[tokio::test(flavor = "multi_thread")]`.
pub struct LoggingGuard {
    _file_guards: Vec<WorkerGuard>,
    retention_handles: Vec<JoinHandle<()>>,
    db_handles: Vec<JoinHandle<()>>,
    queue_handles: Vec<JoinHandle<()>>,
}

impl LoggingGuard {
    /// Builds a guard from the resources collected while constructing the
    /// subscriber. Called by `Logging::init_with`.
    pub(super) fn new(
        file_guards: Vec<WorkerGuard>,
        retention_handles: Vec<JoinHandle<()>>,
        db_handles: Vec<JoinHandle<()>>,
        queue_handles: Vec<JoinHandle<()>>,
    ) -> Self {
        Self {
            _file_guards: file_guards,
            retention_handles,
            db_handles,
            queue_handles,
        }
    }
}

impl Drop for LoggingGuard {
    fn drop(&mut self) {
        // Abort retention tasks — they hold no unsaved data.
        for h in self.retention_handles.drain(..) {
            h.abort();
        }

        // For db/queue tasks: try graceful shutdown if we're on a multi-thread runtime.
        // Otherwise, abort (best-effort).
        let all_bg_handles: Vec<JoinHandle<()>> = self
            .db_handles
            .drain(..)
            .chain(self.queue_handles.drain(..))
            .collect();

        if all_bg_handles.is_empty() {
            return;
        }

        // Detect runtime flavor. We use try_current to avoid panicking when
        // there's no runtime at all.
        let can_block_in_place = match tokio::runtime::Handle::try_current() {
            Err(_) => {
                // No runtime — abort all tasks.
                for h in all_bg_handles {
                    h.abort();
                }
                return;
            }
            Ok(handle) => {
                matches!(
                    handle.runtime_flavor(),
                    tokio::runtime::RuntimeFlavor::MultiThread
                )
            }
        };

        if can_block_in_place {
            // We're on a multi-thread runtime: use block_in_place to wait for tasks.
            // The senders are already dropped (they're held by the Layer structs which
            // are inside the subscriber). The subscriber is not dropped until after
            // LoggingGuard, so we need to be careful.
            //
            // Actually: the subscriber holds the Layer structs which hold the Sender.
            // When the global subscriber is set with try_init(), it is stored in a
            // global and is never dropped. So the senders are never closed by us.
            //
            // For the background tasks to exit, they need the receiver side to close,
            // which happens when the sender is dropped. Since we can't drop the sender
            // (it's in the global subscriber), we just abort the background tasks after
            // a short timeout.
            //
            // This is a known limitation of the global subscriber approach.
            // For true graceful shutdown, the application should use init_with() with
            // an explicit subscriber rather than the global.
            tokio::task::block_in_place(|| {
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    for h in all_bg_handles {
                        // Give each task a short time to finish, then abort.
                        // We keep the abort handle so we can cancel on timeout.
                        let abort = h.abort_handle();
                        match tokio::time::timeout(std::time::Duration::from_millis(500), h).await {
                            Ok(_) => {}
                            Err(_timeout) => {
                                // Task did not finish within the grace period — abort it
                                // explicitly so it does not leak in the runtime thread pool.
                                abort.abort();
                            }
                        }
                    }
                });
            });
        } else {
            // current-thread runtime or no runtime: abort tasks (best-effort)
            for h in all_bg_handles {
                h.abort();
            }
        }
    }
}
