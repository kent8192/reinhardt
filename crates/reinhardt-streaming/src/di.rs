//! DI registration helpers for the Kafka streaming backend.
//!
//! Provides utility functions to create and share `KafkaProducer` and
//! `KafkaConsumer` instances. Full `#[inject]` trait integration is
//! added in Phase 2 (macro integration).

use crate::{
	StreamingError,
	kafka::{KafkaConfig, KafkaConsumer, KafkaProducer},
};
use std::sync::Arc;

/// Create `Arc`-wrapped `KafkaProducer` and `KafkaConsumer` from `config`.
///
/// Returns an error if the brokers are unreachable at startup.
pub async fn build_kafka_clients(
	config: &KafkaConfig,
) -> Result<(Arc<KafkaProducer>, Arc<KafkaConsumer>), StreamingError> {
	let producer = Arc::new(KafkaProducer::connect(config).await?);
	let consumer = Arc::new(KafkaConsumer::connect(config).await?);
	Ok((producer, consumer))
}
