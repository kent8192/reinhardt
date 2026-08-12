#[test]
fn typed_aggregate_contracts() {
	let tests = trybuild::TestCases::new();
	tests.pass("tests/ui/typed_aggregation/pass/*.rs");
	tests.compile_fail("tests/ui/typed_aggregation/fail/*.rs");
}
