#![cfg(not(target_arch = "wasm32"))]
//! Integration tests for SSR and Hydration
//!
//! These tests verify the Server-Side Rendering and Client-Side Hydration flow:
//! 1. Components render to HTML strings correctly
//! 2. SSR state is serialized and can be restored
//! 3. Hydration markers are properly embedded
//! 4. View tree serialization works correctly

use reinhardt_pages::component::{Component, IntoPage, Page, PageElement};
use reinhardt_pages::reactive::entity::{
	Entity, EntityDependencies, EntityProjection, EntityReader, EntityValue, EntityWriter,
	ProjectionMaterialization, ProjectionRemoval, RemovedEntities,
};
use reinhardt_pages::reactive::{QueryFamily, QueryOptions, QueryStatus, use_query};
use reinhardt_pages::ssr::{SsrOptions, SsrRenderer, SsrState};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
struct HydratedProject {
	id: u64,
	name: String,
}

impl Entity for HydratedProject {
	type Id = u64;

	const TYPE: &'static str = "ssr_hydration_integration.project";

	fn entity_id(&self) -> Self::Id {
		self.id
	}
}

#[derive(Clone, Copy)]
struct MissingHydrationProjection;

impl EntityProjection<HydratedProject> for MissingHydrationProjection {
	type Recipe = u64;

	const SCHEMA: &'static str = "ssr_hydration_integration.missing-v1";

	fn normalize(&self, value: HydratedProject, _entities: &mut EntityWriter<'_>) -> Self::Recipe {
		value.id
	}

	fn dependencies(&self, recipe: &Self::Recipe, dependencies: &mut EntityDependencies) {
		dependencies.extend::<HydratedProject>([*recipe]);
	}

	fn materialize(
		&self,
		recipe: &Self::Recipe,
		entities: &EntityReader<'_>,
	) -> ProjectionMaterialization<HydratedProject> {
		entities.required::<HydratedProject>(recipe)
	}

	fn apply_removals(
		&self,
		_recipe: &mut Self::Recipe,
		_removed: &RemovedEntities<'_>,
	) -> ProjectionRemoval {
		ProjectionRemoval::Unchanged
	}
}

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

#[tokio::test]
async fn normalized_ssr_writes_one_reachable_entity_table() {
	let view = Page::reactive(|| {
		let query = use_query(
			QueryFamily::<u64, HydratedProject, String>::new("tests::ssr-normalized-table")
				.query(1, || async {
					Ok::<_, String>(HydratedProject {
						id: 7,
						name: "Ada".to_string(),
					})
				})
				.with_entities(EntityValue::new()),
			QueryOptions::default(),
		);
		match query.snapshot().status {
			QueryStatus::Success => PageElement::new("p").child("Ada").into_page(),
			QueryStatus::Idle | QueryStatus::Pending => {
				PageElement::new("p").child("loading").into_page()
			}
			QueryStatus::Error => PageElement::new("p").child("error").into_page(),
		}
	});

	let mut renderer = SsrRenderer::new();
	let html = renderer.render_view(&view).await;
	assert!(html.contains("Ada"));
	let table = renderer
		.state()
		.get_resource_state("pages.query-entities:v1")
		.expect("normalized SSR must emit a reserved entity table");
	assert_eq!(table["version"], serde_json::json!(1));
	assert_eq!(
		table["entities"][HydratedProject::TYPE]
			.as_array()
			.unwrap()
			.len(),
		1
	);
	assert_eq!(
		table["entities"][HydratedProject::TYPE][0]["id"],
		serde_json::json!(7)
	);
}

#[tokio::test]
async fn plain_query_ssr_wire_shape_remains_unchanged() {
	let family = QueryFamily::<(), String, String>::new("tests::ssr-plain-wire-shape");
	let hydration_id = Rc::new(RefCell::new(None));
	let hydration_id_for_view = Rc::clone(&hydration_id);
	let view = Page::reactive(move || {
		let query = use_query(
			family.query((), || async { Ok::<_, String>("plain-value".to_string()) }),
			QueryOptions::default(),
		);
		hydration_id_for_view
			.borrow_mut()
			.replace(query.ssr_key().to_string());
		PageElement::new("p")
			.child(query.data().unwrap_or_default())
			.into_page()
	});

	let mut renderer = SsrRenderer::new();
	renderer.render_view(&view).await;
	let hydration_id = hydration_id
		.borrow()
		.clone()
		.expect("plain query should register a hydration key");

	assert_eq!(
		renderer.state().get_resource_state(&hydration_id),
		Some(&serde_json::json!({
			"state": {"Success": "plain-value"},
			"refetch_error": null,
			"is_fetching": false,
			"is_stale": false,
		}))
	);
	assert_eq!(
		renderer
			.state()
			.get_resource_state("pages.query-entities:v1"),
		None
	);
}

#[tokio::test]
async fn normalized_ssr_entity_tables_are_isolated_between_requests() {
	fn view(value: HydratedProject) -> Page {
		let family = QueryFamily::<(), HydratedProject, String>::new("tests::ssr-isolated-table");
		Page::reactive(move || {
			let value = value.clone();
			let query = use_query(
				family
					.query((), move || {
						let value = value.clone();
						async move { Ok::<_, String>(value) }
					})
					.with_entities(EntityValue::new()),
				QueryOptions::default(),
			);
			PageElement::new("p")
				.child(query.data().map(|project| project.name).unwrap_or_default())
				.into_page()
		})
	}

	let mut first_renderer = SsrRenderer::new();
	let first_html = first_renderer
		.render_view(&view(HydratedProject {
			id: 1,
			name: "first-request".to_string(),
		}))
		.await;
	assert!(first_html.contains("first-request"));
	let first_table = first_renderer
		.state()
		.get_resource_state("pages.query-entities:v1")
		.expect("first request should emit an entity table");
	assert_eq!(
		first_table["entities"][HydratedProject::TYPE][0]["id"],
		serde_json::json!(1)
	);

	let mut second_renderer = SsrRenderer::new();
	let second_html = second_renderer
		.render_view(&view(HydratedProject {
			id: 2,
			name: "second-request".to_string(),
		}))
		.await;
	assert!(second_html.contains("second-request"));
	let second_table = second_renderer
		.state()
		.get_resource_state("pages.query-entities:v1")
		.expect("second request should emit an entity table");
	assert_eq!(
		second_table["entities"][HydratedProject::TYPE][0]["id"],
		serde_json::json!(2)
	);
}

#[tokio::test]
async fn missing_required_normalized_success_is_omitted_from_ssr_state() {
	let family = QueryFamily::<(), HydratedProject, String>::new("tests::ssr-missing-required");
	let hydration_id = Rc::new(RefCell::new(None));
	let hydration_id_for_view = Rc::clone(&hydration_id);
	let view = Page::reactive(move || {
		let query = use_query(
			family
				.query((), || async {
					Ok::<_, String>(HydratedProject {
						id: 9,
						name: "fallback".to_string(),
					})
				})
				.with_entities(MissingHydrationProjection),
			QueryOptions::default(),
		);
		hydration_id_for_view
			.borrow_mut()
			.replace(query.ssr_key().to_string());
		PageElement::new("p")
			.child(query.data().map(|project| project.name).unwrap_or_default())
			.into_page()
	});

	let mut renderer = SsrRenderer::new();
	let html = renderer.render_view(&view).await;
	let hydration_id = hydration_id
		.borrow()
		.clone()
		.expect("missing normalized query should still have a hydration key");
	assert!(html.contains("fallback"));
	assert_eq!(renderer.state().get_resource_state(&hydration_id), None);
	assert_eq!(
		renderer
			.state()
			.get_resource_state("pages.query-entities:v1"),
		None
	);
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
#[tokio::test]
async fn test_ssr_renderer_with_hydration_markers() {
	let counter = Counter::new(10);

	let options = SsrOptions::new();

	let mut renderer = SsrRenderer::with_options(options);
	// Use render_with_marker to get hydration markers
	let html = renderer.render_with_marker(&counter).await;

	// Should contain hydration marker
	assert!(html.contains("data-rh-id"));
	// Should contain component content
	assert!(html.contains("Count: 10"));
}

/// Regression test for #4972: hydration IDs must be deterministic per render.
#[tokio::test]
async fn test_hydration_marker_ids_reset_for_each_render_context() {
	let counter = Counter::new(7);
	let card = UserCard::new("Alice", "alice@example.com");

	let mut first_renderer = SsrRenderer::new();
	let first_counter = first_renderer.render_with_marker(&counter).await;
	let first_card = first_renderer.render_with_marker(&card).await;

	let mut second_renderer = SsrRenderer::new();
	let second_counter = second_renderer.render_with_marker(&counter).await;
	let second_card = second_renderer.render_with_marker(&card).await;

	assert_eq!(first_counter, second_counter);
	assert_eq!(first_card, second_card);
	assert_eq!(first_counter.matches(r#"data-rh-id="rh-0""#).count(), 1);
	assert_eq!(first_card.matches(r#"data-rh-id="rh-1""#).count(), 1);
}

/// Success Criterion 3: SSR renderer without hydration markers
#[tokio::test]
async fn test_ssr_renderer_without_hydration_markers() {
	let counter = Counter::new(5);

	// Use no_hydration() to disable hydration markers
	let options = SsrOptions::new().no_hydration();

	let mut renderer = SsrRenderer::with_options(options);
	// render_with_marker respects the no_hydration option
	let html = renderer.render_with_marker(&counter).await;

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
#[tokio::test]
async fn test_full_ssr_flow() {
	// 1. Create component
	let counter = Counter::new(100);

	// 2. Create SSR state
	let mut state = SsrState::new();
	state.add_signal("initial_count", serde_json::json!(100));

	// 3. Render component
	let mut renderer = SsrRenderer::new();
	let html = renderer.render(&counter).await;

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
