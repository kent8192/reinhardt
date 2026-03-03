use chrono::{DateTime, Utc};
use reinhardt::prelude::*;
use serde::{Deserialize, Serialize};

/// Snippet model representing a code snippet
#[derive(Serialize, Deserialize)]
#[model(app_label = "snippets", table_name = "snippets")]
pub struct Snippet {
	#[field(primary_key = true)]
	pub id: i64,

	#[field(max_length = 100)]
	pub title: String,

	#[field(max_length = 10000)]
	pub code: String,

	#[field(max_length = 50)]
	pub language: String,

	#[field(auto_now_add = true)]
	pub created_at: DateTime<Utc>,
}

impl Snippet {
	/// Get a highlighted version of the code using syntect
	///
	/// Returns HTML-formatted code with syntax highlighting based on the snippet's language.
	/// Falls back to plain text if the language is not recognized.
	///
	/// # Example
	///
	/// ```ignore
	/// let snippet = Snippet {
	///     id: 1,
	///     title: "Hello World".to_string(),
	///     code: "fn main() { println!(\"Hello!\"); }".to_string(),
	///     language: "rust".to_string(),
	///     created_at: Utc::now(),
	/// };
	/// let html = snippet.highlighted();
	/// assert!(html.contains("<span"));
	/// ```
	pub fn highlighted(&self) -> String {
		use syntect::highlighting::ThemeSet;
		use syntect::html::highlighted_html_for_string;
		use syntect::parsing::SyntaxSet;

		let ss = SyntaxSet::load_defaults_newlines();
		let ts = ThemeSet::load_defaults();

		// Try to find syntax by name first, then by extension
		let syntax = ss
			.find_syntax_by_name(&self.language)
			.or_else(|| ss.find_syntax_by_extension(&self.language))
			.unwrap_or_else(|| ss.find_syntax_plain_text());

		let theme = &ts.themes["base16-ocean.dark"];

		highlighted_html_for_string(&self.code, &ss, syntax, theme)
			.unwrap_or_else(|_| self.code.clone())
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	/// Test that Snippet can be created with valid data
	#[rstest]
	fn test_snippet_creation() {
		let snippet = Snippet {
			id: 1,
			title: "Hello World".to_string(),
			code: "fn main() { println!(\"Hello!\"); }".to_string(),
			language: "rust".to_string(),
			created_at: Utc::now(),
		};

		assert_eq!(snippet.id, 1);
		assert_eq!(snippet.title, "Hello World");
		assert_eq!(snippet.language, "rust");
	}

	/// Test highlighted() method produces HTML with syntax highlighting for Rust
	#[rstest]
	fn test_highlighted_rust_code() {
		let snippet = Snippet {
			id: 1,
			title: "Rust Example".to_string(),
			code: "fn main() { println!(\"Hello!\"); }".to_string(),
			language: "rust".to_string(),
			created_at: Utc::now(),
		};

		let html = snippet.highlighted();

		// Verify HTML output contains span tags (syntax highlighting)
		assert!(
			html.contains("<span"),
			"Expected HTML with <span> tags for syntax highlighting"
		);
		// Verify key elements are present in the output
		assert!(html.contains("fn"), "Expected 'fn' keyword in output");
		assert!(html.contains("main"), "Expected 'main' in output");
	}

	/// Test highlighted() method works for Python language
	#[rstest]
	fn test_highlighted_python_code() {
		let snippet = Snippet {
			id: 2,
			title: "Python Example".to_string(),
			code: "def hello():\n    print('Hello!')".to_string(),
			language: "python".to_string(),
			created_at: Utc::now(),
		};

		let html = snippet.highlighted();

		// Verify HTML output contains span tags
		assert!(
			html.contains("<span"),
			"Expected HTML with <span> tags for Python syntax highlighting"
		);
		assert!(html.contains("def"), "Expected 'def' keyword in output");
	}

	/// Test highlighted() method falls back to plain text for unknown languages
	#[rstest]
	fn test_highlighted_unknown_language() {
		let snippet = Snippet {
			id: 3,
			title: "Unknown Language".to_string(),
			code: "some unknown syntax here".to_string(),
			language: "unknown_lang_xyz".to_string(),
			created_at: Utc::now(),
		};

		let html = snippet.highlighted();

		// Should still produce output (plain text highlighting)
		assert!(
			!html.is_empty(),
			"Expected non-empty output for unknown language"
		);
		assert!(
			html.contains("some unknown syntax here"),
			"Expected original code to be present in output"
		);
	}

	/// Test highlighted() method handles empty code gracefully
	#[rstest]
	fn test_highlighted_empty_code() {
		let snippet = Snippet {
			id: 4,
			title: "Empty Code".to_string(),
			code: String::new(),
			language: "rust".to_string(),
			created_at: Utc::now(),
		};

		let html = snippet.highlighted();

		// Should not panic on empty code and should return valid (possibly empty) HTML
		// The test verifies the method completes successfully without panicking
		// NOTE: Empty code produces minimal HTML structure from syntect, exact output varies
		let _ = html; // Verifies method returns successfully
	}

	/// Test highlighted() method handles multiline code correctly
	#[rstest]
	fn test_highlighted_multiline_code() {
		let snippet = Snippet {
			id: 5,
			title: "Multiline Rust".to_string(),
			code: r#"fn main() {
    let x = 42;
    println!("{}", x);
}"#
			.to_string(),
			language: "rust".to_string(),
			created_at: Utc::now(),
		};

		let html = snippet.highlighted();

		// Verify HTML contains expected elements
		assert!(
			html.contains("<span"),
			"Expected span tags in multiline output"
		);
		assert!(html.contains("let"), "Expected 'let' keyword");
		assert!(html.contains("42"), "Expected number literal");
	}
}
