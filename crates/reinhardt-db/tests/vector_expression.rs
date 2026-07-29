#[test]
fn vector_expression_compile_contracts() {
	let tests = trybuild::TestCases::new();
	tests.pass("tests/ui/vector_expression/pass/*.rs");
	tests.compile_fail("tests/ui/vector_expression/fail/*.rs");
}
