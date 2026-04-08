//! DI macro compile-time tests
//!
//! Tests compile-time validation of DI macros using trybuild.
//!
//! Note: compile-fail tests have a 300s timeout configured in `.cargo/nextest.toml`
//! because the compiler takes significant time to produce error messages.

/// Test: Compile-fail cases
///
/// Tests that invalid DI usage fails at compile time.
#[test]
fn test_compile_fail_cases() {
	let t = trybuild::TestCases::new();

	// Test: Non-Injectable type resolution should fail
	t.compile_fail("tests/di/ui/fail/invalid_inject_type.rs");

	// Test: Missing Clone trait should fail
	t.compile_fail("tests/di/ui/fail/missing_clone_trait.rs");

	// Test: Unknown macro argument should fail
	t.compile_fail("tests/di/ui/fail/unknown_injectable_arg.rs");

	// Note: circular_dependency compiles but fails at runtime (tested in core_error_handling.rs)
}

/// Test: Compile-pass cases
///
/// Tests that valid DI usage compiles successfully.

#[test]
fn test_compile_pass_cases() {
	let t = trybuild::TestCases::new();

	// Test: Basic Injectable implementation
	t.pass("tests/di/ui/pass/basic_injectable.rs");

	// Test: Injectable with scope attribute
	t.pass("tests/di/ui/pass/injectable_with_scope.rs");

	// Test: Nested dependencies
	t.pass("tests/di/ui/pass/nested_dependencies.rs");

	// Test: Complex types (Vec, HashMap, Option)
	t.pass("tests/di/ui/pass/complex_types.rs");

	// Test: #[injectable_factory] with #[inject] parameters (T and Arc<T>)
	t.pass("tests/di/ui/pass/injectable_factory_inject_params.rs");
}
