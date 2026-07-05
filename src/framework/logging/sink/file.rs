use std::path::Path;
use std::time::Duration;

use tokio::task::JoinHandle;
use tracing::Subscriber;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::Layer;
use tracing_subscriber::fmt;
use tracing_subscriber::registry::LookupSpan;

use super::super::config::{FileSinkConfig, FormatKind, RotationKind};
use super::super::error::LoggingError;

/// Builds a file sink layer with rolling rotation.
///
/// Returns `(Layer, WorkerGuard, JoinHandle)`:
/// - `WorkerGuard` must be kept alive for the duration of the program; dropping
///   it flushes and closes the background I/O thread.
/// - `JoinHandle` drives the retention cleanup task; abort it on shutdown if
///   desired, or simply let it be dropped.
///
/// # Filtering
///
/// An `EnvFilter` is attached to this layer, resolved by
/// [`super::build_env_filter`] with precedence `RUST_LOG` > `LOG_LEVEL` >
/// per-sink `level`, plus any `exclude_targets` as `prefix=off` directives.
/// Database and queue sinks use their own `level_passes` check and are
/// unaffected by `RUST_LOG` / `LOG_LEVEL`.
#[allow(clippy::type_complexity)]
pub fn build_file_layer<S>(
    cfg: &FileSinkConfig,
) -> Result<(Box<dyn Layer<S> + Send + Sync>, WorkerGuard, JoinHandle<()>), LoggingError>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    // Ensure the log directory exists before handing it to the appender.
    let log_dir = Path::new(&cfg.path);
    std::fs::create_dir_all(log_dir).map_err(|source| LoggingError::FilePathError {
        path: cfg.path.clone(),
        source,
    })?;

    // Build the rolling appender for the chosen rotation policy. `filename_suffix`
    // gives files a `.log` extension regardless of rotation (e.g. `app.2026-07-05.log`,
    // or just `app.log` for `Never`).
    let rotation = match cfg.rotation {
        RotationKind::Hourly => tracing_appender::rolling::Rotation::HOURLY,
        RotationKind::Daily => tracing_appender::rolling::Rotation::DAILY,
        RotationKind::Never => tracing_appender::rolling::Rotation::NEVER,
    };
    let appender = tracing_appender::rolling::RollingFileAppender::builder()
        .rotation(rotation)
        .filename_prefix(&cfg.prefix)
        .filename_suffix("log")
        .build(log_dir)
        .map_err(|source| LoggingError::FilePathError {
            path: cfg.path.clone(),
            source: std::io::Error::other(source),
        })?;

    let (non_blocking, guard) = tracing_appender::non_blocking(appender);

    let env_filter = super::build_env_filter(cfg.level, &cfg.exclude_targets);

    let layer: Box<dyn Layer<S> + Send + Sync> = match cfg.format {
        FormatKind::Json => Box::new(
            fmt::layer()
                .json()
                .with_writer(non_blocking)
                .with_filter(env_filter),
        ),
        FormatKind::Pretty => Box::new(
            fmt::layer()
                .pretty()
                .with_writer(non_blocking)
                .with_filter(env_filter),
        ),
        FormatKind::Compact => Box::new(
            fmt::layer()
                .compact()
                .with_writer(non_blocking)
                .with_filter(env_filter),
        ),
    };

    let retention_handle = spawn_retention_task(cfg);

    Ok((layer, guard, retention_handle))
}

/// Maps a `RotationKind` to the interval between retention sweeps.
fn rotation_period(rotation: RotationKind) -> Duration {
    match rotation {
        RotationKind::Hourly => Duration::from_secs(3_600),
        RotationKind::Daily => Duration::from_secs(86_400),
        // One sweep per year is equivalent to "never clean up".
        RotationKind::Never => Duration::from_secs(86_400 * 365),
    }
}

/// Spawns an async task that periodically deletes old log files, keeping only
/// the `keep_files` most-recently-modified files whose names start with `prefix`.
fn spawn_retention_task(cfg: &FileSinkConfig) -> JoinHandle<()> {
    let path = cfg.path.clone();
    let prefix = cfg.prefix.clone();
    let keep_files = cfg.keep_files;
    let period = rotation_period(cfg.rotation);

    tokio::spawn(async move {
        loop {
            tokio::time::sleep(period).await;

            let Some(keep) = keep_files else {
                // keep_files = None means retain everything.
                continue;
            };

            // S-1: run synchronous fs work on a blocking thread to avoid
            // stalling the async executor.
            let path_c = path.clone();
            let prefix_c = prefix.clone();
            tokio::task::spawn_blocking(move || run_retention(&path_c, &prefix_c, keep))
                .await
                .unwrap_or_else(|e| eprintln!("[kolas/log] retention: task panicked: {e}"));
        }
    })
}

/// Synchronous retention sweep — reads the directory, sorts by mtime descending,
/// and removes every file beyond the `keep` count whose name starts with `prefix`.
fn run_retention(dir: &str, prefix: &str, keep: u32) {
    let dir_path = Path::new(dir);

    let entries = match std::fs::read_dir(dir_path) {
        Ok(e) => e,
        Err(err) => {
            eprintln!("[kolas/log] retention: failed to read dir {dir}: {err}");
            return;
        }
    };

    let mut files: Vec<(std::time::SystemTime, std::path::PathBuf)> = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path
            .file_name()
            .and_then(|n| n.to_str())
            .map(str::to_string)
        else {
            continue;
        };
        if !name.starts_with(prefix) {
            continue;
        }
        // N-5: skip files whose metadata cannot be read rather than falling back to
        // UNIX_EPOCH (which could cause the active log file to appear oldest).
        let mtime = match std::fs::metadata(&path).and_then(|m| m.modified()) {
            Ok(t) => t,
            Err(_) => continue,
        };
        files.push((mtime, path));
    }

    // Sort newest-first so we skip the ones we want to keep.
    files.sort_by(|a, b| b.0.cmp(&a.0));

    for (_, path) in files.into_iter().skip(keep as usize) {
        match std::fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                eprintln!(
                    "[kolas/log] retention: failed to remove {}: {e}",
                    path.display()
                );
            }
        }
    }
}
