//! Binary search utilities for message size management
//!
//! These functions help determine how many messages can fit within
//! a given size limit (e.g., cookie size limit).
/// Binary search to find how many items from the left can fit within max_size
///
/// Returns the number of items that can be kept from the left side of the list
/// while staying within max_size when serialized.
///
/// # Arguments
/// * `items` - The list of items to evaluate
/// * `max_size` - Maximum allowed size in bytes
/// * `serializer` - Function that serializes items to bytes
///
/// # Examples
///
/// ```
/// use reinhardt_core::messages::utils::bisect::bisect_keep_left;
///
/// let items = vec!["short".to_string(), "medium text".to_string(), "very long text here".to_string()];
/// let count = bisect_keep_left(&items, 30, |items| {
///     serde_json::to_vec(items).unwrap()
/// });
// Only the first few items fit within 30 bytes
/// assert!(count <= items.len());
/// ```
pub fn bisect_keep_left<T, F>(items: &[T], max_size: usize, serializer: F) -> usize
where
	F: Fn(&[T]) -> Vec<u8>,
{
	if items.is_empty() {
		return 0;
	}

	// Check if all items fit
	let all_serialized = serializer(items);
	if all_serialized.len() <= max_size {
		return items.len();
	}

	// Binary search for the maximum number of items that fit
	let mut left = 0;
	let mut right = items.len();

	while left < right {
		let mid = (left + right).div_ceil(2);
		let slice = &items[..mid];
		let serialized = serializer(slice);

		if serialized.len() <= max_size {
			left = mid;
		} else {
			right = mid - 1;
		}
	}

	left
}
/// Binary search to find how many items from the right can fit within max_size
///
/// Returns the number of items that can be kept from the right side of the list
/// while staying within max_size when serialized.
///
/// # Arguments
/// * `items` - The list of items to evaluate
/// * `max_size` - Maximum allowed size in bytes
/// * `serializer` - Function that serializes items to bytes
///
/// # Examples
///
/// ```
/// use reinhardt_core::messages::utils::bisect::bisect_keep_right;
///
/// let items = vec!["old".to_string(), "older".to_string(), "newest".to_string()];
/// let count = bisect_keep_right(&items, 20, |items| {
///     serde_json::to_vec(items).unwrap()
/// });
/// // Keep the most recent messages that fit
/// assert!(count <= items.len());
/// ```
pub fn bisect_keep_right<T, F>(items: &[T], max_size: usize, serializer: F) -> usize
where
	F: Fn(&[T]) -> Vec<u8>,
{
	if items.is_empty() {
		return 0;
	}

	// Check if all items fit
	let all_serialized = serializer(items);
	if all_serialized.len() <= max_size {
		return items.len();
	}

	// Binary search for the maximum number of items that fit
	let mut left = 0;
	let mut right = items.len();

	while left < right {
		let mid = (left + right).div_ceil(2);
		let slice = &items[items.len() - mid..];
		let serialized = serializer(slice);

		if serialized.len() <= max_size {
			left = mid;
		} else {
			right = mid - 1;
		}
	}

	left
}

#[cfg(test)]
mod tests {
	use super::*;

	fn string_serializer(items: &[String]) -> Vec<u8> {
		serde_json::to_vec(items).unwrap()
	}

	#[test]
	fn test_bisect_keep_left_all_fit() {
		let items = vec!["a".to_string(), "b".to_string(), "c".to_string()];
		let count = bisect_keep_left(&items, 1000, string_serializer);
		assert_eq!(count, 3);
	}

	#[test]
	fn test_bisect_keep_left_partial() {
		let items = vec![
			"message1".to_string(),
			"message2".to_string(),
			"message3".to_string(),
			"message4".to_string(),
		];
		// Small size limit - only first few should fit
		let count = bisect_keep_left(&items, 30, string_serializer);
		assert!(count < 4);
		assert!(count > 0);
	}

	#[test]
	fn test_bisect_keep_left_none_fit() {
		let items = vec!["very_long_message_that_exceeds_limit".to_string()];
		let count = bisect_keep_left(&items, 5, string_serializer);
		assert_eq!(count, 0);
	}

	#[test]
	fn test_bisect_keep_left_empty() {
		let items: Vec<String> = vec![];
		let count = bisect_keep_left(&items, 100, string_serializer);
		assert_eq!(count, 0);
	}

	#[test]
	fn test_bisect_keep_right_all_fit() {
		let items = vec!["a".to_string(), "b".to_string(), "c".to_string()];
		let count = bisect_keep_right(&items, 1000, string_serializer);
		assert_eq!(count, 3);
	}

	#[test]
	fn test_bisect_keep_right_partial() {
		let items = vec![
			"message1".to_string(),
			"message2".to_string(),
			"message3".to_string(),
			"message4".to_string(),
		];
		// Small size limit - only last few should fit
		let count = bisect_keep_right(&items, 30, string_serializer);
		assert!(count < 4);
		assert!(count > 0);
	}

	#[test]
	fn test_bisect_keep_right_none_fit() {
		let items = vec!["very_long_message_that_exceeds_limit".to_string()];
		let count = bisect_keep_right(&items, 5, string_serializer);
		assert_eq!(count, 0);
	}

	#[test]
	fn test_bisect_keep_right_empty() {
		let items: Vec<String> = vec![];
		let count = bisect_keep_right(&items, 100, string_serializer);
		assert_eq!(count, 0);
	}

	#[test]
	fn test_bisect_keep_left_vs_right() {
		let items = vec!["a".to_string(), "b".to_string(), "c".to_string()];
		let size_limit = 20;

		let left_count = bisect_keep_left(&items, size_limit, string_serializer);
		let right_count = bisect_keep_right(&items, size_limit, string_serializer);

		// Both should keep some items
		assert!(left_count > 0 || right_count > 0);
	}
}
