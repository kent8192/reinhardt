//! Streaming module.
//!
//! Provides backend-agnostic streaming with Kafka as the primary implementation.
//!
//! # Examples
//!
//! ```rust,no_run
//! use reinhardt::streaming::kafka::{KafkaConfig, KafkaProducer};
//! ```

#[cfg(feature = "streaming")]
pub use reinhardt_streaming::*;
