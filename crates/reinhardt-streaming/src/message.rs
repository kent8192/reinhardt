use serde::{Deserialize, Serialize};

/// A streaming message wrapping a typed payload with topic and offset metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message<T> {
	pub topic: String,
	pub payload: T,
	pub offset: Option<i64>,
	pub partition: Option<i32>,
}

impl<T> Message<T> {
	pub fn new(topic: impl Into<String>, payload: T) -> Self {
		Self {
			topic: topic.into(),
			payload,
			offset: None,
			partition: None,
		}
	}

	pub fn with_offset(mut self, offset: i64) -> Self {
		self.offset = Some(offset);
		self
	}

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
	fn message_roundtrips_json() {
		let msg = Message::new("topic", vec![1u8, 2, 3]);
		let json = serde_json::to_string(&msg).unwrap();
		let decoded: Message<Vec<u8>> = serde_json::from_str(&json).unwrap();
		assert_eq!(decoded.payload, vec![1, 2, 3]);
	}
}
