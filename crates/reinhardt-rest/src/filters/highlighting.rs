//! Search result highlighting
//!
//! Provides functionality to highlight search terms in text results.
//!
//! # Examples
//!
//! ```
//! use reinhardt_rest::filters::{SearchHighlighter, HtmlHighlighter, PlainTextHighlighter};
//!
//! // HTML highlighting
//! let html = HtmlHighlighter::new();
//! let result = html.highlight("The quick brown fox", "quick");
//! assert_eq!(result, "The <mark>quick</mark> brown fox");
//!
//! // Plain text highlighting
//! let plain = PlainTextHighlighter::new();
//! let result = plain.highlight("The quick brown fox", "quick");
//! assert_eq!(result, "The **quick** brown fox");
//! ```

use regex::{RegexBuilder, escape};
use serde::{Deserialize, Serialize};

/// Trait for search result highlighting
///
/// Implementations provide different highlighting strategies.
///
/// # Examples
///
/// ```
/// use reinhardt_rest::filters::{SearchHighlighter, HtmlHighlighter};
///
/// let highlighter = HtmlHighlighter::new();
/// let result = highlighter.highlight("Hello world", "world");
/// assert!(result.contains("<mark>"));
/// ```
pub trait SearchHighlighter {
	/// Highlight search terms in text
	///
	/// # Arguments
	///
	/// * `text` - The text to highlight
	/// * `query` - The search query to highlight
	///
	/// # Returns
	///
	/// The text with highlighted search terms
	fn highlight(&self, text: &str, query: &str) -> String;

	/// Highlight multiple terms in text
	///
	/// # Arguments
	///
	/// * `text` - The text to highlight
	/// * `queries` - Multiple search queries to highlight
	///
	/// # Returns
	///
	/// The text with all search terms highlighted
	fn highlight_many(&self, text: &str, queries: &[&str]) -> String {
		let mut result = text.to_string();
		for query in queries {
			result = self.highlight(&result, query);
		}
		result
	}
}

/// HTML highlighter using `<mark>` tags
///
/// # Examples
///
/// ```
/// use reinhardt_rest::filters::{SearchHighlighter, HtmlHighlighter};
///
/// let highlighter = HtmlHighlighter::new();
/// let result = highlighter.highlight("The quick brown fox", "quick");
/// assert_eq!(result, "The <mark>quick</mark> brown fox");
/// ```
#[derive(Debug, Clone)]
pub struct HtmlHighlighter {
	tag: String,
	case_sensitive: bool,
}

impl HtmlHighlighter {
	/// Create a new HTML highlighter with default `<mark>` tag
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::HtmlHighlighter;
	///
	/// let highlighter = HtmlHighlighter::new();
	/// ```
	pub fn new() -> Self {
		Self {
			tag: "mark".to_string(),
			case_sensitive: false,
		}
	}

	/// Set a custom HTML tag for highlighting
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::{SearchHighlighter, HtmlHighlighter};
	///
	/// let highlighter = HtmlHighlighter::new().with_tag("strong");
	/// let result = highlighter.highlight("Hello world", "world");
	/// assert_eq!(result, "Hello <strong>world</strong>");
	/// ```
	pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
		self.tag = tag.into();
		self
	}

	/// Enable case-sensitive highlighting
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::{SearchHighlighter, HtmlHighlighter};
	///
	/// let highlighter = HtmlHighlighter::new().case_sensitive(true);
	/// let result = highlighter.highlight("Hello World", "world");
	/// assert_eq!(result, "Hello World"); // No match due to case
	/// ```
	pub fn case_sensitive(mut self, enabled: bool) -> Self {
		self.case_sensitive = enabled;
		self
	}

	/// Escape HTML entities in text
	// Allow dead_code: utility method reserved for safe HTML output in highlight rendering
	#[allow(dead_code)]
	fn escape_html(&self, text: &str) -> String {
		reinhardt_core::security::escape_html(text)
	}
}

impl Default for HtmlHighlighter {
	fn default() -> Self {
		Self::new()
	}
}

impl SearchHighlighter for HtmlHighlighter {
	fn highlight(&self, text: &str, query: &str) -> String {
		if query.is_empty() {
			return text.to_string();
		}

		let escaped_query = escape(query);
		let regex = match RegexBuilder::new(&escaped_query)
			.case_insensitive(!self.case_sensitive)
			.build()
		{
			Ok(r) => r,
			Err(_) => return text.to_string(),
		};

		regex
			.replace_all(text, format!("<{}>$0</{}>", self.tag, self.tag))
			.to_string()
	}
}

/// Plain text highlighter using markdown-style emphasis
///
/// # Examples
///
/// ```
/// use reinhardt_rest::filters::{SearchHighlighter, PlainTextHighlighter};
///
/// let highlighter = PlainTextHighlighter::new();
/// let result = highlighter.highlight("The quick brown fox", "quick");
/// assert_eq!(result, "The **quick** brown fox");
/// ```
#[derive(Debug, Clone)]
pub struct PlainTextHighlighter {
	prefix: String,
	suffix: String,
	case_sensitive: bool,
}

impl PlainTextHighlighter {
	/// Create a new plain text highlighter with default `**` markers
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::PlainTextHighlighter;
	///
	/// let highlighter = PlainTextHighlighter::new();
	/// ```
	pub fn new() -> Self {
		Self {
			prefix: "**".to_string(),
			suffix: "**".to_string(),
			case_sensitive: false,
		}
	}

	/// Set custom prefix and suffix for highlighting
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::{SearchHighlighter, PlainTextHighlighter};
	///
	/// let highlighter = PlainTextHighlighter::new().with_markers(">>", "<<");
	/// let result = highlighter.highlight("Hello world", "world");
	/// assert_eq!(result, "Hello >>world<<");
	/// ```
	pub fn with_markers(mut self, prefix: impl Into<String>, suffix: impl Into<String>) -> Self {
		self.prefix = prefix.into();
		self.suffix = suffix.into();
		self
	}

	/// Enable case-sensitive highlighting
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::{SearchHighlighter, PlainTextHighlighter};
	///
	/// let highlighter = PlainTextHighlighter::new().case_sensitive(true);
	/// let result = highlighter.highlight("Hello World", "world");
	/// assert_eq!(result, "Hello World"); // No match due to case
	/// ```
	pub fn case_sensitive(mut self, enabled: bool) -> Self {
		self.case_sensitive = enabled;
		self
	}
}

impl Default for PlainTextHighlighter {
	fn default() -> Self {
		Self::new()
	}
}

impl SearchHighlighter for PlainTextHighlighter {
	fn highlight(&self, text: &str, query: &str) -> String {
		if query.is_empty() {
			return text.to_string();
		}

		let escaped_query = escape(query);
		let regex = match RegexBuilder::new(&escaped_query)
			.case_insensitive(!self.case_sensitive)
			.build()
		{
			Ok(r) => r,
			Err(_) => return text.to_string(),
		};

		regex
			.replace_all(text, format!("{}$0{}", self.prefix, self.suffix))
			.to_string()
	}
}

/// Highlighted search result
///
/// Contains both the original and highlighted versions of a field.
///
/// # Examples
///
/// ```
/// use reinhardt_rest::filters::HighlightedResult;
///
/// let result = HighlightedResult {
///     field: "title".to_string(),
///     original: "The Rust Programming Language".to_string(),
///     highlighted: "The <mark>Rust</mark> Programming Language".to_string(),
/// };
///
/// assert_eq!(result.field, "title");
/// assert!(result.highlighted.contains("<mark>"));
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HighlightedResult {
	/// The field name
	pub field: String,
	/// The original text
	pub original: String,
	/// The highlighted text
	pub highlighted: String,
}

impl HighlightedResult {
	/// Create a new highlighted result
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::HighlightedResult;
	///
	/// let result = HighlightedResult::new(
	///     "title",
	///     "Hello world",
	///     "Hello <mark>world</mark>"
	/// );
	///
	/// assert_eq!(result.field, "title");
	/// assert_eq!(result.original, "Hello world");
	/// assert_eq!(result.highlighted, "Hello <mark>world</mark>");
	/// ```
	pub fn new(
		field: impl Into<String>,
		original: impl Into<String>,
		highlighted: impl Into<String>,
	) -> Self {
		Self {
			field: field.into(),
			original: original.into(),
			highlighted: highlighted.into(),
		}
	}
}

/// Multi-field highlighter for search results
///
/// Highlights search terms across multiple fields in a document.
///
/// # Examples
///
/// ```
/// use reinhardt_rest::filters::{MultiFieldHighlighter, HtmlHighlighter};
/// use std::collections::HashMap;
///
/// let highlighter = MultiFieldHighlighter::new(Box::new(HtmlHighlighter::new()));
///
/// let mut fields = HashMap::new();
/// fields.insert("title".to_string(), "The Rust Programming Language".to_string());
/// fields.insert("content".to_string(), "Rust is a systems programming language".to_string());
///
/// let results = highlighter.highlight_fields(&fields, "Rust");
/// assert_eq!(results.len(), 2);
/// ```
pub struct MultiFieldHighlighter {
	highlighter: Box<dyn SearchHighlighter + Send + Sync>,
}

impl MultiFieldHighlighter {
	/// Create a new multi-field highlighter
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::{MultiFieldHighlighter, HtmlHighlighter};
	///
	/// let highlighter = MultiFieldHighlighter::new(Box::new(HtmlHighlighter::new()));
	/// ```
	pub fn new(highlighter: Box<dyn SearchHighlighter + Send + Sync>) -> Self {
		Self { highlighter }
	}

	/// Highlight a search query across multiple fields
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_rest::filters::{MultiFieldHighlighter, HtmlHighlighter};
	/// use std::collections::HashMap;
	///
	/// let highlighter = MultiFieldHighlighter::new(Box::new(HtmlHighlighter::new()));
	///
	/// let mut fields = HashMap::new();
	/// fields.insert("title".to_string(), "Hello world".to_string());
	///
	/// let results = highlighter.highlight_fields(&fields, "world");
	/// assert_eq!(results.len(), 1);
	/// assert!(results[0].highlighted.contains("<mark>"));
	/// ```
	pub fn highlight_fields(
		&self,
		fields: &std::collections::HashMap<String, String>,
		query: &str,
	) -> Vec<HighlightedResult> {
		fields
			.iter()
			.map(|(field, text)| {
				let highlighted = self.highlighter.highlight(text, query);
				HighlightedResult::new(field, text, highlighted)
			})
			.collect()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::collections::HashMap;

	#[test]
	fn test_html_highlighter_basic() {
		let highlighter = HtmlHighlighter::new();
		let result = highlighter.highlight("The quick brown fox", "quick");
		assert_eq!(result, "The <mark>quick</mark> brown fox");
	}

	#[test]
	fn test_html_highlighter_custom_tag() {
		let highlighter = HtmlHighlighter::new().with_tag("strong");
		let result = highlighter.highlight("Hello world", "world");
		assert_eq!(result, "Hello <strong>world</strong>");
	}

	#[test]
	fn test_html_highlighter_case_insensitive() {
		let highlighter = HtmlHighlighter::new();
		let result = highlighter.highlight("Hello World", "world");
		assert_eq!(result, "Hello <mark>World</mark>");
	}

	#[test]
	fn test_html_highlighter_case_sensitive() {
		let highlighter = HtmlHighlighter::new().case_sensitive(true);
		let result = highlighter.highlight("Hello World", "world");
		assert_eq!(result, "Hello World");
	}

	#[test]
	fn test_html_highlighter_empty_query() {
		let highlighter = HtmlHighlighter::new();
		let result = highlighter.highlight("Hello world", "");
		assert_eq!(result, "Hello world");
	}

	#[test]
	fn test_html_highlighter_multiple_occurrences() {
		let highlighter = HtmlHighlighter::new();
		let result = highlighter.highlight("rust rust rust", "rust");
		assert_eq!(
			result,
			"<mark>rust</mark> <mark>rust</mark> <mark>rust</mark>"
		);
	}

	#[test]
	fn test_plain_text_highlighter_basic() {
		let highlighter = PlainTextHighlighter::new();
		let result = highlighter.highlight("The quick brown fox", "quick");
		assert_eq!(result, "The **quick** brown fox");
	}

	#[test]
	fn test_plain_text_highlighter_custom_markers() {
		let highlighter = PlainTextHighlighter::new().with_markers(">>", "<<");
		let result = highlighter.highlight("Hello world", "world");
		assert_eq!(result, "Hello >>world<<");
	}

	#[test]
	fn test_plain_text_highlighter_case_insensitive() {
		let highlighter = PlainTextHighlighter::new();
		let result = highlighter.highlight("Hello World", "world");
		assert_eq!(result, "Hello **World**");
	}

	#[test]
	fn test_plain_text_highlighter_case_sensitive() {
		let highlighter = PlainTextHighlighter::new().case_sensitive(true);
		let result = highlighter.highlight("Hello World", "world");
		assert_eq!(result, "Hello World");
	}

	#[test]
	fn test_plain_text_highlighter_empty_query() {
		let highlighter = PlainTextHighlighter::new();
		let result = highlighter.highlight("Hello world", "");
		assert_eq!(result, "Hello world");
	}

	#[test]
	fn test_highlighted_result_creation() {
		let result = HighlightedResult::new("title", "Hello world", "Hello <mark>world</mark>");

		assert_eq!(result.field, "title");
		assert_eq!(result.original, "Hello world");
		assert_eq!(result.highlighted, "Hello <mark>world</mark>");
	}

	#[test]
	fn test_multi_field_highlighter() {
		let highlighter = MultiFieldHighlighter::new(Box::new(HtmlHighlighter::new()));

		let mut fields = HashMap::new();
		fields.insert("title".to_string(), "The Rust Book".to_string());
		fields.insert(
			"content".to_string(),
			"Rust is a systems programming language".to_string(),
		);

		let results = highlighter.highlight_fields(&fields, "Rust");

		assert_eq!(results.len(), 2);
		assert!(
			results
				.iter()
				.all(|r| r.highlighted.contains("<mark>Rust</mark>"))
		);
	}

	#[test]
	fn test_highlight_many() {
		let highlighter = HtmlHighlighter::new();
		let result = highlighter.highlight_many("The quick brown fox jumps", &["quick", "fox"]);

		assert!(result.contains("<mark>quick</mark>"));
		assert!(result.contains("<mark>fox</mark>"));
	}

	#[test]
	fn test_highlight_with_special_characters() {
		let highlighter = HtmlHighlighter::new();
		let result = highlighter.highlight("Price: $100", "$100");

		assert!(result.contains("<mark>$100</mark>"));
	}
}
