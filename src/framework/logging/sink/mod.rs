pub mod console;
pub mod database;
pub mod file;
pub mod queue;

use tracing_subscriber::EnvFilter;

use super::config::LevelFilter;

/// Returns `true` when `event_level` satisfies the configured `filter`.
/// `Off` never passes; `Trace` passes everything.
pub fn level_passes(event_level: &tracing::Level, filter: LevelFilter) -> bool {
    match filter {
        LevelFilter::Off => false,
        LevelFilter::Error => *event_level <= tracing::Level::ERROR,
        LevelFilter::Warn => *event_level <= tracing::Level::WARN,
        LevelFilter::Info => *event_level <= tracing::Level::INFO,
        LevelFilter::Debug => *event_level <= tracing::Level::DEBUG,
        LevelFilter::Trace => true,
    }
}

/// Builds the `EnvFilter` for a console or file layer, resolving the level from
/// the environment with the following precedence (highest first):
///
/// 1. **`RUST_LOG`** — a full tracing directive string (e.g.
///    `info,tower_http=trace`). Power-user override: when set it replaces the
///    whole directive for this layer.
/// 2. **`LOG_LEVEL`** — a single level word (`trace`/`debug`/`info`/`warn`/
///    `error`/`off`). Sets the minimum level for this layer while still applying
///    `exclude_targets`. Intended as the simple prod/local knob
///    (`info` in production, `debug` locally). An unknown value is ignored with
///    a warning and the configured level is used instead.
/// 3. The per-sink **`level`** from `logging.toml`.
///
/// `exclude_targets` are appended as `prefix=off` directives in cases 2 and 3.
/// Database and queue sinks do not use this — they filter via [`level_passes`]
/// and are unaffected by `RUST_LOG` / `LOG_LEVEL`.
pub(crate) fn build_env_filter(level: LevelFilter, exclude_targets: &[String]) -> EnvFilter {
    // 1. RUST_LOG wins outright.
    if let Ok(filter) = EnvFilter::try_from_default_env() {
        return filter;
    }

    // 2/3. Base level directive from LOG_LEVEL, falling back to the TOML level.
    let log_level = std::env::var("LOG_LEVEL").ok();
    let level_directive = resolve_level_directive(log_level.as_deref(), level);

    let mut directives = vec![level_directive];
    for prefix in exclude_targets {
        directives.push(format!("{prefix}=off"));
    }
    EnvFilter::new(directives.join(","))
}

/// Pure resolution of the base level directive from an optional `LOG_LEVEL`
/// value, falling back to the configured per-sink level when it is absent,
/// empty, or not a recognized level word. Split out from environment access so
/// it can be unit-tested without mutating process-wide env vars.
fn resolve_level_directive(log_level: Option<&str>, configured: LevelFilter) -> String {
    match log_level {
        Some(raw) => {
            let candidate = raw.trim().to_ascii_lowercase();
            if matches!(
                candidate.as_str(),
                "trace" | "debug" | "info" | "warn" | "error" | "off"
            ) {
                candidate
            } else {
                if !candidate.is_empty() {
                    eprintln!(
                        "[kolas/log] ignoring invalid LOG_LEVEL='{raw}'; using configured level"
                    );
                }
                configured_level_directive(configured)
            }
        }
        None => configured_level_directive(configured),
    }
}

/// Renders the configured per-sink level as an `EnvFilter` directive string.
fn configured_level_directive(level: LevelFilter) -> String {
    let lf: tracing_subscriber::filter::LevelFilter = level.into();
    lf.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_level_valid_value_is_used_lowercased() {
        assert_eq!(
            resolve_level_directive(Some("DEBUG"), LevelFilter::Info),
            "debug"
        );
        assert_eq!(
            resolve_level_directive(Some("  info "), LevelFilter::Error),
            "info"
        );
        assert_eq!(
            resolve_level_directive(Some("off"), LevelFilter::Trace),
            "off"
        );
    }

    #[test]
    fn log_level_invalid_or_empty_falls_back_to_configured() {
        let configured = configured_level_directive(LevelFilter::Warn);
        assert_eq!(
            resolve_level_directive(Some("verbose"), LevelFilter::Warn),
            configured
        );
        assert_eq!(
            resolve_level_directive(Some(""), LevelFilter::Warn),
            configured
        );
        assert_eq!(
            resolve_level_directive(Some("   "), LevelFilter::Warn),
            configured
        );
    }

    #[test]
    fn log_level_absent_uses_configured() {
        assert_eq!(
            resolve_level_directive(None, LevelFilter::Debug),
            configured_level_directive(LevelFilter::Debug)
        );
    }
}
