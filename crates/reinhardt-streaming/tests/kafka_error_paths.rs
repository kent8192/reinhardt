//! Error-handling and retry-path integration tests for the Kafka backend.
//!
//! Covers:
//! - Connection failure when brokers are unreachable (producer + consumer).
//! - Consumer `receive` on an empty topic returns `Ok(None)` (idle path).
//! - Consumer `receive::<T>` returns `StreamingError::Serialization` for malformed JSON
//!   produced via `send_raw`.
//! - Producer `send` against a brand-new topic succeeds via the
//!   `UnknownTopicHandling::Retry` path inside `partition_client`.
//! - Consumer offset tracking advances monotonically across consecutive receives
//!   (retry-safe sequencing for at-least-once delivery).

use std::time::Duration;

use reinhardt_streaming::{
	Message, StreamingError,
	kafka::{KafkaConfig, KafkaConsumer, KafkaProducer},
};
use reinhardt_testkit::containers::KafkaContainer;
use rstest::*;
use serde::{Deserialize, Serialize};

/// Bounded upper-limit for `connect` against an unreachable broker. Loopback
/// kernels respond with TCP RST instantly, but firewalled CI environments may
/// silently drop the SYN; cap the wait so the test fails fast and predictably.
const UNREACHABLE_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Bounded upper-limit for the initial `receive` after a `send`. Kafka metadata
/// refresh and fetch round-trips can leave the first poll empty even when the
/// record is durable; retry with a small budget instead of asserting on the
/// very first call.
const FIRST_RECEIVE_TIMEOUT: Duration = Duration::from_secs(15);
const FIRST_RECEIVE_POLL_INTERVAL: Duration = Duration::from_millis(200);

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Event {
	id: u64,
	name: String,
}

#[fixture]
async fn kafka() -> KafkaContainer {
	KafkaContainer::new().await
}

/// Use a localhost TCP port that is virtually guaranteed to refuse connections.
/// Port 1 is reserved/privileged and not bound by any normal service, so the
/// initial metadata request raises a transport error and `connect` must
/// surface `StreamingError::Connection`.
///
/// `rskafka` retries metadata refresh indefinitely by default, which on
/// firewalled CI runners (where the SYN to `127.0.0.1:1` may be dropped
/// rather than answered with RST) outlasts the outer `tokio` timeout.
/// Cap the retry budget below that timeout so the failure surfaces as
/// `StreamingError::Connection` from `connect` itself, not as an `Elapsed`
/// panic from the test harness.
fn unreachable_config() -> KafkaConfig {
	KafkaConfig::new(["127.0.0.1:1"])
		.with_client_id("reinhardt-error-test")
		.with_backoff_deadline(Duration::from_secs(3))
}

#[rstest]
#[tokio::test]
async fn producer_connect_returns_connection_error_when_broker_unreachable() {
	// Arrange
	let config = unreachable_config();

	// Act
	let result = tokio::time::timeout(UNREACHABLE_CONNECT_TIMEOUT, KafkaProducer::connect(&config))
		.await
		.expect("connect must fail fast against an unreachable broker, not hang");

	// Assert
	match result {
		Ok(_) => panic!("connect must fail when brokers refuse TCP"),
		Err(StreamingError::Connection(_)) => {}
		Err(other) => panic!("expected StreamingError::Connection, got {other:?}"),
	}
}

#[rstest]
#[tokio::test]
async fn consumer_connect_returns_connection_error_when_broker_unreachable() {
	// Arrange
	let config = unreachable_config();

	// Act
	let result = tokio::time::timeout(UNREACHABLE_CONNECT_TIMEOUT, KafkaConsumer::connect(&config))
		.await
		.expect("connect must fail fast against an unreachable broker, not hang");

	// Assert
	match result {
		Ok(_) => panic!("connect must fail when brokers refuse TCP"),
		Err(StreamingError::Connection(_)) => {}
		Err(other) => panic!("expected StreamingError::Connection, got {other:?}"),
	}
}

/// Poll `receive` until a record materializes or the budget is exhausted.
/// Tolerates the legitimate `Ok(None)` window caused by metadata refresh / fetch
/// timing on a freshly-published topic.
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
			Some(msg) => return msg,
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
async fn receive_on_empty_topic_returns_none(#[future] kafka: KafkaContainer) {
	// Arrange
	let kafka = kafka.await;
	let config = KafkaConfig::new(kafka.brokers());
	// Materialize the topic with one record so the consumer can fetch from
	// offset 0, then drain it. After drain, the next receive must yield None.
	let producer = KafkaProducer::connect(&config).await.unwrap();
	let consumer = KafkaConsumer::connect(&config).await.unwrap();
	let topic = "empty-after-drain";
	producer
		.send(
			topic,
			&Event {
				id: 1,
				name: "seed".to_owned(),
			},
		)
		.await
		.unwrap();
	// Wait through any metadata-refresh / fetch-timing window so the seed
	// record is observable before we assert on the post-drain receive.
	let _first = receive_first_with_retry::<Event>(&consumer, topic).await;

	// Act
	let second = consumer.receive::<Event>(topic).await.unwrap();

	// Assert
	assert_eq!(second.is_none(), true, "drained topic must return None");
}

#[rstest]
#[tokio::test]
async fn receive_typed_returns_serialization_error_for_malformed_payload(
	#[future] kafka: KafkaContainer,
) {
	// Arrange
	let kafka = kafka.await;
	let config = KafkaConfig::new(kafka.brokers());
	let producer = KafkaProducer::connect(&config).await.unwrap();
	let consumer = KafkaConsumer::connect(&config).await.unwrap();
	let topic = "malformed-payload";
	// Non-JSON bytes published via the raw API.
	producer
		.send_raw(topic, b"not-json".to_vec())
		.await
		.unwrap();

	// Act
	let result = consumer.receive::<Event>(topic).await;

	// Assert
	let err = result.expect_err("typed receive must fail to decode non-JSON bytes");
	assert!(
		matches!(err, StreamingError::Serialization(_)),
		"expected StreamingError::Serialization, got {err:?}",
	);
}

#[rstest]
#[tokio::test]
async fn producer_send_creates_unknown_topic_via_retry_path(#[future] kafka: KafkaContainer) {
	// Arrange
	let kafka = kafka.await;
	let config = KafkaConfig::new(kafka.brokers());
	let producer = KafkaProducer::connect(&config).await.unwrap();
	let consumer = KafkaConsumer::connect(&config).await.unwrap();
	let topic = "auto-created-by-retry";
	let event = Event {
		id: 7,
		name: "first-publish".to_owned(),
	};

	// Act
	// Producing to a topic the cluster has never seen exercises
	// `UnknownTopicHandling::Retry` inside `partition_client`; the call must
	// resolve into a successful publish once the broker auto-creates the topic.
	producer.send(topic, &event).await.unwrap();
	// Retry the first poll: a brand-new topic typically requires one or two
	// metadata refresh cycles before the consumer can fetch from offset 0.
	let message = receive_first_with_retry::<Event>(&consumer, topic).await;

	// Assert
	assert_eq!(message.payload, event);
}

#[rstest]
#[tokio::test]
async fn send_to_partition_rejects_negative_partition(#[future] kafka: KafkaContainer) {
	// Arrange
	let kafka = kafka.await;
	let config = KafkaConfig::new(kafka.brokers());
	let producer = KafkaProducer::connect(&config).await.unwrap();

	// Act
	let error = producer
		.send_to_partition("orders", -1, b"invalid-partition".to_vec())
		.await
		.expect_err("negative partitions must be rejected before publishing");

	// Assert
	match error {
		StreamingError::Fatal(message) => {
			assert_eq!(message, "partition must be non-negative, got -1");
		}
		other => panic!("expected StreamingError::Fatal, got {other:?}"),
	}
}

#[rstest]
#[tokio::test]
async fn consumer_offsets_advance_monotonically_across_receives(#[future] kafka: KafkaContainer) {
	// Arrange
	let kafka = kafka.await;
	let config = KafkaConfig::new(kafka.brokers());
	let producer = KafkaProducer::connect(&config).await.unwrap();
	let consumer = KafkaConsumer::connect(&config).await.unwrap();
	let topic = "offset-progression";
	for i in 0..3u64 {
		producer
			.send(
				topic,
				&Event {
					id: i,
					name: format!("event-{i}"),
				},
			)
			.await
			.unwrap();
	}

	// Act
	let m0 = consumer.receive::<Event>(topic).await.unwrap().unwrap();
	let m1 = consumer.receive::<Event>(topic).await.unwrap().unwrap();
	let m2 = consumer.receive::<Event>(topic).await.unwrap().unwrap();
	let m3 = consumer.receive::<Event>(topic).await.unwrap();

	// Assert
	assert_eq!(m0.payload.id, 0);
	assert_eq!(m1.payload.id, 1);
	assert_eq!(m2.payload.id, 2);
	assert_eq!(m0.offset, Some(0));
	assert_eq!(m1.offset, Some(1));
	assert_eq!(m2.offset, Some(2));
	assert_eq!(m3.is_none(), true, "no further records expected");
}
