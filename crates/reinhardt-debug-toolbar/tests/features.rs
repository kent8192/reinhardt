//! Feature-gated tests for reinhardt-debug-toolbar
//!
//! Tests for SQL panel functionality, SQL normalization, and N+1 detection.
//! All tests require the `sql-panel` feature.

#![cfg(feature = "sql-panel")]

use crate::common::{
	builders::SqlQueryBuilder,
	fixtures::{test_context, test_request_info},
	helpers::{assert_html_contains, assert_html_not_contains, populate_sql_queries},
};
use reinhardt_debug_toolbar::{
	context::{SqlQuery, ToolbarContext},
	panels::{Panel, sql::SqlPanel},
	utils::sql_normalization::{detect_n_plus_one, normalize_sql},
};
use rstest::*;
use std::time::Duration;

// ============================================================================
// 1. Happy Path Tests
// ============================================================================

#[rstest]
#[tokio::test]
async fn sql_panel_generate_stats_with_zero_queries(test_context: ToolbarContext) {
	// Arrange
	let panel = SqlPanel::new();

	// Act
	let stats = panel.generate_stats(&test_context).await.unwrap();

	// Assert
	assert_eq!(stats.data["total_queries"].as_u64().unwrap(), 0);
	assert_eq!(stats.data["total_time_ms"].as_u64().unwrap(), 0);
	assert_eq!(stats.summary, "0 queries in 0ms");
}

#[rstest]
#[tokio::test]
async fn sql_panel_render_with_zero_queries(test_context: ToolbarContext) {
	// Arrange
	let panel = SqlPanel::new();

	// Act
	let stats = panel.generate_stats(&test_context).await.unwrap();
	let html = panel.render(&stats).unwrap();

	// Assert
	assert_html_contains(&html, "SQL Queries");
	assert_html_contains(&html, "Total Queries:</strong> 0");
	assert_html_not_contains(&html, "djdt-warning");
}

#[rstest]
#[tokio::test]
async fn sql_panel_with_custom_threshold_identifies_slow_queries(test_context: ToolbarContext) {
	// Arrange
	let panel = SqlPanel::with_threshold(50);

	// Add queries: one at 40ms (normal), one at 60ms (slow)
	let normal = SqlQueryBuilder::new()
		.sql("SELECT * FROM fast_table")
		.duration(Duration::from_millis(40))
		.build();
	let slow = SqlQueryBuilder::new()
		.sql("SELECT * FROM slow_table")
		.duration(Duration::from_millis(60))
		.build();
	test_context.record_sql_query(normal);
	test_context.record_sql_query(slow);

	// Act
	let stats = panel.generate_stats(&test_context).await.unwrap();

	// Assert
	assert_eq!(stats.data["slow_queries_count"].as_u64().unwrap(), 1);
	assert_eq!(stats.data["warning_threshold_ms"].as_u64().unwrap(), 50);

	let queries = stats.data["queries"].as_array().unwrap();
	let fast_q = &queries[0];
	let slow_q = &queries[1];
	assert!(!fast_q["is_slow"].as_bool().unwrap());
	assert!(slow_q["is_slow"].as_bool().unwrap());
}

#[rstest]
#[tokio::test]
async fn sql_panel_render_shows_badges_for_issues(test_context: ToolbarContext) {
	// Arrange
	let panel = SqlPanel::with_threshold(50);

	// Add duplicate queries (>1 of same normalized pattern)
	for i in 0..2 {
		let q = SqlQueryBuilder::new()
			.sql(format!("SELECT * FROM users WHERE id = {}", i))
			.duration(Duration::from_millis(10))
			.build();
		test_context.record_sql_query(q);
	}

	// Add slow query
	let slow = SqlQueryBuilder::new()
		.sql("SELECT * FROM big_table WHERE name = 'test'")
		.duration(Duration::from_millis(200))
		.build();
	test_context.record_sql_query(slow);

	// Add N+1 pattern (>3 of the same normalized query)
	for i in 0..5 {
		let q = SqlQueryBuilder::new()
			.sql(format!("SELECT * FROM posts WHERE user_id = {}", i))
			.duration(Duration::from_millis(5))
			.build();
		test_context.record_sql_query(q);
	}

	// Act
	let stats = panel.generate_stats(&test_context).await.unwrap();
	let html = panel.render(&stats).unwrap();

	// Assert
	assert_html_contains(&html, "DUPLICATE");
	assert_html_contains(&html, "SLOW");
	assert_html_contains(&html, "N+1");
}

// ============================================================================
// 2. Edge Cases Tests
// ============================================================================

#[rstest]
fn normalize_sql_empty_string() {
	// Arrange / Act
	let result = normalize_sql("");

	// Assert
	assert_eq!(result, "");
}

#[rstest]
fn normalize_sql_only_whitespace() {
	// Arrange / Act
	let result = normalize_sql("   \t\n  ");

	// Assert
	assert_eq!(result, "");
}

#[rstest]
fn normalize_sql_only_comments() {
	// Arrange / Act
	let result = normalize_sql("-- this is a comment\n-- another comment");

	// Assert
	assert_eq!(result, "");
}

#[rstest]
fn detect_n_plus_one_empty_queries() {
	// Arrange
	let queries: Vec<SqlQuery> = vec![];

	// Act
	let patterns = detect_n_plus_one(&queries);

	// Assert
	assert!(patterns.is_empty());
}

#[rstest]
#[tokio::test]
async fn sql_panel_render_zero_queries_average_no_div_by_zero(test_context: ToolbarContext) {
	// Arrange
	let panel = SqlPanel::new();

	// Act
	let stats = panel.generate_stats(&test_context).await.unwrap();
	let html = panel.render(&stats).unwrap();

	// Assert - should show 0ms average without panic
	assert_html_contains(&html, "Average Time:</strong> 0ms");
}

#[rstest]
fn normalize_sql_with_escaped_quotes_in_strings() {
	// Arrange
	let sql = r"SELECT * FROM users WHERE name = 'O\'Brien'";

	// Act
	let normalized = normalize_sql(sql);

	// Assert - escaped quotes should be handled; string replaced with ?
	assert_html_contains(&normalized, "SELECT * FROM USERS WHERE NAME = ?");
}

// ============================================================================
// 3. State Transitions Tests
// ============================================================================

#[rstest]
#[tokio::test]
async fn sql_panel_stats_reflect_growing_query_count(test_context: ToolbarContext) {
	// Arrange
	let panel = SqlPanel::new();

	// Act / Assert - 0 queries
	let stats = panel.generate_stats(&test_context).await.unwrap();
	assert_eq!(stats.data["total_queries"].as_u64().unwrap(), 0);

	// Add 5 queries
	populate_sql_queries(&test_context, 5, "SELECT 1");
	let stats = panel.generate_stats(&test_context).await.unwrap();
	assert_eq!(stats.data["total_queries"].as_u64().unwrap(), 5);
}

// ============================================================================
// 4. Fuzz Tests
// ============================================================================

#[rstest]
fn normalize_sql_with_varied_inputs() {
	// Arrange
	let inputs = [
		"",
		"   ",
		"SELECT",
		"SELECT * FROM \u{1f4a9}_table WHERE id = 42",
		&"SELECT ".repeat(1000),
		"SELECT ''; DROP TABLE users; --",
		"SELECT /* inline comment */ * FROM t",
		"SELECT * FROM t WHERE x IN (1,2,3,4,5,6,7,8,9,10)",
	];

	// Act / Assert - no panics
	for input in &inputs {
		let _result = normalize_sql(input);
	}
}

#[rstest]
fn detect_n_plus_one_with_varied_query_counts() {
	// Arrange / Act / Assert
	for count in [0, 1, 3, 4, 10, 100, 1000] {
		let queries: Vec<SqlQuery> = (0..count)
			.map(|i| {
				SqlQueryBuilder::new()
					.sql(format!("SELECT * FROM t WHERE id = {}", i))
					.duration(Duration::from_millis(1))
					.build()
			})
			.collect();

		let patterns = detect_n_plus_one(&queries);

		// For count <= 3, no N+1 should be detected
		if count <= 3 {
			assert!(
				patterns.is_empty(),
				"Expected no N+1 for count={}, got {}",
				count,
				patterns.len()
			);
		}
		// For count > 3, should detect N+1
		if count > 3 {
			assert_eq!(
				patterns.len(),
				1,
				"Expected 1 N+1 pattern for count={}, got {}",
				count,
				patterns.len()
			);
		}
	}
}

// ============================================================================
// 5. Property-Based Tests
// ============================================================================

#[rstest]
fn normalize_sql_idempotency() {
	// Arrange
	let sqls = [
		"SELECT * FROM users WHERE id = 123",
		"INSERT INTO orders (name) VALUES ('test')",
		"UPDATE t SET col = 42 WHERE id = 1",
		"  SELECT  *  FROM  t  ",
	];

	// Act / Assert - normalize(normalize(x)) == normalize(x)
	for sql in &sqls {
		let once = normalize_sql(sql);
		let twice = normalize_sql(&once);
		assert_eq!(once, twice, "Idempotency failed for: {}", sql);
	}
}

#[rstest]
fn normalize_sql_equivalence_for_differing_literals() {
	// Arrange
	let pairs = [
		(
			"SELECT * FROM users WHERE id = 1",
			"SELECT * FROM users WHERE id = 9999",
		),
		(
			"SELECT * FROM users WHERE name = 'Alice'",
			"SELECT * FROM users WHERE name = 'Bob'",
		),
		(
			"INSERT INTO t VALUES (1, 'a', 2)",
			"INSERT INTO t VALUES (99, 'zzz', 100)",
		),
	];

	// Act / Assert - queries differing only in literals normalize equally
	for (sql1, sql2) in &pairs {
		assert_eq!(
			normalize_sql(sql1),
			normalize_sql(sql2),
			"Equivalence failed: '{}' vs '{}'",
			sql1,
			sql2
		);
	}
}

// ============================================================================
// 6. Equivalence Partitioning Tests
// ============================================================================

#[rstest]
#[case(
	"SELECT * FROM users WHERE id = 42",
	"SELECT * FROM USERS WHERE ID = ?"
)]
#[case(
	"SELECT * FROM users WHERE name = 'Alice'",
	"SELECT * FROM USERS WHERE NAME = ?"
)]
#[case(
	"SELECT * FROM items WHERE price = 19.99",
	"SELECT * FROM ITEMS WHERE PRICE = ?.?"
)]
#[case(
	"SELECT * FROM t WHERE a = 1 AND b = 2 AND c = 3",
	"SELECT * FROM T WHERE A = ? AND B = ? AND C = ?"
)]
fn normalize_sql_literal_type_partitions(#[case] input: &str, #[case] expected: &str) {
	// Arrange / Act
	let result = normalize_sql(input);

	// Assert
	assert_eq!(result, expected);
}

// ============================================================================
// 7. Boundary Value Analysis Tests
// ============================================================================

#[rstest]
#[case(1, 0)]
#[case(2, 0)]
#[case(3, 0)]
#[case(4, 1)]
#[case(5, 1)]
#[case(10, 1)]
fn detect_n_plus_one_threshold_boundary(
	#[case] query_count: usize,
	#[case] expected_patterns: usize,
) {
	// Arrange
	let queries: Vec<SqlQuery> = (0..query_count)
		.map(|i| {
			SqlQueryBuilder::new()
				.sql(format!("SELECT * FROM posts WHERE user_id = {}", i))
				.duration(Duration::from_millis(5))
				.build()
		})
		.collect();

	// Act
	let patterns = detect_n_plus_one(&queries);

	// Assert
	assert_eq!(
		patterns.len(),
		expected_patterns,
		"query_count={} expected {} patterns, got {}",
		query_count,
		expected_patterns,
		patterns.len()
	);
}

#[rstest]
#[case(99, 0)]
#[case(100, 1)]
#[case(101, 1)]
#[tokio::test]
async fn sql_panel_slow_query_threshold_boundary(
	#[case] duration_ms: u64,
	#[case] expected_slow: u64,
) {
	// Arrange
	let ctx = ToolbarContext::new(test_request_info());
	let panel = SqlPanel::with_threshold(100);

	let query = SqlQueryBuilder::new()
		.sql("SELECT * FROM boundary_test")
		.duration(Duration::from_millis(duration_ms))
		.build();
	ctx.record_sql_query(query);

	// Act
	let stats = panel.generate_stats(&ctx).await.unwrap();

	// Assert
	assert_eq!(
		stats.data["slow_queries_count"].as_u64().unwrap(),
		expected_slow,
		"duration={}ms expected {} slow queries",
		duration_ms,
		expected_slow
	);
}

// ============================================================================
// 8. Decision Table Tests
// ============================================================================

#[rstest]
#[tokio::test]
async fn sql_panel_query_classification_decision_table(test_context: ToolbarContext) {
	// Arrange
	let panel = SqlPanel::with_threshold(100);

	// Normal query (not dup, not slow, not N+1)
	let normal = SqlQueryBuilder::new()
		.sql("SELECT DISTINCT thing FROM unique_table")
		.duration(Duration::from_millis(10))
		.build();
	test_context.record_sql_query(normal);

	// Duplicate + slow query
	for i in 0..2 {
		let q = SqlQueryBuilder::new()
			.sql(format!("SELECT * FROM dup_table WHERE id = {}", i))
			.duration(Duration::from_millis(150))
			.build();
		test_context.record_sql_query(q);
	}

	// N+1 pattern (>3 similar queries, not slow)
	for i in 0..5 {
		let q = SqlQueryBuilder::new()
			.sql(format!("SELECT * FROM child_table WHERE parent_id = {}", i))
			.duration(Duration::from_millis(5))
			.build();
		test_context.record_sql_query(q);
	}

	// Act
	let stats = panel.generate_stats(&test_context).await.unwrap();
	let html = panel.render(&stats).unwrap();

	// Assert
	let queries = stats.data["queries"].as_array().unwrap();

	// First query: normal (not dup, not slow, not N+1)
	assert!(!queries[0]["is_duplicate"].as_bool().unwrap());
	assert!(!queries[0]["is_slow"].as_bool().unwrap());
	assert!(!queries[0]["is_n_plus_one"].as_bool().unwrap());

	// Duplicate queries should be marked as duplicate and slow
	assert!(queries[1]["is_duplicate"].as_bool().unwrap());
	assert!(queries[1]["is_slow"].as_bool().unwrap());

	// N+1 queries should be marked
	assert!(queries[3]["is_n_plus_one"].as_bool().unwrap());

	// HTML has all badge types
	assert_html_contains(&html, "DUPLICATE");
	assert_html_contains(&html, "SLOW");
	assert_html_contains(&html, "N+1");
}
