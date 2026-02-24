//! # DropUserStatement Unit Tests
//!
//! Comprehensive unit tests for DropUserStatement covering:
//! - Happy Path: Normal operations
//! - Error Path: Validation and error handling
//! - Edge Cases: Boundary values and special cases
//!
//! ## Test Coverage
//!
//! - Statements tested: DropUserStatement
//! - Code coverage target: 96%
//! - Total tests: ~20

use crate::dcl::DropUserStatement;
use rstest::rstest;

// ============================================================================
// Happy Path Tests
// ============================================================================

#[rstest]
fn test_drop_user_new() {
	let stmt = DropUserStatement::new();
	assert!(stmt.user_names.is_empty());
	assert!(!stmt.if_exists);
}

#[rstest]
#[case("test_user")]
fn test_drop_single_user(#[case] user_name: &str) {
	let stmt = DropUserStatement::new().user(user_name);
	assert_eq!(stmt.user_names.len(), 1);
	assert_eq!(stmt.user_names[0], user_name);
}

#[rstest]
fn test_drop_multiple_users() {
	let stmt = DropUserStatement::new().user("user1").user("user2");
	assert_eq!(stmt.user_names.len(), 2);
	assert!(stmt.user_names.contains(&String::from("user1")));
	assert!(stmt.user_names.contains(&String::from("user2")));
}

#[rstest]
fn test_drop_user_with_if_exists() {
	let stmt = DropUserStatement::new().user("test_user").if_exists(true);
	assert!(stmt.if_exists);
}

#[rstest]
fn test_drop_user_with_users_method() {
	let users = vec!["user1".to_string(), "user2".to_string()];
	let stmt = DropUserStatement::new().users(users.clone());
	assert_eq!(stmt.user_names, users);
}

// ============================================================================
// Error Path Tests
// ============================================================================

#[rstest]
fn test_empty_user_list_validation() {
	let stmt = DropUserStatement::new();
	let result = stmt.validate();
	assert!(result.is_err());
}

#[rstest]
fn test_empty_user_in_list() {
	let stmt = DropUserStatement::new().user("");
	let result = stmt.validate();
	assert!(result.is_err());
}

#[rstest]
fn test_whitespace_only_user() {
	let stmt = DropUserStatement::new().user("   ");
	let result = stmt.validate();
	assert!(result.is_err());
}

#[rstest]
fn test_valid_user_validation() {
	let stmt = DropUserStatement::new().user("valid_user");
	let result = stmt.validate();
	assert!(result.is_ok());
}

#[rstest]
fn test_multiple_valid_users_validation() {
	let stmt = DropUserStatement::new().users(vec![
		"user1".to_string(),
		"user2".to_string(),
		"user3".to_string(),
	]);
	let result = stmt.validate();
	assert!(result.is_ok());
}
