//! # SetRoleStatement Unit Tests
//!
//! Comprehensive unit tests for SetRoleStatement covering:
//! - Happy Path: Normal operations
//! - Error Path: Validation and error handling
//! - Edge Cases: Boundary values and special cases
//! - Backend-Specific: PostgreSQL and MySQL variants
//!
//! ## Test Coverage
//!
//! - Statements tested: SetRoleStatement
//! - Code coverage target: 93%
//! - Total tests: ~30

use crate::backend::{MySqlQueryBuilder, PostgresQueryBuilder, QueryBuilder};
use crate::dcl::{RoleTarget, SetRoleStatement};
use rstest::rstest;

// ============================================================================
// Happy Path Tests
// ============================================================================

#[rstest]
fn test_set_role_new() {
	let stmt = SetRoleStatement::new();

	// Default should be None or not set
	assert!(stmt.target.is_none() || matches!(stmt.target, None));
}

#[rstest]
fn test_set_named_role() {
	let stmt = SetRoleStatement::new().role(RoleTarget::Named("admin".to_string()));

	assert!(matches!(stmt.target, Some(RoleTarget::Named(_))));
	if let Some(RoleTarget::Named(name)) = &stmt.target {
		assert_eq!(name, "admin");
	}
}

#[rstest]
fn test_set_role_none() {
	let stmt = SetRoleStatement::new().role(RoleTarget::None);

	assert!(matches!(stmt.target, Some(RoleTarget::None)));
}

#[rstest]
fn test_set_role_all() {
	let stmt = SetRoleStatement::new().role(RoleTarget::All);

	assert!(matches!(stmt.target, Some(RoleTarget::All)));
}

#[rstest]
fn test_set_role_all_except_single() {
	let stmt =
		SetRoleStatement::new().role(RoleTarget::AllExcept(vec!["restricted_role".to_string()]));

	assert!(matches!(stmt.target, Some(RoleTarget::AllExcept(_))));
	if let Some(RoleTarget::AllExcept(roles)) = &stmt.target {
		assert_eq!(roles.len(), 1);
	}
}

#[rstest]
fn test_set_role_all_except_multiple() {
	let roles = vec!["role1".to_string(), "role2".to_string()];
	let stmt = SetRoleStatement::new().role(RoleTarget::AllExcept(roles.clone()));

	assert!(matches!(stmt.target, Some(RoleTarget::AllExcept(_))));
	if let Some(RoleTarget::AllExcept(r)) = &stmt.target {
		assert_eq!(r, &roles);
	}
}

#[rstest]
fn test_set_role_default() {
	let stmt = SetRoleStatement::new().role(RoleTarget::Default);

	assert!(matches!(stmt.target, Some(RoleTarget::Default)));
}

#[rstest]
fn test_set_role_various_targets() {
	let targets = vec![
		RoleTarget::Named("admin".to_string()),
		RoleTarget::None,
		RoleTarget::All,
		RoleTarget::Default,
	];

	for target in targets {
		let stmt = SetRoleStatement::new().role(target.clone());
		assert_eq!(stmt.target, Some(target));
	}
}

#[rstest]
fn test_clone_statement() {
	let stmt1 = SetRoleStatement::new().role(RoleTarget::Named("admin".to_string()));
	let stmt2 = stmt1.clone();

	assert_eq!(stmt1.target, stmt2.target);
}

// ============================================================================
// Error Path Tests
// ============================================================================

#[rstest]
fn test_set_role_none_target_validation() {
	let stmt = SetRoleStatement::new();

	let result = stmt.validate();
	assert!(result.is_err());
}

#[rstest]
fn test_empty_role_name() {
	let stmt = SetRoleStatement::new().role(RoleTarget::Named("".to_string()));

	let result = stmt.validate();
	assert!(result.is_err());
}

#[rstest]
fn test_whitespace_only_role_name() {
	let stmt = SetRoleStatement::new().role(RoleTarget::Named("   ".to_string()));

	let result = stmt.validate();
	assert!(result.is_err());
}

#[rstest]
fn test_empty_all_except_list() {
	let stmt = SetRoleStatement::new().role(RoleTarget::AllExcept(vec![]));

	let result = stmt.validate();
	assert!(result.is_err());
}

#[rstest]
fn test_all_except_with_empty_role() {
	let stmt = SetRoleStatement::new().role(RoleTarget::AllExcept(vec![
		"role1".to_string(),
		"".to_string(),
	]));

	let result = stmt.validate();
	assert!(result.is_err());
}

#[rstest]
fn test_valid_named_role() {
	let stmt = SetRoleStatement::new().role(RoleTarget::Named("valid_role".to_string()));

	let result = stmt.validate();
	assert!(result.is_ok());
}

#[rstest]
fn test_valid_none_target() {
	let stmt = SetRoleStatement::new().role(RoleTarget::None);

	let result = stmt.validate();
	assert!(result.is_ok());
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[rstest]
fn test_all_except_single_role() {
	let stmt = SetRoleStatement::new().role(RoleTarget::AllExcept(vec!["restricted".to_string()]));

	if let Some(RoleTarget::AllExcept(roles)) = &stmt.target {
		assert_eq!(roles.len(), 1);
	} else {
		panic!("Expected AllExcept variant");
	}
}

#[rstest]
fn test_all_except_many_roles() {
	let roles: Vec<String> = (0..50).map(|i| format!("role_{}", i)).collect();
	let stmt = SetRoleStatement::new().role(RoleTarget::AllExcept(roles.clone()));

	if let Some(RoleTarget::AllExcept(r)) = &stmt.target {
		assert_eq!(r.len(), 50);
	}
}

#[rstest]
fn test_role_name_lengths() {
	let lengths = vec![1, 63, 64, 255];

	for length in lengths {
		let name = "a".repeat(length);
		let stmt = SetRoleStatement::new().role(RoleTarget::Named(name.clone()));

		if let Some(RoleTarget::Named(n)) = stmt.target {
			assert_eq!(n, name);
			assert_eq!(n.len(), length);
		} else {
			panic!("Expected Named variant");
		}
	}
}

#[rstest]
fn test_unicode_role_name() {
	let name = "ロール名";
	let stmt = SetRoleStatement::new().role(RoleTarget::Named(name.to_string()));

	if let Some(RoleTarget::Named(n)) = &stmt.target {
		assert_eq!(n, name);
	}
}

#[rstest]
fn test_role_with_special_characters() {
	let name = "test-role_123";
	let stmt = SetRoleStatement::new().role(RoleTarget::Named(name.to_string()));

	if let Some(RoleTarget::Named(n)) = &stmt.target {
		assert_eq!(n, name);
	}
}

// ============================================================================
// Backend-Specific Tests
// ============================================================================

#[rstest]
fn test_postgres_named_role() {
	let builder = PostgresQueryBuilder::new();
	let stmt = SetRoleStatement::new().role(RoleTarget::Named("admin".to_string()));

	let (sql, values) = builder.build_set_role(&stmt);

	assert_eq!(sql, r#"SET ROLE "admin""#);
	assert!(values.is_empty());
}

#[rstest]
fn test_postgres_role_none() {
	let builder = PostgresQueryBuilder::new();
	let stmt = SetRoleStatement::new().role(RoleTarget::None);

	let (sql, values) = builder.build_set_role(&stmt);

	assert_eq!(sql, r#"SET ROLE NONE"#);
	assert!(values.is_empty());
}

#[rstest]
fn test_mysql_named_role() {
	let builder = MySqlQueryBuilder::new();
	let stmt = SetRoleStatement::new().role(RoleTarget::Named("admin".to_string()));

	let (sql, values) = builder.build_set_role(&stmt);

	assert_eq!(sql, r#"SET ROLE `admin`"#);
	assert!(values.is_empty());
}

#[rstest]
fn test_mysql_role_all() {
	let builder = MySqlQueryBuilder::new();
	let stmt = SetRoleStatement::new().role(RoleTarget::All);

	let (sql, values) = builder.build_set_role(&stmt);

	assert_eq!(sql, "SET ROLE ALL");
	assert!(values.is_empty());
}

#[rstest]
fn test_mysql_role_none() {
	let builder = MySqlQueryBuilder::new();
	let stmt = SetRoleStatement::new().role(RoleTarget::None);

	let (sql, values) = builder.build_set_role(&stmt);

	assert_eq!(sql, "SET ROLE NONE");
	assert!(values.is_empty());
}

#[rstest]
fn test_mysql_role_all_except() {
	let builder = MySqlQueryBuilder::new();
	let stmt = SetRoleStatement::new().role(RoleTarget::AllExcept(vec!["restricted".to_string()]));

	let (sql, values) = builder.build_set_role(&stmt);

	assert!(sql.contains(r#"SET ROLE ALL EXCEPT"#));
	assert!(sql.contains("`restricted`"));
	assert!(values.is_empty());
}

#[rstest]
fn test_mysql_role_all_except_multiple() {
	let builder = MySqlQueryBuilder::new();
	let stmt = SetRoleStatement::new().role(RoleTarget::AllExcept(vec![
		"role1".to_string(),
		"role2".to_string(),
		"role3".to_string(),
	]));

	let (sql, values) = builder.build_set_role(&stmt);

	assert!(sql.contains(r#"SET ROLE ALL EXCEPT"#));
	assert!(sql.contains("`role1`"));
	assert!(sql.contains("`role2`"));
	assert!(sql.contains("`role3`"));
	assert!(values.is_empty());
}

#[rstest]
fn test_mysql_role_default() {
	let builder = MySqlQueryBuilder::new();
	let stmt = SetRoleStatement::new().role(RoleTarget::Default);

	let (sql, values) = builder.build_set_role(&stmt);

	assert_eq!(sql, "SET ROLE DEFAULT");
	assert!(values.is_empty());
}
