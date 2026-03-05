//! Protobuf message nesting depth limits
//!
//! This module provides a configurable depth limit for protobuf message
//! parsing to prevent stack overflow attacks from deeply nested messages.
//!
//! # Security
//!
//! Without depth limits, an attacker can craft a protobuf message with
//! extreme nesting (e.g., thousands of levels) causing stack overflow
//! during deserialization. This module provides [`DepthLimitedDecoder`]
//! to enforce a configurable maximum nesting depth.
//!
//! # Example
//!
//! ```rust
//! use reinhardt_grpc::depth_limit::DepthLimitedDecoder;
//! use prost::Message;
//!
//! // Create a decoder with default depth limit (64 levels)
//! let decoder = DepthLimitedDecoder::default();
//! assert_eq!(decoder.max_depth(), 64);
//!
//! // Create a decoder with custom depth limit
//! let decoder = DepthLimitedDecoder::new(128);
//! assert_eq!(decoder.max_depth(), 128);
//! ```

/// Default maximum nesting depth for protobuf messages.
///
/// 64 levels is sufficient for legitimate use cases while preventing
/// stack overflow from maliciously crafted deeply nested messages.
const DEFAULT_MAX_NESTING_DEPTH: u32 = 64;

/// Decoder that enforces a maximum nesting depth for protobuf messages.
///
/// Wraps `prost::Message::decode` with an additional depth check
/// by scanning the wire format for nested message fields before
/// full deserialization.
///
/// # Example
///
/// ```rust
/// use reinhardt_grpc::depth_limit::DepthLimitedDecoder;
///
/// let decoder = DepthLimitedDecoder::new(32);
///
/// // Decode a simple (non-nested) message
/// let data: &[u8] = &[];
/// let result = decoder.decode::<reinhardt_grpc::proto::common::Empty>(data);
/// assert!(result.is_ok());
/// ```
#[derive(Debug, Clone)]
pub struct DepthLimitedDecoder {
	max_depth: u32,
}

impl DepthLimitedDecoder {
	/// Create a new decoder with the specified maximum nesting depth.
	pub fn new(max_depth: u32) -> Self {
		Self { max_depth }
	}

	/// Returns the configured maximum nesting depth.
	pub fn max_depth(&self) -> u32 {
		self.max_depth
	}

	/// Decode a protobuf message with depth limit enforcement.
	///
	/// First scans the wire-format bytes to measure nesting depth,
	/// then decodes the message if the depth is within limits.
	///
	/// # Errors
	///
	/// Returns [`DepthLimitError::ExceededMaxDepth`] if the message
	/// nesting exceeds the configured limit, or [`DepthLimitError::DecodeError`]
	/// if the message fails to decode.
	pub fn decode<M: prost::Message + Default>(&self, buf: &[u8]) -> Result<M, DepthLimitError> {
		let measured_depth = measure_wire_depth(buf);
		if measured_depth > self.max_depth {
			tracing::warn!(
				measured_depth = measured_depth,
				max_depth = self.max_depth,
				"Protobuf message nesting depth exceeded limit"
			);
			return Err(DepthLimitError::ExceededMaxDepth {
				depth: measured_depth,
				limit: self.max_depth,
			});
		}
		M::decode(buf).map_err(DepthLimitError::DecodeError)
	}
}

impl Default for DepthLimitedDecoder {
	fn default() -> Self {
		Self {
			max_depth: DEFAULT_MAX_NESTING_DEPTH,
		}
	}
}

/// Errors that can occur during depth-limited decoding.
#[derive(Debug, thiserror::Error)]
pub enum DepthLimitError {
	/// The message nesting depth exceeded the configured limit.
	#[error("protobuf nesting depth {depth} exceeds limit of {limit}")]
	ExceededMaxDepth {
		/// The measured nesting depth.
		depth: u32,
		/// The configured maximum depth.
		limit: u32,
	},

	/// The message failed to decode.
	#[error("protobuf decode error: {0}")]
	DecodeError(#[from] prost::DecodeError),
}

/// Measure the maximum nesting depth in a protobuf wire-format byte slice.
///
/// Scans for length-delimited fields (wire type 2) and tracks how deeply
/// they nest. Returns the maximum depth observed.
///
/// This is a best-effort scan that works on valid protobuf encoding.
/// For malformed data, it returns a conservative estimate.
fn measure_wire_depth(buf: &[u8]) -> u32 {
	if buf.is_empty() {
		return 0;
	}
	measure_depth_recursive(buf, 0)
}

/// Recursively measure depth by scanning wire-format bytes.
///
/// Parses field tags and scans length-delimited fields for sub-messages.
fn measure_depth_recursive(buf: &[u8], current_depth: u32) -> u32 {
	let mut max_depth = current_depth;
	let mut pos = 0;

	while pos < buf.len() {
		// Parse varint field tag
		let (tag, bytes_read) = match decode_varint(&buf[pos..]) {
			Some(v) => v,
			None => break,
		};
		pos += bytes_read;

		let wire_type = (tag & 0x07) as u8;

		match wire_type {
			// Varint
			0 => {
				// Skip varint value
				match decode_varint(&buf[pos..]) {
					Some((_, n)) => pos += n,
					None => break,
				}
			}
			// 64-bit
			1 => {
				pos += 8;
				if pos > buf.len() {
					break;
				}
			}
			// Length-delimited (could be a sub-message, string, or bytes)
			2 => {
				let (length, bytes_read) = match decode_varint(&buf[pos..]) {
					Some(v) => v,
					None => break,
				};
				pos += bytes_read;
				let length = length as usize;

				if pos + length > buf.len() {
					break;
				}

				// Try to measure sub-message depth
				let sub_buf = &buf[pos..pos + length];
				let sub_depth = measure_depth_recursive(sub_buf, current_depth + 1);
				if sub_depth > max_depth {
					max_depth = sub_depth;
				}

				pos += length;
			}
			// Start group (deprecated)
			3 => {
				// Skip to end group
				break;
			}
			// End group (deprecated)
			4 => break,
			// 32-bit
			5 => {
				pos += 4;
				if pos > buf.len() {
					break;
				}
			}
			// Unknown wire type
			_ => break,
		}
	}

	max_depth
}

/// Decode a protobuf varint from the buffer.
///
/// Returns the decoded value and the number of bytes consumed, or `None`
/// if the buffer is too short or the varint is malformed.
fn decode_varint(buf: &[u8]) -> Option<(u64, usize)> {
	let mut value: u64 = 0;
	let mut shift = 0u32;

	for (i, &byte) in buf.iter().enumerate() {
		if shift >= 64 {
			return None;
		}
		value |= ((byte & 0x7F) as u64) << shift;
		if byte & 0x80 == 0 {
			return Some((value, i + 1));
		}
		shift += 7;
	}
	None
}

#[cfg(test)]
mod tests {
	use super::*;
	use prost::Message;
	use rstest::rstest;

	#[rstest]
	fn default_depth_limit_is_64() {
		// Arrange & Act
		let decoder = DepthLimitedDecoder::default();

		// Assert
		assert_eq!(decoder.max_depth(), DEFAULT_MAX_NESTING_DEPTH);
		assert_eq!(decoder.max_depth(), 64);
	}

	#[rstest]
	#[case(1)]
	#[case(32)]
	#[case(64)]
	#[case(128)]
	#[case(256)]
	fn custom_depth_limit(#[case] limit: u32) {
		// Arrange & Act
		let decoder = DepthLimitedDecoder::new(limit);

		// Assert
		assert_eq!(decoder.max_depth(), limit);
	}

	#[rstest]
	fn decode_empty_message_succeeds() {
		// Arrange
		let decoder = DepthLimitedDecoder::default();
		let empty = crate::proto::common::Empty {};
		let encoded = empty.encode_to_vec();

		// Act
		let result = decoder.decode::<crate::proto::common::Empty>(&encoded);

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn decode_simple_message_succeeds() {
		// Arrange
		let decoder = DepthLimitedDecoder::default();
		let timestamp = crate::proto::common::Timestamp {
			seconds: 1_000_000,
			nanos: 500_000,
		};
		let encoded = timestamp.encode_to_vec();

		// Act
		let result = decoder.decode::<crate::proto::common::Timestamp>(&encoded);

		// Assert
		assert!(result.is_ok());
		let decoded = result.unwrap();
		assert_eq!(decoded.seconds, 1_000_000);
		assert_eq!(decoded.nanos, 500_000);
	}

	#[rstest]
	fn decode_nested_message_within_limit_succeeds() {
		// Arrange
		let decoder = DepthLimitedDecoder::new(10);
		let event = crate::proto::graphql::SubscriptionEvent {
			id: "test".to_string(),
			event_type: "update".to_string(),
			payload: Some(crate::proto::graphql::GraphQlResponse {
				data: Some("{}".to_string()),
				errors: vec![],
				extensions: None,
			}),
			timestamp: Some(crate::proto::common::Timestamp {
				seconds: 100,
				nanos: 0,
			}),
		};
		let encoded = event.encode_to_vec();

		// Act
		let result = decoder.decode::<crate::proto::graphql::SubscriptionEvent>(&encoded);

		// Assert
		assert!(result.is_ok());
	}

	#[rstest]
	fn decode_rejects_message_exceeding_depth_limit() {
		// Arrange: Create a decoder with depth limit of 1
		let decoder = DepthLimitedDecoder::new(0);

		// Nested message: a BatchResult with errors (nesting depth >= 1)
		let batch = crate::proto::common::BatchResult {
			success_count: 1,
			failure_count: 1,
			errors: vec![crate::proto::common::Error {
				code: "500".to_string(),
				message: "fail".to_string(),
				metadata: Default::default(),
			}],
		};
		let encoded = batch.encode_to_vec();

		// Act
		let result = decoder.decode::<crate::proto::common::BatchResult>(&encoded);

		// Assert
		assert!(result.is_err());
		let err = result.unwrap_err();
		assert!(
			matches!(err, DepthLimitError::ExceededMaxDepth { .. }),
			"Expected ExceededMaxDepth error, got: {err:?}"
		);
	}

	#[rstest]
	fn depth_limit_error_display_message() {
		// Arrange
		let error = DepthLimitError::ExceededMaxDepth {
			depth: 100,
			limit: 64,
		};

		// Act
		let message = error.to_string();

		// Assert
		assert_eq!(message, "protobuf nesting depth 100 exceeds limit of 64");
	}

	#[rstest]
	fn measure_empty_buffer_returns_zero() {
		// Arrange
		let buf: &[u8] = &[];

		// Act
		let depth = measure_wire_depth(buf);

		// Assert
		assert_eq!(depth, 0);
	}

	#[rstest]
	fn decoder_clone_preserves_limit() {
		// Arrange
		let decoder = DepthLimitedDecoder::new(42);

		// Act
		let cloned = decoder.clone();

		// Assert
		assert_eq!(cloned.max_depth(), 42);
	}
}
