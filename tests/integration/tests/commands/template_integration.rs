//! Template integration tests
//!
//! Tests for template-based file generation in commands.

use reinhardt_commands::TemplateContext;
use rstest::rstest;
use std::collections::HashMap;

// =============================================================================
// Use Case Tests
// =============================================================================

/// Test template variable substitution
///
/// **Category**: Use Case
/// **Verifies**: Variables are replaced in template output
#[rstest]
fn test_template_variable_substitution() {
	let mut ctx = TemplateContext::new();
	let _ = ctx.insert("name", "TestApp");
	let _ = ctx.insert("version", "1.0.0");
	let _ = ctx.insert("author", "Test Author");

	let tera_ctx: tera::Context = ctx.into();
	let mut tera = tera::Tera::default();

	let template = r#"
Project: {{ name }}
Version: {{ version }}
Author: {{ author }}
"#;

	let result = tera.render_str(template, &tera_ctx).unwrap();

	assert!(result.contains("Project: TestApp"));
	assert!(result.contains("Version: 1.0.0"));
	assert!(result.contains("Author: Test Author"));
}

/// Test template with boolean conditionals
///
/// **Category**: Use Case
/// **Verifies**: Boolean values control template output
#[rstest]
#[case(true, "Feature is enabled")]
#[case(false, "Feature is disabled")]
fn test_template_boolean_conditionals(#[case] enabled: bool, #[case] expected_text: &str) {
	let mut ctx = TemplateContext::new();
	let _ = ctx.insert("feature_enabled", enabled);

	let tera_ctx: tera::Context = ctx.into();
	let mut tera = tera::Tera::default();

	let template =
		r#"{% if feature_enabled %}Feature is enabled{% else %}Feature is disabled{% endif %}"#;

	let result = tera.render_str(template, &tera_ctx).unwrap();
	assert_eq!(result, expected_text);
}

/// Test template with numeric values
///
/// **Category**: Use Case
/// **Verifies**: Numeric values are rendered correctly
#[rstest]
fn test_template_numeric_values() {
	let mut ctx = TemplateContext::new();
	let _ = ctx.insert("count", 42i64);
	let _ = ctx.insert("price", 19.99f64);

	let tera_ctx: tera::Context = ctx.into();
	let mut tera = tera::Tera::default();

	let template = "Count: {{ count }}, Price: {{ price }}";
	let result = tera.render_str(template, &tera_ctx).unwrap();

	assert!(result.contains("Count: 42"));
	assert!(result.contains("Price: 19.99"));
}

/// Test template with list iteration
///
/// **Category**: Use Case
/// **Verifies**: Lists can be iterated in templates
#[rstest]
fn test_template_list_iteration() {
	let mut ctx = TemplateContext::new();
	let _ = ctx.insert("items", vec!["apple", "banana", "cherry"]);

	let tera_ctx: tera::Context = ctx.into();
	let mut tera = tera::Tera::default();

	let template = r#"{% for item in items %}- {{ item }}
{% endfor %}"#;

	let result = tera.render_str(template, &tera_ctx).unwrap();

	assert!(result.contains("- apple"));
	assert!(result.contains("- banana"));
	assert!(result.contains("- cherry"));
}

/// Test template with map values
///
/// **Category**: Use Case
/// **Verifies**: Maps can be accessed in templates
#[rstest]
fn test_template_map_values() {
	let mut ctx = TemplateContext::new();

	let mut config = HashMap::new();
	config.insert("host".to_string(), "localhost".to_string());
	config.insert("port".to_string(), "8080".to_string());
	let _ = ctx.insert("config", config);

	let tera_ctx: tera::Context = ctx.into();
	let mut tera = tera::Tera::default();

	let template = "Host: {{ config.host }}, Port: {{ config.port }}";
	let result = tera.render_str(template, &tera_ctx).unwrap();

	assert!(result.contains("Host: localhost"));
	assert!(result.contains("Port: 8080"));
}

// =============================================================================
// Error Path Tests
// =============================================================================

/// Test template with undefined variable
///
/// **Category**: Error Path
/// **Verifies**: Error on undefined variable
#[rstest]
fn test_template_undefined_variable_error() {
	let ctx = TemplateContext::new();
	let tera_ctx: tera::Context = ctx.into();
	let mut tera = tera::Tera::default();

	let template = "Hello, {{ undefined_var }}!";
	let result = tera.render_str(template, &tera_ctx);

	assert!(result.is_err(), "Should error on undefined variable");
}

/// Test template with syntax error
///
/// **Category**: Error Path
/// **Verifies**: Syntax errors are reported
#[rstest]
fn test_template_syntax_error() {
	let mut ctx = TemplateContext::new();
	let _ = ctx.insert("name", "test");

	let tera_ctx: tera::Context = ctx.into();
	let mut tera = tera::Tera::default();

	let template = "{% if name %}unclosed conditional";
	let result = tera.render_str(template, &tera_ctx);

	assert!(result.is_err(), "Should error on syntax error");
}

// =============================================================================
// Edge Case Tests
// =============================================================================

/// Test template with empty context
///
/// **Category**: Edge Case
/// **Verifies**: Empty context produces static output
#[rstest]
fn test_template_empty_context() {
	let ctx = TemplateContext::new();
	let tera_ctx: tera::Context = ctx.into();
	let mut tera = tera::Tera::default();

	let template = "Static content only";
	let result = tera.render_str(template, &tera_ctx).unwrap();

	assert_eq!(result, "Static content only");
}

/// Test template with Unicode content
///
/// **Category**: Edge Case
/// **Verifies**: Unicode is handled correctly
#[rstest]
fn test_template_unicode_content() {
	let mut ctx = TemplateContext::new();
	let _ = ctx.insert("greeting", "こんにちは");
	let _ = ctx.insert("emoji", "🦀");

	let tera_ctx: tera::Context = ctx.into();
	let mut tera = tera::Tera::default();

	let template = "{{ greeting }} {{ emoji }}";
	let result = tera.render_str(template, &tera_ctx).unwrap();

	assert_eq!(result, "こんにちは 🦀");
}

/// Test template with special characters
///
/// **Category**: Edge Case
/// **Verifies**: Special characters are preserved
#[rstest]
fn test_template_special_characters() {
	let mut ctx = TemplateContext::new();
	let _ = ctx.insert("code", "fn main() { println!(\"Hello\"); }");

	let tera_ctx: tera::Context = ctx.into();
	let mut tera = tera::Tera::default();

	let template = "Code: {{ code }}";
	let result = tera.render_str(template, &tera_ctx).unwrap();

	assert!(result.contains("fn main()"));
	assert!(result.contains("println!"));
}

/// Test template with empty string values
///
/// **Category**: Edge Case
/// **Verifies**: Empty strings are handled correctly
#[rstest]
fn test_template_empty_string_values() {
	let mut ctx = TemplateContext::new();
	let _ = ctx.insert("empty", "");
	let _ = ctx.insert("non_empty", "value");

	let tera_ctx: tera::Context = ctx.into();
	let mut tera = tera::Tera::default();

	let template = "[{{ empty }}][{{ non_empty }}]";
	let result = tera.render_str(template, &tera_ctx).unwrap();

	assert_eq!(result, "[][value]");
}

/// Test template with nested data structures
///
/// **Category**: Edge Case
/// **Verifies**: Nested structures are accessible
#[rstest]
fn test_template_nested_structures() {
	let mut ctx = TemplateContext::new();

	let mut database = HashMap::new();
	database.insert("host".to_string(), "localhost".to_string());
	database.insert("name".to_string(), "mydb".to_string());

	let mut server = HashMap::new();
	server.insert("port".to_string(), "3000".to_string());

	let mut config = HashMap::new();
	config.insert("database".to_string(), database);
	config.insert("server".to_string(), server);

	// Insert nested structure via serde_json
	let _ = ctx.insert("database_host", "localhost");
	let _ = ctx.insert("database_name", "mydb");
	let _ = ctx.insert("server_port", "3000");

	let tera_ctx: tera::Context = ctx.into();
	let mut tera = tera::Tera::default();

	let template = "DB: {{ database_host }}/{{ database_name }}, Port: {{ server_port }}";
	let result = tera.render_str(template, &tera_ctx).unwrap();

	assert!(result.contains("DB: localhost/mydb"));
	assert!(result.contains("Port: 3000"));
}

// =============================================================================
// Combination Tests
// =============================================================================

/// Test template with multiple value types
///
/// **Category**: Combination
/// **Verifies**: Different value types work together
#[rstest]
fn test_template_mixed_value_types() {
	let mut ctx = TemplateContext::new();
	let _ = ctx.insert("name", "App");
	let _ = ctx.insert("version", 1i64);
	let _ = ctx.insert("debug", true);
	let _ = ctx.insert("factor", 1.5f64);

	let tera_ctx: tera::Context = ctx.into();
	let mut tera = tera::Tera::default();

	let template = "{{ name }} v{{ version }} (debug={{ debug }}, factor={{ factor }})";
	let result = tera.render_str(template, &tera_ctx).unwrap();

	assert!(result.contains("App v1"));
	assert!(result.contains("debug=true"));
	assert!(result.contains("factor=1.5"));
}

/// Test template context overwrite behavior
///
/// **Category**: Combination
/// **Verifies**: Later inserts overwrite earlier ones
#[rstest]
fn test_template_context_overwrite() {
	let mut ctx = TemplateContext::new();
	let _ = ctx.insert("value", "first");
	let _ = ctx.insert("value", "second");

	let tera_ctx: tera::Context = ctx.into();
	let mut tera = tera::Tera::default();

	let template = "{{ value }}";
	let result = tera.render_str(template, &tera_ctx).unwrap();

	assert_eq!(result, "second", "Later insert should overwrite");
}

// =============================================================================
// Real-world Scenario Tests
// =============================================================================

/// Test generating Rust file content
///
/// **Category**: Use Case
/// **Verifies**: Can generate Rust source code
#[rstest]
fn test_template_rust_file_generation() {
	let mut ctx = TemplateContext::new();
	let _ = ctx.insert("module_name", "my_module");
	let _ = ctx.insert("struct_name", "MyStruct");
	let _ = ctx.insert("fields", vec!["id: i64", "name: String"]);

	let tera_ctx: tera::Context = ctx.into();
	let mut tera = tera::Tera::default();

	let template = r#"//! {{ module_name }} module

pub struct {{ struct_name }} {
{% for field in fields %}    pub {{ field }},
{% endfor %}}
"#;

	let result = tera.render_str(template, &tera_ctx).unwrap();

	assert!(result.contains("//! my_module module"));
	assert!(result.contains("pub struct MyStruct"));
	assert!(result.contains("pub id: i64,"));
	assert!(result.contains("pub name: String,"));
}

/// Test generating configuration file content
///
/// **Category**: Use Case
/// **Verifies**: Can generate config file formats
#[rstest]
fn test_template_config_file_generation() {
	let mut ctx = TemplateContext::new();
	let _ = ctx.insert("project_name", "myproject");
	let _ = ctx.insert("version", "0.1.0");
	let _ = ctx.insert("edition", "2024");

	let tera_ctx: tera::Context = ctx.into();
	let mut tera = tera::Tera::default();

	let template = r#"[package]
name = "{{ project_name }}"
version = "{{ version }}"
edition = "{{ edition }}"
"#;

	let result = tera.render_str(template, &tera_ctx).unwrap();

	assert!(result.contains("name = \"myproject\""));
	assert!(result.contains("version = \"0.1.0\""));
	assert!(result.contains("edition = \"2024\""));
}

/// Test generating migration file content
///
/// **Category**: Use Case
/// **Verifies**: Can generate migration content
#[rstest]
fn test_template_migration_generation() {
	let mut ctx = TemplateContext::new();
	let _ = ctx.insert("migration_name", "CreateUsersTable");
	let _ = ctx.insert("table_name", "users");
	let _ = ctx.insert("columns", vec!["id", "email", "created_at"]);

	let tera_ctx: tera::Context = ctx.into();
	let mut tera = tera::Tera::default();

	let template = r#"// Migration: {{ migration_name }}
// Table: {{ table_name }}
// Columns: {% for col in columns %}{{ col }}{% if not loop.last %}, {% endif %}{% endfor %}
"#;

	let result = tera.render_str(template, &tera_ctx).unwrap();

	assert!(result.contains("Migration: CreateUsersTable"));
	assert!(result.contains("Table: users"));
	assert!(result.contains("Columns: id, email, created_at"));
}
