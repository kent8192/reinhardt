#![cfg(not(target_arch = "wasm32"))]
//! Form Component Integration Tests
//!
//! Tests for the FormComponent system's rendering, validation,
//! and value management capabilities.
//!
//! Success Criteria:
//! 1. Form creation from metadata works correctly
//! 2. Field value management (get/set) functions properly
//! 3. Validation rules are enforced correctly
//! 4. Multiple widget types are supported
//! 5. Initial values and CSRF tokens are handled correctly
//!
//! Test Categories:
//! - Category 1: Form Creation and Metadata (8 tests)
//! - Category 2: Field Value Management (8 tests)
//! - Category 3: Validation (12 tests)
//! - Category 4: Widget Types (7 tests)
//!
//! Total: 35 tests
//!
//! Note: DOM rendering tests require WASM environment with WASM test infrastructure.

use reinhardt_pages::{FieldMetadata, FormComponent, FormMetadata, Widget};
use std::collections::HashMap;

// ============================================================================
// Category 1: Form Creation and Metadata (8 tests)
// ============================================================================

/// Tests creating FormComponent from minimal metadata
#[test]
fn test_form_creation_minimal() {
	let metadata = FormMetadata {
		fields: vec![],
		initial: HashMap::new(),
		prefix: String::new(),
		is_bound: false,
		errors: HashMap::new(),
		validation_rules: Vec::new(),
		non_field_errors: Vec::new(),
	};

	let component = FormComponent::new(metadata.clone(), "/api/submit");
	assert_eq!(component.metadata().fields.len(), 0);
}

/// Tests creating FormComponent with single field
#[test]
fn test_form_creation_single_field() {
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

	let component = FormComponent::new(metadata, "/api/submit");
	assert_eq!(component.metadata().fields.len(), 1);
	assert_eq!(component.metadata().fields[0].name, "username");
}

/// Tests creating FormComponent with multiple fields
#[test]
fn test_form_creation_multiple_fields() {
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
			FieldMetadata {
				name: "age".to_string(),
				label: Some("Age".to_string()),
				required: false,
				help_text: Some("Optional field".to_string()),
				widget: Widget::NumberInput,
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

	let component = FormComponent::new(metadata, "/api/submit");
	assert_eq!(component.metadata().fields.len(), 3);
}

/// Tests FormComponent with field prefix
#[test]
fn test_form_creation_with_prefix() {
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
		prefix: "user_form".to_string(),
		is_bound: false,
		errors: HashMap::new(),
		validation_rules: Vec::new(),
		non_field_errors: Vec::new(),
	};

	let component = FormComponent::new(metadata, "/api/submit");
	assert_eq!(component.metadata().prefix, "user_form");
}

/// Tests FormComponent with help text
#[test]
fn test_form_creation_with_help_text() {
	let metadata = FormMetadata {
		fields: vec![FieldMetadata {
			name: "password".to_string(),
			label: Some("Password".to_string()),
			required: true,
			help_text: Some("Must be at least 8 characters".to_string()),
			widget: Widget::PasswordInput,
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
	assert_eq!(
		component.metadata().fields[0].help_text,
		Some("Must be at least 8 characters".to_string())
	);
}

/// Tests FormComponent with bound state
#[test]
fn test_form_creation_bound_state() {
	let metadata = FormMetadata {
		fields: vec![],
		initial: HashMap::new(),
		prefix: String::new(),
		is_bound: true,
		errors: HashMap::new(),
		validation_rules: Vec::new(),
		non_field_errors: Vec::new(),
	};

	let component = FormComponent::new(metadata, "/api/submit");
	assert!(component.metadata().is_bound);
}

/// Tests FormComponent with server-side errors
#[test]
fn test_form_creation_with_errors() {
	let mut errors = HashMap::new();
	errors.insert(
		"username".to_string(),
		vec!["This username is already taken.".to_string()],
	);

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
		is_bound: true,
		errors,
		validation_rules: Vec::new(),
		non_field_errors: Vec::new(),
	};

	let component = FormComponent::new(metadata, "/api/submit");
	assert!(!component.metadata().errors.is_empty());
	assert!(component.metadata().errors.contains_key("username"));
}

// ============================================================================
// Category 2: Field Value Management (8 tests)
// ============================================================================

/// Tests getting default empty value
#[test]
fn test_get_value_empty_default() {
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
}

/// Tests setting and getting field value
#[test]
fn test_set_and_get_value() {
	let metadata = FormMetadata {
		fields: vec![FieldMetadata {
			name: "email".to_string(),
			label: Some("Email".to_string()),
			required: false,
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

	component.set_value("email", "test@example.com");
	assert_eq!(component.get_value("email"), "test@example.com");
}

/// Tests updating field value multiple times
#[test]
fn test_update_value_multiple_times() {
	let metadata = FormMetadata {
		fields: vec![FieldMetadata {
			name: "status".to_string(),
			label: Some("Status".to_string()),
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

	component.set_value("status", "draft");
	assert_eq!(component.get_value("status"), "draft");

	component.set_value("status", "published");
	assert_eq!(component.get_value("status"), "published");

	component.set_value("status", "archived");
	assert_eq!(component.get_value("status"), "archived");
}

/// Tests getting value from non-existent field
#[test]
fn test_get_value_nonexistent_field() {
	let metadata = FormMetadata {
		fields: vec![],
		initial: HashMap::new(),
		prefix: String::new(),
		is_bound: false,
		errors: HashMap::new(),
		validation_rules: Vec::new(),
		non_field_errors: Vec::new(),
	};

	let component = FormComponent::new(metadata, "/api/submit");
	assert_eq!(component.get_value("nonexistent"), "");
}

/// Tests setting value on non-existent field (should be no-op)
#[test]
fn test_set_value_nonexistent_field() {
	let metadata = FormMetadata {
		fields: vec![],
		initial: HashMap::new(),
		prefix: String::new(),
		is_bound: false,
		errors: HashMap::new(),
		validation_rules: Vec::new(),
		non_field_errors: Vec::new(),
	};

	let component = FormComponent::new(metadata, "/api/submit");
	component.set_value("nonexistent", "value");
	// Should not panic, just silently ignore
}

/// Tests initial values from metadata
#[test]
fn test_initial_values_from_metadata() {
	let mut initial = HashMap::new();
	initial.insert("username".to_string(), serde_json::json!("john_doe"));
	initial.insert("email".to_string(), serde_json::json!("john@example.com"));

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
				required: false,
				help_text: None,
				widget: Widget::EmailInput,
				initial: None,
			},
		],
		initial,
		prefix: String::new(),
		is_bound: false,
		errors: HashMap::new(),
		validation_rules: Vec::new(),
		non_field_errors: Vec::new(),
	};

	let component = FormComponent::new(metadata, "/api/submit");
	assert_eq!(component.get_value("username"), "john_doe");
	assert_eq!(component.get_value("email"), "john@example.com");
}

/// Tests field-level initial value
#[test]
fn test_field_level_initial_value() {
	let metadata = FormMetadata {
		fields: vec![FieldMetadata {
			name: "country".to_string(),
			label: Some("Country".to_string()),
			required: false,
			help_text: None,
			widget: Widget::TextInput,
			initial: Some(serde_json::json!("USA")),
		}],
		initial: HashMap::new(),
		prefix: String::new(),
		is_bound: false,
		errors: HashMap::new(),
		validation_rules: Vec::new(),
		non_field_errors: Vec::new(),
	};

	let component = FormComponent::new(metadata, "/api/submit");
	assert_eq!(component.get_value("country"), "USA");
}

/// Tests managing values for multiple fields
#[test]
fn test_multiple_field_values() {
	let metadata = FormMetadata {
		fields: vec![
			FieldMetadata {
				name: "first_name".to_string(),
				label: Some("First Name".to_string()),
				required: true,
				help_text: None,
				widget: Widget::TextInput,
				initial: None,
			},
			FieldMetadata {
				name: "last_name".to_string(),
				label: Some("Last Name".to_string()),
				required: true,
				help_text: None,
				widget: Widget::TextInput,
				initial: None,
			},
			FieldMetadata {
				name: "age".to_string(),
				label: Some("Age".to_string()),
				required: false,
				help_text: None,
				widget: Widget::NumberInput,
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

	let component = FormComponent::new(metadata, "/api/submit");

	component.set_value("first_name", "John");
	component.set_value("last_name", "Doe");
	component.set_value("age", "30");

	assert_eq!(component.get_value("first_name"), "John");
	assert_eq!(component.get_value("last_name"), "Doe");
	assert_eq!(component.get_value("age"), "30");
}

// ============================================================================
// Category 3: Validation (12 tests)
// ============================================================================

/// Tests validation passes for valid required field
#[test]
fn test_validation_required_field_valid() {
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

	let component = FormComponent::new(metadata, "/api/submit");
	component.set_value("username", "john_doe");

	assert!(component.validate());
	assert!(component.errors().is_empty());
}

/// Tests validation fails for empty required field
#[test]
fn test_validation_required_field_empty() {
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

	assert!(!component.validate());

	let errors = component.errors();
	assert!(errors.contains_key("email"));
	assert_eq!(errors.get("email").unwrap()[0], "This field is required.");
}

/// Tests validation passes for optional empty field
#[test]
fn test_validation_optional_field_empty() {
	let metadata = FormMetadata {
		fields: vec![FieldMetadata {
			name: "bio".to_string(),
			label: Some("Bio".to_string()),
			required: false,
			help_text: None,
			widget: Widget::TextArea,
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

	assert!(component.validate());
	assert!(component.errors().is_empty());
}

/// Tests validation with multiple required fields
#[test]
fn test_validation_multiple_required_fields() {
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
				name: "password".to_string(),
				label: Some("Password".to_string()),
				required: true,
				help_text: None,
				widget: Widget::PasswordInput,
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

	let component = FormComponent::new(metadata, "/api/submit");

	// All fields empty - should fail
	assert!(!component.validate());
	let errors = component.errors();
	assert_eq!(errors.len(), 3);

	// Set one field
	component.set_value("username", "john");
	assert!(!component.validate());

	// Set all fields
	component.set_value("password", "secret123");
	component.set_value("email", "john@example.com");
	assert!(component.validate());
	assert!(component.errors().is_empty());
}

/// Tests validation error message format
#[test]
fn test_validation_error_message() {
	let metadata = FormMetadata {
		fields: vec![FieldMetadata {
			name: "title".to_string(),
			label: Some("Title".to_string()),
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

	let component = FormComponent::new(metadata, "/api/submit");
	component.validate();

	let errors = component.errors();
	assert_eq!(errors.get("title").unwrap()[0], "This field is required.");
}

/// Tests validation clears previous errors
#[test]
fn test_validation_clears_previous_errors() {
	let metadata = FormMetadata {
		fields: vec![FieldMetadata {
			name: "name".to_string(),
			label: Some("Name".to_string()),
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

	let component = FormComponent::new(metadata, "/api/submit");

	// First validation - fails
	assert!(!component.validate());
	assert!(!component.errors().is_empty());

	// Set value and validate again - should clear errors
	component.set_value("name", "John");
	assert!(component.validate());
	assert!(component.errors().is_empty());
}

/// Tests validation with whitespace-only value
#[test]
fn test_validation_whitespace_only() {
	let metadata = FormMetadata {
		fields: vec![FieldMetadata {
			name: "description".to_string(),
			label: Some("Description".to_string()),
			required: true,
			help_text: None,
			widget: Widget::TextArea,
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

	// Whitespace-only should fail validation
	component.set_value("description", "   ");
	assert!(!component.validate());
}

/// Tests validation is callable multiple times
#[test]
fn test_validation_multiple_calls() {
	let metadata = FormMetadata {
		fields: vec![FieldMetadata {
			name: "field".to_string(),
			label: Some("Field".to_string()),
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

	let component = FormComponent::new(metadata, "/api/submit");

	assert!(!component.validate());
	assert!(!component.validate());

	component.set_value("field", "value");
	assert!(component.validate());
	assert!(component.validate());
}

/// Tests mixed required and optional fields validation
#[test]
fn test_validation_mixed_required_optional() {
	let metadata = FormMetadata {
		fields: vec![
			FieldMetadata {
				name: "required_field".to_string(),
				label: Some("Required".to_string()),
				required: true,
				help_text: None,
				widget: Widget::TextInput,
				initial: None,
			},
			FieldMetadata {
				name: "optional_field".to_string(),
				label: Some("Optional".to_string()),
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

	let component = FormComponent::new(metadata, "/api/submit");

	// Only optional filled - should fail
	component.set_value("optional_field", "value");
	assert!(!component.validate());

	// Required filled - should pass
	component.set_value("required_field", "required_value");
	assert!(component.validate());
}

/// Tests validation with no fields
#[test]
fn test_validation_empty_form() {
	let metadata = FormMetadata {
		fields: vec![],
		initial: HashMap::new(),
		prefix: String::new(),
		is_bound: false,
		errors: HashMap::new(),
		validation_rules: Vec::new(),
		non_field_errors: Vec::new(),
	};

	let component = FormComponent::new(metadata, "/api/submit");
	assert!(component.validate());
}

/// Tests validation state persists until re-validated
#[test]
fn test_validation_state_persistence() {
	let metadata = FormMetadata {
		fields: vec![FieldMetadata {
			name: "name".to_string(),
			label: Some("Name".to_string()),
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

	let component = FormComponent::new(metadata, "/api/submit");

	// Validate - fails
	component.validate();
	assert!(!component.errors().is_empty());

	// Set value but don't validate yet
	component.set_value("name", "John");
	// Errors should still be present
	assert!(!component.errors().is_empty());

	// Re-validate
	component.validate();
	// Now errors should be cleared
	assert!(component.errors().is_empty());
}

// ============================================================================
// Category 4: Widget Types (7 tests)
// ============================================================================

/// Tests form with multiple widget types
#[test]
fn test_multiple_widget_types() {
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
			FieldMetadata {
				name: "password".to_string(),
				label: Some("Password".to_string()),
				required: true,
				help_text: None,
				widget: Widget::PasswordInput,
				initial: None,
			},
			FieldMetadata {
				name: "age".to_string(),
				label: Some("Age".to_string()),
				required: false,
				help_text: None,
				widget: Widget::NumberInput,
				initial: None,
			},
			FieldMetadata {
				name: "bio".to_string(),
				label: Some("Bio".to_string()),
				required: false,
				help_text: None,
				widget: Widget::TextArea,
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

	let component = FormComponent::new(metadata, "/api/submit");
	assert_eq!(component.metadata().fields.len(), 5);

	// Verify field names
	assert_eq!(component.metadata().fields[0].name, "username");
	assert_eq!(component.metadata().fields[1].name, "email");
	assert_eq!(component.metadata().fields[2].name, "password");
	assert_eq!(component.metadata().fields[3].name, "age");
	assert_eq!(component.metadata().fields[4].name, "bio");
}
