//! Compile-time coverage for ORM QuerySet examples shown in website docs.

#[test]
fn queryset_docs_example_compile_pass() {
	let t = trybuild::TestCases::new();
	t.pass("tests/orm/ui/pass/queryset_docs_example.rs");
	t.pass("tests/orm/ui/pass/queryset_typed_retrieval.rs");
	t.pass("tests/orm/ui/pass/model_enum_typed_filter.rs");
	t.pass("tests/orm/ui/pass/legacy_field_codec_compat.rs");
	t.compile_fail("tests/orm/ui/fail/model_enum_string_filter.rs");
	t.compile_fail("tests/orm/ui/fail/model_enum_integer_filter.rs");
	t.compile_fail("tests/orm/ui/fail/queryset_latest_cross_model_field.rs");
	t.compile_fail("tests/orm/ui/fail/queryset_relationship_ordering.rs");
	t.compile_fail("tests/orm/ui/fail/queryset_in_bulk_non_unique_field.rs");
}
