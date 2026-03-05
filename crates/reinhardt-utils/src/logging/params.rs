//! Parameter representation utilities for logging
//! Based on SQLAlchemy's _repr_params functionality

use serde_json::Value;

/// Configuration for parameter representation
pub struct ReprParamsConfig {
	pub max_chars: usize,
	pub batches: usize,
	pub is_multi: bool,
}

impl Default for ReprParamsConfig {
	fn default() -> Self {
		Self {
			max_chars: 300,
			batches: 10,
			is_multi: false,
		}
	}
}
/// Represent parameters in a truncated form suitable for logging
///
/// # Examples
///
/// ```
/// use reinhardt_utils::logging::params::{repr_params, ReprParamsConfig};
/// use serde_json::json;
///
/// // Simple parameter representation
/// let params = json!({"user_id": 42, "email": "test@example.com"});
/// let config = ReprParamsConfig::default();
/// let output = repr_params(&params, &config);
///
/// // Output includes the parameter values
/// assert!(output.contains("user_id"));
/// assert!(output.contains("42"));
/// ```
pub fn repr_params(params: &Value, config: &ReprParamsConfig) -> String {
	match params {
		Value::Array(arr) if config.is_multi && !arr.is_empty() => repr_multi_params(arr, config),
		Value::Array(arr) => repr_array(arr, config),
		Value::Object(obj) => repr_object(obj, config),
		_ => format!("{:?}", params),
	}
}

fn repr_multi_params(params: &[Value], config: &ReprParamsConfig) -> String {
	let total = params.len();

	if total <= config.batches {
		return format!("{:?}", params);
	}

	let show_count = config.batches / 2;
	let first_items: Vec<String> = params
		.iter()
		.take(show_count)
		.map(|v| format!("{:?}", v))
		.collect();
	let last_items: Vec<String> = params
		.iter()
		.rev()
		.take(show_count)
		.map(|v| format!("{:?}", v))
		.collect::<Vec<_>>()
		.into_iter()
		.rev()
		.collect();

	format!(
		"[{}  ... displaying {} of {} total bound parameter sets ...  {}]",
		first_items.join(", "),
		config.batches,
		total,
		last_items.join(", ")
	)
}

// Fixes #802: Find byte index at the n-th character boundary for safe UTF-8 slicing.
// Returns `s.len()` if `n` >= number of characters in `s`.
fn byte_index_at_char(s: &str, n: usize) -> usize {
	s.char_indices().nth(n).map(|(i, _)| i).unwrap_or(s.len())
}

fn repr_array(arr: &[Value], config: &ReprParamsConfig) -> String {
	let repr = format!("{:?}", arr);
	let char_count = repr.chars().count();

	if char_count <= config.max_chars {
		return repr;
	}

	let half_chars = config.max_chars / 2;
	let truncated_chars = char_count - config.max_chars;

	// Fixes #802: Use char_indices() for character-aware slicing
	let start_end = byte_index_at_char(&repr, half_chars);
	let tail_start = byte_index_at_char(&repr, char_count - half_chars);

	format!(
		"{}  ... ({} characters truncated) ...  {}",
		&repr[..start_end],
		truncated_chars,
		&repr[tail_start..]
	)
}

fn repr_object(obj: &serde_json::Map<String, Value>, config: &ReprParamsConfig) -> String {
	let repr = format!("{:?}", obj);
	let char_count = repr.chars().count();

	if char_count <= config.max_chars {
		return repr;
	}

	// For huge dicts, truncate from middle
	if obj.len() > 100 {
		let half_chars = config.max_chars / 2;
		let truncated_params = obj.len() - 10; // Show ~10 params

		// Fixes #802: Use char_indices() for character-aware slicing
		let start_end = byte_index_at_char(&repr, half_chars);
		let tail_start = byte_index_at_char(&repr, char_count - half_chars);

		format!(
			"{} ... {} parameters truncated ... {}",
			&repr[..start_end],
			truncated_params,
			&repr[tail_start..]
		)
	} else {
		let half_chars = config.max_chars / 2;
		let truncated_chars = char_count - config.max_chars;

		// Fixes #802: Use char_indices() for character-aware slicing
		let start_end = byte_index_at_char(&repr, half_chars);
		let tail_start = byte_index_at_char(&repr, char_count - half_chars);

		format!(
			"{}  ... ({} characters truncated) ...  {}",
			&repr[..start_end],
			truncated_chars,
			&repr[tail_start..]
		)
	}
}
/// Truncate a single parameter value if it's too long
///
/// # Examples
///
/// ```
/// use reinhardt_utils::logging::params::truncate_param;
///
/// // Short strings are returned unchanged
/// let short_text = "Hello";
/// assert_eq!(truncate_param(short_text, 100), "Hello");
///
/// // Long strings are truncated with indication
/// let long_text = "a".repeat(1000);
/// let truncated = truncate_param(&long_text, 50);
/// assert!(truncated.len() < 1000);
/// assert!(truncated.contains("characters truncated"));
/// ```
pub fn truncate_param(value: &str, max_chars: usize) -> String {
	let char_count = value.chars().count();

	if char_count <= max_chars {
		return value.to_string();
	}

	let half_chars = max_chars / 2;
	let truncated_chars = char_count - max_chars;

	// Fixes #802: Use char_indices() for character-aware slicing
	let start_end = byte_index_at_char(value, half_chars);
	let tail_start = byte_index_at_char(value, char_count - half_chars);

	format!(
		"{} ... ({} characters truncated) ... {}",
		&value[..start_end],
		truncated_chars,
		&value[tail_start..]
	)
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use serde_json::json;

	#[rstest]
	fn test_repr_params_large_list_of_dict() {
		let params: Vec<Value> = (0..100).map(|i| json!({"data": i.to_string()})).collect();

		let config = ReprParamsConfig {
			max_chars: 300,
			batches: 10,
			is_multi: true,
		};

		let result = repr_params(&Value::Array(params), &config);
		eprintln!("LARGE_LIST OUTPUT: {}", result);

		// Should show first few and last few with exact message format
		assert!(
			result.contains("displaying 10 of 100 total bound parameter sets"),
			"Expected truncation message not found in: {}",
			result
		);
		// First element should contain "data": "0"
		assert!(
			result.contains(r#""data""#) && result.contains(r#""0""#),
			"Expected first element with data: 0 not found in: {}",
			result
		);
		// Last element should contain "data": "99"
		assert!(
			result.contains(r#""99""#),
			"Expected last element with data: 99 not found in: {}",
			result
		);
		// Should start with "[" and end with "]"
		assert!(result.starts_with('[') && result.ends_with(']'));
	}

	#[rstest]
	fn test_repr_params_positional_array() {
		let params = json!([[1, 2, 3], 5]);

		let config = ReprParamsConfig {
			max_chars: 300,
			batches: 10,
			is_multi: false,
		};

		let result = repr_params(&params, &config);
		// Verify exact debug format output
		assert_eq!(
			result,
			"[Array [Number(1), Number(2), Number(3)], Number(5)]"
		);
	}

	#[rstest]
	fn test_repr_params_unknown_list() {
		let large_array: Vec<i32> = (0..300).collect();
		let params = json!([large_array, 5]);

		let config = ReprParamsConfig {
			max_chars: 80,
			batches: 10,
			is_multi: false,
		};

		let result = repr_params(&params, &config);
		// Should be truncated with exact message format
		assert!(
			result.contains("characters truncated"),
			"Expected truncation message not found in: {}",
			result
		);
		// Should have ellipsis pattern: "...  ... (N characters truncated) ...  ..."
		assert!(
			result.matches("...").count() >= 2,
			"Expected ellipsis pattern not found in: {}",
			result
		);
		// Should start with "[" (array format)
		assert!(
			result.starts_with('['),
			"Expected array start not found in: {}",
			result
		);
	}

	#[rstest]
	fn test_repr_params_named_dict() {
		let mut params = serde_json::Map::new();
		for i in 0..10 {
			params.insert(format!("key_{}", i), json!(i));
		}

		let config = ReprParamsConfig {
			max_chars: 300,
			batches: 10,
			is_multi: false,
		};

		let result = repr_params(&Value::Object(params.clone()), &config);
		// Should fit without truncation
		assert!(
			!result.contains("truncated"),
			"Unexpected truncation in: {}",
			result
		);
		// Should contain all keys
		for i in 0..10 {
			assert!(
				result.contains(&format!(r#""key_{}""#, i)),
				"Expected key_{}  not found in: {}",
				i,
				result
			);
		}
		// Should start with "{" (object format from Debug)
		assert!(
			result.starts_with('{'),
			"Expected object start not found in: {}",
			result
		);
	}

	#[rstest]
	fn test_repr_params_huge_named_dict() {
		let mut params = serde_json::Map::new();
		for i in 0..800 {
			params.insert(format!("key_{}", i), json!(i));
		}

		let config = ReprParamsConfig {
			max_chars: 1400,
			batches: 10,
			is_multi: false,
		};

		let result = repr_params(&Value::Object(params), &config);
		// Should be truncated with exact message format for huge dicts
		assert!(
			result.contains("parameters truncated"),
			"Expected parameters truncation message not found in: {}",
			result
		);
		// Should have ellipsis pattern: "... N parameters truncated ..."
		assert!(
			result.matches("...").count() >= 2,
			"Expected ellipsis pattern not found in: {}",
			result
		);
		// Should start with "{" (object format)
		assert!(
			result.starts_with('{'),
			"Expected object start not found in: {}",
			result
		);
	}

	#[rstest]
	fn test_repr_params_ismulti_named_dict() {
		let param: serde_json::Map<String, Value> =
			(0..10).map(|i| (format!("key_{}", i), json!(i))).collect();

		let params: Vec<Value> = (0..50).map(|_| Value::Object(param.clone())).collect();

		let config = ReprParamsConfig {
			max_chars: 80,
			batches: 5,
			is_multi: true,
		};

		let result = repr_params(&Value::Array(params), &config);
		// Should show truncation with exact message format
		assert!(
			result.contains("displaying 5 of 50 total bound parameter sets"),
			"Expected multi-batch truncation message not found in: {}",
			result
		);
		// Should start with "[" and end with "]"
		assert!(
			result.starts_with('[') && result.ends_with(']'),
			"Expected array format not found in: {}",
			result
		);
	}

	#[rstest]
	fn test_truncate_param() {
		let large_param = "a".repeat(5000);
		let result = truncate_param(&large_param, 298);

		// Should be truncated to approximately max_chars + overhead
		assert!(
			result.len() < 5000,
			"Expected truncated length, got: {}",
			result.len()
		);
		assert!(
			result.len() > 298,
			"Result should be longer than max_chars due to truncation message"
		);
		// Should contain exact truncation message format
		assert!(
			result.contains("characters truncated"),
			"Expected truncation message not found in: {}",
			result
		);
		// Should start and end with repeated 'a'
		assert!(
			result.starts_with("aaaa"),
			"Expected start pattern not found in: {}",
			result
		);
		assert!(
			result.ends_with("aaaa"),
			"Expected end pattern not found in: {}",
			result
		);
		// Should have ellipsis pattern: "aaa ... (N characters truncated) ... aaa"
		assert!(
			result.matches("...").count() == 2,
			"Expected exactly 2 ellipsis markers, found: {}",
			result.matches("...").count()
		);
	}

	#[rstest]
	fn test_truncate_param_small() {
		let small_param = "small";
		let result = truncate_param(small_param, 100);

		assert_eq!(result, "small");
	}

	#[rstest]
	fn test_repr_array_with_multibyte_utf8_does_not_panic() {
		// Arrange
		let params = json!(["こんにちは世界", "テスト文字列", "日本語データ"]);
		let arr = params.as_array().unwrap();
		let config = ReprParamsConfig {
			max_chars: 20,
			batches: 10,
			is_multi: false,
		};

		// Act
		let result = repr_array(arr, &config);

		// Assert
		assert!(result.contains("characters truncated"));
	}

	#[rstest]
	fn test_repr_object_with_multibyte_utf8_does_not_panic() {
		// Arrange
		let mut obj = serde_json::Map::new();
		for i in 0..10 {
			obj.insert(format!("キー_{}", i), json!(format!("値_{}", i)));
		}
		let config = ReprParamsConfig {
			max_chars: 20,
			batches: 10,
			is_multi: false,
		};

		// Act
		let result = repr_object(&obj, &config);

		// Assert
		assert!(result.contains("characters truncated"));
	}

	#[rstest]
	fn test_repr_object_huge_with_multibyte_utf8_does_not_panic() {
		// Arrange
		let mut obj = serde_json::Map::new();
		for i in 0..200 {
			obj.insert(format!("キー_{}", i), json!(format!("値_{}", i)));
		}
		let config = ReprParamsConfig {
			max_chars: 50,
			batches: 10,
			is_multi: false,
		};

		// Act
		let result = repr_object(&obj, &config);

		// Assert
		assert!(result.contains("parameters truncated"));
	}

	#[rstest]
	fn test_truncate_param_with_multibyte_utf8_does_not_panic() {
		// Arrange
		let multibyte_param = "あ".repeat(500);

		// Act
		let result = truncate_param(&multibyte_param, 50);

		// Assert
		assert!(result.contains("characters truncated"));
		assert!(result.starts_with("あ"));
		assert!(result.ends_with("あ"));
	}

	#[rstest]
	fn test_truncate_param_with_mixed_ascii_and_multibyte() {
		// Arrange
		let mixed = format!("{}abc{}", "日本語".repeat(50), "中文字".repeat(50));

		// Act
		let result = truncate_param(&mixed, 30);

		// Assert
		assert!(result.contains("characters truncated"));
	}

	#[rstest]
	fn test_byte_index_at_char_with_ascii() {
		// Arrange
		let s = "hello";

		// Act & Assert
		assert_eq!(byte_index_at_char(s, 0), 0);
		assert_eq!(byte_index_at_char(s, 3), 3);
		assert_eq!(byte_index_at_char(s, 5), 5);
		assert_eq!(byte_index_at_char(s, 10), 5); // beyond end
	}

	#[rstest]
	fn test_byte_index_at_char_with_multibyte() {
		// Arrange
		let s = "あいう"; // Each char is 3 bytes

		// Act & Assert
		assert_eq!(byte_index_at_char(s, 0), 0);
		assert_eq!(byte_index_at_char(s, 1), 3);
		assert_eq!(byte_index_at_char(s, 2), 6);
		assert_eq!(byte_index_at_char(s, 3), 9);
		assert_eq!(byte_index_at_char(s, 10), 9); // beyond end
	}

	// Regression test for #762: truncate_param must produce valid UTF-8 output when
	// truncating strings containing multibyte characters. Previously, byte-level slicing
	// could split in the middle of a multibyte sequence, causing a panic or corrupted output.
	#[rstest]
	#[case("あいうえお".repeat(30), 20, "あ", "お")] // 3-byte CJK chars (hiragana)
	#[case("日本語テスト".repeat(30), 20, "日", "ト")] // 3-byte CJK chars (kanji + katakana)
	#[case("中文测试".repeat(30), 20, "中", "试")] // 3-byte CJK chars (Chinese)
	#[case("αβγδεζηθ".repeat(30), 20, "α", "θ")] // 2-byte Greek letters
	fn test_truncate_param_multibyte_produces_valid_utf8_regression(
		#[case] input: String,
		#[case] max_chars: usize,
		#[case] expected_start: &str,
		#[case] expected_end: &str,
	) {
		// Arrange - input is longer than max_chars and contains only multibyte characters

		// Act
		let result = truncate_param(&input, max_chars);

		// Assert - output must be valid UTF-8 (str is always valid UTF-8 in Rust,
		// but byte-level slicing at non-char boundaries would have panicked before #762 fix)
		assert!(
			result.contains("characters truncated"),
			"Regression #762: truncation message must be present, got: {}",
			result
		);
		assert!(
			result.starts_with(expected_start),
			"Regression #762: result must start with a complete character '{}', got: {}",
			expected_start,
			result
		);
		assert!(
			result.ends_with(expected_end),
			"Regression #762: result must end with a complete character '{}', got: {}",
			expected_end,
			result
		);
		// Verify the split position is on a character boundary by checking char count
		let prefix: &str = result.split(" ... ").next().unwrap_or("");
		let prefix_char_count = prefix.chars().count();
		assert_eq!(
			prefix_char_count,
			max_chars / 2,
			"Regression #762: prefix must contain exactly half of max_chars characters, got {}",
			prefix_char_count
		);
	}

	// Regression test for #762: truncate_param with a string that starts with ASCII
	// and transitions to multibyte. The split point may land at the transition boundary.
	#[rstest]
	fn test_truncate_param_ascii_then_multibyte_split_regression() {
		// Arrange - 10 ASCII chars then many 3-byte CJK chars
		let input = format!("{}{}", "abcdefghij", "あ".repeat(200));

		// Act
		let result = truncate_param(&input, 30);

		// Assert - result must contain truncation message and be valid UTF-8
		assert!(
			result.contains("characters truncated"),
			"Regression #762: truncation message expected, got: {}",
			result
		);
		// Verify all slices are on valid char boundaries (no panic means the fix works)
		let _ = result.chars().count(); // would panic if result contained invalid UTF-8
	}
}
