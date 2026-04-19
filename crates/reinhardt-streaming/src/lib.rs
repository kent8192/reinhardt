//! Backend-agnostic streaming abstraction for reinhardt.
//!
//! # Features
//!
//! - `kafka` — Kafka backend using rskafka (pure Rust, no C bindings)
//!
//! The Kafka-backed `TaskBackend` lives in `reinhardt-tasks` behind its
//! `kafka-backend` feature (not here).
//!
//! # Direct API
//!
//! ```rust,ignore
//! use reinhardt_streaming::kafka::{KafkaConfig, KafkaProducer};
//!
//! let config = KafkaConfig::new(vec!["localhost:9092"]);
//! let producer = KafkaProducer::connect(&config).await?;
//! producer.send("orders", &order).await?;
//! ```

pub mod backend;
pub mod error;
pub mod in_memory;
pub mod message;

#[cfg(feature = "kafka")]
pub mod kafka;

#[cfg(feature = "kafka")]
pub mod di;

pub use backend::StreamingBackend;
pub use error::StreamingError;
pub use in_memory::InMemoryStreamingBackend;
pub use message::Message;
