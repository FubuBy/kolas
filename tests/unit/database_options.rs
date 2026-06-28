use kolas::framework::database::{
    ConnectionConfig, DatabaseError, mysql_options, pg_options, sqlite_options,
};

fn make(toml_str: &str) -> ConnectionConfig {
    toml::from_str(toml_str).expect("connection config must parse")
}

#[test]
fn pg_options_populate_fields_from_components() {
    let c = make(
        r#"
        driver = "postgres"
        host = "db.example"
        port = 6543
        database = "app"
        username = "u"
        password = "p"
        "#,
    );
    let opts = pg_options("primary", &c).unwrap();
    assert_eq!(opts.get_host(), "db.example");
    assert_eq!(opts.get_port(), 6543);
    assert_eq!(opts.get_username(), "u");
    assert_eq!(opts.get_database(), Some("app"));
}

#[test]
fn pg_options_use_sqlx_default_port_when_unset() {
    let c = make(
        r#"
        driver = "postgres"
        host = "h"
        database = "d"
        "#,
    );
    let opts = pg_options("primary", &c).unwrap();
    // No port set explicitly → SQLx default. We don't hardcode 5432
    // anywhere in our code; we trust SQLx for the default.
    assert_eq!(opts.get_port(), 5432);
}

#[test]
fn pg_options_parse_explicit_url_with_query_params() {
    let c = make(
        r#"
        driver = "postgres"
        url = "postgres://u:p@h:7777/d?sslmode=require"
        "#,
    );
    let opts = pg_options("primary", &c).unwrap();
    assert_eq!(opts.get_host(), "h");
    assert_eq!(opts.get_port(), 7777);
    assert_eq!(opts.get_username(), "u");
    assert_eq!(opts.get_database(), Some("d"));
}

#[test]
fn pg_options_report_missing_host() {
    let c = make(
        r#"
        driver = "postgres"
        database = "d"
        "#,
    );
    let err = pg_options("primary", &c).unwrap_err();
    assert!(matches!(err, DatabaseError::MissingField { field, .. } if field == "host"));
}

#[test]
fn mysql_options_use_sqlx_default_port_when_unset() {
    let c = make(
        r#"
        driver = "mysql"
        host = "h"
        database = "d"
        "#,
    );
    let opts = mysql_options("primary", &c).unwrap();
    assert_eq!(opts.get_host(), "h");
    // SQLx default for MySQL is 3306 — we don't hardcode it.
    assert_eq!(opts.get_port(), 3306);
}

#[test]
fn mysql_options_parse_explicit_url() {
    let c = make(
        r#"
        driver = "mysql"
        url = "mysql://u:p@h:4242/d"
        "#,
    );
    let opts = mysql_options("primary", &c).unwrap();
    assert_eq!(opts.get_host(), "h");
    assert_eq!(opts.get_port(), 4242);
}

#[test]
fn mysql_options_report_missing_database() {
    let c = make(
        r#"
        driver = "mysql"
        host = "h"
        "#,
    );
    let err = mysql_options("primary", &c).unwrap_err();
    assert!(matches!(err, DatabaseError::MissingField { field, .. } if field == "database"));
}

#[test]
fn sqlite_options_require_explicit_url() {
    let c = make(r#"driver = "sqlite""#);
    let err = sqlite_options("cache", &c).unwrap_err();
    assert!(matches!(err, DatabaseError::MissingField { field, .. } if field == "url"));
}

#[test]
fn sqlite_options_parse_in_memory_url() {
    let c = make(
        r#"
        driver = "sqlite"
        url = "sqlite::memory:"
        "#,
    );
    // SQLx normalizes `:memory:` to its own internal form; we only assert
    // that parsing succeeds.
    sqlite_options("test", &c).expect("sqlite memory url parses");
}

#[test]
fn sqlite_options_carry_filename_from_file_url() {
    let c = make(
        r#"
        driver = "sqlite"
        url = "sqlite:./storage/cache.sqlite"
        "#,
    );
    let opts = sqlite_options("cache", &c).unwrap();
    assert!(
        opts.get_filename()
            .to_string_lossy()
            .contains("storage/cache.sqlite")
    );
}
