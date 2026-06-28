use kolas::framework::logging::LevelFilter;
use kolas::framework::logging::sink::level_passes;
use tracing_subscriber::filter::LevelFilter as SubsFilter;

#[test]
fn level_filter_converts_to_tracing_subscriber() {
    assert_eq!(SubsFilter::from(LevelFilter::Trace), SubsFilter::TRACE);
    assert_eq!(SubsFilter::from(LevelFilter::Debug), SubsFilter::DEBUG);
    assert_eq!(SubsFilter::from(LevelFilter::Info), SubsFilter::INFO);
    assert_eq!(SubsFilter::from(LevelFilter::Warn), SubsFilter::WARN);
    assert_eq!(SubsFilter::from(LevelFilter::Error), SubsFilter::ERROR);
    assert_eq!(SubsFilter::from(LevelFilter::Off), SubsFilter::OFF);
}

#[test]
fn level_filter_default_is_info() {
    assert_eq!(LevelFilter::default(), LevelFilter::Info);
}

#[test]
fn level_filter_equality() {
    assert_eq!(LevelFilter::Debug, LevelFilter::Debug);
    assert_ne!(LevelFilter::Debug, LevelFilter::Info);
}

/// M-4: `Off` must block ALL events — it must not be misinterpreted as INFO.
#[test]
fn level_filter_off_blocks_everything() {
    // Every tracing level must be blocked when the filter is Off.
    assert!(!level_passes(&tracing::Level::ERROR, LevelFilter::Off));
    assert!(!level_passes(&tracing::Level::WARN, LevelFilter::Off));
    assert!(!level_passes(&tracing::Level::INFO, LevelFilter::Off));
    assert!(!level_passes(&tracing::Level::DEBUG, LevelFilter::Off));
    assert!(!level_passes(&tracing::Level::TRACE, LevelFilter::Off));
}

/// Sanity-check that Trace passes everything and Error passes only Error.
#[test]
fn level_filter_trace_passes_all_error_passes_only_error() {
    assert!(level_passes(&tracing::Level::TRACE, LevelFilter::Trace));
    assert!(level_passes(&tracing::Level::DEBUG, LevelFilter::Trace));
    assert!(level_passes(&tracing::Level::INFO, LevelFilter::Trace));
    assert!(level_passes(&tracing::Level::WARN, LevelFilter::Trace));
    assert!(level_passes(&tracing::Level::ERROR, LevelFilter::Trace));

    assert!(level_passes(&tracing::Level::ERROR, LevelFilter::Error));
    assert!(!level_passes(&tracing::Level::WARN, LevelFilter::Error));
    assert!(!level_passes(&tracing::Level::INFO, LevelFilter::Error));
}

#[test]
fn level_filter_deserializes_from_lowercase_string() {
    let filter: LevelFilter = toml::from_str(r#"value = "debug""#)
        .ok()
        .and_then(|t: toml::Table| t.get("value").cloned())
        .and_then(|v| v.try_into().ok())
        .expect("must deserialize");
    assert_eq!(filter, LevelFilter::Debug);
}
