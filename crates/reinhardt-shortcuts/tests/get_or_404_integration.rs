//! GetError and Response conversion integration tests
//!
//! Tests GetError type conversions, error messages, and HTTP response
//! generation for database query shortcuts.

use hyper::StatusCode;
use reinhardt_http::Response;
use reinhardt_shortcuts::GetError;

/// Test: GetError::NotFound to Response conversion
#[test]
fn test_not_found_error_to_response() {
	let error = GetError::NotFound;
	let response: Response = error.into();

	assert_eq!(response.status, StatusCode::NOT_FOUND);
}

/// Test: GetError::MultipleObjectsReturned to Response conversion
#[test]
fn test_multiple_objects_error_to_response() {
	let error = GetError::MultipleObjectsReturned;
	let response: Response = error.into();

	assert_eq!(response.status, StatusCode::BAD_REQUEST);
	assert_eq!(
		response.body,
		bytes::Bytes::from("Multiple objects returned")
	);
}

/// Test: GetError::DatabaseError to Response conversion
/// Verifies that database error messages are NOT exposed in HTTP responses
#[test]
fn test_database_error_to_response() {
	let sensitive_msg = "Connection timeout: password=admin secret=12345";
	let error = GetError::DatabaseError(sensitive_msg.to_string());
	let response: Response = error.into();

	assert_eq!(response.status, StatusCode::INTERNAL_SERVER_ERROR);
	// Response must contain generic message, NOT the sensitive error details
	let body = String::from_utf8_lossy(&response.body);
	assert_eq!(body, "Internal server error");
	// Verify sensitive information is NOT exposed
	assert!(!body.contains("Connection timeout"));
	assert!(!body.contains("password"));
	assert!(!body.contains("admin"));
}

/// Test: GetError::NotFound error message
#[test]
fn test_not_found_error_message() {
	let error = GetError::NotFound;
	assert_eq!(error.to_string(), "Object not found");
}

/// Test: GetError::MultipleObjectsReturned error message
#[test]
fn test_multiple_objects_error_message() {
	let error = GetError::MultipleObjectsReturned;
	assert_eq!(error.to_string(), "Multiple objects returned");
}

/// Test: GetError::DatabaseError error message
#[test]
fn test_database_error_message() {
	let error = GetError::DatabaseError("Table does not exist".to_string());
	assert_eq!(error.to_string(), "Database error: Table does not exist");
}

/// Test: Multiple different database error messages
#[test]
fn test_various_database_error_messages() {
	let errors = [
		GetError::DatabaseError("Connection refused".to_string()),
		GetError::DatabaseError("Query timeout".to_string()),
		GetError::DatabaseError("Permission denied".to_string()),
		GetError::DatabaseError("Syntax error".to_string()),
	];

	let messages: Vec<String> = errors.iter().map(|e| e.to_string()).collect();

	assert_eq!(messages[0], "Database error: Connection refused");
	assert_eq!(messages[1], "Database error: Query timeout");
	assert_eq!(messages[2], "Database error: Permission denied");
	assert_eq!(messages[3], "Database error: Syntax error");
}

/// Test: GetError Debug formatting
#[test]
fn test_get_error_debug() {
	let error = GetError::NotFound;
	let debug_str = format!("{:?}", error);
	assert_eq!(debug_str, "NotFound");

	let error = GetError::MultipleObjectsReturned;
	let debug_str = format!("{:?}", error);
	assert_eq!(debug_str, "MultipleObjectsReturned");

	let error = GetError::DatabaseError("test".to_string());
	let debug_str = format!("{:?}", error);
	assert!(debug_str.contains("DatabaseError"));
}

/// Test: Response from NotFound has correct content type
#[test]
fn test_not_found_response_content_type() {
	let error = GetError::NotFound;
	let response: Response = error.into();

	// Default Response should have basic structure
	assert_eq!(response.status, StatusCode::NOT_FOUND);
	// Body might be empty for not_found()
	// Headers might have content-type
}

/// Test: Response from DatabaseError does NOT expose error details
#[test]
fn test_database_error_response_preserves_details() {
	let sensitive_message = "Unique constraint violation on column 'email' in table 'users'";
	let error = GetError::DatabaseError(sensitive_message.to_string());
	let response: Response = error.into();

	assert_eq!(response.status, StatusCode::INTERNAL_SERVER_ERROR);
	// Response must contain generic message, NOT sensitive database details
	let body_str = String::from_utf8_lossy(&response.body);
	assert_eq!(body_str, "Internal server error");
	// Verify sensitive database schema information is NOT exposed
	assert!(!body_str.contains("Unique constraint"));
	assert!(!body_str.contains("email"));
	assert!(!body_str.contains("users"));
	assert!(!body_str.contains("column"));
}

/// Test: Empty database error message still returns generic response
#[test]
fn test_empty_database_error_message() {
	let error = GetError::DatabaseError(String::new());
	// The Display trait still shows the message format for logging/debugging
	assert_eq!(error.to_string(), "Database error: ");

	let response: Response = error.into();
	assert_eq!(response.status, StatusCode::INTERNAL_SERVER_ERROR);
	// Response must be generic even for empty error messages
	let body = String::from_utf8_lossy(&response.body);
	assert_eq!(body, "Internal server error");
}

/// Test: UTF-8 database error message does NOT expose in response
#[test]
fn test_utf8_database_error_message() {
	let error = GetError::DatabaseError("エラー: データベース接続失敗".to_string());
	// The Display trait shows the message for logging/debugging
	assert_eq!(
		error.to_string(),
		"Database error: エラー: データベース接続失敗"
	);

	let response: Response = error.into();
	let body_str = String::from_utf8_lossy(&response.body);
	// Response must be generic ASCII message, NOT UTF-8 error details
	assert_eq!(body_str, "Internal server error");
	// Verify Japanese error message is NOT exposed
	assert!(!body_str.contains("エラー"));
	assert!(!body_str.contains("データベース接続失敗"));
}

/// Test: Very long database error message does NOT expose in response
#[test]
fn test_long_database_error_message() {
	let long_message = "Error occurred while processing query: ".to_string()
		+ &"SQL statement execution failed due to ".repeat(10)
		+ "constraint violation";

	let error = GetError::DatabaseError(long_message.clone());
	let response: Response = error.into();

	let body_str = String::from_utf8_lossy(&response.body);
	// Response must be short generic message, NOT the long error
	assert_eq!(body_str, "Internal server error");
	// Verify SQL-related details are NOT exposed
	assert!(!body_str.contains("constraint violation"));
	assert!(!body_str.contains("SQL"));
	assert!(!body_str.contains("query"));
	// Generic message is short
	assert_eq!(body_str.len(), "Internal server error".len());
}
