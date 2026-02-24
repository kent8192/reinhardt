//! XSS prevention utilities

use regex::Regex;
use std::sync::OnceLock;

/// Escape HTML special characters
///
/// # Examples
///
/// ```
/// use reinhardt_core::security::escape_html;
///
/// let input = "<script>alert('XSS')</script>";
/// let escaped = escape_html(input);
/// assert_eq!(escaped, "&lt;script&gt;alert(&#x27;XSS&#x27;)&lt;/script&gt;");
///
/// let with_quotes = r#"<a href="javascript:alert('xss')">Click</a>"#;
/// let escaped_quotes = escape_html(with_quotes);
/// assert_eq!(escaped_quotes, "&lt;a href=&quot;javascript:alert(&#x27;xss&#x27;)&quot;&gt;Click&lt;/a&gt;");
/// ```
pub fn escape_html(input: &str) -> String {
	input
		.replace('&', "&amp;")
		.replace('<', "&lt;")
		.replace('>', "&gt;")
		.replace('"', "&quot;")
		.replace('\'', "&#x27;")
}

/// Escape HTML attributes
///
/// # Examples
///
/// ```
/// use reinhardt_core::security::xss::escape_html_attr;
///
/// let attr = r#"value" onload="alert('xss')"#;
/// let escaped = escape_html_attr(attr);
/// // The onload itself remains, but the quotes are escaped to neutralize it
/// assert!(escaped.contains("&quot;"));
/// assert!(escaped.contains("&#x27;"));
/// ```
pub fn escape_html_attr(input: &str) -> String {
	input
		.replace('&', "&amp;")
		.replace('<', "&lt;")
		.replace('>', "&gt;")
		.replace('"', "&quot;")
		.replace('\'', "&#x27;")
		.replace('\n', "&#10;")
		.replace('\r', "&#13;")
}

/// Escape for JavaScript context
///
/// # Examples
///
/// ```
/// use reinhardt_core::security::xss::escape_javascript;
///
/// let script = "'; alert('xss'); var x='";
/// let escaped = escape_javascript(script);
/// // Verify that single quotes are escaped
/// assert!(escaped.contains("\\'"));
/// // Verify the format after escaping
/// assert_eq!(escaped, "\\'; alert(\\'xss\\'); var x=\\'");
/// ```
pub fn escape_javascript(input: &str) -> String {
	input
		.replace('\\', "\\\\")
		.replace('\'', "\\'")
		.replace('"', "\\\"")
		.replace('\n', "\\n")
		.replace('\r', "\\r")
		.replace('\t', "\\t")
		.replace('<', "\\x3C")
		.replace('>', "\\x3E")
		.replace('/', "\\/")
}

/// Escape for URLs
///
/// # Examples
///
/// ```
/// use reinhardt_core::security::xss::escape_url;
///
/// let url = "javascript:alert('xss')";
/// let escaped = escape_url(url);
/// assert!(escaped.contains("%3A"));
/// ```
pub fn escape_url(input: &str) -> String {
	urlencoding::encode(input).to_string()
}

static DANGEROUS_PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();

fn get_dangerous_patterns() -> &'static Vec<Regex> {
	DANGEROUS_PATTERNS.get_or_init(|| {
		vec![
			// JavaScript protocol
			Regex::new(r"(?i)javascript:").unwrap(),
			// Data URI
			Regex::new(r"(?i)data:text/html").unwrap(),
			// VBScript (IE)
			Regex::new(r"(?i)vbscript:").unwrap(),
			// Event handlers
			Regex::new(r"(?i)on\w+\s*=").unwrap(),
			// Dangerous tags like iframe/embed
			Regex::new(r"(?i)<(iframe|embed|object|applet|meta|link|base)").unwrap(),
			// script tag
			Regex::new(r"(?i)<script").unwrap(),
		]
	})
}

/// Detect dangerous patterns
///
/// # Examples
///
/// ```
/// use reinhardt_core::security::xss::detect_xss_patterns;
///
/// assert!(detect_xss_patterns("<script>alert(1)</script>"));
/// assert!(detect_xss_patterns(r#"<img src=x onerror="alert(1)">"#));
/// assert!(detect_xss_patterns("javascript:alert(1)"));
/// assert!(!detect_xss_patterns("Safe text"));
/// ```
pub fn detect_xss_patterns(input: &str) -> bool {
	get_dangerous_patterns()
		.iter()
		.any(|pattern| pattern.is_match(input))
}

/// Sanitize HTML (enhanced implementation)
///
/// # Examples
///
/// ```
/// use reinhardt_core::security::sanitize_html;
///
/// let dangerous = "<script>alert('XSS')</script><b>Bold text</b>";
/// let sanitized = sanitize_html(dangerous);
/// assert_eq!(sanitized, "&lt;script&gt;alert(&#x27;XSS&#x27;)&lt;/script&gt;&lt;b&gt;Bold text&lt;/b&gt;");
///
/// let user_input = "User's comment with <img src=x onerror=alert(1)>";
/// let safe_output = sanitize_html(user_input);
/// assert!(safe_output.contains("&lt;img"));
/// ```
pub fn sanitize_html(input: &str) -> String {
	escape_html(input)
}

/// Validate URLs and allow only safe protocols
///
/// Allows relative paths (`/path`, `./path`), anchor links (`#section`),
/// and safe protocols (`http://`, `https://`, `mailto:`, `ftp://`, `ftps://`).
///
/// Rejects dangerous protocols (`javascript:`, `data:`, `vbscript:`) and
/// path traversal prefixes (`../`).
///
/// # Examples
///
/// ```
/// use reinhardt_core::security::xss::is_safe_url;
///
/// assert!(is_safe_url("https://example.com"));
/// assert!(is_safe_url("http://example.com"));
/// assert!(is_safe_url("/path/to/page"));
/// assert!(is_safe_url("mailto:user@example.com"));
/// assert!(!is_safe_url("javascript:alert(1)"));
/// assert!(!is_safe_url("data:text/html,<script>alert(1)</script>"));
/// assert!(!is_safe_url("../parent/path"));
/// ```
pub fn is_safe_url(url: &str) -> bool {
	let url_lower = url.to_lowercase();

	// Allow relative URLs and anchor links (but NOT parent traversal)
	if url.starts_with('/') || url.starts_with("./") || url.starts_with('#') {
		return true;
	}

	// Allow only safe protocols
	let safe_protocols = ["http://", "https://", "mailto:", "ftp://", "ftps://"];

	safe_protocols
		.iter()
		.any(|protocol| url_lower.starts_with(protocol))
}

/// Strip HTML tags with proper handling of malformed HTML
///
/// Unlike simple state machines, this handles:
/// - `>` inside quoted attributes (e.g., `<a title="x>y">`)
/// - Unclosed tags at end of input
/// - HTML comments (`<!-- ... -->`)
/// - Self-closing tags
///
/// # Examples
///
/// ```
/// use reinhardt_core::security::xss::strip_tags_safe;
///
/// // Basic tag stripping
/// assert_eq!(strip_tags_safe("<p>Hello <b>World</b></p>"), "Hello World");
///
/// // Handles > inside quoted attributes
/// assert_eq!(strip_tags_safe(r#"<a title="x>y">Link</a>"#), "Link");
/// assert_eq!(strip_tags_safe("<a title='x>y'>Link</a>"), "Link");
///
/// // Handles HTML comments
/// assert_eq!(strip_tags_safe("Hello<!-- comment -->World"), "HelloWorld");
///
/// // Handles malformed/unclosed tags
/// assert_eq!(strip_tags_safe("Hello<br"), "Hello");
/// assert_eq!(strip_tags_safe("Hello<"), "Hello");
/// ```
pub fn strip_tags_safe(html: &str) -> String {
	let mut result = String::with_capacity(html.len());
	let chars: Vec<char> = html.chars().collect();
	let len = chars.len();
	let mut i = 0;

	while i < len {
		if chars[i] == '<' {
			// Check for HTML comment
			if i + 3 < len && chars[i + 1] == '!' && chars[i + 2] == '-' && chars[i + 3] == '-' {
				// Skip until -->
				i += 4;
				let mut found_close = false;
				while i + 2 < len {
					if chars[i] == '-' && chars[i + 1] == '-' && chars[i + 2] == '>' {
						i += 3;
						found_close = true;
						break;
					}
					i += 1;
				}
				if !found_close {
					// Unclosed comment, skip to end
					break;
				}
				continue;
			}

			// Inside a tag - skip until matching > (respecting quotes)
			i += 1;
			let mut in_single_quote = false;
			let mut in_double_quote = false;

			while i < len {
				match chars[i] {
					'"' if !in_single_quote => in_double_quote = !in_double_quote,
					'\'' if !in_double_quote => in_single_quote = !in_single_quote,
					'>' if !in_single_quote && !in_double_quote => {
						i += 1;
						break;
					}
					_ => {}
				}
				i += 1;
			}
		} else {
			result.push(chars[i]);
			i += 1;
		}
	}
	result
}

/// Escape HTML content for safe insertion into HTML body
///
/// This is an alias for [`escape_html`] that makes the intent clearer
/// when escaping dynamic content for insertion into HTML templates.
///
/// # Examples
///
/// ```
/// use reinhardt_core::security::xss::escape_html_content;
///
/// let user_input = "<script>alert('XSS')</script>";
/// let safe = escape_html_content(user_input);
/// assert_eq!(safe, "&lt;script&gt;alert(&#x27;XSS&#x27;)&lt;/script&gt;");
///
/// // Safe for template insertion
/// let template = format!("<div>{}</div>", escape_html_content("User's <input>"));
/// assert!(!template.contains("<input>"));
/// ```
pub fn escape_html_content(input: &str) -> String {
	escape_html(input)
}

/// Escape a value for use in CSS selectors
///
/// Escapes CSS metacharacters to prevent selector injection attacks.
/// Based on the CSS.escape() specification.
///
/// # Examples
///
/// ```
/// use reinhardt_core::security::xss::escape_css_selector;
///
/// // Basic escaping
/// assert_eq!(escape_css_selector("my-class"), "my-class");
///
/// // Metacharacters are escaped
/// assert_eq!(escape_css_selector("a.b"), r"a\.b");
/// assert_eq!(escape_css_selector("a#b"), r"a\#b");
/// assert_eq!(escape_css_selector("a[0]"), r"a\[0\]");
///
/// // Special first-character handling
/// assert_eq!(escape_css_selector("-"), r"\-");
/// ```
pub fn escape_css_selector(input: &str) -> String {
	if input.is_empty() {
		return String::new();
	}

	let mut result = String::with_capacity(input.len() * 2);
	let chars: Vec<char> = input.chars().collect();

	for (i, &ch) in chars.iter().enumerate() {
		match ch {
			// Null character
			'\0' => result.push('\u{FFFD}'),
			// Control characters (U+0001 to U+001F, U+007F)
			'\u{0001}'..='\u{001F}' | '\u{007F}' => {
				result.push_str(&format!("\\{:x} ", ch as u32));
			}
			// If first character is a digit
			'0'..='9' if i == 0 => {
				result.push_str(&format!("\\{:x} ", ch as u32));
			}
			// If first character is a dash and it's the only character
			'-' if i == 0 && chars.len() == 1 => {
				result.push('\\');
				result.push(ch);
			}
			// CSS metacharacters that need escaping
			'!' | '"' | '#' | '$' | '%' | '&' | '\'' | '(' | ')' | '*' | '+' | ',' | '.' | '/'
			| ':' | ';' | '<' | '=' | '>' | '?' | '@' | '[' | '\\' | ']' | '^' | '`' | '{'
			| '|' | '}' | '~' => {
				result.push('\\');
				result.push(ch);
			}
			_ => result.push(ch),
		}
	}
	result
}

/// Validate that a string is safe to use as a CSS selector value
///
/// Returns `true` if the input contains no CSS metacharacters that could
/// cause selector injection.
///
/// # Examples
///
/// ```
/// use reinhardt_core::security::xss::validate_css_selector;
///
/// assert!(validate_css_selector("my-class"));
/// assert!(validate_css_selector("item_123"));
/// assert!(!validate_css_selector("a.b"));
/// assert!(!validate_css_selector("a[0]"));
/// assert!(!validate_css_selector("a{b}"));
/// ```
pub fn validate_css_selector(input: &str) -> bool {
	if input.is_empty() {
		return false;
	}

	input.chars().all(|ch| {
		matches!(ch,
			'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_'
		)
	})
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_escape_html() {
		assert_eq!(
			escape_html("<script>alert('xss')</script>"),
			"&lt;script&gt;alert(&#x27;xss&#x27;)&lt;/script&gt;"
		);
	}

	#[test]
	fn test_escape_html_attr() {
		let attr = r#"value" onload="alert('xss')"#;
		let escaped = escape_html_attr(attr);
		assert!(escaped.contains("&quot;"));
		assert!(escaped.contains("&#x27;"));
	}

	#[test]
	fn test_escape_javascript() {
		let script = "'; alert('xss'); var x='";
		let escaped = escape_javascript(script);
		// Verify that single quotes are escaped
		assert!(escaped.contains("\\'"));
		// Verify the actual output
		// Input: '; alert('xss'); var x='
		// Output: \\'; alert(\\'xss\\'); var x=\\'
		assert_eq!(escaped, "\\'; alert(\\'xss\\'); var x=\\'");
	}

	#[test]
	fn test_escape_url() {
		let url = "javascript:alert('xss')";
		let escaped = escape_url(url);
		assert!(escaped.contains("%3A"));
	}

	#[test]
	fn test_detect_xss_patterns() {
		assert!(detect_xss_patterns("<script>alert(1)</script>"));
		assert!(detect_xss_patterns(r#"<img src=x onerror="alert(1)">"#));
		assert!(detect_xss_patterns("javascript:alert(1)"));
		assert!(detect_xss_patterns("<iframe src='evil.com'>"));
		assert!(!detect_xss_patterns("Safe text"));
		assert!(!detect_xss_patterns("Normal <b>HTML</b>"));
	}

	#[test]
	fn test_is_safe_url() {
		assert!(is_safe_url("https://example.com"));
		assert!(is_safe_url("http://example.com"));
		assert!(is_safe_url("/path/to/page"));
		assert!(is_safe_url("./relative/path"));
		assert!(!is_safe_url("../parent/path")); // Path traversal is unsafe
		assert!(is_safe_url("#section")); // Anchor links are safe
		assert!(is_safe_url("mailto:user@example.com"));
		assert!(!is_safe_url("javascript:alert(1)"));
		assert!(!is_safe_url("data:text/html,<script>alert(1)</script>"));
		assert!(!is_safe_url("vbscript:alert(1)"));
	}

	#[test]
	fn test_sanitize_html() {
		let dangerous = "<script>alert('XSS')</script><b>Bold text</b>";
		let sanitized = sanitize_html(dangerous);
		assert_eq!(
			sanitized,
			"&lt;script&gt;alert(&#x27;XSS&#x27;)&lt;/script&gt;&lt;b&gt;Bold text&lt;/b&gt;"
		);
	}

	#[test]
	fn test_strip_tags_safe_basic() {
		assert_eq!(strip_tags_safe("<p>Hello <b>World</b></p>"), "Hello World");
		assert_eq!(strip_tags_safe("No tags here"), "No tags here");
		assert_eq!(strip_tags_safe(""), "");
	}

	#[test]
	fn test_strip_tags_safe_quoted_attributes() {
		// > inside double-quoted attribute
		assert_eq!(strip_tags_safe(r#"<a title="x>y">Link</a>"#), "Link");
		// > inside single-quoted attribute
		assert_eq!(strip_tags_safe("<a title='x>y'>Link</a>"), "Link");
		// Multiple quoted attributes with >
		assert_eq!(
			strip_tags_safe(r#"<a title="a>b" href="c>d">Text</a>"#),
			"Text"
		);
	}

	#[test]
	fn test_strip_tags_safe_html_comments() {
		assert_eq!(strip_tags_safe("Hello<!-- comment -->World"), "HelloWorld");
		assert_eq!(strip_tags_safe("A<!-- multi\nline -->B"), "AB");
		// Unclosed comment
		assert_eq!(strip_tags_safe("Hello<!-- unclosed"), "Hello");
	}

	#[test]
	fn test_strip_tags_safe_malformed() {
		// Unclosed tag at end
		assert_eq!(strip_tags_safe("Hello<br"), "Hello");
		assert_eq!(strip_tags_safe("Hello<"), "Hello");
		// Self-closing tags
		assert_eq!(strip_tags_safe("Hello<br/>World"), "HelloWorld");
	}

	#[test]
	fn test_escape_html_content() {
		assert_eq!(
			escape_html_content("<script>alert('XSS')</script>"),
			"&lt;script&gt;alert(&#x27;XSS&#x27;)&lt;/script&gt;"
		);
		assert_eq!(escape_html_content("safe text"), "safe text");
	}

	#[test]
	fn test_escape_css_selector_basic() {
		assert_eq!(escape_css_selector("my-class"), "my-class");
		assert_eq!(escape_css_selector("item_123"), "item_123");
		assert_eq!(escape_css_selector(""), "");
	}

	#[test]
	fn test_escape_css_selector_metacharacters() {
		assert_eq!(escape_css_selector("a.b"), r"a\.b");
		assert_eq!(escape_css_selector("a#b"), r"a\#b");
		assert_eq!(escape_css_selector("a[0]"), r"a\[0\]");
		assert_eq!(escape_css_selector("a{b}"), r"a\{b\}");
		assert_eq!(escape_css_selector("a:hover"), r"a\:hover");
	}

	#[test]
	fn test_escape_css_selector_first_char() {
		// Lone dash
		assert_eq!(escape_css_selector("-"), r"\-");
		// Digit as first character
		assert_eq!(escape_css_selector("1abc"), r"\31 abc");
	}

	#[test]
	fn test_escape_css_selector_null_and_control() {
		assert_eq!(escape_css_selector("\0"), "\u{FFFD}");
		assert_eq!(escape_css_selector("\u{0001}"), r"\1 ");
	}

	#[test]
	fn test_validate_css_selector() {
		assert!(validate_css_selector("my-class"));
		assert!(validate_css_selector("item_123"));
		assert!(validate_css_selector("CamelCase"));
		assert!(!validate_css_selector(""));
		assert!(!validate_css_selector("a.b"));
		assert!(!validate_css_selector("a[0]"));
		assert!(!validate_css_selector("a{b}"));
		assert!(!validate_css_selector("a b"));
	}
}
