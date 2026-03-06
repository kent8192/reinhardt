//! Unit tests for TaggitError
//!
//! Tests all TaggitError variants and Result type alias.

use reinhardt_taggit::{Result, TaggitError};
use rstest::rstest;

/// Test InvalidTagName error creation and formatting
#[test]
fn test_error_invalid_tag_name() {
	// Arrange & Act
	let error = TaggitError::InvalidTagName("test tag".to_string());

	// Assert
	let msg = format!("{}", error);
	assert!(msg.contains("invalid"));
	assert!(msg.contains("test tag"));
}

/// Test TagNameTooLong error creation and formatting
#[test]
fn test_error_tag_name_too_long() {
	// Arrange & Act
	let error = TaggitError::TagNameTooLong { max: 255, len: 300 };

	// Assert
	let msg = format!("{}", error);
	assert!(msg.contains("255"));
	assert!(msg.contains("300"));
	assert!(msg.contains("too long"));
}

/// Test InvalidCharacters error creation and formatting
#[test]
fn test_error_invalid_characters() {
	// Arrange & Act
	let error = TaggitError::InvalidCharacters("tag@name".to_string());

	// Assert
	let msg = format!("{}", error);
	assert!(msg.contains("invalid characters"));
	assert!(msg.contains("tag@name"));
}

/// Test TagNotFound error creation and formatting
#[test]
fn test_error_tag_not_found() {
	// Arrange & Act
	let error = TaggitError::TagNotFound("nonexistent".to_string());

	// Assert
	let msg = format!("{}", error);
	assert!(msg.contains("not found"));
	assert!(msg.contains("nonexistent"));
}

/// Test TaggedItemNotFound error creation and formatting
#[test]
fn test_error_tagged_item_not_found() {
	// Arrange & Act
	let error = TaggitError::TaggedItemNotFound {
		content_type: "Food".to_string(),
		object_id: 42,
		tag_name: "spicy".to_string(),
	};

	// Assert
	let msg = format!("{}", error);
	assert!(msg.contains("not found"));
	assert!(msg.contains("Food"));
	assert!(msg.contains("42"));
	assert!(msg.contains("spicy"));
}

/// Test ObjectNotFound error creation and formatting
#[test]
fn test_error_object_not_found() {
	// Arrange & Act
	let error = TaggitError::ObjectNotFound {
		content_type: "Food".to_string(),
		object_id: 999,
	};

	// Assert
	let msg = format!("{}", error);
	assert!(msg.contains("not found"));
	assert!(msg.contains("Food"));
	assert!(msg.contains("999"));
}

/// Test DuplicateTag error creation and formatting
#[test]
fn test_error_duplicate_tag() {
	// Arrange & Act
	let error = TaggitError::DuplicateTag {
		content_type: "Food".to_string(),
		object_id: 42,
		tag_name: "spicy".to_string(),
	};

	// Assert
	let msg = format!("{}", error);
	assert!(msg.contains("Duplicate"));
	assert!(msg.contains("Food"));
	assert!(msg.contains("42"));
	assert!(msg.contains("spicy"));
}

/// Test DatabaseError error creation and formatting
#[test]
fn test_error_database_error() {
	// Arrange & Act
	let error = TaggitError::DatabaseError("connection failed".to_string());

	// Assert
	let msg = format!("{}", error);
	assert!(msg.contains("Database"));
	assert!(msg.contains("connection failed"));
}

/// Test TransactionError error creation and formatting
#[test]
fn test_error_transaction_error() {
	// Arrange & Act
	let error = TaggitError::TransactionError("rollback failed".to_string());

	// Assert
	let msg = format!("{}", error);
	assert!(msg.contains("Transaction"));
	assert!(msg.contains("rollback failed"));
}

/// Test ConfigError error creation and formatting
#[test]
fn test_error_config_error() {
	// Arrange & Act
	let error = TaggitError::ConfigError("invalid option".to_string());

	// Assert
	let msg = format!("{}", error);
	assert!(msg.contains("Configuration"));
	assert!(msg.contains("invalid option"));
}

/// Test `Result<T>` type alias with Ok value
#[test]
fn test_result_ok() {
	// Arrange
	fn ok_result() -> Result<String> {
		Ok("success".to_string())
	}

	// Act
	let result = ok_result();

	// Assert
	assert!(result.is_ok());
	assert_eq!(result.unwrap(), "success");
}

/// Test Result<T> type alias with Err value
#[test]
fn test_result_err() {
	// Arrange & Act
	let result: Result<String> = Err(TaggitError::TagNotFound("test".to_string()));

	// Assert
	assert!(result.is_err());
	assert!(matches!(result, Err(TaggitError::TagNotFound(_))));
}

/// Test Result<T> with ? operator (Ok case)
#[test]
fn test_result_question_mark_ok() {
	// Arrange
	fn returns_ok() -> Result<String> {
		Ok("value".to_string())
	}

	fn uses_question_mark() -> Result<String> {
		let value = returns_ok()?;
		Ok(value)
	}

	// Act & Assert
	let result = uses_question_mark();
	assert!(result.is_ok());
	assert_eq!(result.unwrap(), "value");
}

/// Test Result<T> with ? operator (Err case)
#[test]
fn test_result_question_mark_err() {
	// Arrange
	fn returns_err() -> Result<String> {
		Err(TaggitError::TagNotFound("test".to_string()))
	}

	fn uses_question_mark() -> Result<String> {
		let _value = returns_err()?;
		Ok("unreachable".to_string())
	}

	// Act & Assert
	let result = uses_question_mark();
	assert!(result.is_err());
}

/// Test all error variants are constructible
#[rstest]
#[case(TaggitError::InvalidTagName("test".to_string()))]
#[case(TaggitError::TagNameTooLong { max: 255, len: 300 })]
#[case(TaggitError::InvalidCharacters("test@".to_string()))]
#[case(TaggitError::TagNotFound("missing".to_string()))]
#[case(TaggitError::DatabaseError("db error".to_string()))]
#[case(TaggitError::TransactionError("tx error".to_string()))]
#[case(TaggitError::ConfigError("config error".to_string()))]
fn test_all_error_variants_constructible(#[case] error: TaggitError) {
	// Assert - just verify the error can be created and formatted
	let msg = format!("{}", error);
	assert!(!msg.is_empty());
}
