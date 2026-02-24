use crate::bound_field::BoundField;
use crate::field::{FieldError, FormField};
use crate::wasm_compat::ValidationRule;
use std::collections::HashMap;
use std::ops::Index;

/// Constant-time byte comparison to prevent timing attacks on CSRF tokens.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
	if a.len() != b.len() {
		return false;
	}
	let mut result = 0u8;
	for (x, y) in a.iter().zip(b.iter()) {
		result |= x ^ y;
	}
	result == 0
}

#[derive(Debug, thiserror::Error)]
pub enum FormError {
	#[error("Field error in {field}: {error}")]
	Field { field: String, error: FieldError },
	#[error("Validation error: {0}")]
	Validation(String),
	#[error("No model instance available for save operation")]
	NoInstance,
}

pub type FormResult<T> = Result<T, FormError>;

type CleanFunction =
	Box<dyn Fn(&HashMap<String, serde_json::Value>) -> FormResult<()> + Send + Sync>;
type FieldCleanFunction =
	Box<dyn Fn(&serde_json::Value) -> FormResult<serde_json::Value> + Send + Sync>;

/// Special key for form-level (non-field-specific) errors.
///
/// In Django, this is `"__all__"`, but in Rust we use a single underscore
/// to follow Rust conventions for internal/private identifiers.
pub const ALL_FIELDS_KEY: &str = "_all";

/// Form data structure (Phase 2-A: Enhanced with client-side validation rules)
pub struct Form {
	fields: Vec<Box<dyn FormField>>,
	data: HashMap<String, serde_json::Value>,
	initial: HashMap<String, serde_json::Value>,
	errors: HashMap<String, Vec<String>>,
	is_bound: bool,
	clean_functions: Vec<CleanFunction>,
	field_clean_functions: HashMap<String, FieldCleanFunction>,
	prefix: String,
	/// Client-side validation rules (Phase 2-A)
	/// These rules are transmitted to the client for UX enhancement.
	/// Server-side validation is still mandatory for security.
	validation_rules: Vec<ValidationRule>,
	/// Expected CSRF token for form validation
	csrf_token: Option<String>,
	/// Whether CSRF validation is enabled
	csrf_enabled: bool,
}

impl Form {
	/// Create a new empty form
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::Form;
	///
	/// let form = Form::new();
	/// assert!(!form.is_bound());
	/// assert!(form.fields().is_empty());
	/// ```
	pub fn new() -> Self {
		Self {
			fields: vec![],
			data: HashMap::new(),
			initial: HashMap::new(),
			errors: HashMap::new(),
			is_bound: false,
			clean_functions: vec![],
			field_clean_functions: HashMap::new(),
			prefix: String::new(),
			validation_rules: vec![],
			csrf_token: None,
			csrf_enabled: false,
		}
	}
	/// Create a new form with initial data
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::Form;
	/// use std::collections::HashMap;
	/// use serde_json::json;
	///
	/// let mut initial = HashMap::new();
	/// initial.insert("name".to_string(), json!("John"));
	///
	/// let form = Form::with_initial(initial);
	/// assert_eq!(form.initial().get("name"), Some(&json!("John")));
	/// ```
	pub fn with_initial(initial: HashMap<String, serde_json::Value>) -> Self {
		Self {
			fields: vec![],
			data: HashMap::new(),
			initial,
			errors: HashMap::new(),
			is_bound: false,
			clean_functions: vec![],
			field_clean_functions: HashMap::new(),
			prefix: String::new(),
			validation_rules: vec![],
			csrf_token: None,
			csrf_enabled: false,
		}
	}
	/// Create a new form with a field prefix
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::Form;
	///
	/// let form = Form::with_prefix("user".to_string());
	/// assert_eq!(form.prefix(), "user");
	/// assert_eq!(form.add_prefix_to_field_name("email"), "user-email");
	/// ```
	pub fn with_prefix(prefix: String) -> Self {
		Self {
			fields: vec![],
			data: HashMap::new(),
			initial: HashMap::new(),
			errors: HashMap::new(),
			is_bound: false,
			clean_functions: vec![],
			field_clean_functions: HashMap::new(),
			prefix,
			validation_rules: vec![],
			csrf_token: None,
			csrf_enabled: false,
		}
	}
	/// Add a field to the form
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::{Form, CharField, Field};
	///
	/// let mut form = Form::new();
	/// let field = CharField::new("username".to_string());
	/// form.add_field(Box::new(field));
	/// assert_eq!(form.fields().len(), 1);
	/// ```
	pub fn add_field(&mut self, field: Box<dyn FormField>) {
		self.fields.push(field);
	}
	/// Bind form data for validation
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::Form;
	/// use std::collections::HashMap;
	/// use serde_json::json;
	///
	/// let mut form = Form::new();
	/// let mut data = HashMap::new();
	/// data.insert("username".to_string(), json!("john"));
	///
	/// form.bind(data);
	/// assert!(form.is_bound());
	/// ```
	pub fn bind(&mut self, data: HashMap<String, serde_json::Value>) {
		self.data = data;
		self.is_bound = true;
	}
	/// Validate the form and return true if all fields are valid
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::{Form, CharField, Field};
	/// use std::collections::HashMap;
	/// use serde_json::json;
	///
	/// let mut form = Form::new();
	/// form.add_field(Box::new(CharField::new("username".to_string())));
	///
	/// let mut data = HashMap::new();
	/// data.insert("username".to_string(), json!("john"));
	/// form.bind(data);
	///
	/// assert!(form.is_valid());
	/// assert!(form.errors().is_empty());
	/// assert_eq!(form.cleaned_data().get("username"), Some(&json!("john")));
	/// ```
	pub fn is_valid(&mut self) -> bool {
		if !self.is_bound {
			return false;
		}

		self.errors.clear();

		// Validate CSRF token if enabled
		if !self.validate_csrf() {
			self.errors
				.entry(ALL_FIELDS_KEY.to_string())
				.or_default()
				.push("CSRF token missing or incorrect.".to_string());
			return false;
		}

		for field in &self.fields {
			let value = self.data.get(field.name());

			match field.clean(value) {
				Ok(mut cleaned) => {
					// Run field-specific clean function if exists
					if let Some(field_clean) = self.field_clean_functions.get(field.name()) {
						match field_clean(&cleaned) {
							Ok(further_cleaned) => {
								cleaned = further_cleaned;
							}
							Err(e) => {
								self.errors
									.entry(field.name().to_string())
									.or_default()
									.push(e.to_string());
								continue;
							}
						}
					}
					self.data.insert(field.name().to_string(), cleaned);
				}
				Err(e) => {
					self.errors
						.entry(field.name().to_string())
						.or_default()
						.push(e.to_string());
				}
			}
		}

		// Run custom clean functions
		for clean_fn in &self.clean_functions {
			if let Err(e) = clean_fn(&self.data) {
				match e {
					FormError::Field { field, error } => {
						self.errors
							.entry(field)
							.or_default()
							.push(error.to_string());
					}
					FormError::Validation(msg) => {
						self.errors
							.entry(ALL_FIELDS_KEY.to_string())
							.or_default()
							.push(msg);
					}
					FormError::NoInstance => {
						self.errors
							.entry(ALL_FIELDS_KEY.to_string())
							.or_default()
							.push(e.to_string());
					}
				}
			}
		}

		self.errors.is_empty()
	}
	pub fn cleaned_data(&self) -> &HashMap<String, serde_json::Value> {
		&self.data
	}
	pub fn errors(&self) -> &HashMap<String, Vec<String>> {
		&self.errors
	}
	pub fn is_bound(&self) -> bool {
		self.is_bound
	}
	pub fn fields(&self) -> &[Box<dyn FormField>] {
		&self.fields
	}
	pub fn initial(&self) -> &HashMap<String, serde_json::Value> {
		&self.initial
	}
	/// Set initial data for the form
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::Form;
	/// use std::collections::HashMap;
	/// use serde_json::json;
	///
	/// let mut form = Form::new();
	/// let mut initial = HashMap::new();
	/// initial.insert("name".to_string(), json!("John"));
	/// form.set_initial(initial);
	/// ```
	pub fn set_initial(&mut self, initial: HashMap<String, serde_json::Value>) {
		self.initial = initial;
	}
	/// Check if any field has changed from its initial value
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::{Form, CharField, Field};
	/// use std::collections::HashMap;
	/// use serde_json::json;
	///
	/// let mut initial = HashMap::new();
	/// initial.insert("name".to_string(), json!("John"));
	///
	/// let mut form = Form::with_initial(initial);
	/// form.add_field(Box::new(CharField::new("name".to_string())));
	///
	/// let mut data = HashMap::new();
	/// data.insert("name".to_string(), json!("Jane"));
	/// form.bind(data);
	///
	/// assert!(form.has_changed());
	/// ```
	pub fn has_changed(&self) -> bool {
		if !self.is_bound {
			return false;
		}

		for field in &self.fields {
			let initial_val = self.initial.get(field.name());
			let data_val = self.data.get(field.name());
			if field.has_changed(initial_val, data_val) {
				return true;
			}
		}
		false
	}
	pub fn get_field(&self, name: &str) -> Option<&dyn FormField> {
		self.fields
			.iter()
			.find(|f| f.name() == name)
			.map(|f| f.as_ref())
	}
	pub fn remove_field(&mut self, name: &str) -> Option<Box<dyn FormField>> {
		let pos = self.fields.iter().position(|f| f.name() == name)?;
		Some(self.fields.remove(pos))
	}
	pub fn field_count(&self) -> usize {
		self.fields.len()
	}
	/// Add a custom clean function for form validation
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::Form;
	/// use std::collections::HashMap;
	/// use serde_json::json;
	///
	/// let mut form = Form::new();
	/// form.add_clean_function(|data| {
	///     if data.get("password") != data.get("confirm_password") {
	///         Err(reinhardt_forms::FormError::Validation("Passwords do not match".to_string()))
	///     } else {
	///         Ok(())
	///     }
	/// });
	/// ```
	pub fn add_clean_function<F>(&mut self, f: F)
	where
		F: Fn(&HashMap<String, serde_json::Value>) -> FormResult<()> + Send + Sync + 'static,
	{
		self.clean_functions.push(Box::new(f));
	}
	/// Add a custom clean function for a specific field
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::Form;
	/// use serde_json::json;
	///
	/// let mut form = Form::new();
	/// form.add_field_clean_function("email", |value| {
	///     if let Some(email) = value.as_str() {
	///         if email.contains("@") {
	///             Ok(value.clone())
	///         } else {
	///             Err(reinhardt_forms::FormError::Validation("Invalid email".to_string()))
	///         }
	///     } else {
	///         Ok(value.clone())
	///     }
	/// });
	/// ```
	pub fn add_field_clean_function<F>(&mut self, field_name: &str, f: F)
	where
		F: Fn(&serde_json::Value) -> FormResult<serde_json::Value> + Send + Sync + 'static,
	{
		self.field_clean_functions
			.insert(field_name.to_string(), Box::new(f));
	}

	/// Get client-side validation rules (Phase 2-A)
	///
	/// # Returns
	///
	/// Reference to the validation rules vector
	pub fn validation_rules(&self) -> &[ValidationRule] {
		&self.validation_rules
	}

	/// Add a minimum length validator (Phase 2-A)
	///
	/// Adds a validator that checks if a string field has at least `min` characters.
	/// This validator is executed on the client-side for immediate feedback.
	///
	/// **Security Note**: Client-side validation is for UX enhancement only.
	/// Server-side validation is still mandatory for security.
	///
	/// # Arguments
	///
	/// - `field_name`: Name of the field to validate
	/// - `min`: Minimum required length
	/// - `error_message`: Error message to display on validation failure
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::Form;
	///
	/// let mut form = Form::new();
	/// form.add_min_length_validator("password", 8, "Password must be at least 8 characters");
	/// ```
	pub fn add_min_length_validator(
		&mut self,
		field_name: impl Into<String>,
		min: usize,
		error_message: impl Into<String>,
	) {
		self.validation_rules.push(ValidationRule::MinLength {
			field_name: field_name.into(),
			min,
			error_message: error_message.into(),
		});
	}

	/// Add a maximum length validator (Phase 2-A)
	///
	/// Adds a validator that checks if a string field has at most `max` characters.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::Form;
	///
	/// let mut form = Form::new();
	/// form.add_max_length_validator("username", 50, "Username must be at most 50 characters");
	/// ```
	pub fn add_max_length_validator(
		&mut self,
		field_name: impl Into<String>,
		max: usize,
		error_message: impl Into<String>,
	) {
		self.validation_rules.push(ValidationRule::MaxLength {
			field_name: field_name.into(),
			max,
			error_message: error_message.into(),
		});
	}

	/// Add a pattern validator (Phase 2-A)
	///
	/// Adds a validator that checks if a string field matches a regex pattern.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::Form;
	///
	/// let mut form = Form::new();
	/// form.add_pattern_validator("code", "^[A-Z]{3}$", "Code must be 3 uppercase letters");
	/// ```
	pub fn add_pattern_validator(
		&mut self,
		field_name: impl Into<String>,
		pattern: impl Into<String>,
		error_message: impl Into<String>,
	) {
		self.validation_rules.push(ValidationRule::Pattern {
			field_name: field_name.into(),
			pattern: pattern.into(),
			error_message: error_message.into(),
		});
	}

	/// Add a minimum value validator (Phase 2-A)
	///
	/// Adds a validator that checks if a numeric field is at least `min`.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::Form;
	///
	/// let mut form = Form::new();
	/// form.add_min_value_validator("age", 0.0, "Age must be non-negative");
	/// ```
	pub fn add_min_value_validator(
		&mut self,
		field_name: impl Into<String>,
		min: f64,
		error_message: impl Into<String>,
	) {
		self.validation_rules.push(ValidationRule::MinValue {
			field_name: field_name.into(),
			min,
			error_message: error_message.into(),
		});
	}

	/// Add a maximum value validator (Phase 2-A)
	///
	/// Adds a validator that checks if a numeric field is at most `max`.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::Form;
	///
	/// let mut form = Form::new();
	/// form.add_max_value_validator("age", 150.0, "Age must be at most 150");
	/// ```
	pub fn add_max_value_validator(
		&mut self,
		field_name: impl Into<String>,
		max: f64,
		error_message: impl Into<String>,
	) {
		self.validation_rules.push(ValidationRule::MaxValue {
			field_name: field_name.into(),
			max,
			error_message: error_message.into(),
		});
	}

	/// Add an email format validator (Phase 2-A)
	///
	/// Adds a validator that checks if a field contains a valid email format.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::Form;
	///
	/// let mut form = Form::new();
	/// form.add_email_validator("email", "Enter a valid email address");
	/// ```
	pub fn add_email_validator(
		&mut self,
		field_name: impl Into<String>,
		error_message: impl Into<String>,
	) {
		self.validation_rules.push(ValidationRule::Email {
			field_name: field_name.into(),
			error_message: error_message.into(),
		});
	}

	/// Add a URL format validator (Phase 2-A)
	///
	/// Adds a validator that checks if a field contains a valid URL format.
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::Form;
	///
	/// let mut form = Form::new();
	/// form.add_url_validator("website", "Enter a valid URL");
	/// ```
	pub fn add_url_validator(
		&mut self,
		field_name: impl Into<String>,
		error_message: impl Into<String>,
	) {
		self.validation_rules.push(ValidationRule::Url {
			field_name: field_name.into(),
			error_message: error_message.into(),
		});
	}

	/// Add a fields equality validator (Phase 2-A)
	///
	/// Adds a validator that checks if multiple fields have equal values.
	/// Commonly used for password confirmation.
	///
	/// # Arguments
	///
	/// - `field_names`: Names of fields to compare for equality
	/// - `error_message`: Error message to display on validation failure
	/// - `target_field`: Target field for error display (None = non-field error)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::Form;
	///
	/// let mut form = Form::new();
	/// form.add_fields_equal_validator(
	///     vec!["password".to_string(), "password_confirm".to_string()],
	///     "Passwords do not match",
	///     Some("password_confirm".to_string())
	/// );
	/// ```
	pub fn add_fields_equal_validator(
		&mut self,
		field_names: Vec<String>,
		error_message: impl Into<String>,
		target_field: Option<String>,
	) {
		self.validation_rules.push(ValidationRule::FieldsEqual {
			field_names,
			error_message: error_message.into(),
			target_field,
		});
	}

	/// Add a client-side validator reference (Phase 2-A)
	///
	/// Adds a reference to a reinhardt-validators Validator.
	/// This validator is executed on the client-side for immediate feedback.
	///
	/// **Security Note**: Client-side validation is for UX enhancement only.
	/// Server-side validation is still mandatory for security.
	///
	/// # Arguments
	///
	/// - `field_name`: Name of the field to validate
	/// - `validator_id`: Validator identifier (e.g., "email", "url", "min_length")
	/// - `params`: Validator parameters as JSON
	/// - `error_message`: Error message to display on validation failure
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::Form;
	/// use serde_json::json;
	///
	/// let mut form = Form::new();
	/// form.add_validator_rule(
	///     "email",
	///     "email",
	///     json!({}),
	///     "Enter a valid email address"
	/// );
	///
	/// form.add_validator_rule(
	///     "username",
	///     "min_length",
	///     json!({"min": 3}),
	///     "Username must be at least 3 characters"
	/// );
	/// ```
	pub fn add_validator_rule(
		&mut self,
		field_name: impl Into<String>,
		validator_id: impl Into<String>,
		params: serde_json::Value,
		error_message: impl Into<String>,
	) {
		self.validation_rules.push(ValidationRule::ValidatorRef {
			field_name: field_name.into(),
			validator_id: validator_id.into(),
			params,
			error_message: error_message.into(),
		});
	}

	/// Helper: Add a date range validator (Phase 2-A)
	///
	/// Adds a validator that checks if end_date >= start_date.
	///
	/// # Arguments
	///
	/// - `start_field`: Name of the start date field
	/// - `end_field`: Name of the end date field
	/// - `error_message`: Error message (optional, defaults to a standard message)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::Form;
	///
	/// let mut form = Form::new();
	/// form.add_date_range_validator("start_date", "end_date", None);
	/// ```
	pub fn add_date_range_validator(
		&mut self,
		start_field: impl Into<String>,
		end_field: impl Into<String>,
		error_message: Option<String>,
	) {
		let start = start_field.into();
		let end = end_field.into();
		let message = error_message
			.unwrap_or_else(|| "End date must be after or equal to start date".to_string());

		self.validation_rules.push(ValidationRule::DateRange {
			start_field: start,
			end_field: end.clone(),
			error_message: message,
			target_field: Some(end),
		});
	}

	/// Helper: Add a numeric range validator (Phase 2-A)
	///
	/// Adds a validator that checks if max >= min.
	///
	/// # Arguments
	///
	/// - `min_field`: Name of the minimum value field
	/// - `max_field`: Name of the maximum value field
	/// - `error_message`: Error message (optional, defaults to a standard message)
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::Form;
	///
	/// let mut form = Form::new();
	/// form.add_numeric_range_validator("min_price", "max_price", None);
	/// ```
	pub fn add_numeric_range_validator(
		&mut self,
		min_field: impl Into<String>,
		max_field: impl Into<String>,
		error_message: Option<String>,
	) {
		let min = min_field.into();
		let max = max_field.into();
		let message = error_message.unwrap_or_else(|| {
			"Maximum value must be greater than or equal to minimum value".to_string()
		});

		self.validation_rules.push(ValidationRule::NumericRange {
			min_field: min,
			max_field: max.clone(),
			error_message: message,
			target_field: Some(max),
		});
	}
	/// Enable CSRF protection for this form.
	///
	/// When enabled, `is_valid()` will check that the submitted data
	/// contains a matching CSRF token.
	///
	/// # Arguments
	///
	/// * `token` - The expected CSRF token for this form
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::Form;
	///
	/// let mut form = Form::new();
	/// form.set_csrf_token("abc123".to_string());
	/// assert!(form.csrf_enabled());
	/// ```
	pub fn set_csrf_token(&mut self, token: String) {
		self.csrf_token = Some(token);
		self.csrf_enabled = true;
	}

	/// Check if CSRF protection is enabled
	pub fn csrf_enabled(&self) -> bool {
		self.csrf_enabled
	}

	/// Get the CSRF token, if set
	pub fn csrf_token(&self) -> Option<&str> {
		self.csrf_token.as_deref()
	}

	/// Validate the submitted CSRF token against the expected token.
	///
	/// Returns `true` if CSRF is disabled or the token matches.
	fn validate_csrf(&self) -> bool {
		if !self.csrf_enabled {
			return true;
		}

		let expected = match &self.csrf_token {
			Some(t) => t,
			None => return false,
		};

		let submitted = self
			.data
			.get("csrfmiddlewaretoken")
			.and_then(|v| v.as_str());

		match submitted {
			Some(token) => {
				// Use constant-time comparison to prevent timing attacks
				constant_time_eq(token.as_bytes(), expected.as_bytes())
			}
			None => false,
		}
	}

	pub fn prefix(&self) -> &str {
		&self.prefix
	}
	pub fn set_prefix(&mut self, prefix: String) {
		self.prefix = prefix;
	}
	pub fn add_prefix_to_field_name(&self, field_name: &str) -> String {
		if self.prefix.is_empty() {
			field_name.to_string()
		} else {
			format!("{}-{}", self.prefix, field_name)
		}
	}
	/// Render CSS `<link>` tags for form media with HTML-escaped paths.
	///
	/// All paths are escaped using `escape_attribute()` to prevent XSS
	/// via malicious CSS file paths.
	///
	/// # Arguments
	///
	/// * `css_files` - Slice of CSS file paths to include
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::Form;
	///
	/// let form = Form::new();
	/// let html = form.render_css_media(&["/static/forms.css"]);
	/// assert!(html.contains("href=\"/static/forms.css\""));
	/// ```
	pub fn render_css_media(&self, css_files: &[&str]) -> String {
		use crate::field::escape_attribute;
		let mut html = String::new();
		for path in css_files {
			html.push_str(&format!(
				"<link rel=\"stylesheet\" href=\"{}\" />\n",
				escape_attribute(path)
			));
		}
		html
	}

	/// Render JS `<script>` tags for form media with HTML-escaped paths.
	///
	/// All paths are escaped using `escape_attribute()` to prevent XSS
	/// via malicious JS file paths.
	///
	/// # Arguments
	///
	/// * `js_files` - Slice of JS file paths to include
	///
	/// # Examples
	///
	/// ```
	/// use reinhardt_forms::Form;
	///
	/// let form = Form::new();
	/// let html = form.render_js_media(&["/static/forms.js"]);
	/// assert!(html.contains("src=\"/static/forms.js\""));
	/// ```
	pub fn render_js_media(&self, js_files: &[&str]) -> String {
		use crate::field::escape_attribute;
		let mut html = String::new();
		for path in js_files {
			html.push_str(&format!(
				"<script src=\"{}\"></script>\n",
				escape_attribute(path)
			));
		}
		html
	}

	pub fn get_bound_field<'a>(&'a self, name: &str) -> Option<BoundField<'a>> {
		let field = self.get_field(name)?;
		let data = self.data.get(name);
		let errors = self.errors.get(name).map(|e| e.as_slice()).unwrap_or(&[]);

		Some(BoundField::new(
			"form".to_string(),
			field,
			data,
			errors,
			&self.prefix,
		))
	}
}

impl Default for Form {
	fn default() -> Self {
		Self::new()
	}
}

/// Safe field access by name.
///
/// Returns `None` if the field is not found instead of panicking.
///
/// # Examples
///
/// ```
/// use reinhardt_forms::{Form, CharField, Field};
///
/// let mut form = Form::new();
/// form.add_field(Box::new(CharField::new("name".to_string())));
///
/// assert!(form.get("name").is_some());
/// assert!(form.get("nonexistent").is_none());
/// ```
impl Form {
	// Allow borrowed_box because Index trait impl requires &Box<dyn FormField>
	#[allow(clippy::borrowed_box)]
	pub fn get(&self, name: &str) -> Option<&Box<dyn FormField>> {
		self.fields.iter().find(|f| f.name() == name)
	}
}

impl Index<&str> for Form {
	type Output = Box<dyn FormField>;

	fn index(&self, name: &str) -> &Self::Output {
		self.get(name)
			.unwrap_or_else(|| panic!("Field '{}' not found", name))
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::fields::CharField;

	#[test]
	fn test_form_validation() {
		let mut form = Form::new();

		let mut name_field = CharField::new("name".to_string());
		name_field.max_length = Some(50);
		form.add_field(Box::new(name_field));

		let mut data = HashMap::new();
		data.insert("name".to_string(), serde_json::json!("John Doe"));

		form.bind(data);
		assert!(form.is_valid());
		assert!(form.errors().is_empty());
	}

	#[test]
	fn test_form_validation_error() {
		let mut form = Form::new();

		let mut name_field = CharField::new("name".to_string());
		name_field.max_length = Some(5);
		form.add_field(Box::new(name_field));

		let mut data = HashMap::new();
		data.insert("name".to_string(), serde_json::json!("Very Long Name"));

		form.bind(data);
		assert!(!form.is_valid());
		assert!(!form.errors().is_empty());
	}

	// Additional tests based on Django forms tests

	#[test]
	fn test_form_basic() {
		// Test based on Django FormsTestCase.test_form
		use crate::fields::CharField;

		let mut form = Form::new();
		form.add_field(Box::new(CharField::new("first_name".to_string())));
		form.add_field(Box::new(CharField::new("last_name".to_string())));

		let mut data = HashMap::new();
		data.insert("first_name".to_string(), serde_json::json!("John"));
		data.insert("last_name".to_string(), serde_json::json!("Lennon"));

		form.bind(data);

		assert!(form.is_bound());
		assert!(form.is_valid());
		assert!(form.errors().is_empty());

		// Check cleaned data
		let cleaned = form.cleaned_data();
		assert_eq!(
			cleaned.get("first_name").unwrap(),
			&serde_json::json!("John")
		);
		assert_eq!(
			cleaned.get("last_name").unwrap(),
			&serde_json::json!("Lennon")
		);
	}

	#[test]
	fn test_form_missing_required_fields() {
		// Form with missing required fields should have errors
		use crate::fields::CharField;

		let mut form = Form::new();
		form.add_field(Box::new(CharField::new("username".to_string()).required()));
		form.add_field(Box::new(CharField::new("email".to_string()).required()));

		let data = HashMap::new(); // Empty data

		form.bind(data);

		assert!(form.is_bound());
		assert!(!form.is_valid());
		assert!(form.errors().contains_key("username"));
		assert!(form.errors().contains_key("email"));
	}

	#[test]
	fn test_form_optional_fields() {
		// Form with optional fields should validate even if they're missing
		use crate::fields::CharField;

		let mut form = Form::new();

		let username_field = CharField::new("username".to_string());
		form.add_field(Box::new(username_field));

		let mut bio_field = CharField::new("bio".to_string());
		bio_field.required = false;
		form.add_field(Box::new(bio_field));

		let mut data = HashMap::new();
		data.insert("username".to_string(), serde_json::json!("john"));
		// bio is omitted

		form.bind(data);

		assert!(form.is_bound());
		assert!(form.is_valid());
		assert!(form.errors().is_empty());
	}

	#[test]
	fn test_form_unbound() {
		// Unbound form (no data provided)
		use crate::fields::CharField;

		let mut form = Form::new();
		form.add_field(Box::new(CharField::new("name".to_string())));

		assert!(!form.is_bound());
		assert!(!form.is_valid()); // Unbound forms are not valid
	}

	#[test]
	fn test_form_extra_data() {
		// Form should ignore extra data not defined in fields
		use crate::fields::CharField;

		let mut form = Form::new();
		form.add_field(Box::new(CharField::new("name".to_string())));

		let mut data = HashMap::new();
		data.insert("name".to_string(), serde_json::json!("John"));
		data.insert(
			"extra_field".to_string(),
			serde_json::json!("should be ignored"),
		);

		form.bind(data);

		assert!(form.is_valid());
		let cleaned = form.cleaned_data();
		assert_eq!(cleaned.get("name").unwrap(), &serde_json::json!("John"));
		// extra_field is still in data but not validated
		assert!(cleaned.contains_key("extra_field"));
	}

	#[test]
	fn test_forms_form_multiple_fields() {
		// Test form with multiple field types
		use crate::fields::{CharField, IntegerField};

		let mut form = Form::new();
		form.add_field(Box::new(CharField::new("username".to_string())));

		let mut age_field = IntegerField::new("age".to_string());
		age_field.min_value = Some(0);
		age_field.max_value = Some(150);
		form.add_field(Box::new(age_field));

		let mut data = HashMap::new();
		data.insert("username".to_string(), serde_json::json!("alice"));
		data.insert("age".to_string(), serde_json::json!(30));

		form.bind(data);

		assert!(form.is_valid());
		assert!(form.errors().is_empty());
	}

	#[test]
	fn test_form_multiple_fields_invalid() {
		// Test form with multiple field types, some invalid
		use crate::fields::{CharField, IntegerField};

		let mut form = Form::new();

		let mut username_field = CharField::new("username".to_string());
		username_field.min_length = Some(3);
		form.add_field(Box::new(username_field));

		let mut age_field = IntegerField::new("age".to_string());
		age_field.min_value = Some(0);
		age_field.max_value = Some(150);
		form.add_field(Box::new(age_field));

		let mut data = HashMap::new();
		data.insert("username".to_string(), serde_json::json!("ab")); // Too short
		data.insert("age".to_string(), serde_json::json!(200)); // Too large

		form.bind(data);

		assert!(!form.is_valid());
		assert!(form.errors().contains_key("username"));
		assert!(form.errors().contains_key("age"));
	}

	#[test]
	fn test_form_multiple_instances() {
		// Multiple form instances should be independent
		use crate::fields::CharField;

		let mut form1 = Form::new();
		form1.add_field(Box::new(CharField::new("name".to_string())));

		let mut form2 = Form::new();
		form2.add_field(Box::new(CharField::new("name".to_string())));

		let mut data1 = HashMap::new();
		data1.insert("name".to_string(), serde_json::json!("Form1"));
		form1.bind(data1);

		let mut data2 = HashMap::new();
		data2.insert("name".to_string(), serde_json::json!("Form2"));
		form2.bind(data2);

		assert!(form1.is_valid());
		assert!(form2.is_valid());

		assert_eq!(
			form1.cleaned_data().get("name").unwrap(),
			&serde_json::json!("Form1")
		);
		assert_eq!(
			form2.cleaned_data().get("name").unwrap(),
			&serde_json::json!("Form2")
		);
	}

	#[test]
	fn test_form_with_initial_data() {
		let mut initial = HashMap::new();
		initial.insert("name".to_string(), serde_json::json!("Initial Name"));
		initial.insert("age".to_string(), serde_json::json!(25));

		let mut form = Form::with_initial(initial);

		let name_field = CharField::new("name".to_string());
		form.add_field(Box::new(name_field));

		let age_field = crate::IntegerField::new("age".to_string());
		form.add_field(Box::new(age_field));

		assert_eq!(
			form.initial().get("name").unwrap(),
			&serde_json::json!("Initial Name")
		);
		assert_eq!(form.initial().get("age").unwrap(), &serde_json::json!(25));
	}

	#[test]
	fn test_form_has_changed() {
		let mut initial = HashMap::new();
		initial.insert("name".to_string(), serde_json::json!("John"));

		let mut form = Form::with_initial(initial);

		let name_field = CharField::new("name".to_string());
		form.add_field(Box::new(name_field));

		// Same data as initial - should not have changed
		let mut data1 = HashMap::new();
		data1.insert("name".to_string(), serde_json::json!("John"));
		form.bind(data1);
		assert!(!form.has_changed());

		// Different data - should have changed
		let mut data2 = HashMap::new();
		data2.insert("name".to_string(), serde_json::json!("Jane"));
		form.bind(data2);
		assert!(form.has_changed());
	}

	#[test]
	fn test_form_index_access() {
		let mut form = Form::new();

		let name_field = CharField::new("name".to_string());
		form.add_field(Box::new(name_field));

		let field = &form["name"];
		assert_eq!(field.name(), "name");
	}

	#[test]
	#[should_panic(expected = "Field 'nonexistent' not found")]
	fn test_form_index_access_nonexistent() {
		let form = Form::new();
		let _ = &form["nonexistent"];
	}

	#[test]
	fn test_form_get_field() {
		let mut form = Form::new();

		let name_field = CharField::new("name".to_string());
		form.add_field(Box::new(name_field));

		assert!(form.get_field("name").is_some());
		assert!(form.get_field("nonexistent").is_none());
	}

	#[test]
	fn test_form_remove_field() {
		let mut form = Form::new();

		let name_field = CharField::new("name".to_string());
		form.add_field(Box::new(name_field));

		assert_eq!(form.field_count(), 1);

		let removed = form.remove_field("name");
		assert!(removed.is_some());
		assert_eq!(form.field_count(), 0);

		let not_removed = form.remove_field("nonexistent");
		assert!(not_removed.is_none());
	}

	#[test]
	fn test_form_custom_validation() {
		let mut form = Form::new();

		let mut password_field = CharField::new("password".to_string());
		password_field.min_length = Some(8);
		form.add_field(Box::new(password_field));

		let mut confirm_field = CharField::new("confirm".to_string());
		confirm_field.min_length = Some(8);
		form.add_field(Box::new(confirm_field));

		// Add custom validation to check passwords match
		form.add_clean_function(|data| {
			let password = data.get("password").and_then(|v| v.as_str());
			let confirm = data.get("confirm").and_then(|v| v.as_str());

			if password != confirm {
				return Err(FormError::Validation("Passwords do not match".to_string()));
			}

			Ok(())
		});

		// Test with matching passwords
		let mut data1 = HashMap::new();
		data1.insert("password".to_string(), serde_json::json!("secret123"));
		data1.insert("confirm".to_string(), serde_json::json!("secret123"));
		form.bind(data1);
		assert!(form.is_valid());

		// Test with non-matching passwords
		let mut data2 = HashMap::new();
		data2.insert("password".to_string(), serde_json::json!("secret123"));
		data2.insert("confirm".to_string(), serde_json::json!("different"));
		form.bind(data2);
		assert!(!form.is_valid());
		assert!(form.errors().contains_key(ALL_FIELDS_KEY));
	}

	#[test]
	fn test_form_prefix() {
		let mut form = Form::with_prefix("profile".to_string());
		assert_eq!(form.prefix(), "profile");
		assert_eq!(form.add_prefix_to_field_name("name"), "profile-name");

		form.set_prefix("user".to_string());
		assert_eq!(form.prefix(), "user");
		assert_eq!(form.add_prefix_to_field_name("email"), "user-email");
	}

	#[test]
	fn test_form_field_clean_function() {
		let mut form = Form::new();

		let mut name_field = CharField::new("name".to_string());
		name_field.required = true;
		form.add_field(Box::new(name_field));

		// Add field-specific clean function to uppercase the name
		form.add_field_clean_function("name", |value| {
			if let Some(s) = value.as_str() {
				Ok(serde_json::json!(s.to_uppercase()))
			} else {
				Err(FormError::Validation("Expected string".to_string()))
			}
		});

		let mut data = HashMap::new();
		data.insert("name".to_string(), serde_json::json!("john doe"));
		form.bind(data);

		assert!(form.is_valid());
		assert_eq!(
			form.cleaned_data().get("name").unwrap(),
			&serde_json::json!("JOHN DOE")
		);
	}
}
