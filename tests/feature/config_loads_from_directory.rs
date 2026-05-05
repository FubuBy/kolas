use std::fs;

use kolas::framework::config::Config;
use tempfile::TempDir;

fn write(dir: &TempDir, name: &str, body: &str) {
    fs::write(dir.path().join(name), body).expect("write fixture");
}

#[test]
fn discovers_all_toml_files_and_namespaces_them_by_filename() {
    let dir = TempDir::new().unwrap();
    write(
        &dir,
        "app.toml",
        r#"
        name = "Kolas"
        port = 3000
        "#,
    );
    write(
        &dir,
        "database.toml",
        r#"
        default = "primary"
        [connections.primary]
        host = "127.0.0.1"
        "#,
    );

    let cfg = Config::load(dir.path()).expect("load");

    assert_eq!(cfg.value::<String>("app.name", "x".into()), "Kolas");
    assert_eq!(cfg.value::<u16>("app.port", 0), 3000);
    assert_eq!(
        cfg.value::<String>("database.default", "x".into()),
        "primary"
    );
    assert_eq!(
        cfg.value::<String>("database.connections.primary.host", "x".into()),
        "127.0.0.1"
    );
}

#[test]
fn skips_non_toml_files() {
    let dir = TempDir::new().unwrap();
    write(&dir, "app.toml", "name = \"Kolas\"\n");
    write(&dir, "readme.md", "# not a config");
    write(&dir, "ignored.json", "{\"x\": 1}");

    let cfg = Config::load(dir.path()).expect("load");
    assert_eq!(cfg.value::<String>("app.name", "x".into()), "Kolas");
    assert!(!cfg.has_key("readme"));
    assert!(!cfg.has_key("ignored"));
}

#[test]
fn fails_on_invalid_toml_with_filename_in_error() {
    let dir = TempDir::new().unwrap();
    write(&dir, "broken.toml", "not = valid = toml");

    let err = Config::load(dir.path()).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("broken"), "error must mention file: {msg}");
}
