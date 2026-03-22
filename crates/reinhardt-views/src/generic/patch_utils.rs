//! Utility helpers for JSON Merge Patch (RFC 7396) operations.
//!
//! Provides [`merge_patch_object_into`] to validate and merge a PATCH body
//! into an existing JSON representation, used by all generic update views.

/// Merges a JSON patch object into an existing JSON object in place.
///
/// Both `target` and `patch` must be JSON objects (`Value::Object`).
/// Returns an error string if either value is not an object, which the
/// caller can wrap in the appropriate error type.
///
/// # Errors
///
/// - Returns `Err` if `patch` is not a JSON object.
/// - Returns `Err` if `target` is not a JSON object.
pub(crate) fn merge_patch_object_into(
	target: &mut serde_json::Value,
	patch: &serde_json::Value,
) -> Result<(), String> {
	let patch_obj = patch
		.as_object()
		.ok_or_else(|| "PATCH request body must be a JSON object".to_string())?;

	let target_obj = target
		.as_object_mut()
		.ok_or_else(|| "Existing object is not a JSON object".to_string())?;

	for (key, value) in patch_obj {
		target_obj.insert(key.clone(), value.clone());
	}

	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;
	use rstest::rstest;
	use serde_json::json;

	#[rstest]
	fn test_merge_patch_overwrites_existing_fields() {
		// Arrange
		let mut target = json!({"name": "Alice", "age": 30});
		let patch = json!({"age": 31});

		// Act
		merge_patch_object_into(&mut target, &patch).unwrap();

		// Assert
		assert_eq!(target, json!({"name": "Alice", "age": 31}));
	}

	#[rstest]
	fn test_merge_patch_adds_new_fields() {
		// Arrange
		let mut target = json!({"name": "Alice"});
		let patch = json!({"email": "alice@example.com"});

		// Act
		merge_patch_object_into(&mut target, &patch).unwrap();

		// Assert
		assert_eq!(
			target,
			json!({"name": "Alice", "email": "alice@example.com"})
		);
	}

	#[rstest]
	fn test_merge_patch_rejects_non_object_patch() {
		// Arrange
		let mut target = json!({"name": "Alice"});
		let patch = json!("not an object");

		// Act
		let result = merge_patch_object_into(&mut target, &patch);

		// Assert
		assert_eq!(
			result.unwrap_err(),
			"PATCH request body must be a JSON object"
		);
	}

	#[rstest]
	fn test_merge_patch_rejects_non_object_target() {
		// Arrange
		let mut target = json!("not an object");
		let patch = json!({"name": "Alice"});

		// Act
		let result = merge_patch_object_into(&mut target, &patch);

		// Assert
		assert_eq!(result.unwrap_err(), "Existing object is not a JSON object");
	}
}
