//! Development error pages with enhanced debugging information
//!
//! Provides detailed error pages during development with stack traces,
//! source code context, and helpful debugging information.

use reinhardt_core::security::xss::escape_html;
use std::backtrace::{Backtrace, BacktraceStatus};
use std::error::Error;
use std::fmt;

/// Development error handler
///
/// Generates detailed error pages with stack traces and debugging information.
/// Should only be enabled in development environments.
pub struct DevelopmentErrorHandler {
	/// Show full stack traces
	show_stack_trace: bool,
	/// Show source code context
	show_source: bool,
	/// Maximum lines of source code to show
	source_context_lines: usize,
}

impl DevelopmentErrorHandler {
	/// Create a new development error handler with default settings
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_utils::staticfiles::DevelopmentErrorHandler;
	///
	/// let handler = DevelopmentErrorHandler::new();
	/// ```
	pub fn new() -> Self {
		Self {
			show_stack_trace: true,
			show_source: true,
			source_context_lines: 5,
		}
	}

	/// Enable or disable stack trace display
	pub fn with_stack_trace(mut self, enable: bool) -> Self {
		self.show_stack_trace = enable;
		self
	}

	/// Enable or disable source code display
	pub fn with_source_context(mut self, enable: bool) -> Self {
		self.show_source = enable;
		self
	}

	/// Set the number of source context lines to show
	pub fn with_context_lines(mut self, lines: usize) -> Self {
		self.source_context_lines = lines;
		self
	}

	/// Format an error into an HTML error page
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_utils::staticfiles::DevelopmentErrorHandler;
	/// use std::io;
	///
	/// let handler = DevelopmentErrorHandler::new();
	/// let error = io::Error::new(io::ErrorKind::NotFound, "File not found");
	/// let html = handler.format_error(&error);
	/// assert!(html.contains("File not found"));
	/// ```
	pub fn format_error(&self, error: &dyn Error) -> String {
		let mut html = String::new();

		html.push_str("<!DOCTYPE html>\n");
		html.push_str("<html>\n");
		html.push_str("<head>\n");
		html.push_str("  <meta charset=\"utf-8\">\n");
		html.push_str("  <title>Development Error</title>\n");
		html.push_str("  <style>\n");
		html.push_str(&self.error_page_styles());
		html.push_str("  </style>\n");
		html.push_str("</head>\n");
		html.push_str("<body>\n");

		html.push_str("  <div class=\"error-container\">\n");
		html.push_str("    <h1>Development Error</h1>\n");

		// Main error message
		html.push_str("    <div class=\"error-message\">\n");
		html.push_str(&format!(
			"      <p><strong>Error:</strong> {}</p>\n",
			escape_html(&error.to_string())
		));
		html.push_str("    </div>\n");

		// Stack trace
		if self.show_stack_trace {
			html.push_str(&self.format_stack_trace(error));
		}

		// Error chain
		if let Some(source) = error.source() {
			html.push_str("    <div class=\"error-chain\">\n");
			html.push_str("      <h2>Caused by:</h2>\n");
			html.push_str("      <ul>\n");

			let mut current = Some(source);
			while let Some(err) = current {
				html.push_str(&format!(
					"        <li>{}</li>\n",
					escape_html(&err.to_string())
				));
				current = err.source();
			}

			html.push_str("      </ul>\n");
			html.push_str("    </div>\n");
		}

		html.push_str("  </div>\n");
		html.push_str("</body>\n");
		html.push_str("</html>\n");

		html
	}

	/// Format a simple error message (plain text)
	///
	/// # Examples
	///
	/// ```rust
	/// use reinhardt_utils::staticfiles::DevelopmentErrorHandler;
	/// use std::io;
	///
	/// let handler = DevelopmentErrorHandler::new();
	/// let error = io::Error::new(io::ErrorKind::NotFound, "File not found");
	/// let text = handler.format_error_text(&error);
	/// assert!(text.contains("Error: File not found"));
	/// ```
	pub fn format_error_text(&self, error: &dyn Error) -> String {
		let mut text = String::new();

		text.push_str("Development Error\n");
		text.push_str("================\n\n");
		text.push_str(&format!("Error: {}\n", error));

		if let Some(source) = error.source() {
			text.push_str("\nCaused by:\n");

			let mut current = Some(source);
			while let Some(err) = current {
				text.push_str(&format!("  - {}\n", err));
				current = err.source();
			}
		}

		text
	}

	fn format_stack_trace(&self, _error: &dyn Error) -> String {
		let mut html = String::new();

		html.push_str("    <div class=\"stack-trace\">\n");
		html.push_str("      <h2>Stack Trace</h2>\n");
		html.push_str("      <pre>\n");

		let backtrace = Backtrace::capture();
		if backtrace.status() == BacktraceStatus::Captured {
			html.push_str(&escape_html(&format!("{}\n", backtrace)));
		} else {
			html.push_str("Stack trace not available (compile with RUST_BACKTRACE=1)\n");
		}

		html.push_str("      </pre>\n");
		html.push_str("    </div>\n");

		html
	}

	fn error_page_styles(&self) -> String {
		r#"
    body {
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
      margin: 0;
      padding: 20px;
      background: #f5f5f5;
      color: #333;
    }
    .error-container {
      max-width: 1200px;
      margin: 0 auto;
      background: white;
      border-radius: 8px;
      box-shadow: 0 2px 8px rgba(0,0,0,0.1);
      padding: 40px;
    }
    h1 {
      color: #d32f2f;
      margin-top: 0;
      border-bottom: 2px solid #d32f2f;
      padding-bottom: 10px;
    }
    h2 {
      color: #555;
      margin-top: 30px;
      font-size: 1.2em;
    }
    .error-message {
      background: #ffebee;
      border-left: 4px solid #d32f2f;
      padding: 15px;
      margin: 20px 0;
    }
    .error-message p {
      margin: 0;
    }
    .error-chain {
      background: #fff3e0;
      border-left: 4px solid #ff9800;
      padding: 15px;
      margin: 20px 0;
    }
    .error-chain ul {
      margin: 10px 0 0 0;
      padding-left: 20px;
    }
    .error-chain li {
      margin: 5px 0;
    }
    .stack-trace {
      background: #f5f5f5;
      border: 1px solid #ddd;
      border-radius: 4px;
      padding: 15px;
      margin: 20px 0;
    }
    .stack-trace pre {
      margin: 10px 0 0 0;
      overflow-x: auto;
      font-family: "Courier New", monospace;
      font-size: 0.9em;
      line-height: 1.5;
    }
"#
        .to_string()
	}
}

impl Default for DevelopmentErrorHandler {
	fn default() -> Self {
		Self::new()
	}
}

/// Error type for development server operations
#[derive(Debug)]
pub struct DevServerError {
	message: String,
	source: Option<Box<dyn Error + Send + Sync>>,
}

impl DevServerError {
	/// Create a new development server error
	pub fn new(message: impl Into<String>) -> Self {
		Self {
			message: message.into(),
			source: None,
		}
	}

	/// Create an error with a source
	pub fn with_source(
		message: impl Into<String>,
		source: impl Error + Send + Sync + 'static,
	) -> Self {
		Self {
			message: message.into(),
			source: Some(Box::new(source)),
		}
	}
}

impl fmt::Display for DevServerError {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.message)
	}
}

impl Error for DevServerError {
	fn source(&self) -> Option<&(dyn Error + 'static)> {
		self.source
			.as_ref()
			.map(|e| e.as_ref() as &(dyn Error + 'static))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use std::io;

	#[test]
	fn test_new_handler() {
		let handler = DevelopmentErrorHandler::new();
		assert!(
			handler.show_stack_trace,
			"DevelopmentErrorHandler::new() should enable stack trace by default"
		);
		assert!(
			handler.show_source,
			"DevelopmentErrorHandler::new() should enable source context by default"
		);
		assert_eq!(
			handler.source_context_lines, 5,
			"DevelopmentErrorHandler::new() should set source_context_lines to 5 by default. Got: {}",
			handler.source_context_lines
		);
	}

	#[test]
	fn test_handler_builder() {
		let handler = DevelopmentErrorHandler::new()
			.with_stack_trace(false)
			.with_source_context(false)
			.with_context_lines(10);

		assert!(
			!handler.show_stack_trace,
			"with_stack_trace(false) should disable stack trace. Got: {}",
			handler.show_stack_trace
		);
		assert!(
			!handler.show_source,
			"with_source_context(false) should disable source context. Got: {}",
			handler.show_source
		);
		assert_eq!(
			handler.source_context_lines, 10,
			"with_context_lines(10) should set source_context_lines to 10. Got: {}",
			handler.source_context_lines
		);
	}

	#[test]
	fn test_format_error_html() {
		let handler = DevelopmentErrorHandler::new();
		let error = io::Error::new(io::ErrorKind::NotFound, "File not found");

		let html = handler.format_error(&error);

		// Verify HTML document structure
		assert!(
			html.starts_with("<!DOCTYPE html>\n"),
			"HTML output should start with DOCTYPE declaration. Got: {}",
			&html[..100.min(html.len())]
		);
		assert!(
			html.ends_with("</html>\n"),
			"HTML output should end with </html> tag. Got last 100 chars: {}",
			&html[html.len().saturating_sub(100)..]
		);

		// Strictly verify existence of important HTML elements
		assert!(
			html.contains("<title>Development Error</title>"),
			"HTML should contain <title> element with 'Development Error'. HTML head section: {}",
			html.split("</head>")
				.next()
				.unwrap_or("")
				.get(..500)
				.unwrap_or("")
		);
		assert!(
			html.contains("<h1>Development Error</h1>"),
			"HTML should contain <h1> element with 'Development Error'. HTML body: {}",
			html.split("<body>")
				.nth(1)
				.and_then(|s| s.get(..500))
				.unwrap_or("")
		);
		assert!(
			html.contains("<div class=\"error-message\">"),
			"HTML should contain error-message div. Error sections found: {:?}",
			html.match_indices("<div")
				.map(|(i, _)| &html[i..i.saturating_add(100).min(html.len())])
				.collect::<Vec<_>>()
		);

		// Accurately verify error message
		assert!(
			html.contains("<strong>Error:</strong> File not found"),
			"HTML should contain error message with 'File not found'. Error message section: {}",
			html.split("<div class=\"error-message\">")
				.nth(1)
				.and_then(|s| s.split("</div>").next())
				.unwrap_or("Error message div not found")
		);
	}

	#[test]
	fn test_format_error_text() {
		let handler = DevelopmentErrorHandler::new();
		let error = io::Error::new(io::ErrorKind::NotFound, "File not found");

		let text = handler.format_error_text(&error);

		// Accurately verify plain text structure
		assert!(
			text.starts_with("Development Error\n"),
			"Text output should start with 'Development Error\\n'. Got: {}",
			text.lines().next().unwrap_or("")
		);
		assert!(
			text.contains("================\n\n"),
			"Text output should contain separator line followed by blank line. Got first 100 chars: {}",
			&text[..100.min(text.len())]
		);

		// Accurately verify error message
		assert!(
			text.contains("Error: File not found\n"),
			"Text output should contain 'Error: File not found\\n'. Lines found: {:?}",
			text.lines().collect::<Vec<_>>()
		);
	}

	#[test]
	fn test_format_with_disabled_stack_trace() {
		let handler = DevelopmentErrorHandler::new().with_stack_trace(false);
		let error = io::Error::new(io::ErrorKind::NotFound, "File not found");

		let html = handler.format_error(&error);

		// Strictly verify that entire stack trace section does not exist
		assert!(
			!html.contains("<div class=\"stack-trace\">"),
			"HTML should NOT contain stack-trace div when stack trace is disabled. Div elements found: {:?}",
			html.match_indices("<div")
				.map(|(i, _)| &html[i..i.saturating_add(50).min(html.len())])
				.collect::<Vec<_>>()
		);
		assert!(
			!html.contains("<h2>Stack Trace</h2>"),
			"HTML should NOT contain 'Stack Trace' heading when disabled. H2 elements found: {:?}",
			html.match_indices("<h2>")
				.map(|(i, _)| &html[i..i.saturating_add(50).min(html.len())])
				.collect::<Vec<_>>()
		);
	}

	#[test]
	fn test_dev_server_error_new() {
		let error = DevServerError::new("Test error");
		assert_eq!(
			error.to_string(),
			"Test error",
			"DevServerError::to_string() should return the error message. Got: {}",
			error
		);
		assert!(
			error.source().is_none(),
			"DevServerError created with new() should not have a source error"
		);
	}

	#[test]
	fn test_dev_server_error_with_source() {
		let source = io::Error::new(io::ErrorKind::NotFound, "File not found");
		let error = DevServerError::with_source("Failed to load file", source);

		assert_eq!(
			error.to_string(),
			"Failed to load file",
			"DevServerError::to_string() should return the error message. Got: {}",
			error
		);
		assert!(
			error.source().is_some(),
			"DevServerError created with with_source() should have a source error"
		);
	}

	#[test]
	fn test_error_page_styles() {
		let handler = DevelopmentErrorHandler::new();
		let styles = handler.error_page_styles();

		// Verify accurate existence of CSS selectors
		assert!(
			styles.contains("body {"),
			"CSS should contain 'body' selector. Selectors found: {:?}",
			styles
				.match_indices('{')
				.filter_map(|(i, _)| {
					styles[..i]
						.rsplit_once('\n')
						.or_else(|| styles[..i].rsplit_once(' '))
						.map(|(_, selector)| selector.trim())
				})
				.collect::<Vec<_>>()
		);
		assert!(
			styles.contains(".error-container {"),
			"CSS should contain '.error-container' selector. Class selectors found: {:?}",
			styles
				.lines()
				.filter(|line| line.trim().starts_with('.'))
				.collect::<Vec<_>>()
		);
		assert!(
			styles.contains(".error-message {"),
			"CSS should contain '.error-message' selector. Class selectors found: {:?}",
			styles
				.lines()
				.filter(|line| line.trim().starts_with('.'))
				.collect::<Vec<_>>()
		);
		assert!(
			styles.contains(".error-chain {"),
			"CSS should contain '.error-chain' selector. Class selectors found: {:?}",
			styles
				.lines()
				.filter(|line| line.trim().starts_with('.'))
				.collect::<Vec<_>>()
		);
		assert!(
			styles.contains(".stack-trace {"),
			"CSS should contain '.stack-trace' selector. Class selectors found: {:?}",
			styles
				.lines()
				.filter(|line| line.trim().starts_with('.'))
				.collect::<Vec<_>>()
		);

		// Verify important style properties
		assert!(
			styles.contains("font-family:"),
			"CSS should contain font-family property. Properties found: {:?}",
			styles
				.lines()
				.filter(|line| line.contains(':'))
				.take(10)
				.collect::<Vec<_>>()
		);
		assert!(
			styles.contains("background:"),
			"CSS should contain background property. Properties found: {:?}",
			styles
				.lines()
				.filter(|line| line.contains(':'))
				.take(10)
				.collect::<Vec<_>>()
		);
	}

	#[test]
	fn test_default() {
		let handler = DevelopmentErrorHandler::default();
		assert!(
			handler.show_stack_trace,
			"DevelopmentErrorHandler::default() should enable stack trace by default"
		);
		assert!(
			handler.show_source,
			"DevelopmentErrorHandler::default() should enable source context by default"
		);
	}

	#[test]
	fn test_format_error_html_escapes_xss() {
		// Arrange
		let handler = DevelopmentErrorHandler::new().with_stack_trace(false);
		let xss_payload = "<script>alert('xss')</script>";
		let error = io::Error::new(io::ErrorKind::Other, xss_payload);

		// Act
		let html = handler.format_error(&error);

		// Assert
		assert!(
			!html.contains(xss_payload),
			"HTML output must not contain unescaped script tags. Found raw payload in: {}",
			html.split("<div class=\"error-message\">")
				.nth(1)
				.and_then(|s| s.split("</div>").next())
				.unwrap_or("error-message div not found")
		);
		assert!(
			html.contains("&lt;script&gt;"),
			"HTML output should contain escaped script tag. Error section: {}",
			html.split("<div class=\"error-message\">")
				.nth(1)
				.and_then(|s| s.split("</div>").next())
				.unwrap_or("error-message div not found")
		);
	}

	#[test]
	fn test_format_error_html_escapes_xss_in_error_chain() {
		// Arrange
		let handler = DevelopmentErrorHandler::new().with_stack_trace(false);
		let xss_payload = "<img src=x onerror=alert(1)>";
		let source = io::Error::new(io::ErrorKind::Other, xss_payload);
		let error = DevServerError::with_source("outer error", source);

		// Act
		let html = handler.format_error(&error);

		// Assert
		assert!(
			!html.contains(xss_payload),
			"HTML output must not contain unescaped XSS payload in error chain"
		);
		assert!(
			html.contains("&lt;img"),
			"HTML should contain escaped img tag in error chain"
		);
	}
}
