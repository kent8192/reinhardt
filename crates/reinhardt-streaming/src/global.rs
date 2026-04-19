use crate::kafka::KafkaProducer;
use std::sync::{Arc, OnceLock};

static GLOBAL_PRODUCER: OnceLock<Arc<KafkaProducer>> = OnceLock::new();

/// Register a `KafkaProducer` as the global instance used by `#[producer]` macros.
///
/// Call once at application startup after connecting to Kafka.
/// Subsequent calls are silently ignored (first writer wins).
pub fn set_global_producer(producer: Arc<KafkaProducer>) {
    let _ = GLOBAL_PRODUCER.set(producer);
}

/// Access the globally registered producer, if any.
pub fn global_producer() -> Option<Arc<KafkaProducer>> {
    GLOBAL_PRODUCER.get().cloned()
}
