//! Compile-time tests for gRPC macros using trybuild
//!
//! This test suite validates that invalid `#[inject]` attribute usage
//! produces correct compile errors.

#[test]
fn test_inject_compile_fail() {
	let t = trybuild::TestCases::new();
	t.compile_fail("tests/ui/inject/fail/*.rs");
}
