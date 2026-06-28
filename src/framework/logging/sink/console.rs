use tracing::Subscriber;
use tracing_subscriber::Layer;
use tracing_subscriber::fmt;
use tracing_subscriber::registry::LookupSpan;

use super::super::config::{ConsoleSinkConfig, ConsoleTarget, FormatKind};

/// Builds a console sink layer configured from `cfg`.
///
/// Returns a type-erased `Box<dyn Layer<S>>` because the concrete type
/// differs between stdout/stderr and format variants at runtime.
///
/// # Filtering
///
/// An `EnvFilter` is attached to this layer, resolved by
/// [`super::build_env_filter`] with precedence `RUST_LOG` > `LOG_LEVEL` >
/// per-sink `level`, plus any `exclude_targets` as `prefix=off` directives.
/// Database and queue sinks use their own `level_passes` check and are
/// unaffected by `RUST_LOG` / `LOG_LEVEL`.
pub fn build_console_layer<S>(cfg: &ConsoleSinkConfig) -> Box<dyn Layer<S> + Send + Sync>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    let env_filter = super::build_env_filter(cfg.level, &cfg.exclude_targets);

    match (cfg.target, cfg.format) {
        (ConsoleTarget::Stdout, FormatKind::Pretty) => Box::new(
            fmt::layer()
                .pretty()
                .with_writer(std::io::stdout)
                .with_filter(env_filter),
        ),
        (ConsoleTarget::Stdout, FormatKind::Json) => Box::new(
            fmt::layer()
                .json()
                .with_writer(std::io::stdout)
                .with_filter(env_filter),
        ),
        (ConsoleTarget::Stdout, FormatKind::Compact) => Box::new(
            fmt::layer()
                .compact()
                .with_writer(std::io::stdout)
                .with_filter(env_filter),
        ),
        (ConsoleTarget::Stderr, FormatKind::Pretty) => Box::new(
            fmt::layer()
                .pretty()
                .with_writer(std::io::stderr)
                .with_filter(env_filter),
        ),
        (ConsoleTarget::Stderr, FormatKind::Json) => Box::new(
            fmt::layer()
                .json()
                .with_writer(std::io::stderr)
                .with_filter(env_filter),
        ),
        (ConsoleTarget::Stderr, FormatKind::Compact) => Box::new(
            fmt::layer()
                .compact()
                .with_writer(std::io::stderr)
                .with_filter(env_filter),
        ),
    }
}
