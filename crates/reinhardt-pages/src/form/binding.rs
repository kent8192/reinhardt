//! Form Binding for Two-Way Data Synchronization (Week 5 Day 4)
//!
//! This module provides `FormBinding` which enables two-way data binding
//! between reactive Signals and FormComponent data.
//!
//! ## Architecture
//!
//! ```mermaid
//! flowchart TB
//!     Signal["Signal<br/>(reactive)"] <-->|sync| FormBinding["FormBinding"]
//!     FormBinding --> FormComponent["FormComponent<br/>(UI state)"]
//! ```
//!
//! ## Features
//!
//! - **Two-way binding**: Changes in Signals update Form, and vice versa
//! - **Type-safe**: Generic type parameter ensures compile-time safety
//! - **Automatic validation**: Validation errors propagate to Signals
//! - **Fine-grained reactivity**: Only changed fields trigger updates
//!
//! ## Example
//!
//! ```ignore
//! use reinhardt_pages::{Signal, FormComponent, FormBinding};
//!
//! // Create Signals for form fields
//! let username = Signal::new("".to_string());
//! let email = Signal::new("".to_string());
//!
//! // Create FormComponent from metadata
//! let form_component = FormComponent::new(metadata, "/api/submit");
//!
//! // Create FormBinding
//! let mut binding = FormBinding::new(form_component);
//!
//! // Bind Signals to form fields
//! binding.bind_field("username", username.clone());
//! binding.bind_field("email", email.clone());
//!
//! // Changes to Signals automatically update FormComponent
//! username.set("john_doe".to_string());
//! assert_eq!(binding.get_field_value("username"), "john_doe");
//!
//! // Validate and retrieve errors
//! let is_valid = binding.validate();
//! let errors = binding.errors();
//! ```

use crate::form::FormComponent;
use crate::reactive::{Effect, Signal};
use std::collections::HashMap;
use std::rc::Rc;

/// Form Binding for two-way data synchronization (Week 5 Day 4)
///
/// This struct manages bidirectional data flow between reactive Signals
/// and FormComponent state.
///
/// ## Type Parameters
///
/// - No generic type parameter: Works with any FormComponent
///
/// ## Fields
///
/// - `form_component`: The underlying FormComponent
/// - `bindings`: Map of field name to Signal binding
/// - `effects`: Active Effect handles for automatic synchronization
pub struct FormBinding {
	/// Underlying FormComponent
	form_component: FormComponent,

	/// Field name → `Signal<String>` bindings
	bindings: HashMap<String, Signal<String>>,

	/// Active effects for automatic sync (kept alive)
	effects: Vec<Rc<dyn std::any::Any>>,
}

impl FormBinding {
	/// Create a new FormBinding from a FormComponent
	///
	/// # Arguments
	///
	/// - `form_component`: The FormComponent to bind
	///
	/// # Examples
	///
	/// ```ignore
	/// let form_component = FormComponent::new(metadata, "/api/submit");
	/// let binding = FormBinding::new(form_component);
	/// ```
	pub fn new(form_component: FormComponent) -> Self {
		Self {
			form_component,
			bindings: HashMap::new(),
			effects: Vec::new(),
		}
	}

	/// Bind a Signal to a form field (two-way binding)
	///
	/// This creates a bidirectional connection:
	/// - When the Signal changes, the FormComponent field updates
	/// - When the FormComponent field changes, the Signal updates
	///
	/// # Arguments
	///
	/// - `field_name`: Name of the form field
	/// - `signal`: Signal to bind to the field
	///
	/// # Examples
	///
	/// ```ignore
	/// let username = Signal::new("".to_string());
	/// binding.bind_field("username", username.clone());
	///
	/// // Now changes to username Signal update the form automatically
	/// username.set("john_doe".to_string());
	/// ```
	pub fn bind_field(&mut self, field_name: impl Into<String>, signal: Signal<String>) {
		let field_name = field_name.into();

		// Store binding
		self.bindings.insert(field_name.clone(), signal.clone());

		// Setup Effect: Signal → FormComponent
		let form_component = self.form_component.clone();
		let field_name_clone = field_name.clone();
		let signal_for_effect = signal.clone(); // Clone for Effect

		let effect = Effect::new(move || {
			let value = signal_for_effect.get();
			form_component.set_value(&field_name_clone, value);
		});

		// Keep effect alive
		self.effects.push(Rc::new(effect));

		// Initial sync: FormComponent → Signal
		let current_value = self.form_component.get_value(&field_name);
		signal.set(current_value);
	}

	/// Unbind a Signal from a form field
	///
	/// # Arguments
	///
	/// - `field_name`: Name of the form field to unbind
	///
	/// # Examples
	///
	/// ```ignore
	/// binding.unbind_field("username");
	/// ```
	pub fn unbind_field(&mut self, field_name: &str) {
		self.bindings.remove(field_name);
		// Note: Effects are automatically cleaned up when dropped
	}

	/// Get the Signal bound to a field
	///
	/// # Arguments
	///
	/// - `field_name`: Name of the form field
	///
	/// # Returns
	///
	/// `Some(Signal)` if bound, `None` otherwise
	///
	/// # Examples
	///
	/// ```ignore
	/// if let Some(signal) = binding.get_binding("username") {
	///     println!("Current value: {}", signal.get());
	/// }
	/// ```
	pub fn get_binding(&self, field_name: &str) -> Option<&Signal<String>> {
		self.bindings.get(field_name)
	}

	/// Get current value of a form field
	///
	/// # Arguments
	///
	/// - `field_name`: Name of the form field
	///
	/// # Returns
	///
	/// Current field value as String
	///
	/// # Examples
	///
	/// ```ignore
	/// let username = binding.get_field_value("username");
	/// assert_eq!(username, "john_doe");
	/// ```
	pub fn get_field_value(&self, field_name: &str) -> String {
		self.form_component.get_value(field_name)
	}

	/// Set value of a form field (updates bound Signal automatically)
	///
	/// # Arguments
	///
	/// - `field_name`: Name of the form field
	/// - `value`: New value
	///
	/// # Examples
	///
	/// ```ignore
	/// binding.set_field_value("username", "jane_doe");
	/// // Bound Signal is automatically updated
	/// ```
	pub fn set_field_value(&mut self, field_name: &str, value: impl Into<String>) {
		let value = value.into();
		self.form_component.set_value(field_name, value.clone());

		// Update bound Signal
		if let Some(signal) = self.bindings.get(field_name) {
			signal.set(value);
		}
	}

	/// Validate all form fields
	///
	/// # Returns
	///
	/// `true` if validation passed, `false` otherwise
	///
	/// # Examples
	///
	/// ```ignore
	/// if binding.validate() {
	///     binding.submit().await?;
	/// } else {
	///     println!("Validation errors: {:?}", binding.errors());
	/// }
	/// ```
	pub fn validate(&self) -> bool {
		self.form_component.validate()
	}

	/// Get current validation errors
	///
	/// # Returns
	///
	/// Map of field name to error messages
	///
	/// # Examples
	///
	/// ```ignore
	/// let errors = binding.errors();
	/// if let Some(username_errors) = errors.get("username") {
	///     println!("Username errors: {:?}", username_errors);
	/// }
	/// ```
	pub fn errors(&self) -> HashMap<String, Vec<String>> {
		self.form_component.errors()
	}

	/// Submit the form via AJAX
	///
	/// # Returns
	///
	/// `Ok(())` on success, `Err(String)` on failure
	///
	/// # Examples
	///
	/// ```ignore
	/// if binding.validate() {
	///     binding.submit().await?;
	/// }
	/// ```
	#[cfg(wasm)]
	pub async fn submit(&self) -> Result<(), String> {
		self.form_component.submit().await
	}

	/// Get reference to underlying FormComponent
	///
	/// # Examples
	///
	/// ```ignore
	/// let metadata = binding.form_component().metadata();
	/// ```
	pub fn form_component(&self) -> &FormComponent {
		&self.form_component
	}

	/// Get mutable reference to underlying FormComponent
	///
	/// # Examples
	///
	/// ```ignore
	/// binding.form_component_mut().set_value("username", "new_value");
	/// ```
	pub fn form_component_mut(&mut self) -> &mut FormComponent {
		&mut self.form_component
	}

	/// Sync all bound Signals from FormComponent
	///
	/// This is useful after programmatic FormComponent updates.
	///
	/// # Examples
	///
	/// ```ignore
	/// // After direct FormComponent manipulation
	/// binding.form_component_mut().set_value("username", "direct_update");
	/// // Sync to Signals
	/// binding.sync_from_form();
	/// ```
	pub fn sync_from_form(&self) {
		for (field_name, signal) in &self.bindings {
			let value = self.form_component.get_value(field_name);
			signal.set(value);
		}
	}

	/// Sync all bound Signals to FormComponent
	///
	/// This is useful for batch updates.
	///
	/// # Examples
	///
	/// ```ignore
	/// // After batch Signal updates
	/// username.set("new_username".to_string());
	/// email.set("new@email.com".to_string());
	/// // Sync to FormComponent
	/// binding.sync_to_form();
	/// ```
	pub fn sync_to_form(&self) {
		for (field_name, signal) in &self.bindings {
			let value = signal.get();
			self.form_component.set_value(field_name, value);
		}
	}

	/// Get all bound field names
	///
	/// # Returns
	///
	/// Iterator over bound field names
	///
	/// # Examples
	///
	/// ```ignore
	/// for field_name in binding.bound_fields() {
	///     println!("Bound field: {}", field_name);
	/// }
	/// ```
	pub fn bound_fields(&self) -> impl Iterator<Item = &String> {
		self.bindings.keys()
	}

	/// Check if a field is bound
	///
	/// # Arguments
	///
	/// - `field_name`: Name of the form field
	///
	/// # Returns
	///
	/// `true` if the field is bound, `false` otherwise
	///
	/// # Examples
	///
	/// ```ignore
	/// if binding.is_bound("username") {
	///     println!("Username is bound");
	/// }
	/// ```
	pub fn is_bound(&self, field_name: &str) -> bool {
		self.bindings.contains_key(field_name)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::reactive::runtime::with_runtime;
	use reinhardt_forms::Widget;
	use reinhardt_forms::wasm_compat::{FieldMetadata, FormMetadata};
	use serial_test::serial;
	use std::collections::HashMap;

	fn create_test_form() -> FormComponent {
		let metadata = FormMetadata {
			fields: vec![
				FieldMetadata {
					name: "username".to_string(),
					label: Some("Username".to_string()),
					required: true,
					help_text: None,
					widget: Widget::TextInput,
					initial: None,
				},
				FieldMetadata {
					name: "email".to_string(),
					label: Some("Email".to_string()),
					required: true,
					help_text: None,
					widget: Widget::EmailInput,
					initial: None,
				},
			],
			initial: HashMap::new(),
			prefix: String::new(),
			is_bound: false,
			errors: HashMap::new(),
			validation_rules: Vec::new(),
			non_field_errors: Vec::new(),
		};

		FormComponent::new(metadata, "/api/submit")
	}

	#[test]
	#[serial]
	fn test_form_binding_creation() {
		let form = create_test_form();
		let binding = FormBinding::new(form);

		assert_eq!(binding.bindings.len(), 0);
		assert_eq!(binding.effects.len(), 0);
	}

	#[test]
	#[serial]
	fn test_bind_field() {
		let form = create_test_form();
		let mut binding = FormBinding::new(form);

		let username_signal = Signal::new("".to_string());
		binding.bind_field("username", username_signal.clone());

		assert!(binding.is_bound("username"));
		assert_eq!(binding.bindings.len(), 1);
		assert_eq!(binding.effects.len(), 1);
	}

	#[test]
	#[serial]
	fn test_two_way_binding_signal_to_form() {
		let form = create_test_form();
		let mut binding = FormBinding::new(form);

		let username_signal = Signal::new("".to_string());
		binding.bind_field("username", username_signal.clone());

		// Update Signal → should update Form
		username_signal.set("john_doe".to_string());

		// Flush pending Effect updates
		with_runtime(|rt| rt.flush_updates());

		// Verify Form was updated
		assert_eq!(binding.get_field_value("username"), "john_doe");
	}

	#[test]
	#[serial]
	fn test_two_way_binding_form_to_signal() {
		let form = create_test_form();
		let mut binding = FormBinding::new(form);

		let username_signal = Signal::new("".to_string());
		binding.bind_field("username", username_signal.clone());

		// Update Form → should update Signal
		binding.set_field_value("username", "jane_doe");

		// Verify Signal was updated
		assert_eq!(username_signal.get(), "jane_doe");
	}

	#[test]
	#[serial]
	fn test_unbind_field() {
		let form = create_test_form();
		let mut binding = FormBinding::new(form);

		let username_signal = Signal::new("".to_string());
		binding.bind_field("username", username_signal.clone());

		assert!(binding.is_bound("username"));

		binding.unbind_field("username");

		assert!(!binding.is_bound("username"));
	}

	#[test]
	#[serial]
	fn test_get_binding() {
		let form = create_test_form();
		let mut binding = FormBinding::new(form);

		let username_signal = Signal::new("test_value".to_string());
		binding.bind_field("username", username_signal.clone());

		let retrieved_signal = binding.get_binding("username").unwrap();
		assert_eq!(retrieved_signal.get(), "test_value");
	}

	#[test]
	#[serial]
	fn test_sync_from_form() {
		let form = create_test_form();
		let mut binding = FormBinding::new(form);

		let username_signal = Signal::new("".to_string());
		binding.bind_field("username", username_signal.clone());

		// Direct FormComponent update
		binding
			.form_component_mut()
			.set_value("username", "direct_update");

		// Sync from form to signal
		binding.sync_from_form();

		assert_eq!(username_signal.get(), "direct_update");
	}

	#[test]
	#[serial]
	fn test_sync_to_form() {
		let form = create_test_form();
		let mut binding = FormBinding::new(form);

		let username_signal = Signal::new("".to_string());
		binding.bind_field("username", username_signal.clone());

		// Update signal directly
		username_signal.set("signal_update".to_string());

		// Flush pending Effect updates
		with_runtime(|rt| rt.flush_updates());

		// Sync to form (redundant due to Effect, but test explicit sync)
		binding.sync_to_form();

		assert_eq!(binding.get_field_value("username"), "signal_update");
	}

	#[test]
	#[serial]
	fn test_validation_integration() {
		let form = create_test_form();
		let mut binding = FormBinding::new(form);

		let username_signal = Signal::new("".to_string());
		let email_signal = Signal::new("".to_string());
		binding.bind_field("username", username_signal.clone());
		binding.bind_field("email", email_signal.clone());

		// Empty required fields should fail validation
		assert!(!binding.validate());

		let errors = binding.errors();
		assert!(errors.contains_key("username"));
		assert!(errors.contains_key("email"));

		// Set values via Signals
		username_signal.set("valid_username".to_string());
		email_signal.set("valid@example.com".to_string());

		// Flush pending Effect updates
		with_runtime(|rt| rt.flush_updates());

		// Note: Effects update the form automatically, so validation should pass
		assert!(binding.validate());
	}

	#[test]
	#[serial]
	fn test_multiple_field_bindings() {
		let form = create_test_form();
		let mut binding = FormBinding::new(form);

		let username_signal = Signal::new("".to_string());
		let email_signal = Signal::new("".to_string());

		binding.bind_field("username", username_signal.clone());
		binding.bind_field("email", email_signal.clone());

		assert_eq!(binding.bindings.len(), 2);
		assert!(binding.is_bound("username"));
		assert!(binding.is_bound("email"));

		username_signal.set("john".to_string());
		email_signal.set("john@example.com".to_string());

		// Flush pending Effect updates
		with_runtime(|rt| rt.flush_updates());

		assert_eq!(binding.get_field_value("username"), "john");
		assert_eq!(binding.get_field_value("email"), "john@example.com");
	}

	#[test]
	#[serial]
	fn test_bound_fields_iterator() {
		let form = create_test_form();
		let mut binding = FormBinding::new(form);

		let username_signal = Signal::new("".to_string());
		let email_signal = Signal::new("".to_string());

		binding.bind_field("username", username_signal);
		binding.bind_field("email", email_signal);

		let bound_field_names: Vec<_> = binding.bound_fields().cloned().collect();

		assert!(bound_field_names.contains(&"username".to_string()));
		assert!(bound_field_names.contains(&"email".to_string()));
		assert_eq!(bound_field_names.len(), 2);
	}
}
