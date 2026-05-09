use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt;

/// Hooks up process-wide `tracing` output (console + `RUST_LOG` filter).
pub struct Telemetry;

impl Telemetry {
    /// Registers the default fmt subscriber. Idempotent: repeated calls are ignored after the first successful `try_init`.
    pub fn init() {
        let _ = fmt()
            .with_env_filter(
                EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| EnvFilter::new("info,tower_http=trace")),
            )
            .try_init();
    }
}
