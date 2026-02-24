//! Template integration for email content generation
//!
//! This module provides integration with template engines for generating
//! dynamic email content. It supports both plain text and HTML emails.

use crate::{EmailMessage, EmailResult};
use reinhardt_core::security::escape_html;
use std::collections::HashMap;

/// Context for template rendering
pub type TemplateContext = HashMap<String, serde_json::Value>;

/// Trait for template engines
pub trait TemplateEngine: Send + Sync {
	/// Render a template with the given context
	fn render(&self, template: &str, context: &TemplateContext) -> EmailResult<String>;
}

/// Email message builder with template support
///
/// # Examples
///
/// ```rust,no_run
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use reinhardt_mail::templates::{TemplateEmailBuilder, TemplateContext};
/// use reinhardt_mail::EmailMessage;
///
/// let mut context = TemplateContext::new();
/// context.insert("name".to_string(), "Alice".into());
/// context.insert("order_id".to_string(), "12345".into());
///
/// let email = TemplateEmailBuilder::new()
///     .from("orders@example.com")
///     .to(vec!["customer@example.com".to_string()])
///     .subject_template("Order {{order_id}} Confirmation")
///     .body_template("Hello {{name}}, your order {{order_id}} is confirmed.")
///     .html_template("<h1>Hello {{name}}</h1><p>Order {{order_id}} confirmed.</p>")
///     .context(context)
///     .build()?;
/// # Ok(())
/// # }
/// ```
pub struct TemplateEmailBuilder {
	from_email: String,
	to: Vec<String>,
	cc: Vec<String>,
	bcc: Vec<String>,
	reply_to: Vec<String>,
	subject_template: String,
	body_template: String,
	html_template: Option<String>,
	context: TemplateContext,
}

impl TemplateEmailBuilder {
	pub fn new() -> Self {
		Self {
			from_email: String::new(),
			to: Vec::new(),
			cc: Vec::new(),
			bcc: Vec::new(),
			reply_to: Vec::new(),
			subject_template: String::new(),
			body_template: String::new(),
			html_template: None,
			context: HashMap::new(),
		}
	}

	pub fn from(mut self, from: impl Into<String>) -> Self {
		self.from_email = from.into();
		self
	}

	pub fn to(mut self, to: Vec<String>) -> Self {
		self.to = to;
		self
	}

	pub fn cc(mut self, cc: Vec<String>) -> Self {
		self.cc = cc;
		self
	}

	pub fn bcc(mut self, bcc: Vec<String>) -> Self {
		self.bcc = bcc;
		self
	}

	pub fn reply_to(mut self, reply_to: Vec<String>) -> Self {
		self.reply_to = reply_to;
		self
	}

	pub fn subject_template(mut self, template: impl Into<String>) -> Self {
		self.subject_template = template.into();
		self
	}

	pub fn body_template(mut self, template: impl Into<String>) -> Self {
		self.body_template = template.into();
		self
	}

	pub fn html_template(mut self, template: impl Into<String>) -> Self {
		self.html_template = Some(template.into());
		self
	}

	pub fn context(mut self, context: TemplateContext) -> Self {
		self.context = context;
		self
	}

	pub fn add_context(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
		self.context.insert(key.into(), value);
		self
	}

	/// Build the email message with rendered templates using simple string replacement
	///
	/// This is a simple implementation that replaces `{{key}}` with values from context.
	/// Values in HTML templates are automatically HTML-escaped to prevent XSS.
	/// For more advanced template rendering, integrate with a template engine like Tera.
	pub fn build(self) -> EmailResult<EmailMessage> {
		let subject = self.render_simple(&self.subject_template, false)?;
		let body = self.render_simple(&self.body_template, false)?;
		let html_body = if let Some(html_template) = &self.html_template {
			Some(self.render_simple(html_template, true)?)
		} else {
			None
		};

		let mut builder = EmailMessage::builder()
			.from(self.from_email)
			.to(self.to)
			.subject(subject)
			.body(body);

		if !self.cc.is_empty() {
			builder = builder.cc(self.cc);
		}

		if !self.bcc.is_empty() {
			builder = builder.bcc(self.bcc);
		}

		if !self.reply_to.is_empty() {
			builder = builder.reply_to(self.reply_to);
		}

		if let Some(html) = html_body {
			builder = builder.html(html);
		}

		builder.build()
	}

	/// Simple template rendering using string replacement
	///
	/// Replaces `{{key}}` with the corresponding value from the context.
	/// When `html_escape` is true, dynamic values are HTML-escaped to prevent XSS.
	fn render_simple(&self, template: &str, html_escape: bool) -> EmailResult<String> {
		let mut result = template.to_string();

		for (key, value) in &self.context {
			let placeholder = format!("{{{{{}}}}}", key);
			let raw = match value {
				serde_json::Value::String(s) => s.clone(),
				serde_json::Value::Number(n) => n.to_string(),
				serde_json::Value::Bool(b) => b.to_string(),
				serde_json::Value::Null => String::new(),
				_ => value.to_string(),
			};
			let replacement = if html_escape { escape_html(&raw) } else { raw };

			result = result.replace(&placeholder, &replacement);
		}

		Ok(result)
	}
}

impl Default for TemplateEmailBuilder {
	fn default() -> Self {
		Self::new()
	}
}

/// Render a template string with context using simple string replacement
///
/// When `html_escape` is true, dynamic values are HTML-escaped to prevent XSS.
///
/// # Examples
///
/// ```
/// use reinhardt_mail::templates::{render_template, TemplateContext};
///
/// let mut context = TemplateContext::new();
/// context.insert("name".to_string(), "Alice".into());
/// context.insert("age".to_string(), 30.into());
///
/// let result = render_template("Hello {{name}}, you are {{age}} years old.", &context, false).unwrap();
/// assert_eq!(result, "Hello Alice, you are 30 years old.");
/// ```
pub fn render_template(
	template: &str,
	context: &TemplateContext,
	html_escape: bool,
) -> EmailResult<String> {
	let mut result = template.to_string();

	for (key, value) in context {
		let placeholder = format!("{{{{{}}}}}", key);
		let raw = match value {
			serde_json::Value::String(s) => s.clone(),
			serde_json::Value::Number(n) => n.to_string(),
			serde_json::Value::Bool(b) => b.to_string(),
			serde_json::Value::Null => String::new(),
			_ => value.to_string(),
		};
		let replacement = if html_escape { escape_html(&raw) } else { raw };

		result = result.replace(&placeholder, &replacement);
	}

	Ok(result)
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;

	#[rstest]
	fn test_render_template() {
		// Arrange
		let mut context = TemplateContext::new();
		context.insert("name".to_string(), "Alice".into());
		context.insert("age".to_string(), 30.into());

		// Act
		let result = render_template(
			"Hello {{name}}, you are {{age}} years old.",
			&context,
			false,
		)
		.unwrap();

		// Assert
		assert_eq!(result, "Hello Alice, you are 30 years old.");
	}

	#[rstest]
	fn test_render_template_with_boolean() {
		// Arrange
		let mut context = TemplateContext::new();
		context.insert("active".to_string(), true.into());

		// Act
		let result = render_template("Account active: {{active}}", &context, false).unwrap();

		// Assert
		assert_eq!(result, "Account active: true");
	}

	#[rstest]
	fn test_render_template_html_escaping() {
		// Arrange
		let mut context = TemplateContext::new();
		context.insert("name".to_string(), "<script>alert('xss')</script>".into());

		// Act
		let result = render_template("<p>Hello {{name}}</p>", &context, true).unwrap();

		// Assert
		assert_eq!(
			result,
			"<p>Hello &lt;script&gt;alert(&#x27;xss&#x27;)&lt;/script&gt;</p>"
		);
	}

	#[rstest]
	fn test_render_template_no_escape_when_disabled() {
		// Arrange
		let mut context = TemplateContext::new();
		context.insert("name".to_string(), "<b>bold</b>".into());

		// Act
		let result = render_template("Hello {{name}}", &context, false).unwrap();

		// Assert
		assert_eq!(result, "Hello <b>bold</b>");
	}

	#[rstest]
	fn test_template_email_builder() {
		// Arrange
		let mut context = TemplateContext::new();
		context.insert("name".to_string(), "Bob".into());
		context.insert("order_id".to_string(), "12345".into());

		// Act
		let email = TemplateEmailBuilder::new()
			.from("orders@example.com")
			.to(vec!["customer@example.com".to_string()])
			.subject_template("Order {{order_id}} Confirmation")
			.body_template("Hello {{name}}, your order {{order_id}} is confirmed.")
			.context(context)
			.build()
			.unwrap();

		// Assert
		assert_eq!(email.subject(), "Order 12345 Confirmation");
		assert_eq!(email.body(), "Hello Bob, your order 12345 is confirmed.");
	}

	#[rstest]
	fn test_template_email_builder_with_html() {
		// Arrange
		let mut context = TemplateContext::new();
		context.insert("name".to_string(), "Charlie".into());

		// Act
		let email = TemplateEmailBuilder::new()
			.from("noreply@example.com")
			.to(vec!["user@example.com".to_string()])
			.subject_template("Welcome {{name}}")
			.body_template("Welcome {{name}}")
			.html_template("<h1>Welcome {{name}}</h1>")
			.context(context)
			.build()
			.unwrap();

		// Assert
		assert_eq!(email.subject(), "Welcome Charlie");
		assert_eq!(email.html_body(), Some("<h1>Welcome Charlie</h1>"));
	}

	#[rstest]
	fn test_template_email_builder_html_escapes_xss() {
		// Arrange
		let mut context = TemplateContext::new();
		context.insert("name".to_string(), "<script>alert('xss')</script>".into());

		// Act
		let email = TemplateEmailBuilder::new()
			.from("noreply@example.com")
			.to(vec!["user@example.com".to_string()])
			.subject_template("Welcome")
			.body_template("Hello {{name}}")
			.html_template("<h1>Hello {{name}}</h1>")
			.context(context)
			.build()
			.unwrap();

		// Assert - HTML body should be escaped, plain text body should not
		assert_eq!(email.body(), "Hello <script>alert('xss')</script>");
		assert_eq!(
			email.html_body(),
			Some("<h1>Hello &lt;script&gt;alert(&#x27;xss&#x27;)&lt;/script&gt;</h1>")
		);
	}

	#[rstest]
	fn test_add_context() {
		// Arrange / Act
		let email = TemplateEmailBuilder::new()
			.from("test@example.com")
			.to(vec!["user@example.com".to_string()])
			.subject_template("Test {{subject}}")
			.body_template("Body {{body}}")
			.add_context("subject", "Value1".into())
			.add_context("body", "Value2".into())
			.build()
			.unwrap();

		// Assert
		assert_eq!(email.subject(), "Test Value1");
		assert_eq!(email.body(), "Body Value2");
	}
}
