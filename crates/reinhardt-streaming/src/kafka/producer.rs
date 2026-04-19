use crate::{StreamingError, kafka::KafkaConfig};
use chrono::Utc;
use rskafka::{
	client::{Client, ClientBuilder, partition::UnknownTopicHandling},
	record::Record,
};
use serde::Serialize;
use std::{collections::BTreeMap, sync::Arc};

/// Kafka producer. Obtain via `KafkaProducer::connect(&config).await`.
pub struct KafkaProducer {
	client: Arc<Client>,
}

impl KafkaProducer {
	/// Connect to the Kafka brokers specified in `config`.
	pub async fn connect(config: &KafkaConfig) -> Result<Self, StreamingError> {
		let client = ClientBuilder::new(config.brokers.clone())
			.client_id(config.client_id.clone())
			.build()
			.await
			.map_err(|e| StreamingError::Connection(e.to_string()))?;
		Ok(Self {
			client: Arc::new(client),
		})
	}

	/// Serialize `value` to JSON and publish to `topic` (partition 0).
	pub async fn send<T: Serialize>(&self, topic: &str, value: &T) -> Result<(), StreamingError> {
		let payload =
			serde_json::to_vec(value).map_err(|e| StreamingError::Serialization(e.to_string()))?;
		self.send_raw(topic, payload).await
	}

	/// Publish raw bytes to `topic` (partition 0).
	pub async fn send_raw(&self, topic: &str, payload: Vec<u8>) -> Result<(), StreamingError> {
		let partition_client = self
			.client
			.partition_client(topic, 0, UnknownTopicHandling::Retry)
			.await
			.map_err(|e| StreamingError::Backend(e.to_string()))?;

		partition_client
			.produce(
				vec![Record {
					key: None,
					value: Some(payload),
					headers: BTreeMap::new(),
					timestamp: Utc::now(),
				}],
				Default::default(),
			)
			.await
			.map_err(|e| StreamingError::Backend(e.to_string()))?;

		Ok(())
	}
}

#[cfg(test)]
mod tests {
	use rstest::*;
	use serde::Serialize;

	#[derive(Serialize)]
	struct Order {
		id: u64,
		item: String,
	}

	#[rstest]
	fn order_serializes_to_json() {
		let order = Order {
			id: 1,
			item: "book".to_owned(),
		};
		let bytes = serde_json::to_vec(&order).unwrap();
		let parsed: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
		assert_eq!(parsed["id"], 1);
		assert_eq!(parsed["item"], "book");
	}
}
