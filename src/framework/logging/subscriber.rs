use tokio::task::JoinHandle;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

use super::config::{
    ConsoleSinkConfig, ConsoleTarget, FormatKind, LevelFilter, LoggingConfig, SinkConfig,
};
use super::error::LoggingError;
use super::guard::LoggingGuard;
use super::sink;

/// Process-wide logging facade.
pub struct Logging;

impl Logging {
    /// Reads the `logging` section from the global Config and initializes
    /// the tracing subscriber. Falls back to console-pretty-INFO if the
    /// section is missing. Returns a `LoggingGuard` that flushes all sinks
    /// on drop.
    ///
    /// Must be called after `Config::load(...).install_global()` and
    /// `Database::install_global()`.
    pub fn init() -> Result<LoggingGuard, LoggingError> {
        let cfg = crate::framework::config::Config::try_get::<LoggingConfig>("logging")
            .unwrap_or_else(fallback_config);
        Self::init_with(cfg)
    }

    /// Initializes tracing with an explicit config. Useful for tests.
    ///
    /// Returns `Err(LoggingError::NoSinks)` when `config.sinks` is empty.
    pub fn init_with(config: LoggingConfig) -> Result<LoggingGuard, LoggingError> {
        if config.sinks.is_empty() {
            return Err(LoggingError::NoSinks);
        }
        let mut file_guards: Vec<WorkerGuard> = Vec::new();
        let mut retention_handles: Vec<JoinHandle<()>> = Vec::new();
        let mut db_handles: Vec<JoinHandle<()>> = Vec::new();
        let mut queue_handles: Vec<JoinHandle<()>> = Vec::new();

        // Build a layered subscriber. We use a dynamic dispatch approach:
        // collect all layers as Box<dyn Layer<Registry>> and fold them.
        //
        // tracing_subscriber requires a concrete subscriber type at init time,
        // but we can use the `BoxedLayer` approach with the registry.
        use tracing_subscriber::Registry;

        // We start with the base registry and layer each sink on top.
        // The subscriber is built as Box<dyn Subscriber + Send + Sync>.
        let registry = Registry::default();

        // Collect layers. Because tracing_subscriber::layer::Layered is not
        // object-safe across different layer types, we build via fold on a
        // type-erased Vec. The idiomatic approach is using the `with` builder
        // pattern and accepting the resulting Layered types.
        //
        // We use the `Vec<Box<dyn Layer<S>>>` approach as documented in
        // tracing-subscriber's multiple_subscribers example.

        let mut layers: Vec<Box<dyn tracing_subscriber::Layer<Registry> + Send + Sync>> =
            Vec::new();

        for sink_cfg in &config.sinks {
            match sink_cfg {
                SinkConfig::Console(cfg) => {
                    let layer = sink::console::build_console_layer(cfg);
                    layers.push(layer);
                }
                SinkConfig::File(cfg) => {
                    let (layer, guard, retention_handle) = sink::file::build_file_layer(cfg)?;
                    layers.push(layer);
                    file_guards.push(guard);
                    retention_handles.push(retention_handle);
                }
                SinkConfig::Database(cfg) => {
                    let (layer, handle) = sink::database::build_database_layer(cfg)?;
                    layers.push(layer);
                    db_handles.push(handle);
                }
                SinkConfig::Queue(cfg) => {
                    let (layer, handle) = sink::queue::build_queue_layer(cfg)?;
                    layers.push(layer);
                    queue_handles.push(handle);
                }
            }
        }

        // Register the subscriber. Returns AlreadyInitialized on subsequent calls
        // (e.g. multiple tests in the same process). This is the expected behavior
        // in tests — the first call wins.
        registry
            .with(layers)
            .try_init()
            .map_err(|_| LoggingError::AlreadyInitialized)?;

        Ok(LoggingGuard::new(
            file_guards,
            retention_handles,
            db_handles,
            queue_handles,
        ))
    }
}

fn fallback_config() -> LoggingConfig {
    LoggingConfig {
        default_level: LevelFilter::Info,
        sinks: vec![SinkConfig::Console(ConsoleSinkConfig {
            level: LevelFilter::Info,
            format: FormatKind::Pretty,
            target: ConsoleTarget::Stdout,
            exclude_targets: Vec::new(),
        })],
    }
}
