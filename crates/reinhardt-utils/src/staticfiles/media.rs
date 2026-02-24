//! Form media support for CSS and JavaScript assets
//!
//! Provides a way to declare and render CSS/JS dependencies for forms and widgets,
//! similar to Django's Media class.

use reinhardt_core::security::xss::escape_html_attr;

/// Media assets (CSS and JavaScript) for forms and widgets
///
/// Allows forms to declare their CSS and JavaScript dependencies,
/// which can then be rendered in templates.
///
/// # Example
///
/// ```rust
/// use reinhardt_utils::staticfiles::media::Media;
///
/// let mut media = Media::new();
/// media.add_css("all", "css/forms.css");
/// media.add_js("js/widgets.js");
///
/// let css_html = media.render_css();
/// let js_html = media.render_js();
/// ```
#[derive(Debug, Clone, Default)]
pub struct Media {
	/// CSS files organized by media type (e.g., "all", "screen", "print")
	css: std::collections::HashMap<String, Vec<String>>,
	/// JavaScript files
	js: Vec<String>,
}

impl Media {
	/// Create a new empty Media instance
	pub fn new() -> Self {
		Self::default()
	}

	/// Add a CSS file for a specific media type
	///
	/// # Arguments
	///
	/// * `media_type` - The media type (e.g., "all", "screen", "print")
	/// * `path` - Path to the CSS file
	///
	/// # Example
	///
	/// ```rust
	/// # use reinhardt_utils::staticfiles::media::Media;
	/// let mut media = Media::new();
	/// media.add_css("all", "css/style.css");
	/// media.add_css("print", "css/print.css");
	/// ```
	pub fn add_css(&mut self, media_type: impl Into<String>, path: impl Into<String>) {
		let media_type = media_type.into();
		let path = path.into();

		self.css.entry(media_type).or_default().push(path);
	}

	/// Add a JavaScript file
	///
	/// # Example
	///
	/// ```rust
	/// # use reinhardt_utils::staticfiles::media::Media;
	/// let mut media = Media::new();
	/// media.add_js("js/script.js");
	/// ```
	pub fn add_js(&mut self, path: impl Into<String>) {
		self.js.push(path.into());
	}

	/// Merge another Media instance into this one
	///
	/// Combines CSS and JS files from both instances, avoiding duplicates.
	///
	/// # Example
	///
	/// ```rust
	/// # use reinhardt_utils::staticfiles::media::Media;
	/// let mut media1 = Media::new();
	/// media1.add_css("all", "css/base.css");
	///
	/// let mut media2 = Media::new();
	/// media2.add_css("all", "css/forms.css");
	///
	/// media1.merge(&media2);
	/// ```
	pub fn merge(&mut self, other: &Media) {
		// Merge CSS
		for (media_type, files) in &other.css {
			let entry = self.css.entry(media_type.clone()).or_default();
			for file in files {
				if !entry.contains(file) {
					entry.push(file.clone());
				}
			}
		}

		// Merge JS (avoiding duplicates)
		for file in &other.js {
			if !self.js.contains(file) {
				self.js.push(file.clone());
			}
		}
	}

	/// Render CSS as HTML link tags
	///
	/// Returns HTML string with <link> tags for all CSS files.
	///
	/// # Example
	///
	/// ```rust
	/// # use reinhardt_utils::staticfiles::media::Media;
	/// let mut media = Media::new();
	/// media.add_css("all", "/static/css/style.css");
	///
	/// let html = media.render_css();
	/// assert!(html.contains("<link"));
	/// assert!(html.contains("media=\"all\""));
	/// ```
	pub fn render_css(&self) -> String {
		let mut output = String::new();

		// Sort media types for consistent output
		let mut media_types: Vec<_> = self.css.keys().collect();
		media_types.sort();

		for media_type in media_types {
			if let Some(files) = self.css.get(media_type.as_str()) {
				for file in files {
					output.push_str(&format!(
						"<link rel=\"stylesheet\" href=\"{}\" media=\"{}\">\n",
						escape_html_attr(file),
						escape_html_attr(media_type)
					));
				}
			}
		}

		output
	}

	/// Render JavaScript as HTML script tags
	///
	/// Returns HTML string with `<script>` tags for all JS files.
	///
	/// # Example
	///
	/// ```rust
	/// # use reinhardt_utils::staticfiles::media::Media;
	/// let mut media = Media::new();
	/// media.add_js("/static/js/app.js");
	///
	/// let html = media.render_js();
	/// assert!(html.contains("<script"));
	/// ```
	pub fn render_js(&self) -> String {
		let mut output = String::new();

		for file in &self.js {
			output.push_str(&format!(
				"<script src=\"{}\"></script>\n",
				escape_html_attr(file)
			));
		}

		output
	}

	/// Get all CSS files as a deduplicated list
	pub fn get_css_files(&self) -> Vec<(String, String)> {
		let mut files = Vec::new();
		for (media_type, paths) in &self.css {
			for path in paths {
				files.push((media_type.clone(), path.clone()));
			}
		}
		files
	}

	/// Get all JS files as a deduplicated list
	pub fn get_js_files(&self) -> Vec<String> {
		self.js.clone()
	}
}

/// Trait for types that can provide media assets
///
/// Implement this trait for forms, widgets, and other components
/// that need to declare CSS/JS dependencies.
///
/// # Example
///
/// ```rust
/// use reinhardt_utils::staticfiles::media::{Media, HasMedia};
///
/// struct DatePickerWidget;
///
/// impl HasMedia for DatePickerWidget {
///     fn media(&self) -> Media {
///         let mut media = Media::new();
///         media.add_css("all", "/static/css/datepicker.css");
///         media.add_js("/static/js/datepicker.js");
///         media
///     }
/// }
/// ```
pub trait HasMedia {
	/// Get the media assets for this component
	fn media(&self) -> Media;
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_add_css() {
		let mut media = Media::new();
		media.add_css("all", "css/style.css");
		media.add_css("print", "css/print.css");

		assert_eq!(media.css.get("all").unwrap()[0], "css/style.css");
		assert_eq!(media.css.get("print").unwrap()[0], "css/print.css");
	}

	#[test]
	fn test_add_js() {
		let mut media = Media::new();
		media.add_js("js/script1.js");
		media.add_js("js/script2.js");

		assert_eq!(media.js[0], "js/script1.js");
		assert_eq!(media.js[1], "js/script2.js");
	}

	#[test]
	fn test_merge_media() {
		let mut media1 = Media::new();
		media1.add_css("all", "css/base.css");
		media1.add_js("js/base.js");

		let mut media2 = Media::new();
		media2.add_css("all", "css/forms.css");
		media2.add_js("js/forms.js");

		media1.merge(&media2);

		assert_eq!(media1.css.get("all").unwrap().len(), 2);
		assert_eq!(media1.js.len(), 2);
	}

	#[test]
	fn test_merge_avoids_duplicates() {
		let mut media1 = Media::new();
		media1.add_css("all", "css/common.css");
		media1.add_js("js/common.js");

		let mut media2 = Media::new();
		media2.add_css("all", "css/common.css"); // Duplicate
		media2.add_js("js/common.js"); // Duplicate

		media1.merge(&media2);

		// Should still have only one of each
		assert_eq!(media1.css.get("all").unwrap().len(), 1);
		assert_eq!(media1.js.len(), 1);
	}

	#[test]
	fn test_render_css() {
		let mut media = Media::new();
		media.add_css("all", "/static/css/style.css");
		media.add_css("print", "/static/css/print.css");

		let html = media.render_css();

		assert!(html.contains("<link rel=\"stylesheet\""));
		assert!(html.contains("href=\"/static/css/style.css\""));
		assert!(html.contains("media=\"all\""));
		assert!(html.contains("href=\"/static/css/print.css\""));
		assert!(html.contains("media=\"print\""));
	}

	#[test]
	fn test_render_js() {
		let mut media = Media::new();
		media.add_js("/static/js/app.js");
		media.add_js("/static/js/widgets.js");

		let html = media.render_js();

		assert!(html.contains("<script src=\"/static/js/app.js\"></script>"));
		assert!(html.contains("<script src=\"/static/js/widgets.js\"></script>"));
	}

	#[test]
	fn test_has_media_trait() {
		struct TestWidget;

		impl HasMedia for TestWidget {
			fn media(&self) -> Media {
				let mut media = Media::new();
				media.add_css("all", "widget.css");
				media.add_js("widget.js");
				media
			}
		}

		let widget = TestWidget;
		let media = widget.media();

		assert_eq!(media.css.get("all").unwrap()[0], "widget.css");
		assert_eq!(media.js[0], "widget.js");
	}

	#[test]
	fn test_get_css_files() {
		let mut media = Media::new();
		media.add_css("all", "css/a.css");
		media.add_css("print", "css/b.css");

		let files = media.get_css_files();
		assert_eq!(files.len(), 2);
	}

	#[test]
	fn test_get_js_files() {
		let mut media = Media::new();
		media.add_js("js/a.js");
		media.add_js("js/b.js");

		let files = media.get_js_files();
		assert_eq!(files, vec!["js/a.js", "js/b.js"]);
	}

	#[test]
	fn test_render_css_escapes_xss_in_paths() {
		// Arrange
		let mut media = Media::new();
		media.add_css("all", "\"><script>alert(1)</script><link href=\"");

		// Act
		let html = media.render_css();

		// Assert
		assert!(
			!html.contains("<script>"),
			"CSS rendering must not contain unescaped script tags. Got: {}",
			html
		);
		assert!(
			html.contains("&quot;"),
			"CSS rendering should contain escaped quotes. Got: {}",
			html
		);
	}

	#[test]
	fn test_render_js_escapes_xss_in_paths() {
		// Arrange
		let mut media = Media::new();
		media.add_js("\"><script>alert(1)</script><script src=\"");

		// Act
		let html = media.render_js();

		// Assert
		assert!(
			!html.contains("<script>alert(1)</script>"),
			"JS rendering must not contain unescaped script tags. Got: {}",
			html
		);
		assert!(
			html.contains("&quot;"),
			"JS rendering should contain escaped quotes. Got: {}",
			html
		);
	}
}
