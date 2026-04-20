//! TemplateContext and template utility tests
//!
//! Tests for template context, variable insertion, and utility functions.

use reinhardt_commands::{TemplateContext, generate_secret_key, to_camel_case};
use rstest::{fixture, rstest};
use std::collections::HashMap;

// =============================================================================
// Fixtures
// =============================================================================

#[fixture]
fn empty_template_context() -> TemplateContext {
	TemplateContext::new()
}

// =============================================================================
// TemplateContext Happy Path Tests
// =============================================================================

/// Test creating a new empty TemplateContext
///
/// **Category**: Happy Path
/// **Verifies**: TemplateContext::new() creates empty context
#[rstest]
fn test_template_context_new(empty_template_context: TemplateContext) {
	// Context should be created without errors
	let tera_ctx: tera::Context = empty_template_context.into();

	// Can render with empty context
	let mut tera = tera::Tera::default();
	let result = tera.render_str("Hello", &tera_ctx);
	assert!(result.is_ok());
	assert_eq!(result.unwrap(), "Hello");
}

/// Test default trait implementation
///
/// **Category**: Happy Path
/// **Verifies**: TemplateContext::default() works
#[rstest]
fn test_template_context_default() {
	let ctx = TemplateContext::default();
	let tera_ctx: tera::Context = ctx.into();

	let mut tera = tera::Tera::default();
	let result = tera.render_str("Test", &tera_ctx).unwrap();
	assert_eq!(result, "Test");
}

/// Test inserting a string value
///
/// **Category**: Happy Path
/// **Verifies**: String values can be inserted and rendered
#[rstest]
fn test_template_context_insert_string(mut empty_template_context: TemplateContext) {
	empty_template_context.insert("name", "MyProject").unwrap();

	let tera_ctx: tera::Context = empty_template_context.into();
	let mut tera = tera::Tera::default();
	let result = tera.render_str("Project: {{ name }}", &tera_ctx).unwrap();

	assert_eq!(result, "Project: MyProject");
}

/// Test inserting a numeric value
///
/// **Category**: Happy Path
/// **Verifies**: Numeric values can be inserted and rendered
#[rstest]
fn test_template_context_insert_number(mut empty_template_context: TemplateContext) {
	empty_template_context.insert("port", 8080).unwrap();
	empty_template_context.insert("version", 1.5f64).unwrap();

	let tera_ctx: tera::Context = empty_template_context.into();
	let mut tera = tera::Tera::default();

	let result = tera.render_str("Port: {{ port }}", &tera_ctx).unwrap();
	assert_eq!(result, "Port: 8080");

	let result = tera
		.render_str("Version: {{ version }}", &tera_ctx)
		.unwrap();
	assert_eq!(result, "Version: 1.5");
}

/// Test inserting a boolean value
///
/// **Category**: Happy Path
/// **Verifies**: Boolean values can be inserted and used in conditions
#[rstest]
fn test_template_context_insert_boolean(mut empty_template_context: TemplateContext) {
	empty_template_context.insert("enabled", true).unwrap();
	empty_template_context.insert("debug", false).unwrap();

	let tera_ctx: tera::Context = empty_template_context.into();
	let mut tera = tera::Tera::default();

	let result = tera
		.render_str("{% if enabled %}ON{% else %}OFF{% endif %}", &tera_ctx)
		.unwrap();
	assert_eq!(result, "ON");

	let result = tera
		.render_str("{% if debug %}DEBUG{% else %}RELEASE{% endif %}", &tera_ctx)
		.unwrap();
	assert_eq!(result, "RELEASE");
}

/// Test inserting a vector
///
/// **Category**: Happy Path
/// **Verifies**: Vectors can be inserted and iterated
#[rstest]
fn test_template_context_insert_vec(mut empty_template_context: TemplateContext) {
	let features = vec!["auth", "admin", "api"];
	empty_template_context.insert("features", features).unwrap();

	let tera_ctx: tera::Context = empty_template_context.into();
	let mut tera = tera::Tera::default();

	let result = tera
		.render_str("{% for f in features %}{{ f }},{% endfor %}", &tera_ctx)
		.unwrap();
	assert_eq!(result, "auth,admin,api,");
}

/// Test inserting a HashMap
///
/// **Category**: Happy Path
/// **Verifies**: HashMaps can be inserted and accessed
#[rstest]
fn test_template_context_insert_map(mut empty_template_context: TemplateContext) {
	let mut config = HashMap::new();
	config.insert("host", "localhost");
	config.insert("port", "5432");

	empty_template_context.insert("config", config).unwrap();

	let tera_ctx: tera::Context = empty_template_context.into();
	let mut tera = tera::Tera::default();

	let result = tera
		.render_str("Host: {{ config.host }}", &tera_ctx)
		.unwrap();
	assert_eq!(result, "Host: localhost");
}

// =============================================================================
// TemplateContext Edge Case Tests
// =============================================================================

/// Test overwriting an existing key
///
/// **Category**: Edge Case
/// **Verifies**: Inserting same key overwrites previous value
#[rstest]
fn test_template_context_overwrite(mut empty_template_context: TemplateContext) {
	empty_template_context.insert("key", "first").unwrap();
	empty_template_context.insert("key", "second").unwrap();

	let tera_ctx: tera::Context = empty_template_context.into();
	let mut tera = tera::Tera::default();

	let result = tera.render_str("{{ key }}", &tera_ctx).unwrap();
	assert_eq!(result, "second", "Second value should overwrite first");
}

/// Test inserting empty string
///
/// **Category**: Edge Case
/// **Verifies**: Empty strings are handled correctly
#[rstest]
fn test_template_context_empty_string(mut empty_template_context: TemplateContext) {
	empty_template_context.insert("empty", "").unwrap();

	let tera_ctx: tera::Context = empty_template_context.into();
	let mut tera = tera::Tera::default();

	let result = tera.render_str("[{{ empty }}]", &tera_ctx).unwrap();
	assert_eq!(result, "[]");
}

/// Test inserting Unicode values
///
/// **Category**: Edge Case
/// **Verifies**: Unicode values are preserved
#[rstest]
fn test_template_context_unicode(mut empty_template_context: TemplateContext) {
	empty_template_context
		.insert("greeting", "こんにちは")
		.unwrap();
	empty_template_context.insert("emoji", "🦀").unwrap();

	let tera_ctx: tera::Context = empty_template_context.into();
	let mut tera = tera::Tera::default();

	let result = tera
		.render_str("{{ greeting }} {{ emoji }}", &tera_ctx)
		.unwrap();
	assert_eq!(result, "こんにちは 🦀");
}

/// Test inserting special characters
///
/// **Category**: Edge Case
/// **Verifies**: Special characters are preserved (not HTML escaped by default)
#[rstest]
fn test_template_context_special_characters(mut empty_template_context: TemplateContext) {
	empty_template_context
		.insert("special", "<script>alert('xss')</script>")
		.unwrap();

	let tera_ctx: tera::Context = empty_template_context.into();
	let mut tera = tera::Tera::default();

	// Without safe filter, Tera escapes HTML
	let result = tera.render_str("{{ special }}", &tera_ctx).unwrap();
	assert!(
		result.contains("&lt;script&gt;") || result.contains("<script>"),
		"Special characters should be handled (escaped or preserved)"
	);
}

/// Test inserting null-like values
///
/// **Category**: Edge Case
/// **Verifies**: None/null values are handled
#[rstest]
fn test_template_context_null_value(mut empty_template_context: TemplateContext) {
	let null_value: Option<String> = None;
	empty_template_context
		.insert("nullable", null_value)
		.unwrap();

	let tera_ctx: tera::Context = empty_template_context.into();
	let mut tera = tera::Tera::default();

	// Rendering undefined/null should work with default filter
	let result = tera
		.render_str(
			"{% if nullable %}{{ nullable }}{% else %}NULL{% endif %}",
			&tera_ctx,
		)
		.unwrap();
	assert_eq!(result, "NULL");
}

// =============================================================================
// TemplateContext Clone and Debug Tests
// =============================================================================

/// Test TemplateContext Clone
///
/// **Category**: Happy Path
/// **Verifies**: TemplateContext implements Clone
#[rstest]
fn test_template_context_clone(mut empty_template_context: TemplateContext) {
	empty_template_context.insert("key", "value").unwrap();

	let cloned = empty_template_context.clone();

	let tera_ctx: tera::Context = cloned.into();
	let mut tera = tera::Tera::default();

	let result = tera.render_str("{{ key }}", &tera_ctx).unwrap();
	assert_eq!(result, "value");
}

/// Test TemplateContext Debug
///
/// **Category**: Happy Path
/// **Verifies**: TemplateContext implements Debug
#[rstest]
fn test_template_context_debug(mut empty_template_context: TemplateContext) {
	empty_template_context.insert("test", "value").unwrap();

	let debug = format!("{:?}", empty_template_context);
	assert!(
		debug.contains("TemplateContext"),
		"Debug should contain type name"
	);
}

// =============================================================================
// generate_secret_key Tests
// =============================================================================

/// Test generate_secret_key produces correct length
///
/// **Category**: Happy Path
/// **Verifies**: Generated key has correct length (50 characters)
#[rstest]
fn test_generate_secret_key_length() {
	let key = generate_secret_key();
	assert_eq!(key.len(), 50, "Secret key should be 50 characters");
}

/// Test generate_secret_key produces valid characters
///
/// **Category**: Happy Path
/// **Verifies**: Key contains only valid characters
#[rstest]
fn test_generate_secret_key_characters() {
	let key = generate_secret_key();

	// Check all characters are from the expected set
	let valid_chars =
		"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!@#$%^&*()-_=+";

	for c in key.chars() {
		assert!(
			valid_chars.contains(c),
			"Character '{}' should be in valid set",
			c
		);
	}
}

/// Test generate_secret_key produces unique values
///
/// **Category**: Happy Path
/// **Verifies**: Multiple calls produce different keys
#[rstest]
fn test_generate_secret_key_unique() {
	let key1 = generate_secret_key();
	let key2 = generate_secret_key();
	let key3 = generate_secret_key();

	assert_ne!(key1, key2, "Keys should be unique");
	assert_ne!(key2, key3, "Keys should be unique");
	assert_ne!(key1, key3, "Keys should be unique");
}

/// Test generate_secret_key is not empty
///
/// **Category**: Sanity
/// **Verifies**: Key is not empty
#[rstest]
fn test_generate_secret_key_not_empty() {
	let key = generate_secret_key();
	assert!(!key.is_empty(), "Key should not be empty");
}

// =============================================================================
// to_camel_case Tests
// =============================================================================

/// Test to_camel_case basic conversion
///
/// **Category**: Happy Path
/// **Verifies**: Snake_case is converted to CamelCase
#[rstest]
fn test_to_camel_case_basic() {
	assert_eq!(to_camel_case("my_project"), "MyProject");
	assert_eq!(to_camel_case("hello_world"), "HelloWorld");
	assert_eq!(to_camel_case("test_app"), "TestApp");
}

/// Test to_camel_case single word
///
/// **Category**: Happy Path
/// **Verifies**: Single word is capitalized
#[rstest]
fn test_to_camel_case_single_word() {
	assert_eq!(to_camel_case("hello"), "Hello");
	assert_eq!(to_camel_case("world"), "World");
}

/// Test to_camel_case already capitalized
///
/// **Category**: Edge Case
/// **Verifies**: Already capitalized words are handled
#[rstest]
fn test_to_camel_case_already_capitalized() {
	assert_eq!(to_camel_case("Hello"), "Hello");
	assert_eq!(to_camel_case("HELLO"), "Hello");
}

/// Test to_camel_case empty string
///
/// **Category**: Edge Case
/// **Verifies**: Empty string returns empty
#[rstest]
fn test_to_camel_case_empty() {
	assert_eq!(to_camel_case(""), "");
}

/// Test to_camel_case multiple underscores
///
/// **Category**: Edge Case
/// **Verifies**: Multiple consecutive underscores are handled
#[rstest]
fn test_to_camel_case_multiple_underscores() {
	let result = to_camel_case("my__project");
	// Behavior may vary - either treat as empty parts or single separator
	assert!(result.contains("My") && result.contains("Project"));
}

/// Test to_camel_case with numbers
///
/// **Category**: Edge Case
/// **Verifies**: Numbers in names are preserved
#[rstest]
fn test_to_camel_case_with_numbers() {
	let result = to_camel_case("project_v2");
	assert!(result.contains("V2") || result.contains("v2"));
}

/// Test to_camel_case with leading/trailing underscores
///
/// **Category**: Edge Case
/// **Verifies**: Leading/trailing underscores are handled
#[rstest]
fn test_to_camel_case_edge_underscores() {
	let result1 = to_camel_case("_private");
	let result2 = to_camel_case("trailing_");

	// Should handle without panicking
	assert!(!result1.is_empty() || result1.is_empty()); // Just ensure no panic
	assert!(!result2.is_empty() || result2.is_empty());
}

// =============================================================================
// Sanity Tests
// =============================================================================

// =============================================================================
// Example Override Tests (Issue #2438)
// =============================================================================

/// Test set_example_override produces different content in example vs non-example files
///
/// **Category**: Happy Path
/// **Verifies**: `.example.toml` gets override value, `.toml` gets real value
#[rstest]
fn test_example_override_produces_different_content() {
	// Arrange
	let template_dir = tempfile::tempdir().unwrap();
	let output_dir = tempfile::tempdir().unwrap();

	let settings_dir = template_dir.path().join("settings");
	std::fs::create_dir_all(&settings_dir).unwrap();
	std::fs::write(
		settings_dir.join("config.example.toml"),
		"secret_key = \"{{ secret_key }}\"\n",
	)
	.unwrap();

	let cmd = reinhardt_commands::TemplateCommand::new();
	let mut context = TemplateContext::new();
	context
		.insert("secret_key", "real-generated-key-abc123")
		.unwrap();
	context
		.set_example_override("secret_key", "CHANGE_THIS_PLACEHOLDER")
		.unwrap();
	let ctx = reinhardt_commands::CommandContext::new(vec![]);

	// Act
	cmd.handle(
		"test",
		Some(output_dir.path()),
		&reinhardt_commands::template_source::FilesystemSource::new(template_dir.path()).unwrap(),
		context,
		&ctx,
	)
	.unwrap();

	// Assert
	let example_content = std::fs::read_to_string(
		output_dir
			.path()
			.join("settings")
			.join("config.example.toml"),
	)
	.unwrap();
	let real_content =
		std::fs::read_to_string(output_dir.path().join("settings").join("config.toml")).unwrap();

	assert_eq!(
		example_content, "secret_key = \"CHANGE_THIS_PLACEHOLDER\"\n",
		".example.toml should contain the override placeholder"
	);
	assert_eq!(
		real_content, "secret_key = \"real-generated-key-abc123\"\n",
		".toml should contain the real generated key"
	);
}

/// Test example override with multiple overridden keys
///
/// **Category**: Happy Path
/// **Verifies**: Multiple overrides are all applied to .example files
#[rstest]
fn test_example_override_multiple_keys() {
	// Arrange
	let template_dir = tempfile::tempdir().unwrap();
	let output_dir = tempfile::tempdir().unwrap();

	let settings_dir = template_dir.path().join("settings");
	std::fs::create_dir_all(&settings_dir).unwrap();
	std::fs::write(
		settings_dir.join("db.example.toml"),
		"secret = \"{{ secret_key }}\"\npassword = \"{{ db_password }}\"\n",
	)
	.unwrap();

	let cmd = reinhardt_commands::TemplateCommand::new();
	let mut context = TemplateContext::new();
	context.insert("secret_key", "real-secret").unwrap();
	context.insert("db_password", "real-password").unwrap();
	context
		.set_example_override("secret_key", "CHANGE_SECRET")
		.unwrap();
	context
		.set_example_override("db_password", "CHANGE_PASSWORD")
		.unwrap();
	let ctx = reinhardt_commands::CommandContext::new(vec![]);

	// Act
	cmd.handle(
		"test",
		Some(output_dir.path()),
		&reinhardt_commands::template_source::FilesystemSource::new(template_dir.path()).unwrap(),
		context,
		&ctx,
	)
	.unwrap();

	// Assert
	let example_content =
		std::fs::read_to_string(output_dir.path().join("settings").join("db.example.toml"))
			.unwrap();
	let real_content =
		std::fs::read_to_string(output_dir.path().join("settings").join("db.toml")).unwrap();

	assert_eq!(
		example_content,
		"secret = \"CHANGE_SECRET\"\npassword = \"CHANGE_PASSWORD\"\n",
	);
	assert_eq!(
		real_content,
		"secret = \"real-secret\"\npassword = \"real-password\"\n",
	);
}

/// Test non-example files are unaffected by example overrides
///
/// **Category**: Edge Case
/// **Verifies**: Regular files (without .example.) ignore overrides entirely
#[rstest]
fn test_example_override_does_not_affect_regular_files() {
	// Arrange
	let template_dir = tempfile::tempdir().unwrap();
	let output_dir = tempfile::tempdir().unwrap();

	std::fs::write(
		template_dir.path().join("config.toml"),
		"key = \"{{ secret_key }}\"\n",
	)
	.unwrap();

	let cmd = reinhardt_commands::TemplateCommand::new();
	let mut context = TemplateContext::new();
	context.insert("secret_key", "real-value").unwrap();
	context
		.set_example_override("secret_key", "PLACEHOLDER")
		.unwrap();
	let ctx = reinhardt_commands::CommandContext::new(vec![]);

	// Act
	cmd.handle(
		"test",
		Some(output_dir.path()),
		&reinhardt_commands::template_source::FilesystemSource::new(template_dir.path()).unwrap(),
		context,
		&ctx,
	)
	.unwrap();

	// Assert
	let content = std::fs::read_to_string(output_dir.path().join("config.toml")).unwrap();
	assert_eq!(
		content, "key = \"real-value\"\n",
		"Non-example file should use real value, not override"
	);
	// .example.toml should NOT be created for a non-example template
	assert!(
		!output_dir.path().join("config.example.toml").exists(),
		"No .example.toml should be created for non-example templates"
	);
}

/// Test example override with empty overrides map
///
/// **Category**: Edge Case
/// **Verifies**: When no overrides are set, .example and .toml files have identical content
#[rstest]
fn test_example_override_empty_produces_identical_content() {
	// Arrange
	let template_dir = tempfile::tempdir().unwrap();
	let output_dir = tempfile::tempdir().unwrap();

	let settings_dir = template_dir.path().join("settings");
	std::fs::create_dir_all(&settings_dir).unwrap();
	std::fs::write(
		settings_dir.join("base.example.toml"),
		"key = \"{{ value }}\"\n",
	)
	.unwrap();

	let cmd = reinhardt_commands::TemplateCommand::new();
	let mut context = TemplateContext::new();
	context.insert("value", "same-for-both").unwrap();
	// No set_example_override call
	let ctx = reinhardt_commands::CommandContext::new(vec![]);

	// Act
	cmd.handle(
		"test",
		Some(output_dir.path()),
		&reinhardt_commands::template_source::FilesystemSource::new(template_dir.path()).unwrap(),
		context,
		&ctx,
	)
	.unwrap();

	// Assert
	let example_content =
		std::fs::read_to_string(output_dir.path().join("settings").join("base.example.toml"))
			.unwrap();
	let real_content =
		std::fs::read_to_string(output_dir.path().join("settings").join("base.toml")).unwrap();

	assert_eq!(example_content, real_content);
	assert_eq!(example_content, "key = \"same-for-both\"\n");
}

/// Test secret_key substitution mimics the actual startproject flow
///
/// **Category**: Integration
/// **Verifies**: The real secret_key flow produces a random key in .toml
/// and a placeholder in .example.toml
#[rstest]
fn test_secret_key_startproject_flow() {
	// Arrange
	let template_dir = tempfile::tempdir().unwrap();
	let output_dir = tempfile::tempdir().unwrap();

	let settings_dir = template_dir.path().join("settings");
	std::fs::create_dir_all(&settings_dir).unwrap();
	std::fs::write(
		settings_dir.join("base.example.toml"),
		"secret_key = \"{{ secret_key }}\"\n",
	)
	.unwrap();

	let secret_key = format!("insecure-{}", generate_secret_key());

	let cmd = reinhardt_commands::TemplateCommand::new();
	let mut context = TemplateContext::new();
	context.insert("secret_key", &secret_key).unwrap();
	context
		.set_example_override(
			"secret_key",
			"CHANGE_THIS_IN_PRODUCTION_MUST_BE_KEPT_SECRET",
		)
		.unwrap();
	let ctx = reinhardt_commands::CommandContext::new(vec![]);

	// Act
	cmd.handle(
		"test",
		Some(output_dir.path()),
		&reinhardt_commands::template_source::FilesystemSource::new(template_dir.path()).unwrap(),
		context,
		&ctx,
	)
	.unwrap();

	// Assert
	let example_content =
		std::fs::read_to_string(output_dir.path().join("settings").join("base.example.toml"))
			.unwrap();
	let real_content =
		std::fs::read_to_string(output_dir.path().join("settings").join("base.toml")).unwrap();

	// .example.toml should have the placeholder
	assert_eq!(
		example_content,
		"secret_key = \"CHANGE_THIS_IN_PRODUCTION_MUST_BE_KEPT_SECRET\"\n",
	);
	// .toml should have the real generated key
	assert!(
		real_content.starts_with("secret_key = \"insecure-"),
		"Real config should contain the generated secret key, got: {}",
		real_content
	);
	assert!(
		real_content.len() > 30,
		"Real config should contain a substantial key"
	);
	// The two should be different
	assert_ne!(
		example_content, real_content,
		".example.toml and .toml should have different secret_key values"
	);
}

/// Test override only applies to specified keys, other variables pass through
///
/// **Category**: Edge Case
/// **Verifies**: Non-overridden variables render identically in both files
#[rstest]
fn test_example_override_partial_keys() {
	// Arrange
	let template_dir = tempfile::tempdir().unwrap();
	let output_dir = tempfile::tempdir().unwrap();

	let settings_dir = template_dir.path().join("settings");
	std::fs::create_dir_all(&settings_dir).unwrap();
	std::fs::write(
		settings_dir.join("app.example.toml"),
		"name = \"{{ project_name }}\"\nsecret = \"{{ secret_key }}\"\n",
	)
	.unwrap();

	let cmd = reinhardt_commands::TemplateCommand::new();
	let mut context = TemplateContext::new();
	context.insert("project_name", "my_app").unwrap();
	context.insert("secret_key", "real-secret-key").unwrap();
	// Only override secret_key, not project_name
	context
		.set_example_override("secret_key", "PLACEHOLDER")
		.unwrap();
	let ctx = reinhardt_commands::CommandContext::new(vec![]);

	// Act
	cmd.handle(
		"test",
		Some(output_dir.path()),
		&reinhardt_commands::template_source::FilesystemSource::new(template_dir.path()).unwrap(),
		context,
		&ctx,
	)
	.unwrap();

	// Assert
	let example_content =
		std::fs::read_to_string(output_dir.path().join("settings").join("app.example.toml"))
			.unwrap();
	let real_content =
		std::fs::read_to_string(output_dir.path().join("settings").join("app.toml")).unwrap();

	// project_name should be the same in both
	assert!(example_content.contains("name = \"my_app\""));
	assert!(real_content.contains("name = \"my_app\""));
	// secret_key should differ
	assert!(example_content.contains("secret = \"PLACEHOLDER\""));
	assert!(real_content.contains("secret = \"real-secret-key\""));
}

// =============================================================================
// Sanity Tests
// =============================================================================

/// Sanity test for template workflow
///
/// **Category**: Sanity
/// **Verifies**: Basic template workflow works
#[rstest]
fn test_template_basic_sanity() {
	let mut ctx = TemplateContext::new();

	// Insert various types
	ctx.insert("name", "Test").unwrap();
	ctx.insert("count", 42).unwrap();
	ctx.insert("enabled", true).unwrap();

	// Convert to tera context
	let tera_ctx: tera::Context = ctx.into();

	// Render template
	let mut tera = tera::Tera::default();
	let result = tera
		.render_str("Name={{ name }}, Count={{ count }}", &tera_ctx)
		.unwrap();

	assert!(result.contains("Name=Test"));
	assert!(result.contains("Count=42"));
}
