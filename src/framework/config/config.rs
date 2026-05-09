use std::path::Path;
use std::sync::OnceLock;

use serde::Deserialize;
use toml::Value;
use toml::map::Map;

use super::error::ConfigError;

static GLOBAL: OnceLock<Config> = OnceLock::new();

/// Immutable snapshot of the application configuration.
#[derive(Debug, Clone)]
pub struct Config {
    root: Value,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            root: Value::Table(Map::new()),
        }
    }
}

impl Config {
    /// Scans a directory and loads every `*.toml` file, using the file stem as
    /// the top-level namespace. Then applies overrides from `.env` and process
    /// environment variables.
    pub fn load(dir: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let _ = dotenvy::dotenv();

        let dir = dir.as_ref();
        let mut root = Map::new();

        let entries = std::fs::read_dir(dir).map_err(|e| ConfigError::ReadDir {
            path: dir.display().to_string(),
            source: e,
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| ConfigError::ReadDir {
                path: dir.display().to_string(),
                source: e,
            })?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("toml") {
                continue;
            }
            let stem = match path.file_stem().and_then(|s| s.to_str()) {
                Some(s) => s.to_owned(),
                None => continue,
            };

            let text = std::fs::read_to_string(&path).map_err(|e| ConfigError::ReadFile {
                path: path.display().to_string(),
                source: e,
            })?;

            let value: Value = toml::from_str(&text).map_err(|e| ConfigError::Parse {
                file: stem.clone(),
                source: e,
            })?;

            if !value.is_table() {
                return Err(ConfigError::NotTable(stem));
            }

            root.insert(stem, value);
        }

        let mut config = Config {
            root: Value::Table(root),
        };
        config.apply_overrides(std::env::vars());
        Ok(config)
    }

    /// Builds a config from in-memory sections. Each `(name, toml_str)` pair
    /// becomes a top-level section, as if it had been a file in `config/`.
    /// Does not touch the filesystem and does not apply environment overrides —
    /// useful for tests and in-memory composition.
    pub fn from_sections<I, K, V>(sections: I) -> Result<Self, ConfigError>
    where
        I: IntoIterator<Item = (K, V)>,
        K: Into<String>,
        V: AsRef<str>,
    {
        let mut root = Map::new();
        for (name, content) in sections {
            let name: String = name.into();
            let value: Value =
                toml::from_str(content.as_ref()).map_err(|e| ConfigError::Parse {
                    file: name.clone(),
                    source: e,
                })?;
            if !value.is_table() {
                return Err(ConfigError::NotTable(name));
            }
            root.insert(name, value);
        }
        Ok(Config {
            root: Value::Table(root),
        })
    }

    /// Registers this config as a global singleton. Subsequent calls are ignored.
    pub fn install_global(self) {
        let _ = GLOBAL.set(self);
    }

    /// Returns the global instance. Panics if `install_global` has not been
    /// called yet.
    pub fn global() -> &'static Self {
        GLOBAL
            .get()
            .expect("Config is not initialized; call Config::load(...).install_global() at startup")
    }

    // === Instance API (used in tests and when injected manually) ===

    /// Returns the value at the given dot-path deserialized into `T`, or
    /// `default` if the path is missing or the type does not match.
    pub fn value<T>(&self, path: &str, default: T) -> T
    where
        T: for<'de> Deserialize<'de>,
    {
        self.try_value(path).unwrap_or(default)
    }

    /// Same as `value`, but returns `None` instead of a default.
    pub fn try_value<T>(&self, path: &str) -> Option<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        let value = self.resolve(path)?.clone();
        T::deserialize(value).ok()
    }

    /// Returns whether the given dot-path exists in the configuration tree.
    pub fn has_key(&self, path: &str) -> bool {
        self.resolve(path).is_some()
    }

    // === Static facade — primary entry point for application code ===

    /// Shortcut for `Config::global().value(...)`.
    pub fn get<T>(path: &str, default: T) -> T
    where
        T: for<'de> Deserialize<'de>,
    {
        Self::global().value(path, default)
    }

    /// Shortcut for `Config::global().try_value(...)`.
    pub fn try_get<T>(path: &str) -> Option<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        Self::global().try_value(path)
    }

    /// Shortcut for `Config::global().has_key(...)`.
    pub fn has(path: &str) -> bool {
        Self::global().has_key(path)
    }

    /// Applies overrides from an arbitrary `(key, value)` iterator.
    /// Used both by `load()` and by tests with synthetic variables.
    pub fn apply_overrides<I>(&mut self, vars: I)
    where
        I: IntoIterator<Item = (String, String)>,
    {
        let known_sections: Vec<String> = match &self.root {
            Value::Table(table) => table.keys().cloned().collect(),
            _ => return,
        };

        for (key, raw) in vars {
            if !key.contains("__") {
                continue;
            }
            let parts: Vec<String> = key.split("__").map(|s| s.to_lowercase()).collect();
            if parts.len() < 2 {
                continue;
            }
            // Only override within already known sections; otherwise any
            // unrelated env var like `FOO__BAR` would create a phantom `foo`
            // section that no TOML file is responsible for.
            if !known_sections.contains(&parts[0]) {
                continue;
            }
            self.set_path(&parts, parse_env_value(&raw));
        }
    }

    // === Internals ===

    fn resolve(&self, path: &str) -> Option<&Value> {
        let mut current = &self.root;
        for part in path.split('.') {
            current = match current {
                Value::Table(table) => table.get(part)?,
                _ => return None,
            };
        }
        Some(current)
    }

    fn set_path(&mut self, path: &[String], value: Value) {
        let Value::Table(root) = &mut self.root else {
            return;
        };
        let mut cursor = root;
        for key in &path[..path.len() - 1] {
            let entry = cursor
                .entry(key.clone())
                .or_insert_with(|| Value::Table(Map::new()));
            if !entry.is_table() {
                *entry = Value::Table(Map::new());
            }
            cursor = entry
                .as_table_mut()
                .expect("cursor must be a table after the check above");
        }
        cursor.insert(path.last().unwrap().clone(), value);
    }
}

fn parse_env_value(raw: &str) -> Value {
    if let Ok(b) = raw.parse::<bool>() {
        return Value::Boolean(b);
    }
    if let Ok(i) = raw.parse::<i64>() {
        return Value::Integer(i);
    }
    if let Ok(f) = raw.parse::<f64>() {
        return Value::Float(f);
    }
    Value::String(raw.to_string())
}
