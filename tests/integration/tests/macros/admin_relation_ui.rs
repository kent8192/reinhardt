//! Compile-time contracts for admin relation field aliases.

#[test]
fn admin_relation_field_aliases_compile() {
	let tests = trybuild::TestCases::new();
	tests.pass("tests/macros/ui/pass/admin_relation_physical_alias.rs");
}
