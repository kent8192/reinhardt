//! Kafka backend for reinhardt-streaming.
//!
//! Requires feature `kafka`.

pub mod config;
pub mod consumer;
pub mod producer;

pub use config::KafkaConfig;
pub use consumer::KafkaConsumer;
pub use producer::KafkaProducer;
