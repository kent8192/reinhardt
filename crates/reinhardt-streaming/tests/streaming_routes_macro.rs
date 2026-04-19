use reinhardt_streaming::{Message, StreamingError, streaming_routes};
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
fn streaming_routes_macro_builds_router() {
    // Arrange
    // Act
    let router = streaming_routes![create_order, handle_order];

    // Assert
    let _ = router;
}
