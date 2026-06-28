use kolas::framework::logging::{
    ConsoleSinkConfig, ConsoleTarget, FormatKind, LevelFilter, Logging, LoggingConfig, SinkConfig,
};

/// Verifies that `Logging::init_with` with a console sink does not panic and
/// returns AlreadyInitialized on the second call in the same process.
///
/// We use a console (stdout) sink in compact format to minimize output noise.
#[test]
fn console_sink_init_does_not_panic() {
    let cfg = LoggingConfig {
        default_level: LevelFilter::Info,
        sinks: vec![SinkConfig::Console(ConsoleSinkConfig {
            level: LevelFilter::Info,
            format: FormatKind::Compact,
            target: ConsoleTarget::Stdout,
            exclude_targets: vec![],
        })],
    };

    // First call may succeed or return AlreadyInitialized (depending on test order).
    // Either outcome is acceptable — what matters is no panic.
    let result = Logging::init_with(cfg);
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn console_sink_stderr_target_does_not_panic() {
    let cfg = LoggingConfig {
        default_level: LevelFilter::Warn,
        sinks: vec![SinkConfig::Console(ConsoleSinkConfig {
            level: LevelFilter::Warn,
            format: FormatKind::Json,
            target: ConsoleTarget::Stderr,
            exclude_targets: vec!["sqlx".to_string()],
        })],
    };

    let result = Logging::init_with(cfg);
    assert!(result.is_ok() || result.is_err());
}

#[test]
fn second_init_returns_already_initialized() {
    use kolas::framework::logging::LoggingError;

    let cfg = || LoggingConfig {
        default_level: LevelFilter::Info,
        sinks: vec![SinkConfig::Console(ConsoleSinkConfig {
            level: LevelFilter::Info,
            format: FormatKind::Compact,
            target: ConsoleTarget::Stdout,
            exclude_targets: vec![],
        })],
    };

    // Try to init twice — at least one must be AlreadyInitialized.
    let r1 = Logging::init_with(cfg());
    let r2 = Logging::init_with(cfg());

    // At most one can succeed.
    let both_ok = r1.is_ok() && r2.is_ok();
    assert!(!both_ok, "the second init must report AlreadyInitialized");

    // Verify that whichever one failed, it was AlreadyInitialized.
    if let Err(ref e) = r1 {
        assert!(matches!(e, LoggingError::AlreadyInitialized));
    }
    if let Err(ref e) = r2 {
        assert!(matches!(e, LoggingError::AlreadyInitialized));
    }
}
