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
/// An `EnvFilter` is attached to this layer. The filter directive is built
/// from the per-sink `level` (and any `exclude_targets` as `prefix=off`
/// directives). If `RUST_LOG` is set in the environment, it is used *instead*
/// of the TOML-derived directive for **this layer only** — it does not affect
/// other sinks. Database and queue sinks use their own `level_passes` check
/// and are unaffected by `RUST_LOG`.
pub fn build_console_layer<S>(cfg: &ConsoleSinkConfig) -> Box<dyn Layer<S> + Send + Sync>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    let level_filter: tracing_subscriber::filter::LevelFilter = cfg.level.into();

    // Combine level with exclude_targets as `prefix=off` directives.
    // RUST_LOG overrides the whole directive string when set.
    let mut directives = vec![level_filter.to_string()];
    for prefix in &cfg.exclude_targets {
        directives.push(format!("{prefix}=off"));
    }
    let filter_str = directives.join(",");

    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(filter_str));

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
