use crate::{Message, StreamingError, kafka::KafkaConfig};
use rskafka::client::{Client, ClientBuilder, partition::UnknownTopicHandling};
use serde::de::DeserializeOwned;
use std::{
	collections::HashMap,
	sync::{Arc, Mutex},
};
use tokio::sync::Mutex as AsyncMutex;

/// Kafka consumer with in-process offset tracking (partition 0).
///
/// For production multi-partition deployments, extend with persistent offset storage
/// or a Kafka consumer group coordinator.
pub struct KafkaConsumer {
	client: Arc<Client>,
	offsets: Mutex<HashMap<String, i64>>,
	// Per-topic fetch lock. Ensures that for a given topic the sequence
	// (read offset → fetch → advance offset) is atomic, so concurrent
	// `receive_raw` calls cannot observe the same start offset and deliver
	// the same record twice.
	topic_locks: Mutex<HashMap<String, Arc<AsyncMutex<()>>>>,
}

impl KafkaConsumer {
	/// Connect to the Kafka brokers specified in `config`.
	pub async fn connect(config: &KafkaConfig) -> Result<Self, StreamingError> {
		let client = ClientBuilder::new(config.brokers.clone())
			.client_id(config.client_id.clone())
			.build()
			.await
			.map_err(|e| StreamingError::Connection(e.to_string()))?;
		Ok(Self {
			client: Arc::new(client),
			offsets: Mutex::new(HashMap::new()),
			topic_locks: Mutex::new(HashMap::new()),
		})
	}

	/// Fetch the next message from `topic`, starting from the stored offset.
	///
	/// Returns `Ok(None)` when no new messages are available.
	pub async fn receive<T: DeserializeOwned>(
		&self,
		topic: &str,
	) -> Result<Option<Message<T>>, StreamingError> {
		match self.receive_raw(topic).await? {
			None => Ok(None),
			Some((bytes, offset)) => {
				let payload = serde_json::from_slice(&bytes)
					.map_err(|e| StreamingError::Serialization(e.to_string()))?;
				Ok(Some(Message::new(topic, payload).with_offset(offset)))
			}
		}
	}

	/// Fetch the next raw bytes from `topic`. Returns `(bytes, offset)`.
	pub async fn receive_raw(&self, topic: &str) -> Result<Option<(Vec<u8>, i64)>, StreamingError> {
		let topic_lock = self.topic_lock(topic);
		let _guard = topic_lock.lock().await;

		let start_offset = self.current_offset(topic);

		let partition_client = self
			.client
			.partition_client(topic, 0, UnknownTopicHandling::Retry)
			.await
			.map_err(|e| StreamingError::Backend(e.to_string()))?;

		let (records, _high_watermark) = partition_client
			.fetch_records(start_offset, 1..1_000_000, 1_000)
			.await
			.map_err(|e| StreamingError::Backend(e.to_string()))?;

		match records.into_iter().next() {
			None => Ok(None),
			Some(record_and_offset) => {
				let offset = record_and_offset.offset;
				let bytes = record_and_offset.record.value.unwrap_or_default();
				self.advance_offset(topic, offset + 1);
				Ok(Some((bytes, offset)))
			}
		}
	}

	fn current_offset(&self, topic: &str) -> i64 {
		*self.offsets.lock().unwrap().get(topic).unwrap_or(&0)
	}

	fn advance_offset(&self, topic: &str, next: i64) {
		self.offsets.lock().unwrap().insert(topic.to_owned(), next);
	}

	fn topic_lock(&self, topic: &str) -> Arc<AsyncMutex<()>> {
		let mut locks = self.topic_locks.lock().unwrap();
		locks
			.entry(topic.to_owned())
			.or_insert_with(|| Arc::new(AsyncMutex::new(())))
			.clone()
	}
}
