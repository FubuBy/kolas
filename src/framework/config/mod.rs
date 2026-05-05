//! Application configuration loaded from `config/*.toml` files.
//!
//! Each file becomes a top-level namespace; values are accessed via
//! dot-notation (`Config::get("app.name", default)`) and can be overridden
//! by environment variables using the `<SECTION>__<PATH>` convention.

mod config;
mod error;

pub use config::Config;
pub use error::ConfigError;
