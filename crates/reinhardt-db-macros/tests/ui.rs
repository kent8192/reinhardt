//! Trybuild UI tests for reinhardt-db-macros
//!
//! This test file compiles all test cases in tests/ui/pass/ and tests/ui/fail/
//! and verifies that they produce the expected compilation results.

use rstest::rstest;

#[rstest]
fn ui_tests() {
	let t = trybuild::TestCases::new();
	t.pass("tests/ui/pass/basic_document.rs");
	t.pass("tests/ui/pass/field_attributes.rs");
	t.compile_fail("tests/ui/fail/missing_collection.rs");
	t.compile_fail("tests/ui/fail/unsupported_backend.rs");
	t.compile_fail("tests/ui/fail/missing_primary_key.rs");
}
