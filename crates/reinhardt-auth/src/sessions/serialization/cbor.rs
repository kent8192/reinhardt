//! CBOR serialization support for sessions (requires "cbor" feature)
//!
//! This module provides CBOR (Concise Binary Object Representation) serialization using `ciborium`.
//! CBOR is an RFC 7049 compliant format designed for cross-platform data interchange.
//!
//! ## Example
//!
//! ```rust,no_run
//! use reinhardt_auth::sessions::serialization::{Serializer, CborSerializer};
//! use serde::{Serialize, Deserialize};
//!
//! #[derive(Serialize, Deserialize, PartialEq, Debug)]
//! struct UserSession {
//!     user_id: u64,
//!     username: String,
//!     authenticated: bool,
//! }
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let serializer = CborSerializer;
//! let session = UserSession {
//!     user_id: 42,
//!     username: "alice".to_string(),
//!     authenticated: true,
//! };
//!
//! let bytes = serializer.serialize(&session)?;
//! let restored: UserSession = serializer.deserialize(&bytes)?;
//!
//! assert_eq!(session, restored);
//! # Ok(())
//! # }
//! ```

use super::{SerializationError, Serializer};
use serde::{Deserialize, Serialize};

/// CBOR serializer (requires "cbor" feature)
///
/// CBOR (Concise Binary Object Representation) is an RFC 7049 compliant format
/// designed for cross-platform data interchange. It provides:
/// - Smaller payload size compared to JSON
/// - Support for binary data
/// - Cross-platform compatibility
///
/// # Example
///
/// ```rust,no_run
/// use reinhardt_auth::sessions::serialization::{Serializer, CborSerializer};
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Serialize, Deserialize, PartialEq, Debug)]
/// struct Data {
///     value: String,
/// }
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let serializer = CborSerializer;
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
pub struct CborSerializer;

impl Serializer for CborSerializer {
	fn serialize<T: Serialize>(&self, data: &T) -> Result<Vec<u8>, SerializationError> {
		let mut buffer = Vec::new();
		ciborium::ser::into_writer(data, &mut buffer)
			.map_err(|e| SerializationError::SerializationFailed(e.to_string()))?;
		Ok(buffer)
	}

	fn deserialize<T: for<'de> Deserialize<'de>>(
		&self,
		bytes: &[u8],
	) -> Result<T, SerializationError> {
		ciborium::de::from_reader(bytes)
			.map_err(|e| SerializationError::DeserializationFailed(e.to_string()))
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
	fn test_cbor_serializer() {
		let serializer = CborSerializer;
		let data = TestData {
			id: 42,
			name: "cbor_test".to_string(),
			active: true,
		};

		let bytes = serializer.serialize(&data).unwrap();
		let restored: TestData = serializer.deserialize(&bytes).unwrap();

		assert_eq!(data, restored);
	}

	#[test]
	fn test_cbor_serializer_with_complex_types() {
		#[derive(Serialize, Deserialize, PartialEq, Debug)]
		struct Complex {
			numbers: Vec<i32>,
			nested: Nested,
		}

		#[derive(Serialize, Deserialize, PartialEq, Debug)]
		struct Nested {
			key: String,
			value: i32,
		}

		let serializer = CborSerializer;
		let data = Complex {
			numbers: vec![1, 2, 3, 4, 5],
			nested: Nested {
				key: "test".to_string(),
				value: 999,
			},
		};

		let bytes = serializer.serialize(&data).unwrap();
		let restored: Complex = serializer.deserialize(&bytes).unwrap();

		assert_eq!(data, restored);
	}

	#[test]
	fn test_cbor_serializer_with_option_types() {
		#[derive(Serialize, Deserialize, PartialEq, Debug)]
		struct WithOption {
			required: String,
			optional: Option<i32>,
		}

		let serializer = CborSerializer;

		// Test with Some
		let data_some = WithOption {
			required: "test".to_string(),
			optional: Some(42),
		};
		let bytes_some = serializer.serialize(&data_some).unwrap();
		let restored_some: WithOption = serializer.deserialize(&bytes_some).unwrap();
		assert_eq!(data_some, restored_some);

		// Test with None
		let data_none = WithOption {
			required: "test".to_string(),
			optional: None,
		};
		let bytes_none = serializer.serialize(&data_none).unwrap();
		let restored_none: WithOption = serializer.deserialize(&bytes_none).unwrap();
		assert_eq!(data_none, restored_none);
	}

	#[rstest]
	#[test]
	fn test_empty_data_roundtrip() {
		// Arrange
		let serializer = CborSerializer;
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
