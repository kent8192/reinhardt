//! WASM tests for authentication shared types
//!
//! Tests that authentication request/response types serialize correctly
//! in the WASM environment, ensuring client-server communication compatibility.
//!
//! **Run with**: `cargo make wasm-test`

#![cfg(wasm)]

use wasm_bindgen_test::*;

use examples_twitter::apps::auth::shared::types::*;

wasm_bindgen_test_configure!(run_in_browser);

// ============================================================================
// LoginRequest Tests
// ============================================================================

/// Test LoginRequest serialization roundtrip in WASM
#[wasm_bindgen_test]
fn test_login_request_serialization_roundtrip() {
	// Arrange
	let request = LoginRequest {
		email: "test@example.com".to_string(),
		password: "password123".to_string(),
	};

	// Act
	let json = serde_json::to_string(&request).unwrap();
	let deserialized: LoginRequest = serde_json::from_str(&json).unwrap();

	// Assert
	assert_eq!(deserialized.email, "test@example.com");
	assert_eq!(deserialized.password, "password123");
}

// ============================================================================
// RegisterRequest Tests
// ============================================================================

/// Test RegisterRequest serialization roundtrip in WASM
#[wasm_bindgen_test]
fn test_register_request_serialization_roundtrip() {
	// Arrange
	let request = RegisterRequest {
		username: "testuser".to_string(),
		email: "test@example.com".to_string(),
		password: "password123".to_string(),
		password_confirmation: "password123".to_string(),
	};

	// Act
	let json = serde_json::to_string(&request).unwrap();
	let deserialized: RegisterRequest = serde_json::from_str(&json).unwrap();

	// Assert
	assert_eq!(deserialized.username, "testuser");
	assert_eq!(deserialized.email, "test@example.com");
}

/// Test RegisterRequest password validation succeeds when passwords match
#[wasm_bindgen_test]
fn test_register_request_validate_passwords_match() {
	// Arrange
	let request = RegisterRequest {
		username: "testuser".to_string(),
		email: "test@example.com".to_string(),
		password: "password123".to_string(),
		password_confirmation: "password123".to_string(),
	};

	// Act & Assert
	assert!(request.validate_passwords_match().is_ok());
}

/// Test RegisterRequest password validation fails when passwords differ
#[wasm_bindgen_test]
fn test_register_request_validate_passwords_mismatch() {
	// Arrange
	let request = RegisterRequest {
		username: "testuser".to_string(),
		email: "test@example.com".to_string(),
		password: "password123".to_string(),
		password_confirmation: "different456".to_string(),
	};

	// Act
	let result = request.validate_passwords_match();

	// Assert
	assert!(result.is_err());
	assert_eq!(result.unwrap_err(), "Passwords do not match");
}

// ============================================================================
// UserInfo Tests
// ============================================================================

/// Test UserInfo serialization roundtrip in WASM
#[wasm_bindgen_test]
fn test_user_info_serialization_roundtrip() {
	// Arrange
	let user = UserInfo {
		id: uuid::Uuid::nil(),
		username: "testuser".to_string(),
		email: "test@example.com".to_string(),
		is_active: true,
	};

	// Act
	let json = serde_json::to_string(&user).unwrap();
	let deserialized: UserInfo = serde_json::from_str(&json).unwrap();

	// Assert
	assert_eq!(deserialized.username, "testuser");
	assert_eq!(deserialized.email, "test@example.com");
	assert!(deserialized.is_active);
}

/// Test UserInfo deserialization from JSON string
#[wasm_bindgen_test]
fn test_user_info_deserialization() {
	// Arrange
	let json = r#"{
		"id": "00000000-0000-0000-0000-000000000000",
		"username": "admin",
		"email": "admin@example.com",
		"is_active": false
	}"#;

	// Act
	let user: UserInfo = serde_json::from_str(json).unwrap();

	// Assert
	assert_eq!(user.username, "admin");
	assert_eq!(user.email, "admin@example.com");
	assert!(!user.is_active);
}

// ============================================================================
// SessionData Tests
// ============================================================================

/// Test SessionData serialization roundtrip in WASM
#[wasm_bindgen_test]
fn test_session_data_serialization_roundtrip() {
	// Arrange
	let session = SessionData {
		user_id: uuid::Uuid::nil(),
		username: "testuser".to_string(),
		email: "test@example.com".to_string(),
	};

	// Act
	let json = serde_json::to_string(&session).unwrap();
	let deserialized: SessionData = serde_json::from_str(&json).unwrap();

	// Assert
	assert_eq!(deserialized.username, "testuser");
	assert_eq!(deserialized.email, "test@example.com");
	assert_eq!(deserialized.user_id, uuid::Uuid::nil());
}
