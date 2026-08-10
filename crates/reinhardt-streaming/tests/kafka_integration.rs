use std::time::Duration;

use reinhardt_streaming::{
	Message,
	di::build_kafka_clients,
	kafka::{KafkaConfig, KafkaConsumer, KafkaProducer},
};
use reinhardt_testkit::containers::KafkaContainer;
use rstest::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Order {
	id: u64,
	item: String,
}

/// Bound the initial receive while Kafka propagates topic metadata and starts fetching.
const FIRST_RECEIVE_TIMEOUT: Duration = Duration::from_secs(15);
const FIRST_RECEIVE_POLL_INTERVAL: Duration = Duration::from_millis(200);

#[fixture]
async fn kafka() -> KafkaContainer {
	KafkaContainer::new().await
}

/// Poll `receive` until a record materializes or the bounded initial-fetch window expires.
async fn receive_first_with_retry<T>(consumer: &KafkaConsumer, topic: &str) -> Message<T>
where
	T: serde::de::DeserializeOwned,
{
	let deadline = tokio::time::Instant::now() + FIRST_RECEIVE_TIMEOUT;
	loop {
		match consumer
			.receive::<T>(topic)
			.await
			.expect("first receive must not return a transport error")
		{
			Some(message) => return message,
			None => {
				assert!(
					tokio::time::Instant::now() < deadline,
					"no record arrived within {FIRST_RECEIVE_TIMEOUT:?}",
				);
				tokio::time::sleep(FIRST_RECEIVE_POLL_INTERVAL).await;
			}
		}
	}
}

#[rstest]
#[tokio::test]
async fn producer_and_consumer_roundtrip(#[future] kafka: KafkaContainer) {
	let kafka = kafka.await;
	// Arrange
	let config = KafkaConfig::new(kafka.brokers());
	let producer = KafkaProducer::connect(&config).await.unwrap();
	let consumer = KafkaConsumer::connect(&config).await.unwrap();
	let order = Order {
		id: 42,
		item: "book".to_owned(),
	};

	// Act
	producer.send("orders-test", &order).await.unwrap();
	let received = consumer.receive::<Order>("orders-test").await.unwrap();

	// Assert
	assert!(received.is_some());
	assert_eq!(received.unwrap().payload, order);
}

#[rstest]
#[tokio::test]
async fn build_kafka_clients_returns_usable_shared_clients(#[future] kafka: KafkaContainer) {
	// Arrange
	let kafka = kafka.await;
	let config = KafkaConfig::new(kafka.brokers());
	let topic = "build-kafka-clients-roundtrip";
	let order = Order {
		id: 73,
		item: "notebook".to_owned(),
	};

	// Act
	let (producer, consumer) = build_kafka_clients(&config).await.unwrap();
	producer.send(topic, &order).await.unwrap();
	let received = receive_first_with_retry::<Order>(&consumer, topic).await;

	// Assert
	assert_eq!(
		(
			received.topic,
			received.payload,
			received.offset,
			received.partition,
		),
		(topic.to_owned(), order, Some(0), None),
	);
}
