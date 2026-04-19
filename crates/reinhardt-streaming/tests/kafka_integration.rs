use reinhardt_streaming::kafka::{KafkaConfig, KafkaConsumer, KafkaProducer};
use reinhardt_testkit::containers::KafkaContainer;
use rstest::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Order {
	id: u64,
	item: String,
}

#[fixture]
async fn kafka() -> KafkaContainer {
	KafkaContainer::new().await
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
