//! # DropRoleStatement Unit Tests
//!
//! Comprehensive unit tests for DropRoleStatement covering:
//! - Happy Path: Normal operations
//! - Error Path: Validation and error handling
//! - Edge Cases: Boundary values and special cases
//!
//! ## Test Coverage
//!
//! - Statements tested: DropRoleStatement
//! - Code coverage target: 97%
//! - Total tests: ~20

use crate::dcl::DropRoleStatement;
use rstest::rstest;

// ============================================================================
// Happy Path Tests
// ============================================================================

#[rstest]
fn test_drop_role_new() {
	let stmt = DropRoleStatement::new();
	assert!(stmt.role_names.is_empty());
	assert!(!stmt.if_exists);
}

#[rstest]
#[case("test_role")]
fn test_drop_single_role(#[case] role_name: &str) {
	let stmt = DropRoleStatement::new().role(role_name);
	assert_eq!(stmt.role_names.len(), 1);
	assert_eq!(stmt.role_names[0], role_name);
}

#[rstest]
fn test_drop_multiple_roles() {
	let stmt = DropRoleStatement::new().role("role_1").role("role_2");
	assert_eq!(stmt.role_names.len(), 2);
	assert!(stmt.role_names.contains(&String::from("role_1")));
	assert!(stmt.role_names.contains(&String::from("role_2")));
}

#[rstest]
fn test_drop_role_with_if_exists() {
	let stmt = DropRoleStatement::new().role("test_role").if_exists(true);
	assert!(stmt.if_exists);
}

#[rstest]
fn test_drop_role_with_roles_method() {
	let roles = vec!["role_1".to_string(), "role_2".to_string()];
	let stmt = DropRoleStatement::new().roles(roles.clone());
	assert_eq!(stmt.role_names, roles);
}

// ============================================================================
// Error Path Tests
// ============================================================================

#[rstest]
fn test_empty_role_list_validation() {
	let stmt = DropRoleStatement::new();
	let result = stmt.validate();
	assert!(result.is_err());
}

#[rstest]
fn test_empty_role_in_list() {
	let stmt = DropRoleStatement::new().role("");
	let result = stmt.validate();
	assert!(result.is_err());
}

#[rstest]
fn test_whitespace_only_role() {
	let stmt = DropRoleStatement::new().role("   ");
	let result = stmt.validate();
	assert!(result.is_err());
}

#[rstest]
fn test_valid_role_validation() {
	let stmt = DropRoleStatement::new().role("valid_role");
	let result = stmt.validate();
	assert!(result.is_ok());
}

#[rstest]
fn test_multiple_valid_roles_validation() {
	let stmt = DropRoleStatement::new().roles(vec![
		"role_1".to_string(),
		"role_2".to_string(),
		"role_3".to_string(),
	]);
	let result = stmt.validate();
	assert!(result.is_ok());
}
