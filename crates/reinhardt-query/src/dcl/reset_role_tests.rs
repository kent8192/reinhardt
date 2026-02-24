//! # ResetRoleStatement Unit Tests
//!
//! Comprehensive unit tests for ResetRoleStatement covering:
//! - Happy Path: Normal operations
//! - Backend Panic Tests: Unsupported backend verification
//!
//! ## Test Coverage
//!
//! - Statements tested: ResetRoleStatement
//! - Code coverage target: 90%
//! - Total tests: ~10

use crate::backend::{MySqlQueryBuilder, PostgresQueryBuilder, QueryBuilder, SqliteQueryBuilder};
use crate::dcl::ResetRoleStatement;
use rstest::rstest;

// ============================================================================
// Happy Path Tests
// ============================================================================

#[rstest]
fn test_reset_role_new() {
	let stmt = ResetRoleStatement::new();

	// ResetRoleStatement has no fields
	let _ = stmt;
}

#[rstest]
fn test_reset_role_clone() {
	let stmt1 = ResetRoleStatement::new();
	let stmt2 = stmt1.clone();

	// Both should be equal (empty structs)
	let _ = (stmt1, stmt2);
}

#[rstest]
fn test_reset_role_validation() {
	let stmt = ResetRoleStatement::new();

	let result = stmt.validate();
	assert!(result.is_ok());
}

#[rstest]
fn test_reset_role_debug() {
	let stmt = ResetRoleStatement::new();
	let debug_str = format!("{:?}", stmt);

	assert!(debug_str.contains("ResetRoleStatement"));
}

// ============================================================================
// Backend Panic Tests
// ============================================================================

#[rstest]
#[should_panic(expected = "RESET ROLE is not supported by MySQL")]
fn test_mysql_panics() {
	let builder = MySqlQueryBuilder::new();
	let stmt = ResetRoleStatement::new();

	builder.build_reset_role(&stmt);
}

#[rstest]
#[should_panic(expected = "SQLite does not support RESET ROLE statement")]
fn test_sqlite_panics() {
	let builder = SqliteQueryBuilder::new();
	let stmt = ResetRoleStatement::new();

	builder.build_reset_role(&stmt);
}

#[rstest]
fn test_mysql_panic_message() {
	let builder = MySqlQueryBuilder::new();
	let stmt = ResetRoleStatement::new();

	let result = std::panic::catch_unwind(|| {
		builder.build_reset_role(&stmt);
	});

	assert!(result.is_err());
}

#[rstest]
fn test_sqlite_panic_message() {
	let builder = SqliteQueryBuilder::new();
	let stmt = ResetRoleStatement::new();

	let result = std::panic::catch_unwind(|| {
		builder.build_reset_role(&stmt);
	});

	assert!(result.is_err());
}

// ============================================================================
// SQL Generation Tests (PostgreSQL)
// ============================================================================

#[rstest]
fn test_postgres_reset_role() {
	let builder = PostgresQueryBuilder::new();
	let stmt = ResetRoleStatement::new();

	let (sql, values) = builder.build_reset_role(&stmt);

	assert_eq!(sql, "RESET ROLE");
	assert!(values.is_empty());
}

#[rstest]
fn test_postgres_reset_role_multiple_calls() {
	let builder = PostgresQueryBuilder::new();
	let stmt1 = ResetRoleStatement::new();
	let stmt2 = ResetRoleStatement::new();

	let (sql1, values1) = builder.build_reset_role(&stmt1);
	let (sql2, values2) = builder.build_reset_role(&stmt2);

	assert_eq!(sql1, "RESET ROLE");
	assert_eq!(sql2, "RESET ROLE");
	assert!(values1.is_empty());
	assert!(values2.is_empty());
}
