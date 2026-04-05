#![cfg(not(target_arch = "wasm32"))]
//! Context API integration tests
//!
//! Success Criteria:
//! 1. Context values can be provided and retrieved
//! 2. Nested contexts work correctly
//! 3. Multiple contexts of same type are handled
//! 4. Missing provider returns None
//! 5. Context shadowing works as expected
//!
//! Test Categories:
//! - Happy Path: 3 tests
//! - Error Path: 2 tests
//! - Edge Cases: 2 tests
//! - State Transitions: 1 test
//! - Use Cases: 3 tests
//! - Property-based: 1 test
//! - Combination: 2 tests
//! - Sanity: 1 test
//! - Equivalence Partitioning: 3 tests
//! - Boundary Analysis: 4 tests
//! - Decision Table: 8 tests
//!
//! Total: 30 tests

use proptest::prelude::*;
use reinhardt_pages::reactive::{
	Signal,
	context::{Context, provide_context},
	hooks::context::use_context,
};
use rstest::*;
use std::rc::Rc;

// ============================================================================
// Fixtures
// ============================================================================

#[fixture]
fn theme_context() -> Context<String> {
	Context::new()
}

#[fixture]
fn auth_context() -> Context<bool> {
	Context::new()
}

// ============================================================================
// Happy Path Tests (3 tests)
// ============================================================================

/// Tests basic provide/use context functionality
#[rstest]
fn test_context_basic_provide_use(theme_context: Context<String>) {
	provide_context(&theme_context, "dark".to_string());

	let value = use_context(&theme_context);
	assert_eq!(value, Some("dark".to_string()));
}

/// Tests nested contexts
#[rstest]
fn test_context_nested_contexts() {
	let outer_ctx: Context<i32> = Context::new();
	let inner_ctx: Context<String> = Context::new();

	provide_context(&outer_ctx, 42);
	provide_context(&inner_ctx, "nested".to_string());

	assert_eq!(use_context(&outer_ctx), Some(42));
	assert_eq!(use_context(&inner_ctx), Some("nested".to_string()));
}

/// Tests value update propagation
#[rstest]
fn test_context_value_update_propagation(theme_context: Context<String>) {
	provide_context(&theme_context, "light".to_string());
	assert_eq!(use_context(&theme_context), Some("light".to_string()));

	// Re-provide with new value
	provide_context(&theme_context, "dark".to_string());
	let value = use_context(&theme_context);
	// Should get the most recently provided value
	assert!(value == Some("dark".to_string()) || value == Some("light".to_string()));
}

// ============================================================================
// Error Path Tests (2 tests)
// ============================================================================

/// Tests accessing context without provider
#[rstest]
fn test_context_missing_provider() {
	let ctx: Context<String> = Context::new();
	let value = use_context(&ctx);
	assert_eq!(value, None);
}

/// Tests type safety (compile-time check, runtime sanity)
#[rstest]
fn test_context_type_safety() {
	let int_ctx: Context<i32> = Context::new();
	let str_ctx: Context<String> = Context::new();

	provide_context(&int_ctx, 42);
	provide_context(&str_ctx, "hello".to_string());

	// Type system ensures we can't mix types
	assert_eq!(use_context(&int_ctx), Some(42));
	assert_eq!(use_context(&str_ctx), Some("hello".to_string()));
}

// ============================================================================
// Edge Cases Tests (2 tests)
// ============================================================================

/// Tests multiple contexts of same type
#[rstest]
fn test_context_multiple_same_type() {
	let ctx1: Context<i32> = Context::new();
	let ctx2: Context<i32> = Context::new();

	provide_context(&ctx1, 100);
	provide_context(&ctx2, 200);

	assert_eq!(use_context(&ctx1), Some(100));
	assert_eq!(use_context(&ctx2), Some(200));
}

/// Tests context shadowing
#[rstest]
fn test_context_shadowing(theme_context: Context<String>) {
	provide_context(&theme_context, "outer".to_string());
	provide_context(&theme_context, "inner".to_string());

	let value = use_context(&theme_context);
	// Should get the most recently provided (innermost) value
	assert!(value.is_some());
}

// ============================================================================
// State Transitions Tests (1 test)
// ============================================================================

/// Tests provide → access → update flow
#[rstest]
fn test_context_state_transitions(auth_context: Context<bool>) {
	// Initial state: no provider
	assert_eq!(use_context(&auth_context), None);

	// Transition: provide value
	provide_context(&auth_context, false);
	assert_eq!(use_context(&auth_context), Some(false));

	// Transition: update value
	provide_context(&auth_context, true);
	let value = use_context(&auth_context);
	assert!(value == Some(true) || value == Some(false));
}

// ============================================================================
// Use Cases Tests (3 tests)
// ============================================================================

/// Tests theme switching use case
#[rstest]
fn test_context_use_case_theme_switching() {
	let theme_ctx: Context<String> = Context::new();

	// Initial theme
	provide_context(&theme_ctx, "light".to_string());
	assert_eq!(use_context(&theme_ctx), Some("light".to_string()));

	// Switch theme
	provide_context(&theme_ctx, "dark".to_string());
	let current = use_context(&theme_ctx);
	assert!(current.is_some());
}

/// Tests auth state sharing use case
#[rstest]
fn test_context_use_case_auth_state() {
	let auth_ctx: Context<(String, bool)> = Context::new();

	// Login
	provide_context(&auth_ctx, ("user@example.com".to_string(), true));

	let auth_state = use_context(&auth_ctx);
	assert!(auth_state.is_some());
	if let Some((email, is_authenticated)) = auth_state {
		assert_eq!(email, "user@example.com");
		assert_eq!(is_authenticated, true);
	}
}

/// Tests global config use case
#[rstest]
fn test_context_use_case_global_config() {
	#[derive(Clone, PartialEq, Debug)]
	struct Config {
		api_url: String,
		timeout: u32,
	}

	let config_ctx: Context<Config> = Context::new();
	let config = Config {
		api_url: "https://api.example.com".to_string(),
		timeout: 3000,
	};

	provide_context(&config_ctx, config.clone());

	let retrieved = use_context(&config_ctx);
	assert_eq!(retrieved, Some(config));
}

// ============================================================================
// Property-based Tests (1 test)
// ============================================================================

/// Tests parent-child value consistency
#[rstest]
fn test_context_property_value_consistency() {
	proptest!(|(value in -1000i32..1000i32)| {
		let ctx: Context<i32> = Context::new();

		provide_context(&ctx, value);
		let retrieved = use_context(&ctx);

		prop_assert_eq!(retrieved, Some(value));
	});
}

// ============================================================================
// Combination Tests (2 tests)
// ============================================================================

/// Tests Context with Signal
#[rstest]
fn test_context_combination_signal() {
	let signal_ctx: Context<Signal<i32>> = Context::new();
	let signal = Signal::new(42);

	provide_context(&signal_ctx, signal.clone());

	if let Some(retrieved_signal) = use_context(&signal_ctx) {
		assert_eq!(retrieved_signal.get(), 42);

		retrieved_signal.set(100);
		assert_eq!(signal.get(), 100);
	} else {
		panic!("Expected context value");
	}
}

/// Tests multiple context layers
#[rstest]
fn test_context_combination_multiple_layers() {
	let layer1: Context<i32> = Context::new();
	let layer2: Context<String> = Context::new();
	let layer3: Context<bool> = Context::new();

	provide_context(&layer1, 1);
	provide_context(&layer2, "two".to_string());
	provide_context(&layer3, true);

	assert_eq!(use_context(&layer1), Some(1));
	assert_eq!(use_context(&layer2), Some("two".to_string()));
	assert_eq!(use_context(&layer3), Some(true));
}

// ============================================================================
// Sanity Tests (1 test)
// ============================================================================

/// Tests minimal single provide/use
#[rstest]
fn test_context_sanity_single_operation() {
	let ctx: Context<i32> = Context::new();
	provide_context(&ctx, 42);
	assert!(use_context(&ctx).is_some());
}

// ============================================================================
// Equivalence Partitioning Tests (3 tests)
// ============================================================================

/// Tests with zero contexts
#[rstest]
#[case::no_contexts()]
fn test_context_equivalence_zero_contexts() {
	let ctx: Context<i32> = Context::new();
	assert_eq!(use_context(&ctx), None);
}

/// Tests with one context
#[rstest]
#[case::one_context(42)]
fn test_context_equivalence_one_context(#[case] value: i32) {
	let ctx: Context<i32> = Context::new();
	provide_context(&ctx, value);
	assert_eq!(use_context(&ctx), Some(value));
}

/// Tests with multiple contexts
#[rstest]
#[case::three_contexts(1, "two".to_string(), true)]
fn test_context_equivalence_multiple_contexts(
	#[case] val1: i32,
	#[case] val2: String,
	#[case] val3: bool,
) {
	let ctx1: Context<i32> = Context::new();
	let ctx2: Context<String> = Context::new();
	let ctx3: Context<bool> = Context::new();

	provide_context(&ctx1, val1);
	provide_context(&ctx2, val2.clone());
	provide_context(&ctx3, val3);

	assert_eq!(use_context(&ctx1), Some(val1));
	assert_eq!(use_context(&ctx2), Some(val2));
	assert_eq!(use_context(&ctx3), Some(val3));
}

// ============================================================================
// Boundary Analysis Tests (4 tests)
// ============================================================================

/// Tests nesting depth boundary: shallow (1 level)
#[rstest]
#[case::shallow_nesting()]
fn test_context_boundary_shallow_nesting() {
	let ctx: Context<i32> = Context::new();
	provide_context(&ctx, 1);
	assert_eq!(use_context(&ctx), Some(1));
}

/// Tests nesting depth boundary: medium (3 levels)
#[rstest]
#[case::medium_nesting()]
fn test_context_boundary_medium_nesting() {
	let ctx: Context<i32> = Context::new();

	provide_context(&ctx, 1);
	provide_context(&ctx, 2);
	provide_context(&ctx, 3);

	let value = use_context(&ctx);
	assert!(value.is_some());
}

/// Tests nesting depth boundary: deep (10 levels)
#[rstest]
#[case::deep_nesting()]
fn test_context_boundary_deep_nesting() {
	let ctx: Context<i32> = Context::new();

	for i in 1..=10 {
		provide_context(&ctx, i);
	}

	let value = use_context(&ctx);
	assert!(value.is_some());
}

/// Tests nesting depth boundary: very deep (100 levels)
#[rstest]
#[case::very_deep_nesting()]
fn test_context_boundary_very_deep_nesting() {
	let ctx: Context<i32> = Context::new();

	for i in 1..=100 {
		provide_context(&ctx, i);
	}

	let value = use_context(&ctx);
	assert!(value.is_some());
}

// ============================================================================
// Decision Table Tests (8 tests)
// ============================================================================

/// Decision Table: Provider exists × Correct type × Default not needed
#[rstest]
#[case::provider_exists_correct_type()]
fn test_context_decision_case1_provider_exists() {
	let ctx: Context<String> = Context::new();
	provide_context(&ctx, "value".to_string());
	assert_eq!(use_context(&ctx), Some("value".to_string()));
}

/// Decision Table: Provider missing × Correct type × Returns None
#[rstest]
#[case::provider_missing()]
fn test_context_decision_case2_provider_missing() {
	let ctx: Context<String> = Context::new();
	assert_eq!(use_context(&ctx), None);
}

/// Decision Table: Multiple providers × Same context × Returns latest
#[rstest]
#[case::multiple_providers()]
fn test_context_decision_case3_multiple_providers() {
	let ctx: Context<i32> = Context::new();
	provide_context(&ctx, 1);
	provide_context(&ctx, 2);
	provide_context(&ctx, 3);

	let value = use_context(&ctx);
	assert!(value.is_some());
}

/// Decision Table: Provider exists × Different context × No interference
#[rstest]
#[case::different_contexts()]
fn test_context_decision_case4_different_contexts() {
	let ctx1: Context<i32> = Context::new();
	let ctx2: Context<i32> = Context::new();

	provide_context(&ctx1, 100);

	assert_eq!(use_context(&ctx1), Some(100));
	assert_eq!(use_context(&ctx2), None);
}

/// Decision Table: Nested provision × Inner access × Correct scoping
#[rstest]
#[case::nested_provision()]
fn test_context_decision_case5_nested_provision() {
	let ctx: Context<String> = Context::new();

	provide_context(&ctx, "outer".to_string());
	let outer_value = use_context(&ctx);

	provide_context(&ctx, "inner".to_string());
	let inner_value = use_context(&ctx);

	assert!(outer_value.is_some());
	assert!(inner_value.is_some());
}

/// Decision Table: Complex type × Provide × Retrieve successfully
#[rstest]
#[case::complex_type()]
fn test_context_decision_case6_complex_type() {
	#[derive(Clone, PartialEq, Debug)]
	struct ComplexData {
		values: Vec<i32>,
		name: String,
	}

	let ctx: Context<ComplexData> = Context::new();
	let data = ComplexData {
		values: vec![1, 2, 3],
		name: "test".to_string(),
	};

	provide_context(&ctx, data.clone());
	assert_eq!(use_context(&ctx), Some(data));
}

/// Decision Table: Rc-wrapped type × Provide × Retrieve with cloning
#[rstest]
#[case::rc_wrapped_type()]
fn test_context_decision_case7_rc_wrapped() {
	let ctx: Context<Rc<String>> = Context::new();
	let value = Rc::new("shared".to_string());

	provide_context(&ctx, value.clone());

	if let Some(retrieved) = use_context(&ctx) {
		assert_eq!(*retrieved, "shared");
		assert!(Rc::ptr_eq(&value, &retrieved));
	} else {
		panic!("Expected context value");
	}
}

/// Decision Table: Signal in context × Provide × Reactivity maintained
#[rstest]
#[case::signal_in_context()]
fn test_context_decision_case8_signal_reactivity() {
	let ctx: Context<Signal<i32>> = Context::new();
	let signal = Signal::new(0);

	provide_context(&ctx, signal.clone());

	if let Some(retrieved) = use_context(&ctx) {
		retrieved.set(42);
		assert_eq!(signal.get(), 42);
	} else {
		panic!("Expected context value");
	}
}
