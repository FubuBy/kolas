use serde::Deserialize;

fn default_level_info() -> LevelFilter {
    LevelFilter::Info
}

fn default_level_debug() -> LevelFilter {
    LevelFilter::Debug
}

fn default_format_pretty() -> FormatKind {
    FormatKind::Pretty
}

fn default_format_json() -> FormatKind {
    FormatKind::Json
}

fn default_console_target() -> ConsoleTarget {
    ConsoleTarget::Stdout
}

fn default_exclude_targets() -> Vec<String> {
    Vec::new()
}

fn default_keep_files() -> Option<u32> {
    Some(7)
}

fn default_rotation() -> RotationKind {
    RotationKind::Daily
}

fn default_table() -> String {
    "log_entries".to_string()
}

fn default_channel_capacity() -> usize {
    1024
}

fn default_batch_size() -> usize {
    100
}

fn default_flush_interval_ms() -> u64 {
    1000
}

fn default_queue_channel_capacity() -> usize {
    512
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingConfig {
    #[serde(default = "default_level_info")]
    pub default_level: LevelFilter,
    #[serde(default)]
    pub sinks: Vec<SinkConfig>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SinkConfig {
    Console(ConsoleSinkConfig),
    File(FileSinkConfig),
    Database(DatabaseSinkConfig),
    Queue(QueueSinkConfig),
}

#[derive(Debug, Clone, Deserialize)]
pub struct ConsoleSinkConfig {
    #[serde(default = "default_level_debug")]
    pub level: LevelFilter,
    #[serde(default = "default_format_pretty")]
    pub format: FormatKind,
    #[serde(default = "default_console_target")]
    pub target: ConsoleTarget,
    #[serde(default = "default_exclude_targets")]
    pub exclude_targets: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileSinkConfig {
    #[serde(default = "default_level_info")]
    pub level: LevelFilter,
    #[serde(default = "default_format_json")]
    pub format: FormatKind,
    pub path: String,
    pub prefix: String,
    #[serde(default = "default_rotation")]
    pub rotation: RotationKind,
    #[serde(default = "default_keep_files")]
    pub keep_files: Option<u32>,
    #[serde(default = "default_exclude_targets")]
    pub exclude_targets: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseSinkConfig {
    #[serde(default = "default_level_info")]
    pub level: LevelFilter,
    pub connection: String,
    #[serde(default = "default_table")]
    pub table: String,
    #[serde(default = "default_channel_capacity")]
    pub channel_capacity: usize,
    #[serde(default = "default_batch_size")]
    pub batch_size: usize,
    #[serde(default = "default_flush_interval_ms")]
    pub flush_interval_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QueueSinkConfig {
    #[serde(default = "default_level_info")]
    pub level: LevelFilter,
    pub driver: String,
    #[serde(default = "default_queue_channel_capacity")]
    pub channel_capacity: usize,
    pub connection: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LevelFilter {
    Trace,
    Debug,
    #[default]
    Info,
    Warn,
    Error,
    Off,
}

impl From<LevelFilter> for tracing_subscriber::filter::LevelFilter {
    fn from(f: LevelFilter) -> Self {
        match f {
            LevelFilter::Trace => tracing_subscriber::filter::LevelFilter::TRACE,
            LevelFilter::Debug => tracing_subscriber::filter::LevelFilter::DEBUG,
            LevelFilter::Info => tracing_subscriber::filter::LevelFilter::INFO,
            LevelFilter::Warn => tracing_subscriber::filter::LevelFilter::WARN,
            LevelFilter::Error => tracing_subscriber::filter::LevelFilter::ERROR,
            LevelFilter::Off => tracing_subscriber::filter::LevelFilter::OFF,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FormatKind {
    #[default]
    Pretty,
    Json,
    Compact,
}

#[derive(Debug, Clone, Copy, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RotationKind {
    Hourly,
    #[default]
    Daily,
    Never,
}

#[derive(Debug, Clone, Copy, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ConsoleTarget {
    #[default]
    Stdout,
    Stderr,
}
