//! Compile-fail tests for the `#[injectable]` macro
//!
//! Uses trybuild to verify that invalid usages produce clear compiler errors.

#[test]
fn injectable_compile_fail() {
	let t = trybuild::TestCases::new();
	t.compile_fail("tests/compile_fail/*.rs");
}
