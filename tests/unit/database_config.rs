use kolas::framework::database::{DatabaseConfig, DriverKind};

fn parse(toml_str: &str) -> DatabaseConfig {
    toml::from_str(toml_str).expect("config must parse")
}

#[test]
fn parses_full_section() {
    let cfg = parse(
        r#"
        default = "primary"
        auto_migrate = true
        migrations_path = "./db/migs"

        [connections.primary]
        driver = "mysql"
        host = "127.0.0.1"
        port = 3306
        database = "kolas"
        username = "root"
        password = "secret"
        read = ["replica1", "replica2"]

        [connections.primary.pool]
        max = 20
        min = 5
        acquire_timeout_ms = 7000
        idle_timeout_ms = 60000
        max_lifetime_ms = 600000

        [connections.replica1]
        driver = "mysql"
        host = "10.0.0.5"
        port = 3306
        database = "kolas"
        username = "ro"
        "#,
    );

    assert_eq!(cfg.default.as_deref(), Some("primary"));
    assert!(cfg.auto_migrate);
    assert_eq!(cfg.migrations_path.as_deref(), Some("./db/migs"));

    let primary = cfg.connections.get("primary").expect("primary must exist");
    assert_eq!(primary.driver, DriverKind::Mysql);
    assert_eq!(primary.host.as_deref(), Some("127.0.0.1"));
    assert_eq!(
        primary.read,
        vec!["replica1".to_string(), "replica2".to_string()]
    );
    assert_eq!(primary.pool.max, Some(20));
    assert_eq!(primary.pool.acquire_timeout_ms, Some(7000));

    assert!(cfg.connections.contains_key("replica1"));
}

#[test]
fn defaults_are_safe_when_section_is_empty() {
    let cfg: DatabaseConfig = toml::from_str("").unwrap();
    assert!(cfg.default.is_none());
    assert!(!cfg.auto_migrate);
    assert!(cfg.migrations_path.is_none());
    assert!(cfg.connections.is_empty());
}

#[test]
fn auto_migrate_defaults_to_false() {
    let cfg = parse(r#"default = "primary""#);
    assert!(!cfg.auto_migrate);
}

#[test]
fn migrations_path_falls_back_to_default_when_unset() {
    let cfg = parse(r#"default = "primary""#);
    assert_eq!(cfg.migrations_path(), "./database/migrations");
}

#[test]
fn migrations_path_returns_explicit_value() {
    let cfg = parse(r#"migrations_path = "/var/lib/migs""#);
    assert_eq!(cfg.migrations_path(), "/var/lib/migs");
}

#[test]
fn unknown_driver_is_rejected_at_parse_time() {
    let parsed: Result<DatabaseConfig, _> = toml::from_str(
        r#"
        [connections.primary]
        driver = "oracle"
        "#,
    );
    assert!(parsed.is_err(), "oracle is not a recognised driver");
}

#[test]
fn read_defaults_to_empty_vec() {
    let cfg = parse(
        r#"
        [connections.primary]
        driver = "sqlite"
        url = "sqlite::memory:"
        "#,
    );
    let primary = cfg.connections.get("primary").unwrap();
    assert!(primary.read.is_empty());
    assert!(primary.pool == kolas::framework::database::PoolConfig::default());
}

#[test]
fn migrations_path_for_prefers_connection_override() {
    let cfg = parse(
        r#"
        migrations_path = "./db/global"

        [connections.pg]
        driver = "postgres"
        url = "postgres://localhost/app"
        migrations_path = "./db/postgres"

        [connections.analytics]
        driver = "mysql"
        url = "mysql://localhost/analytics"
        "#,
    );
    // Own override wins.
    assert_eq!(cfg.migrations_path_for("pg"), "./db/postgres");
    // No override → global path.
    assert_eq!(cfg.migrations_path_for("analytics"), "./db/global");
    // Unknown connection → global path.
    assert_eq!(cfg.migrations_path_for("missing"), "./db/global");
}

#[test]
fn resolve_connection_uses_request_then_default() {
    let cfg = parse(r#"default = "primary""#);
    assert_eq!(
        cfg.resolve_connection(Some("analytics")).unwrap(),
        "analytics"
    );
    assert_eq!(cfg.resolve_connection(None).unwrap(), "primary");

    let no_default = parse("");
    assert!(no_default.resolve_connection(None).is_err());
}

#[test]
fn auto_migrate_targets_covers_default_plus_explicit_paths() {
    let cfg = parse(
        r#"
        default = "pg"
        migrations_path = "./db/global"

        [connections.pg]
        driver = "postgres"
        url = "postgres://localhost/app"

        [connections.analytics]
        driver = "mysql"
        url = "mysql://localhost/analytics"
        migrations_path = "./db/mysql"

        [connections.cache]
        driver = "sqlite"
        url = "sqlite::memory:"
        "#,
    );

    let targets = cfg.auto_migrate_targets();
    // Default (pg, via global path) first, then connections with explicit
    // paths (analytics). `cache` has no path and isn't default → skipped.
    assert_eq!(
        targets,
        vec![
            ("pg".to_string(), "./db/global".to_string()),
            ("analytics".to_string(), "./db/mysql".to_string()),
        ]
    );
}

#[test]
fn auto_migrate_targets_empty_without_default_or_paths() {
    let cfg = parse(
        r#"
        [connections.cache]
        driver = "sqlite"
        url = "sqlite::memory:"
        "#,
    );
    assert!(cfg.auto_migrate_targets().is_empty());
}
