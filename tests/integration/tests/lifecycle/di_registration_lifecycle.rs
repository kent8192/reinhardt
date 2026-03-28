//! DI Registration Lifecycle Tests
//!
//! Verifies the specification invariants of the deferred DI registration chain:
//! `register_di_registrations()` → `take_di_registrations()` → `apply_to(scope)`
//! → `InjectionContext::builder(scope).build()`
//!
//! These tests would have detected Issue #3033 (DI registrations silently lost)
//! because they exercise the full lifecycle chain end-to-end.

use reinhardt_di::{DiRegistrationList, InjectionContext, SingletonScope};
use reinhardt_urls::routers::{clear_router, register_di_registrations, take_di_registrations};
use rstest::rstest;
use serial_test::serial;
use std::sync::Arc;

/// Specification: `take_di_registrations()` returns `None` when no registrations exist.
#[rstest]
#[serial(di_registration)]
fn take_without_register_returns_none() {
	// Arrange
	clear_router();

	// Act
	let result = take_di_registrations();

	// Assert
	assert!(
		result.is_none(),
		"take_di_registrations must return None when nothing was registered"
	);
}

/// Specification: `take_di_registrations()` returns `Some` after registration.
#[rstest]
#[serial(di_registration)]
fn register_then_take_returns_some() {
	// Arrange
	clear_router();
	let mut list = DiRegistrationList::new();
	list.register_arc(Arc::new(42_i32));
	register_di_registrations(list);

	// Act
	let result = take_di_registrations();

	// Assert
	assert!(
		result.is_some(),
		"take_di_registrations must return Some after registration"
	);
}

/// Specification: `take_di_registrations()` is single-consumer.
/// The second call must return `None` because registrations are consumed on first take.
#[rstest]
#[serial(di_registration)]
fn take_is_single_consumer() {
	// Arrange
	clear_router();
	let mut list = DiRegistrationList::new();
	list.register_arc(Arc::new(42_i32));
	register_di_registrations(list);

	// Act
	let first_take = take_di_registrations();
	let second_take = take_di_registrations();

	// Assert
	assert!(first_take.is_some(), "first take must return Some");
	assert!(
		second_take.is_none(),
		"second take must return None (single-consumer semantics)"
	);
}

/// Specification: Registrations applied to a scope BEFORE building InjectionContext
/// must be resolvable from that scope.
///
/// This is the key test that would have caught Issue #3033.
/// The full chain:
/// 1. Create DiRegistrationList and add a type
/// 2. Store globally via register_di_registrations()
/// 3. Retrieve via take_di_registrations()
/// 4. Apply to a SingletonScope
/// 5. Build InjectionContext with that same scope
/// 6. Verify the type is resolvable from the scope
#[rstest]
#[serial(di_registration)]
fn applied_registrations_are_resolvable_from_context() {
	// Arrange
	clear_router();
	let expected_value = 42_i32;
	let mut list = DiRegistrationList::new();
	list.register_arc(Arc::new(expected_value));
	register_di_registrations(list);

	// Act
	let scope = Arc::new(SingletonScope::new());
	let registrations = take_di_registrations().expect("registrations must exist");
	registrations.apply_to(&scope);
	let _context = InjectionContext::builder(scope.clone()).build();

	// Assert
	let resolved = scope.get::<i32>();
	assert!(
		resolved.is_some(),
		"registered type must be resolvable after apply_to"
	);
	assert_eq!(*resolved.unwrap(), expected_value);
}

/// Specification: Without calling `apply_to()`, registered types must NOT be
/// resolvable from the scope. The scope starts empty.
#[rstest]
#[serial(di_registration)]
fn unapplied_registrations_are_not_resolvable() {
	// Arrange
	clear_router();
	let mut list = DiRegistrationList::new();
	list.register_arc(Arc::new(42_i32));
	register_di_registrations(list);

	// Act — take but do NOT apply
	let _registrations = take_di_registrations();
	let scope = Arc::new(SingletonScope::new());
	let _context = InjectionContext::builder(scope.clone()).build();

	// Assert
	let resolved = scope.get::<i32>();
	assert!(
		resolved.is_none(),
		"type must NOT be resolvable when apply_to was not called"
	);
}

/// Specification: `apply_to(scope_a)` followed by `InjectionContext::builder(scope_b)`
/// means the type is resolvable from scope_a but NOT from scope_b.
/// The scope identity matters.
#[rstest]
#[serial(di_registration)]
fn scope_identity_required_for_resolution() {
	// Arrange
	clear_router();
	let mut list = DiRegistrationList::new();
	list.register_arc(Arc::new(42_i32));
	register_di_registrations(list);

	// Act
	let scope_a = Arc::new(SingletonScope::new());
	let scope_b = Arc::new(SingletonScope::new());
	let registrations = take_di_registrations().expect("registrations must exist");
	registrations.apply_to(&scope_a);
	let _context = InjectionContext::builder(scope_b.clone()).build();

	// Assert
	assert!(
		scope_a.get::<i32>().is_some(),
		"type must be resolvable from the scope that received apply_to"
	);
	assert!(
		scope_b.get::<i32>().is_none(),
		"type must NOT be resolvable from a different scope"
	);
}

/// Specification: Multiple `register_di_registrations()` calls merge registrations.
/// All merged types must be present after take + apply.
#[rstest]
#[serial(di_registration)]
fn merged_registrations_all_applied() {
	// Arrange
	clear_router();
	let mut list_a = DiRegistrationList::new();
	list_a.register_arc(Arc::new(42_i32));
	register_di_registrations(list_a);

	let mut list_b = DiRegistrationList::new();
	list_b.register_arc(Arc::new("hello".to_string()));
	register_di_registrations(list_b);

	// Act
	let scope = Arc::new(SingletonScope::new());
	let registrations = take_di_registrations().expect("merged registrations must exist");
	registrations.apply_to(&scope);

	// Assert
	assert!(
		scope.get::<i32>().is_some(),
		"i32 from first registration must be present"
	);
	assert!(
		scope.get::<String>().is_some(),
		"String from second registration must be present"
	);
}

/// Specification: `clear_router()` also clears deferred DI registrations.
#[rstest]
#[serial(di_registration)]
fn clear_router_also_clears_di_registrations() {
	// Arrange
	let mut list = DiRegistrationList::new();
	list.register_arc(Arc::new(42_i32));
	register_di_registrations(list);

	// Act
	clear_router();
	let result = take_di_registrations();

	// Assert
	assert!(
		result.is_none(),
		"DI registrations must be cleared by clear_router()"
	);
}
