//! Unit tests for DiError to reinhardt_core::exception::Error conversion

use reinhardt_core::exception::Error;
use reinhardt_di::DiError;
use rstest::*;

#[rstest]
fn authentication_error_maps_to_http_401() {
	// Arrange
	let di_error = DiError::Authentication("User is not authenticated".to_string());

	// Act
	let error: Error = di_error.into();

	// Assert
	assert!(matches!(error, Error::Authentication(ref msg) if msg == "User is not authenticated"));
	assert_eq!(error.status_code(), 401);
}

#[rstest]
fn not_found_error_maps_to_internal() {
	// Arrange
	let di_error = DiError::NotFound("SomeService".to_string());

	// Act
	let error: Error = di_error.into();

	// Assert
	assert!(matches!(error, Error::Internal(_)));
	assert_eq!(error.status_code(), 500);
}

#[rstest]
fn circular_dependency_error_maps_to_internal() {
	// Arrange
	let di_error = DiError::CircularDependency("A -> B -> A".to_string());

	// Act
	let error: Error = di_error.into();

	// Assert
	assert!(matches!(error, Error::Internal(_)));
	assert_eq!(error.status_code(), 500);
}

#[rstest]
fn provider_error_maps_to_internal() {
	// Arrange
	let di_error = DiError::ProviderError("provider failed".to_string());

	// Act
	let error: Error = di_error.into();

	// Assert
	assert!(matches!(error, Error::Internal(_)));
	assert_eq!(error.status_code(), 500);
}

#[rstest]
fn scope_error_maps_to_internal() {
	// Arrange
	let di_error = DiError::ScopeError("invalid scope".to_string());

	// Act
	let error: Error = di_error.into();

	// Assert
	assert!(matches!(error, Error::Internal(_)));
	assert_eq!(error.status_code(), 500);
}

#[rstest]
fn internal_di_error_maps_to_internal() {
	// Arrange
	let di_error = DiError::Internal {
		message: "something went wrong".to_string(),
	};

	// Act
	let error: Error = di_error.into();

	// Assert
	assert!(matches!(error, Error::Internal(_)));
	assert_eq!(error.status_code(), 500);
}
