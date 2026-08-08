use reinhardt_forms::{CharField, Form};
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
	form.bind(HashMap::from([("display_name".to_string(), json!(7))]));

	// Act
	assert!(!form.is_valid());
	let bound = form.get_bound_field("display_name").unwrap();

	// Assert
	assert_eq!(bound.html_name(), "profile-display_name");
	assert_eq!(bound.id_for_label(), "id_profile-display_name");
	assert_eq!(bound.value(), Some(&json!(7)));
	assert_eq!(bound.errors(), ["Value must be a string"]);
}
