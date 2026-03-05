use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Escapes special HTML characters to prevent XSS attacks.
///
/// This function converts the following characters to their HTML entity equivalents:
/// - `&` → `&amp;`
/// - `<` → `&lt;`
/// - `>` → `&gt;`
/// - `"` → `&quot;`
/// - `'` → `&#x27;`
///
/// # Examples
///
/// ```
/// use reinhardt_forms::field::html_escape;
///
/// assert_eq!(html_escape("<script>"), "&lt;script&gt;");
/// assert_eq!(html_escape("a & b"), "a &amp; b");
/// assert_eq!(html_escape("\"quoted\""), "&quot;quoted&quot;");
/// ```
pub fn html_escape(s: &str) -> String {
	s.replace('&', "&amp;")
		.replace('<', "&lt;")
		.replace('>', "&gt;")
		.replace('"', "&quot;")
		.replace('\'', "&#x27;")
}

/// Escapes a value for use in an HTML attribute context.
/// This is an alias for [`html_escape`] as the escaping rules are the same.
///
/// # Examples
///
/// ```
/// use reinhardt_forms::field::escape_attribute;
///
/// assert_eq!(escape_attribute("on\"click"), "on&quot;click");
/// ```
pub fn escape_attribute(s: &str) -> String {
	html_escape(s)
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ErrorType {
	Required,
	Invalid,
	MinLength,
	MaxLength,
	MinValue,
	MaxValue,
	Custom(String),
}

#[derive(Debug, thiserror::Error)]
pub enum FieldError {
	#[error("{0}")]
	Required(String),
	#[error("{0}")]
	Invalid(String),
	#[error("{0}")]
	Validation(String),
}

pub type FieldResult<T> = Result<T, FieldError>;

impl FieldError {
	/// Creates a required field error
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::FieldError;
	///
	/// let error = FieldError::required(None);
	/// assert_eq!(error.to_string(), "This field is required.");
	///
	/// let custom_error = FieldError::required(Some("Name is mandatory"));
	/// assert_eq!(custom_error.to_string(), "Name is mandatory");
	/// ```
	pub fn required(custom_msg: Option<&str>) -> Self {
		FieldError::Required(custom_msg.unwrap_or("This field is required.").to_string())
	}
	/// Creates an invalid field error
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::FieldError;
	///
	/// let error = FieldError::invalid(None, "Invalid input format");
	/// assert_eq!(error.to_string(), "Invalid input format");
	///
	/// let custom_error = FieldError::invalid(Some("Must be a number"), "Invalid");
	/// assert_eq!(custom_error.to_string(), "Must be a number");
	/// ```
	pub fn invalid(custom_msg: Option<&str>, default_msg: &str) -> Self {
		FieldError::Invalid(custom_msg.unwrap_or(default_msg).to_string())
	}
	/// Creates a validation field error
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::FieldError;
	///
	/// let error = FieldError::validation(None, "Value out of range");
	/// assert_eq!(error.to_string(), "Value out of range");
	///
	/// let custom_error = FieldError::validation(Some("Too long"), "Length exceeded");
	/// assert_eq!(custom_error.to_string(), "Too long");
	/// ```
	pub fn validation(custom_msg: Option<&str>, default_msg: &str) -> Self {
		FieldError::Validation(custom_msg.unwrap_or(default_msg).to_string())
	}
}

/// Field widget type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Widget {
	TextInput,
	PasswordInput,
	EmailInput,
	NumberInput,
	TextArea,
	Select { choices: Vec<(String, String)> },
	CheckboxInput,
	RadioSelect { choices: Vec<(String, String)> },
	DateInput,
	DateTimeInput,
	FileInput,
	HiddenInput,
}

impl Widget {
	/// Renders the widget as HTML with XSS protection.
	///
	/// All user-controlled values (name, value, attributes, choices) are
	/// HTML-escaped to prevent Cross-Site Scripting (XSS) attacks.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::Widget;
	///
	/// let widget = Widget::TextInput;
	/// let html = widget.render_html("username", Some("john_doe"), None);
	/// assert!(html.contains("<input"));
	/// assert!(html.contains("type=\"text\""));
	/// assert!(html.contains("name=\"username\""));
	/// assert!(html.contains("value=\"john_doe\""));
	/// ```
	///
	/// # XSS Protection
	///
	/// ```
	/// use reinhardt_forms::Widget;
	///
	/// let widget = Widget::TextInput;
	/// // Malicious input is escaped
	/// let html = widget.render_html("field", Some("<script>alert('xss')</script>"), None);
	/// assert!(!html.contains("<script>"));
	/// assert!(html.contains("&lt;script&gt;"));
	/// ```
	pub fn render_html(
		&self,
		name: &str,
		value: Option<&str>,
		attrs: Option<&HashMap<String, String>>,
	) -> String {
		let mut html = String::new();
		let default_attrs = HashMap::new();
		let attrs = attrs.unwrap_or(&default_attrs);

		// Escape name for use in attributes
		let escaped_name = escape_attribute(name);

		// Build common attributes with escaping
		let mut common_attrs = String::new();
		for (key, val) in attrs {
			common_attrs.push_str(&format!(
				" {}=\"{}\"",
				escape_attribute(key),
				escape_attribute(val)
			));
		}

		match self {
			Widget::TextInput => {
				html.push_str(&format!(
					"<input type=\"text\" name=\"{}\" value=\"{}\"{}",
					escaped_name,
					escape_attribute(value.unwrap_or("")),
					common_attrs
				));
				if !attrs.contains_key("id") {
					html.push_str(&format!(" id=\"id_{}\"", escaped_name));
				}
				html.push_str(" />");
			}
			Widget::PasswordInput => {
				// Security: Password fields NEVER render the value attribute
				// to prevent password leakage in HTML source
				html.push_str(&format!(
					"<input type=\"password\" name=\"{}\"{}",
					escaped_name, common_attrs
				));
				if !attrs.contains_key("id") {
					html.push_str(&format!(" id=\"id_{}\"", escaped_name));
				}
				html.push_str(" />");
			}
			Widget::EmailInput => {
				html.push_str(&format!(
					"<input type=\"email\" name=\"{}\" value=\"{}\"{}",
					escaped_name,
					escape_attribute(value.unwrap_or("")),
					common_attrs
				));
				if !attrs.contains_key("id") {
					html.push_str(&format!(" id=\"id_{}\"", escaped_name));
				}
				html.push_str(" />");
			}
			Widget::NumberInput => {
				html.push_str(&format!(
					"<input type=\"number\" name=\"{}\" value=\"{}\"{}",
					escaped_name,
					escape_attribute(value.unwrap_or("")),
					common_attrs
				));
				if !attrs.contains_key("id") {
					html.push_str(&format!(" id=\"id_{}\"", escaped_name));
				}
				html.push_str(" />");
			}
			Widget::TextArea => {
				html.push_str(&format!(
					"<textarea name=\"{}\"{}",
					escaped_name, common_attrs
				));
				if !attrs.contains_key("id") {
					html.push_str(&format!(" id=\"id_{}\"", escaped_name));
				}
				html.push('>');
				// TextArea content is in HTML body context - must escape
				html.push_str(&html_escape(value.unwrap_or("")));
				html.push_str("</textarea>");
			}
			Widget::Select { choices } => {
				html.push_str(&format!(
					"<select name=\"{}\"{}",
					escaped_name, common_attrs
				));
				if !attrs.contains_key("id") {
					html.push_str(&format!(" id=\"id_{}\"", escaped_name));
				}
				html.push('>');
				for (choice_value, choice_label) in choices {
					let selected = if Some(choice_value.as_str()) == value {
						" selected"
					} else {
						""
					};
					html.push_str(&format!(
						"<option value=\"{}\"{}>{}</option>",
						escape_attribute(choice_value),
						selected,
						html_escape(choice_label)
					));
				}
				html.push_str("</select>");
			}
			Widget::CheckboxInput => {
				html.push_str(&format!(
					"<input type=\"checkbox\" name=\"{}\"",
					escaped_name
				));
				if value == Some("true") || value == Some("on") {
					html.push_str(" checked");
				}
				html.push_str(&common_attrs);
				if !attrs.contains_key("id") {
					html.push_str(&format!(" id=\"id_{}\"", escaped_name));
				}
				html.push_str(" />");
			}
			Widget::RadioSelect { choices } => {
				for (i, (choice_value, choice_label)) in choices.iter().enumerate() {
					let checked = if Some(choice_value.as_str()) == value {
						" checked"
					} else {
						""
					};
					html.push_str(&format!(
						"<input type=\"radio\" name=\"{}\" value=\"{}\" id=\"id_{}_{}\"{}{} />",
						escaped_name,
						escape_attribute(choice_value),
						escaped_name,
						i,
						checked,
						common_attrs
					));
					html.push_str(&format!(
						"<label for=\"id_{}_{}\">{}</label>",
						escaped_name,
						i,
						html_escape(choice_label)
					));
				}
			}
			Widget::DateInput => {
				html.push_str(&format!(
					"<input type=\"date\" name=\"{}\" value=\"{}\"{}",
					escaped_name,
					escape_attribute(value.unwrap_or("")),
					common_attrs
				));
				if !attrs.contains_key("id") {
					html.push_str(&format!(" id=\"id_{}\"", escaped_name));
				}
				html.push_str(" />");
			}
			Widget::DateTimeInput => {
				html.push_str(&format!(
					"<input type=\"datetime-local\" name=\"{}\" value=\"{}\"{}",
					escaped_name,
					escape_attribute(value.unwrap_or("")),
					common_attrs
				));
				if !attrs.contains_key("id") {
					html.push_str(&format!(" id=\"id_{}\"", escaped_name));
				}
				html.push_str(" />");
			}
			Widget::FileInput => {
				html.push_str(&format!(
					"<input type=\"file\" name=\"{}\"{}",
					escaped_name, common_attrs
				));
				if !attrs.contains_key("id") {
					html.push_str(&format!(" id=\"id_{}\"", escaped_name));
				}
				html.push_str(" />");
			}
			Widget::HiddenInput => {
				html.push_str(&format!(
					"<input type=\"hidden\" name=\"{}\" value=\"{}\" />",
					escaped_name,
					escape_attribute(value.unwrap_or(""))
				));
			}
		}

		html
	}
}

/// Base field trait for forms
///
/// This trait is specifically for form fields. For ORM fields, use `reinhardt_db::orm::Field`.
pub trait FormField: Send + Sync {
	fn name(&self) -> &str;
	fn label(&self) -> Option<&str>;
	fn required(&self) -> bool;
	fn help_text(&self) -> Option<&str>;
	fn widget(&self) -> &Widget;
	fn initial(&self) -> Option<&serde_json::Value>;

	fn clean(&self, value: Option<&serde_json::Value>) -> FieldResult<serde_json::Value>;

	/// Check if the field value has changed from its initial value
	fn has_changed(
		&self,
		initial: Option<&serde_json::Value>,
		data: Option<&serde_json::Value>,
	) -> bool {
		// Default implementation: compare values directly
		match (initial, data) {
			(None, None) => false,
			(Some(_), None) | (None, Some(_)) => true,
			(Some(i), Some(d)) => i != d,
		}
	}

	/// Get custom error messages for this field
	fn error_messages(&self) -> HashMap<ErrorType, String> {
		HashMap::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	// Note: Field-specific tests have been moved to their respective field modules
	// in the fields/ directory. Only FormField trait tests remain here.

	#[test]
	fn test_field_has_changed() {
		use crate::fields::CharField;

		let field = CharField::new("name".to_string());

		// No change: both None
		assert!(!field.has_changed(None, None));

		// Change: initial None, data Some
		assert!(field.has_changed(None, Some(&serde_json::json!("John"))));

		// Change: initial Some, data None
		assert!(field.has_changed(Some(&serde_json::json!("John")), None));

		// No change: same value
		assert!(!field.has_changed(
			Some(&serde_json::json!("John")),
			Some(&serde_json::json!("John"))
		));

		// Change: different value
		assert!(field.has_changed(
			Some(&serde_json::json!("John")),
			Some(&serde_json::json!("Jane"))
		));
	}

	#[test]
	fn test_field_error_messages() {
		use crate::fields::CharField;

		let field = CharField::new("name".to_string());

		// Default implementation returns empty HashMap
		assert!(field.error_messages().is_empty());
	}

	// ============================================================================
	// XSS Prevention Tests (Issue #547)
	// ============================================================================

	#[test]
	fn test_html_escape_basic() {
		assert_eq!(html_escape("<script>"), "&lt;script&gt;");
		assert_eq!(html_escape("a & b"), "a &amp; b");
		assert_eq!(html_escape("\"quoted\""), "&quot;quoted&quot;");
		assert_eq!(html_escape("'single'"), "&#x27;single&#x27;");
	}

	#[test]
	fn test_html_escape_all_special_chars() {
		let input = "<script>alert('xss')&\"</script>";
		let expected = "&lt;script&gt;alert(&#x27;xss&#x27;)&amp;&quot;&lt;/script&gt;";
		assert_eq!(html_escape(input), expected);
	}

	#[test]
	fn test_html_escape_no_special_chars() {
		assert_eq!(html_escape("normal text"), "normal text");
		assert_eq!(html_escape(""), "");
	}

	#[test]
	fn test_escape_attribute() {
		assert_eq!(escape_attribute("on\"click"), "on&quot;click");
		assert_eq!(
			escape_attribute("javascript:alert('xss')"),
			"javascript:alert(&#x27;xss&#x27;)"
		);
	}

	#[test]
	fn test_widget_render_html_escapes_value_in_text_input() {
		let widget = Widget::TextInput;
		let xss_payload = "\"><script>alert('xss')</script>";
		let html = widget.render_html("field", Some(xss_payload), None);

		// Should NOT contain raw script tag
		assert!(!html.contains("<script>"));
		// Should contain escaped version
		assert!(html.contains("&lt;script&gt;"));
		assert!(html.contains("&quot;"));
	}

	#[test]
	fn test_widget_render_html_escapes_name() {
		let widget = Widget::TextInput;
		let xss_name = "field\"><script>alert('xss')</script>";
		let html = widget.render_html(xss_name, Some("value"), None);

		// Should NOT contain raw script tag
		assert!(!html.contains("<script>"));
		// Should contain escaped version
		assert!(html.contains("&lt;script&gt;"));
	}

	#[test]
	fn test_widget_render_html_escapes_textarea_content() {
		let widget = Widget::TextArea;
		let xss_content = "</textarea><script>alert('xss')</script>";
		let html = widget.render_html("comment", Some(xss_content), None);

		// Should NOT contain raw script tag
		assert!(!html.contains("<script>"));
		// Should contain escaped version
		assert!(html.contains("&lt;script&gt;"));
		// Raw </textarea> should be escaped
		assert!(!html.contains("</textarea><"));
	}

	#[test]
	fn test_widget_render_html_escapes_select_choices() {
		let widget = Widget::Select {
			choices: vec![
				(
					"value\"><script>alert('xss')</script>".to_string(),
					"Label".to_string(),
				),
				(
					"safe_value".to_string(),
					"</option><script>alert('xss')</script>".to_string(),
				),
			],
		};

		let html = widget.render_html("choice", Some("safe_value"), None);

		// Should NOT contain raw script tags
		assert!(!html.contains("<script>"));
		// Should contain escaped versions
		assert!(html.contains("&lt;script&gt;"));
		// The malicious </option> in the label should be escaped
		assert!(html.contains("&lt;/option&gt;"));
	}

	#[test]
	fn test_widget_render_html_escapes_radio_choices() {
		let widget = Widget::RadioSelect {
			choices: vec![(
				"value\"><script>alert('xss')</script>".to_string(),
				"</label><script>alert('xss')</script>".to_string(),
			)],
		};

		let html = widget.render_html("radio", None, None);

		// Should NOT contain raw script tags
		assert!(!html.contains("<script>"));
		// Should contain escaped versions
		assert!(html.contains("&lt;script&gt;"));
	}

	#[test]
	fn test_widget_render_html_escapes_attributes() {
		let widget = Widget::TextInput;
		let mut attrs = HashMap::new();
		attrs.insert("class".to_string(), "\" onclick=\"alert('xss')".to_string());
		attrs.insert(
			"data-evil".to_string(),
			"\"><script>alert('xss')</script>".to_string(),
		);

		let html = widget.render_html("field", Some("value"), Some(&attrs));

		// Should NOT contain raw script tags or unescaped quotes that could break out
		assert!(!html.contains("<script>"));
		assert!(html.contains("&lt;script&gt;"));
		assert!(html.contains("&quot;"));
	}

	#[test]
	fn test_widget_render_html_all_widget_types_escape_value() {
		let xss_payload = "\"><script>alert('xss')</script>";

		// Test widget types that render a value attribute
		let widgets_with_value: Vec<Widget> = vec![
			Widget::TextInput,
			Widget::EmailInput,
			Widget::NumberInput,
			Widget::TextArea,
			Widget::DateInput,
			Widget::DateTimeInput,
			Widget::HiddenInput,
		];

		for widget in widgets_with_value {
			let html = widget.render_html("field", Some(xss_payload), None);
			assert!(
				!html.contains("<script>"),
				"Widget {:?} did not escape XSS payload",
				widget
			);
			assert!(
				html.contains("&lt;script&gt;"),
				"Widget {:?} did not encode < and > characters",
				widget
			);
		}

		// PasswordInput intentionally does not render the value attribute
		let password_html = Widget::PasswordInput.render_html("field", Some(xss_payload), None);
		assert!(
			!password_html.contains("value="),
			"PasswordInput should never render the value attribute"
		);
	}

	#[test]
	fn test_widget_render_html_normal_values_preserved() {
		let widget = Widget::TextInput;
		let html = widget.render_html("username", Some("john_doe"), None);

		// Normal values should work correctly
		assert!(html.contains("name=\"username\""));
		assert!(html.contains("value=\"john_doe\""));
	}

	#[test]
	fn test_widget_render_html_ampersand_escaped_first() {
		// Critical test: & must be escaped FIRST to prevent double-encoding
		// e.g., if we escape < first, & becomes &amp;, then if we escape & again,
		// it becomes &amp;amp;
		let input = "&lt;"; // This is already an entity
		let result = html_escape(input);
		// Should become &amp;lt; (the & is escaped, not the <)
		assert_eq!(result, "&amp;lt;");
	}
}
