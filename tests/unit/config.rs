use kolas::framework::config::Config;
use serde::Deserialize;

fn fixture() -> Config {
    Config::from_sections([
        (
            "app",
            r#"
            name = "Kolas"
            debug = true
            port = 3000

            [mail]
            from = "no-reply@example.com"
            "#,
        ),
        (
            "database",
            r#"
            default = "primary"

            [connections.primary]
            host = "127.0.0.1"
            port = 5432
            "#,
        ),
    ])
    .expect("fixture must build")
}

#[test]
fn returns_string_value_by_dot_path() {
    let cfg = fixture();
    let name: String = cfg.value("app.name", "fallback".into());
    assert_eq!(name, "Kolas");
}

#[test]
fn returns_default_when_path_missing() {
    let cfg = fixture();
    let unknown: String = cfg.value("app.unknown", "fallback".into());
    assert_eq!(unknown, "fallback");
}

#[test]
fn supports_typed_integer_and_bool() {
    let cfg = fixture();
    let port: u16 = cfg.value("app.port", 0);
    let debug: bool = cfg.value("app.debug", false);
    assert_eq!(port, 3000);
    assert!(debug);
}

#[test]
fn returns_default_when_type_mismatches() {
    let cfg = fixture();
    let port_as_string: String = cfg.value("app.port", "fallback".into());
    assert_eq!(port_as_string, "fallback");
}

#[test]
fn navigates_nested_tables() {
    let cfg = fixture();
    let host: String = cfg.value("database.connections.primary.host", "x".into());
    let port: u16 = cfg.value("database.connections.primary.port", 0);
    assert_eq!(host, "127.0.0.1");
    assert_eq!(port, 5432);
}

#[test]
fn try_value_returns_none_for_missing_path() {
    let cfg = fixture();
    let v: Option<String> = cfg.try_value("app.absent");
    assert!(v.is_none());
}

#[test]
fn has_key_reflects_presence() {
    let cfg = fixture();
    assert!(cfg.has_key("app.name"));
    assert!(cfg.has_key("database.connections.primary"));
    assert!(!cfg.has_key("app.absent"));
    assert!(!cfg.has_key("nope"));
}

#[test]
fn deserializes_subtree_into_struct() {
    #[derive(Deserialize)]
    struct PrimaryConn {
        host: String,
        port: u16,
    }

    let cfg = fixture();
    let conn: PrimaryConn = cfg
        .try_value("database.connections.primary")
        .expect("primary connection must deserialize");
    assert_eq!(conn.host, "127.0.0.1");
    assert_eq!(conn.port, 5432);
}

#[test]
fn env_overrides_apply_to_known_sections() {
    let mut cfg = fixture();
    cfg.apply_overrides([
        ("APP__NAME".into(), "Kolas Prod".into()),
        ("APP__PORT".into(), "8080".into()),
        ("APP__DEBUG".into(), "false".into()),
        (
            "DATABASE__CONNECTIONS__PRIMARY__HOST".into(),
            "db.prod.internal".into(),
        ),
    ]);

    assert_eq!(cfg.value::<String>("app.name", "x".into()), "Kolas Prod");
    assert_eq!(cfg.value::<u16>("app.port", 0), 8080);
    assert!(!cfg.value::<bool>("app.debug", true));
    assert_eq!(
        cfg.value::<String>("database.connections.primary.host", "x".into()),
        "db.prod.internal"
    );
}

#[test]
fn env_ignores_unknown_top_level_sections() {
    let mut cfg = fixture();
    cfg.apply_overrides([("PATH".into(), "/usr/bin".into())]);
    assert!(!cfg.has_key("path"));

    cfg.apply_overrides([("UNRELATED__FOO".into(), "bar".into())]);
    assert!(!cfg.has_key("unrelated"));
}

#[test]
fn env_ignores_single_underscore_keys() {
    let mut cfg = fixture();
    cfg.apply_overrides([("APP_NAME".into(), "should-not-apply".into())]);
    assert_eq!(cfg.value::<String>("app.name", "x".into()), "Kolas");
}

#[test]
fn env_creates_intermediate_tables_on_demand() {
    let mut cfg = fixture();
    cfg.apply_overrides([("APP__FEATURES__BETA__ENABLED".into(), "true".into())]);
    assert!(cfg.value::<bool>("app.features.beta.enabled", false));
}
