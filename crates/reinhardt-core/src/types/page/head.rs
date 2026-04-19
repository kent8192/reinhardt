//! HTML Head Section Representation
//!
//! This module provides types for representing HTML `<head>` section content
//! in a structured, type-safe manner.
//!
//! ## Features
//!
//! - **Head struct**: Represents the complete `<head>` section
//! - **MetaTag struct**: Represents individual `<meta>` tags
//! - **LinkTag struct**: Represents individual `<link>` tags
//! - **ScriptTag struct**: Represents individual `<script>` tags
//!
//! ## Usage
//!
//! ```rust
//! use reinhardt_core::types::page::Head;
//!
//! let head = Head::new()
//!     .title("My Page")
//!     .meta_description("Page description")
//!     .css("/static/css/style.css")
//!     .js("/static/js/app.js");
//!
//! let html = head.to_html();
//! ```

use std::borrow::Cow;

use super::util::html_escape;

/// Represents an HTML `<meta>` tag.
///
/// Meta tags provide metadata about the HTML document.
/// They can specify character sets, descriptions, keywords,
/// viewport settings, and Open Graph properties.
#[derive(Debug, Clone, PartialEq)]
pub struct MetaTag {
	/// The `name` attribute (e.g., "description", "viewport").
	pub name: Option<Cow<'static, str>>,
	/// The `property` attribute (used for Open Graph, e.g., "og:title").
	pub property: Option<Cow<'static, str>>,
	/// The `content` attribute value.
	pub content: Cow<'static, str>,
	/// The `charset` attribute (e.g., "UTF-8").
	pub charset: Option<Cow<'static, str>>,
	/// The `http-equiv` attribute (e.g., "X-UA-Compatible").
	pub http_equiv: Option<Cow<'static, str>>,
}

impl MetaTag {
	/// Creates a new meta tag with name and content.
	pub fn new(name: impl Into<Cow<'static, str>>, content: impl Into<Cow<'static, str>>) -> Self {
		Self {
			name: Some(name.into()),
			property: None,
			content: content.into(),
			charset: None,
			http_equiv: None,
		}
	}

	/// Creates a charset meta tag.
	pub fn charset(charset: impl Into<Cow<'static, str>>) -> Self {
		Self {
			name: None,
			property: None,
			content: Cow::Borrowed(""),
			charset: Some(charset.into()),
			http_equiv: None,
		}
	}

	/// Creates an Open Graph property meta tag.
	pub fn property(
		property: impl Into<Cow<'static, str>>,
		content: impl Into<Cow<'static, str>>,
	) -> Self {
		Self {
			name: None,
			property: Some(property.into()),
			content: content.into(),
			charset: None,
			http_equiv: None,
		}
	}

	/// Creates an http-equiv meta tag.
	pub fn http_equiv(
		http_equiv: impl Into<Cow<'static, str>>,
		content: impl Into<Cow<'static, str>>,
	) -> Self {
		Self {
			name: None,
			property: None,
			content: content.into(),
			charset: None,
			http_equiv: Some(http_equiv.into()),
		}
	}

	/// Renders the meta tag to HTML string.
	pub fn to_html(&self) -> String {
		let mut attrs = Vec::new();

		if let Some(ref charset) = self.charset {
			return format!("<meta charset=\"{}\">", html_escape(charset));
		}

		if let Some(ref name) = self.name {
			attrs.push(format!("name=\"{}\"", html_escape(name)));
		}

		if let Some(ref property) = self.property {
			attrs.push(format!("property=\"{}\"", html_escape(property)));
		}

		if let Some(ref http_equiv) = self.http_equiv {
			attrs.push(format!("http-equiv=\"{}\"", html_escape(http_equiv)));
		}

		if !self.content.is_empty() {
			attrs.push(format!("content=\"{}\"", html_escape(&self.content)));
		}

		format!("<meta {}>", attrs.join(" "))
	}
}

/// Represents an HTML `<link>` tag.
///
/// Link tags define relationships between the document and external resources,
/// such as stylesheets, icons, and preload directives.
#[derive(Debug, Clone, PartialEq)]
pub struct LinkTag {
	/// The `rel` attribute (e.g., "stylesheet", "icon", "preload").
	pub rel: Cow<'static, str>,
	/// The `href` attribute.
	pub href: Cow<'static, str>,
	/// The `type` attribute (e.g., "text/css", "image/png").
	pub type_attr: Option<Cow<'static, str>>,
	/// The `as` attribute for preload (e.g., "style", "script", "image").
	pub as_attr: Option<Cow<'static, str>>,
	/// The `crossorigin` attribute.
	pub crossorigin: Option<Cow<'static, str>>,
	/// The `integrity` attribute for SRI.
	pub integrity: Option<Cow<'static, str>>,
	/// The `media` attribute for media queries.
	pub media: Option<Cow<'static, str>>,
	/// The `sizes` attribute for icons.
	pub sizes: Option<Cow<'static, str>>,
}

impl LinkTag {
	/// Creates a new link tag with rel and href.
	pub fn new(rel: impl Into<Cow<'static, str>>, href: impl Into<Cow<'static, str>>) -> Self {
		Self {
			rel: rel.into(),
			href: href.into(),
			type_attr: None,
			as_attr: None,
			crossorigin: None,
			integrity: None,
			media: None,
			sizes: None,
		}
	}

	/// Creates a stylesheet link.
	pub fn stylesheet(href: impl Into<Cow<'static, str>>) -> Self {
		Self::new("stylesheet", href)
	}

	/// Creates an icon link.
	pub fn icon(href: impl Into<Cow<'static, str>>) -> Self {
		Self::new("icon", href)
	}

	/// Creates a preload link.
	pub fn preload(
		href: impl Into<Cow<'static, str>>,
		as_type: impl Into<Cow<'static, str>>,
	) -> Self {
		let mut link = Self::new("preload", href);
		link.as_attr = Some(as_type.into());
		link
	}

	/// Sets the type attribute.
	pub fn with_type(mut self, type_attr: impl Into<Cow<'static, str>>) -> Self {
		self.type_attr = Some(type_attr.into());
		self
	}

	/// Sets the crossorigin attribute.
	pub fn with_crossorigin(mut self, crossorigin: impl Into<Cow<'static, str>>) -> Self {
		self.crossorigin = Some(crossorigin.into());
		self
	}

	/// Sets the integrity attribute for SRI.
	pub fn with_integrity(mut self, integrity: impl Into<Cow<'static, str>>) -> Self {
		self.integrity = Some(integrity.into());
		self
	}

	/// Sets the media attribute.
	pub fn with_media(mut self, media: impl Into<Cow<'static, str>>) -> Self {
		self.media = Some(media.into());
		self
	}

	/// Sets the sizes attribute.
	pub fn with_sizes(mut self, sizes: impl Into<Cow<'static, str>>) -> Self {
		self.sizes = Some(sizes.into());
		self
	}

	/// Renders the link tag to HTML string.
	pub fn to_html(&self) -> String {
		let mut attrs = vec![
			format!("rel=\"{}\"", html_escape(&self.rel)),
			format!("href=\"{}\"", html_escape(&self.href)),
		];

		if let Some(ref type_attr) = self.type_attr {
			attrs.push(format!("type=\"{}\"", html_escape(type_attr)));
		}

		if let Some(ref as_attr) = self.as_attr {
			attrs.push(format!("as=\"{}\"", html_escape(as_attr)));
		}

		if let Some(ref crossorigin) = self.crossorigin {
			attrs.push(format!("crossorigin=\"{}\"", html_escape(crossorigin)));
		}

		if let Some(ref integrity) = self.integrity {
			attrs.push(format!("integrity=\"{}\"", html_escape(integrity)));
		}

		if let Some(ref media) = self.media {
			attrs.push(format!("media=\"{}\"", html_escape(media)));
		}

		if let Some(ref sizes) = self.sizes {
			attrs.push(format!("sizes=\"{}\"", html_escape(sizes)));
		}

		format!("<link {}>", attrs.join(" "))
	}
}

/// Represents an HTML `<script>` tag.
///
/// Script tags can reference external scripts or contain inline JavaScript.
#[derive(Debug, Clone, PartialEq)]
pub struct ScriptTag {
	/// The `src` attribute for external scripts.
	pub src: Option<Cow<'static, str>>,
	/// Inline script content.
	pub content: Option<Cow<'static, str>>,
	/// The `type` attribute (e.g., "module", "text/javascript").
	pub type_attr: Option<Cow<'static, str>>,
	/// Whether to add `async` attribute.
	pub is_async: bool,
	/// Whether to add `defer` attribute.
	pub is_defer: bool,
	/// The `crossorigin` attribute.
	pub crossorigin: Option<Cow<'static, str>>,
	/// The `integrity` attribute for SRI.
	pub integrity: Option<Cow<'static, str>>,
	/// The `nonce` attribute for CSP.
	pub nonce: Option<Cow<'static, str>>,
}

impl ScriptTag {
	/// Creates a new external script tag.
	pub fn external(src: impl Into<Cow<'static, str>>) -> Self {
		Self {
			src: Some(src.into()),
			content: None,
			type_attr: None,
			is_async: false,
			is_defer: false,
			crossorigin: None,
			integrity: None,
			nonce: None,
		}
	}

	/// Creates a new inline script tag.
	pub fn inline(content: impl Into<Cow<'static, str>>) -> Self {
		Self {
			src: None,
			content: Some(content.into()),
			type_attr: None,
			is_async: false,
			is_defer: false,
			crossorigin: None,
			integrity: None,
			nonce: None,
		}
	}

	/// Creates a module script.
	pub fn module(src: impl Into<Cow<'static, str>>) -> Self {
		let mut script = Self::external(src);
		script.type_attr = Some(Cow::Borrowed("module"));
		script
	}

	/// Sets the script to async.
	pub fn with_async(mut self) -> Self {
		self.is_async = true;
		self
	}

	/// Sets the script to defer.
	pub fn with_defer(mut self) -> Self {
		self.is_defer = true;
		self
	}

	/// Sets the type attribute.
	pub fn with_type(mut self, type_attr: impl Into<Cow<'static, str>>) -> Self {
		self.type_attr = Some(type_attr.into());
		self
	}

	/// Sets the crossorigin attribute.
	pub fn with_crossorigin(mut self, crossorigin: impl Into<Cow<'static, str>>) -> Self {
		self.crossorigin = Some(crossorigin.into());
		self
	}

	/// Sets the integrity attribute for SRI.
	pub fn with_integrity(mut self, integrity: impl Into<Cow<'static, str>>) -> Self {
		self.integrity = Some(integrity.into());
		self
	}

	/// Sets the nonce attribute for CSP.
	pub fn with_nonce(mut self, nonce: impl Into<Cow<'static, str>>) -> Self {
		self.nonce = Some(nonce.into());
		self
	}

	/// Renders the script tag to HTML string.
	pub fn to_html(&self) -> String {
		let mut attrs = Vec::new();

		if let Some(ref src) = self.src {
			attrs.push(format!("src=\"{}\"", html_escape(src)));
		}

		if let Some(ref type_attr) = self.type_attr {
			attrs.push(format!("type=\"{}\"", html_escape(type_attr)));
		}

		if self.is_async {
			attrs.push("async".to_string());
		}

		if self.is_defer {
			attrs.push("defer".to_string());
		}

		if let Some(ref crossorigin) = self.crossorigin {
			attrs.push(format!("crossorigin=\"{}\"", html_escape(crossorigin)));
		}

		if let Some(ref integrity) = self.integrity {
			attrs.push(format!("integrity=\"{}\"", html_escape(integrity)));
		}

		if let Some(ref nonce) = self.nonce {
			attrs.push(format!("nonce=\"{}\"", html_escape(nonce)));
		}

		if let Some(ref content) = self.content {
			// Escape closing tags in inline scripts to prevent premature script termination.
			// The "</script>" sequence inside a <script> block would be interpreted by the HTML
			// parser as closing the script element, enabling injection. Replacing "</" with "<\\/"
			// is a well-established defense used by Django, Rails, and other frameworks.
			// This pattern only matches the literal two-character sequence "</" -- it does not
			// affect expressions like `a < / b` (which have spaces).
			let escaped_content = content.replace("</", "<\\/");

			if attrs.is_empty() {
				format!("<script>{}</script>", escaped_content)
			} else {
				format!("<script {}>{}</script>", attrs.join(" "), escaped_content)
			}
		} else {
			format!("<script {}></script>", attrs.join(" "))
		}
	}
}

/// Represents an HTML `<style>` tag with inline CSS.
#[derive(Debug, Clone, PartialEq)]
pub struct StyleTag {
	/// The CSS content.
	pub content: Cow<'static, str>,
	/// The `media` attribute for media queries.
	pub media: Option<Cow<'static, str>>,
	/// The `nonce` attribute for CSP.
	pub nonce: Option<Cow<'static, str>>,
}

impl StyleTag {
	/// Creates a new inline style tag.
	pub fn new(content: impl Into<Cow<'static, str>>) -> Self {
		Self {
			content: content.into(),
			media: None,
			nonce: None,
		}
	}

	/// Sets the media attribute.
	pub fn with_media(mut self, media: impl Into<Cow<'static, str>>) -> Self {
		self.media = Some(media.into());
		self
	}

	/// Sets the nonce attribute for CSP.
	pub fn with_nonce(mut self, nonce: impl Into<Cow<'static, str>>) -> Self {
		self.nonce = Some(nonce.into());
		self
	}

	/// Renders the style tag to HTML string.
	pub fn to_html(&self) -> String {
		let mut attrs = Vec::new();

		if let Some(ref media) = self.media {
			attrs.push(format!("media=\"{}\"", html_escape(media)));
		}

		if let Some(ref nonce) = self.nonce {
			attrs.push(format!("nonce=\"{}\"", html_escape(nonce)));
		}

		// Escape "</", preventing content from closing the style tag (XSS mitigation)
		let escaped_content = self.content.replace("</", "<\\/");

		if attrs.is_empty() {
			format!("<style>{}</style>", escaped_content)
		} else {
			format!("<style {}>{}</style>", attrs.join(" "), escaped_content)
		}
	}
}

/// Represents the complete HTML `<head>` section.
///
/// The Head struct provides a builder pattern for constructing the head section
/// of an HTML document. It supports titles, meta tags, stylesheets, scripts,
/// and other head elements.
///
/// ## Example
///
/// ```rust
/// use reinhardt_core::types::page::Head;
///
/// let head = Head::new()
///     .title("My Application")
///     .meta_description("A great application")
///     .meta_viewport("width=device-width, initial-scale=1.0")
///     .css("/static/css/style.css")
///     .js_defer("/static/js/app.js");
/// ```
#[derive(Debug, Clone, Default)]
pub struct Head {
	/// The document title.
	pub title: Option<Cow<'static, str>>,
	/// Collection of meta tags.
	pub meta_tags: Vec<MetaTag>,
	/// Collection of link tags (stylesheets, icons, etc.).
	pub links: Vec<LinkTag>,
	/// Collection of script tags.
	pub scripts: Vec<ScriptTag>,
	/// Collection of inline style tags.
	pub styles: Vec<StyleTag>,
	/// Base URL for relative URLs.
	pub base: Option<Cow<'static, str>>,
}

impl Head {
	/// Creates a new empty Head.
	pub fn new() -> Self {
		Self::default()
	}

	/// Creates a Head with default meta tags (charset and viewport).
	pub fn with_defaults() -> Self {
		Self::new()
			.meta_charset("UTF-8")
			.meta_viewport("width=device-width, initial-scale=1.0")
	}

	/// Sets the document title.
	pub fn title(mut self, title: impl Into<Cow<'static, str>>) -> Self {
		self.title = Some(title.into());
		self
	}

	/// Adds a meta tag.
	pub fn meta(mut self, tag: MetaTag) -> Self {
		self.meta_tags.push(tag);
		self
	}

	/// Adds a charset meta tag.
	pub fn meta_charset(self, charset: impl Into<Cow<'static, str>>) -> Self {
		self.meta(MetaTag::charset(charset))
	}

	/// Adds a description meta tag.
	pub fn meta_description(self, description: impl Into<Cow<'static, str>>) -> Self {
		self.meta(MetaTag::new("description", description))
	}

	/// Adds a viewport meta tag.
	pub fn meta_viewport(self, viewport: impl Into<Cow<'static, str>>) -> Self {
		self.meta(MetaTag::new("viewport", viewport))
	}

	/// Adds a keywords meta tag.
	pub fn meta_keywords(self, keywords: impl Into<Cow<'static, str>>) -> Self {
		self.meta(MetaTag::new("keywords", keywords))
	}

	/// Adds an author meta tag.
	pub fn meta_author(self, author: impl Into<Cow<'static, str>>) -> Self {
		self.meta(MetaTag::new("author", author))
	}

	/// Adds a robots meta tag.
	pub fn meta_robots(self, robots: impl Into<Cow<'static, str>>) -> Self {
		self.meta(MetaTag::new("robots", robots))
	}

	/// Adds an Open Graph title meta tag.
	pub fn og_title(self, title: impl Into<Cow<'static, str>>) -> Self {
		self.meta(MetaTag::property("og:title", title))
	}

	/// Adds an Open Graph description meta tag.
	pub fn og_description(self, description: impl Into<Cow<'static, str>>) -> Self {
		self.meta(MetaTag::property("og:description", description))
	}

	/// Adds an Open Graph image meta tag.
	pub fn og_image(self, image_url: impl Into<Cow<'static, str>>) -> Self {
		self.meta(MetaTag::property("og:image", image_url))
	}

	/// Adds an Open Graph URL meta tag.
	pub fn og_url(self, url: impl Into<Cow<'static, str>>) -> Self {
		self.meta(MetaTag::property("og:url", url))
	}

	/// Adds an Open Graph type meta tag.
	pub fn og_type(self, og_type: impl Into<Cow<'static, str>>) -> Self {
		self.meta(MetaTag::property("og:type", og_type))
	}

	/// Adds a Twitter card meta tag.
	pub fn twitter_card(self, card_type: impl Into<Cow<'static, str>>) -> Self {
		self.meta(MetaTag::new("twitter:card", card_type))
	}

	/// Adds a link tag.
	pub fn link(mut self, tag: LinkTag) -> Self {
		self.links.push(tag);
		self
	}

	/// Adds a stylesheet link.
	pub fn css(self, href: impl Into<Cow<'static, str>>) -> Self {
		self.link(LinkTag::stylesheet(href))
	}

	/// Adds an icon link.
	pub fn icon(self, href: impl Into<Cow<'static, str>>) -> Self {
		self.link(LinkTag::icon(href))
	}

	/// Adds a favicon link with type.
	pub fn favicon(
		self,
		href: impl Into<Cow<'static, str>>,
		type_attr: impl Into<Cow<'static, str>>,
	) -> Self {
		self.link(LinkTag::icon(href).with_type(type_attr))
	}

	/// Adds a preload link.
	pub fn preload(
		self,
		href: impl Into<Cow<'static, str>>,
		as_type: impl Into<Cow<'static, str>>,
	) -> Self {
		self.link(LinkTag::preload(href, as_type))
	}

	/// Adds a canonical URL link.
	pub fn canonical(self, href: impl Into<Cow<'static, str>>) -> Self {
		self.link(LinkTag::new("canonical", href))
	}

	/// Adds a script tag.
	pub fn script(mut self, tag: ScriptTag) -> Self {
		self.scripts.push(tag);
		self
	}

	/// Adds an external script.
	pub fn js(self, src: impl Into<Cow<'static, str>>) -> Self {
		self.script(ScriptTag::external(src))
	}

	/// Adds an external script with defer attribute.
	pub fn js_defer(self, src: impl Into<Cow<'static, str>>) -> Self {
		self.script(ScriptTag::external(src).with_defer())
	}

	/// Adds an external script with async attribute.
	pub fn js_async(self, src: impl Into<Cow<'static, str>>) -> Self {
		self.script(ScriptTag::external(src).with_async())
	}

	/// Adds a module script.
	pub fn js_module(self, src: impl Into<Cow<'static, str>>) -> Self {
		self.script(ScriptTag::module(src))
	}

	/// Adds an inline script.
	pub fn inline_js(self, content: impl Into<Cow<'static, str>>) -> Self {
		self.script(ScriptTag::inline(content))
	}

	/// Adds a style tag.
	pub fn style(mut self, tag: StyleTag) -> Self {
		self.styles.push(tag);
		self
	}

	/// Adds inline CSS.
	pub fn inline_css(self, css: impl Into<Cow<'static, str>>) -> Self {
		self.style(StyleTag::new(css))
	}

	/// Sets the base URL.
	pub fn base_url(mut self, url: impl Into<Cow<'static, str>>) -> Self {
		self.base = Some(url.into());
		self
	}

	/// Merges another Head into this one.
	///
	/// The other Head's values take precedence for title and base.
	/// All other elements (meta, links, scripts, styles) are appended.
	pub fn merge(mut self, other: Head) -> Self {
		if other.title.is_some() {
			self.title = other.title;
		}
		if other.base.is_some() {
			self.base = other.base;
		}
		self.meta_tags.extend(other.meta_tags);
		self.links.extend(other.links);
		self.scripts.extend(other.scripts);
		self.styles.extend(other.styles);
		self
	}

	/// Renders the head section to HTML string.
	///
	/// This method generates the complete `<head>` tag content,
	/// including all meta tags, links, scripts, and styles.
	pub fn to_html(&self) -> String {
		let mut parts = Vec::new();

		// Base tag
		if let Some(ref base) = self.base {
			parts.push(format!("<base href=\"{}\">", html_escape(base)));
		}

		// Meta tags
		for meta in &self.meta_tags {
			parts.push(meta.to_html());
		}

		// Title
		if let Some(ref title) = self.title {
			parts.push(format!("<title>{}</title>", html_escape(title)));
		}

		// Link tags
		for link in &self.links {
			parts.push(link.to_html());
		}

		// Style tags
		for style in &self.styles {
			parts.push(style.to_html());
		}

		// Script tags
		for script in &self.scripts {
			parts.push(script.to_html());
		}

		parts.join("\n")
	}

	/// Checks if the head is empty (has no content).
	pub fn is_empty(&self) -> bool {
		self.title.is_none()
			&& self.meta_tags.is_empty()
			&& self.links.is_empty()
			&& self.scripts.is_empty()
			&& self.styles.is_empty()
			&& self.base.is_none()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_meta_tag_name_content() {
		let meta = MetaTag::new("description", "Test page");
		assert_eq!(
			meta.to_html(),
			"<meta name=\"description\" content=\"Test page\">"
		);
	}

	#[rstest]
	fn test_meta_tag_charset() {
		let meta = MetaTag::charset("UTF-8");
		assert_eq!(meta.to_html(), "<meta charset=\"UTF-8\">");
	}

	#[rstest]
	fn test_meta_tag_property() {
		let meta = MetaTag::property("og:title", "My Page");
		assert_eq!(
			meta.to_html(),
			"<meta property=\"og:title\" content=\"My Page\">"
		);
	}

	#[rstest]
	fn test_link_tag_stylesheet() {
		let link = LinkTag::stylesheet("/static/css/style.css");
		assert_eq!(
			link.to_html(),
			"<link rel=\"stylesheet\" href=\"/static/css/style.css\">"
		);
	}

	#[rstest]
	fn test_link_tag_with_integrity() {
		let link = LinkTag::stylesheet("https://cdn.example.com/style.css")
			.with_integrity("sha384-abc123")
			.with_crossorigin("anonymous");
		assert!(link.to_html().contains("integrity=\"sha384-abc123\""));
		assert!(link.to_html().contains("crossorigin=\"anonymous\""));
	}

	#[rstest]
	fn test_script_tag_external() {
		let script = ScriptTag::external("/static/js/app.js").with_defer();
		assert_eq!(
			script.to_html(),
			"<script src=\"/static/js/app.js\" defer></script>"
		);
	}

	#[rstest]
	fn test_script_tag_inline() {
		let script = ScriptTag::inline("console.log('Hello');");
		assert_eq!(script.to_html(), "<script>console.log('Hello');</script>");
	}

	#[rstest]
	fn test_script_tag_module() {
		let script = ScriptTag::module("/static/js/main.js");
		assert_eq!(
			script.to_html(),
			"<script src=\"/static/js/main.js\" type=\"module\"></script>"
		);
	}

	#[rstest]
	fn test_style_tag() {
		let style = StyleTag::new("body { margin: 0; }");
		assert_eq!(style.to_html(), "<style>body { margin: 0; }</style>");
	}

	#[rstest]
	fn test_style_tag_escapes_closing_tag_xss() {
		// Arrange
		let style = StyleTag::new("</style><script>alert(1)</script>");

		// Act
		let html = style.to_html();

		// Assert
		assert_eq!(html, "<style><\\/style><script>alert(1)<\\/script></style>");
		assert!(!html.contains("</style><script>"));
	}

	#[rstest]
	fn test_style_tag_escapes_multiple_closing_sequences() {
		// Arrange
		let style = StyleTag::new("a{} </style> b{} </style>");

		// Act
		let html = style.to_html();

		// Assert
		assert_eq!(html, "<style>a{} <\\/style> b{} <\\/style></style>");
	}

	#[rstest]
	fn test_head_builder() {
		let head = Head::new()
			.title("Test Page")
			.meta_charset("UTF-8")
			.meta_description("A test page")
			.css("/static/css/style.css")
			.js_defer("/static/js/app.js");

		let html = head.to_html();
		assert!(html.contains("<title>Test Page</title>"));
		assert!(html.contains("<meta charset=\"UTF-8\">"));
		assert!(html.contains("name=\"description\""));
		assert!(html.contains("href=\"/static/css/style.css\""));
		assert!(html.contains("src=\"/static/js/app.js\""));
	}

	#[rstest]
	fn test_head_with_defaults() {
		let head = Head::with_defaults().title("My App");
		let html = head.to_html();
		assert!(html.contains("<meta charset=\"UTF-8\">"));
		assert!(html.contains("name=\"viewport\""));
		assert!(html.contains("<title>My App</title>"));
	}

	#[rstest]
	fn test_head_merge() {
		let base = Head::new()
			.title("Base Title")
			.meta_charset("UTF-8")
			.css("/base.css");

		let overlay = Head::new()
			.title("Override Title")
			.meta_description("Description")
			.css("/overlay.css");

		let merged = base.merge(overlay);
		assert_eq!(merged.title, Some(Cow::Borrowed("Override Title")));
		assert_eq!(merged.meta_tags.len(), 2); // charset + description
		assert_eq!(merged.links.len(), 2); // base.css + overlay.css
	}

	#[rstest]
	fn test_head_is_empty() {
		assert!(Head::new().is_empty());
		assert!(!Head::new().title("Title").is_empty());
	}

	#[rstest]
	fn test_html_escape() {
		let head = Head::new().title("Test <script>alert('XSS')</script>");
		let html = head.to_html();
		assert!(html.contains("&lt;script&gt;"));
		assert!(!html.contains("<script>"));
	}

	#[rstest]
	fn test_open_graph_meta() {
		let head = Head::new()
			.og_title("OG Title")
			.og_description("OG Description")
			.og_image("https://example.com/image.png")
			.og_type("website");

		let html = head.to_html();
		assert!(html.contains("property=\"og:title\""));
		assert!(html.contains("property=\"og:description\""));
		assert!(html.contains("property=\"og:image\""));
		assert!(html.contains("property=\"og:type\""));
	}

	#[rstest]
	fn test_script_tag_escapes_closing_tag_xss() {
		// Arrange
		let script = ScriptTag::inline("</script><script>alert(1)</script>");

		// Act
		let html = script.to_html();

		// Assert
		assert_eq!(
			html,
			"<script><\\/script><script>alert(1)<\\/script></script>"
		);
		assert!(!html.contains("</script><script>"));
	}

	#[rstest]
	fn test_script_tag_preserves_normal_content() {
		// Arrange
		let script = ScriptTag::inline("console.log('hello');");

		// Act
		let html = script.to_html();

		// Assert
		assert_eq!(html, "<script>console.log('hello');</script>");
	}
}
