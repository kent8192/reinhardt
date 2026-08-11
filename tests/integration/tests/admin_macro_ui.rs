//! Compile-time coverage for the `#[admin]` fieldset grammar.

// The macros emit Reinhardt-specific configuration names in this integration test.
#![allow(unexpected_cfgs)]

use reinhardt::admin::ModelAdmin;
use reinhardt::{admin, model};
use rstest::rstest;
use serde::{Deserialize, Serialize};

#[model(app_label = "admin_ui", table_name = "runtime_articles")]
#[derive(Clone, Debug, Serialize, Deserialize)]
struct RuntimeArticle {
	#[field(primary_key = true)]
	id: i64,
	#[field(max_length = 255)]
	name: String,
	#[field(max_length = 255)]
	notes: String,
}

#[admin(model,
	for = RuntimeArticle,
	name = "Article",
	fieldsets = [
		(title = "Main", fields = [name]),
		(fields = [notes], collapsed = true)
	]
)]
struct RuntimeArticleAdmin;

#[rstest]
fn admin_macro_generates_exact_fieldsets() {
	// Arrange
	let admin = RuntimeArticleAdmin;

	// Act
	let fieldsets = admin.fieldsets().unwrap();

	// Assert
	assert_eq!(fieldsets.len(), 2);
	assert_eq!(fieldsets[0].title.as_deref(), Some("Main"));
	assert_eq!(fieldsets[0].fields, vec!["name"]);
	assert_eq!(fieldsets[0].collapsed, false);
	assert_eq!(fieldsets[1].title, None);
	assert_eq!(fieldsets[1].fields, vec!["notes"]);
	assert_eq!(fieldsets[1].collapsed, true);
}

#[rstest]
fn model_admin_fieldsets_ui() {
	// Arrange
	let tests = trybuild::TestCases::new();

	// Act & Assert
	tests.pass("tests/admin/ui/pass/fieldsets.rs");
	tests.compile_fail("tests/admin/ui/fail/fieldsets_unknown_field.rs");
	tests.compile_fail("tests/admin/ui/fail/fields_and_fieldsets.rs");
	tests.compile_fail("tests/admin/ui/fail/fieldsets_empty_group.rs");
	tests.compile_fail("tests/admin/ui/fail/fieldsets_empty.rs");
	tests.compile_fail("tests/admin/ui/fail/fieldsets_duplicate_field.rs");
	tests.compile_fail("tests/admin/ui/fail/fieldsets_duplicate_attribute.rs");
}
