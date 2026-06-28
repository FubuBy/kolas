use kolas::framework::config::Config;
use kolas::framework::logging::{
    ConsoleTarget, FormatKind, LevelFilter, LoggingConfig, RotationKind, SinkConfig,
};

fn parse_logging(toml: &str) -> LoggingConfig {
    let cfg = Config::from_sections([("logging", toml)]).expect("config must parse");
    cfg.try_value("logging")
        .expect("logging section must deserialize")
}

#[test]
fn parses_default_level() {
    let cfg = parse_logging(
        r#"
        default_level = "warn"
        sinks = []
        "#,
    );
    assert_eq!(cfg.default_level, LevelFilter::Warn);
}

#[test]
fn parses_console_sink() {
    let cfg = parse_logging(
        r#"
        default_level = "info"

        [[sinks]]
        type   = "console"
        level  = "debug"
        format = "pretty"
        target = "stdout"
        "#,
    );
    assert_eq!(cfg.sinks.len(), 1);
    let SinkConfig::Console(ref c) = cfg.sinks[0] else {
        panic!("expected Console sink");
    };
    assert_eq!(c.level, LevelFilter::Debug);
    assert_eq!(c.format, FormatKind::Pretty);
    assert_eq!(c.target, ConsoleTarget::Stdout);
    assert!(c.exclude_targets.is_empty());
}

#[test]
fn parses_console_sink_with_exclude_targets() {
    let cfg = parse_logging(
        r#"
        default_level = "info"

        [[sinks]]
        type            = "console"
        level           = "info"
        format          = "json"
        target          = "stderr"
        exclude_targets = ["sqlx", "tower_http"]
        "#,
    );
    let SinkConfig::Console(ref c) = cfg.sinks[0] else {
        panic!("expected Console sink");
    };
    assert_eq!(c.exclude_targets, vec!["sqlx", "tower_http"]);
    assert_eq!(c.target, ConsoleTarget::Stderr);
    assert_eq!(c.format, FormatKind::Json);
}

#[test]
fn parses_file_sink() {
    let cfg = parse_logging(
        r#"
        default_level = "info"

        [[sinks]]
        type       = "file"
        level      = "info"
        format     = "json"
        path       = "./storage/logs"
        prefix     = "app"
        rotation   = "daily"
        keep_files = 7
        "#,
    );
    let SinkConfig::File(ref f) = cfg.sinks[0] else {
        panic!("expected File sink");
    };
    assert_eq!(f.level, LevelFilter::Info);
    assert_eq!(f.format, FormatKind::Json);
    assert_eq!(f.path, "./storage/logs");
    assert_eq!(f.prefix, "app");
    assert_eq!(f.rotation, RotationKind::Daily);
    assert_eq!(f.keep_files, Some(7));
}

#[test]
fn file_sink_keep_files_none() {
    let cfg = parse_logging(
        r#"
        default_level = "info"

        [[sinks]]
        type     = "file"
        path     = "./logs"
        prefix   = "app"
        "#,
    );
    let SinkConfig::File(ref f) = cfg.sinks[0] else {
        panic!("expected File sink");
    };
    // Default keep_files is Some(7)
    assert_eq!(f.keep_files, Some(7));
}

#[test]
fn parses_database_sink() {
    let cfg = parse_logging(
        r#"
        default_level = "info"

        [[sinks]]
        type              = "database"
        level             = "warn"
        connection        = "logs"
        table             = "log_entries"
        channel_capacity  = 1024
        batch_size        = 100
        flush_interval_ms = 1000
        "#,
    );
    let SinkConfig::Database(ref d) = cfg.sinks[0] else {
        panic!("expected Database sink");
    };
    assert_eq!(d.level, LevelFilter::Warn);
    assert_eq!(d.connection, "logs");
    assert_eq!(d.table, "log_entries");
    assert_eq!(d.channel_capacity, 1024);
    assert_eq!(d.batch_size, 100);
    assert_eq!(d.flush_interval_ms, 1000);
}

#[test]
fn parses_queue_sink() {
    let cfg = parse_logging(
        r#"
        default_level = "info"

        [[sinks]]
        type             = "queue"
        level            = "error"
        driver           = "null"
        channel_capacity = 512
        "#,
    );
    let SinkConfig::Queue(ref q) = cfg.sinks[0] else {
        panic!("expected Queue sink");
    };
    assert_eq!(q.level, LevelFilter::Error);
    assert_eq!(q.driver, "null");
    assert_eq!(q.channel_capacity, 512);
    assert!(q.connection.is_none());
}

#[test]
fn parses_multiple_sinks() {
    let cfg = parse_logging(
        r#"
        default_level = "info"

        [[sinks]]
        type   = "console"
        level  = "debug"
        format = "pretty"
        target = "stdout"

        [[sinks]]
        type     = "file"
        level    = "info"
        format   = "json"
        path     = "./logs"
        prefix   = "app"
        rotation = "daily"
        "#,
    );
    assert_eq!(cfg.sinks.len(), 2);
    assert!(matches!(cfg.sinks[0], SinkConfig::Console(_)));
    assert!(matches!(cfg.sinks[1], SinkConfig::File(_)));
}

#[test]
fn empty_sinks_list_is_valid() {
    let cfg = parse_logging(
        r#"
        default_level = "info"
        sinks = []
        "#,
    );
    assert!(cfg.sinks.is_empty());
}
