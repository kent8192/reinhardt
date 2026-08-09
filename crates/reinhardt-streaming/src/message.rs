use serde::{Deserialize, Serialize};

/// A streaming message wrapping a typed payload with topic and offset metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message<T> {
	/// Topic from which the message was received or to which it will be sent.
	pub topic: String,
	/// Typed message payload.
	pub payload: T,
	/// Backend offset, when provided by the streaming provider.
	pub offset: Option<i64>,
	/// Backend partition, when provided by the streaming provider.
	pub partition: Option<i32>,
}

impl<T> Message<T> {
	/// Create a message with topic and payload metadata.
	pub fn new(topic: impl Into<String>, payload: T) -> Self {
		Self {
			topic: topic.into(),
			payload,
			offset: None,
			partition: None,
		}
	}

	/// Attach a backend offset to the message.
	pub fn with_offset(mut self, offset: i64) -> Self {
		self.offset = Some(offset);
		self
	}

	/// Attach a backend partition to the message.
	pub fn with_partition(mut self, partition: i32) -> Self {
		self.partition = Some(partition);
		self
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::*;

	#[rstest]
	fn message_stores_payload() {
		let msg = Message::new("orders", 42u64);
		assert_eq!(msg.topic, "orders");
		assert_eq!(msg.payload, 42u64);
		assert_eq!(msg.offset, None);
	}

	#[rstest]
	fn message_with_offset() {
		let msg = Message::new("orders", "hello").with_offset(7);
		assert_eq!(msg.offset, Some(7));
	}

	#[rstest]
	fn message_with_partition_preserves_existing_metadata() {
		// Arrange
		let message = Message::new("orders", vec![1u8, 2, 3]).with_offset(42);

		// Act
		let message = message.with_partition(3);

		// Assert
		assert_eq!(
			(
				message.topic,
				message.payload,
				message.offset,
				message.partition
			),
			("orders".to_owned(), vec![1, 2, 3], Some(42), Some(3)),
		);
	}

	#[rstest]
	fn message_roundtrips_json() {
		let msg = Message::new("topic", vec![1u8, 2, 3]);
		let json = serde_json::to_string(&msg).unwrap();
		let decoded: Message<Vec<u8>> = serde_json::from_str(&json).unwrap();
		assert_eq!(decoded.payload, vec![1, 2, 3]);
	}
}
