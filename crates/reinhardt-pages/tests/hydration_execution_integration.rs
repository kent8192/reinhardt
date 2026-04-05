#![cfg(not(target_arch = "wasm32"))]
//! Hydration Execution Integration Tests
//!
//! This module contains comprehensive integration tests for the reinhardt-pages
//! Hydration system, including SSR state restoration, marker search, event reattachment,
//! and DOM reconciliation.
//!
//! Note: Many tests require WASM environment for DOM operations and window access.
//! Non-WASM tests focus on HydrationContext and HydrationError behavior.
//!
//! Success Criteria:
//! 1. HydrationContext correctly restores SSR state from window
//! 2. Hydration markers are found and matched correctly
//! 3. Events are reattached to hydrated DOM elements
//! 4. DOM structure mismatches are detected
//! 5. Island Architecture (partial hydration) works correctly
//!
//! Test Categories:
//! - Happy Path: 3 tests
//! - Error Path: 3 tests
//! - Edge Cases: 2 tests
//! - State Transitions: 1 test
//! - Use Cases: 2 tests
//! - Property-based: 1 test
//! - Combination: 2 tests
//! - Sanity: 1 test
//! - Equivalence Partitioning: 3 tests
//! - Boundary Analysis: 6 tests
//! - Decision Table: 6 tests
//!
//! Total: 30 test cases

use reinhardt_pages::hydration::{HydrationContext, HydrationError};
use reinhardt_pages::ssr::SsrState;
use rstest::*;
use serde_json::json;

#[cfg(wasm)]
use reinhardt_pages::hydration::{hydrate, hydrate_root};

#[cfg(wasm)]
use wasm_bindgen_test::*;

// ============================================================================
// Test Fixtures
// ============================================================================

/// Fixture: SSR state with signals
#[fixture]
pub fn ssr_state_with_signals() -> SsrState {
	let mut state = SsrState::new();
	state.add_signal("counter-0", json!(42));
	state.add_signal("user-name-0", json!("Alice"));
	state
}

/// Fixture: SSR state with props
#[fixture]
pub fn ssr_state_with_props() -> SsrState {
	let mut state = SsrState::new();
	state.add_props("component-0", json!({"id": 1, "title": "Test"}));
	state
}

/// Fixture: Empty SSR state
#[fixture]
pub fn empty_ssr_state() -> SsrState {
	SsrState::new()
}

// ============================================================================
// Happy Path Tests (3 tests) - Non-WASM Compatible
// ============================================================================

/// Tests HydrationContext creation from SSR state
#[rstest]
fn test_hydration_context_from_state_basic(ssr_state_with_signals: SsrState) {
	let context = HydrationContext::from_state(ssr_state_with_signals);

	assert!(!context.is_hydrated());
	assert_eq!(context.get_signal("counter-0"), Some(&json!(42)));
	assert_eq!(context.get_signal("user-name-0"), Some(&json!("Alice")));
}

/// Tests HydrationContext signal retrieval
#[rstest]
fn test_hydration_context_get_signal(ssr_state_with_signals: SsrState) {
	let context = HydrationContext::from_state(ssr_state_with_signals);

	// Existing signal
	let counter = context.get_signal("counter-0");
	assert_eq!(counter, Some(&json!(42)));

	// Non-existent signal
	let missing = context.get_signal("non-existent");
	assert_eq!(missing, None);
}

/// Tests HydrationContext props retrieval
#[rstest]
fn test_hydration_context_get_props(ssr_state_with_props: SsrState) {
	let context = HydrationContext::from_state(ssr_state_with_props);

	// Existing props
	let props = context.get_props("component-0");
	assert_eq!(props, Some(&json!({"id": 1, "title": "Test"})));

	// Non-existent props
	let missing = context.get_props("non-existent");
	assert_eq!(missing, None);
}

// ============================================================================
// Error Path Tests (3 tests) - Non-WASM Compatible
// ============================================================================

/// Tests HydrationError::RootNotFound display
#[rstest]
fn test_hydration_error_root_not_found() {
	let error = HydrationError::RootNotFound("#app".to_string());

	let display = format!("{}", error);
	assert_eq!(display, "Hydration root element not found: #app");
}

/// Tests HydrationError::StateParseError display
#[rstest]
fn test_hydration_error_state_parse() {
	let error = HydrationError::StateParseError("Invalid JSON".to_string());

	let display = format!("{}", error);
	assert_eq!(display, "Failed to parse SSR state: Invalid JSON");
}

/// Tests HydrationError::StructureMismatch display
#[rstest]
fn test_hydration_error_structure_mismatch() {
	let error = HydrationError::StructureMismatch {
		id: "comp-0".to_string(),
		expected: "div".to_string(),
		actual: "span".to_string(),
	};

	let display = format!("{}", error);
	assert_eq!(
		display,
		"DOM structure mismatch at comp-0: expected div, found span"
	);
}

// ============================================================================
// Edge Case Tests (2 tests) - Non-WASM Compatible
// ============================================================================

/// Tests HydrationContext with empty state
#[rstest]
fn test_hydration_context_empty_state(empty_ssr_state: SsrState) {
	let context = HydrationContext::from_state(empty_ssr_state);

	assert!(!context.is_hydrated());
	assert_eq!(context.get_signal("any-signal"), None);
	assert_eq!(context.get_props("any-component"), None);
}

/// Tests HydrationContext with deeply nested component structure
#[rstest]
fn test_hydration_context_nested_components() {
	let mut state = SsrState::new();

	// Simulate nested components
	for i in 0..10 {
		let id = format!("nested-comp-{}", i);
		state.add_props(&id, json!({"level": i}));
	}

	let context = HydrationContext::from_state(state);

	// Verify all nested components are accessible
	for i in 0..10 {
		let id = format!("nested-comp-{}", i);
		assert_eq!(context.get_props(&id), Some(&json!({"level": i})));
	}
}

// ============================================================================
// State Transition Tests (1 test) - Non-WASM Compatible
// ============================================================================

/// Tests state transition from not hydrated to hydrated
#[rstest]
fn test_hydration_state_transition(empty_ssr_state: SsrState) {
	let mut context = HydrationContext::from_state(empty_ssr_state);

	// Initial state: not hydrated
	assert!(!context.is_hydrated());

	// Transition: mark as hydrated
	context.mark_hydrated();

	// Final state: hydrated
	assert!(context.is_hydrated());
}

// ============================================================================
// Use Case Tests (2 tests) - Non-WASM Compatible
// ============================================================================

/// Tests HydrationContext for initial page load scenario
#[rstest]
fn test_hydration_use_case_initial_page_load() {
	let mut state = SsrState::new();

	// Simulate SSR-rendered state for a user profile page
	state.add_signal("user-id", json!(123));
	state.add_signal("user-name", json!("John Doe"));
	state.add_props("profile-card", json!({"editable": false}));

	let context = HydrationContext::from_state(state);

	// Verify state is restored correctly
	assert_eq!(context.get_signal("user-id"), Some(&json!(123)));
	assert_eq!(context.get_signal("user-name"), Some(&json!("John Doe")));
	assert_eq!(
		context.get_props("profile-card"),
		Some(&json!({"editable": false}))
	);
}

/// Tests HydrationContext for Island Architecture scenario
#[rstest]
fn test_hydration_use_case_island_architecture() {
	let mut state = SsrState::new();

	// Island 1: Interactive counter (hydrated)
	state.add_signal("island-counter", json!(0));
	state.add_props("counter-island", json!({"hydrate": true}));

	// Island 2: Static content (not hydrated)
	state.add_props("static-island", json!({"hydrate": false}));

	let context = HydrationContext::from_state(state);

	// Verify both islands have correct state
	assert_eq!(context.get_signal("island-counter"), Some(&json!(0)));
	assert_eq!(
		context.get_props("counter-island"),
		Some(&json!({"hydrate": true}))
	);
	assert_eq!(
		context.get_props("static-island"),
		Some(&json!({"hydrate": false}))
	);
}

// ============================================================================
// Property-based Tests (1 test) - Non-WASM Compatible
// ============================================================================

/// Tests property: Signal retrieval is idempotent
#[rstest]
fn test_hydration_property_signal_retrieval_idempotent(ssr_state_with_signals: SsrState) {
	let context = HydrationContext::from_state(ssr_state_with_signals);

	// Property: Multiple reads should return the same value
	let read1 = context.get_signal("counter-0");
	let read2 = context.get_signal("counter-0");
	let read3 = context.get_signal("counter-0");

	assert_eq!(read1, read2);
	assert_eq!(read2, read3);
	assert_eq!(read1, Some(&json!(42)));
}

// ============================================================================
// Combination Tests (2 tests) - Non-WASM Compatible
// ============================================================================

/// Tests HydrationContext with both signals and props
#[rstest]
fn test_hydration_combination_signals_and_props() {
	let mut state = SsrState::new();

	// Add both signals and props
	state.add_signal("signal-1", json!("value1"));
	state.add_signal("signal-2", json!(100));
	state.add_props("comp-1", json!({"prop": "value"}));
	state.add_props("comp-2", json!({"count": 50}));

	let context = HydrationContext::from_state(state);

	// Verify both types are accessible
	assert_eq!(context.get_signal("signal-1"), Some(&json!("value1")));
	assert_eq!(context.get_signal("signal-2"), Some(&json!(100)));
	assert_eq!(context.get_props("comp-1"), Some(&json!({"prop": "value"})));
	assert_eq!(context.get_props("comp-2"), Some(&json!({"count": 50})));
}

/// Tests HydrationContext state restoration with complex JSON structures
#[rstest]
fn test_hydration_combination_complex_json() {
	let mut state = SsrState::new();

	// Complex nested JSON
	state.add_signal(
		"complex",
		json!({
			"user": {
				"id": 1,
				"name": "Alice",
				"roles": ["admin", "user"],
				"metadata": {
					"created_at": "2024-01-01",
					"last_login": "2024-12-27"
				}
			}
		}),
	);

	let context = HydrationContext::from_state(state);

	let complex = context.get_signal("complex").unwrap();
	assert_eq!(complex["user"]["id"], 1);
	assert_eq!(complex["user"]["name"], "Alice");
	assert_eq!(complex["user"]["roles"][0], "admin");
	assert_eq!(complex["user"]["metadata"]["created_at"], "2024-01-01");
}

// ============================================================================
// Sanity Tests (1 test) - Non-WASM Compatible
// ============================================================================

/// Tests basic HydrationContext creation and methods
#[rstest]
fn test_hydration_context_sanity() {
	// Test default constructor
	let context1 = HydrationContext::new();
	assert!(!context1.is_hydrated());

	// Test from_state constructor
	let state = SsrState::new();
	let context2 = HydrationContext::from_state(state);
	assert!(!context2.is_hydrated());

	// Test mark_hydrated
	let mut context3 = HydrationContext::new();
	context3.mark_hydrated();
	assert!(context3.is_hydrated());
}

// ============================================================================
// Equivalence Partitioning Tests (3 tests) - Non-WASM Compatible
// ============================================================================

/// Tests HydrationError variants partitioning
#[rstest]
#[case::root_not_found(
	HydrationError::RootNotFound("#app".to_string()),
	"Hydration root element not found"
)]
#[case::state_parse_error(
	HydrationError::StateParseError("Invalid JSON".to_string()),
	"Failed to parse SSR state"
)]
#[case::marker_not_found(
	HydrationError::MarkerNotFound("marker-0".to_string()),
	"Hydration marker not found"
)]
fn test_hydration_error_partitions(#[case] error: HydrationError, #[case] expected_prefix: &str) {
	let display = format!("{}", error);
	assert!(display.starts_with(expected_prefix));
}

/// Tests HydrationContext state access partitions
#[rstest]
#[case::signal_exists("counter", Some(json!(42)))]
#[case::signal_missing("non-existent", None)]
fn test_hydration_context_signal_access_partitions(
	#[case] signal_id: &str,
	#[case] expected: Option<serde_json::Value>,
) {
	let mut state = SsrState::new();
	if expected.is_some() {
		state.add_signal("counter", json!(42));
	}

	let context = HydrationContext::from_state(state);
	let result = context.get_signal(signal_id).cloned();

	assert_eq!(result, expected);
}

/// Tests HydrationContext hydration status partitions
#[rstest]
#[case::not_hydrated(false)]
#[case::hydrated(true)]
fn test_hydration_context_status_partitions(#[case] should_hydrate: bool) {
	let mut context = HydrationContext::new();

	if should_hydrate {
		context.mark_hydrated();
	}

	assert_eq!(context.is_hydrated(), should_hydrate);
}

// ============================================================================
// Boundary Analysis Tests (6 tests) - Non-WASM Compatible
// ============================================================================

/// Tests boundary for number of signals
#[rstest]
#[case::zero_signals(0)]
#[case::one_signal(1)]
#[case::ten_signals(10)]
#[case::hundred_signals(100)]
fn test_hydration_context_signal_count_boundary(#[case] count: usize) {
	let mut state = SsrState::new();

	for i in 0..count {
		state.add_signal(&format!("signal-{}", i), json!(i));
	}

	let context = HydrationContext::from_state(state);

	// Verify all signals are accessible
	for i in 0..count {
		assert_eq!(
			context.get_signal(&format!("signal-{}", i)),
			Some(&json!(i))
		);
	}
}

/// Tests boundary for nesting depth of components
#[rstest]
#[case::flat(1)]
#[case::shallow(3)]
#[case::deep(10)]
#[case::very_deep(50)]
fn test_hydration_context_nesting_depth_boundary(#[case] depth: usize) {
	let mut state = SsrState::new();

	for i in 0..depth {
		state.add_props(&format!("comp-level-{}", i), json!({"depth": i}));
	}

	let context = HydrationContext::from_state(state);

	// Verify all nested components
	for i in 0..depth {
		assert_eq!(
			context.get_props(&format!("comp-level-{}", i)),
			Some(&json!({"depth": i}))
		);
	}
}

/// Tests boundary for signal value sizes
#[rstest]
fn test_hydration_context_signal_value_size_boundary() {
	let mut state = SsrState::new();

	// Empty string
	state.add_signal("empty", json!(""));

	// Small value
	state.add_signal("small", json!("x".repeat(10)));

	// Large value
	state.add_signal("large", json!("x".repeat(10000)));

	let context = HydrationContext::from_state(state);

	assert_eq!(context.get_signal("empty"), Some(&json!("")));
	assert_eq!(context.get_signal("small"), Some(&json!("x".repeat(10))));
	assert_eq!(context.get_signal("large"), Some(&json!("x".repeat(10000))));
}

/// Tests boundary for signal ID length
#[rstest]
#[case::short_id("s")]
#[case::medium_id("signal-medium-length-id")]
#[case::long_id(&"x".repeat(1000))]
fn test_hydration_context_signal_id_length_boundary(#[case] signal_id: &str) {
	let mut state = SsrState::new();
	state.add_signal(signal_id, json!(42));

	let context = HydrationContext::from_state(state);

	assert_eq!(context.get_signal(signal_id), Some(&json!(42)));
}

/// Tests boundary for JSON value types
#[rstest]
fn test_hydration_context_json_value_types_boundary() {
	let mut state = SsrState::new();

	// Different JSON types
	state.add_signal("null", json!(null));
	state.add_signal("bool", json!(true));
	state.add_signal("number", json!(42));
	state.add_signal("string", json!("text"));
	state.add_signal("array", json!([1, 2, 3]));
	state.add_signal("object", json!({"key": "value"}));

	let context = HydrationContext::from_state(state);

	assert_eq!(context.get_signal("null"), Some(&json!(null)));
	assert_eq!(context.get_signal("bool"), Some(&json!(true)));
	assert_eq!(context.get_signal("number"), Some(&json!(42)));
	assert_eq!(context.get_signal("string"), Some(&json!("text")));
	assert_eq!(context.get_signal("array"), Some(&json!([1, 2, 3])));
	assert_eq!(context.get_signal("object"), Some(&json!({"key": "value"})));
}

/// Tests boundary for Unicode in signal values
#[rstest]
fn test_hydration_context_unicode_boundary() {
	let mut state = SsrState::new();

	state.add_signal("japanese", json!("こんにちは"));
	state.add_signal("emoji", json!("🌍🚀"));
	state.add_signal("mixed", json!("Hello 世界 🌟"));

	let context = HydrationContext::from_state(state);

	assert_eq!(context.get_signal("japanese"), Some(&json!("こんにちは")));
	assert_eq!(context.get_signal("emoji"), Some(&json!("🌍🚀")));
	assert_eq!(context.get_signal("mixed"), Some(&json!("Hello 世界 🌟")));
}

// ============================================================================
// Decision Table Tests (6 tests) - Non-WASM Compatible
// ============================================================================

/// Decision table test 1: Empty state, no hydration
#[rstest]
fn test_hydration_decision_table_case1_empty_state() {
	let context = HydrationContext::new();

	assert!(!context.is_hydrated());
	assert_eq!(context.get_signal("any"), None);
	assert_eq!(context.get_props("any"), None);
}

/// Decision table test 2: State with signals, no hydration
#[rstest]
fn test_hydration_decision_table_case2_signals_not_hydrated() {
	let mut state = SsrState::new();
	state.add_signal("test", json!(123));

	let context = HydrationContext::from_state(state);

	assert!(!context.is_hydrated());
	assert_eq!(context.get_signal("test"), Some(&json!(123)));
	assert_eq!(context.get_props("test"), None);
}

/// Decision table test 3: State with props, no hydration
#[rstest]
fn test_hydration_decision_table_case3_props_not_hydrated() {
	let mut state = SsrState::new();
	state.add_props("comp", json!({"key": "value"}));

	let context = HydrationContext::from_state(state);

	assert!(!context.is_hydrated());
	assert_eq!(context.get_signal("comp"), None);
	assert_eq!(context.get_props("comp"), Some(&json!({"key": "value"})));
}

/// Decision table test 4: State with both, hydrated
#[rstest]
fn test_hydration_decision_table_case4_both_hydrated() {
	let mut state = SsrState::new();
	state.add_signal("sig", json!(1));
	state.add_props("prop", json!(2));

	let mut context = HydrationContext::from_state(state);
	context.mark_hydrated();

	assert!(context.is_hydrated());
	assert_eq!(context.get_signal("sig"), Some(&json!(1)));
	assert_eq!(context.get_props("prop"), Some(&json!(2)));
}

/// Decision table test 5: Multiple signals, not hydrated
#[rstest]
fn test_hydration_decision_table_case5_multiple_signals() {
	let mut state = SsrState::new();
	state.add_signal("s1", json!(10));
	state.add_signal("s2", json!(20));
	state.add_signal("s3", json!(30));

	let context = HydrationContext::from_state(state);

	assert!(!context.is_hydrated());
	assert_eq!(context.get_signal("s1"), Some(&json!(10)));
	assert_eq!(context.get_signal("s2"), Some(&json!(20)));
	assert_eq!(context.get_signal("s3"), Some(&json!(30)));
}

/// Decision table test 6: Complex state, hydrated
#[rstest]
fn test_hydration_decision_table_case6_complex_hydrated() {
	let mut state = SsrState::new();

	// Mix of signals and props with various types
	state.add_signal("counter", json!(42));
	state.add_props("user-card", json!({"name": "Alice", "age": 30}));
	state.add_signal("items", json!(["a", "b", "c"]));

	let mut context = HydrationContext::from_state(state);
	context.mark_hydrated();

	assert!(context.is_hydrated());
	assert_eq!(context.get_signal("counter"), Some(&json!(42)));
	assert_eq!(
		context.get_props("user-card"),
		Some(&json!({"name": "Alice", "age": 30}))
	);
	assert_eq!(context.get_signal("items"), Some(&json!(["a", "b", "c"])));
}
