//! Compile-time coverage for the `#[admin]` fieldset grammar.

// The macros emit Reinhardt-specific configuration names in this integration test.
#![allow(unexpected_cfgs)]

use reinhardt::admin::{AdminForm, AdminWidget, FormFieldOverride, ModelAdmin, PrepopulatedField};
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

#[derive(Debug, Default)]
struct RuntimeArticleForm;

impl AdminForm for RuntimeArticleForm {}

#[admin(model,
	for = RuntimeArticle,
	name = "Article",
	fieldsets = [
		(title = "Main", fields = [name]),
		(fields = [notes], collapsed = true)
	],
	form = RuntimeArticleForm,
	formfield_overrides = [
		(name, widget = text_input, label = "Headline", help_text = "Displayed title", placeholder = "Enter a headline", required = false),
		(notes, widget = textarea, rows = 7),
	],
	prepopulated_fields = [(notes, sources = [name])]
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
fn admin_macro_generates_form_customization() {
	// Arrange
	let admin = RuntimeArticleAdmin;

	// Act
	let form = admin.form().expect("configured form should be returned");
	let form_again = admin.form().expect("configured form should be stable");
	let overrides = admin.formfield_overrides();
	let prepopulated = admin.prepopulated_fields();

	// Assert
	assert!(std::ptr::eq(form, form_again));
	assert_eq!(
		overrides,
		vec![
			FormFieldOverride::new("name")
				.widget(AdminWidget::TextInput)
				.label("Headline")
				.help_text("Displayed title")
				.placeholder("Enter a headline")
				.required(false),
			FormFieldOverride::new("notes").widget(AdminWidget::TextArea { rows: Some(7) }),
		]
	);
	assert_eq!(
		prepopulated,
		vec![PrepopulatedField::new("notes", ["name"])]
	);
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
	tests.compile_fail("tests/admin/ui/fail/fieldsets_duplicate_top_level.rs");
	tests.pass("tests/admin/ui/pass/form_customization.rs");
	tests.compile_fail("tests/admin/ui/fail/form_customization_unknown_field.rs");
	tests.compile_fail("tests/admin/ui/fail/form_customization_duplicate_target.rs");
	tests.compile_fail("tests/admin/ui/fail/form_customization_duplicate_setting.rs");
	tests.compile_fail("tests/admin/ui/fail/form_customization_malformed_choices.rs");
	tests.compile_fail("tests/admin/ui/fail/form_customization_form_bound.rs");
}
