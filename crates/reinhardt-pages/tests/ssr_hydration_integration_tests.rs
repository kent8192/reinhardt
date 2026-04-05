#![cfg(not(target_arch = "wasm32"))]
//! Integration tests for SSR and Hydration
//!
//! These tests verify the Server-Side Rendering and Client-Side Hydration flow:
//! 1. Components render to HTML strings correctly
//! 2. SSR state is serialized and can be restored
//! 3. Hydration markers are properly embedded
//! 4. View tree serialization works correctly

use reinhardt_pages::component::{Component, IntoPage, Page, PageElement};
use reinhardt_pages::ssr::{SsrOptions, SsrRenderer, SsrState};
use serde::de::DeserializeOwned;

/// Test component for SSR
struct Counter {
	initial: i32,
}

impl Counter {
	fn new(initial: i32) -> Self {
		Self { initial }
	}
}

impl Component for Counter {
	fn render(&self) -> Page {
		PageElement::new("div")
			.attr("class", "counter")
			.child(
				PageElement::new("span")
					.attr("data-count", self.initial.to_string())
					.child(format!("Count: {}", self.initial))
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

/// User card test component
struct UserCard {
	name: String,
	email: String,
}

impl UserCard {
	fn new(name: impl Into<String>, email: impl Into<String>) -> Self {
		Self {
			name: name.into(),
			email: email.into(),
		}
	}
}

impl Component for UserCard {
	fn render(&self) -> Page {
		PageElement::new("article")
			.attr("class", "user-card")
			.child(PageElement::new("h2").child(self.name.clone()).into_page())
			.child(
				PageElement::new("p")
					.attr("class", "email")
					.child(self.email.clone())
					.into_page(),
			)
			.into_page()
	}

	fn name() -> &'static str {
		"UserCard"
	}
}

/// Success Criterion 1: Components render to HTML strings
#[test]
fn test_component_render_to_string() {
	let counter = Counter::new(42);
	let html = counter.render().render_to_string();

	assert!(html.contains("class=\"counter\""));
	assert!(html.contains("data-count=\"42\""));
	assert!(html.contains("Count: 42"));
	assert!(html.contains("<button"));
	assert!(html.contains("Increment"));
}

/// Success Criterion 1: Nested components render correctly
#[test]
fn test_nested_component_render() {
	let card = UserCard::new("Alice", "alice@example.com");
	let html = card.render().render_to_string();

	assert!(html.contains("class=\"user-card\""));
	assert!(html.contains("<h2>Alice</h2>"));
	assert!(html.contains("class=\"email\""));
	assert!(html.contains("alice@example.com"));
}

/// Helper function to get and deserialize a signal value
fn get_signal_as<T: DeserializeOwned>(state: &SsrState, key: &str) -> Option<T> {
	state
		.get_signal(key)
		.and_then(|v| serde_json::from_value(v.clone()).ok())
}

/// Success Criterion 2: SSR state serialization
#[test]
fn test_ssr_state_serialization() {
	let mut state = SsrState::new();

	// Add signal value
	state.add_signal("count", serde_json::json!(42));
	state.add_signal("name", serde_json::json!("Alice"));

	// Serialize to JSON
	let json = state.to_json().expect("Serialization failed");

	// Verify JSON structure
	assert!(json.contains("42"));
	assert!(json.contains("Alice"));

	// Test round-trip
	let restored = SsrState::from_json(&json).expect("Deserialization failed");
	assert_eq!(get_signal_as::<i32>(&restored, "count"), Some(42));
	assert_eq!(
		get_signal_as::<String>(&restored, "name"),
		Some("Alice".to_string())
	);
}

/// Success Criterion 2: SSR state with complex values
#[test]
fn test_ssr_state_complex_values() {
	let mut state = SsrState::new();

	// Add array value
	state.add_signal("items", serde_json::json!(["a", "b", "c"]));

	// Add object value
	state.add_signal(
		"user",
		serde_json::json!({
			"name": "Bob",
			"age": 30
		}),
	);

	let json = state.to_json().expect("Serialization failed");
	let restored = SsrState::from_json(&json).expect("Deserialization failed");

	let items: Option<Vec<String>> = get_signal_as(&restored, "items");
	assert_eq!(
		items,
		Some(vec!["a".to_string(), "b".to_string(), "c".to_string()])
	);
}

/// Success Criterion 3: SSR renderer with hydration markers
#[test]
fn test_ssr_renderer_with_hydration_markers() {
	let counter = Counter::new(10);

	let options = SsrOptions::new();

	let mut renderer = SsrRenderer::with_options(options);
	// Use render_with_marker to get hydration markers
	let html = renderer.render_with_marker(&counter);

	// Should contain hydration marker
	assert!(html.contains("data-rh-id"));
	// Should contain component content
	assert!(html.contains("Count: 10"));
}

/// Success Criterion 3: SSR renderer without hydration markers
#[test]
fn test_ssr_renderer_without_hydration_markers() {
	let counter = Counter::new(5);

	// Use no_hydration() to disable hydration markers
	let options = SsrOptions::new().no_hydration();

	let mut renderer = SsrRenderer::with_options(options);
	// render_with_marker respects the no_hydration option
	let html = renderer.render_with_marker(&counter);

	// Should NOT contain hydration marker when disabled
	assert!(!html.contains("data-rh-id"));
	// Should still contain component content
	assert!(html.contains("Count: 5"));
}

/// Success Criterion 4: View fragment rendering
#[test]
fn test_view_fragment_rendering() {
	let fragment = Page::Fragment(vec![Page::text("Hello, "), Page::text("World!")]);

	let html = fragment.render_to_string();
	assert_eq!(html, "Hello, World!");
}

/// Success Criterion 4: View empty rendering
#[test]
fn test_view_empty_rendering() {
	let empty = Page::Empty;
	let html = empty.render_to_string();
	assert_eq!(html, "");
}

/// Integration test: Full SSR flow with state
#[test]
fn test_full_ssr_flow() {
	// 1. Create component
	let counter = Counter::new(100);

	// 2. Create SSR state
	let mut state = SsrState::new();
	state.add_signal("initial_count", serde_json::json!(100));

	// 3. Render component
	let mut renderer = SsrRenderer::new();
	let html = renderer.render(&counter);

	// 4. Serialize state
	let state_json = state.to_json().expect("State serialization failed");

	// 5. Create script tag for hydration (pure JSON, non-executable)
	let script = format!(
		r#"<script id="ssr-state" type="application/json">{}</script>"#,
		state_json
	);

	// 6. Combine into full page
	let page = format!("{}{}", script, html);

	// Verify page structure
	assert!(page.contains(r#"type="application/json""#));
	assert!(page.contains("100"));
	assert!(page.contains("Count: 100"));
}

/// Integration test: Multiple components rendering
#[test]
fn test_multiple_components_rendering() {
	let components: Vec<Box<dyn Component>> = vec![
		Box::new(Counter::new(1)),
		Box::new(Counter::new(2)),
		Box::new(UserCard::new("Test", "test@example.com")),
	];

	let mut html = String::new();
	for component in &components {
		html.push_str(&component.render().render_to_string());
	}

	assert!(html.contains("Count: 1"));
	assert!(html.contains("Count: 2"));
	assert!(html.contains("Test"));
	assert!(html.contains("test@example.com"));
}

/// Test SSR state script tag generation
#[test]
fn test_ssr_state_script_tag() {
	let mut state = SsrState::new();
	state.add_signal("test", serde_json::json!(42));

	// to_script_tag returns String directly
	let script = state.to_script_tag();

	assert!(script.starts_with(r#"<script id="ssr-state" type="application/json">"#));
	assert!(script.ends_with("</script>"));
	assert!(!script.contains("window."));
	assert!(script.contains("42"));
}
