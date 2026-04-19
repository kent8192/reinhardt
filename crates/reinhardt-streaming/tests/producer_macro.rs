// Compile test: #[producer] and #[consumer] macros expand without error
use reinhardt_streaming::{Message, StreamingError};
use reinhardt_macros::{consumer, producer};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct Order {
    id: u64,
}

#[producer(topic = "orders", name = "create_order")]
pub async fn create_order(id: u64) -> Result<Order, StreamingError> {
    Ok(Order { id })
}

#[consumer(topic = "orders", group = "processor", name = "handle_order")]
pub async fn handle_order(_msg: Message<Order>) -> Result<(), StreamingError> {
    Ok(())
}

#[test]
fn macros_compile_and_preserve_function_signature() {
    // If this test file compiles, both macros work correctly.
    // The functions are callable with the original signatures:
    let _ = create_order; // producer wrapper exists
    let _ = handle_order; // consumer function exists
}
