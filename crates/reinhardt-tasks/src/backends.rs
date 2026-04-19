//! Task backend implementations

pub mod metadata_store;

#[cfg(feature = "redis-backend")]
pub mod redis;

#[cfg(feature = "database-backend")]
pub mod sqlite;

#[cfg(feature = "sqs-backend")]
pub mod sqs;

#[cfg(feature = "rabbitmq-backend")]
pub mod rabbitmq;

/// Kafka-backed task queue implementation.
#[cfg(feature = "kafka-backend")]
pub mod kafka;

pub use metadata_store::{InMemoryMetadataStore, MetadataStore, MetadataStoreError, TaskMetadata};

#[cfg(feature = "redis-backend")]
pub use redis::RedisTaskBackend;

#[cfg(feature = "database-backend")]
pub use sqlite::SqliteBackend;

#[cfg(feature = "sqs-backend")]
pub use sqs::{SqsBackend, SqsConfig};

#[cfg(feature = "rabbitmq-backend")]
pub use rabbitmq::{RabbitMQBackend, RabbitMQConfig};

#[cfg(feature = "kafka-backend")]
pub use kafka::KafkaTaskBackend;
