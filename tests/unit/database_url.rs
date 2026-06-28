use kolas::framework::database::{ConnectionConfig, DatabaseError, any_url};

fn make(toml_str: &str) -> ConnectionConfig {
    toml::from_str(toml_str).expect("connection config must parse")
}

#[test]
fn postgres_url_includes_credentials_and_explicit_port() {
    let c = make(
        r#"
        driver = "postgres"
        host = "db.example"
        port = 5432
        database = "app"
        username = "u"
        password = "p"
        "#,
    );
    assert_eq!(any_url("primary", &c).unwrap(), "postgres://u:p@db.example:5432/app");
}

#[test]
fn mysql_url_omits_port_when_unset() {
    let c = make(
        r#"
        driver = "mysql"
        host = "db.example"
        database = "app"
        username = "u"
        password = "p"
        "#,
    );
    // No `port` in config → no `:port` in the URL. SQLx will fill in 3306
    // (the MySQL default) when parsing.
    assert_eq!(any_url("primary", &c).unwrap(), "mysql://u:p@db.example/app");
}

#[test]
fn postgres_url_omits_port_when_unset() {
    let c = make(
        r#"
        driver = "postgres"
        host = "db.example"
        database = "app"
        "#,
    );
    assert_eq!(any_url("primary", &c).unwrap(), "postgres://db.example/app");
}

#[test]
fn explicit_url_takes_precedence_over_components() {
    let c = make(
        r#"
        driver = "postgres"
        url = "postgres://override/db"
        host = "ignored"
        database = "ignored"
        "#,
    );
    assert_eq!(any_url("primary", &c).unwrap(), "postgres://override/db");
}

#[test]
fn sqlite_without_url_yields_missing_field() {
    let c = make(r#"driver = "sqlite""#);
    let err = any_url("cache", &c).expect_err("sqlite needs url");
    match err {
        DatabaseError::MissingField { name, field } => {
            assert_eq!(name, "cache");
            assert_eq!(field, "url");
        }
        other => panic!("expected MissingField, got {other:?}"),
    }
}

#[test]
fn missing_host_for_postgres_is_reported() {
    let c = make(
        r#"
        driver = "postgres"
        database = "app"
        "#,
    );
    let err = any_url("primary", &c).expect_err("host is required");
    assert!(matches!(err, DatabaseError::MissingField { field, .. } if field == "host"));
}

#[test]
fn missing_database_for_mysql_is_reported() {
    let c = make(
        r#"
        driver = "mysql"
        host = "h"
        "#,
    );
    let err = any_url("primary", &c).expect_err("database is required");
    assert!(matches!(err, DatabaseError::MissingField { field, .. } if field == "database"));
}

#[test]
fn username_only_yields_userinfo_without_colon() {
    let c = make(
        r#"
        driver = "postgres"
        host = "h"
        database = "d"
        username = "u"
        "#,
    );
    assert_eq!(any_url("primary", &c).unwrap(), "postgres://u@h/d");
}

#[test]
fn empty_credentials_omit_userinfo() {
    let c = make(
        r#"
        driver = "mysql"
        host = "h"
        database = "d"
        "#,
    );
    assert_eq!(any_url("primary", &c).unwrap(), "mysql://h/d");
}
