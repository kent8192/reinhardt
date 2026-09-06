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
	Component, ControlBinding, Head, IntoPage, LinkTag, MetaTag, Page, PageElement, ScriptTag,
};
use reinhardt_pages::page;
use reinhardt_pages::reactive::{ReactiveScope, Signal};
use reinhardt_pages::ssr::{HydrationStrategy, SsrOptions, SsrRenderer};
use rstest::rstest;
use std::cell::Cell;
use std::rc::Rc;

fn signal_in_scope<T: 'static>(scope: &ReactiveScope, value: T) -> Signal<T> {
	scope.enter(|| Signal::new(value))
}

fn has_managed_head_entry(html: &str, tag: &str, semantic_content: &str) -> bool {
	let marker_prefix = format!("<{tag} data-reinhardt-head=\"");
	html.lines()
		.any(|line| line.starts_with(&marker_prefix) && line.contains(semantic_content))
}

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
	let items = ["Apple", "Banana", "Cherry"];
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

#[tokio::test]
async fn ssr_renderer_omits_falsy_reactive_boolean_attributes() {
	// Arrange
	let view = PageElement::new("button")
		.reactive_attr("DISABLED", || Some("false".into()))
		.into_page();
	let mut renderer = SsrRenderer::new();

	// Act
	let html = renderer.render_view(&view).await;

	// Assert
	assert_eq!(html, "<button></button>");
}

#[tokio::test]
async fn ssr_renderer_rejects_incompatible_reactive_control_shapes() {
	// Arrange
	let scope = ReactiveScope::new();
	let (input, select) = scope.enter(|| {
		let input = PageElement::new("input")
			.attr("type", "text")
			.reactive_attr("type", || Some("file".into()))
			.control_binding(ControlBinding::text(Signal::new("secret".to_owned())))
			.into_page();
		let select = PageElement::new("select")
			.attr("multiple", "multiple")
			.reactive_attr("multiple", || Some("false".into()))
			.control_binding(ControlBinding::select_many(Signal::new(vec![
				"rust".to_owned(),
			])))
			.into_page();
		(input, select)
	});
	let mut renderer = SsrRenderer::new();

	// Act
	let input_html = renderer.render_view(&input).await;
	let select_html = renderer.render_view(&select).await;

	// Assert
	assert_eq!(input_html, "<input type=\"text\" value=\"secret\" />");
	assert_eq!(select_html, "<select multiple=\"multiple\"></select>");
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

#[tokio::test]
async fn test_hydration_marker_enabled() {
	let counter = Counter::new(10);
	let options = SsrOptions::new();
	let mut renderer = SsrRenderer::with_options(options);
	let html = renderer.render_with_marker(&counter).await;

	assert!(html.contains("data-rh-id"));
	assert!(html.contains("data-rh-component=\"Counter\""));
}

#[tokio::test]
async fn test_hydration_marker_disabled() {
	let counter = Counter::new(5);
	let options = SsrOptions::new().no_hydration();
	let mut renderer = SsrRenderer::with_options(options);
	let html = renderer.render_with_marker(&counter).await;

	assert!(!html.contains("data-rh-id"));
	assert!(html.contains("Count: 5"));
}

#[tokio::test]
async fn test_hydration_marker_component_name() {
	let card = UserCard::new("Test", "test@example.com");
	let mut renderer = SsrRenderer::new();
	let html = renderer.render_with_marker(&card).await;

	assert!(html.contains("data-rh-component=\"UserCard\""));
}

#[tokio::test]
async fn test_hydration_marker_wraps_content() {
	let counter = Counter::new(42);
	let mut renderer = SsrRenderer::new();
	let html = renderer.render_with_marker(&counter).await;

	assert!(html.starts_with("<div"));
	assert!(html.ends_with("</div>"));
	assert!(html.contains("Count: 42"));
}

#[tokio::test]
async fn test_multiple_components_different_markers() {
	let counter1 = Counter::new(1);
	let counter2 = Counter::new(2);

	let mut renderer = SsrRenderer::new();
	let html1 = renderer.render_with_marker(&counter1).await;
	let html2 = renderer.render_with_marker(&counter2).await;

	// Both should have markers but with different IDs
	assert_eq!(html1.matches(r#"data-rh-id="rh-0""#).count(), 1);
	assert_eq!(html2.matches(r#"data-rh-id="rh-1""#).count(), 1);
}

// ============================================================================
// Category 4: Full Page Rendering Tests (15-20 tests)
// ============================================================================

#[tokio::test]
async fn test_full_page_doctype() {
	let counter = Counter::new(0);
	let mut renderer = SsrRenderer::new();
	let html = renderer.render_page_to_string(&counter).await;

	assert!(html.starts_with("<!DOCTYPE html>"));
}

#[tokio::test]
async fn test_full_page_html_structure() {
	let counter = Counter::new(0);
	let mut renderer = SsrRenderer::new();
	let html = renderer.render_page_to_string(&counter).await;

	assert!(html.contains("<html lang=\"en\">"));
	assert!(html.contains("<head>"));
	assert!(html.contains("</head>"));
	assert!(html.contains("<body>"));
	assert!(html.contains("</body>"));
	assert!(html.ends_with("</html>"));
}

#[tokio::test]
async fn test_full_page_meta_charset() {
	let counter = Counter::new(0);
	let mut renderer = SsrRenderer::new();
	let html = renderer.render_page_to_string(&counter).await;

	assert!(html.contains("<meta charset=\"UTF-8\">"));
}

#[tokio::test]
async fn test_full_page_meta_viewport() {
	let counter = Counter::new(0);
	let mut renderer = SsrRenderer::new();
	let html = renderer.render_page_to_string(&counter).await;

	assert!(
		html.contains("<meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\">")
	);
}

#[tokio::test]
async fn test_full_page_custom_title() {
	let counter = Counter::new(0);
	let page_head = Head::new().title("Test Page");
	let view = counter.render().with_head(page_head);
	let mut renderer = SsrRenderer::new();
	let html = renderer.render_page_with_view_head_to_string(view).await;

	assert!(has_managed_head_entry(
		&html,
		"title",
		"\">Test Page</title>",
	));
}

#[rstest]
#[tokio::test]
async fn reused_renderer_resets_managed_head_slot_identity_per_request() {
	// Arrange
	let mut renderer = SsrRenderer::new();
	let first = Page::text("first").with_head(Head::new().title("First request"));
	let second = Page::text("second").with_head(Head::new().title("Second request"));

	// Act
	let first_html = renderer.render_page_with_view_head_to_string(first).await;
	let second_html = renderer.render_page_with_view_head_to_string(second).await;

	// Assert
	assert!(has_managed_head_entry(
		&first_html,
		"title",
		"\">First request</title>",
	));
	assert!(has_managed_head_entry(
		&second_html,
		"title",
		"\">Second request</title>",
	));
	assert!(!second_html.contains("First request"));
}

#[tokio::test]
async fn test_full_page_custom_lang() {
	let counter = Counter::new(0);
	let options = SsrOptions::new().lang("ja");
	let mut renderer = SsrRenderer::with_options(options);
	let html = renderer.render_page_to_string(&counter).await;

	assert!(html.contains("<html lang=\"ja\">"));
}

#[tokio::test]
async fn test_full_page_css_link() {
	let counter = Counter::new(0);
	let page_head = Head::new().link(LinkTag::stylesheet("/styles.css"));
	let view = counter.render().with_head(page_head);
	let mut renderer = SsrRenderer::new();
	let html = renderer.render_page_with_view_head_to_string(view).await;

	assert!(has_managed_head_entry(
		&html,
		"link",
		"rel=\"stylesheet\" href=\"/styles.css\"",
	));
}

#[tokio::test]
async fn test_full_page_multiple_css_links() {
	let counter = Counter::new(0);
	let page_head = Head::new()
		.link(LinkTag::stylesheet("/reset.css"))
		.link(LinkTag::stylesheet("/main.css"));
	let view = counter.render().with_head(page_head);
	let mut renderer = SsrRenderer::new();
	let html = renderer.render_page_with_view_head_to_string(view).await;

	assert!(has_managed_head_entry(
		&html,
		"link",
		"rel=\"stylesheet\" href=\"/reset.css\"",
	));
	assert!(has_managed_head_entry(
		&html,
		"link",
		"rel=\"stylesheet\" href=\"/main.css\"",
	));
}

#[tokio::test]
async fn test_full_page_js_script() {
	let counter = Counter::new(0);
	let page_head = Head::new().script(ScriptTag::external("/app.js"));
	let view = counter.render().with_head(page_head);
	let mut renderer = SsrRenderer::new();
	let html = renderer.render_page_with_view_head_to_string(view).await;

	assert!(has_managed_head_entry(
		&html,
		"script",
		"src=\"/app.js\"></script>",
	));
}

#[tokio::test]
async fn test_full_page_custom_meta_tags() {
	let counter = Counter::new(0);
	let page_head = Head::new()
		.meta(MetaTag::new("description", "Test page"))
		.meta(MetaTag::new("keywords", "test, rust, ssr"));
	let view = counter.render().with_head(page_head);
	let mut renderer = SsrRenderer::new();
	let html = renderer.render_page_with_view_head_to_string(view).await;

	assert!(has_managed_head_entry(
		&html,
		"meta",
		"name=\"description\" content=\"Test page\"",
	));
	assert!(has_managed_head_entry(
		&html,
		"meta",
		"name=\"keywords\" content=\"test, rust, ssr\"",
	));
}

#[tokio::test]
async fn test_full_page_csrf_token() {
	let counter = Counter::new(0);
	let options = SsrOptions::new().csrf("test-token-123");
	let mut renderer = SsrRenderer::with_options(options);
	let html = renderer.render_page_to_string(&counter).await;

	assert!(html.contains("<meta name=\"csrf-token\" content=\"test-token-123\">"));
}

#[tokio::test]
async fn test_full_page_app_container() {
	let counter = Counter::new(42);
	let mut renderer = SsrRenderer::new();
	let html = renderer.render_page_to_string(&counter).await;

	assert!(html.contains("<div id=\"app\">"));
	assert!(html.contains("Count: 42"));
	assert!(html.contains("</div>"));
}

#[tokio::test]
async fn test_full_page_with_auth_data() {
	use reinhardt_pages::auth::AuthData;

	let counter = Counter::new(0);
	let auth = AuthData::authenticated("1", "testuser");
	let options = SsrOptions::new().auth(auth);
	let mut renderer = SsrRenderer::with_options(options);
	let html = renderer.render_page_to_string(&counter).await;

	assert!(html.contains("<script id=\"auth-data\""));
	assert!(html.contains("type=\"application/json\""));
	assert!(html.contains("testuser"));
}

#[tokio::test]
async fn test_full_page_combined_options() {
	let counter = Counter::new(99);
	let page_head = Head::new()
		.title("Combined Test")
		.link(LinkTag::stylesheet("/style.css"))
		.script(ScriptTag::external("/script.js"))
		.meta(MetaTag::new("author", "Test Author"));

	let view = counter.render().with_head(page_head);

	let options = SsrOptions::new().lang("fr").csrf("csrf-token");

	let mut renderer = SsrRenderer::with_options(options);
	let html = renderer.render_page_with_view_head_to_string(view).await;

	assert!(has_managed_head_entry(
		&html,
		"title",
		"\">Combined Test</title>",
	));
	assert!(html.contains("<html lang=\"fr\">"));
	assert!(html.contains("href=\"/style.css\""));
	assert!(html.contains("src=\"/script.js\""));
	assert!(html.contains("name=\"author\""));
	assert!(html.contains("csrf-token"));
	assert!(html.contains("Count: 99"));
}

#[tokio::test]
async fn test_ssr_options_struct_literal_remains_exhaustive() {
	let options = SsrOptions {
		include_hydration_markers: true,
		minify: false,
		include_state_script: true,
		lang: "en".to_string(),
		csrf_token: None,
		auth_data: None,
		enable_partial_hydration: false,
		default_hydration_strategy: HydrationStrategy::Full,
		resource_timeout: std::time::Duration::from_secs(2),
		query_defaults: reinhardt_pages::reactive::QueryDefaults::default(),
		suspense_streaming: true,
		script_nonce: None,
	};

	let mut renderer = SsrRenderer::with_options(options);
	let html = renderer
		.render_page(&Counter::new(3))
		.await
		.collect_string()
		.await;

	assert!(html.contains("Count: 3"));
}

#[test]
fn ssr_query_retry_builder_order_preserves_the_same_nested_defaults() {
	let defaults = reinhardt_pages::reactive::QueryDefaults::new()
		.stale_time(std::time::Duration::from_secs(7))
		.gc_time(std::time::Duration::from_secs(11));
	let gate_then_defaults = SsrOptions::new()
		.query_retries(true)
		.query_defaults(defaults.clone());
	let defaults_then_gate = SsrOptions::new()
		.query_defaults(defaults)
		.query_retries(true);

	assert_eq!(
		gate_then_defaults.query_defaults,
		defaults_then_gate.query_defaults
	);
}

fn controlled_bindings_page(scope: &ReactiveScope) -> Page {
	let text = signal_in_scope(scope, "A&B".to_owned());
	let date = signal_in_scope(scope, "2026-08-31".to_owned());
	let datetime_local = signal_in_scope(scope, "2026-08-31T10:30".to_owned());
	let month = signal_in_scope(scope, "2026-08".to_owned());
	let week = signal_in_scope(scope, "2026-W36".to_owned());
	let time = signal_in_scope(scope, String::new());
	let checked = signal_in_scope(scope, true);
	let selected = signal_in_scope(scope, vec!["rust".to_owned(), "wasm".to_owned()]);

	page!({
		input {
			a11y: off,
			bind: text
		}
		textarea {
			a11y: off,
			bind: text
		}
		input {
			a11y: off,
			type: "date",
			bind: date
		}
		input {
			a11y: off,
			type: "datetime-local",
			bind: datetime_local
		}
		input {
			a11y: off,
			type: "month",
			bind: month
		}
		input {
			a11y: off,
			type: "week",
			bind: week
		}
		input {
			a11y: off,
			type: "time",
			bind: time
		}
		input {
			a11y: off,
			type: "checkbox",
			bind: checked
		}
		select {
			a11y: off,
			multiple: true,
			bind: selected,
			optgroup {
				option {
					value: "rust",
					"Rust"
				}
				option {
					value: "wasm",
					"WebAssembly"
				}
			}
		}
	})
}

#[rstest]
#[tokio::test]
async fn controlled_bindings_render_html_initial_state() {
	// Arrange
	let reactive_scope = ReactiveScope::new();
	let component = controlled_bindings_page(&reactive_scope);
	let mut renderer = SsrRenderer::new();

	// Act
	let html = renderer.render_page_into_page_to_string(component).await;

	// Assert
	let body = html
		.split_once("<body>")
		.unwrap()
		.1
		.split_once("</body>")
		.unwrap()
		.0
		.strip_prefix("\n<div id=\"app\">")
		.unwrap()
		.strip_suffix("</div>\n")
		.unwrap();
	assert_eq!(
		body,
		concat!(
			"<input value=\"A&amp;B\" />",
			"<textarea>A&amp;B</textarea>",
			"<input type=\"date\" value=\"2026-08-31\" />",
			"<input type=\"datetime-local\" value=\"2026-08-31T10:30\" />",
			"<input type=\"month\" value=\"2026-08\" />",
			"<input type=\"week\" value=\"2026-W36\" />",
			"<input type=\"time\" value=\"\" />",
			"<input type=\"checkbox\" checked=\"checked\" />",
			"<select multiple=\"multiple\"><optgroup>",
			"<option value=\"rust\" selected=\"selected\">Rust</option>",
			"<option value=\"wasm\" selected=\"selected\">WebAssembly</option>",
			"</optgroup></select>"
		)
	);
}

#[rstest]
#[tokio::test]
async fn bound_radio_value_expression_is_evaluated_once_for_attribute_and_binding() {
	// Arrange
	let reactive_scope = ReactiveScope::new();
	let selected = signal_in_scope(&reactive_scope, "first".to_owned());
	let evaluations = Rc::new(Cell::new(0));
	let value_evaluations = Rc::clone(&evaluations);
	let component = page!({
		input {
			a11y: off,
			type: "radio",
			value: {
				let count = value_evaluations.get() + 1;
				value_evaluations.set(count);
				if count == 1 { "first" } else { "second" }
			},
			bind: selected
		}
	});
	let mut renderer = SsrRenderer::new();

	// Act
	let html = renderer.render_page_into_page_to_string(component).await;

	// Assert
	assert_eq!(evaluations.get(), 1);
	let body = html
		.split_once("<body>")
		.unwrap()
		.1
		.split_once("</body>")
		.unwrap()
		.0
		.strip_prefix("\n<div id=\"app\">")
		.unwrap()
		.strip_suffix("</div>\n")
		.unwrap();
	assert_eq!(
		body,
		"<input type=\"radio\" value=\"first\" checked=\"checked\" />"
	);
}

#[rstest]
#[tokio::test]
async fn manual_bound_radio_projects_its_binding_value() {
	// Arrange
	let reactive_scope = ReactiveScope::new();
	let selected = signal_in_scope(&reactive_scope, "draft".to_owned());
	let component = PageElement::new("input")
		.attr("type", "radio")
		.attr("value", "stale")
		.control_binding(ControlBinding::radio(selected, "draft".to_owned()))
		.into_page();
	let mut renderer = SsrRenderer::new();

	// Act
	let html = renderer.render_page_into_page_to_string(component).await;

	// Assert
	assert!(html.contains("type=\"radio\" value=\"draft\" checked=\"checked\""));
}

#[rstest]
#[tokio::test]
async fn controlled_single_select_marks_only_the_first_duplicate_in_tree_order() {
	// Arrange
	let reactive_scope = ReactiveScope::new();
	let selected = signal_in_scope(&reactive_scope, "duplicate".to_owned());
	let component = PageElement::new("select")
		.control_binding(ControlBinding::select_one(selected))
		.child(
			PageElement::new("option")
				.attr("value", "duplicate")
				.child("Before"),
		)
		.child(
			PageElement::new("optgroup")
				.child(
					PageElement::new("option")
						.attr("value", "duplicate")
						.child("Inside"),
				)
				.child(
					PageElement::new("option")
						.attr("value", "duplicate")
						.child("Inside after"),
				),
		)
		.child(
			PageElement::new("option")
				.attr("value", "duplicate")
				.child("After"),
		)
		.into_page();
	let mut buffered_renderer = SsrRenderer::new();
	let mut streaming_renderer = SsrRenderer::new();

	// Act
	let buffered = buffered_renderer
		.render_page_into_page_to_string(component.clone())
		.await;
	let streaming = streaming_renderer
		.render_page_into_page(component)
		.await
		.collect_string()
		.await;

	// Assert
	assert_eq!(streaming, buffered);
	assert_eq!(buffered.matches("selected=\"selected\"").count(), 1);
	assert!(buffered.contains("<option value=\"duplicate\" selected=\"selected\">Before</option>"));
}

#[rstest]
#[tokio::test]
async fn controlled_multiple_select_marks_every_duplicate() {
	// Arrange
	let reactive_scope = ReactiveScope::new();
	let selected = signal_in_scope(&reactive_scope, vec!["duplicate".to_owned()]);
	let component = PageElement::new("select")
		.bool_attr("multiple", true)
		.control_binding(ControlBinding::select_many(selected))
		.child(
			PageElement::new("option")
				.attr("value", "duplicate")
				.child("First"),
		)
		.child(
			PageElement::new("optgroup").child(
				PageElement::new("option")
					.attr("value", "duplicate")
					.child("Second"),
			),
		)
		.into_page();
	let mut renderer = SsrRenderer::new();

	// Act
	let html = renderer.render_page_into_page_to_string(component).await;

	// Assert
	assert_eq!(html.matches("selected=\"selected\"").count(), 2);
}

#[rstest]
#[tokio::test]
async fn controlled_select_uses_flattened_option_text_when_value_is_omitted() {
	// Arrange
	let reactive_scope = ReactiveScope::new();
	let selected = signal_in_scope(
		&reactive_scope,
		vec![
			"Rust ignored & WebAssembly".to_owned(),
			"Nested\u{a0}<Choice>ignored".to_owned(),
		],
	);
	let component = PageElement::new("select")
		.bool_attr("multiple", true)
		.control_binding(ControlBinding::select_many(selected))
		.child(
			PageElement::new("optgroup")
				.child(
					PageElement::new("option")
						.child(" \tRust\n")
						.child(PageElement::new("script").child("ignored"))
						.child("  &\r\nWebAssembly\x0c "),
				)
				.child(PageElement::new("option").child(Page::Fragment(vec![
					Page::text(" Nested\u{a0}"),
					PageElement::new("span").child("<Choice>").into_page(),
					PageElement::new("script").child("ignored").into_page(),
					Page::text(" "),
				]))),
		)
		.into_page();
	let mut buffered_renderer =
		SsrRenderer::with_options(SsrOptions::new().suspense_streaming(false));
	let mut streaming_renderer = SsrRenderer::new();

	// Act
	let html = buffered_renderer
		.render_page_into_page_to_string(component.clone())
		.await;
	let streaming = streaming_renderer
		.render_page_into_page(component)
		.await
		.collect_string()
		.await;

	// Assert
	assert_eq!(streaming, html);
	let body = html
		.split_once("<body>")
		.unwrap()
		.1
		.split_once("</body>")
		.unwrap()
		.0
		.strip_prefix("\n<div id=\"app\">")
		.unwrap()
		.strip_suffix("</div>\n")
		.unwrap();
	assert_eq!(
		body,
		concat!(
			"<select multiple=\"multiple\"><optgroup>",
			"<option selected=\"selected\"> \tRust\n<span>ignored</span>  &amp;\r\nWebAssembly\x0c </option>",
			"<option selected=\"selected\"> Nested\u{a0}<span>&lt;Choice&gt;</span><span>ignored</span> </option>",
			"</optgroup></select>"
		)
	);
}

#[rstest]
#[tokio::test]
async fn controlled_bindings_have_buffered_streaming_byte_parity() {
	// Arrange
	let reactive_scope = ReactiveScope::new();
	let component = controlled_bindings_page(&reactive_scope);
	let mut buffered_renderer = SsrRenderer::new();
	let mut streaming_renderer = SsrRenderer::new();

	// Act
	let buffered = buffered_renderer
		.render_page_into_page_to_string(component.clone())
		.await;
	let streaming = streaming_renderer
		.render_page_into_page(component)
		.await
		.collect_string()
		.await;

	// Assert
	assert_eq!(streaming, buffered);
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

#[tokio::test]
async fn ssr_omits_unsafe_batch_boolean_attribute_names() {
	struct BooleanAttributeComponent;

	impl Component for BooleanAttributeComponent {
		fn render(&self) -> Page {
			PageElement::new("button")
				.with_bool_attrs([("disabled", true), ("x=\" onmouseover=\"alert(1)", true)])
				.into_page()
		}

		fn name() -> &'static str {
			"BooleanAttributeComponent"
		}
	}

	let mut renderer = SsrRenderer::new();
	let html = renderer
		.render_page_to_string(&BooleanAttributeComponent)
		.await;

	assert!(html.contains("disabled=\"disabled\""));
	assert!(!html.contains("onmouseover"));
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
