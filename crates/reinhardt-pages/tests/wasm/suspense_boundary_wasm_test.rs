//! SuspenseBoundary WASM Integration Tests
//!
//! These tests verify the WASM-specific `render_fallback` path of
//! `SuspenseBoundary`. They assert:
//!
//! - When a resource is loading: `data-rh-suspense="pending"` marker is present
//!   and fallback content is rendered.
//! - When a resource has resolved: `data-rh-suspense="resolved"` marker is
//!   present and actual content replaces the fallback.
//!
//! **Run with**: `wasm-pack test --headless --chrome`

#![cfg(wasm)]

use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

use reinhardt_pages::component::suspense::{ResourceTracker, SuspenseBoundary};
use reinhardt_pages::component::{IntoPage, Page, PageElement};

// ============================================================================
// Test Helpers
// ============================================================================

/// Minimal stub implementing `ResourceTracker` with a fixed loading state.
struct MockResource {
	loading: bool,
}

impl MockResource {
	fn loading() -> Self {
		Self { loading: true }
	}

	fn resolved() -> Self {
		Self { loading: false }
	}
}

impl ResourceTracker for MockResource {
	fn is_loading(&self) -> bool {
		self.loading
	}
}

// ============================================================================
// Pending State Tests — render_fallback() path
// ============================================================================

/// When a resource is loading, `render()` must use the WASM `render_fallback`
/// path: the wrapper div carries `data-rh-suspense="pending"`.
#[wasm_bindgen_test]
fn pending_state_has_data_rh_suspense_pending() {
	// Arrange
	let boundary = SuspenseBoundary::new()
		.fallback(|| Page::text("Loading..."))
		.track_custom(MockResource::loading())
		.content(|| PageElement::new("p").child("Content").into_page());

	// Act
	let html = boundary.render().render_to_string();

	// Assert — WASM render_fallback wraps fallback in data-rh-suspense="pending"
	assert!(
		html.contains("data-rh-suspense=\"pending\""),
		"expected data-rh-suspense=\"pending\" in: {html}"
	);
}

/// When a resource is loading, the fallback content must be visible in the
/// rendered HTML.
#[wasm_bindgen_test]
fn pending_state_shows_fallback_content() {
	// Arrange
	let boundary = SuspenseBoundary::new()
		.fallback(|| Page::text("Loading..."))
		.track_custom(MockResource::loading())
		.content(|| PageElement::new("p").child("Real content").into_page());

	// Act
	let html = boundary.render().render_to_string();

	// Assert
	assert!(
		html.contains("Loading..."),
		"expected fallback text in: {html}"
	);
}

/// When a resource is loading, the actual content must NOT appear in the
/// rendered HTML — the fallback takes precedence.
#[wasm_bindgen_test]
fn pending_state_does_not_show_content() {
	// Arrange
	let boundary = SuspenseBoundary::new()
		.fallback(|| Page::text("Loading..."))
		.track_custom(MockResource::loading())
		.content(|| PageElement::new("p").child("Real content").into_page());

	// Act
	let html = boundary.render().render_to_string();

	// Assert
	assert!(
		!html.contains("Real content"),
		"did not expect real content while loading, got: {html}"
	);
}

/// When a resource is loading, `data-rh-suspense="resolved"` must NOT appear.
#[wasm_bindgen_test]
fn pending_state_does_not_have_resolved_marker() {
	// Arrange
	let boundary = SuspenseBoundary::new()
		.fallback(|| Page::text("Loading..."))
		.track_custom(MockResource::loading())
		.content(|| Page::text("Content"));

	// Act
	let html = boundary.render().render_to_string();

	// Assert
	assert!(
		!html.contains("data-rh-suspense=\"resolved\""),
		"did not expect resolved marker while pending, got: {html}"
	);
}

/// Fallback element attributes (e.g. class="spinner") must be preserved inside
/// the `data-rh-suspense="pending"` wrapper.
#[wasm_bindgen_test]
fn pending_state_preserves_fallback_element_attributes() {
	// Arrange
	let boundary = SuspenseBoundary::new()
		.fallback(|| {
			PageElement::new("div")
				.attr("class", "spinner")
				.child("Spinning...")
				.into_page()
		})
		.track_custom(MockResource::loading())
		.content(|| Page::text("Done"));

	// Act
	let html = boundary.render().render_to_string();

	// Assert
	assert!(
		html.contains("class=\"spinner\""),
		"expected spinner class in fallback, got: {html}"
	);
	assert!(
		html.contains("Spinning..."),
		"expected spinner text in fallback, got: {html}"
	);
}

// ============================================================================
// Resolved State Tests — render_content() path
// ============================================================================

/// When all resources have resolved, the output must carry
/// `data-rh-suspense="resolved"`.
#[wasm_bindgen_test]
fn resolved_state_has_data_rh_suspense_resolved() {
	// Arrange
	let boundary = SuspenseBoundary::new()
		.fallback(|| Page::text("Loading..."))
		.track_custom(MockResource::resolved())
		.content(|| PageElement::new("p").child("Hello").into_page());

	// Act
	let html = boundary.render().render_to_string();

	// Assert
	assert!(
		html.contains("data-rh-suspense=\"resolved\""),
		"expected data-rh-suspense=\"resolved\" in: {html}"
	);
}

/// When all resources have resolved, the actual content must be rendered.
#[wasm_bindgen_test]
fn resolved_state_shows_content() {
	// Arrange
	let boundary = SuspenseBoundary::new()
		.fallback(|| Page::text("Loading..."))
		.track_custom(MockResource::resolved())
		.content(|| PageElement::new("p").child("Hello").into_page());

	// Act
	let html = boundary.render().render_to_string();

	// Assert
	assert!(
		html.contains("<p>Hello</p>"),
		"expected actual content in: {html}"
	);
}

/// When all resources have resolved, the fallback text must NOT appear.
#[wasm_bindgen_test]
fn resolved_state_does_not_show_fallback() {
	// Arrange
	let boundary = SuspenseBoundary::new()
		.fallback(|| Page::text("Loading..."))
		.track_custom(MockResource::resolved())
		.content(|| PageElement::new("p").child("Hello").into_page());

	// Act
	let html = boundary.render().render_to_string();

	// Assert
	assert!(
		!html.contains("Loading..."),
		"did not expect fallback text after resolve, got: {html}"
	);
}

/// Resolved state must not carry `data-rh-suspense="pending"`.
#[wasm_bindgen_test]
fn resolved_state_does_not_have_pending_marker() {
	// Arrange
	let boundary = SuspenseBoundary::new()
		.fallback(|| Page::text("Loading..."))
		.track_custom(MockResource::resolved())
		.content(|| Page::text("Content"));

	// Act
	let html = boundary.render().render_to_string();

	// Assert
	assert!(
		!html.contains("data-rh-suspense=\"pending\""),
		"did not expect pending marker after resolve, got: {html}"
	);
}

// ============================================================================
// State Transition Tests
// ============================================================================

/// Simulate the transition from loading → resolved: two separate renders
/// produce the correct markers at each phase.
#[wasm_bindgen_test]
fn state_transition_loading_then_resolved() {
	// Phase 1: loading
	let boundary_loading = SuspenseBoundary::new()
		.fallback(|| Page::text("Loading..."))
		.track_custom(MockResource::loading())
		.content(|| PageElement::new("p").child("Ready").into_page());

	let html_loading = boundary_loading.render().render_to_string();

	assert!(
		html_loading.contains("data-rh-suspense=\"pending\""),
		"phase 1: expected pending marker, got: {html_loading}"
	);
	assert!(
		html_loading.contains("Loading..."),
		"phase 1: expected fallback, got: {html_loading}"
	);
	assert!(
		!html_loading.contains("Ready"),
		"phase 1: did not expect content, got: {html_loading}"
	);

	// Phase 2: resolved
	let boundary_resolved = SuspenseBoundary::new()
		.fallback(|| Page::text("Loading..."))
		.track_custom(MockResource::resolved())
		.content(|| PageElement::new("p").child("Ready").into_page());

	let html_resolved = boundary_resolved.render().render_to_string();

	assert!(
		html_resolved.contains("data-rh-suspense=\"resolved\""),
		"phase 2: expected resolved marker, got: {html_resolved}"
	);
	assert!(
		html_resolved.contains("<p>Ready</p>"),
		"phase 2: expected content, got: {html_resolved}"
	);
	assert!(
		!html_resolved.contains("Loading..."),
		"phase 2: did not expect fallback, got: {html_resolved}"
	);
}

// ============================================================================
// Multiple Resources Tests
// ============================================================================

/// When ANY of several tracked resources is still loading, the pending path
/// must be taken (all-or-nothing rule).
#[wasm_bindgen_test]
fn multiple_resources_one_loading_shows_pending() {
	// Arrange — two resolved, one loading
	let boundary = SuspenseBoundary::new()
		.fallback(|| Page::text("Still loading..."))
		.track_custom(MockResource::resolved())
		.track_custom(MockResource::loading())
		.track_custom(MockResource::resolved())
		.content(|| Page::text("All done"));

	// Act
	let html = boundary.render().render_to_string();

	// Assert
	assert!(
		html.contains("data-rh-suspense=\"pending\""),
		"expected pending while any resource loads, got: {html}"
	);
	assert!(
		html.contains("Still loading..."),
		"expected fallback text, got: {html}"
	);
	assert!(
		!html.contains("All done"),
		"did not expect content while loading, got: {html}"
	);
}

/// When ALL resources have resolved, the resolved path must be taken.
#[wasm_bindgen_test]
fn multiple_resources_all_resolved_shows_content() {
	// Arrange
	let boundary = SuspenseBoundary::new()
		.fallback(|| Page::text("Loading..."))
		.track_custom(MockResource::resolved())
		.track_custom(MockResource::resolved())
		.track_custom(MockResource::resolved())
		.content(|| PageElement::new("main").child("All data ready").into_page());

	// Act
	let html = boundary.render().render_to_string();

	// Assert
	assert!(
		html.contains("data-rh-suspense=\"resolved\""),
		"expected resolved marker, got: {html}"
	);
	assert!(
		html.contains("All data ready"),
		"expected content, got: {html}"
	);
}

// ============================================================================
// into_page() Integration Test
// ============================================================================

/// `into_page()` must respect the WASM loading path the same way `render()`
/// does — pending resource → pending marker.
#[wasm_bindgen_test]
fn into_page_pending_resource_produces_pending_marker() {
	// Arrange
	let boundary = SuspenseBoundary::new()
		.fallback(|| Page::text("Loading via into_page"))
		.track_custom(MockResource::loading())
		.content(|| Page::text("Content via into_page"));

	// Act — into_page() calls render() internally
	let html = boundary.into_page().render_to_string();

	// Assert
	assert!(
		html.contains("data-rh-suspense=\"pending\""),
		"expected pending marker from into_page, got: {html}"
	);
	assert!(
		html.contains("Loading via into_page"),
		"expected fallback from into_page, got: {html}"
	);
}
