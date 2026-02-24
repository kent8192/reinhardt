//! # RenameUserStatement Unit Tests
//!
//! Comprehensive unit tests for RenameUserStatement covering:
//! - Happy Path: Normal operations
//! - Error Path: Validation and error handling
//! - Edge Cases: Boundary values and special cases
//! - Backend Panic Tests: Unsupported backend verification
//!
//! ## Test Coverage
//!
//! - Statements tested: RenameUserStatement
//! - Code coverage target: 95%
//! - Total tests: ~25

use crate::backend::{MySqlQueryBuilder, PostgresQueryBuilder, QueryBuilder, SqliteQueryBuilder};
use crate::dcl::RenameUserStatement;
use rstest::rstest;

// ============================================================================
// Happy Path Tests
// ============================================================================

#[rstest]
fn test_rename_user_new() {
	let stmt = RenameUserStatement::new();

	assert!(stmt.renames.is_empty());
}

#[rstest]
fn test_rename_single_user() {
	let stmt = RenameUserStatement::new().rename("old_user@localhost", "new_user@localhost");

	assert_eq!(stmt.renames.len(), 1);
	assert_eq!(stmt.renames[0].0, "old_user@localhost");
	assert_eq!(stmt.renames[0].1, "new_user@localhost");
}

#[rstest]
fn test_rename_multiple_users() {
	let stmt = RenameUserStatement::new()
		.rename("user1@localhost", "new1@localhost")
		.rename("user2@localhost", "new2@localhost")
		.rename("user3@localhost", "new3@localhost");

	assert_eq!(stmt.renames.len(), 3);
}

#[rstest]
fn test_rename_to_same_name() {
	let stmt = RenameUserStatement::new().rename("test_user@localhost", "test_user@localhost");

	assert_eq!(stmt.renames.len(), 1);
	assert_eq!(stmt.renames[0].0, stmt.renames[0].1);
}

#[rstest]
fn test_clone_statement() {
	let stmt1 = RenameUserStatement::new().rename("old", "new");
	let stmt2 = stmt1.clone();

	assert_eq!(stmt1.renames, stmt2.renames);
}

#[rstest]
fn test_builder_pattern() {
	let stmt = RenameUserStatement::new()
		.rename("user1@host1", "new1@host1")
		.rename("user2@host2", "new2@host2")
		.rename("user3@host3", "new3@host3");

	assert_eq!(stmt.renames.len(), 3);
}

#[rstest]
fn test_various_hosts() {
	let stmt = RenameUserStatement::new()
		.rename("user1@localhost", "new1@localhost")
		.rename("user2@'192.168.1.1'", "new2@'192.168.1.1'")
		.rename("user3@'example.com'", "new3@'example.com'");

	assert_eq!(stmt.renames.len(), 3);
}

#[rstest]
fn test_comprehensive_rename() {
	let stmt = RenameUserStatement::new()
		.rename("old_user1@localhost", "new_user1@localhost")
		.rename("old_user2@%", "new_user2@%")
		.rename("old_user3@'10.0.0.1'", "new_user3@'10.0.0.1'");

	assert_eq!(stmt.renames.len(), 3);
}

// ============================================================================
// Error Path Tests
// ============================================================================

#[rstest]
fn test_empty_rename_list_validation() {
	let stmt = RenameUserStatement::new();

	let result = stmt.validate();
	assert!(result.is_err());
}

#[rstest]
fn test_empty_old_name() {
	let stmt = RenameUserStatement::new().rename("", "new_user@localhost");

	let result = stmt.validate();
	assert!(result.is_err());
}

#[rstest]
fn test_empty_new_name() {
	let stmt = RenameUserStatement::new().rename("old_user@localhost", "");

	let result = stmt.validate();
	assert!(result.is_err());
}

#[rstest]
fn test_whitespace_only_old_name() {
	let stmt = RenameUserStatement::new().rename("   ", "new_user@localhost");

	let result = stmt.validate();
	assert!(result.is_err());
}

#[rstest]
fn test_whitespace_only_new_name() {
	let stmt = RenameUserStatement::new().rename("old_user@localhost", "   ");

	let result = stmt.validate();
	assert!(result.is_err());
}

#[rstest]
fn test_mixed_empty_and_valid() {
	let stmt = RenameUserStatement::new()
		.rename("user1@localhost", "new1@localhost")
		.rename("", "new2@localhost");

	let result = stmt.validate();
	assert!(result.is_err());
}

#[rstest]
fn test_valid_rename_validation() {
	let stmt = RenameUserStatement::new().rename("old_user@localhost", "new_user@localhost");

	let result = stmt.validate();
	assert!(result.is_ok());
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[rstest]
#[case(1, "single char")]
#[case(63, "boundary before limit")]
#[case(64, "boundary at limit")]
#[case(255, "long name")]
fn test_name_length(#[case] length: usize, #[case] _desc: &str) {
	let old_name = "a".repeat(length);
	let new_name = "b".repeat(length);
	let stmt = RenameUserStatement::new().rename(old_name.clone(), new_name.clone());

	assert_eq!(stmt.renames[0].0, old_name);
	assert_eq!(stmt.renames[0].1, new_name);
}

#[rstest]
fn test_very_long_name() {
	let name = "a".repeat(256);
	let stmt = RenameUserStatement::new().rename(name.clone(), name);

	assert_eq!(stmt.renames[0].0.len(), 256);
}

#[rstest]
fn test_unicode_names() {
	let stmt = RenameUserStatement::new().rename("ユーザー1@localhost", "ユーザー2@localhost");

	assert_eq!(stmt.renames.len(), 1);
}

#[rstest]
fn test_special_characters() {
	let stmt =
		RenameUserStatement::new().rename("test-user_123@localhost", "test-user-456@localhost");

	assert_eq!(stmt.renames.len(), 1);
}

#[rstest]
fn test_many_renames() {
	let mut stmt = RenameUserStatement::new();
	for i in 0..100 {
		stmt = stmt.rename(
			format!("user{}@localhost", i),
			format!("new{}@localhost", i),
		);
	}

	assert_eq!(stmt.renames.len(), 100);
}

// ============================================================================
// Backend Panic Tests
// ============================================================================

#[rstest]
#[should_panic(expected = "RENAME USER is not supported by PostgreSQL")]
fn test_postgres_panics() {
	let builder = PostgresQueryBuilder::new();
	let stmt = RenameUserStatement::new().rename("old_user@localhost", "new_user@localhost");

	builder.build_rename_user(&stmt);
}

#[rstest]
#[should_panic(expected = "SQLite does not support RENAME USER")]
fn test_sqlite_panics() {
	let builder = SqliteQueryBuilder::new();
	let stmt = RenameUserStatement::new().rename("old_user@localhost", "new_user@localhost");

	builder.build_rename_user(&stmt);
}

#[rstest]
fn test_postgres_panic_message() {
	let builder = PostgresQueryBuilder::new();
	let stmt = RenameUserStatement::new().rename("old", "new");

	let result = std::panic::catch_unwind(|| {
		builder.build_rename_user(&stmt);
	});

	assert!(result.is_err());
}

// ============================================================================
// SQL Generation Tests (MySQL)
// ============================================================================

#[rstest]
fn test_mysql_rename_single_user() {
	let builder = MySqlQueryBuilder::new();
	let stmt = RenameUserStatement::new().rename("old_user@localhost", "new_user@localhost");

	let (sql, values) = builder.build_rename_user(&stmt);

	assert_eq!(
		sql,
		r#"RENAME USER 'old_user'@'localhost' TO 'new_user'@'localhost'"#
	);
	assert!(values.is_empty());
}

#[rstest]
fn test_mysql_rename_multiple_users() {
	let builder = MySqlQueryBuilder::new();
	let stmt = RenameUserStatement::new()
		.rename("user1@localhost", "new1@localhost")
		.rename("user2@localhost", "new2@localhost")
		.rename("user3@localhost", "new3@localhost");

	let (sql, values) = builder.build_rename_user(&stmt);

	assert!(sql.contains(r#"RENAME USER"#));
	assert!(sql.contains("'user1'@'localhost' TO 'new1'@'localhost'"));
	assert!(sql.contains("'user2'@'localhost' TO 'new2'@'localhost'"));
	assert!(sql.contains("'user3'@'localhost' TO 'new3'@'localhost'"));
	assert!(values.is_empty());
}

#[rstest]
fn test_mysql_rename_with_ip_host() {
	let builder = MySqlQueryBuilder::new();
	let stmt = RenameUserStatement::new().rename("user@'192.168.1.1'", "new_user@'192.168.1.1'");

	let (sql, values) = builder.build_rename_user(&stmt);

	assert!(sql.contains(r#"RENAME USER"#));
	assert!(sql.contains("'user'@'192.168.1.1' TO 'new_user'@'192.168.1.1'"));
	assert!(values.is_empty());
}
