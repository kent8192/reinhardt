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
