use crate::field::{FormField, Widget};

/// BoundField represents a field bound to form data
pub struct BoundField<'a> {
	// Allow dead_code: field stored for form-level operations and rendering context
	#[allow(dead_code)]
	form_name: String,
	field: &'a dyn FormField,
	data: Option<&'a serde_json::Value>,
	errors: &'a [String],
	prefix: &'a str,
}

impl<'a> BoundField<'a> {
	/// Documentation for `new`
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::{BoundField, CharField, FormField};
	///
	/// let field: Box<dyn FormField> = Box::new(CharField::new("name".to_string()));
	/// let data = serde_json::json!("John");
	/// let errors = vec![];
	///
	/// let bound = BoundField::new("my_form".to_string(), field.as_ref(), Some(&data), &errors, "");
	/// assert_eq!(bound.name(), "name");
	/// assert_eq!(bound.value(), Some(&data));
	/// ```
	pub fn new(
		form_name: String,
		field: &'a dyn FormField,
		data: Option<&'a serde_json::Value>,
		errors: &'a [String],
		prefix: &'a str,
	) -> Self {
		Self {
			form_name,
			field,
			data,
			errors,
			prefix,
		}
	}
	/// Get the field name
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::{BoundField, CharField, FormField};
	///
	/// let field: Box<dyn FormField> = Box::new(CharField::new("email".to_string()));
	/// let bound = BoundField::new("form".to_string(), field.as_ref(), None, &[], "");
	/// assert_eq!(bound.name(), "email");
	/// ```
	pub fn name(&self) -> &str {
		self.field.name()
	}
	/// Get the HTML name attribute (with prefix)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::{BoundField, CharField, FormField};
	///
	/// let field: Box<dyn FormField> = Box::new(CharField::new("email".to_string()));
	///
	/// // Without prefix
	/// let bound = BoundField::new("form".to_string(), field.as_ref(), None, &[], "");
	/// assert_eq!(bound.html_name(), "email");
	///
	/// // With prefix
	/// let bound_prefixed = BoundField::new("form".to_string(), field.as_ref(), None, &[], "user");
	/// assert_eq!(bound_prefixed.html_name(), "user-email");
	/// ```
	pub fn html_name(&self) -> String {
		if self.prefix.is_empty() {
			self.field.name().to_string()
		} else {
			format!("{}-{}", self.prefix, self.field.name())
		}
	}
	/// Get the HTML id attribute
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::{BoundField, CharField, FormField};
	///
	/// let field: Box<dyn FormField> = Box::new(CharField::new("username".to_string()));
	/// let bound = BoundField::new("form".to_string(), field.as_ref(), None, &[], "profile");
	///
	/// assert_eq!(bound.id_for_label(), "id_profile-username");
	/// ```
	pub fn id_for_label(&self) -> String {
		format!("id_{}", self.html_name())
	}
	/// Get the field label
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::{BoundField, CharField, FormField};
	///
	/// let mut field = CharField::new("name".to_string());
	/// field.label = Some("Full Name".to_string());
	/// let field_box: Box<dyn FormField> = Box::new(field);
	///
	/// let bound = BoundField::new("form".to_string(), field_box.as_ref(), None, &[], "");
	/// assert_eq!(bound.label(), Some("Full Name"));
	/// ```
	pub fn label(&self) -> Option<&str> {
		self.field.label()
	}
	/// Get the field value
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::{BoundField, CharField, FormField};
	///
	/// let field: Box<dyn FormField> = Box::new(CharField::new("name".to_string()));
	/// let data = serde_json::json!("Alice");
	///
	/// let bound = BoundField::new("form".to_string(), field.as_ref(), Some(&data), &[], "");
	/// assert_eq!(bound.value(), Some(&data));
	/// ```
	pub fn value(&self) -> Option<&serde_json::Value> {
		self.data.or_else(|| self.field.initial())
	}
	/// Get field errors
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::{BoundField, CharField, FormField};
	///
	/// let field: Box<dyn FormField> = Box::new(CharField::new("email".to_string()));
	/// let errors = vec!["Invalid email format".to_string(), "Email is required".to_string()];
	///
	/// let bound = BoundField::new("form".to_string(), field.as_ref(), None, &errors, "");
	/// assert_eq!(bound.errors().len(), 2);
	/// assert_eq!(bound.errors()[0], "Invalid email format");
	/// ```
	pub fn errors(&self) -> &[String] {
		self.errors
	}
	/// Check if field has errors
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::{BoundField, CharField, FormField};
	///
	/// let field: Box<dyn FormField> = Box::new(CharField::new("username".to_string()));
	///
	/// // Without errors
	/// let bound_ok = BoundField::new("form".to_string(), field.as_ref(), None, &[], "");
	/// assert!(!bound_ok.has_errors());
	///
	/// // With errors
	/// let errors = vec!["Username is required".to_string()];
	/// let bound_err = BoundField::new("form".to_string(), field.as_ref(), None, &errors, "");
	/// assert!(bound_err.has_errors());
	/// ```
	pub fn has_errors(&self) -> bool {
		!self.errors.is_empty()
	}
	/// Get the widget
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::{BoundField, CharField, EmailField, FormField, Widget};
	///
	/// let field: Box<dyn FormField> = Box::new(CharField::new("name".to_string()));
	/// let bound = BoundField::new("form".to_string(), field.as_ref(), None, &[], "");
	/// assert!(matches!(bound.widget(), Widget::TextInput));
	///
	/// let email_field: Box<dyn FormField> = Box::new(EmailField::new("email".to_string()));
	/// let email_bound = BoundField::new("form".to_string(), email_field.as_ref(), None, &[], "");
	/// assert!(matches!(email_bound.widget(), Widget::EmailInput));
	/// ```
	pub fn widget(&self) -> &Widget {
		self.field.widget()
	}
	/// Get help text
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::{BoundField, CharField, FormField};
	///
	/// let mut field = CharField::new("password".to_string());
	/// field.help_text = Some("Must be at least 8 characters".to_string());
	/// let field_box: Box<dyn FormField> = Box::new(field);
	///
	/// let bound = BoundField::new("form".to_string(), field_box.as_ref(), None, &[], "");
	/// assert_eq!(bound.help_text(), Some("Must be at least 8 characters"));
	/// ```
	pub fn help_text(&self) -> Option<&str> {
		self.field.help_text()
	}
	/// Check if field is required
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::{BoundField, CharField, FormField};
	///
	/// let mut field = CharField::new("name".to_string());
	/// field.required = true;
	/// let field_box: Box<dyn FormField> = Box::new(field);
	///
	/// let bound = BoundField::new("form".to_string(), field_box.as_ref(), None, &[], "");
	/// assert!(bound.is_required());
	///
	/// let mut optional_field = CharField::new("nickname".to_string());
	/// optional_field.required = false;
	/// let optional_box: Box<dyn FormField> = Box::new(optional_field);
	///
	/// let optional_bound = BoundField::new("form".to_string(), optional_box.as_ref(), None, &[], "");
	/// assert!(!optional_bound.is_required());
	/// ```
	pub fn is_required(&self) -> bool {
		self.field.required()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::fields::CharField;

	#[test]
	fn test_bound_field_basic() {
		let field: Box<dyn FormField> = Box::new(CharField::new("name".to_string()));
		let data = serde_json::json!("John Doe");
		let errors = vec![];

		let bound = BoundField::new(
			"test_form".to_string(),
			field.as_ref(),
			Some(&data),
			&errors,
			"",
		);

		assert_eq!(bound.name(), "name");
		assert_eq!(bound.html_name(), "name");
		assert_eq!(bound.id_for_label(), "id_name");
		assert_eq!(bound.value(), Some(&data));
		assert!(!bound.has_errors());
	}

	#[test]
	fn test_bound_field_with_prefix() {
		let field: Box<dyn FormField> = Box::new(CharField::new("name".to_string()));
		let data = serde_json::json!("John Doe");
		let errors = vec![];

		let bound = BoundField::new(
			"test_form".to_string(),
			field.as_ref(),
			Some(&data),
			&errors,
			"profile",
		);

		assert_eq!(bound.html_name(), "profile-name");
		assert_eq!(bound.id_for_label(), "id_profile-name");
	}

	#[test]
	fn test_bound_field_with_errors() {
		let field: Box<dyn FormField> = Box::new(CharField::new("name".to_string()));
		let data = serde_json::json!("");
		let errors = vec!["This field is required.".to_string()];

		let bound = BoundField::new(
			"test_form".to_string(),
			field.as_ref(),
			Some(&data),
			&errors,
			"",
		);

		assert!(bound.has_errors());
		assert_eq!(bound.errors().len(), 1);
	}
}
