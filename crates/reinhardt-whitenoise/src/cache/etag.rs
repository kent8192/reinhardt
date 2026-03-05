//! ETag generation utilities

use std::time::{SystemTime, UNIX_EPOCH};

/// Generates an ETag from timestamp and file size
///
/// Format: `{timestamp_hex}-{size_hex}`
///
/// # Arguments
///
/// * `modified` - File modification time
/// * `size` - File size in bytes
///
/// # Returns
///
/// ETag string in format "timestamp-size" (hex encoded)
///
/// # Example
///
/// ```rust
/// use std::time::{SystemTime, UNIX_EPOCH};
/// use reinhardt_whitenoise::cache::generate_etag;
///
/// let modified = UNIX_EPOCH + std::time::Duration::from_secs(1234567890);
/// let size = 4096;
/// let etag = generate_etag(modified, size);
/// assert_eq!(etag, "499602d2-1000");
/// ```
pub fn generate_etag(modified: SystemTime, size: u64) -> String {
	let timestamp = modified
		.duration_since(UNIX_EPOCH)
		.unwrap_or_default()
		.as_secs();

	format!("{:x}-{:x}", timestamp, size)
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use std::time::Duration;

	#[rstest]
	fn test_generate_etag() {
		let modified = UNIX_EPOCH + Duration::from_secs(1234567890);
		let size = 4096;
		let etag = generate_etag(modified, size);
		assert_eq!(etag, "499602d2-1000");
	}

	#[rstest]
	#[case(1000, 500, "3e8-1f4")]
	#[case(0, 0, "0-0")]
	#[case(1234567890, 1048576, "499602d2-100000")]
	fn test_generate_etag_various_values(
		#[case] timestamp: u64,
		#[case] size: u64,
		#[case] expected: &str,
	) {
		let modified = UNIX_EPOCH + Duration::from_secs(timestamp);
		let etag = generate_etag(modified, size);
		assert_eq!(etag, expected);
	}
}
