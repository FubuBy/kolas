/// Abstraction over a message queue broker.
/// Each call publishes a single log record. Batching is the implementation's responsibility.
#[async_trait::async_trait]
pub trait QueueDriver: Send + Sync + 'static {
    async fn publish(
        &self,
        payload: serde_json::Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;

    fn name(&self) -> &'static str;
}

/// No-op driver: events are silently discarded.
/// Useful for tests and dev environments.
pub struct NullQueueDriver;

#[async_trait::async_trait]
impl QueueDriver for NullQueueDriver {
    async fn publish(
        &self,
        _payload: serde_json::Value,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    fn name(&self) -> &'static str {
        "null"
    }
}
