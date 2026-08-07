//! Cross-crate parameter error conversion tests.

use reinhardt_di::params::{ParamError, ParamType};
use reinhardt_http::Error as CoreError;

#[test]
fn parameter_errors_preserve_authentication_internal_and_validation_semantics() {
	// Arrange
	let authentication = ParamError::Authentication("token missing".to_owned());
	let internal = ParamError::Internal("provider unavailable".to_owned());
	let invalid = ParamError::invalid::<u64>(ParamType::Query, "expected a positive integer");

	// Act
	let authentication = CoreError::from(authentication);
	let internal = CoreError::from(internal);
	let invalid = CoreError::from(invalid);

	// Assert
	assert!(
		matches!(authentication, CoreError::Authentication(message) if message == "token missing")
	);
	assert!(matches!(internal, CoreError::Internal(message) if message == "provider unavailable"));
	assert!(
		matches!(invalid, CoreError::ParamValidation(context) if context.message == "expected a positive integer")
	);
}
