//! Unit tests for FunctionHandle

use reinhardt_di::InjectionContext;
use reinhardt_test::fixtures::*;
use rstest::*;
use std::sync::Arc;

// Test factory functions
fn create_test_string() -> String {
	"production".to_string()
}

fn create_test_number() -> i32 {
	42
}

fn create_another_string() -> String {
	"another_production".to_string()
}

#[rstest]
fn function_handle_stores_function(singleton_scope: Arc<reinhardt_di::SingletonScope>) {
	// Arrange
	let ctx = InjectionContext::builder(singleton_scope).build();

	// Act
	let handle = ctx.dependency(create_test_string);

	// Assert
	assert_eq!(handle.func_ptr(), create_test_string as usize);
}

#[rstest]
fn function_handle_executes(singleton_scope: Arc<reinhardt_di::SingletonScope>) {
	// Arrange
	let ctx = InjectionContext::builder(singleton_scope).build();

	// Act - Set override
	ctx.dependency(create_test_string)
		.override_with("mock".to_string());

	// Assert - Override can be retrieved
	let override_value = ctx.dependency(create_test_string).get_override();
	assert_eq!(override_value, Some("mock".to_string()));
}

#[rstest]
fn function_handle_with_dependencies(singleton_scope: Arc<reinhardt_di::SingletonScope>) {
	// Arrange
	let ctx = InjectionContext::builder(singleton_scope).build();

	// Act - Override multiple dependencies
	ctx.dependency(create_test_string)
		.override_with("string_mock".to_string());
	ctx.dependency(create_test_number).override_with(100);

	// Assert
	assert!(ctx.dependency(create_test_string).has_override());
	assert!(ctx.dependency(create_test_number).has_override());

	let string_override = ctx.dependency(create_test_string).get_override();
	let number_override = ctx.dependency(create_test_number).get_override();

	assert_eq!(string_override, Some("string_mock".to_string()));
	assert_eq!(number_override, Some(100));
}
