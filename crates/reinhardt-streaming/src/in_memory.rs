use crate::{StreamingBackend, StreamingError};
use async_trait::async_trait;
use std::{
	collections::{HashMap, VecDeque},
	sync::Mutex,
};

/// In-memory streaming backend for unit tests (no Kafka required).
pub struct InMemoryStreamingBackend {
	queues: Mutex<HashMap<String, VecDeque<Vec<u8>>>>,
}

impl InMemoryStreamingBackend {
	pub fn new() -> Self {
		Self {
			queues: Mutex::new(HashMap::new()),
		}
	}
}

impl Default for InMemoryStreamingBackend {
	fn default() -> Self {
		Self::new()
	}
}

#[async_trait]
impl StreamingBackend for InMemoryStreamingBackend {
	async fn publish(&self, topic: &str, payload: Vec<u8>) -> Result<(), StreamingError> {
		self.queues
			.lock()
			.map_err(|e| StreamingError::Backend(e.to_string()))?
			.entry(topic.to_owned())
			.or_default()
			.push_back(payload);
		Ok(())
	}

	async fn poll(&self, topic: &str) -> Result<Option<Vec<u8>>, StreamingError> {
		Ok(self
			.queues
			.lock()
			.map_err(|e| StreamingError::Backend(e.to_string()))?
			.entry(topic.to_owned())
			.or_default()
			.pop_front())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[fixture]
	fn backend() -> InMemoryStreamingBackend {
		InMemoryStreamingBackend::new()
	}

	#[rstest]
	#[tokio::test]
	async fn publish_then_poll_returns_message(backend: InMemoryStreamingBackend) {
		// Arrange
		let payload = b"hello".to_vec();

		// Act
		backend
			.publish("test-topic", payload.clone())
			.await
			.unwrap();
		let result = backend.poll("test-topic").await.unwrap();

		// Assert
		assert_eq!(result, Some(payload));
	}

	#[rstest]
	#[tokio::test]
	async fn poll_empty_topic_returns_none(backend: InMemoryStreamingBackend) {
		let result = backend.poll("empty").await.unwrap();
		assert_eq!(result, None);
	}

	#[rstest]
	#[tokio::test]
	async fn fifo_order_preserved(backend: InMemoryStreamingBackend) {
		// Arrange
		backend.publish("t", b"first".to_vec()).await.unwrap();
		backend.publish("t", b"second".to_vec()).await.unwrap();

		// Act & Assert
		assert_eq!(backend.poll("t").await.unwrap(), Some(b"first".to_vec()));
		assert_eq!(backend.poll("t").await.unwrap(), Some(b"second".to_vec()));
		assert_eq!(backend.poll("t").await.unwrap(), None);
	}

	#[rstest]
	#[tokio::test]
	async fn independent_topics_do_not_interfere(backend: InMemoryStreamingBackend) {
		backend.publish("a", b"msg-a".to_vec()).await.unwrap();
		backend.publish("b", b"msg-b".to_vec()).await.unwrap();

		assert_eq!(backend.poll("a").await.unwrap(), Some(b"msg-a".to_vec()));
		assert_eq!(backend.poll("b").await.unwrap(), Some(b"msg-b".to_vec()));
	}
}
