//! DI macro compile-time tests
//!
//! Tests compile-time validation of DI macros using trybuild.
//!
//! Note: compile-fail tests have a 300s timeout configured in `.cargo/nextest.toml`
//! because the compiler takes significant time to produce error messages.

/// Test: Compile-fail cases for #[injectable]
///
/// Tests that invalid DI usage fails at compile time.
#[test]
fn test_injectable_compile_fail_cases() {
	let t = trybuild::TestCases::new();

	// Note: Non-Injectable type resolution is now a runtime error (not compile-time)
	// because Depends::resolve() no longer requires T: Injectable at compile time.
	// Runtime detection is tested in core_error_handling.rs.

	// Test: Missing Clone trait should fail
	t.compile_fail("tests/di/ui/fail/missing_clone_trait.rs");

	// Test: Unknown macro argument should fail
	t.compile_fail("tests/di/ui/fail/unknown_injectable_arg.rs");

	// Note: circular_dependency compiles but fails at runtime (tested in core_error_handling.rs)
}

/// Test: Compile-fail cases for #[injectable_factory]
///
/// Tests that invalid injectable_factory usage is rejected at compile time.
#[test]
fn test_injectable_factory_compile_fail_cases() {
	let t = trybuild::TestCases::new();

	// Test: Non-async function should fail
	t.compile_fail("tests/di/ui/fail/injectable_factory_sync_fn.rs");

	// Test: Missing return type should fail
	t.compile_fail("tests/di/ui/fail/injectable_factory_no_return_type.rs");

	// Test: Non-inject parameters should fail
	t.compile_fail("tests/di/ui/fail/injectable_factory_non_inject_params.rs");

	// Test: Invalid scope value should fail
	t.compile_fail("tests/di/ui/fail/injectable_factory_invalid_scope.rs");

	// Test: Mixed inject and non-inject params should fail
	t.compile_fail("tests/di/ui/fail/injectable_factory_mixed_params.rs");
}

/// Test: Compile-pass cases for #[injectable]
///
/// Tests that valid DI usage compiles successfully.
#[test]
fn test_injectable_compile_pass_cases() {
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

// Note: Compile-pass tests for #[injectable_factory] are NOT possible via trybuild
// because the macro generates `inventory::submit!` which requires linker-level static
// initialization that trybuild's isolated compilation environment cannot support (E0015).
// Instead, compile-pass coverage for injectable_factory is provided by integration tests
// in auto_injection_basic.rs and registry_tests.rs.
