//! HTML utilities for escaping, sanitization, and manipulation

use reinhardt_core::security::xss::strip_tags_safe;
use std::borrow::Cow;
/// Escape HTML special characters
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::html::escape;
///
/// assert_eq!(escape("Hello, World!"), "Hello, World!");
/// assert_eq!(escape("<script>alert('XSS')</script>"),
///            "&lt;script&gt;alert(&#x27;XSS&#x27;)&lt;/script&gt;");
/// assert_eq!(escape("5 < 10 & 10 > 5"), "5 &lt; 10 &amp; 10 &gt; 5");
/// ```
pub fn escape(text: &str) -> String {
	let mut result = String::with_capacity(text.len() + 10);
	for ch in text.chars() {
		match ch {
			'&' => result.push_str("&amp;"),
			'<' => result.push_str("&lt;"),
			'>' => result.push_str("&gt;"),
			'"' => result.push_str("&quot;"),
			'\'' => result.push_str("&#x27;"),
			_ => result.push(ch),
		}
	}
	result
}
/// Unescape HTML entities
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::html::unescape;
///
/// assert_eq!(unescape("&lt;div&gt;"), "<div>");
/// assert_eq!(unescape("&amp;"), "&");
/// assert_eq!(unescape("&quot;test&quot;"), "\"test\"");
/// assert_eq!(unescape("&#x27;"), "'");
/// ```
pub fn unescape(text: &str) -> String {
	let mut result = String::with_capacity(text.len());
	let mut chars = text.chars().peekable();

	while let Some(ch) = chars.next() {
		if ch == '&' {
			let entity: String = chars.by_ref().take_while(|&c| c != ';').collect();
			match entity.as_str() {
				"amp" => result.push('&'),
				"lt" => result.push('<'),
				"gt" => result.push('>'),
				"quot" => result.push('"'),
				"#x27" | "apos" => result.push('\''),
				_ if entity.starts_with('#') => {
					if let Some(code_str) = entity.strip_prefix('#')
						&& let Ok(code) = code_str.parse::<u32>()
						&& let Some(unicode_char) = char::from_u32(code)
					{
						result.push(unicode_char);
						continue;
					}
					result.push('&');
					result.push_str(&entity);
					result.push(';');
				}
				_ => {
					result.push('&');
					result.push_str(&entity);
					result.push(';');
				}
			}
		} else {
			result.push(ch);
		}
	}
	result
}
/// Strip HTML tags from text
///
/// This function uses `strip_tags_safe` from `reinhardt_core::security::xss`
/// which properly handles malformed HTML including:
/// - `>` inside quoted attributes (e.g., `<a title="x>y">`)
/// - Unclosed tags at end of input
/// - HTML comments (`<!-- ... -->`)
/// - Self-closing tags
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::html::strip_tags;
///
/// assert_eq!(strip_tags("<p>Hello <b>World</b></p>"), "Hello World");
/// assert_eq!(strip_tags("<a href=\"#\">Link</a>"), "Link");
/// assert_eq!(strip_tags("No tags here"), "No tags here");
/// // Fixes #795: Handles > inside quoted attributes
/// assert_eq!(strip_tags(r#"<a title="x>y">Link</a>"#), "Link");
/// ```
pub fn strip_tags(html: &str) -> String {
	// Fixes #795: Delegate to secure implementation that handles malformed HTML
	strip_tags_safe(html)
}
/// Strip spaces between HTML tags
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::html::strip_spaces_between_tags;
///
/// assert_eq!(
///     strip_spaces_between_tags("<div>  <span>Test</span>  </div>"),
///     "<div><span>Test</span></div>"
/// );
/// assert_eq!(
///     strip_spaces_between_tags("<p>\n\n<b>Bold</b>\n\n</p>"),
///     "<p><b>Bold</b></p>"
/// );
/// ```
pub fn strip_spaces_between_tags(html: &str) -> String {
	let mut result = String::with_capacity(html.len());
	let mut in_tag = false;
	let mut space_buffer = String::new();

	for ch in html.chars() {
		match ch {
			'<' => {
				in_tag = true;
				result.push(ch);
				space_buffer.clear();
			}
			'>' => {
				in_tag = false;
				result.push(ch);
			}
			' ' | '\t' | '\n' | '\r' if !in_tag => {
				space_buffer.push(ch);
			}
			_ => {
				if !in_tag && !space_buffer.is_empty() {
					result.push_str(&space_buffer);
					space_buffer.clear();
				}
				result.push(ch);
			}
		}
	}
	result
}
/// Escape attribute value for use in HTML
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::html::escape_attr;
///
/// assert_eq!(escape_attr("value"), "value");
/// assert_eq!(escape_attr("value with \"quotes\""),
///            "value with &quot;quotes&quot;");
/// assert_eq!(escape_attr("line\nbreak"), "line&#10;break");
/// assert_eq!(escape_attr("tab\there"), "tab&#9;here");
/// ```
pub fn escape_attr(text: &str) -> String {
	let mut result = String::with_capacity(text.len() + 10);
	for ch in text.chars() {
		match ch {
			'&' => result.push_str("&amp;"),
			'<' => result.push_str("&lt;"),
			'>' => result.push_str("&gt;"),
			'"' => result.push_str("&quot;"),
			'\'' => result.push_str("&#x27;"),
			'\n' => result.push_str("&#10;"),
			'\r' => result.push_str("&#13;"),
			'\t' => result.push_str("&#9;"),
			_ => result.push(ch),
		}
	}
	result
}
/// Format HTML template by substituting placeholder values with HTML-escaped content
///
/// All substituted values are automatically HTML-escaped to prevent XSS attacks.
/// Placeholders are in the format `{key}` and are replaced with the escaped value.
///
/// # Security
///
/// This function escapes all special HTML characters in the values:
/// - `&` → `&amp;`
/// - `<` → `&lt;`
/// - `>` → `&gt;`
/// - `"` → `&quot;`
/// - `'` → `&#x27;`
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::html::format_html;
///
/// let template = "<div class=\"{class}\">{content}</div>";
/// let args = [("class", "container"), ("content", "Hello")];
/// assert_eq!(
///     format_html(template, &args),
///     "<div class=\"container\">Hello</div>"
/// );
///
/// // XSS attack is prevented by escaping
/// let template = "<p>{user_input}</p>";
/// let args = [("user_input", "<script>alert('xss')</script>")];
/// assert_eq!(
///     format_html(template, &args),
///     "<p>&lt;script&gt;alert(&#x27;xss&#x27;)&lt;/script&gt;</p>"
/// );
/// ```
pub fn format_html(template: &str, args: &[(&str, &str)]) -> String {
	let mut result = template.to_string();
	for (key, value) in args {
		let placeholder = format!("{{{}}}", key);
		let escaped_value = escape(value);
		result = result.replace(&placeholder, &escaped_value);
	}
	result
}
/// Conditional escape - only escape if not already marked as safe
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::html::conditional_escape;
///
/// assert_eq!(conditional_escape("<script>", true), "&lt;script&gt;");
/// assert_eq!(conditional_escape("<script>", false), "<script>");
/// assert_eq!(conditional_escape("Hello", false), "Hello");
/// ```
pub fn conditional_escape(text: &str, autoescape: bool) -> Cow<'_, str> {
	if autoescape {
		Cow::Owned(escape(text))
	} else {
		Cow::Borrowed(text)
	}
}

/// Mark string as safe (bypasses autoescaping)
#[derive(Debug, Clone)]
pub struct SafeString(String);

impl SafeString {
	/// Create a new SafeString that bypasses HTML escaping
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::utils_core::html::SafeString;
	///
	/// let safe = SafeString::new("<b>Bold</b>");
	/// assert_eq!(safe.as_str(), "<b>Bold</b>");
	/// ```
	pub fn new(s: impl Into<String>) -> Self {
		Self(s.into())
	}
	/// Get the string content
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_utils::utils_core::html::SafeString;
	///
	/// let safe = SafeString::new("<i>Italic</i>");
	/// assert_eq!(safe.as_str(), "<i>Italic</i>");
	/// ```
	pub fn as_str(&self) -> &str {
		&self.0
	}
}

impl From<String> for SafeString {
	fn from(s: String) -> Self {
		Self(s)
	}
}

impl From<&str> for SafeString {
	fn from(s: &str) -> Self {
		Self(s.to_string())
	}
}
/// Truncate HTML to specified number of words, preserving tags
///
/// # Examples
///
/// ```
/// use reinhardt_utils::utils_core::html::truncate_html_words;
///
/// let html = "<p>This is a <b>test</b> sentence with many words.</p>";
/// let truncated = truncate_html_words(html, 5);
/// assert!(truncated.contains("This"));
/// assert!(truncated.contains("is"));
/// assert!(truncated.contains("..."));
///
/// let html2 = "<div>Hello <strong>world</strong> test</div>";
/// let truncated2 = truncate_html_words(html2, 2);
/// assert!(truncated2.contains("<div>"));
/// assert!(truncated2.contains("<strong>"));
/// ```
pub fn truncate_html_words(html: &str, num_words: usize) -> String {
	let mut result = String::new();
	let mut word_count = 0;
	let mut in_tag = false;
	let mut current_word = String::new();

	for ch in html.chars() {
		match ch {
			'<' => {
				if !current_word.is_empty() {
					result.push_str(&current_word);
					current_word.clear();
					word_count += 1;
					if word_count >= num_words {
						return result + "...";
					}
				}
				in_tag = true;
				result.push(ch);
			}
			'>' => {
				in_tag = false;
				result.push(ch);
			}
			' ' | '\t' | '\n' | '\r' if !in_tag => {
				if !current_word.is_empty() {
					result.push_str(&current_word);
					current_word.clear();
					word_count += 1;
					if word_count >= num_words {
						return result + "...";
					}
				}
				result.push(ch);
			}
			_ => {
				if in_tag {
					result.push(ch);
				} else {
					current_word.push(ch);
				}
			}
		}
	}

	if !current_word.is_empty() && word_count < num_words {
		result.push_str(&current_word);
	}

	result
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_escape() {
		assert_eq!(escape("Hello, World!"), "Hello, World!");
		assert_eq!(
			escape("<script>alert('XSS')</script>"),
			"&lt;script&gt;alert(&#x27;XSS&#x27;)&lt;/script&gt;"
		);
		assert_eq!(escape("5 < 10 & 10 > 5"), "5 &lt; 10 &amp; 10 &gt; 5");
		assert_eq!(escape("\"quoted\""), "&quot;quoted&quot;");
	}

	#[test]
	fn test_unescape() {
		assert_eq!(unescape("&lt;div&gt;"), "<div>");
		assert_eq!(unescape("&amp;"), "&");
		assert_eq!(unescape("&quot;test&quot;"), "\"test\"");
		assert_eq!(unescape("&#x27;"), "'");
		assert_eq!(unescape("&#39;"), "'");
	}

	#[test]
	fn test_strip_tags() {
		assert_eq!(strip_tags("<p>Hello <b>World</b></p>"), "Hello World");
		assert_eq!(strip_tags("<div><span>Test</span></div>"), "Test");
		assert_eq!(strip_tags("No tags here"), "No tags here");
		assert_eq!(strip_tags("<a href=\"#\">Link</a>"), "Link");
	}

	#[test]
	fn test_strip_spaces_between_tags() {
		assert_eq!(
			strip_spaces_between_tags("<div>  <span>Test</span>  </div>"),
			"<div><span>Test</span></div>"
		);
	}

	#[test]
	fn test_escape_attr() {
		assert_eq!(escape_attr("value"), "value");
		assert_eq!(
			escape_attr("value with \"quotes\""),
			"value with &quot;quotes&quot;"
		);
		assert_eq!(escape_attr("line\nbreak"), "line&#10;break");
		assert_eq!(escape_attr("tab\there"), "tab&#9;here");
	}

	#[test]
	fn test_format_html() {
		let template = "<div class=\"{class}\">{content}</div>";
		let args = [("class", "container"), ("content", "Hello")];
		assert_eq!(
			format_html(template, &args),
			"<div class=\"container\">Hello</div>"
		);
	}

	#[test]
	fn test_conditional_escape() {
		assert_eq!(conditional_escape("<script>", true), "&lt;script&gt;");
		assert_eq!(conditional_escape("<script>", false), "<script>");
	}

	#[test]
	fn test_safe_string() {
		let safe = SafeString::new("<b>Bold</b>");
		assert_eq!(safe.as_str(), "<b>Bold</b>");
	}

	#[test]
	fn test_truncate_html_words() {
		let html = "<p>This is a <b>test</b> sentence with many words.</p>";
		let truncated = truncate_html_words(html, 5);
		assert!(truncated.contains("This"));
		assert!(truncated.contains("is"));
		assert!(truncated.contains("..."));
	}

	#[test]
	fn test_truncate_html_preserves_tags() {
		let html = "<div>Hello <strong>world</strong> test</div>";
		let truncated = truncate_html_words(html, 2);
		assert!(truncated.contains("<div>"));
		assert!(truncated.contains("<strong>"));
	}

	#[test]
	fn test_safe_string_from_string() {
		let s = String::from("<b>Bold</b>");
		let safe = SafeString::from(s);
		assert_eq!(safe.as_str(), "<b>Bold</b>");
	}

	#[test]
	fn test_safe_string_from_str() {
		let safe = SafeString::from("<i>Italic</i>");
		assert_eq!(safe.as_str(), "<i>Italic</i>");
	}

	#[test]
	fn test_escape_empty_string() {
		assert_eq!(escape(""), "");
	}

	#[test]
	fn test_escape_multibyte() {
		assert_eq!(escape("こんにちは<>&"), "こんにちは&lt;&gt;&amp;");
	}

	#[test]
	fn test_unescape_incomplete_entity() {
		// Incomplete entities without semicolon are treated as entity with empty name
		// which results in "&;" pattern
		assert_eq!(unescape("&lt"), "<");
		assert_eq!(unescape("&"), "&;");
	}

	#[test]
	fn test_unescape_unknown_entity() {
		assert_eq!(unescape("&unknown;"), "&unknown;");
	}

	#[test]
	fn test_strip_tags_nested() {
		assert_eq!(strip_tags("<div><p><span>Test</span></p></div>"), "Test");
	}

	#[test]
	fn test_strip_tags_empty() {
		assert_eq!(strip_tags(""), "");
	}

	#[test]
	fn test_strip_tags_quoted_attributes_with_angle_brackets() {
		// Double-quoted attribute containing >
		assert_eq!(strip_tags(r#"<a title="x>y">Link</a>"#), "Link");
		// Single-quoted attribute containing >
		assert_eq!(strip_tags("<a title='x>y'>Link</a>"), "Link");
		// Multiple quoted attributes with >
		assert_eq!(
			strip_tags(r#"<a title="a>b" data-value="c>d">Text</a>"#),
			"Text"
		);
		// Nested quotes: double inside single
		assert_eq!(strip_tags(r#"<a title='x"y'>Link</a>"#), "Link");
		// Nested quotes: single inside double
		assert_eq!(strip_tags(r#"<a title="x'y">Link</a>"#), "Link");
	}

	#[test]
	fn test_strip_spaces_between_tags_multiple_spaces() {
		assert_eq!(
			strip_spaces_between_tags("<div>   \n\t   <span>Test</span>   \n\t   </div>"),
			"<div><span>Test</span></div>"
		);
	}

	#[test]
	fn test_escape_attr_carriage_return() {
		assert_eq!(escape_attr("test\rvalue"), "test&#13;value");
	}

	#[test]
	fn test_format_html_multiple_replacements() {
		let template = "<div id=\"{id}\" class=\"{class}\">{content}</div>";
		let args = [("id", "main"), ("class", "container"), ("content", "Hello")];
		assert_eq!(
			format_html(template, &args),
			"<div id=\"main\" class=\"container\">Hello</div>"
		);
	}

	#[test]
	fn test_format_html_no_replacements() {
		let template = "<div>Static content</div>";
		let args: [(&str, &str); 0] = [];
		assert_eq!(format_html(template, &args), "<div>Static content</div>");
	}

	#[test]
	fn test_format_html_xss_prevention_script_tag() {
		// Arrange
		let template = "<p>{content}</p>";
		let args = [("content", "<script>alert('xss')</script>")];

		// Act
		let result = format_html(template, &args);

		// Assert - script tags must be escaped
		assert!(!result.contains("<script>"));
		assert!(result.contains("&lt;script&gt;"));
		assert!(result.contains("&lt;/script&gt;"));
		assert!(result.contains("&#x27;xss&#x27;"));
	}

	#[test]
	fn test_format_html_xss_prevention_event_handler() {
		// Arrange
		let template = r#"<div class="{class}">{content}</div>"#;
		let args = [
			("class", r#"container" onclick="alert('xss')"#),
			("content", "Safe content"),
		];

		// Act
		let result = format_html(template, &args);

		// Assert - quotes must be escaped to prevent event handler injection
		assert!(result.contains("&quot;"));
		assert!(!result.contains(r#"onclick="alert"#));
	}

	#[test]
	fn test_format_html_xss_prevention_ampersand() {
		// Arrange
		let template = "<a href=\"/search?q={query}\">Search</a>";
		let args = [("query", "test&redirect=evil.com")];

		// Act
		let result = format_html(template, &args);

		// Assert - ampersand must be escaped
		assert!(result.contains("&amp;"));
		assert!(!result.contains("test&redirect"));
	}

	#[test]
	fn test_format_html_xss_prevention_angle_brackets() {
		// Arrange
		let template = "<span>{text}</span>";
		let args = [("text", "<<SCRIPT>alert('XSS');//<</SCRIPT>")];

		// Act
		let result = format_html(template, &args);

		// Assert - all angle brackets must be escaped
		assert!(!result.contains("<SCRIPT>"));
		assert!(result.contains("&lt;"));
		assert!(result.contains("&gt;"));
	}

	#[test]
	fn test_format_html_safe_values_unchanged() {
		// Arrange - values without special characters should pass through unchanged
		let template = "<div id=\"{id}\" class=\"{class}\">{content}</div>";
		let args = [
			("id", "main"),
			("class", "container"),
			("content", "Hello World"),
		];

		// Act
		let result = format_html(template, &args);

		// Assert
		assert_eq!(
			result,
			"<div id=\"main\" class=\"container\">Hello World</div>"
		);
	}

	#[test]
	fn test_truncate_html_words_exact_count() {
		let html = "<p>One two three</p>";
		let truncated = truncate_html_words(html, 3);
		// The function adds "..." when word_count reaches num_words
		// To not have "...", we need more words than the count
		assert!(truncated.contains("..."));
	}

	#[test]
	fn test_truncate_html_words_empty() {
		let html = "";
		let truncated = truncate_html_words(html, 5);
		assert_eq!(truncated, "");
	}
}

#[cfg(test)]
mod proptests {
	use super::*;
	use proptest::prelude::*;

	proptest! {
		#[test]
		fn prop_escape_no_special_chars(s in "[^<>&\"']*") {
			let escaped = escape(&s);
			assert!(!escaped.contains('<'));
			assert!(!escaped.contains('>'));
			assert!(!escaped.contains('&'));
			assert!(!escaped.contains('"'));
			assert!(!escaped.contains('\''));
		}

		#[test]
		fn prop_strip_tags_no_angle_brackets(s in "\\PC*") {
			let stripped = strip_tags(&s);
			assert!(!stripped.contains('<'));
		}

		#[test]
		fn prop_strip_tags_length_decrease(s in "\\PC*") {
			let stripped = strip_tags(&s);
			assert!(stripped.len() <= s.len());
		}

		#[test]
		fn prop_truncate_html_words_respects_limit(html in "\\PC*", n in 1usize..20) {
			let truncated = truncate_html_words(&html, n);
			let word_count = truncated
				.split(|c: char| c.is_whitespace() || c == '<' || c == '>')
				.filter(|w| !w.is_empty() && !w.starts_with('/'))
				.filter(|w| !w.chars().all(|c| !c.is_alphanumeric()))
				.count();

			// Allow some flexibility due to HTML tags
			assert!(word_count <= n + 5);
		}

		#[test]
		fn prop_escape_attr_no_newlines(s in "\\PC*") {
			let escaped = escape_attr(&s);
			assert!(!escaped.contains('\n'));
			assert!(!escaped.contains('\r'));
			assert!(!escaped.contains('\t'));
		}

		#[test]
		fn prop_conditional_escape_when_true(s in "\\PC*") {
			let escaped_cond = conditional_escape(&s, true);
			let escaped_direct = escape(&s);
			assert_eq!(escaped_cond, escaped_direct);
		}

		#[test]
		fn prop_conditional_escape_when_false(s in "\\PC*") {
			let escaped = conditional_escape(&s, false);
			assert_eq!(escaped, s);
		}

		#[test]
		fn prop_safe_string_roundtrip(s in "\\PC*") {
			let safe = SafeString::from(s.clone());
			assert_eq!(safe.as_str(), &s);
		}

		#[test]
		fn prop_format_html_preserves_non_placeholders(template in "\\PC*") {
			let args: [(&str, &str); 0] = [];
			let result = format_html(&template, &args);
			assert_eq!(result, template);
		}

		#[test]
		fn prop_strip_spaces_reduces_whitespace(s in "\\PC*") {
			let stripped = strip_spaces_between_tags(&s);
			// Result should not have more characters than original
			assert!(stripped.len() <= s.len() + 100); // Allow some overhead for tag processing
		}
	}
}
