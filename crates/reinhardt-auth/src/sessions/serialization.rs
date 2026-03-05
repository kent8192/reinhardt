//! Multiple serialization format support for sessions
//!
//! This module provides support for different serialization formats
//! including JSON, MessagePack, CBOR, and Bincode.
//!
//! ## Available Serializers
//!
//! - **JSON** (always available): Human-readable, widely compatible
//! - **MessagePack** (feature: `messagepack`): Compact binary format, cross-platform
//! - **CBOR** (feature: `cbor`): RFC 7049 compliant, cross-platform
//! - **Bincode** (feature: `bincode`): Fastest for Rust-to-Rust communication
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

use serde::{Deserialize, Serialize};
use thiserror::Error;

// Submodules
mod json;
pub use json::JsonSerializer;

#[cfg(feature = "messagepack")]
mod messagepack;
#[cfg(feature = "messagepack")]
pub use messagepack::MessagePackSerializer;

#[cfg(feature = "cbor")]
mod cbor;
#[cfg(feature = "cbor")]
pub use cbor::CborSerializer;

#[cfg(feature = "bincode")]
mod bincode;
#[cfg(feature = "bincode")]
pub use self::bincode::BincodeSerializer;

/// Serialization errors
#[non_exhaustive]
#[derive(Debug, Error)]
pub enum SerializationError {
	/// JSON serialization error
	#[error("JSON error: {0}")]
	JsonError(#[from] serde_json::Error),

	/// MessagePack serialization error
	#[cfg(feature = "messagepack")]
	#[error("MessagePack error: {0}")]
	MessagePackError(#[from] rmp_serde::encode::Error),

	/// MessagePack deserialization error
	#[cfg(feature = "messagepack")]
	#[error("MessagePack decode error: {0}")]
	MessagePackDecodeError(#[from] rmp_serde::decode::Error),

	/// Generic serialization failure
	#[error("Serialization failed: {0}")]
	SerializationFailed(String),

	/// Generic deserialization failure
	#[error("Deserialization failed: {0}")]
	DeserializationFailed(String),

	/// Unsupported format
	#[error("Unsupported serialization format")]
	UnsupportedFormat,
}

/// Serializer trait for different formats
///
/// # Example
///
/// ```rust
/// use reinhardt_auth::sessions::serialization::{Serializer, JsonSerializer};
/// use serde::{Serialize, Deserialize};
///
/// #[derive(Serialize, Deserialize, PartialEq, Debug)]
/// struct UserData {
///     id: i32,
///     name: String,
/// }
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let serializer = JsonSerializer;
/// let user = UserData { id: 1, name: "Alice".to_string() };
///
/// let bytes = serializer.serialize(&user)?;
/// let restored: UserData = serializer.deserialize(&bytes)?;
///
/// assert_eq!(user, restored);
/// # Ok(())
/// # }
/// ```
pub trait Serializer: Send + Sync {
	/// Serialize data to bytes
	fn serialize<T: Serialize>(&self, data: &T) -> Result<Vec<u8>, SerializationError>;

	/// Deserialize bytes to data
	fn deserialize<T: for<'de> Deserialize<'de>>(
		&self,
		bytes: &[u8],
	) -> Result<T, SerializationError>;
}

/// Serialization format enum
///
/// # Example
///
/// ```rust
/// use reinhardt_auth::sessions::serialization::SerializationFormat;
///
/// let format = SerializationFormat::Json;
/// assert_eq!(format.name(), "json");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SerializationFormat {
	/// JSON format (always available)
	Json,
	/// MessagePack format (requires "messagepack" feature)
	#[cfg(feature = "messagepack")]
	MessagePack,
	/// CBOR format (requires "cbor" feature)
	#[cfg(feature = "cbor")]
	Cbor,
	/// Bincode format (requires "bincode" feature)
	#[cfg(feature = "bincode")]
	Bincode,
}

impl SerializationFormat {
	/// Get format name as string
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::serialization::SerializationFormat;
	///
	/// assert_eq!(SerializationFormat::Json.name(), "json");
	/// ```
	pub fn name(&self) -> &'static str {
		match self {
			SerializationFormat::Json => "json",
			#[cfg(feature = "messagepack")]
			SerializationFormat::MessagePack => "messagepack",
			#[cfg(feature = "cbor")]
			SerializationFormat::Cbor => "cbor",
			#[cfg(feature = "bincode")]
			SerializationFormat::Bincode => "bincode",
		}
	}

	/// Serialize data using this format
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::serialization::SerializationFormat;
	/// use serde_json::json;
	///
	/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
	/// let format = SerializationFormat::Json;
	///
	/// let data = json!({"test": true});
	/// let bytes = format.serialize(&data)?;
	/// let restored: serde_json::Value = format.deserialize(&bytes)?;
	///
	/// assert_eq!(restored["test"], true);
	/// # Ok(())
	/// # }
	/// ```
	pub fn serialize<T: Serialize>(&self, data: &T) -> Result<Vec<u8>, SerializationError> {
		match self {
			SerializationFormat::Json => JsonSerializer.serialize(data),
			#[cfg(feature = "messagepack")]
			SerializationFormat::MessagePack => MessagePackSerializer.serialize(data),
			#[cfg(feature = "cbor")]
			SerializationFormat::Cbor => CborSerializer.serialize(data),
			#[cfg(feature = "bincode")]
			SerializationFormat::Bincode => BincodeSerializer.serialize(data),
		}
	}

	/// Deserialize data using this format
	pub fn deserialize<T: for<'de> Deserialize<'de>>(
		&self,
		bytes: &[u8],
	) -> Result<T, SerializationError> {
		match self {
			SerializationFormat::Json => JsonSerializer.deserialize(bytes),
			#[cfg(feature = "messagepack")]
			SerializationFormat::MessagePack => MessagePackSerializer.deserialize(bytes),
			#[cfg(feature = "cbor")]
			SerializationFormat::Cbor => CborSerializer.deserialize(bytes),
			#[cfg(feature = "bincode")]
			SerializationFormat::Bincode => BincodeSerializer.deserialize(bytes),
		}
	}
}

impl Default for SerializationFormat {
	/// Default serialization format is JSON
	///
	/// # Example
	///
	/// ```rust
	/// use reinhardt_auth::sessions::serialization::SerializationFormat;
	///
	/// let format = SerializationFormat::default();
	/// assert_eq!(format, SerializationFormat::Json);
	/// ```
	fn default() -> Self {
		Self::Json
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_serialization_format_name() {
		assert_eq!(SerializationFormat::Json.name(), "json");

		#[cfg(feature = "messagepack")]
		assert_eq!(SerializationFormat::MessagePack.name(), "messagepack");

		#[cfg(feature = "cbor")]
		assert_eq!(SerializationFormat::Cbor.name(), "cbor");

		#[cfg(feature = "bincode")]
		assert_eq!(SerializationFormat::Bincode.name(), "bincode");
	}

	#[test]
	fn test_serialization_format_default() {
		let format = SerializationFormat::default();
		assert_eq!(format, SerializationFormat::Json);
	}

	#[test]
	fn test_serialization_format_serialize_deserialize() {
		let format = SerializationFormat::Json;

		let data = serde_json::json!({"test": "value"});
		let bytes = format.serialize(&data).unwrap();
		let restored: serde_json::Value = format.deserialize(&bytes).unwrap();

		assert_eq!(restored["test"], "value");
	}
}
