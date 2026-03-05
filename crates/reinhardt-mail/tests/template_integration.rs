//! Template integration tests
//!
//! Tests template rendering for email content, covering template rendering,
//! context injection, variable substitution, HTML templates, and error handling.

use reinhardt_mail::templates::{TemplateContext, TemplateEmailBuilder, render_template};
use serde_json::json;

/// Test: Basic template rendering with string replacement
#[test]
fn test_render_template_basic() {
	let mut context = TemplateContext::new();
	context.insert("name".to_string(), json!("Alice"));
	context.insert("city".to_string(), json!("Tokyo"));

	let template = "Hello {{name}}, welcome to {{city}}!";
	let result = render_template(template, &context, false).expect("Failed to render template");

	assert_eq!(result, "Hello Alice, welcome to Tokyo!");
}

/// Test: Template rendering with numbers
#[test]
fn test_render_template_numbers() {
	let mut context = TemplateContext::new();
	context.insert("age".to_string(), json!(30));
	context.insert("score".to_string(), json!(95.5));

	let template = "Age: {{age}}, Score: {{score}}";
	let result = render_template(template, &context, false).expect("Failed to render");

	assert_eq!(result, "Age: 30, Score: 95.5");
}

/// Test: Template rendering with boolean values
#[test]
fn test_render_template_boolean() {
	let mut context = TemplateContext::new();
	context.insert("active".to_string(), json!(true));
	context.insert("verified".to_string(), json!(false));

	let template = "Active: {{active}}, Verified: {{verified}}";
	let result = render_template(template, &context, false).expect("Failed to render");

	assert_eq!(result, "Active: true, Verified: false");
}

/// Test: Template rendering with null values
#[test]
fn test_render_template_null() {
	let mut context = TemplateContext::new();
	context.insert("missing".to_string(), json!(null));

	let template = "Value: {{missing}}";
	let result = render_template(template, &context, false).expect("Failed to render");

	assert_eq!(result, "Value: ");
}

/// Test: Template rendering with missing variables (no replacement)
#[test]
fn test_render_template_missing_variables() {
	let context = TemplateContext::new();

	let template = "Hello {{name}}, you have {{count}} messages.";
	let result = render_template(template, &context, false).expect("Failed to render");

	// Missing variables remain as-is
	assert_eq!(result, "Hello {{name}}, you have {{count}} messages.");
}

/// Test: TemplateEmailBuilder basic construction
#[test]
fn test_template_email_builder_basic() {
	let mut context = TemplateContext::new();
	context.insert("name".to_string(), json!("Bob"));
	context.insert("order_id".to_string(), json!("12345"));

	let message = TemplateEmailBuilder::new()
		.from("orders@example.com")
		.to(vec!["customer@example.com".to_string()])
		.subject_template("Order {{order_id}} Confirmation")
		.body_template("Hello {{name}}, your order {{order_id}} is confirmed.")
		.context(context)
		.build()
		.expect("Failed to build email");

	assert_eq!(message.subject(), "Order 12345 Confirmation");
	assert_eq!(message.body(), "Hello Bob, your order 12345 is confirmed.");
	assert_eq!(message.from_email(), "orders@example.com");
	assert_eq!(message.to(), vec!["customer@example.com"]);
}

/// Test: TemplateEmailBuilder with HTML template
#[test]
fn test_template_email_builder_html() {
	let mut context = TemplateContext::new();
	context.insert("username".to_string(), json!("Charlie"));
	context.insert(
		"reset_link".to_string(),
		json!("https://example.com/reset/abc123"),
	);

	let message = TemplateEmailBuilder::new()
		.from("noreply@example.com")
		.to(vec!["user@example.com".to_string()])
		.subject_template("Password Reset for {{username}}")
		.body_template("Click the link to reset your password: {{reset_link}}")
		.html_template("<html><body><h1>Hello {{username}}</h1><p><a href='{{reset_link}}'>Reset Password</a></p></body></html>")
		.context(context)
		.build()
		.expect("Failed to build HTML email");

	assert_eq!(message.subject(), "Password Reset for Charlie");
	assert_eq!(
		message.body(),
		"Click the link to reset your password: https://example.com/reset/abc123"
	);
	assert!(message.html_body().is_some());
	let html = message.html_body().unwrap();
	assert!(html.contains("Hello Charlie"));
	assert!(html.contains("https://example.com/reset/abc123"));
}

/// Test: TemplateEmailBuilder with add_context method
#[test]
fn test_template_email_builder_add_context() {
	let message = TemplateEmailBuilder::new()
		.from("app@example.com")
		.to(vec!["user@example.com".to_string()])
		.subject_template("Welcome {{name}}")
		.body_template("Your account number is {{account_id}}")
		.add_context("name", json!("David"))
		.add_context("account_id", json!(987654))
		.build()
		.expect("Failed to build");

	assert_eq!(message.subject(), "Welcome David");
	assert_eq!(message.body(), "Your account number is 987654");
}

/// Test: TemplateEmailBuilder with CC and BCC
#[test]
fn test_template_email_builder_cc_bcc() {
	let mut context = TemplateContext::new();
	context.insert("subject".to_string(), json!("Newsletter"));

	let message = TemplateEmailBuilder::new()
		.from("newsletter@example.com")
		.to(vec!["subscriber@example.com".to_string()])
		.cc(vec!["cc@example.com".to_string()])
		.bcc(vec!["bcc@example.com".to_string()])
		.reply_to(vec!["support@example.com".to_string()])
		.subject_template("{{subject}}")
		.body_template("This is a newsletter")
		.context(context)
		.build()
		.expect("Failed to build");

	assert_eq!(message.cc(), vec!["cc@example.com"]);
	assert_eq!(message.bcc(), vec!["bcc@example.com"]);
	assert_eq!(message.reply_to(), vec!["support@example.com"]);
}

/// Test: Template rendering with complex nested content
#[test]
fn test_template_complex_content() {
	let mut context = TemplateContext::new();
	context.insert("first_name".to_string(), json!("Eve"));
	context.insert("last_name".to_string(), json!("Johnson"));
	context.insert("company".to_string(), json!("TechCorp"));
	context.insert("position".to_string(), json!("Engineer"));

	let template = "Dear {{first_name}} {{last_name}},\n\nCongratulations on your new position as {{position}} at {{company}}!";
	let result = render_template(template, &context, false).expect("Failed to render");

	assert!(result.contains("Dear Eve Johnson"));
	assert!(result.contains("Engineer at TechCorp"));
}

/// Test: Template rendering with UTF-8 content
#[test]
fn test_template_utf8_content() {
	let mut context = TemplateContext::new();
	context.insert("name".to_string(), json!("太郎"));
	context.insert("product".to_string(), json!("ノートパソコン"));

	let template = "{{name}}様、{{product}}のご注文ありがとうございます。";
	let result = render_template(template, &context, false).expect("Failed to render");

	assert_eq!(
		result,
		"太郎様、ノートパソコンのご注文ありがとうございます。"
	);
}

/// Test: TemplateEmailBuilder with empty templates
#[test]
fn test_template_email_builder_empty_templates() {
	let message = TemplateEmailBuilder::new()
		.from("sender@example.com")
		.to(vec!["recipient@example.com".to_string()])
		.subject_template("")
		.body_template("")
		.build()
		.expect("Failed to build");

	assert_eq!(message.subject(), "");
	assert_eq!(message.body(), "");
}

/// Test: TemplateEmailBuilder with multiple variables in one field
#[test]
fn test_template_multiple_variables() {
	let mut context = TemplateContext::new();
	context.insert("first".to_string(), json!("Hello"));
	context.insert("second".to_string(), json!("World"));
	context.insert("third".to_string(), json!("Test"));

	let message = TemplateEmailBuilder::new()
		.from("test@example.com")
		.to(vec!["user@example.com".to_string()])
		.subject_template("{{first}} {{second}} {{third}}")
		.body_template("{{first}}, {{second}}! This is a {{third}}.")
		.context(context)
		.build()
		.expect("Failed to build");

	assert_eq!(message.subject(), "Hello World Test");
	assert_eq!(message.body(), "Hello, World! This is a Test.");
}

/// Test: Template rendering with special characters
#[test]
fn test_template_special_characters() {
	let mut context = TemplateContext::new();
	context.insert("email".to_string(), json!("user@example.com"));
	context.insert(
		"url".to_string(),
		json!("https://example.com/path?query=value&other=123"),
	);

	let template = "Contact: {{email}}\nVisit: {{url}}";
	let result = render_template(template, &context, false).expect("Failed to render");

	assert!(result.contains("user@example.com"));
	assert!(result.contains("https://example.com/path?query=value&other=123"));
}

/// Test: TemplateEmailBuilder default instance
#[test]
fn test_template_email_builder_default() {
	let builder = TemplateEmailBuilder::default();
	let message = builder.build().expect("Failed to build default");

	assert_eq!(message.subject(), "");
	assert_eq!(message.body(), "");
	assert_eq!(message.from_email(), "");
	assert!(message.to().is_empty());
}
