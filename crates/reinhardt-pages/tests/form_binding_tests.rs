#![cfg(not(target_arch = "wasm32"))]
//! Form Binding Integration Tests
//!
//! Tests for the FormBinding system's two-way data binding between
//! Signals and FormComponent fields.
//!
//! Success Criteria:
//! 1. Field binding to Signal works correctly
//! 2. Two-way synchronization (Signal ↔ Form) functions properly
//! 3. Multiple field bindings can coexist
//! 4. Unbinding removes synchronization
//! 5. Validation integrates with binding system
//!
//! Test Categories:
//! - Category 1: Binding Creation and Management (5 tests)
//! - Category 2: Two-Way Synchronization (6 tests)
//! - Category 3: Multiple Bindings (4 tests)
//! - Category 4: Validation Integration (3 tests)
//!
//! Total: 18 tests

use reinhardt_pages::reactive::Signal;
use reinhardt_pages::{FieldMetadata, FormBinding, FormComponent, FormMetadata, Widget};
use serial_test::serial;
use std::collections::HashMap;

// ============================================================================
// Helper Functions
// ============================================================================

/// Creates a test form with common fields
fn create_test_form() -> FormComponent {
	let metadata = FormMetadata {
		fields: vec![
			FieldMetadata {
				name: "username".to_string(),
				label: Some("Username".to_string()),
				required: false,
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

// ============================================================================
// Category 1: Binding Creation and Management (5 tests)
// ============================================================================

/// Tests creating FormBinding from FormComponent
#[test]
#[serial]
fn test_form_binding_creation() {
	let form = create_test_form();
	let binding = FormBinding::new(form);

	assert_eq!(binding.bound_fields().count(), 0);
}

/// Tests binding a single field
#[test]
#[serial]
fn test_bind_single_field() {
	let form = create_test_form();
	let mut binding = FormBinding::new(form);

	let signal = Signal::new(String::new());
	binding.bind_field("username", signal.clone());

	assert!(binding.is_bound("username"));
	assert_eq!(binding.bound_fields().count(), 1);
}

/// Tests binding multiple fields
#[test]
#[serial]
fn test_bind_multiple_fields() {
	let form = create_test_form();
	let mut binding = FormBinding::new(form);

	let username_signal = Signal::new(String::new());
	let email_signal = Signal::new(String::new());

	binding.bind_field("username", username_signal);
	binding.bind_field("email", email_signal);

	assert!(binding.is_bound("username"));
	assert!(binding.is_bound("email"));
	assert_eq!(binding.bound_fields().count(), 2);
}

/// Tests unbinding a field
#[test]
#[serial]
fn test_unbind_field() {
	let form = create_test_form();
	let mut binding = FormBinding::new(form);

	let signal = Signal::new(String::new());
	binding.bind_field("username", signal);

	assert!(binding.is_bound("username"));

	binding.unbind_field("username");

	assert!(!binding.is_bound("username"));
	assert_eq!(binding.bound_fields().count(), 0);
}

/// Tests getting binding for a field
#[test]
#[serial]
fn test_get_binding() {
	let form = create_test_form();
	let mut binding = FormBinding::new(form);

	let signal = Signal::new("initial_value".to_string());
	binding.bind_field("username", signal.clone());

	let retrieved = binding.get_binding("username");
	assert!(retrieved.is_some());

	// Verify it's the same signal
	assert_eq!(retrieved.unwrap().get(), "initial_value");
}

// ============================================================================
// Category 2: Two-Way Synchronization (6 tests)
// ============================================================================

/// Tests Signal → Form synchronization
#[test]
#[serial]
fn test_signal_to_form_sync() {
	let form = create_test_form();
	let mut binding = FormBinding::new(form);

	let signal = Signal::new("initial".to_string());
	binding.bind_field("username", signal.clone());

	// Update signal
	signal.set("updated_from_signal".to_string());
	binding.sync_to_form();

	// Verify form was updated
	assert_eq!(binding.get_field_value("username"), "updated_from_signal");
}

/// Tests Form → Signal synchronization
#[test]
#[serial]
fn test_form_to_signal_sync() {
	let form = create_test_form();
	let mut binding = FormBinding::new(form);

	let signal = Signal::new(String::new());
	binding.bind_field("username", signal.clone());

	// Update form
	binding.set_field_value("username", "updated_from_form");
	binding.sync_from_form();

	// Verify signal was updated
	assert_eq!(signal.get(), "updated_from_form");
}

/// Tests sync_from_form updates all bindings
#[test]
#[serial]
fn test_sync_from_form() {
	let form = create_test_form();
	let mut binding = FormBinding::new(form);

	let username_signal = Signal::new(String::new());
	let email_signal = Signal::new(String::new());

	binding.bind_field("username", username_signal.clone());
	binding.bind_field("email", email_signal.clone());

	// Set form values
	binding.set_field_value("username", "john_doe");
	binding.set_field_value("email", "john@example.com");

	// Sync to all signals
	binding.sync_from_form();

	// Verify all signals updated
	assert_eq!(username_signal.get(), "john_doe");
	assert_eq!(email_signal.get(), "john@example.com");
}

/// Tests sync_to_form updates all fields
#[test]
#[serial]
fn test_sync_to_form() {
	let form = create_test_form();
	let mut binding = FormBinding::new(form);

	let username_signal = Signal::new("alice".to_string());
	let email_signal = Signal::new("alice@example.com".to_string());

	binding.bind_field("username", username_signal.clone());
	binding.bind_field("email", email_signal.clone());

	// Sync from all signals to form
	binding.sync_to_form();

	// Verify all form fields updated
	assert_eq!(binding.get_field_value("username"), "alice");
	assert_eq!(binding.get_field_value("email"), "alice@example.com");
}

/// Tests bidirectional sync maintains consistency
#[test]
#[serial]
fn test_bidirectional_sync() {
	let form = create_test_form();
	let mut binding = FormBinding::new(form);

	let signal = Signal::new("start".to_string());
	binding.bind_field("username", signal.clone());

	// Signal → Form
	signal.set("from_signal".to_string());
	binding.sync_to_form();
	assert_eq!(binding.get_field_value("username"), "from_signal");

	// Form → Signal
	binding.set_field_value("username", "from_form");
	binding.sync_from_form();
	assert_eq!(signal.get(), "from_form");
}

/// Tests sync preserves values for unbound fields
#[test]
#[serial]
fn test_sync_preserves_unbound_fields() {
	let metadata = FormMetadata {
		fields: vec![
			FieldMetadata {
				name: "bound_field".to_string(),
				label: Some("Bound".to_string()),
				required: false,
				help_text: None,
				widget: Widget::TextInput,
				initial: None,
			},
			FieldMetadata {
				name: "unbound_field".to_string(),
				label: Some("Unbound".to_string()),
				required: false,
				help_text: None,
				widget: Widget::TextInput,
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

	let form = FormComponent::new(metadata, "/api/submit");
	let mut binding = FormBinding::new(form);

	// Bind only one field
	let signal = Signal::new("bound_value".to_string());
	binding.bind_field("bound_field", signal);

	// Set unbound field
	binding.set_field_value("unbound_field", "unbound_value");

	// Sync to form
	binding.sync_to_form();

	// Verify unbound field unchanged
	assert_eq!(binding.get_field_value("unbound_field"), "unbound_value");
}

// ============================================================================
// Category 3: Multiple Bindings (4 tests)
// ============================================================================

/// Tests independent updates to multiple bindings
#[test]
#[serial]
fn test_multiple_bindings_independent_updates() {
	let form = create_test_form();
	let mut binding = FormBinding::new(form);

	let signal1 = Signal::new("value1".to_string());
	let signal2 = Signal::new("value2".to_string());

	binding.bind_field("username", signal1.clone());
	binding.bind_field("email", signal2.clone());

	// Sync initial values to form
	binding.sync_to_form();
	assert_eq!(binding.get_field_value("username"), "value1");
	assert_eq!(binding.get_field_value("email"), "value2");

	// Update only signal1 and sync
	signal1.set("updated1".to_string());
	binding.sync_to_form();

	// Verify both fields have current signal values
	assert_eq!(binding.get_field_value("username"), "updated1");
	assert_eq!(binding.get_field_value("email"), "value2");

	// Update signal2 and sync
	signal2.set("updated2".to_string());
	binding.sync_to_form();

	assert_eq!(binding.get_field_value("username"), "updated1");
	assert_eq!(binding.get_field_value("email"), "updated2");
}

/// Tests bound_fields iterator
#[test]
#[serial]
fn test_bound_fields_iterator() {
	let form = create_test_form();
	let mut binding = FormBinding::new(form);

	let signal1 = Signal::new(String::new());
	let signal2 = Signal::new(String::new());

	binding.bind_field("username", signal1);
	binding.bind_field("email", signal2);

	let bound: Vec<_> = binding.bound_fields().collect();
	assert_eq!(bound.len(), 2);
	assert!(bound.iter().any(|s| s.as_str() == "username"));
	assert!(bound.iter().any(|s| s.as_str() == "email"));
}

/// Tests rebinding same field updates binding
#[test]
#[serial]
fn test_rebind_field() {
	let form = create_test_form();
	let mut binding = FormBinding::new(form);

	let signal1 = Signal::new("signal1".to_string());
	binding.bind_field("username", signal1.clone());

	// Rebind with new signal
	let signal2 = Signal::new("signal2".to_string());
	binding.bind_field("username", signal2.clone());

	binding.sync_to_form();

	// Should use signal2 value
	assert_eq!(binding.get_field_value("username"), "signal2");
}

/// Tests unbinding one field doesn't affect others
#[test]
#[serial]
fn test_unbind_preserves_other_bindings() {
	let form = create_test_form();
	let mut binding = FormBinding::new(form);

	let signal1 = Signal::new("value1".to_string());
	let signal2 = Signal::new("value2".to_string());

	binding.bind_field("username", signal1.clone());
	binding.bind_field("email", signal2.clone());

	// Unbind one field
	binding.unbind_field("username");

	// Other binding still works
	assert!(binding.is_bound("email"));
	assert_eq!(binding.bound_fields().count(), 1);

	signal2.set("updated".to_string());
	binding.sync_to_form();
	assert_eq!(binding.get_field_value("email"), "updated");
}

// ============================================================================
// Category 4: Validation Integration (3 tests)
// ============================================================================

/// Tests validation through FormBinding
#[test]
#[serial]
fn test_validation_integration() {
	let form = create_test_form();
	let mut binding = FormBinding::new(form);

	// email is required in test form
	assert!(!binding.validate());

	binding.set_field_value("email", "test@example.com");
	assert!(binding.validate());
}

/// Tests errors accessible through FormBinding
#[test]
#[serial]
fn test_errors_integration() {
	let form = create_test_form();
	let binding = FormBinding::new(form);

	binding.validate();

	let errors = binding.errors();
	assert!(errors.contains_key("email"));
	assert_eq!(errors.get("email").unwrap()[0], "This field is required.");
}

/// Tests validation updates after sync
#[test]
#[serial]
fn test_validation_after_sync() {
	let form = create_test_form();
	let mut binding = FormBinding::new(form);

	let email_signal = Signal::new("valid@example.com".to_string());
	binding.bind_field("email", email_signal.clone());

	// Sync from signal
	binding.sync_to_form();

	// Should pass validation now
	assert!(binding.validate());
	assert!(binding.errors().is_empty());
}
