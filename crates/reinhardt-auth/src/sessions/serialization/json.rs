//! JSON serialization support for sessions
//!
//! This module provides JSON serialization using `serde_json`.
//!
//! ## Example
//!
//! ```rust
//! use reinhardt_auth::sessions::serialization::{Serializer, JsonSerializer};
//! use serde_json::json;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let serializer = JsonSerializer;
//!
//! let data = json!({
//!     "user_id": 42,
//!     "username": "alice",
//! });
//!
//! // Serialize to bytes
//! let bytes = serializer.serialize(&data)?;
//!
//! // Deserialize back
//! let restored: serde_json::Value = serializer.deserialize(&bytes)?;
//! assert_eq!(restored["user_id"], 42);
//! # Ok(())
//! # }
//! ```

use super::{SerializationError, Serializer};
use serde::{Deserialize, Serialize};

/// JSON serializer
///
/// # Example
///
/// ```rust
/// use reinhardt_auth::sessions::serialization::{Serializer, JsonSerializer};
/// use serde_json::json;
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let serializer = JsonSerializer;
/// let data = json!({"key": "value"});
///
/// let bytes = serializer.serialize(&data)?;
/// let restored: serde_json::Value = serializer.deserialize(&bytes)?;
///
/// assert_eq!(restored["key"], "value");
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Copy)]
pub struct JsonSerializer;

impl Serializer for JsonSerializer {
	fn serialize<T: Serialize>(&self, data: &T) -> Result<Vec<u8>, SerializationError> {
		Ok(serde_json::to_vec(data)?)
	}

	fn deserialize<T: for<'de> Deserialize<'de>>(
		&self,
		bytes: &[u8],
	) -> Result<T, SerializationError> {
		Ok(serde_json::from_slice(bytes)?)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use serde::{Deserialize, Serialize};

	#[derive(Serialize, Deserialize, PartialEq, Debug)]
	struct TestData {
		id: i32,
		name: String,
		active: bool,
	}

	#[test]
	fn test_json_serializer() {
		let serializer = JsonSerializer;
		let data = TestData {
			id: 42,
			name: "test".to_string(),
			active: true,
		};

		let bytes = serializer.serialize(&data).unwrap();
		let restored: TestData = serializer.deserialize(&bytes).unwrap();

		assert_eq!(data, restored);
	}

	#[test]
	fn test_json_serializer_with_value() {
		let serializer = JsonSerializer;
		let data = serde_json::json!({
			"user_id": 123,
			"username": "alice",
			"roles": ["admin", "user"],
		});

		let bytes = serializer.serialize(&data).unwrap();
		let restored: serde_json::Value = serializer.deserialize(&bytes).unwrap();

		assert_eq!(restored["user_id"], 123);
		assert_eq!(restored["username"], "alice");
	}

	#[rstest]
	#[test]
	fn test_empty_data_roundtrip() {
		// Arrange
		let serializer = JsonSerializer;
		let data: std::collections::HashMap<String, serde_json::Value> =
			std::collections::HashMap::new();

		// Act
		let bytes = serializer.serialize(&data).unwrap();
		let restored: std::collections::HashMap<String, serde_json::Value> =
			serializer.deserialize(&bytes).unwrap();

		// Assert
		assert_eq!(restored, data);
	}
}
