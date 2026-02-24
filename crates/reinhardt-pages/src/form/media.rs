//! Media assets for form widgets
//!
//! This module provides CSS and JavaScript asset management for form widgets.
//! It allows forms to specify their required CSS and JavaScript resources.

/// Escapes special HTML characters to prevent XSS attacks.
///
/// This function converts the following characters to their HTML entity equivalents:
/// - `&` → `&amp;`
/// - `<` → `&lt;`
/// - `>` → `&gt;`
/// - `"` → `&quot;`
/// - `'` → `&#x27;`
fn html_escape(s: &str) -> String {
	s.replace('&', "&amp;")
		.replace('<', "&lt;")
		.replace('>', "&gt;")
		.replace('"', "&quot;")
		.replace('\'', "&#x27;")
}

/// Media assets for form widgets
///
/// Contains CSS and JavaScript resources required by form widgets.
#[derive(Debug, Clone, Default)]
pub struct Media {
	/// CSS resources
	css: Vec<String>,
	/// JavaScript resources
	js: Vec<String>,
}

impl Media {
	/// Create a new empty Media instance
	pub fn new() -> Self {
		Self::default()
	}

	/// Add a CSS resource
	pub fn add_css(&mut self, css: impl Into<String>) {
		self.css.push(css.into());
	}

	/// Add a JavaScript resource
	pub fn add_js(&mut self, js: impl Into<String>) {
		self.js.push(js.into());
	}

	/// Get CSS resources
	pub fn css(&self) -> &[String] {
		&self.css
	}

	/// Get JavaScript resources
	pub fn js(&self) -> &[String] {
		&self.js
	}

	/// Check if media is empty
	pub fn is_empty(&self) -> bool {
		self.css.is_empty() && self.js.is_empty()
	}

	/// Merge another Media instance into this one
	pub fn extend(&mut self, other: &Media) {
		self.css.extend(other.css.iter().cloned());
		self.js.extend(other.js.iter().cloned());
	}

	/// Render CSS link tags
	///
	/// All paths are HTML-escaped to prevent XSS attacks.
	pub fn render_css(&self) -> String {
		self.css
			.iter()
			.map(|path| format!(r#"<link rel="stylesheet" href="{}">"#, html_escape(path)))
			.collect::<Vec<_>>()
			.join("\n")
	}

	/// Render JavaScript script tags
	///
	/// All paths are HTML-escaped to prevent XSS attacks.
	pub fn render_js(&self) -> String {
		self.js
			.iter()
			.map(|path| format!(r#"<script src="{}"></script>"#, html_escape(path)))
			.collect::<Vec<_>>()
			.join("\n")
	}
}

/// Trait for widgets that define their own media
pub trait MediaDefiningWidget {
	/// Get the media required by this widget
	fn media(&self) -> Media;
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_media_creation() {
		let media = Media::new();
		assert!(media.is_empty());
	}

	#[test]
	fn test_media_add_resources() {
		let mut media = Media::new();
		media.add_css("/static/css/forms.css");
		media.add_js("/static/js/forms.js");

		assert!(!media.is_empty());
		assert_eq!(media.css(), &["/static/css/forms.css".to_string()]);
		assert_eq!(media.js(), &["/static/js/forms.js".to_string()]);
	}

	#[test]
	fn test_media_render_css() {
		let mut media = Media::new();
		media.add_css("/static/css/forms.css");
		media.add_css("/static/css/widgets.css");

		let rendered = media.render_css();
		assert!(rendered.contains(r#"<link rel="stylesheet" href="/static/css/forms.css">"#));
		assert!(rendered.contains(r#"<link rel="stylesheet" href="/static/css/widgets.css">"#));
	}

	#[test]
	fn test_media_render_js() {
		let mut media = Media::new();
		media.add_js("/static/js/forms.js");

		let rendered = media.render_js();
		assert_eq!(rendered, r#"<script src="/static/js/forms.js"></script>"#);
	}

	#[test]
	fn test_media_extend() {
		let mut media1 = Media::new();
		media1.add_css("/static/css/base.css");

		let mut media2 = Media::new();
		media2.add_css("/static/css/extra.css");
		media2.add_js("/static/js/extra.js");

		media1.extend(&media2);

		assert_eq!(media1.css().len(), 2);
		assert_eq!(media1.js().len(), 1);
	}

	// ============================================================================
	// XSS Prevention Tests (Issue #595)
	// ============================================================================

	#[test]
	fn test_html_escape_basic() {
		assert_eq!(html_escape("<script>"), "&lt;script&gt;");
		assert_eq!(html_escape("a & b"), "a &amp; b");
		assert_eq!(html_escape("\"quoted\""), "&quot;quoted&quot;");
		assert_eq!(html_escape("'single'"), "&#x27;single&#x27;");
	}

	#[test]
	fn test_media_render_css_escapes_path() {
		let mut media = Media::new();
		// Malicious path that could break out of the href attribute
		media.add_css("\"><script>alert('xss')</script>");

		let rendered = media.render_css();
		// Should NOT contain raw script tag
		assert!(!rendered.contains("<script>"));
		// Should contain escaped version
		assert!(rendered.contains("&lt;script&gt;"));
		assert!(rendered.contains("&quot;"));
	}

	#[test]
	fn test_media_render_js_escapes_path() {
		let mut media = Media::new();
		// Malicious path that could break out of the src attribute
		media.add_js("\"><script>alert('xss')</script>");

		let rendered = media.render_js();
		// Should NOT contain raw script tag
		assert!(!rendered.contains("<script>"));
		// Should contain escaped version
		assert!(rendered.contains("&lt;script&gt;"));
		assert!(rendered.contains("&quot;"));
	}

	#[test]
	fn test_media_render_normal_paths_preserved() {
		let mut media = Media::new();
		media.add_css("/static/css/forms.css");
		media.add_js("/static/js/forms.js");

		let css = media.render_css();
		let js = media.render_js();

		// Normal paths should work correctly
		assert!(css.contains(r#"href="/static/css/forms.css""#));
		assert!(js.contains(r#"src="/static/js/forms.js""#));
	}
}
