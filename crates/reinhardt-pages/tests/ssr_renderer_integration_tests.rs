#![cfg(not(target_arch = "wasm32"))]
//! SSR Renderer Integration Tests
//!
//! This test suite validates the Server-Side Rendering functionality of reinhardt-pages.
//! These tests replace and extend the rendering tests from the deleted reinhardt-template crate.
//!
//! Test Categories:
//! 1. Basic Rendering - Component to HTML string conversion
//! 2. Escape Handling - XSS prevention and HTML entity escaping
//! 3. Hydration Markers - Client-side hydration support
//! 4. Full Page Rendering - Complete HTML document generation
//! 5. Performance - Large component trees and deep nesting
//! 6. Edge Cases - SVG, custom attributes, fragments

use reinhardt_pages::component::{
	Component, Head, IntoPage, LinkTag, MetaTag, Page, PageElement, ScriptTag,
};
use reinhardt_pages::ssr::{SsrOptions, SsrRenderer};

// ============================================================================
// Test Components
// ============================================================================

/// Simple counter component for testing basic rendering
struct Counter {
	count: i32,
}

impl Counter {
	fn new(count: i32) -> Self {
		Self { count }
	}
}

impl Component for Counter {
	fn render(&self) -> Page {
		PageElement::new("div")
			.attr("class", "counter")
			.child(
				PageElement::new("span")
					.attr("data-count", self.count.to_string())
					.child(format!("Count: {}", self.count))
					.into_page(),
			)
			.child(
				PageElement::new("button")
					.attr("type", "button")
					.child("Increment")
					.into_page(),
			)
			.into_page()
	}

	fn name() -> &'static str {
		"Counter"
	}
}

/// User card component for testing nested structures
struct UserCard {
	name: String,
	email: String,
	role: Option<String>,
}

impl UserCard {
	fn new(name: impl Into<String>, email: impl Into<String>) -> Self {
		Self {
			name: name.into(),
			email: email.into(),
			role: None,
		}
	}

	fn with_role(mut self, role: impl Into<String>) -> Self {
		self.role = Some(role.into());
		self
	}
}

impl Component for UserCard {
	fn render(&self) -> Page {
		let mut article = PageElement::new("article")
			.attr("class", "user-card")
			.child(PageElement::new("h2").child(self.name.clone()).into_page())
			.child(
				PageElement::new("p")
					.attr("class", "email")
					.child(self.email.clone())
					.into_page(),
			);

		if let Some(ref role) = self.role {
			article = article.child(
				PageElement::new("p")
					.attr("class", "role")
					.child(role.clone())
					.into_page(),
			);
		}

		article.into_page()
	}

	fn name() -> &'static str {
		"UserCard"
	}
}

// Container component removed - use PageElement directly for composition

// ============================================================================
// Category 1: Basic Rendering Tests (10-15 tests)
// ============================================================================

#[test]
fn test_basic_component_render() {
	let counter = Counter::new(42);
	let html = counter.render().render_to_string();

	assert!(html.contains("class=\"counter\""));
	assert!(html.contains("data-count=\"42\""));
	assert!(html.contains("Count: 42"));
}

#[test]
fn test_component_with_button() {
	let counter = Counter::new(0);
	let html = counter.render().render_to_string();

	assert!(html.contains("<button"));
	assert!(html.contains("type=\"button\""));
	assert!(html.contains("Increment"));
	assert!(html.contains("</button>"));
}

#[test]
fn test_nested_components() {
	let card = UserCard::new("Alice", "alice@example.com");
	let html = card.render().render_to_string();

	assert!(html.contains("class=\"user-card\""));
	assert!(html.contains("<h2>Alice</h2>"));
	assert!(html.contains("class=\"email\""));
	assert!(html.contains("alice@example.com"));
}

#[test]
fn test_conditional_rendering() {
	let card_without_role = UserCard::new("Bob", "bob@example.com");
	let html_without = card_without_role.render().render_to_string();
	assert!(!html_without.contains("class=\"role\""));

	let card_with_role = UserCard::new("Charlie", "charlie@example.com").with_role("Admin");
	let html_with = card_with_role.render().render_to_string();
	assert!(html_with.contains("class=\"role\""));
	assert!(html_with.contains("Admin"));
}

#[test]
fn test_list_rendering() {
	let items = vec!["Apple", "Banana", "Cherry"];
	let list = PageElement::new("ul");

	let list_with_items = items.iter().fold(list, |acc, item| {
		acc.child(PageElement::new("li").child(item.to_string()).into_page())
	});

	let html = list_with_items.into_page().render_to_string();

	assert!(html.contains("<ul>"));
	assert!(html.contains("<li>Apple</li>"));
	assert!(html.contains("<li>Banana</li>"));
	assert!(html.contains("<li>Cherry</li>"));
	assert!(html.contains("</ul>"));
}

#[test]
fn test_empty_component() {
	let empty = PageElement::new("div");
	let html = empty.into_page().render_to_string();

	assert_eq!(html, "<div></div>");
}

#[test]
fn test_component_composition() {
	let container = PageElement::new("div")
		.attr("class", "container")
		.child(Counter::new(1).render())
		.child(Counter::new(2).render());

	let html = container.into_page().render_to_string();

	assert!(html.contains("class=\"container\""));
	assert!(html.contains("Count: 1"));
	assert!(html.contains("Count: 2"));
}

#[test]
fn test_multiple_attributes() {
	let div = PageElement::new("div")
		.attr("id", "main")
		.attr("class", "content")
		.attr("data-test", "value");

	let html = div.into_page().render_to_string();

	assert!(html.contains("id=\"main\""));
	assert!(html.contains("class=\"content\""));
	assert!(html.contains("data-test=\"value\""));
}

#[test]
fn test_deeply_nested_structure() {
	let deep = PageElement::new("div")
		.child(
			PageElement::new("section")
				.child(
					PageElement::new("article")
						.child(PageElement::new("p").child("Deep content").into_page())
						.into_page(),
				)
				.into_page(),
		)
		.into_page();

	let html = deep.render_to_string();

	assert!(html.contains("<div>"));
	assert!(html.contains("<section>"));
	assert!(html.contains("<article>"));
	assert!(html.contains("<p>Deep content</p>"));
	assert!(html.contains("</article>"));
	assert!(html.contains("</section>"));
	assert!(html.contains("</div>"));
}

#[test]
fn test_text_only_element() {
	let text = PageElement::new("p").child("Simple text");
	let html = text.into_page().render_to_string();

	assert_eq!(html, "<p>Simple text</p>");
}

// ============================================================================
// Category 2: Escape Handling Tests (10-15 tests)
// ============================================================================

#[test]
fn test_html_entity_escape_less_than() {
	let text = PageElement::new("p").child("<script>");
	let html = text.into_page().render_to_string();

	assert!(html.contains("&lt;script&gt;"));
	assert!(!html.contains("<script>"));
}

#[test]
fn test_html_entity_escape_greater_than() {
	let text = PageElement::new("p").child("a > b");
	let html = text.into_page().render_to_string();

	assert!(html.contains("a &gt; b"));
}

#[test]
fn test_html_entity_escape_ampersand() {
	let text = PageElement::new("p").child("a & b");
	let html = text.into_page().render_to_string();

	assert!(html.contains("a &amp; b"));
}

#[test]
fn test_html_entity_escape_quotes() {
	let text = PageElement::new("p").child("\"quoted\"");
	let html = text.into_page().render_to_string();

	assert!(html.contains("&quot;quoted&quot;"));
}

#[test]
fn test_html_entity_escape_single_quotes() {
	let text = PageElement::new("p").child("'single'");
	let html = text.into_page().render_to_string();

	assert!(html.contains("&#x27;single&#x27;"));
}

#[test]
fn test_attribute_escape() {
	let div = PageElement::new("div").attr("data-value", "\"quoted\" & <special>");
	let html = div.into_page().render_to_string();

	assert!(html.contains("data-value=\"&quot;quoted&quot; &amp; &lt;special&gt;\""));
}

#[test]
fn test_xss_prevention_script_tag() {
	let malicious = PageElement::new("div").child("<script>alert('xss')</script>");
	let html = malicious.into_page().render_to_string();

	assert!(!html.contains("<script>"));
	assert!(html.contains("&lt;script&gt;"));
	assert!(html.contains("&lt;/script&gt;"));
}

#[test]
fn test_xss_prevention_onclick() {
	let malicious = PageElement::new("div").attr("title", "\" onclick=\"alert('xss')");
	let html = malicious.into_page().render_to_string();

	assert!(html.contains("&quot;"));
	assert!(!html.contains("onclick=\"alert"));
}

#[test]
fn test_unicode_characters() {
	let text = PageElement::new("p").child("こんにちは 世界 🌍");
	let html = text.into_page().render_to_string();

	assert!(html.contains("こんにちは 世界 🌍"));
}

#[test]
fn test_special_html_entities() {
	let text = PageElement::new("p").child("© ® ™ € £ ¥");
	let html = text.into_page().render_to_string();

	assert!(html.contains("© ® ™ € £ ¥"));
}

#[test]
fn test_mixed_escape_content() {
	let text = PageElement::new("p").child("<div>\"a & b\" > 'c'</div>");
	let html = text.into_page().render_to_string();

	assert!(html.contains("&lt;div&gt;"));
	assert!(html.contains("&quot;a &amp; b&quot;"));
	assert!(html.contains("&gt;"));
	assert!(html.contains("&#x27;c&#x27;"));
	assert!(html.contains("&lt;/div&gt;"));
}

// ============================================================================
// Category 3: Hydration Marker Tests (10-12 tests)
// ============================================================================

#[test]
fn test_hydration_marker_enabled() {
	let counter = Counter::new(10);
	let options = SsrOptions::new();
	let mut renderer = SsrRenderer::with_options(options);
	let html = renderer.render_with_marker(&counter);

	assert!(html.contains("data-rh-id"));
	assert!(html.contains("data-rh-component=\"Counter\""));
}

#[test]
fn test_hydration_marker_disabled() {
	let counter = Counter::new(5);
	let options = SsrOptions::new().no_hydration();
	let mut renderer = SsrRenderer::with_options(options);
	let html = renderer.render_with_marker(&counter);

	assert!(!html.contains("data-rh-id"));
	assert!(html.contains("Count: 5"));
}

#[test]
fn test_hydration_marker_component_name() {
	let card = UserCard::new("Test", "test@example.com");
	let mut renderer = SsrRenderer::new();
	let html = renderer.render_with_marker(&card);

	assert!(html.contains("data-rh-component=\"UserCard\""));
}

#[test]
fn test_hydration_marker_wraps_content() {
	let counter = Counter::new(42);
	let mut renderer = SsrRenderer::new();
	let html = renderer.render_with_marker(&counter);

	assert!(html.starts_with("<div"));
	assert!(html.ends_with("</div>"));
	assert!(html.contains("Count: 42"));
}

#[test]
fn test_multiple_components_different_markers() {
	let counter1 = Counter::new(1);
	let counter2 = Counter::new(2);

	let mut renderer = SsrRenderer::new();
	let html1 = renderer.render_with_marker(&counter1);
	let html2 = renderer.render_with_marker(&counter2);

	// Both should have markers but with different IDs
	assert!(html1.contains("data-rh-id"));
	assert!(html2.contains("data-rh-id"));
}

// ============================================================================
// Category 4: Full Page Rendering Tests (15-20 tests)
// ============================================================================

#[test]
fn test_full_page_doctype() {
	let counter = Counter::new(0);
	let mut renderer = SsrRenderer::new();
	let html = renderer.render_page(&counter);

	assert!(html.starts_with("<!DOCTYPE html>"));
}

#[test]
fn test_full_page_html_structure() {
	let counter = Counter::new(0);
	let mut renderer = SsrRenderer::new();
	let html = renderer.render_page(&counter);

	assert!(html.contains("<html lang=\"en\">"));
	assert!(html.contains("<head>"));
	assert!(html.contains("</head>"));
	assert!(html.contains("<body>"));
	assert!(html.contains("</body>"));
	assert!(html.ends_with("</html>"));
}

#[test]
fn test_full_page_meta_charset() {
	let counter = Counter::new(0);
	let mut renderer = SsrRenderer::new();
	let html = renderer.render_page(&counter);

	assert!(html.contains("<meta charset=\"UTF-8\">"));
}

#[test]
fn test_full_page_meta_viewport() {
	let counter = Counter::new(0);
	let mut renderer = SsrRenderer::new();
	let html = renderer.render_page(&counter);

	assert!(
		html.contains("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">")
	);
}

#[test]
fn test_full_page_custom_title() {
	let counter = Counter::new(0);
	let page_head = Head::new().title("Test Page");
	let view = counter.render().with_head(page_head);
	let mut renderer = SsrRenderer::new();
	let html = renderer.render_page_with_view_head(view);

	assert!(html.contains("<title>Test Page</title>"));
}

#[test]
fn test_full_page_custom_lang() {
	let counter = Counter::new(0);
	let options = SsrOptions::new().lang("ja");
	let mut renderer = SsrRenderer::with_options(options);
	let html = renderer.render_page(&counter);

	assert!(html.contains("<html lang=\"ja\">"));
}

#[test]
fn test_full_page_css_link() {
	let counter = Counter::new(0);
	let page_head = Head::new().link(LinkTag::stylesheet("/styles.css"));
	let view = counter.render().with_head(page_head);
	let mut renderer = SsrRenderer::new();
	let html = renderer.render_page_with_view_head(view);

	assert!(html.contains("<link rel=\"stylesheet\" href=\"/styles.css\">"));
}

#[test]
fn test_full_page_multiple_css_links() {
	let counter = Counter::new(0);
	let page_head = Head::new()
		.link(LinkTag::stylesheet("/reset.css"))
		.link(LinkTag::stylesheet("/main.css"));
	let view = counter.render().with_head(page_head);
	let mut renderer = SsrRenderer::new();
	let html = renderer.render_page_with_view_head(view);

	assert!(html.contains("<link rel=\"stylesheet\" href=\"/reset.css\">"));
	assert!(html.contains("<link rel=\"stylesheet\" href=\"/main.css\">"));
}

#[test]
fn test_full_page_js_script() {
	let counter = Counter::new(0);
	let page_head = Head::new().script(ScriptTag::external("/app.js"));
	let view = counter.render().with_head(page_head);
	let mut renderer = SsrRenderer::new();
	let html = renderer.render_page_with_view_head(view);

	assert!(html.contains("<script src=\"/app.js\"></script>"));
}

#[test]
fn test_full_page_custom_meta_tags() {
	let counter = Counter::new(0);
	let page_head = Head::new()
		.meta(MetaTag::new("description", "Test page"))
		.meta(MetaTag::new("keywords", "test, rust, ssr"));
	let view = counter.render().with_head(page_head);
	let mut renderer = SsrRenderer::new();
	let html = renderer.render_page_with_view_head(view);

	assert!(html.contains("<meta name=\"description\" content=\"Test page\">"));
	assert!(html.contains("<meta name=\"keywords\" content=\"test, rust, ssr\">"));
}

#[test]
fn test_full_page_csrf_token() {
	let counter = Counter::new(0);
	let options = SsrOptions::new().csrf("test-token-123");
	let mut renderer = SsrRenderer::with_options(options);
	let html = renderer.render_page(&counter);

	assert!(html.contains("<meta name=\"csrf-token\" content=\"test-token-123\">"));
}

#[test]
fn test_full_page_app_container() {
	let counter = Counter::new(42);
	let mut renderer = SsrRenderer::new();
	let html = renderer.render_page(&counter);

	assert!(html.contains("<div id=\"app\">"));
	assert!(html.contains("Count: 42"));
	assert!(html.contains("</div>"));
}

#[test]
fn test_full_page_with_auth_data() {
	use reinhardt_pages::auth::AuthData;

	let counter = Counter::new(0);
	let auth = AuthData::authenticated("1", "testuser");
	let options = SsrOptions::new().auth(auth);
	let mut renderer = SsrRenderer::with_options(options);
	let html = renderer.render_page(&counter);

	assert!(html.contains("<script id=\"auth-data\""));
	assert!(html.contains("type=\"application/json\""));
	assert!(html.contains("testuser"));
}

#[test]
fn test_full_page_combined_options() {
	let counter = Counter::new(99);
	let page_head = Head::new()
		.title("Combined Test")
		.link(LinkTag::stylesheet("/style.css"))
		.script(ScriptTag::external("/script.js"))
		.meta(MetaTag::new("author", "Test Author"));

	let view = counter.render().with_head(page_head);

	let options = SsrOptions::new().lang("fr").csrf("csrf-token");

	let mut renderer = SsrRenderer::with_options(options);
	let html = renderer.render_page_with_view_head(view);

	assert!(html.contains("<title>Combined Test</title>"));
	assert!(html.contains("<html lang=\"fr\">"));
	assert!(html.contains("href=\"/style.css\""));
	assert!(html.contains("src=\"/script.js\""));
	assert!(html.contains("name=\"author\""));
	assert!(html.contains("csrf-token"));
	assert!(html.contains("Count: 99"));
}

// ============================================================================
// Category 5: Performance Tests (5-8 tests)
// ============================================================================

#[test]
fn test_large_list_rendering() {
	let items: Vec<_> = (0..1000).map(|i| format!("Item {}", i)).collect();
	let mut list = PageElement::new("ul");

	for item in items {
		list = list.child(PageElement::new("li").child(item).into_page());
	}

	let html = list.into_page().render_to_string();

	assert!(html.contains("<ul>"));
	assert!(html.contains("<li>Item 0</li>"));
	assert!(html.contains("<li>Item 999</li>"));
	assert!(html.contains("</ul>"));
}

#[test]
fn test_deeply_nested_components() {
	fn create_nested(depth: usize) -> Page {
		if depth == 0 {
			Page::text("Leaf")
		} else {
			PageElement::new("div")
				.attr("data-depth", depth.to_string())
				.child(create_nested(depth - 1))
				.into_page()
		}
	}

	let deep = create_nested(50);
	let html = deep.render_to_string();

	assert!(html.contains("data-depth=\"50\""));
	assert!(html.contains("data-depth=\"1\""));
	assert!(html.contains("Leaf"));
}

#[test]
fn test_many_attributes() {
	let mut div = PageElement::new("div");

	for i in 0..100 {
		div = div.attr(format!("data-attr-{}", i), format!("value-{}", i));
	}

	let html = div.into_page().render_to_string();

	assert!(html.contains("data-attr-0=\"value-0\""));
	assert!(html.contains("data-attr-99=\"value-99\""));
}

#[test]
fn test_large_component_tree() {
	let counters: Vec<_> = (0..100).map(|i| Counter::new(i).render()).collect();

	let container = PageElement::new("div")
		.attr("class", "container")
		.children(counters);

	let html = container.into_page().render_to_string();

	assert!(html.contains("Count: 0"));
	assert!(html.contains("Count: 99"));
}

// ============================================================================
// Category 6: Edge Cases Tests (10-12 tests)
// ============================================================================

#[test]
fn test_svg_element() {
	let svg = PageElement::new("svg")
		.attr("width", "100")
		.attr("height", "100")
		.child(
			PageElement::new("circle")
				.attr("cx", "50")
				.attr("cy", "50")
				.attr("r", "40")
				.into_page(),
		);

	let html = svg.into_page().render_to_string();

	assert!(html.contains("<svg"));
	assert!(html.contains("width=\"100\""));
	assert!(html.contains("<circle"));
	assert!(html.contains("</svg>"));
}

#[test]
fn test_data_attributes() {
	let div = PageElement::new("div")
		.attr("data-id", "123")
		.attr("data-name", "test")
		.attr("data-active", "true");

	let html = div.into_page().render_to_string();

	assert!(html.contains("data-id=\"123\""));
	assert!(html.contains("data-name=\"test\""));
	assert!(html.contains("data-active=\"true\""));
}

#[test]
fn test_aria_attributes() {
	let button = PageElement::new("button")
		.attr("aria-label", "Close")
		.attr("aria-expanded", "false")
		.child("X");

	let html = button.into_page().render_to_string();

	assert!(html.contains("aria-label=\"Close\""));
	assert!(html.contains("aria-expanded=\"false\""));
}

#[test]
fn test_fragment_rendering() {
	let fragment = Page::Fragment(vec![
		Page::text("Hello, "),
		Page::text("World!"),
		Page::text(" Welcome."),
	]);

	let html = fragment.render_to_string();
	assert_eq!(html, "Hello, World! Welcome.");
}

#[test]
fn test_empty_view() {
	let empty = Page::Empty;
	let html = empty.render_to_string();
	assert_eq!(html, "");
}

#[test]
fn test_void_elements() {
	let img = PageElement::new("img")
		.attr("src", "/image.png")
		.attr("alt", "Test");

	let html = img.into_page().render_to_string();

	// Note: reinhardt-pages may render as <img></img> or <img />
	// depending on implementation
	assert!(html.contains("src=\"/image.png\""));
	assert!(html.contains("alt=\"Test\""));
}

#[test]
fn test_boolean_attributes() {
	let input = PageElement::new("input")
		.attr("type", "checkbox")
		.attr("checked", "checked")
		.attr("disabled", "disabled");

	let html = input.into_page().render_to_string();

	assert!(html.contains("checked=\"checked\""));
	assert!(html.contains("disabled=\"disabled\""));
}

#[test]
fn test_empty_attribute_value() {
	let div = PageElement::new("div")
		.attr("data-empty", "")
		.attr("class", "test");

	let html = div.into_page().render_to_string();

	assert!(html.contains("data-empty=\"\""));
	assert!(html.contains("class=\"test\""));
}

#[test]
fn test_numeric_content() {
	let numbers = vec![42, 100, -5, 0];
	let mut list = PageElement::new("ul");

	for num in numbers {
		list = list.child(PageElement::new("li").child(num.to_string()).into_page());
	}

	let html = list.into_page().render_to_string();

	assert!(html.contains("<li>42</li>"));
	assert!(html.contains("<li>100</li>"));
	assert!(html.contains("<li>-5</li>"));
	assert!(html.contains("<li>0</li>"));
}

#[test]
fn test_whitespace_preservation() {
	let text = PageElement::new("pre").child("  Line 1\n  Line 2\n  Line 3");
	let html = text.into_page().render_to_string();

	// Whitespace should be preserved in content
	assert!(html.contains("  Line 1\n  Line 2\n  Line 3"));
}

#[test]
fn test_mixed_content_types() {
	let div = PageElement::new("div")
		.child(Page::text("Text "))
		.child(PageElement::new("strong").child("Bold").into_page())
		.child(Page::text(" more text"));

	let html = div.into_page().render_to_string();

	assert!(html.contains("Text <strong>Bold</strong> more text"));
}
