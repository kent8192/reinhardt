#[cfg(feature = "migrations")]
#[test]
fn migration_operation_source_compat() {
	let tests = trybuild::TestCases::new();
	tests.pass("tests/ui/drop_constraint_legacy.rs");
	tests.pass("tests/ui/create_index_legacy.rs");
}
