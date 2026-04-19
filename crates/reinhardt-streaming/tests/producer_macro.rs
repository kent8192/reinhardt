// Compile test: #[producer] and #[consumer] macros expand without error
use reinhardt_streaming::{Message, StreamingError};
use reinhardt_macros::{consumer, producer};
use rstest::rstest;
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

#[rstest]
fn macros_compile_and_preserve_function_signature() {
    // Arrange: provided by module-level macro definitions above

    // Act + Assert
    // If this test file compiles, both macros work correctly.
    let _ = create_order;
    let _ = handle_order;
}
