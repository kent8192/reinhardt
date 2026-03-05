//! XSS (Cross-Site Scripting) protection utilities
//!
//! This module provides XSS protection for user inputs including:
//! - HTML entity escaping (context-aware)
//! - Script tag removal via escaping
//! - Event handler neutralization via escaping
//! - Dangerous protocol filtering
//!
//! ## Security Approach
//!
//! Instead of regex-based tag stripping (which is bypassable), this module uses
//! HTML entity escaping as the primary defense. All potentially dangerous characters
//! are escaped to their HTML entity equivalents, making XSS payloads inert.
//!
//! ## Example
//!
//! ```
//! use reinhardt_middleware::xss::XssProtector;
//!
//! let protector = XssProtector::new();
//! let safe = protector.sanitize("<script>alert('xss')</script>Hello");
//! assert!(!safe.contains("<script>"));
//! assert!(safe.contains("Hello"));
//! ```

/// XSS protection error types
#[derive(Debug, thiserror::Error)]
pub enum XssError {
	/// Potentially dangerous content detected
	#[error("Potentially dangerous content detected: {0}")]
	DangerousContent(String),

	/// Sanitization failed
	#[error("Sanitization failed: {0}")]
	SanitizationFailed(String),
}

/// Context for HTML escaping
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EscapeContext {
	/// HTML body context (e.g., text between tags)
	HtmlBody,
	/// HTML attribute context (e.g., attribute values)
	HtmlAttribute,
}

/// Configuration for XSS protection
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct XssConfig {
	/// Allow specific HTML tags (only used in permissive mode)
	pub allowed_tags: Vec<String>,

	/// Allow specific attributes (only used in permissive mode)
	pub allowed_attributes: Vec<String>,

	/// Strip all tags (convert to text via escaping)
	pub strip_all_tags: bool,

	/// Escape HTML entities
	pub escape_html: bool,
}

impl Default for XssConfig {
	fn default() -> Self {
		Self {
			allowed_tags: vec![
				"b".to_string(),
				"i".to_string(),
				"u".to_string(),
				"p".to_string(),
			],
			allowed_attributes: vec!["class".to_string(), "id".to_string()],
			strip_all_tags: false,
			escape_html: true,
		}
	}
}

impl XssConfig {
	/// Create a strict configuration (no HTML allowed)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::xss::XssConfig;
	///
	/// let config = XssConfig::strict();
	/// assert!(config.strip_all_tags);
	/// assert!(config.escape_html);
	/// ```
	pub fn strict() -> Self {
		Self {
			allowed_tags: vec![],
			allowed_attributes: vec![],
			strip_all_tags: true,
			escape_html: true,
		}
	}

	/// Create a permissive configuration (safe HTML tags allowed)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::xss::XssConfig;
	///
	/// let config = XssConfig::permissive();
	/// assert!(!config.strip_all_tags);
	/// assert!(config.allowed_tags.contains(&"a".to_string()));
	/// ```
	pub fn permissive() -> Self {
		Self {
			allowed_tags: vec![
				"b".to_string(),
				"i".to_string(),
				"u".to_string(),
				"strong".to_string(),
				"em".to_string(),
				"p".to_string(),
				"br".to_string(),
				"a".to_string(),
				"ul".to_string(),
				"ol".to_string(),
				"li".to_string(),
			],
			allowed_attributes: vec![
				"class".to_string(),
				"id".to_string(),
				"href".to_string(),
				"title".to_string(),
			],
			strip_all_tags: false,
			escape_html: false,
		}
	}

	/// Allow specific HTML tags
	pub fn allow_tags(mut self, tags: Vec<String>) -> Self {
		self.allowed_tags = tags;
		self
	}

	/// Allow specific attributes
	pub fn allow_attributes(mut self, attributes: Vec<String>) -> Self {
		self.allowed_attributes = attributes;
		self
	}
}

/// XSS protector with configurable sanitization
///
/// Uses HTML entity escaping as the primary defense mechanism instead of
/// regex-based filtering, which is fundamentally bypassable for XSS.
pub struct XssProtector {
	config: XssConfig,
}

impl XssProtector {
	/// Create a new XSS protector with default configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::xss::XssProtector;
	///
	/// let protector = XssProtector::new();
	/// let safe = protector.sanitize("<script>alert('xss')</script>");
	/// assert!(!safe.contains("<script>"));
	/// ```
	pub fn new() -> Self {
		Self {
			config: XssConfig::default(),
		}
	}

	/// Create a protector with custom configuration
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::xss::{XssProtector, XssConfig};
	///
	/// let config = XssConfig::strict();
	/// let protector = XssProtector::with_config(config);
	/// ```
	pub fn with_config(config: XssConfig) -> Self {
		Self { config }
	}

	/// Create a strict protector (no HTML allowed)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::xss::XssProtector;
	///
	/// let protector = XssProtector::strict();
	/// let result = protector.sanitize("<b>Bold</b>");
	/// assert!(!result.contains("<b>"));
	/// ```
	pub fn strict() -> Self {
		Self {
			config: XssConfig::strict(),
		}
	}

	/// Sanitize input string
	///
	/// Uses HTML entity escaping to neutralize dangerous content.
	/// In strict mode, all HTML is escaped. In permissive mode, only
	/// allowed tags are preserved while dangerous content is escaped.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::xss::XssProtector;
	///
	/// let protector = XssProtector::new();
	/// let clean = protector.sanitize("<b>Bold</b><script>alert(1)</script>");
	/// assert!(clean.contains("<b>Bold</b>"));
	/// assert!(!clean.contains("<script>"));
	/// ```
	pub fn sanitize(&self, input: &str) -> String {
		if self.config.strip_all_tags {
			// Strict mode: escape everything
			escape_html_body(input)
		} else {
			// Permissive mode: selectively allow safe tags
			sanitize_with_allowlist(
				input,
				&self.config.allowed_tags,
				&self.config.allowed_attributes,
			)
		}
	}

	/// Check if input contains potential XSS
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::xss::XssProtector;
	///
	/// let protector = XssProtector::new();
	///
	/// assert!(protector.contains_xss("<script>alert(1)</script>"));
	/// assert!(protector.contains_xss(r#"<div onclick="alert(1)">Test</div>"#));
	/// assert!(!protector.contains_xss("<p>Safe content</p>"));
	/// ```
	pub fn contains_xss(&self, input: &str) -> bool {
		let lower = input.to_lowercase();

		// Detect script tags (including encoded variants)
		if lower.contains("<script") || lower.contains("&lt;script") {
			return true;
		}

		// Detect event handlers (on* attributes)
		if detect_event_handlers(&lower) {
			return true;
		}

		// Detect dangerous protocols
		if detect_dangerous_protocols(&lower) {
			return true;
		}

		// Detect SVG/MathML injection contexts
		if lower.contains("<svg") || lower.contains("<math") {
			return true;
		}

		false
	}

	/// Escape HTML for output in HTML body context
	///
	/// Escapes: `<`, `>`, `&`, `"`, `'`
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::xss::XssProtector;
	///
	/// let result = XssProtector::escape_for_html_body("<script>alert(1)</script>");
	/// assert_eq!(result, "&lt;script&gt;alert(1)&lt;/script&gt;");
	/// ```
	pub fn escape_for_html_body(input: &str) -> String {
		escape_html_body(input)
	}

	/// Escape HTML for output in attribute context
	///
	/// Escapes: `<`, `>`, `&`, `"`, `'`, `` ` ``
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_middleware::xss::XssProtector;
	///
	/// let result = XssProtector::escape_for_html_attribute(r#"" onclick="alert(1)"#);
	/// assert!(!result.contains('"'));
	/// ```
	pub fn escape_for_html_attribute(input: &str) -> String {
		escape_html_attribute(input)
	}
}

impl Default for XssProtector {
	fn default() -> Self {
		Self::new()
	}
}

/// Escape HTML entities for body context
///
/// Escapes the five critical characters: `<`, `>`, `&`, `"`, `'`
fn escape_html_body(input: &str) -> String {
	let mut output = String::with_capacity(input.len());
	for ch in input.chars() {
		match ch {
			'&' => output.push_str("&amp;"),
			'<' => output.push_str("&lt;"),
			'>' => output.push_str("&gt;"),
			'"' => output.push_str("&quot;"),
			'\'' => output.push_str("&#x27;"),
			_ => output.push(ch),
		}
	}
	output
}

/// Escape HTML entities for attribute context
///
/// Escapes the six critical characters: `<`, `>`, `&`, `"`, `'`, `` ` ``
fn escape_html_attribute(input: &str) -> String {
	let mut output = String::with_capacity(input.len());
	for ch in input.chars() {
		match ch {
			'&' => output.push_str("&amp;"),
			'<' => output.push_str("&lt;"),
			'>' => output.push_str("&gt;"),
			'"' => output.push_str("&quot;"),
			'\'' => output.push_str("&#x27;"),
			'`' => output.push_str("&#x60;"),
			_ => output.push(ch),
		}
	}
	output
}

/// Sanitize HTML with an allowlist of tags and attributes.
///
/// This is a simple allowlist-based sanitizer that:
/// 1. Parses through the input character by character
/// 2. Preserves allowed tags (with only allowed attributes)
/// 3. Escapes all other tags as HTML entities
/// 4. Removes dangerous protocols from href attributes
fn sanitize_with_allowlist(
	input: &str,
	allowed_tags: &[String],
	allowed_attributes: &[String],
) -> String {
	let mut output = String::with_capacity(input.len());
	let chars: Vec<char> = input.chars().collect();
	let len = chars.len();
	let mut i = 0;

	while i < len {
		if chars[i] == '<' {
			// Try to parse a tag
			if let Some((tag_str, end_idx)) = extract_tag(&chars, i) {
				let tag_lower = tag_str.to_lowercase();
				if let Some(tag_name) = parse_tag_name(&tag_lower)
					&& allowed_tags.iter().any(|t| t == &tag_name)
				{
					// Allowed tag: rebuild with only allowed attributes
					let is_closing = tag_lower.starts_with("</");
					let is_self_closing = tag_lower.ends_with("/>");
					if is_closing {
						output.push_str(&format!("</{}>", tag_name));
					} else {
						let attrs = filter_attributes(&tag_str, allowed_attributes, &tag_name);
						if is_self_closing {
							if attrs.is_empty() {
								output.push_str(&format!("<{} />", tag_name));
							} else {
								output.push_str(&format!("<{} {} />", tag_name, attrs));
							}
						} else if attrs.is_empty() {
							output.push_str(&format!("<{}>", tag_name));
						} else {
							output.push_str(&format!("<{} {}>", tag_name, attrs));
						}
					}
					i = end_idx + 1;
					continue;
				}
				// Not an allowed tag: escape it
				for ch in tag_str.chars() {
					match ch {
						'&' => output.push_str("&amp;"),
						'<' => output.push_str("&lt;"),
						'>' => output.push_str("&gt;"),
						'"' => output.push_str("&quot;"),
						'\'' => output.push_str("&#x27;"),
						_ => output.push(ch),
					}
				}
				i = end_idx + 1;
				continue;
			}
		}

		output.push(chars[i]);
		i += 1;
	}

	output
}

/// Extract a complete tag string from the character slice starting at position `start`.
/// Returns the tag string and the index of the closing `>`.
fn extract_tag(chars: &[char], start: usize) -> Option<(String, usize)> {
	if start >= chars.len() || chars[start] != '<' {
		return None;
	}

	let mut i = start;
	let mut in_quote = false;
	let mut quote_char = '"';

	while i < chars.len() {
		let ch = chars[i];

		if in_quote {
			if ch == quote_char {
				in_quote = false;
			}
		} else if ch == '"' || ch == '\'' {
			in_quote = true;
			quote_char = ch;
		} else if ch == '>' {
			let tag: String = chars[start..=i].iter().collect();
			return Some((tag, i));
		}

		i += 1;
	}

	None
}

/// Parse the tag name from a tag string (e.g., "<div class='foo'>" -> "div")
fn parse_tag_name(tag: &str) -> Option<String> {
	let trimmed = tag.trim_start_matches('<').trim_start_matches('/');
	let name: String = trimmed
		.chars()
		.take_while(|c| c.is_ascii_alphanumeric())
		.collect();
	if name.is_empty() { None } else { Some(name) }
}

/// Filter attributes from a tag, keeping only allowed ones.
/// Also removes dangerous protocols from href values.
fn filter_attributes(tag_str: &str, allowed_attributes: &[String], _tag_name: &str) -> String {
	let mut result = Vec::new();

	// Simple attribute extraction: look for name="value" or name='value' pairs
	let inner = tag_str
		.trim_start_matches('<')
		.trim_end_matches('>')
		.trim_end_matches('/');

	// Skip past the tag name
	let rest = inner
		.trim_start_matches('/')
		.trim_start_matches(|c: char| c.is_ascii_alphanumeric())
		.trim();

	if rest.is_empty() {
		return String::new();
	}

	// Parse attribute pairs
	let mut chars = rest.chars().peekable();
	while chars.peek().is_some() {
		// Skip whitespace
		while chars.peek().is_some_and(|c| c.is_whitespace()) {
			chars.next();
		}

		// Read attribute name
		let attr_name: String = chars
			.by_ref()
			.take_while(|c| *c != '=' && !c.is_whitespace())
			.collect();

		if attr_name.is_empty() {
			break;
		}

		// Skip whitespace and '='
		while chars.peek().is_some_and(|c| c.is_whitespace()) {
			chars.next();
		}

		if chars.peek() == Some(&'=') {
			chars.next(); // consume '='

			// Skip whitespace
			while chars.peek().is_some_and(|c| c.is_whitespace()) {
				chars.next();
			}

			// Read value
			let value = if chars.peek() == Some(&'"') || chars.peek() == Some(&'\'') {
				let quote = chars.next().unwrap();
				let val: String = chars.by_ref().take_while(|c| *c != quote).collect();
				val
			} else {
				let val: String = chars.by_ref().take_while(|c| !c.is_whitespace()).collect();
				val
			};

			let attr_lower = attr_name.to_lowercase();
			if allowed_attributes
				.iter()
				.any(|a| a.to_lowercase() == attr_lower)
			{
				// Check for dangerous protocols in href/src attributes
				if (attr_lower == "href" || attr_lower == "src") && has_dangerous_protocol(&value) {
					continue;
				}
				result.push(format!(
					"{}=\"{}\"",
					attr_lower,
					escape_html_attribute(&value)
				));
			}
		}
	}

	result.join(" ")
}

/// Check if a URL value contains a dangerous protocol
fn has_dangerous_protocol(value: &str) -> bool {
	let normalized = value
		.chars()
		.filter(|c| !c.is_whitespace() && *c != '\0')
		.collect::<String>()
		.to_lowercase();

	normalized.starts_with("javascript:")
		|| normalized.starts_with("vbscript:")
		|| normalized.starts_with("data:text/html")
		|| normalized.starts_with("data:application/xhtml")
}

/// Detect event handler attributes in lowercase input
fn detect_event_handlers(input: &str) -> bool {
	// Common event handlers that are XSS vectors
	let handlers = [
		"onclick",
		"onload",
		"onerror",
		"onmouseover",
		"onmouseout",
		"onmousedown",
		"onmouseup",
		"onfocus",
		"onblur",
		"onsubmit",
		"onchange",
		"oninput",
		"onkeydown",
		"onkeyup",
		"onkeypress",
		"ondblclick",
		"oncontextmenu",
		"ondrag",
		"ondrop",
		"onscroll",
		"onwheel",
		"oncopy",
		"oncut",
		"onpaste",
		"onanimationstart",
		"onanimationend",
		"ontransitionend",
		"onpointerdown",
		"ontouchstart",
	];

	for handler in &handlers {
		if input.contains(handler) {
			return true;
		}
	}
	false
}

/// Detect dangerous protocols in lowercase input
fn detect_dangerous_protocols(input: &str) -> bool {
	input.contains("javascript:")
		|| input.contains("vbscript:")
		|| input.contains("data:text/html")
		|| input.contains("data:application/xhtml")
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	// ============================================================
	// XssConfig tests
	// ============================================================

	#[rstest]
	fn test_xss_config_default() {
		// Arrange / Act
		let config = XssConfig::default();

		// Assert
		assert!(!config.strip_all_tags);
		assert!(config.escape_html);
		assert!(config.allowed_tags.contains(&"b".to_string()));
	}

	#[rstest]
	fn test_xss_config_strict() {
		// Arrange / Act
		let config = XssConfig::strict();

		// Assert
		assert!(config.strip_all_tags);
		assert!(config.escape_html);
		assert!(config.allowed_tags.is_empty());
	}

	#[rstest]
	fn test_xss_config_permissive() {
		// Arrange / Act
		let config = XssConfig::permissive();

		// Assert
		assert!(!config.strip_all_tags);
		assert!(config.allowed_tags.contains(&"a".to_string()));
		assert!(config.allowed_attributes.contains(&"href".to_string()));
	}

	// ============================================================
	// HTML body escaping tests
	// ============================================================

	#[rstest]
	#[case(
		"<script>alert('xss')</script>",
		"&lt;script&gt;alert(&#x27;xss&#x27;)&lt;/script&gt;"
	)]
	#[case("<img onerror=alert(1)>", "&lt;img onerror=alert(1)&gt;")]
	#[case("Hello & World", "Hello &amp; World")]
	#[case(r#"He said "hello""#, "He said &quot;hello&quot;")]
	fn test_escape_html_body(#[case] input: &str, #[case] expected: &str) {
		// Act
		let result = escape_html_body(input);

		// Assert
		assert_eq!(result, expected);
	}

	// ============================================================
	// HTML attribute escaping tests
	// ============================================================

	#[rstest]
	fn test_escape_html_attribute_backtick() {
		// Arrange
		let input = "`onmouseover=alert(1)";

		// Act
		let result = escape_html_attribute(input);

		// Assert
		assert!(result.contains("&#x60;"));
		assert!(!result.contains('`'));
	}

	// ============================================================
	// Strict mode sanitization tests
	// ============================================================

	#[rstest]
	fn test_strict_sanitize_script_tag() {
		// Arrange
		let protector = XssProtector::strict();

		// Act
		let result = protector.sanitize("<script>alert('xss')</script>Hello");

		// Assert
		assert!(!result.contains("<script>"));
		assert!(result.contains("Hello"));
		assert!(result.contains("&lt;script&gt;"));
	}

	#[rstest]
	fn test_strict_sanitize_event_handler() {
		// Arrange
		let protector = XssProtector::strict();

		// Act
		let result = protector.sanitize(r#"<img onerror="alert(1)">"#);

		// Assert
		assert!(!result.contains("<img"));
		assert!(result.contains("&lt;img"));
	}

	#[rstest]
	fn test_strict_sanitize_all_html() {
		// Arrange
		let protector = XssProtector::strict();

		// Act
		let result = protector.sanitize("<b>Bold</b><script>alert(1)</script>");

		// Assert
		assert!(!result.contains("<b>"));
		assert!(!result.contains("<script>"));
		assert!(result.contains("Bold"));
	}

	#[rstest]
	fn test_strict_sanitize_html_entities() {
		// Arrange
		let protector = XssProtector::strict();

		// Act
		let result = protector.sanitize(r#"<b>Test & "quotes"</b>"#);

		// Assert
		assert!(result.contains("&amp;"));
		assert!(result.contains("&quot;"));
	}

	// ============================================================
	// Default mode sanitization tests
	// ============================================================

	#[rstest]
	fn test_default_sanitize_preserves_allowed_tags() {
		// Arrange
		let protector = XssProtector::new();

		// Act
		let result = protector.sanitize("<b>Bold</b><script>alert(1)</script>");

		// Assert
		assert!(result.contains("<b>Bold</b>"));
		assert!(!result.contains("<script>"));
	}

	#[rstest]
	fn test_default_sanitize_escapes_disallowed_tags() {
		// Arrange
		let protector = XssProtector::new();

		// Act
		let result = protector.sanitize(r#"<div onclick="alert(1)">Click</div>"#);

		// Assert
		assert!(!result.contains("<div"));
		assert!(result.contains("&lt;div"));
	}

	#[rstest]
	fn test_default_sanitize_escapes_dangerous_protocols() {
		// Arrange
		let protector = XssProtector::new();

		// Act
		let result = protector.sanitize(r#"<a href="javascript:alert('xss')">Link</a>"#);

		// Assert
		// In default mode, <a> is not in allowed_tags, so the entire tag is escaped
		// The dangerous protocol cannot execute because the tag is escaped
		assert!(
			!result.contains("<a "),
			"Tag should be escaped, not rendered: {}",
			result
		);
	}

	#[rstest]
	fn test_permissive_sanitize_removes_dangerous_protocols() {
		// Arrange
		let config = XssConfig::permissive();
		let protector = XssProtector::with_config(config);

		// Act
		let result = protector.sanitize(r#"<a href="javascript:alert('xss')">Link</a>"#);

		// Assert
		// In permissive mode, <a> is allowed but dangerous href is stripped
		assert!(result.contains("<a>") || result.contains("<a "));
		assert!(
			!result.contains("javascript:"),
			"Dangerous protocol should be removed: {}",
			result
		);
	}

	// ============================================================
	// Permissive mode tests
	// ============================================================

	#[rstest]
	fn test_permissive_sanitize_preserves_safe_tags() {
		// Arrange
		let config = XssConfig::permissive();
		let protector = XssProtector::with_config(config);

		// Act
		let result = protector.sanitize("<b>Bold</b><p>Paragraph</p><script>alert(1)</script>");

		// Assert
		assert!(result.contains("<b>Bold</b>"));
		assert!(result.contains("<p>Paragraph</p>"));
		assert!(!result.contains("<script>"));
	}

	#[rstest]
	fn test_custom_allowed_tags() {
		// Arrange
		let config = XssConfig::default().allow_tags(vec!["b".to_string()]);
		let protector = XssProtector::with_config(config);

		// Act
		let result = protector.sanitize("<b>Bold</b><i>Italic</i>");

		// Assert
		assert!(result.contains("<b>Bold</b>"));
		assert!(!result.contains("<i>"));
	}

	// ============================================================
	// XSS detection tests
	// ============================================================

	#[rstest]
	fn test_contains_xss_detection() {
		// Arrange
		let protector = XssProtector::new();

		// Assert
		assert!(protector.contains_xss("<script>alert(1)</script>"));
		assert!(protector.contains_xss(r#"<div onclick="alert(1)">Test</div>"#));
		assert!(protector.contains_xss(r#"<a href="javascript:alert(1)">Link</a>"#));
		assert!(!protector.contains_xss("<p>Safe content</p>"));
	}

	// ============================================================
	// XSS bypass attempt tests
	// ============================================================

	#[rstest]
	#[case("<ScRiPt>alert(1)</ScRiPt>")]
	#[case("<SCRIPT>alert(1)</SCRIPT>")]
	#[case("<script >alert(1)</script>")]
	fn test_sanitize_mixed_case_script_tags(#[case] input: &str) {
		// Arrange
		let protector = XssProtector::strict();

		// Act
		let result = protector.sanitize(input);

		// Assert
		assert!(
			!result.contains("<script"),
			"Script tag should be escaped: {}",
			result
		);
		assert!(
			!result.contains("<Script"),
			"Script tag should be escaped: {}",
			result
		);
		assert!(
			!result.contains("<SCRIPT"),
			"Script tag should be escaped: {}",
			result
		);
	}

	#[rstest]
	fn test_sanitize_img_onerror() {
		// Arrange
		let protector = XssProtector::strict();

		// Act
		let result = protector.sanitize("<img src=x onerror=alert(1)>");

		// Assert
		assert!(
			!result.contains("<img"),
			"img tag should be escaped: {}",
			result
		);
	}

	#[rstest]
	fn test_sanitize_svg_injection() {
		// Arrange
		let protector = XssProtector::strict();

		// Act
		let result = protector.sanitize(r#"<svg onload="alert(1)">"#);

		// Assert
		assert!(
			!result.contains("<svg"),
			"SVG tag should be escaped: {}",
			result
		);
	}

	#[rstest]
	fn test_sanitize_data_uri_default_mode() {
		// Arrange
		let protector = XssProtector::new();

		// Act
		let result =
			protector.sanitize(r#"<a href="data:text/html,<script>alert(1)</script>">Click</a>"#);

		// Assert
		// In default mode, <a> is not allowed, so the entire tag is escaped
		assert!(!result.contains("<a "), "Tag should be escaped: {}", result);
	}

	#[rstest]
	fn test_sanitize_data_uri_permissive_mode() {
		// Arrange
		let config = XssConfig::permissive();
		let protector = XssProtector::with_config(config);

		// Act
		let result =
			protector.sanitize(r#"<a href="data:text/html,<script>alert(1)</script>">Click</a>"#);

		// Assert
		// In permissive mode, <a> is allowed but data: URI is dangerous and stripped
		assert!(
			!result.contains("data:text/html"),
			"Data URI should be removed: {}",
			result
		);
	}

	#[rstest]
	fn test_sanitize_unicode_in_strict_mode() {
		// Arrange
		let protector = XssProtector::strict();

		// Act
		let result = protector.sanitize("\u{003C}script\u{003E}alert(1)\u{003C}/script\u{003E}");

		// Assert
		assert!(
			!result.contains("<script"),
			"Unicode-encoded script tag should be escaped: {}",
			result
		);
	}

	#[rstest]
	fn test_sanitize_nested_tags() {
		// Arrange
		let protector = XssProtector::new();

		// Act
		let result = protector.sanitize("<div><script>alert(1)</script></div>");

		// Assert
		assert!(
			!result.contains("<script>"),
			"Nested script tags should be escaped"
		);
	}

	#[rstest]
	fn test_sanitize_multiple_event_handlers() {
		// Arrange
		let protector = XssProtector::strict();

		// Act
		let result = protector
			.sanitize(r#"<div onclick="alert(1)" onload="alert(2)" onerror="alert(3)">Test</div>"#);

		// Assert
		// In strict mode, all HTML is escaped so the tag cannot execute
		assert!(
			!result.contains("<div"),
			"HTML tag should be escaped: {}",
			result
		);
		assert!(result.contains("&lt;div"), "Tag should be entity-escaped");
	}

	// ============================================================
	// Static escape method tests
	// ============================================================

	#[rstest]
	fn test_escape_for_html_body_static() {
		// Act
		let result = XssProtector::escape_for_html_body("<script>alert(1)</script>");

		// Assert
		assert_eq!(result, "&lt;script&gt;alert(1)&lt;/script&gt;");
	}

	#[rstest]
	fn test_escape_for_html_attribute_static() {
		// Act
		let result = XssProtector::escape_for_html_attribute(r#"" onclick="alert(1)"#);

		// Assert
		assert!(!result.contains('"'));
		assert!(result.contains("&quot;"));
	}

	// ============================================================
	// Dangerous protocol detection tests
	// ============================================================

	#[rstest]
	#[case("javascript:alert(1)", true)]
	#[case("vbscript:msgbox", true)]
	#[case("data:text/html,<script>", true)]
	#[case("https://example.com", false)]
	#[case("safe text", false)]
	fn test_detect_dangerous_protocols(#[case] input: &str, #[case] expected: bool) {
		// Act
		let result = detect_dangerous_protocols(input);

		// Assert
		assert_eq!(result, expected);
	}

	// ============================================================
	// Event handler detection tests
	// ============================================================

	#[rstest]
	#[case("onclick", true)]
	#[case("onload", true)]
	#[case("onerror", true)]
	#[case("onmouseover", true)]
	#[case("safe text", false)]
	fn test_detect_event_handlers(#[case] input: &str, #[case] expected: bool) {
		// Act
		let result = detect_event_handlers(input);

		// Assert
		assert_eq!(result, expected);
	}

	// ============================================================
	// SVG/MathML context detection tests
	// ============================================================

	#[rstest]
	fn test_contains_xss_svg() {
		// Arrange
		let protector = XssProtector::new();

		// Assert
		assert!(protector.contains_xss(r#"<svg onload="alert(1)">"#));
	}

	#[rstest]
	fn test_contains_xss_math() {
		// Arrange
		let protector = XssProtector::new();

		// Assert
		assert!(protector.contains_xss(r#"<math><mi>test</mi></math>"#));
	}
}
