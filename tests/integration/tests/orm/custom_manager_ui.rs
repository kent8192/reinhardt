//! Compile-time tests for `#[model(manager = ...)]` (Issue #3980).
//!
//! These tests use trybuild to verify both pass and fail behaviors of the
//! `manager = <Path>` attribute argument added to `#[model(...)]`.
//!
//! Pass: a valid manager type wires up `Model::custom_manager()` correctly.
//! Fail: invalid manager paths and type-mismatched managers are rejected at
//! compile time, surfacing actionable error messages to the developer.

#[test]
fn custom_manager_compile_pass() {
	let t = trybuild::TestCases::new();
	t.pass("tests/orm/ui/pass/manager_argument.rs");
}

#[test]
fn custom_manager_compile_fail_invalid_path() {
	let t = trybuild::TestCases::new();
	t.compile_fail("tests/orm/ui/fail/manager_invalid_path.rs");
}

#[test]
fn custom_manager_compile_fail_wrong_model() {
	let t = trybuild::TestCases::new();
	t.compile_fail("tests/orm/ui/fail/manager_wrong_model.rs");
}
