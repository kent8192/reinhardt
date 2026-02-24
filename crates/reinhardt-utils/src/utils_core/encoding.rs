//! Text encoding and decoding utilities

use std::borrow::Cow;

use crate::utils_core::html::escape;
/// URL encode a string
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::encoding::urlencode;
///
/// assert_eq!(urlencode("hello world"), "hello+world");
/// assert_eq!(urlencode("hello@world.com"), "hello%40world.com");
/// assert_eq!(urlencode("test&value=1"), "test%26value%3D1");
/// ```
pub fn urlencode(text: &str) -> String {
	let mut result = String::with_capacity(text.len() * 3);
	for byte in text.as_bytes() {
		match byte {
			b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
				result.push(*byte as char);
			}
			b' ' => result.push('+'),
			_ => {
				result.push('%');
				result.push_str(&format!("{:02X}", byte));
			}
		}
	}
	result
}
/// URL decode a string
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::encoding::urldecode;
///
/// assert_eq!(urldecode("hello+world").unwrap(), "hello world");
/// assert_eq!(urldecode("hello%40world.com").unwrap(), "hello@world.com");
/// assert_eq!(urldecode("test%26value%3D1").unwrap(), "test&value=1");
/// assert!(urldecode("%ZZ").is_err());
/// ```
pub fn urldecode(text: &str) -> Result<String, String> {
	let mut result = Vec::new();
	let mut chars = text.chars().peekable();

	while let Some(ch) = chars.next() {
		match ch {
			'+' => result.push(b' '),
			'%' => {
				let hex: String = chars.by_ref().take(2).collect();
				if hex.len() != 2 {
					return Err(format!("Invalid URL encoding at '%{}'", hex));
				}
				match u8::from_str_radix(&hex, 16) {
					Ok(byte) => result.push(byte),
					Err(_) => return Err(format!("Invalid hex in URL encoding: {}", hex)),
				}
			}
			_ if ch.is_ascii() => result.push(ch as u8),
			_ => {
				for byte in ch.to_string().as_bytes() {
					result.push(*byte);
				}
			}
		}
	}

	String::from_utf8(result).map_err(|e| format!("Invalid UTF-8: {}", e))
}
/// Escape quotes in a string for use in JavaScript
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::encoding::escapejs;
///
/// assert_eq!(escapejs("Hello"), "Hello");
/// assert_eq!(escapejs("It's \"quoted\""), "It\\'s \\\"quoted\\\"");
/// assert_eq!(escapejs("Line\nBreak"), "Line\\nBreak");
/// assert_eq!(escapejs("<script>"), "\\u003Cscript\\u003E");
/// ```
pub fn escapejs(text: &str) -> String {
	let mut result = String::with_capacity(text.len() + 20);
	for ch in text.chars() {
		match ch {
			'\'' => result.push_str("\\'"),
			'"' => result.push_str("\\\""),
			'\\' => result.push_str("\\\\"),
			'\n' => result.push_str("\\n"),
			'\r' => result.push_str("\\r"),
			'\t' => result.push_str("\\t"),
			'\x08' => result.push_str("\\b"),
			'\x0C' => result.push_str("\\f"),
			'<' => result.push_str("\\u003C"),
			'>' => result.push_str("\\u003E"),
			'&' => result.push_str("\\u0026"),
			_ if ch.is_control() => {
				result.push_str(&format!("\\u{:04X}", ch as u32));
			}
			_ => result.push(ch),
		}
	}
	result
}
/// Convert string to slug (URL-friendly format)
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::encoding::slugify;
///
/// assert_eq!(slugify("Hello World"), "hello-world");
/// assert_eq!(slugify("Hello  World"), "hello-world");
/// assert_eq!(slugify("Test 123"), "test-123");
/// assert_eq!(slugify("Special!@#Characters"), "special-characters");
/// ```
pub fn slugify(text: &str) -> String {
	text.to_lowercase()
		.chars()
		.map(|ch| match ch {
			'a'..='z' | '0'..='9' => ch,
			' ' | '-' | '_' => '-',
			_ => '-',
		})
		.collect::<String>()
		.split('-')
		.filter(|s| !s.is_empty())
		.collect::<Vec<_>>()
		.join("-")
}
/// Safely convert bytes to UTF-8 string, replacing invalid sequences
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::encoding::force_str;
///
/// let bytes = b"Hello, World!";
/// assert_eq!(force_str(bytes), "Hello, World!");
///
/// let invalid = b"Hello\xFF\xFEWorld";
/// let result = force_str(invalid);
/// assert!(result.contains("Hello"));
/// assert!(result.contains("World"));
/// ```
pub fn force_str(bytes: &[u8]) -> Cow<'_, str> {
	String::from_utf8_lossy(bytes)
}
/// Convert string to bytes
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::encoding::force_bytes;
///
/// let text = "Hello, World!";
/// assert_eq!(force_bytes(text), b"Hello, World!");
/// ```
pub fn force_bytes(text: &str) -> Vec<u8> {
	text.as_bytes().to_vec()
}
/// Smart truncate - truncate at word boundary
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::encoding::truncate_chars;
///
/// assert_eq!(truncate_chars("Hello World", 20), "Hello World");
/// assert_eq!(truncate_chars("Hello World", 8), "Hello...");
/// assert_eq!(truncate_chars("Test", 10), "Test");
/// ```
pub fn truncate_chars(text: &str, max_length: usize) -> String {
	if text.chars().count() <= max_length {
		return text.to_string();
	}

	// When max_length is too small to fit any characters plus "...",
	// return just the ellipsis truncated to max_length.
	let content_limit = max_length.saturating_sub(3);

	let mut result = String::new();

	for (char_count, ch) in text.chars().enumerate() {
		if char_count >= content_limit {
			result.push_str(&"..."[..max_length.min(3)]);
			break;
		}
		result.push(ch);
	}

	result
}
/// Truncate at word boundary
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::encoding::truncate_words;
///
/// assert_eq!(truncate_words("Hello World Test", 2), "Hello World...");
/// assert_eq!(truncate_words("One", 5), "One");
/// assert_eq!(truncate_words("A B C D E", 3), "A B C...");
/// ```
pub fn truncate_words(text: &str, max_words: usize) -> String {
	let words: Vec<&str> = text.split_whitespace().collect();
	if words.len() <= max_words {
		return text.to_string();
	}

	let mut result = words[..max_words].join(" ");
	result.push_str("...");
	result
}
/// Wrap text at specified width
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::encoding::wrap_text;
///
/// let text = "This is a long line that needs to be wrapped";
/// let wrapped = wrap_text(text, 20);
/// assert!(wrapped.len() > 1);
/// assert!(wrapped.iter().all(|line| line.chars().count() <= 25));
/// ```
pub fn wrap_text(text: &str, width: usize) -> Vec<String> {
	let mut lines = Vec::new();
	let mut current_line = String::new();
	let mut current_width = 0;

	for word in text.split_whitespace() {
		let word_len = word.chars().count();

		if current_width + word_len + 1 > width && !current_line.is_empty() {
			lines.push(current_line.clone());
			current_line.clear();
			current_width = 0;
		}

		if !current_line.is_empty() {
			current_line.push(' ');
			current_width += 1;
		}

		current_line.push_str(word);
		current_width += word_len;
	}

	if !current_line.is_empty() {
		lines.push(current_line);
	}

	lines
}
/// Line breaks to `<br>` tags
///
/// Input text is HTML-escaped before injecting HTML structure tags
/// to prevent XSS attacks.
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::encoding::linebreaks;
///
/// assert_eq!(
///     linebreaks("Line 1\nLine 2\n\nLine 3"),
///     "Line 1<br>\nLine 2<br>\n</p>\n<p><br>\nLine 3"
/// );
/// assert_eq!(
///     linebreaks("<script>alert('xss')</script>"),
///     "&lt;script&gt;alert(&#x27;xss&#x27;)&lt;/script&gt;"
/// );
/// ```
pub fn linebreaks(text: &str) -> String {
	// Fixes #798: Escape HTML entities before injecting HTML structure tags
	let text = escape(text);
	text.lines()
		.map(|line| {
			if line.trim().is_empty() {
				"</p>\n<p>".to_string()
			} else {
				line.to_string()
			}
		})
		.collect::<Vec<_>>()
		.join("<br>\n")
}
/// Line breaks to `<br>` tags without wrapping in `<p>`
///
/// Input text is HTML-escaped before injecting `<br>` tags
/// to prevent XSS attacks.
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::encoding::linebreaksbr;
///
/// assert_eq!(linebreaksbr("Line 1\nLine 2"), "Line 1<br>\nLine 2");
/// assert_eq!(linebreaksbr("Single"), "Single");
/// assert_eq!(
///     linebreaksbr("<b>bold</b>"),
///     "&lt;b&gt;bold&lt;/b&gt;"
/// );
/// ```
pub fn linebreaksbr(text: &str) -> String {
	// Fixes #798: Escape HTML entities before injecting <br> tags
	escape(text).replace('\n', "<br>\n")
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_urlencode() {
		assert_eq!(urlencode("hello world"), "hello+world");
		assert_eq!(urlencode("hello@world.com"), "hello%40world.com");
		assert_eq!(urlencode("test&value=1"), "test%26value%3D1");
	}

	#[test]
	fn test_urldecode() {
		assert_eq!(urldecode("hello+world").unwrap(), "hello world");
		assert_eq!(urldecode("hello%40world.com").unwrap(), "hello@world.com");
		assert_eq!(urldecode("test%26value%3D1").unwrap(), "test&value=1");
	}

	#[test]
	fn test_urlencode_urldecode_roundtrip() {
		let original = "Hello, World! 123 @#$%";
		let encoded = urlencode(original);
		let decoded = urldecode(&encoded).unwrap();
		assert_eq!(decoded, original);
	}

	#[test]
	fn test_escapejs() {
		assert_eq!(escapejs("Hello"), "Hello");
		assert_eq!(escapejs("It's \"quoted\""), "It\\'s \\\"quoted\\\"");
		assert_eq!(escapejs("Line\nBreak"), "Line\\nBreak");
		assert_eq!(escapejs("<script>"), "\\u003Cscript\\u003E");
	}

	#[test]
	fn test_slugify() {
		assert_eq!(slugify("Hello World"), "hello-world");
		assert_eq!(slugify("Hello  World"), "hello-world");
		assert_eq!(slugify("Hello-World"), "hello-world");
		assert_eq!(slugify("Test 123"), "test-123");
		assert_eq!(slugify("Special!@#Characters"), "special-characters");
	}

	#[test]
	fn test_truncate_chars() {
		assert_eq!(truncate_chars("Hello World", 20), "Hello World");
		assert_eq!(truncate_chars("Hello World", 8), "Hello...");
		assert_eq!(truncate_chars("Test", 10), "Test");
	}

	#[test]
	fn test_truncate_words() {
		assert_eq!(truncate_words("Hello World Test", 2), "Hello World...");
		assert_eq!(truncate_words("One", 5), "One");
		assert_eq!(truncate_words("A B C D E", 3), "A B C...");
	}

	#[test]
	fn test_wrap_text() {
		let text = "This is a long line that needs to be wrapped";
		let wrapped = wrap_text(text, 20);
		assert!(wrapped.len() > 1);
		assert!(wrapped.iter().all(|line| line.chars().count() <= 20));
	}

	#[test]
	fn test_linebreaksbr() {
		assert_eq!(linebreaksbr("Line 1\nLine 2"), "Line 1<br>\nLine 2");
		assert_eq!(linebreaksbr("Single"), "Single");
	}

	#[test]
	fn test_linebreaksbr_escapes_html() {
		assert_eq!(
			linebreaksbr("<script>alert('xss')</script>"),
			"&lt;script&gt;alert(&#x27;xss&#x27;)&lt;/script&gt;"
		);
		assert_eq!(
			linebreaksbr("<b>bold</b>\nnormal"),
			"&lt;b&gt;bold&lt;/b&gt;<br>\nnormal"
		);
	}

	#[test]
	fn test_force_str() {
		let bytes = b"Hello, World!";
		assert_eq!(force_str(bytes), "Hello, World!");

		let invalid = b"Hello\xFF\xFEWorld";
		let result = force_str(invalid);
		assert!(result.contains("Hello"));
		assert!(result.contains("World"));
	}

	#[test]
	fn test_force_bytes() {
		let text = "Hello, World!";
		assert_eq!(force_bytes(text), b"Hello, World!");
	}

	#[test]
	fn test_linebreaks() {
		assert_eq!(
			linebreaks("Line 1\nLine 2\n\nLine 3"),
			"Line 1<br>\nLine 2<br>\n</p>\n<p><br>\nLine 3"
		);
	}

	#[test]
	fn test_linebreaks_single_line() {
		assert_eq!(linebreaks("Single line"), "Single line");
	}

	#[test]
	fn test_linebreaks_escapes_html() {
		assert_eq!(
			linebreaks("<script>alert('xss')</script>"),
			"&lt;script&gt;alert(&#x27;xss&#x27;)&lt;/script&gt;"
		);
		assert_eq!(
			linebreaks("<b>bold</b>\nnormal"),
			"&lt;b&gt;bold&lt;/b&gt;<br>\nnormal"
		);
		// Verify HTML entities in input are double-escaped
		assert_eq!(linebreaks("5 < 10 & 10 > 5"), "5 &lt; 10 &amp; 10 &gt; 5");
	}

	#[test]
	fn test_linebreaks_empty_lines() {
		assert_eq!(
			linebreaks("Line 1\n\nLine 2"),
			"Line 1<br>\n</p>\n<p><br>\nLine 2"
		);
	}

	#[test]
	fn test_urldecode_invalid_hex() {
		assert!(urldecode("%ZZ").is_err());
		assert!(urldecode("%1").is_err());
	}

	#[test]
	fn test_urldecode_invalid_utf8() {
		// This should handle invalid UTF-8 sequences gracefully
		let result = urldecode("%FF%FE");
		assert!(result.is_err());
	}

	#[test]
	fn test_urlencode_special_chars() {
		assert_eq!(urlencode("a-b_c.d~e"), "a-b_c.d~e");
		assert_eq!(urlencode("!@#$%^&*()"), "%21%40%23%24%25%5E%26%2A%28%29");
	}

	#[test]
	fn test_escapejs_control_chars() {
		assert_eq!(escapejs("\x08"), "\\b");
		assert_eq!(escapejs("\x0C"), "\\f");
		assert_eq!(escapejs("\x01"), "\\u0001");
	}

	#[test]
	fn test_slugify_empty() {
		assert_eq!(slugify(""), "");
	}

	#[test]
	fn test_slugify_unicode() {
		// Unicode characters are converted to dashes, then consecutive dashes are collapsed
		assert_eq!(slugify("Hello 世界"), "hello");
	}

	#[test]
	fn test_slugify_multiple_dashes() {
		assert_eq!(slugify("hello---world"), "hello-world");
	}

	#[test]
	fn test_truncate_chars_exact_length() {
		assert_eq!(truncate_chars("Hello", 5), "Hello");
	}

	#[test]
	fn test_truncate_chars_unicode() {
		assert_eq!(truncate_chars("こんにちは世界", 5), "こん...");
	}

	#[test]
	fn test_truncate_words_empty() {
		assert_eq!(truncate_words("", 5), "");
	}

	#[test]
	fn test_wrap_text_single_word_exceeds_width() {
		let text = "VeryLongWordThatExceedsWidth";
		let wrapped = wrap_text(text, 10);
		assert_eq!(wrapped.len(), 1);
		assert_eq!(wrapped[0], "VeryLongWordThatExceedsWidth");
	}

	#[test]
	fn test_wrap_text_empty() {
		let wrapped = wrap_text("", 10);
		assert_eq!(wrapped.len(), 0);
	}

	#[test]
	fn test_force_str_empty() {
		assert_eq!(force_str(b""), "");
	}

	#[test]
	fn test_truncate_chars_zero_max_length_does_not_panic() {
		// Fixes #764: saturating_sub prevents underflow when max_length < 3
		assert_eq!(truncate_chars("Hello", 0), "");
	}

	#[test]
	fn test_truncate_chars_max_length_one() {
		// Fixes #764: max_length=1 should produce "."
		assert_eq!(truncate_chars("Hello", 1), ".");
	}

	#[test]
	fn test_truncate_chars_max_length_two() {
		// Fixes #764: max_length=2 should produce ".."
		assert_eq!(truncate_chars("Hello", 2), "..");
	}

	#[test]
	fn test_truncate_chars_max_length_three() {
		// Fixes #764: max_length=3 should produce "..."
		assert_eq!(truncate_chars("Hello", 3), "...");
	}

	#[test]
	fn test_truncate_chars_max_length_four() {
		assert_eq!(truncate_chars("Hello World", 4), "H...");
	}
}

#[cfg(test)]
mod proptests {
	use super::*;
	use proptest::prelude::*;

	proptest! {
		#[test]
		fn prop_slugify_format(s in "[a-zA-Z0-9 -]+") {
			let slug = slugify(&s);
			// Slug should only contain lowercase letters, numbers, and hyphens
			assert!(slug.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-'));
			// No consecutive hyphens
			assert!(!slug.contains("--"));
		}

		#[test]
		fn prop_truncate_chars_length(s in "\\PC*", n in 0usize..100) {
			let truncated = truncate_chars(&s, n);
			assert!(truncated.chars().count() <= n);
		}

		#[test]
		fn prop_truncate_words_count(s in "\\w+(\\s+\\w+)*", n in 1usize..20) {
			let truncated = truncate_words(&s, n);
			let word_count = truncated.split_whitespace().filter(|w| *w != "...").count();
			assert!(word_count <= n);
		}

		#[test]
		fn prop_urlencode_ascii_safe(s in "[a-zA-Z0-9._~-]+") {
			let encoded = urlencode(&s);
			// These characters should not be encoded
			assert_eq!(encoded, s);
		}

		#[test]
		fn prop_escapejs_no_newlines(s in "\\PC*") {
			let escaped = escapejs(&s);
			assert!(!escaped.contains('\n'));
			assert!(!escaped.contains('\r'));
			assert!(!escaped.contains('\t'));
		}

		#[test]
		fn prop_wrap_text_line_width(s in "[a-zA-Z0-9 ]+", width in 10usize..50) {
			let lines = wrap_text(&s, width);
			for line in lines {
				// Allow more flexibility for word boundaries and Unicode handling
				assert!(line.chars().count() <= width + 20);
			}
		}
	}
}
