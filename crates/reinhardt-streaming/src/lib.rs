//! Backend-agnostic streaming abstraction for reinhardt.
//!
//! # Features
//!
//! - `kafka` — Kafka backend using rskafka (pure Rust, no C bindings)
//! - `task-backend` — `KafkaTaskBackend` bridging Kafka into reinhardt-tasks
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

#[cfg(feature = "kafka")]
pub mod global;

#[cfg(feature = "kafka")]
pub use global::{global_producer, set_global_producer};

pub mod router;
pub use router::{ConsumerFactory, StreamingHandlerKind, StreamingHandlerRegistration, StreamingRouter};

pub use backend::StreamingBackend;
pub use error::StreamingError;
pub use in_memory::InMemoryStreamingBackend;
pub use message::Message;
