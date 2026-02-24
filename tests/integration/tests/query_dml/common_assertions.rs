// Common assertion helpers for database testing
//
// This module provides reusable assertion macros and functions for verifying
// database state in integration tests.

use sqlx::{PgPool, Row};

// Assertion functions are provided for future use but currently unused.
// They enable consistent database state verification across test files.
#[allow(dead_code)]
/// Assert that a table has exactly the specified number of rows
///
/// # Arguments
///
/// * `pool` - Database connection pool
/// * `table_name` - Name of the table to check
/// * `expected_count` - Expected number of rows
///
/// # Panics
///
/// Panics if the row count doesn't match the expected value.
///
/// # Examples
///
/// ```rust,no_run
/// use crate::common::assert_row_count;
/// # use sqlx::PgPool;
///
/// # async fn test(pool: &PgPool) {
/// assert_row_count(pool, "users", 5).await;
/// # }
/// ```
pub(crate) async fn assert_row_count(pool: &PgPool, table_name: &str, expected_count: i64) {
	let sql = format!("SELECT COUNT(*) as count FROM {}", table_name);

	let result = sqlx::query(&sql)
		.fetch_one(pool)
		.await
		.expect("Failed to count rows");

	let actual_count: i64 = result.get("count");
	assert_eq!(
		actual_count, expected_count,
		"Table '{}' should have {} rows, but has {}",
		table_name, expected_count, actual_count
	);
}

// Assertion functions are provided for future use but currently unused.
// They enable consistent database state verification across test files.
#[allow(dead_code)]
/// Assert that a table is empty (contains no rows)
///
/// # Arguments
///
/// * `pool` - Database connection pool
/// * `table_name` - Name of the table to check
///
/// # Panics
///
/// Panics if the table contains any rows.
///
/// # Examples
///
/// ```rust,no_run
/// use crate::common::assert_table_empty;
/// # use sqlx::PgPool;
///
/// # async fn test(pool: &PgPool) {
/// assert_table_empty(pool, "users").await;
/// # }
/// ```
pub(crate) async fn assert_table_empty(pool: &PgPool, table_name: &str) {
	let sql = format!("SELECT COUNT(*) as count FROM {}", table_name);

	let result = sqlx::query(&sql)
		.fetch_one(pool)
		.await
		.expect("Failed to count rows");

	let count: i64 = result.get("count");
	assert_eq!(
		count, 0,
		"Table '{}' should be empty, but has {} rows",
		table_name, count
	);
}
