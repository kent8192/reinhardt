//! End-to-end compile test: simulate the full user app pattern.
//!
//! Verifies that a different app label (`AppLabel` → `applabel`) generates
//! the correct struct name and methods, proving the macro handles arbitrary
//! app labels correctly.

use reinhardt_streaming::{Message, StreamingError, streaming_routes};
use reinhardt_macros::{consumer, producer, streaming_patterns};
use serde::{Deserialize, Serialize};
use core::marker::PhantomData;

// Stub __url_resolver_support for StreamingRef impl
pub mod __url_resolver_support {
    pub struct StreamingRef<'a> {
        pub _marker: core::marker::PhantomData<&'a ()>,
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Order {
    id: u64,
}

struct AppLabel;

#[producer(topic = "events", name = "emit_event")]
pub async fn emit_event(id: u64) -> Result<Order, StreamingError> {
    Ok(Order { id })
}

#[consumer(topic = "events", group = "listener", name = "on_event")]
pub async fn on_event(_msg: Message<Order>) -> Result<(), StreamingError> {
    Ok(())
}

#[streaming_patterns(AppLabel)]
pub fn streaming_routes_fn() -> reinhardt_streaming::StreamingRouter {
    streaming_routes![emit_event, on_event]
}

#[test]
fn per_app_struct_has_correct_topic_names() {
    // Arrange: construct the per-app struct directly
    let app_urls = ApplabelStreamingUrls { _marker: PhantomData };

    // Act & Assert: methods return the Kafka topic registered by #[producer]/#[consumer]
    assert_eq!(app_urls.emit_event(), "events");
    assert_eq!(app_urls.on_event(), "events");
}

#[test]
fn streaming_ref_accessor_returns_per_app_struct() {
    let streaming_ref = __url_resolver_support::StreamingRef {
        _marker: PhantomData,
    };

    let app_urls = streaming_ref.applabel();
    assert_eq!(app_urls.emit_event(), "events");
    assert_eq!(app_urls.on_event(), "events");
}

#[test]
fn streaming_resolvers_module_generated() {
    // Compile-time check: use a handler-module re-export to confirm streaming_resolvers exists.
    // If streaming_resolvers was not generated, this block would not compile.
    #[allow(unused_imports)]
    use streaming_resolvers::__streaming_resolver_meta_emit_event;
}
