use crate::error::StreamingError;
use async_trait::async_trait;

/// Abstraction over messaging backends for testing and swappability.
///
/// Use `InMemoryStreamingBackend` in unit tests. Production code uses
/// `KafkaProducer`/`KafkaConsumer` directly rather than this trait.
#[async_trait]
pub trait StreamingBackend: Send + Sync {
	/// Publish raw bytes to a topic.
	async fn publish(&self, topic: &str, payload: Vec<u8>) -> Result<(), StreamingError>;

	/// Pop the next message from a topic. Returns `None` if empty.
	async fn poll(&self, topic: &str) -> Result<Option<Vec<u8>>, StreamingError>;
}
