#![cfg(not(target_arch = "wasm32"))]
//! Head System integration tests
//!
//! Success Criteria:
//! 1. Head type builds correctly with all builder methods
//! 2. head! macro generates correct Head instances
//! 3. View::WithHead correctly attaches head to views
//! 4. find_topmost_head correctly traverses view tree
//! 5. render_to_string handles WithHead variant
//! 6. resolve_static returns correct URLs
//!
//! Test Categories:
//! - Happy Path: 6 tests
//! - Edge Cases: 4 tests
//! - Combination: 3 tests
//!
//! Total: 13 tests

use reinhardt_pages::component::{Head, IntoPage, LinkTag, MetaTag, Page, PageElement, ScriptTag};
use reinhardt_pages::head;
use rstest::*;

// ============================================================================
// Happy Path Tests
// ============================================================================

/// Tests that Head::new() creates an empty head.
#[rstest]
fn test_head_new_creates_empty_head() {
	let head = Head::new();

	assert!(head.title.is_none());
	assert!(head.meta_tags.is_empty());
	assert!(head.links.is_empty());
	assert!(head.scripts.is_empty());
	assert!(head.styles.is_empty());
}

/// Tests that Head builder methods work correctly.
#[rstest]
fn test_head_builder_methods() {
	let head = Head::new()
		.title("Test Page")
		.meta(MetaTag::new("description", "A test page"))
		.link(LinkTag::new("stylesheet", "/style.css"))
		.script(ScriptTag::external("/app.js"));

	assert_eq!(head.title.as_ref().map(|s| s.as_ref()), Some("Test Page"));
	assert_eq!(head.meta_tags.len(), 1);
	assert_eq!(head.links.len(), 1);
	assert_eq!(head.scripts.len(), 1);
}

/// Tests that Head::to_html generates correct HTML.
#[rstest]
fn test_head_to_html() {
	let head = Head::new()
		.title("My Page")
		.meta(MetaTag::new("description", "Page description"));

	let html = head.to_html();

	assert!(html.contains("<title>My Page</title>"));
	assert!(html.contains("<meta name=\"description\" content=\"Page description\""));
}

/// Tests that head! macro creates Head correctly.
#[rstest]
fn test_head_macro_basic() {
	let page_head = head!(|| {
		title { "Macro Page" }
	});

	assert_eq!(
		page_head.title.as_ref().map(|s| s.as_ref()),
		Some("Macro Page")
	);
}

/// Tests that View::with_head attaches head correctly.
#[rstest]
fn test_view_with_head() {
	let view = Page::text("Hello");
	let head = Head::new().title("Test");

	let view_with_head = view.with_head(head);

	// Verify it's a WithHead variant
	assert!(view_with_head.extract_head().is_some());
	assert_eq!(
		view_with_head
			.extract_head()
			.unwrap()
			.title
			.as_ref()
			.map(|s| s.as_ref()),
		Some("Test")
	);
}

/// Tests that render_to_string handles WithHead correctly.
#[rstest]
fn test_render_to_string_with_head() {
	let view = PageElement::new("div")
		.child("Content")
		.into_page()
		.with_head(Head::new().title("Test"));

	let html = view.render_to_string();

	// WithHead should render the inner view content, not the head
	assert!(html.contains("<div>Content</div>"));
	// Head should not appear in the body rendering
	assert!(!html.contains("<title>"));
}

// ============================================================================
// Edge Case Tests
// ============================================================================

/// Tests that extract_head returns None for non-WithHead views.
#[rstest]
fn test_extract_head_returns_none_for_non_withhead() {
	let element_view = PageElement::new("div").into_page();
	let text_view = Page::text("Hello");
	let empty_view = Page::empty();

	assert!(element_view.extract_head().is_none());
	assert!(text_view.extract_head().is_none());
	assert!(empty_view.extract_head().is_none());
}

/// Tests that find_topmost_head works with nested fragments.
#[rstest]
fn test_find_topmost_head_nested_fragment() {
	let inner_view = Page::text("Inner").with_head(Head::new().title("Inner Head"));

	let outer_view = Page::fragment(vec![Page::text("Before"), inner_view, Page::text("After")]);

	// Should find the inner head
	let found_head = outer_view.find_topmost_head();
	assert!(found_head.is_some());
	assert_eq!(
		found_head.unwrap().title.as_ref().map(|s| s.as_ref()),
		Some("Inner Head")
	);
}

/// Tests that find_topmost_head returns outermost head when nested.
#[rstest]
fn test_find_topmost_head_prefers_outer() {
	let inner_view = Page::text("Inner").with_head(Head::new().title("Inner Head"));

	let outer_view = Page::fragment(vec![inner_view]).with_head(Head::new().title("Outer Head"));

	// Should find the outer head first
	let found_head = outer_view.find_topmost_head();
	assert!(found_head.is_some());
	assert_eq!(
		found_head.unwrap().title.as_ref().map(|s| s.as_ref()),
		Some("Outer Head")
	);
}

/// Tests empty fragment has no head.
#[rstest]
fn test_empty_fragment_no_head() {
	let view = Page::fragment(Vec::<Page>::new());

	assert!(view.find_topmost_head().is_none());
}

// ============================================================================
// Combination Tests
// ============================================================================

/// Tests head! macro with multiple elements.
#[rstest]
fn test_head_macro_multiple_elements() {
	let page_head = head!(|| {
		title { "Full Page" }
		meta { name: "description", content: "A full page" }
		link { rel: "stylesheet", href: "/style.css" }
		script { src: "/app.js", defer }
	});

	assert_eq!(
		page_head.title.as_ref().map(|s| s.as_ref()),
		Some("Full Page")
	);
	assert_eq!(page_head.meta_tags.len(), 1);
	assert_eq!(page_head.links.len(), 1);
	assert_eq!(page_head.scripts.len(), 1);
}

/// Tests MetaTag variants.
#[rstest]
fn test_meta_tag_variants() {
	let charset = MetaTag::charset("UTF-8");
	let http_equiv = MetaTag::http_equiv("refresh", "5");
	let property = MetaTag::property("og:title", "OG Title");
	let name = MetaTag::new("author", "Test Author");

	assert!(charset.charset.is_some());
	assert!(http_equiv.http_equiv.is_some());
	assert!(property.property.is_some());
	assert!(name.name.is_some());
}

/// Tests ScriptTag variants.
#[rstest]
fn test_script_tag_variants() {
	let external = ScriptTag::external("/app.js");
	let module = ScriptTag::module("/app.mjs");
	let inline = ScriptTag::inline("console.log('hello');");

	assert!(external.src.is_some());
	// Module scripts have type_attr set to "module"
	assert_eq!(
		module.type_attr.as_ref().map(|s| s.as_ref()),
		Some("module")
	);
	assert!(inline.content.is_some());
}
