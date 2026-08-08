use reinhardt_forms::{CharField, FieldError, Form, FormError, PASSWORD_REDACTED, PasswordField};
use rstest::rstest;
use serde_json::json;
use std::collections::HashMap;

#[rstest]
fn form_and_bound_field_integration_preserves_prefixed_invalid_values() {
	// Arrange
	let mut form = Form::with_prefix("profile".to_string());
	form.add_field(Box::new(
		CharField::new("display_name".to_string()).required(),
	));
	form.bind(HashMap::from([(
		"profile-display_name".to_string(),
		json!(7),
	)]));

	// Act
	assert!(!form.is_valid());
	let bound = form.get_bound_field("display_name").unwrap();

	// Assert
	assert_eq!(bound.html_name(), "profile-display_name");
	assert_eq!(bound.id_for_label(), "id_profile-display_name");
	assert_eq!(bound.value(), Some(&json!(7)));
	assert_eq!(bound.errors(), ["Value must be a string"]);
}

#[rstest]
fn form_and_bound_field_integration_redacts_valid_passwords() {
	// Arrange
	let mut valid_form = Form::new();
	valid_form.add_field(Box::new(PasswordField::new("password").min_length(8)));
	valid_form.bind(HashMap::from([("password".to_string(), json!("Valid1!a"))]));

	// Act
	let valid = valid_form.is_valid();
	let valid_bound = valid_form.get_bound_field("password").unwrap();

	// Assert
	assert!(valid);
	assert_eq!(valid_bound.value(), Some(&json!(PASSWORD_REDACTED)));

	// Arrange
	let mut invalid_form = Form::new();
	invalid_form.add_field(Box::new(PasswordField::new("password").min_length(8)));
	invalid_form.bind(HashMap::from([("password".to_string(), json!("short"))]));

	// Act
	let invalid = invalid_form.is_valid();
	let invalid_bound = invalid_form.get_bound_field("password").unwrap();

	// Assert
	assert!(!invalid);
	assert_eq!(invalid_bound.value(), Some(&json!("short")));
}

#[rstest]
fn form_and_bound_field_integration_keeps_passwords_redacted_after_later_errors() {
	// Arrange
	let mut form = Form::new();
	form.add_field(Box::new(PasswordField::new("password").min_length(8)));
	form.add_clean_function(|_| {
		Err(FormError::Field {
			field: "password".to_string(),
			error: FieldError::validation(None, "Password was rejected later."),
		})
	});
	form.bind(HashMap::from([("password".to_string(), json!("Valid1!a"))]));

	// Act
	let valid = form.is_valid();
	let bound = form.get_bound_field("password").unwrap();

	// Assert
	assert!(!valid);
	assert_eq!(bound.value(), Some(&json!(PASSWORD_REDACTED)));
	assert_eq!(bound.errors(), ["Password was rejected later."]);
}
