//! Form generation for API endpoints

use reinhardt_core::security::xss::{escape_html, escape_html_attr};
use serde_json::Value;
use std::collections::HashMap;

/// HTML form generator for POST/PUT/PATCH operations
///
/// # Examples
///
/// ```
/// use reinhardt_views::browsable_api::FormGenerator;
/// use serde_json::json;
///
/// let mut generator = FormGenerator::new("/api/users/", "POST");
/// generator.add_field("username", "text", true);
/// generator.add_field("email", "email", true);
/// let html = generator.generate().unwrap();
/// assert!(html.contains("<form"));
/// assert!(html.contains("username"));
/// ```
#[derive(Debug, Clone)]
pub struct FormGenerator {
	action: String,
	method: String,
	fields: Vec<FormField>,
	csrf_token: Option<String>,
	errors: HashMap<String, Vec<String>>,
}

/// Represents a form field
#[derive(Debug, Clone)]
pub struct FormField {
	name: String,
	field_type: String,
	required: bool,
	label: Option<String>,
	placeholder: Option<String>,
	default_value: Option<String>,
	help_text: Option<String>,
}

/// Options for configuring a form field
#[derive(Debug, Clone, Default)]
pub struct FieldOptions {
	/// Field label (defaults to field name if not provided)
	pub label: Option<String>,
	/// Placeholder text
	pub placeholder: Option<String>,
	/// Default value
	pub default_value: Option<String>,
	/// Help text to display below the field
	pub help_text: Option<String>,
}

impl FormGenerator {
	/// Create a new form generator
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::browsable_api::FormGenerator;
	///
	/// let generator = FormGenerator::new("/api/items/", "POST");
	/// ```
	pub fn new(action: impl Into<String>, method: impl Into<String>) -> Self {
		Self {
			action: action.into(),
			method: method.into(),
			fields: Vec::new(),
			csrf_token: None,
			errors: HashMap::new(),
		}
	}

	/// Add a field to the form
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::browsable_api::FormGenerator;
	///
	/// let mut generator = FormGenerator::new("/api/items/", "POST");
	/// generator.add_field("name", "text", true);
	/// generator.add_field("description", "textarea", false);
	/// ```
	pub fn add_field(
		&mut self,
		name: impl Into<String>,
		field_type: impl Into<String>,
		required: bool,
	) -> &mut Self {
		self.fields.push(FormField {
			name: name.into(),
			field_type: field_type.into(),
			required,
			label: None,
			placeholder: None,
			default_value: None,
			help_text: None,
		});
		self
	}

	/// Add a field with full configuration
	pub fn add_field_full(
		&mut self,
		name: impl Into<String>,
		field_type: impl Into<String>,
		required: bool,
		options: FieldOptions,
	) -> &mut Self {
		self.fields.push(FormField {
			name: name.into(),
			field_type: field_type.into(),
			required,
			label: options.label,
			placeholder: options.placeholder,
			default_value: options.default_value,
			help_text: options.help_text,
		});
		self
	}

	/// Set CSRF token
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::browsable_api::FormGenerator;
	///
	/// let mut generator = FormGenerator::new("/api/items/", "POST");
	/// generator.set_csrf_token("token123");
	/// ```
	pub fn set_csrf_token(&mut self, token: impl Into<String>) -> &mut Self {
		self.csrf_token = Some(token.into());
		self
	}

	/// Add validation errors
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::browsable_api::FormGenerator;
	///
	/// let mut generator = FormGenerator::new("/api/items/", "POST");
	/// generator.add_error("email", "Invalid email format");
	/// ```
	pub fn add_error(&mut self, field: impl Into<String>, error: impl Into<String>) -> &mut Self {
		self.errors
			.entry(field.into())
			.or_default()
			.push(error.into());
		self
	}

	/// Generate the HTML form
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::browsable_api::FormGenerator;
	///
	/// let mut generator = FormGenerator::new("/api/users/", "POST");
	/// generator.add_field("username", "text", true);
	/// let html = generator.generate().unwrap();
	/// assert!(html.contains("<form"));
	/// ```
	pub fn generate(&self) -> Result<String, String> {
		let mut html = String::new();

		// Form opening tag - escape action and method for HTML attributes
		html.push_str(&format!(
			r#"<form action="{}" method="{}" class="api-form">"#,
			escape_html_attr(&self.action),
			escape_html_attr(&self.method)
		));
		html.push('\n');

		// CSRF token - escape for HTML attribute
		if let Some(token) = &self.csrf_token {
			html.push_str(&format!(
				r#"  <input type="hidden" name="csrfmiddlewaretoken" value="{}">"#,
				escape_html_attr(token)
			));
			html.push('\n');
		}

		// Fields
		for field in &self.fields {
			html.push_str("  <div class=\"form-group\">\n");

			// Label - escape for HTML content and attribute
			let label = field
				.label
				.as_ref()
				.unwrap_or(&field.name)
				.replace('_', " ");
			let required_marker = if field.required { " *" } else { "" };
			html.push_str(&format!(
				"    <label for=\"{}\">{}{}</label>\n",
				escape_html_attr(&field.name),
				escape_html(&label),
				required_marker
			));

			// Field input
			match field.field_type.as_str() {
				"textarea" => {
					let placeholder_attr = field
						.placeholder
						.as_ref()
						.map(|p| format!(" placeholder=\"{}\"", escape_html_attr(p)))
						.unwrap_or_default();
					let default_val = escape_html(field.default_value.as_deref().unwrap_or(""));
					html.push_str(&format!(
						"    <textarea id=\"{}\" name=\"{}\" class=\"form-control\"{}{}>{}</textarea>\n",
						escape_html_attr(&field.name),
						escape_html_attr(&field.name),
						if field.required { " required" } else { "" },
						placeholder_attr,
						default_val
					));
				}
				_ => {
					let placeholder_attr = field
						.placeholder
						.as_ref()
						.map(|p| format!(" placeholder=\"{}\"", escape_html_attr(p)))
						.unwrap_or_default();
					let value_attr = field
						.default_value
						.as_ref()
						.map(|v| format!(" value=\"{}\"", escape_html_attr(v)))
						.unwrap_or_default();
					html.push_str(&format!(
						"    <input type=\"{}\" id=\"{}\" name=\"{}\" class=\"form-control\"{}{}{}>\n",
						escape_html_attr(&field.field_type),
						escape_html_attr(&field.name),
						escape_html_attr(&field.name),
						if field.required { " required" } else { "" },
						placeholder_attr,
						value_attr
					));
				}
			}

			// Help text - escape for HTML content
			if let Some(help) = &field.help_text {
				html.push_str(&format!(
					"    <small class=\"form-text text-muted\">{}</small>\n",
					escape_html(help)
				));
			}

			// Errors - escape for HTML content
			if let Some(errors) = self.errors.get(&field.name) {
				for error in errors {
					html.push_str(&format!(
						"    <div class=\"invalid-feedback d-block\">{}</div>\n",
						escape_html(error)
					));
				}
			}

			html.push_str("  </div>\n");
		}

		// Submit button - method is safe as it's controlled by code
		html.push_str("  <div class=\"form-group\">\n");
		html.push_str(&format!(
			"    <button type=\"submit\" class=\"btn btn-primary\">{}</button>\n",
			self.method.to_uppercase()
		));
		html.push_str("  </div>\n");

		// Form closing tag
		html.push_str("</form>\n");

		Ok(html)
	}

	/// Generate form from a JSON schema
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_views::browsable_api::FormGenerator;
	/// use serde_json::json;
	///
	/// let schema = json!({
	///     "username": {"type": "string", "required": true},
	///     "age": {"type": "integer", "required": false}
	/// });
	/// let generator = FormGenerator::from_schema("/api/users/", "POST", &schema);
	/// ```
	pub fn from_schema(
		action: impl Into<String>,
		method: impl Into<String>,
		schema: &Value,
	) -> Self {
		let mut generator = Self::new(action, method);

		if let Some(properties) = schema.as_object() {
			for (name, field_schema) in properties {
				let field_type = field_schema
					.get("type")
					.and_then(|t| t.as_str())
					.unwrap_or("text");
				let required = field_schema
					.get("required")
					.and_then(|r| r.as_bool())
					.unwrap_or(false);

				let html_type = match field_type {
					"integer" | "number" => "number",
					"boolean" => "checkbox",
					"email" => "email",
					_ => "text",
				};

				generator.add_field(name, html_type, required);
			}
		}

		generator
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde_json::json;

	#[test]
	fn test_form_generator_creation() {
		let generator = FormGenerator::new("/api/test/", "POST");
		assert_eq!(generator.action, "/api/test/");
		assert_eq!(generator.method, "POST");
		assert!(generator.fields.is_empty());
	}

	#[test]
	fn test_add_field() {
		let mut generator = FormGenerator::new("/api/test/", "POST");
		generator.add_field("username", "text", true);
		assert_eq!(generator.fields.len(), 1);
		assert_eq!(generator.fields[0].name, "username");
		assert!(generator.fields[0].required);
	}

	#[test]
	fn test_set_csrf_token() {
		let mut generator = FormGenerator::new("/api/test/", "POST");
		generator.set_csrf_token("test_token");
		assert_eq!(generator.csrf_token, Some("test_token".to_string()));
	}

	#[test]
	fn test_add_error() {
		let mut generator = FormGenerator::new("/api/test/", "POST");
		generator.add_error("email", "Invalid email");
		assert!(generator.errors.contains_key("email"));
		assert_eq!(generator.errors["email"].len(), 1);
	}

	#[test]
	fn test_generate_basic_form() {
		let mut generator = FormGenerator::new("/api/users/", "POST");
		generator.add_field("username", "text", true);
		let html = generator.generate().unwrap();
		assert!(html.contains("<form"));
		assert!(html.contains("username"));
		assert!(html.contains("</form>"));
	}

	#[test]
	fn test_generate_with_csrf() {
		let mut generator = FormGenerator::new("/api/users/", "POST");
		generator.set_csrf_token("token123");
		generator.add_field("name", "text", false);
		let html = generator.generate().unwrap();
		assert!(html.contains("csrfmiddlewaretoken"));
		assert!(html.contains("token123"));
	}

	#[test]
	fn test_generate_with_errors() {
		let mut generator = FormGenerator::new("/api/users/", "POST");
		generator.add_field("email", "email", true);
		generator.add_error("email", "Invalid format");
		let html = generator.generate().unwrap();
		assert!(html.contains("Invalid format"));
		assert!(html.contains("invalid-feedback"));
	}

	#[test]
	fn test_from_schema() {
		let schema = json!({
			"username": {"type": "string", "required": true},
			"age": {"type": "integer", "required": false}
		});
		let generator = FormGenerator::from_schema("/api/users/", "POST", &schema);
		assert_eq!(generator.fields.len(), 2);
	}
}
