//! A resolvable logging abstraction, distinct from [`super::Logging`].
//! `Logging` is a process-wide `tracing` subscriber installer, called once
//! in bootstrap — it has no per-message methods to call. `Logger` is a
//! trait: application services depend on `Arc<dyn Logger>` (typically via
//! `framework::di`) rather than calling `tracing::info!` directly, so an
//! implementation swap (a structured-logging backend, a per-tenant logger,
//! anything else) only requires a new binding, and tests can substitute a
//! fake that captures messages instead of writing them anywhere.

pub trait Logger: Send + Sync {
    fn debug(&self, message: &str);
    fn info(&self, message: &str);
    fn warn(&self, message: &str);
    fn error(&self, message: &str);
}

/// Default implementation — forwards to the `tracing` macros, so it composes
/// with whatever sinks `Logging::init()` configured.
pub struct TracingLogger;

impl Logger for TracingLogger {
    fn debug(&self, message: &str) {
        tracing::debug!("{message}");
    }

    fn info(&self, message: &str) {
        tracing::info!("{message}");
    }

    fn warn(&self, message: &str) {
        tracing::warn!("{message}");
    }

    fn error(&self, message: &str) {
        tracing::error!("{message}");
    }
}
