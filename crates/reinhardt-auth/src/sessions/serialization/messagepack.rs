//! MessagePack serialization support for sessions (requires "messagepack" feature)
//!
//! This module provides MessagePack serialization using `rmp-serde`.
//!
//! ## Example
//!
//! ```rust,no_run
//! use reinhardt_auth::sessions::serialization::{Serializer, MessagePackSerializer};
//! use serde::{Serialize, Deserialize};
//!
//! #[derive(Serialize, Deserialize, PartialEq, Debug)]
//! struct Data {
//!     value: String,
//! }
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let serializer = MessagePackSerializer;
//! let data = Data { value: "test".to_string() };
//!
//! let bytes = serializer.serialize(&data)?;
//! let restored: Data = serializer.deserialize(&bytes)?;
//!
//! assert_eq!(data, restored);
//! # Ok(())
//! # }
//! ```

use super::{SerializationError, Serializer};
use serde::{Deserialize, Serialize};

/// MessagePack serializer (requires "messagepack" feature)
///
/// # Example
///
/// ```rust,no_run
/// use reinhardt_auth::sessions::serialization::{Serializer, MessagePackSerializer};
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Serialize, Deserialize, PartialEq, Debug)]
/// struct Data {
///     value: String,
/// }
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let serializer = MessagePackSerializer;
/// let data = Data { value: "test".to_string() };
///
/// let bytes = serializer.serialize(&data)?;
/// let restored: Data = serializer.deserialize(&bytes)?;
///
/// assert_eq!(data, restored);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Copy)]
pub struct MessagePackSerializer;

impl Serializer for MessagePackSerializer {
	fn serialize<T: Serialize>(&self, data: &T) -> Result<Vec<u8>, SerializationError> {
		Ok(rmp_serde::to_vec(data)?)
	}

	fn deserialize<T: for<'de> Deserialize<'de>>(
		&self,
		bytes: &[u8],
	) -> Result<T, SerializationError> {
		Ok(rmp_serde::from_slice(bytes)?)
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
	fn test_messagepack_serializer() {
		let serializer = MessagePackSerializer;
		let data = TestData {
			id: 99,
			name: "messagepack".to_string(),
			active: false,
		};

		let bytes = serializer.serialize(&data).unwrap();
		let restored: TestData = serializer.deserialize(&bytes).unwrap();

		assert_eq!(data, restored);
	}

	#[rstest]
	#[test]
	fn test_empty_data_roundtrip() {
		// Arrange
		let serializer = MessagePackSerializer;
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
