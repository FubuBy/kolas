use chrono::Utc;
use serde::Serialize;
use std::collections::BTreeMap;
use tracing::Subscriber;
use tracing::field::{Field, Visit};
use tracing_subscriber::layer::Context;
use tracing_subscriber::registry::LookupSpan;

/// Serializable snapshot of a tracing event. Contains no references.
/// Safe to send across thread boundaries via mpsc channels.
#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub timestamp: chrono::DateTime<Utc>,
    pub level: String,
    pub target: String,
    pub message: String,
    pub fields: serde_json::Value,
    pub span_name: Option<String>,
}

struct FieldVisitor {
    message: Option<String>,
    fields: BTreeMap<String, serde_json::Value>,
}

impl FieldVisitor {
    fn new() -> Self {
        Self {
            message: None,
            fields: BTreeMap::new(),
        }
    }
}

impl Visit for FieldVisitor {
    fn record_str(&mut self, field: &Field, value: &str) {
        if field.name() == "message" {
            self.message = Some(value.to_string());
        } else {
            self.fields.insert(
                field.name().to_string(),
                serde_json::Value::String(value.to_string()),
            );
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let s = format!("{value:?}");
        if field.name() == "message" {
            self.message = Some(s);
        } else {
            self.fields
                .insert(field.name().to_string(), serde_json::Value::String(s));
        }
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        self.fields.insert(
            field.name().to_string(),
            serde_json::Value::Number(value.into()),
        );
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        let num = serde_json::Number::from(value);
        self.fields
            .insert(field.name().to_string(), serde_json::Value::Number(num));
    }

    fn record_f64(&mut self, field: &Field, value: f64) {
        let num =
            serde_json::Number::from_f64(value).unwrap_or_else(|| serde_json::Number::from(0i64));
        self.fields
            .insert(field.name().to_string(), serde_json::Value::Number(num));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        self.fields
            .insert(field.name().to_string(), serde_json::Value::Bool(value));
    }

    fn record_error(&mut self, field: &Field, value: &(dyn std::error::Error + 'static)) {
        self.fields.insert(
            field.name().to_string(),
            serde_json::Value::String(value.to_string()),
        );
    }
}

/// Converts a tracing Event + subscriber context into an owned LogEntry.
/// Called synchronously inside Layer::on_event — must not block or await.
pub fn event_to_entry<S>(event: &tracing::Event<'_>, ctx: &Context<'_, S>) -> LogEntry
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    let meta = event.metadata();
    let level = meta.level().to_string();
    let target = meta.target().to_string();

    let mut visitor = FieldVisitor::new();
    event.record(&mut visitor);

    let message = visitor.message.unwrap_or_default();
    // N-2: build Value::Object directly from the BTreeMap instead of a
    // round-trip through serde_json::to_value.
    let fields = serde_json::Value::Object(visitor.fields.into_iter().collect());

    // Try to get current span name from context
    let span_name = ctx.lookup_current().map(|span| span.name().to_string());

    LogEntry {
        timestamp: Utc::now(),
        level,
        target,
        message,
        fields,
        span_name,
    }
}
