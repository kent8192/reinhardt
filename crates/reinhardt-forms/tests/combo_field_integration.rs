use reinhardt_forms::{CharField, ColorField, ComboField, Field, UUIDField};
use rstest::rstest;
use serde_json::json;

#[rstest]
fn combo_field_preserves_validator_order_and_first_failure() {
	// Arrange
	let normalized = ComboField::new("identifier")
		.add_validator(Box::new(UUIDField::new("identifier")))
		.add_validator(Box::new(CharField::new("identifier".to_string())));
	let first_failure = ComboField::new("identifier")
		.add_validator(Box::new(
			UUIDField::new("identifier").error_message("invalid", "UUID must be first."),
		))
		.add_validator(Box::new(ColorField::new("identifier")))
		.required(false);

	// Act and assert
	assert_eq!(
		normalized
			.clean(Some(&json!("550E8400-E29B-41D4-A716-446655440000")))
			.unwrap(),
		json!("550e8400-e29b-41d4-a716-446655440000"),
	);
	assert_eq!(
		first_failure
			.clean(Some(&json!("not-a-uuid")))
			.unwrap_err()
			.to_string(),
		"UUID must be first.",
	);
	assert_eq!(first_failure.clean(None).unwrap(), json!(null));
	assert!(!first_failure.has_changed(Some(&json!("same")), Some(&json!("same"))));
	assert!(first_failure.has_changed(Some(&json!("one")), Some(&json!("two"))));
}
