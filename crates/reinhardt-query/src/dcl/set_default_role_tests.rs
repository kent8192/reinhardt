//! # SetDefaultRoleStatement Unit Tests
//!
//! Comprehensive unit tests for SetDefaultRoleStatement covering:
//! - Happy Path: Normal operations
//! - Error Path: Validation and error handling
//! - Backend Panic Tests: Unsupported backend verification
//!
//! ## Test Coverage
//!
//! - Statements tested: SetDefaultRoleStatement
//! - Code coverage target: 92%
//! - Total tests: ~25

use crate::backend::{MySqlQueryBuilder, PostgresQueryBuilder, QueryBuilder, SqliteQueryBuilder};
use crate::dcl::{DefaultRoleSpec, SetDefaultRoleStatement};
use rstest::rstest;

// ============================================================================
// Happy Path Tests
// ============================================================================

#[rstest]
fn test_set_default_role_new() {
	let stmt = SetDefaultRoleStatement::new();

	assert!(stmt.role_spec.is_none());
	assert!(stmt.user_names.is_empty());
}

#[rstest]
fn test_set_default_role_with_role_list() {
	let roles = vec!["role1".to_string(), "role2".to_string()];
	let stmt = SetDefaultRoleStatement::new()
		.roles(DefaultRoleSpec::RoleList(roles.clone()))
		.user("test_user@localhost");

	assert!(matches!(stmt.role_spec, Some(DefaultRoleSpec::RoleList(_))));
	if let Some(DefaultRoleSpec::RoleList(r)) = &stmt.role_spec {
		assert_eq!(*r, roles);
	}
}

#[rstest]
fn test_set_default_role_all() {
	let stmt = SetDefaultRoleStatement::new()
		.roles(DefaultRoleSpec::All)
		.user("test_user@localhost");

	assert!(matches!(stmt.role_spec, Some(DefaultRoleSpec::All)));
}

#[rstest]
fn test_set_default_role_none() {
	let stmt = SetDefaultRoleStatement::new()
		.roles(DefaultRoleSpec::None)
		.user("test_user@localhost");

	assert!(matches!(stmt.role_spec, Some(DefaultRoleSpec::None)));
}

#[rstest]
fn test_set_default_role_multiple_users() {
	let users = vec!["user1@localhost".to_string(), "user2@localhost".to_string()];
	let stmt = SetDefaultRoleStatement::new()
		.roles(DefaultRoleSpec::All)
		.users(users.clone());

	assert_eq!(stmt.user_names, users);
}

#[rstest]
fn test_set_default_role_variants() {
	let variants = vec![
		DefaultRoleSpec::RoleList(vec!["role1".to_string()]),
		DefaultRoleSpec::All,
		DefaultRoleSpec::None,
	];

	for variant in variants {
		let stmt = SetDefaultRoleStatement::new()
			.roles(variant.clone())
			.user("test_user");

		assert_eq!(stmt.role_spec, Some(variant));
	}
}

#[rstest]
fn test_clone_statement() {
	let stmt1 = SetDefaultRoleStatement::new()
		.roles(DefaultRoleSpec::All)
		.user("test_user");
	let stmt2 = stmt1.clone();

	assert_eq!(stmt1.role_spec, stmt2.role_spec);
	assert_eq!(stmt1.user_names, stmt2.user_names);
}

#[rstest]
fn test_builder_pattern() {
	let stmt = SetDefaultRoleStatement::new()
		.roles(DefaultRoleSpec::RoleList(vec![
			"role1".to_string(),
			"role2".to_string(),
		]))
		.users(vec![
			"user1@localhost".to_string(),
			"user2@localhost".to_string(),
		]);

	assert_eq!(stmt.user_names.len(), 2);
}

#[rstest]
fn test_comprehensive_statement() {
	let stmt = SetDefaultRoleStatement::new()
		.roles(DefaultRoleSpec::RoleList(vec!["app_role".to_string()]))
		.users(vec!["user1@localhost".to_string()]);

	assert_eq!(stmt.user_names.len(), 1);
}

// ============================================================================
// Error Path Tests
// ============================================================================

#[rstest]
fn test_no_role_spec() {
	let stmt = SetDefaultRoleStatement::new().user("test_user");

	let result = stmt.validate();
	assert!(result.is_err());
}

#[rstest]
fn test_no_users() {
	let stmt = SetDefaultRoleStatement::new().roles(DefaultRoleSpec::All);

	let result = stmt.validate();
	assert!(result.is_err());
}

#[rstest]
fn test_empty_role_list() {
	let stmt = SetDefaultRoleStatement::new()
		.roles(DefaultRoleSpec::RoleList(vec![]))
		.user("test_user");

	let result = stmt.validate();
	assert!(result.is_err());
}

#[rstest]
fn test_empty_users_list() {
	let stmt = SetDefaultRoleStatement::new()
		.roles(DefaultRoleSpec::All)
		.users(vec![]);

	let result = stmt.validate();
	assert!(result.is_err());
}

#[rstest]
fn test_empty_user_in_list() {
	let stmt = SetDefaultRoleStatement::new()
		.roles(DefaultRoleSpec::All)
		.users(vec!["user1@localhost".to_string(), "".to_string()]);

	let result = stmt.validate();
	assert!(result.is_err());
}

#[rstest]
fn test_whitespace_only_user() {
	let stmt = SetDefaultRoleStatement::new()
		.roles(DefaultRoleSpec::All)
		.users(vec!["   ".to_string()]);

	let result = stmt.validate();
	assert!(result.is_err());
}

#[rstest]
fn test_valid_role_list() {
	let stmt = SetDefaultRoleStatement::new()
		.roles(DefaultRoleSpec::RoleList(vec!["role1".to_string()]))
		.users(vec!["user@localhost".to_string()]);

	let result = stmt.validate();
	assert!(result.is_ok());
}

#[rstest]
fn test_valid_all() {
	let stmt = SetDefaultRoleStatement::new()
		.roles(DefaultRoleSpec::All)
		.users(vec!["user@localhost".to_string()]);

	let result = stmt.validate();
	assert!(result.is_ok());
}

#[rstest]
fn test_valid_none() {
	let stmt = SetDefaultRoleStatement::new()
		.roles(DefaultRoleSpec::None)
		.users(vec!["user@localhost".to_string()]);

	let result = stmt.validate();
	assert!(result.is_ok());
}

// ============================================================================
// Backend Panic Tests
// ============================================================================

#[rstest]
#[should_panic(expected = "SET DEFAULT ROLE is not supported by PostgreSQL")]
fn test_postgres_panics() {
	let builder = PostgresQueryBuilder::new();
	let stmt = SetDefaultRoleStatement::new()
		.roles(DefaultRoleSpec::All)
		.user("test_user");

	builder.build_set_default_role(&stmt);
}

#[rstest]
#[should_panic(expected = "SQLite does not support SET DEFAULT ROLE statement")]
fn test_sqlite_panics() {
	let builder = SqliteQueryBuilder::new();
	let stmt = SetDefaultRoleStatement::new()
		.roles(DefaultRoleSpec::All)
		.user("test_user");

	builder.build_set_default_role(&stmt);
}

#[rstest]
fn test_postgres_panic_message() {
	let builder = PostgresQueryBuilder::new();
	let stmt = SetDefaultRoleStatement::new()
		.roles(DefaultRoleSpec::All)
		.user("test_user");

	let result = std::panic::catch_unwind(|| {
		builder.build_set_default_role(&stmt);
	});

	assert!(result.is_err());
}

#[rstest]
fn test_sqlite_panic_message() {
	let builder = SqliteQueryBuilder::new();
	let stmt = SetDefaultRoleStatement::new()
		.roles(DefaultRoleSpec::All)
		.user("test_user");

	let result = std::panic::catch_unwind(|| {
		builder.build_set_default_role(&stmt);
	});

	assert!(result.is_err());
}

// ============================================================================
// SQL Generation Tests (MySQL)
// ============================================================================

#[rstest]
fn test_mysql_role_list() {
	let builder = MySqlQueryBuilder::new();
	let stmt = SetDefaultRoleStatement::new()
		.roles(DefaultRoleSpec::RoleList(vec![
			"role1".to_string(),
			"role2".to_string(),
		]))
		.user("test_user@localhost");

	let (sql, values) = builder.build_set_default_role(&stmt);

	assert!(sql.contains(r#"SET DEFAULT ROLE"#));
	assert!(sql.contains("`role1`, `role2`"));
	assert!(sql.contains(r#"TO 'test_user'@'localhost'"#));
	assert!(values.is_empty());
}

#[rstest]
fn test_mysql_role_all() {
	let builder = MySqlQueryBuilder::new();
	let stmt = SetDefaultRoleStatement::new()
		.roles(DefaultRoleSpec::All)
		.user("test_user@localhost");

	let (sql, values) = builder.build_set_default_role(&stmt);

	assert_eq!(sql, r#"SET DEFAULT ROLE ALL TO 'test_user'@'localhost'"#);
	assert!(values.is_empty());
}

#[rstest]
fn test_mysql_role_none() {
	let builder = MySqlQueryBuilder::new();
	let stmt = SetDefaultRoleStatement::new()
		.roles(DefaultRoleSpec::None)
		.user("test_user@localhost");

	let (sql, values) = builder.build_set_default_role(&stmt);

	assert_eq!(sql, r#"SET DEFAULT ROLE NONE TO 'test_user'@'localhost'"#);
	assert!(values.is_empty());
}

#[rstest]
fn test_mysql_multiple_users() {
	let builder = MySqlQueryBuilder::new();
	let stmt = SetDefaultRoleStatement::new()
		.roles(DefaultRoleSpec::All)
		.users(vec![
			"user1@localhost".to_string(),
			"user2@localhost".to_string(),
		]);

	let (sql, values) = builder.build_set_default_role(&stmt);

	assert!(sql.contains(r#"SET DEFAULT ROLE ALL"#));
	assert!(sql.contains(r#"TO 'user1'@'localhost', 'user2'@'localhost'"#));
	assert!(values.is_empty());
}
