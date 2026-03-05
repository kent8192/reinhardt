//! SSR Renderer for Component-based server-side rendering.

use super::markers::{HydrationMarker, HydrationStrategy};
use super::state::SsrState;
use crate::auth::AuthData;
use crate::component::{Component, Head, IntoPage, Page};

/// Options for SSR rendering.
#[derive(Debug, Clone)]
pub struct SsrOptions {
	/// Whether to include hydration markers.
	pub include_hydration_markers: bool,
	/// Whether to minify the output.
	pub minify: bool,
	/// Whether to include SSR state script.
	pub include_state_script: bool,
	/// Language attribute for HTML element.
	pub lang: String,
	/// CSRF token to embed.
	pub csrf_token: Option<String>,
	/// Authentication data to embed.
	pub auth_data: Option<AuthData>,
	/// Enable partial hydration (Island Architecture, Phase 2-B).
	///
	/// When enabled, only components marked as islands are hydrated on the client.
	/// Static content is preserved without hydration, improving performance.
	pub enable_partial_hydration: bool,
	/// Default hydration strategy for components (Phase 2-B).
	///
	/// Determines how unmarked components should be hydrated.
	/// - `Full`: Traditional full hydration (default)
	/// - `Island`: Mark as interactive islands
	/// - `Static`: Mark as static content (no hydration)
	pub default_hydration_strategy: HydrationStrategy,
}

impl Default for SsrOptions {
	fn default() -> Self {
		Self {
			include_hydration_markers: true,
			minify: false,
			include_state_script: true,
			lang: "en".to_string(),
			csrf_token: None,
			auth_data: None,
			enable_partial_hydration: false,
			default_hydration_strategy: HydrationStrategy::Full,
		}
	}
}

impl SsrOptions {
	/// Creates new default options.
	pub fn new() -> Self {
		Self::default()
	}

	/// Sets the language.
	pub fn lang(mut self, lang: impl Into<String>) -> Self {
		self.lang = lang.into();
		self
	}

	/// Disables hydration markers.
	pub fn no_hydration(mut self) -> Self {
		self.include_hydration_markers = false;
		self
	}

	/// Enables minification.
	pub fn minify(mut self) -> Self {
		self.minify = true;
		self
	}

	/// Sets the CSRF token.
	pub fn csrf(mut self, token: impl Into<String>) -> Self {
		self.csrf_token = Some(token.into());
		self
	}

	/// Sets the authentication data.
	pub fn auth(mut self, auth_data: AuthData) -> Self {
		self.auth_data = Some(auth_data);
		self
	}

	/// Enables partial hydration (Island Architecture, Phase 2-B).
	///
	/// When enabled, only components marked as islands will be hydrated on the client.
	/// Static content is preserved without hydration, improving performance.
	///
	/// # Example
	///
	/// ```ignore
	/// let options = SsrOptions::new()
	///     .partial_hydration(true)
	///     .default_strategy(HydrationStrategy::Static);
	/// ```
	pub fn partial_hydration(mut self, enable: bool) -> Self {
		self.enable_partial_hydration = enable;
		self
	}

	/// Sets the default hydration strategy (Phase 2-B).
	///
	/// Determines how unmarked components should be hydrated:
	/// - `Full`: Traditional full hydration (default)
	/// - `Island`: Mark as interactive islands
	/// - `Static`: Mark as static content (no hydration)
	///
	/// # Example
	///
	/// ```ignore
	/// let options = SsrOptions::new()
	///     .default_strategy(HydrationStrategy::Island);
	/// ```
	pub fn default_strategy(mut self, strategy: HydrationStrategy) -> Self {
		self.default_hydration_strategy = strategy;
		self
	}

	/// Enables island-only rendering (convenience method, Phase 2-B).
	///
	/// Shortcut for enabling partial hydration with island strategy.
	/// Equivalent to:
	/// ```ignore
	/// options.partial_hydration(true).default_strategy(HydrationStrategy::Island)
	/// ```
	pub fn islands_only(mut self) -> Self {
		self.enable_partial_hydration = true;
		self.default_hydration_strategy = HydrationStrategy::Island;
		self
	}
}

/// The main SSR renderer.
pub struct SsrRenderer {
	options: SsrOptions,
	state: SsrState,
}

impl Default for SsrRenderer {
	fn default() -> Self {
		Self::new()
	}
}

impl SsrRenderer {
	/// Creates a new renderer with default options.
	pub fn new() -> Self {
		Self {
			options: SsrOptions::default(),
			state: SsrState::new(),
		}
	}

	/// Creates a renderer with custom options.
	pub fn with_options(options: SsrOptions) -> Self {
		Self {
			options,
			state: SsrState::new(),
		}
	}

	/// Returns a reference to the SSR state.
	pub fn state(&self) -> &SsrState {
		&self.state
	}

	/// Returns a mutable reference to the SSR state.
	pub fn state_mut(&mut self) -> &mut SsrState {
		&mut self.state
	}

	/// Renders a component to an HTML string.
	pub fn render<C: Component>(&mut self, component: &C) -> String {
		let view = component.render();
		self.render_view(&view)
	}

	/// Renders an IntoPage to an HTML string.
	pub fn render_into_page<V: IntoPage>(&mut self, view: V) -> String {
		let view = view.into_page();
		self.render_view(&view)
	}

	/// Renders a View to an HTML string.
	pub fn render_view(&self, view: &Page) -> String {
		view.render_to_string()
	}

	/// Renders a component to a full HTML page.
	pub fn render_page<C: Component>(&mut self, component: &C) -> String {
		let content = self.render(component);
		self.wrap_in_html(&content)
	}

	/// Renders an IntoPage to a full HTML page.
	pub fn render_page_into_page<V: IntoPage>(&mut self, view: V) -> String {
		let content = self.render_into_page(view);
		self.wrap_in_html(&content)
	}

	/// Renders a View to a full HTML page, using the View's attached head if present.
	///
	/// This method extracts any `Head` attached to the View using `find_topmost_head()`,
	/// and uses it to render the HTML `<head>` section. If no head is attached,
	/// it falls back to the head settings from `SsrOptions`.
	///
	/// # Arguments
	///
	/// * `view` - The View to render, potentially with an attached Head
	///
	/// # Example
	///
	/// ```ignore
	/// use reinhardt_pages::{head, page, View, SsrRenderer};
	///
	/// let my_head = head!(|| {
	///     title { "My Page" }
	///     meta { name: "description", content: "A page" }
	/// });
	///
	/// let view = page!(|| { div { "Hello" } })().with_head(my_head);
	///
	/// let mut renderer = SsrRenderer::new();
	/// let html = renderer.render_page_with_view_head(view);
	/// // html contains <title>My Page</title> in the head
	/// ```
	pub fn render_page_with_view_head(&mut self, view: Page) -> String {
		// Extract head from the view tree
		let view_head = view.find_topmost_head().cloned();

		// Render the view content
		let content = self.render_view(&view);

		// Wrap in HTML using the extracted head
		self.wrap_in_html_with_head(&content, view_head.as_ref())
	}

	/// Wraps content in a full HTML document with View's head elements.
	///
	/// Head elements (title, meta tags, CSS links, JS scripts) are sourced
	/// from the View's attached Head. Use the `head!` macro to define
	/// head elements.
	///
	/// # Arguments
	///
	/// * `content` - The rendered body content
	/// * `view_head` - Optional head extracted from a View
	fn wrap_in_html_with_head(&self, content: &str, view_head: Option<&Head>) -> String {
		let mut html = String::with_capacity(content.len() + 1024);

		// DOCTYPE and html opening
		html.push_str("<!DOCTYPE html>\n");
		html.push_str(&format!("<html lang=\"{}\">\n", self.options.lang));

		// Head section
		html.push_str("<head>\n");
		html.push_str("<meta charset=\"UTF-8\">\n");
		html.push_str(
			"<meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n",
		);

		// Add View's head elements
		if let Some(head) = view_head {
			// Title from View's head
			if let Some(ref title) = head.title {
				html.push_str(&format!("<title>{}</title>\n", html_escape(title)));
			}

			// View's meta tags
			for meta in &head.meta_tags {
				html.push_str(&meta.to_html());
			}

			// View's links
			for link in &head.links {
				html.push_str(&link.to_html());
			}

			// View's styles
			for style in &head.styles {
				html.push_str(&style.to_html());
			}

			// View's scripts (in head)
			for script in &head.scripts {
				html.push_str(&script.to_html());
			}
		}

		// CSRF token meta tag (always from options)
		if let Some(ref token) = self.options.csrf_token {
			html.push_str(&format!(
				"<meta name=\"csrf-token\" content=\"{}\">\n",
				html_escape(token)
			));
		}

		html.push_str("</head>\n");

		// Body section
		html.push_str("<body>\n");
		html.push_str("<div id=\"app\">");
		html.push_str(content);
		html.push_str("</div>\n");

		// Auth data script (if provided)
		// Note: We escape </script> sequences to prevent XSS attacks where
		// user-controlled data (like username) could break out of the script context
		if let Some(ref auth_data) = self.options.auth_data
			&& let Ok(json) = serde_json::to_string(auth_data)
		{
			let safe_json = escape_json_for_script(&json);
			html.push_str(&format!(
				"<script id=\"auth-data\" type=\"application/json\">{}</script>\n",
				safe_json
			));
		}

		// SSR state script (if enabled)
		if self.options.include_state_script && !self.state.is_empty() {
			html.push_str(&self.state.to_script_tag());
			html.push('\n');
		}

		html.push_str("</body>\n");
		html.push_str("</html>");

		if self.options.minify {
			minify_html(&html)
		} else {
			html
		}
	}

	/// Wraps content in a full HTML document.
	///
	/// This method creates a minimal HTML document without head elements.
	/// Use `render_page_with_view_head` with the `head!` macro for pages
	/// that require title, meta tags, CSS, or JS.
	pub fn wrap_in_html(&self, content: &str) -> String {
		let mut html = String::with_capacity(content.len() + 1024);

		// DOCTYPE and html opening
		html.push_str("<!DOCTYPE html>\n");
		html.push_str(&format!("<html lang=\"{}\">\n", self.options.lang));

		// Head section
		html.push_str("<head>\n");
		html.push_str("<meta charset=\"UTF-8\">\n");
		html.push_str(
			"<meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">\n",
		);

		// CSRF token meta tag
		if let Some(ref token) = self.options.csrf_token {
			html.push_str(&format!(
				"<meta name=\"csrf-token\" content=\"{}\">\n",
				html_escape(token)
			));
		}

		html.push_str("</head>\n");

		// Body section
		html.push_str("<body>\n");

		// Main content
		html.push_str("<div id=\"app\">");
		html.push_str(content);
		html.push_str("</div>\n");

		// Auth data script (if provided)
		// Note: We escape </script> sequences to prevent XSS attacks where
		// user-controlled data (like username) could break out of the script context
		if let Some(ref auth_data) = self.options.auth_data
			&& let Ok(json) = serde_json::to_string(auth_data)
		{
			let safe_json = escape_json_for_script(&json);
			html.push_str(&format!(
				"<script id=\"auth-data\" type=\"application/json\">{}</script>\n",
				safe_json
			));
		}

		// SSR state script (if enabled)
		if self.options.include_state_script && !self.state.is_empty() {
			html.push_str(&self.state.to_script_tag());
			html.push('\n');
		}

		html.push_str("</body>\n");
		html.push_str("</html>");

		if self.options.minify {
			minify_html(&html)
		} else {
			html
		}
	}

	/// Renders a component with hydration marker.
	pub fn render_with_marker<C: Component>(&mut self, component: &C) -> String {
		let marker = HydrationMarker::with_component(C::name());
		let view = component.render();
		let content = view.render_to_string();

		if self.options.include_hydration_markers {
			format!("<div {}>{}</div>", marker.to_attr_string(), content)
		} else {
			content
		}
	}
}

/// Simple HTML escape function.
fn html_escape(s: &str) -> String {
	s.replace('&', "&amp;")
		.replace('<', "&lt;")
		.replace('>', "&gt;")
		.replace('"', "&quot;")
		.replace('\'', "&#x27;")
}

/// Escapes JSON content for safe embedding in HTML script tags.
///
/// This function prevents XSS attacks by escaping `</script>` sequences
/// that could break out of the script context. The escaping is done by
/// replacing `</` with `<\/`, which is safe because:
/// 1. JavaScript string literals interpret `<\/` as `</`
/// 2. HTML parsers don't recognize `<\/script>` as a closing tag
///
/// # Security Note
///
/// When embedding JSON data in `<script>` tags, the `</script>` sequence
/// must be escaped because HTML parsers don't understand JavaScript string
/// context - they will see `</script>` and close the tag, allowing XSS.
fn escape_json_for_script(json: &str) -> String {
	json.replace("</", "<\\/")
}

/// Maximum input size for HTML minification (1 MiB).
///
/// Inputs exceeding this limit are returned unmodified to prevent
/// denial-of-service via excessively large payloads.
const MINIFY_HTML_MAX_INPUT_SIZE: usize = 1024 * 1024;

/// Simple HTML minification (removes extra whitespace).
///
/// Returns the input unmodified when its byte length exceeds
/// `MINIFY_HTML_MAX_INPUT_SIZE` (1MB) to prevent denial-of-service attacks.
///
/// Whitespace inside `<pre>` blocks is preserved.
fn minify_html(html: &str) -> String {
	if html.len() > MINIFY_HTML_MAX_INPUT_SIZE {
		return html.to_string();
	}

	let mut result = String::with_capacity(html.len());
	let mut prev_was_whitespace = false;
	let mut in_pre = false;
	let mut chars = html.char_indices().peekable();

	while let Some((byte_pos, c)) = chars.next() {
		let remaining = &html[byte_pos..];

		// Detect opening <pre tag (e.g. <pre>, <pre class="...">)
		if !in_pre
			&& c == '<'
			&& remaining.strip_prefix("<pre").is_some_and(|after| {
				after.starts_with(|ch: char| ch == '>' || ch.is_ascii_whitespace())
					|| after.is_empty()
			}) {
			in_pre = true;
		}

		// Detect closing </pre> tag
		if in_pre && c == '<' && remaining.starts_with("</pre>") {
			result.push_str("</pre>");
			// Skip the remaining 5 chars of "</pre>" (we already consumed '<')
			for _ in 0..5 {
				chars.next();
			}
			in_pre = false;
			prev_was_whitespace = false;
			continue;
		}

		if in_pre {
			result.push(c);
		} else if c.is_whitespace() {
			if !prev_was_whitespace {
				result.push(' ');
				prev_was_whitespace = true;
			}
		} else {
			result.push(c);
			prev_was_whitespace = false;
		}
	}

	result
}

/// Helper function for simple component rendering.
// Allow dead_code: convenience function for internal module use and tests
#[allow(dead_code)]
pub(super) fn render<C: Component>(component: &C) -> String {
	let mut renderer = SsrRenderer::new();
	renderer.render(component)
}

/// Helper function for rendering to a full HTML page.
// Allow dead_code: convenience function for internal module use and tests
#[allow(dead_code)]
pub(super) fn render_page<C: Component>(component: &C, options: SsrOptions) -> String {
	let mut renderer = SsrRenderer::with_options(options);
	renderer.render_page(component)
}

// Phase 2-B Tests: SsrOptions Extension

#[test]
fn test_ssr_options_partial_hydration_default() {
	let opts = SsrOptions::default();
	assert!(!opts.enable_partial_hydration);
	assert_eq!(opts.default_hydration_strategy, HydrationStrategy::Full);
}

#[test]
fn test_ssr_options_partial_hydration_builder() {
	let opts = SsrOptions::new()
		.partial_hydration(true)
		.default_strategy(HydrationStrategy::Island);

	assert!(opts.enable_partial_hydration);
	assert_eq!(opts.default_hydration_strategy, HydrationStrategy::Island);
}

#[test]
fn test_ssr_options_islands_only() {
	let opts = SsrOptions::new().islands_only();

	assert!(opts.enable_partial_hydration);
	assert_eq!(opts.default_hydration_strategy, HydrationStrategy::Island);
}

#[test]
fn test_ssr_options_default_strategy_static() {
	let opts = SsrOptions::new().default_strategy(HydrationStrategy::Static);

	assert!(!opts.enable_partial_hydration);
	assert_eq!(opts.default_hydration_strategy, HydrationStrategy::Static);
}
#[cfg(test)]
mod tests {
	use super::*;
	use crate::component::PageElement;

	struct TestComponent {
		message: String,
	}

	impl Component for TestComponent {
		fn render(&self) -> Page {
			PageElement::new("div")
				.attr("class", "test")
				.child(self.message.clone())
				.into_page()
		}

		fn name() -> &'static str {
			"TestComponent"
		}
	}

	#[test]
	fn test_ssr_options_default() {
		let opts = SsrOptions::default();
		assert!(opts.include_hydration_markers);
		assert!(!opts.minify);
		assert_eq!(opts.lang, "en");
	}

	#[test]
	fn test_ssr_renderer_render() {
		let component = TestComponent {
			message: "Hello".to_string(),
		};
		let mut renderer = SsrRenderer::new();
		let html = renderer.render(&component);
		assert_eq!(html, "<div class=\"test\">Hello</div>");
	}

	#[test]
	fn test_ssr_renderer_with_csrf() {
		let component = TestComponent {
			message: "Secure".to_string(),
		};
		let opts = SsrOptions::new().csrf("test-token-123");
		let mut renderer = SsrRenderer::with_options(opts);
		let html = renderer.render_page(&component);

		assert!(html.contains("csrf-token"));
		assert!(html.contains("test-token-123"));
	}

	#[test]
	fn test_ssr_renderer_with_auth() {
		let component = TestComponent {
			message: "Auth".to_string(),
		};
		let auth = AuthData::authenticated(1, "testuser");
		let opts = SsrOptions::new().auth(auth);
		let mut renderer = SsrRenderer::with_options(opts);
		let html = renderer.render_page(&component);

		assert!(html.contains("auth-data"));
		assert!(html.contains("testuser"));
	}

	#[test]
	fn test_ssr_renderer_with_marker() {
		let component = TestComponent {
			message: "Hydrate".to_string(),
		};
		let mut renderer = SsrRenderer::new();
		let html = renderer.render_with_marker(&component);

		assert!(html.contains("data-rh-id"));
		assert!(html.contains("data-rh-component=\"TestComponent\""));
	}

	#[test]
	fn test_render_helper() {
		let component = TestComponent {
			message: "Helper".to_string(),
		};
		let html = render(&component);
		assert_eq!(html, "<div class=\"test\">Helper</div>");
	}

	#[test]
	fn test_html_escape() {
		assert_eq!(html_escape("<script>"), "&lt;script&gt;");
		assert_eq!(html_escape("a&b"), "a&amp;b");
		assert_eq!(html_escape("\"quoted\""), "&quot;quoted&quot;");
	}

	#[test]
	fn test_escape_json_for_script() {
		// Verify that </script> is escaped to prevent XSS
		assert_eq!(escape_json_for_script("</script>"), "<\\/script>");
		// Verify that </ is escaped in any context
		assert_eq!(
			escape_json_for_script("</script><script>alert(1)</script>"),
			"<\\/script><script>alert(1)<\\/script>"
		);
		// Normal JSON should not be modified
		assert_eq!(
			escape_json_for_script(r#"{"name":"test"}"#),
			r#"{"name":"test"}"#
		);
	}

	#[test]
	fn test_ssr_renderer_with_auth_xss_prevention() {
		// Test that auth data with </script> in username is properly escaped
		let component = TestComponent {
			message: "Auth".to_string(),
		};
		// Simulate a malicious username that contains </script>
		let malicious_username = "</script><script>alert('xss')</script>";
		let auth = AuthData::authenticated(1, malicious_username);
		let opts = SsrOptions::new().auth(auth);
		let mut renderer = SsrRenderer::with_options(opts);
		let html = renderer.render_page(&component);

		// Verify the auth-data script tag exists
		assert!(html.contains("auth-data"));

		// Verify that </script> sequences are escaped in the JSON
		// The raw </script> should NOT appear in the HTML output
		assert!(!html.contains("</script><script>alert"));

		// The escaped version should be present
		assert!(html.contains("<\\/script>"));
	}

	#[test]
	fn test_ssr_renderer_with_auth_xss_prevention_wrap_in_html_with_head() {
		use crate::component::PageElement;

		// Test XSS prevention via wrap_in_html_with_head path
		struct TestPage {
			message: String,
		}

		impl Component for TestPage {
			fn render(&self) -> Page {
				PageElement::new("div")
					.child(self.message.clone())
					.into_page()
			}

			fn name() -> &'static str {
				"TestPage"
			}
		}

		let component = TestPage {
			message: "Test".to_string(),
		};
		// Simulate a malicious username
		let malicious_username = "</script><img src=x onerror=alert(1)>";
		let auth = AuthData::authenticated(1, malicious_username);
		let opts = SsrOptions::new().auth(auth);
		let mut renderer = SsrRenderer::with_options(opts);
		let view = component.render();
		let html = renderer.render_page_with_view_head(view);

		// Verify that raw </script> does not appear (it should be escaped)
		assert!(!html.contains("</script><img"));
		// The escaped version should be present
		assert!(html.contains("<\\/script>"));
	}
}
