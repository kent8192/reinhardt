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

#[cfg(feature = "kafka")]
pub mod global;

#[cfg(feature = "kafka")]
pub use global::{global_producer, set_global_producer};

pub mod macros;
pub mod router;
pub use router::{ConsumerFactory, StreamingHandlerKind, StreamingHandlerRegistration, StreamingRouter};

/// Type-safe streaming topic resolver trait.
///
/// Mirrored from `reinhardt_urls::StreamingTopicResolver` to avoid requiring
/// reinhardt-urls as a direct dependency of reinhardt-streaming.
pub trait StreamingTopicResolver {
    fn resolve_topic(&self, name: &str) -> &'static str;
}

pub use backend::StreamingBackend;
pub use error::StreamingError;
pub use in_memory::InMemoryStreamingBackend;
pub use message::Message;

/// Metadata about a streaming handler, submitted to inventory by `#[producer]`/`#[consumer]`.
///
/// Used by `ResolvedUrls::streaming()` to resolve topic names at runtime.
#[derive(Debug)]
pub struct StreamingHandlerMetadata {
    pub name: &'static str,
    pub topic: &'static str,
    pub kind: StreamingHandlerKind,
    pub group: Option<&'static str>,
    pub module_path: &'static str,
}

inventory::collect!(StreamingHandlerMetadata);

/// Resolve the Kafka topic name for a streaming handler by its registered `name`.
///
/// Scans the inventory of `StreamingHandlerMetadata` entries submitted by
/// `#[producer]`/`#[consumer]` macros.
///
/// # Panics
///
/// Panics if no handler with `name` is registered.
pub fn resolve_streaming_topic(name: &str) -> &'static str {
    for meta in inventory::iter::<StreamingHandlerMetadata> {
        if meta.name == name {
            return meta.topic;
        }
    }
    panic!("Streaming handler `{name}` not registered. Ensure the function is annotated with `#[producer]` or `#[consumer]`.");
}
