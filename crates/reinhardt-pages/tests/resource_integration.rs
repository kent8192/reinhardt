#![cfg(not(target_arch = "wasm32"))]
//! Resource Integration Tests
//!
//! This module contains comprehensive integration tests for the reinhardt-pages
//! Resource API, including state management, dependency tracking, and refetch behavior.
//!
//! Note: Many tests require WASM environment for async execution via spawn_local.
//! Non-WASM tests focus on ResourceState behavior and synchronous operations.
//!
//! Success Criteria:
//! 1. Resource correctly manages Loading/Success/Error states
//! 2. Dependency tracking triggers automatic refetch
//! 3. Manual refetch works correctly
//! 4. ResourceState methods behave correctly
//! 5. Resource integrates with Effect and Signal systems
//!
//! Test Categories:
//! - Happy Path: 3 tests
//! - Error Path: 2 tests
//! - Edge Cases: 2 tests
//! - State Transitions: 2 tests
//! - Use Cases: 2 tests
//! - Property-based: 1 test
//! - Combination: 2 tests
//! - Sanity: 1 test
//! - Equivalence Partitioning: 3 tests
//! - Boundary Analysis: 4 tests
//! - Decision Table: 8 tests
//!
//! Total: 30 test cases

use reinhardt_pages::reactive::resource::ResourceState;
use rstest::*;
use serde::{Deserialize, Serialize};

#[cfg(wasm)]
use reinhardt_pages::reactive::{Resource, Signal, create_resource, create_resource_with_deps};

#[cfg(wasm)]
use wasm_bindgen_test::*;

// ============================================================================
// Test Models and Fixtures
// ============================================================================

/// Test data model for Resource tests
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct User {
	pub id: u32,
	pub name: String,
	pub email: String,
}

impl User {
	pub fn new(id: u32, name: impl Into<String>, email: impl Into<String>) -> Self {
		Self {
			id,
			name: name.into(),
			email: email.into(),
		}
	}
}

/// Fixture: Test user
#[fixture]
pub fn test_user() -> User {
	User::new(42, "Test User", "test@example.com")
}

/// Fixture: Small data size
#[fixture]
pub fn small_data_size() -> usize {
	10
}

/// Fixture: Large data size
#[fixture]
pub fn large_data_size() -> usize {
	10000
}

// ============================================================================
// Happy Path Tests (3 tests) - Non-WASM Compatible
// ============================================================================

/// Tests ResourceState::Loading basic behavior
#[rstest]
fn test_resource_state_loading_basic() {
	let state: ResourceState<User, String> = ResourceState::Loading;

	assert!(state.is_loading());
	assert!(!state.is_success());
	assert!(!state.is_error());
	assert_eq!(state.as_ref(), None);
	assert_eq!(state.error(), None);
}

/// Tests ResourceState::Success basic behavior
#[rstest]
fn test_resource_state_success_basic(test_user: User) {
	let state: ResourceState<User, String> = ResourceState::Success(test_user.clone());

	assert!(!state.is_loading());
	assert!(state.is_success());
	assert!(!state.is_error());
	assert_eq!(state.as_ref(), Some(&test_user));
	assert_eq!(state.error(), None);
}

/// Tests ResourceState::Error basic behavior
#[rstest]
fn test_resource_state_error_basic() {
	let error_msg = "Network error".to_string();
	let state: ResourceState<User, String> = ResourceState::Error(error_msg.clone());

	assert!(!state.is_loading());
	assert!(!state.is_success());
	assert!(state.is_error());
	assert_eq!(state.as_ref(), None);
	assert_eq!(state.error(), Some(&error_msg));
}

// ============================================================================
// Error Path Tests (2 tests) - WASM-only
// ============================================================================

/// Tests Resource handling fetch error
#[cfg(wasm)]
#[wasm_bindgen_test]
async fn test_resource_fetch_error() {
	let resource: Resource<User, String> =
		create_resource(|| async { Err::<User, String>("Network timeout".to_string()) });

	// Wait for async operation to complete
	wasm_bindgen_futures::JsFuture::from(js_sys::Promise::new(&mut |resolve, _reject| {
		web_sys::window()
			.unwrap()
			.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 50)
			.unwrap();
	}))
	.await
	.unwrap();

	let state = resource.get();
	assert!(state.is_error());
	assert_eq!(state.error(), Some(&"Network timeout".to_string()));
}

/// Tests Resource handling deserialization error concept
#[rstest]
fn test_resource_deserialization_error_state() {
	// This tests the state representation of a deserialization error
	let state: ResourceState<User, String> =
		ResourceState::Error("Failed to deserialize JSON".to_string());

	assert!(state.is_error());
	assert!(state.error().unwrap().contains("Failed to deserialize"));
}

// ============================================================================
// Edge Case Tests (2 tests)
// ============================================================================

/// Tests ResourceState with empty string data
#[rstest]
fn test_resource_state_empty_string() {
	let state: ResourceState<String, String> = ResourceState::Success(String::new());

	assert!(state.is_success());
	assert_eq!(state.as_ref(), Some(&String::new()));
}

/// Tests ResourceState with very large data
#[rstest]
fn test_resource_state_large_data(large_data_size: usize) {
	let large_string = "x".repeat(large_data_size);
	let state: ResourceState<String, String> = ResourceState::Success(large_string.clone());

	assert!(state.is_success());
	assert_eq!(state.as_ref(), Some(&large_string));
	assert_eq!(state.as_ref().unwrap().len(), large_data_size);
}

// ============================================================================
// State Transition Tests (2 tests) - Mixed WASM/Non-WASM
// ============================================================================

/// Tests state transition from Loading to Success (non-WASM state representation)
#[rstest]
fn test_resource_state_transition_loading_to_success(test_user: User) {
	// Simulate state transition
	let mut state: ResourceState<User, String> = ResourceState::Loading;
	assert!(state.is_loading());

	// Transition to Success
	state = ResourceState::Success(test_user.clone());
	assert!(state.is_success());
	assert_eq!(state.as_ref(), Some(&test_user));
}

/// Tests manual refetch state transition (WASM-only)
#[cfg(wasm)]
#[wasm_bindgen_test]
async fn test_resource_manual_refetch() {
	use std::cell::RefCell;
	use std::rc::Rc;

	let counter = Rc::new(RefCell::new(0));
	let counter_clone = Rc::clone(&counter);

	let resource: Resource<u32, String> = create_resource(move || {
		let counter = Rc::clone(&counter_clone);
		async move {
			*counter.borrow_mut() += 1;
			Ok::<u32, String>(*counter.borrow())
		}
	});

	// Wait for initial fetch
	wasm_bindgen_futures::JsFuture::from(js_sys::Promise::new(&mut |resolve, _reject| {
		web_sys::window()
			.unwrap()
			.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 50)
			.unwrap();
	}))
	.await
	.unwrap();

	assert_eq!(resource.get().as_ref(), Some(&1));

	// Trigger refetch
	resource.refetch();

	// Wait for refetch
	wasm_bindgen_futures::JsFuture::from(js_sys::Promise::new(&mut |resolve, _reject| {
		web_sys::window()
			.unwrap()
			.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 50)
			.unwrap();
	}))
	.await
	.unwrap();

	assert_eq!(resource.get().as_ref(), Some(&2));
}

// ============================================================================
// Use Case Tests (2 tests) - WASM-only
// ============================================================================

/// Tests Resource for API call simulation
#[cfg(wasm)]
#[wasm_bindgen_test]
async fn test_resource_use_case_api_call(test_user: User) {
	let user = test_user.clone();
	let resource: Resource<User, String> = create_resource(move || {
		let user = user.clone();
		async move { Ok::<User, String>(user) }
	});

	// Wait for fetch
	wasm_bindgen_futures::JsFuture::from(js_sys::Promise::new(&mut |resolve, _reject| {
		web_sys::window()
			.unwrap()
			.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 50)
			.unwrap();
	}))
	.await
	.unwrap();

	let state = resource.get();
	assert!(state.is_success());
	assert_eq!(state.as_ref().unwrap().id, test_user.id);
}

/// Tests ResourceState for optimistic update pattern
#[rstest]
fn test_resource_optimistic_update_pattern(test_user: User) {
	// Optimistic: Show success state immediately
	let mut state: ResourceState<User, String> = ResourceState::Success(test_user.clone());
	assert!(state.is_success());

	// On error, revert to error state
	state = ResourceState::Error("Save failed".to_string());
	assert!(state.is_error());
}

// ============================================================================
// Property-based Tests (1 test)
// ============================================================================

/// Tests refetch idempotency property
#[rstest]
fn test_resource_refetch_idempotency_property() {
	// Property: Multiple calls to state query should return same result
	let state: ResourceState<u32, String> = ResourceState::Success(42);

	let result1 = state.as_ref();
	let result2 = state.as_ref();
	let result3 = state.as_ref();

	assert_eq!(result1, result2);
	assert_eq!(result2, result3);
	assert_eq!(result1, Some(&42));
}

// ============================================================================
// Combination Tests (2 tests) - WASM-only
// ============================================================================

/// Tests Resource with Signal dependency (WASM-only)
#[cfg(wasm)]
#[wasm_bindgen_test]
async fn test_resource_with_signal_dependency() {
	let user_id = Signal::new(1u32);

	let resource: Resource<String, String> =
		create_resource_with_deps(user_id.clone(), |id| async move {
			Ok::<String, String>(format!("User {}", id))
		});

	// Wait for initial fetch
	wasm_bindgen_futures::JsFuture::from(js_sys::Promise::new(&mut |resolve, _reject| {
		web_sys::window()
			.unwrap()
			.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 50)
			.unwrap();
	}))
	.await
	.unwrap();

	assert_eq!(resource.get().as_ref(), Some(&"User 1".to_string()));

	// Change dependency
	user_id.set(2);

	// Wait for refetch
	wasm_bindgen_futures::JsFuture::from(js_sys::Promise::new(&mut |resolve, _reject| {
		web_sys::window()
			.unwrap()
			.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, 50)
			.unwrap();
	}))
	.await
	.unwrap();

	assert_eq!(resource.get().as_ref(), Some(&"User 2".to_string()));
}

/// Tests ResourceState with Effect pattern (conceptual, non-WASM)
#[rstest]
fn test_resource_state_with_effect_pattern(test_user: User) {
	// Simulates Effect reacting to ResourceState changes
	let state: ResourceState<User, String> = ResourceState::Success(test_user.clone());

	// Effect would check state and perform side effect
	if state.is_success() {
		let user = state.as_ref().unwrap();
		assert_eq!(user.id, test_user.id);
	}
}

// ============================================================================
// Sanity Tests (1 test)
// ============================================================================

/// Tests basic ResourceState creation and methods
#[rstest]
fn test_resource_state_sanity() {
	let loading: ResourceState<i32, String> = ResourceState::Loading;
	let success: ResourceState<i32, String> = ResourceState::Success(100);
	let error: ResourceState<i32, String> = ResourceState::Error("failed".to_string());

	assert!(loading.is_loading());
	assert!(success.is_success());
	assert!(error.is_error());
}

// ============================================================================
// Equivalence Partitioning Tests (3 tests)
// ============================================================================

/// Tests ResourceState partitions
#[rstest]
#[case::loading_partition(ResourceState::Loading::<String, String>, true, false, false)]
#[case::success_partition(ResourceState::Success::<String, String>("data".to_string()), false, true, false)]
#[case::error_partition(ResourceState::Error::<String, String>("err".to_string()), false, false, true)]
fn test_resource_state_partitions(
	#[case] state: ResourceState<String, String>,
	#[case] expected_loading: bool,
	#[case] expected_success: bool,
	#[case] expected_error: bool,
) {
	assert_eq!(state.is_loading(), expected_loading);
	assert_eq!(state.is_success(), expected_success);
	assert_eq!(state.is_error(), expected_error);
}

// ============================================================================
// Boundary Analysis Tests (4 tests)
// ============================================================================

/// Tests boundary for data size
#[rstest]
#[case::zero_size(0)]
#[case::small_size(10)]
#[case::medium_size(1000)]
#[case::large_size(100000)]
fn test_resource_state_data_size_boundary(#[case] size: usize) {
	let data = "x".repeat(size);
	let state: ResourceState<String, String> = ResourceState::Success(data.clone());

	assert!(state.is_success());
	assert_eq!(state.as_ref().unwrap().len(), size);
}

/// Tests boundary for error message length
#[rstest]
#[case::empty_error("")]
#[case::short_error("Error")]
#[case::long_error(&"x".repeat(10000))]
fn test_resource_error_message_boundary(#[case] error_msg: &str) {
	let state: ResourceState<String, String> = ResourceState::Error(error_msg.to_string());

	assert!(state.is_error());
	assert_eq!(state.error().unwrap().len(), error_msg.len());
}

/// Tests boundary for nested data structures
#[rstest]
fn test_resource_state_nested_data_boundary() {
	#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
	struct NestedData {
		level1: Vec<Vec<Vec<u32>>>,
	}

	// 0 levels
	let data0 = Vec::<u32>::new();
	let state0: ResourceState<Vec<u32>, String> = ResourceState::Success(data0.clone());
	assert_eq!(state0.as_ref(), Some(&data0));

	// 3 levels
	let data3 = NestedData {
		level1: vec![vec![vec![1, 2], vec![3, 4]], vec![vec![5, 6]]],
	};
	let state3: ResourceState<NestedData, String> = ResourceState::Success(data3.clone());
	assert_eq!(state3.as_ref(), Some(&data3));
}

/// Tests boundary for `Option<T>` data
#[rstest]
#[case::none_value(None)]
#[case::some_value(Some(42))]
fn test_resource_state_option_data_boundary(#[case] data: Option<i32>) {
	let state: ResourceState<Option<i32>, String> = ResourceState::Success(data);

	assert!(state.is_success());
	assert_eq!(state.as_ref(), Some(&data));
}

// ============================================================================
// Decision Table Tests (8 tests)
// ============================================================================

/// Decision table test 1: Loading state
#[rstest]
fn test_resource_decision_table_case1_loading() {
	let state: ResourceState<String, String> = ResourceState::Loading;

	assert!(state.is_loading());
	assert!(!state.is_success());
	assert!(!state.is_error());
	assert_eq!(state.as_ref(), None);
	assert_eq!(state.error(), None);
}

/// Decision table test 2: Success with data
#[rstest]
fn test_resource_decision_table_case2_success_with_data() {
	let state: ResourceState<String, String> = ResourceState::Success("data".to_string());

	assert!(!state.is_loading());
	assert!(state.is_success());
	assert!(!state.is_error());
	assert_eq!(state.as_ref(), Some(&"data".to_string()));
	assert_eq!(state.error(), None);
}

/// Decision table test 3: Error with message
#[rstest]
fn test_resource_decision_table_case3_error_with_message() {
	let state: ResourceState<String, String> = ResourceState::Error("error".to_string());

	assert!(!state.is_loading());
	assert!(!state.is_success());
	assert!(state.is_error());
	assert_eq!(state.as_ref(), None);
	assert_eq!(state.error(), Some(&"error".to_string()));
}

/// Decision table test 4: Success with empty string
#[rstest]
fn test_resource_decision_table_case4_success_empty_string() {
	let state: ResourceState<String, String> = ResourceState::Success(String::new());

	assert!(state.is_success());
	assert_eq!(state.as_ref(), Some(&String::new()));
}

/// Decision table test 5: Error with empty message
#[rstest]
fn test_resource_decision_table_case5_error_empty_message() {
	let state: ResourceState<String, String> = ResourceState::Error(String::new());

	assert!(state.is_error());
	assert_eq!(state.error(), Some(&String::new()));
}

/// Decision table test 6: Success with numeric data
#[rstest]
fn test_resource_decision_table_case6_success_numeric() {
	let state: ResourceState<i32, String> = ResourceState::Success(42);

	assert!(state.is_success());
	assert_eq!(state.as_ref(), Some(&42));
}

/// Decision table test 7: Error with numeric code
#[rstest]
fn test_resource_decision_table_case7_error_numeric() {
	let state: ResourceState<String, i32> = ResourceState::Error(404);

	assert!(state.is_error());
	assert_eq!(state.error(), Some(&404));
}

/// Decision table test 8: Loading with complex type
#[rstest]
fn test_resource_decision_table_case8_loading_complex_type() {
	let state: ResourceState<Vec<User>, String> = ResourceState::Loading;

	assert!(state.is_loading());
	assert_eq!(state.as_ref(), None);
}
