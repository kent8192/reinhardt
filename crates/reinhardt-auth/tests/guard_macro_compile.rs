//! Compile-time coverage tests for the `guard!` procedural macro.

#[test]
fn guard_macro_compile_cases() {
	let test_cases = trybuild::TestCases::new();
	test_cases.pass("tests/ui/guard/pass/precedence.rs");
	test_cases.compile_fail("tests/ui/guard/fail/invalid_syntax.rs");
	test_cases.compile_fail("tests/ui/guard/fail/escaped_has_perm.rs");
}
