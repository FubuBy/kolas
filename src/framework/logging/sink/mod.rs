pub mod console;
pub mod database;
pub mod file;
pub mod queue;

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
