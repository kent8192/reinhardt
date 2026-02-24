//! Compile-time tests for macros using trybuild
//!
//! This test suite validates that:
//! - Valid macro usage compiles successfully (tests/ui/*/pass/*.rs)
//! - Invalid macro usage fails to compile (tests/ui/*/fail/*.rs)
//!
//! ## Test Strategy
//!
//! To avoid circular dependencies during crates.io publishing, these tests avoid
//! depending on other workspace crates (reinhardt-db, reinhardt-di, reinhardt-admin, etc.).
//!
//! ### Approach
//!
//! - **Fail tests**: Validate that macros produce correct error messages for invalid usage.
//!   Required type definitions are inlined directly in test files.
//! - **Pass tests**: Only include tests that don't require external dependencies.
//!   Full integration testing happens in `tests/integration/` crate.
//! - **Removed tests**: Model and relationship fail tests were removed because they
//!   require `reinhardt_core` dependency, defeating the purpose of avoiding circular deps.
//!
//! This approach ensures macro error messages are correct without creating
//! circular dependencies, while runtime behavior is tested in `tests/integration/`.

#[test]
fn test_compile_pass() {
	let t = trybuild::TestCases::new();
	t.pass("tests/ui/pass/*.rs");
}

#[test]
fn test_compile_fail() {
	let t = trybuild::TestCases::new();
	t.compile_fail("tests/ui/fail/*.rs");
}

#[test]
fn test_path_macro_pass() {
	let t = trybuild::TestCases::new();
	t.pass("tests/ui/path/pass/*.rs");
}

#[test]
fn test_path_macro_fail() {
	let t = trybuild::TestCases::new();
	t.compile_fail("tests/ui/path/fail/*.rs");
}

#[test]
fn test_permission_macro_fail() {
	let t = trybuild::TestCases::new();
	t.compile_fail("tests/ui/permissions/fail/*.rs");
}

#[test]
fn test_routes_macro_fail() {
	let t = trybuild::TestCases::new();
	t.compile_fail("tests/ui/routes/fail/*.rs");
}

#[test]
fn test_action_macro_pass() {
	let t = trybuild::TestCases::new();
	t.pass("tests/ui/action/pass/*.rs");
}

#[test]
fn test_action_macro_fail() {
	let t = trybuild::TestCases::new();
	t.compile_fail("tests/ui/action/fail/*.rs");
}

#[test]
fn test_validation_edge_cases_fail() {
	let t = trybuild::TestCases::new();
	t.compile_fail("tests/ui/validation_edge_cases/multiple_invalid_methods.rs");
	t.compile_fail("tests/ui/validation_edge_cases/action_missing_both.rs");
	t.compile_fail("tests/ui/validation_edge_cases/action_url_path_no_slash.rs");
}

#[test]
fn test_validation_edge_cases_pass() {
	let t = trybuild::TestCases::new();
	t.pass("tests/ui/validation_edge_cases/case_sensitive_method.rs");
	t.pass("tests/ui/validation_edge_cases/empty_methods.rs");
}

// api_view macro tests
#[test]
fn test_api_view_macro_pass() {
	let t = trybuild::TestCases::new();
	t.pass("tests/ui/api_view/pass/*.rs");
}

#[test]
fn test_api_view_macro_fail() {
	let t = trybuild::TestCases::new();
	t.compile_fail("tests/ui/api_view/fail/*.rs");
}

// ===== Injectable =====

#[test]
fn test_injectable_macro_fail() {
	let t = trybuild::TestCases::new();
	t.compile_fail("tests/ui/injectable/fail/*.rs");
}

// ===== Admin =====

#[test]
fn test_admin_fail() {
	let t = trybuild::TestCases::new();
	t.compile_fail("tests/ui/admin/fail/*.rs");
}

// ===== AppConfig =====

#[test]
fn test_app_config_fail() {
	let t = trybuild::TestCases::new();
	t.compile_fail("tests/ui/app_config/fail/*.rs");
}
