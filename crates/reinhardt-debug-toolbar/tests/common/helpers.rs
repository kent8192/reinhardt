//! Helper functions for reinhardt-debug-toolbar tests
//!
//! This module provides utility functions for populating test data and making assertions.

use axum::body::Body;
use axum::response::Response;
use chrono::Utc;
use http::header;
use reinhardt_debug_toolbar::context::{
	CacheOperation, MarkerCategory, PerformanceMarker, SqlQuery, TemplateInfo, ToolbarContext,
};
#[cfg(feature = "sql-panel")]
use reinhardt_debug_toolbar::utils::sql_normalization::normalize_sql;
use std::time::Duration;

/// Populate context with SQL queries
///
/// # Arguments
///
/// * `ctx` - The toolbar context to populate
/// * `count` - Number of queries to add
/// * `base_sql` - Base SQL pattern (will be indexed)
pub fn populate_sql_queries(ctx: &ToolbarContext, count: usize, base_sql: &str) {
	for i in 0..count {
		let query = SqlQuery {
			sql: format!("{} -- {}", base_sql, i),
			params: vec![],
			duration: Duration::from_millis(10),
			stack_trace: String::new(),
			timestamp: Utc::now(),
			connection: None,
		};
		ctx.record_sql_query(query);
	}
}

/// Populate context with duplicate SQL queries
///
/// Creates queries that normalize to the same pattern for testing duplicate detection.
///
/// # Arguments
///
/// * `ctx` - The toolbar context to populate
/// * `count` - Number of duplicate queries to add
/// * `sql_pattern` - SQL pattern to duplicate
pub fn create_duplicate_queries(ctx: &ToolbarContext, count: usize, sql_pattern: &str) {
	for i in 0..count {
		let query = SqlQuery {
			sql: sql_pattern.replace("?", &i.to_string()),
			params: vec![],
			duration: Duration::from_millis(10),
			stack_trace: String::new(),
			timestamp: Utc::now(),
			connection: None,
		};
		ctx.record_sql_query(query);
	}
}

/// Populate context with slow SQL queries
///
/// Creates queries that exceed the warning threshold.
///
/// # Arguments
///
/// * `ctx` - The toolbar context to populate
/// * `count` - Number of slow queries to add
/// * `threshold_ms` - Warning threshold in milliseconds
pub fn create_slow_queries(ctx: &ToolbarContext, count: usize, threshold_ms: u64) {
	for i in 0..count {
		let query = SqlQuery {
			sql: format!("SELECT * FROM slow_table_{}", i),
			params: vec![],
			duration: Duration::from_millis(threshold_ms + 50), // Exceed threshold
			stack_trace: String::new(),
			timestamp: Utc::now(),
			connection: None,
		};
		ctx.record_sql_query(query);
	}
}

/// Create N+1 query pattern in context
///
/// Creates a typical N+1 pattern: 1 parent query + N child queries.
///
/// # Arguments
///
/// * `ctx` - The toolbar context to populate
/// * `n_count` - Number of child queries (N in N+1)
pub fn create_n_plus_one_pattern(ctx: &ToolbarContext, n_count: usize) {
	// Parent query
	let parent_query = SqlQuery {
		sql: "SELECT * FROM users".to_string(),
		params: vec![],
		duration: Duration::from_millis(10),
		stack_trace: String::new(),
		timestamp: Utc::now(),
		connection: None,
	};
	ctx.record_sql_query(parent_query);

	// Child queries (N queries)
	for i in 0..n_count {
		let child_query = SqlQuery {
			sql: format!("SELECT * FROM posts WHERE user_id = {}", i),
			params: vec![],
			duration: Duration::from_millis(5),
			stack_trace: String::new(),
			timestamp: Utc::now(),
			connection: None,
		};
		ctx.record_sql_query(child_query);
	}
}

/// Populate context with templates
///
/// # Arguments
///
/// * `ctx` - The toolbar context to populate
/// * `count` - Number of templates to add
/// * `base_name` - Base template name (will be indexed)
pub fn populate_templates(ctx: &ToolbarContext, count: usize, base_name: &str) {
	for i in 0..count {
		let template = TemplateInfo {
			name: format!("{}_{}.html", base_name, i),
			render_duration: Duration::from_millis(5),
			context_data: serde_json::json!({"index": i}),
			parent: None,
			timestamp: Utc::now(),
		};
		ctx.record_template(template);
	}
}

/// Populate context with cache operations
///
/// # Arguments
///
/// * `ctx` - The toolbar context to populate
/// * `count` - Number of operations to add
pub fn populate_cache_ops(ctx: &ToolbarContext, count: usize) {
	for i in 0..count {
		let is_get = i % 2 == 0;
		let op = if is_get {
			CacheOperation::Get {
				key: format!("cache_key_{}", i),
				hit: i % 4 == 0, // 25% hit rate
				duration: Duration::from_millis(1),
				timestamp: Utc::now(),
			}
		} else {
			CacheOperation::Set {
				key: format!("cache_key_{}", i),
				duration: Duration::from_millis(2),
				timestamp: Utc::now(),
			}
		};
		ctx.record_cache_op(op);
	}
}

/// Populate context with performance markers
///
/// # Arguments
///
/// * `ctx` - The toolbar context to populate
/// * `count` - Number of markers to add
pub fn populate_markers(ctx: &ToolbarContext, count: usize) {
	let categories = vec![
		MarkerCategory::Middleware,
		MarkerCategory::Handler,
		MarkerCategory::Database,
		MarkerCategory::Cache,
		MarkerCategory::Template,
	];

	for i in 0..count {
		let marker = PerformanceMarker {
			name: format!("Marker_{}", i),
			start: Duration::from_millis(i as u64 * 10),
			end: Duration::from_millis((i as u64 + 1) * 10),
			category: categories[i % categories.len()].clone(),
		};
		ctx.add_marker(marker);
	}
}

/// Assert SQL normalization equality
///
/// Normalizes both SQL strings and asserts they are equal.
#[cfg(feature = "sql-panel")]
pub fn assert_sql_eq(sql1: &str, sql2: &str) {
	let normalized1 = normalize_sql(sql1);
	let normalized2 = normalize_sql(sql2);
	assert_eq!(
		normalized1, normalized2,
		"SQL normalization failed: '{}' != '{}'",
		normalized1, normalized2
	);
}

/// Assert HTML contains substring
///
/// Checks that HTML contains the expected substring.
pub fn assert_html_contains(html: &str, expected: &str) {
	assert!(
		html.contains(expected),
		"Expected HTML to contain '{}', but it did not.\nActual HTML:\n{}",
		expected,
		html
	);
}

/// Assert HTML does not contain substring
///
/// Checks that HTML does not contain the unexpected substring.
pub fn assert_html_not_contains(html: &str, unexpected: &str) {
	assert!(
		!html.contains(unexpected),
		"Expected HTML NOT to contain '{}', but it did.\nActual HTML:\n{}",
		unexpected,
		html
	);
}

/// Assert PanelStats has a specific field
///
/// Checks that PanelStats data contains the specified field.
pub fn assert_stats_has_field(stats: &reinhardt_debug_toolbar::panels::PanelStats, field: &str) {
	let data = &stats.data;
	if let Some(obj) = data.as_object() {
		assert!(
			obj.contains_key(field),
			"Expected PanelStats to have field '{}', but it did not.\nAvailable fields: {:?}",
			field,
			obj.keys()
		);
	} else {
		panic!("PanelStats data is not an object");
	}
}

/// Create an HTML response with the given body
pub fn html_response(body: &str) -> Response<Body> {
	Response::builder()
		.header(header::CONTENT_TYPE, "text/html; charset=utf-8")
		.body(Body::from(body.to_string()))
		.unwrap()
}

/// Create a JSON response with the given body
pub fn json_response(body: &str) -> Response<Body> {
	Response::builder()
		.header(header::CONTENT_TYPE, "application/json")
		.body(Body::from(body.to_string()))
		.unwrap()
}

/// Create a response with the given content type and body
pub fn response_with_content_type(content_type: &str, body: &str) -> Response<Body> {
	Response::builder()
		.header(header::CONTENT_TYPE, content_type)
		.body(Body::from(body.to_string()))
		.unwrap()
}

/// Verify HTML is properly escaped
///
/// Checks that special characters are properly HTML-escaped.
pub fn verify_html_escaped(input: &str, escaped: &str) {
	assert!(
		!escaped.contains('<') || escaped.contains("&lt;"),
		"HTML not properly escaped: '<' found in '{}'",
		escaped
	);
	assert!(
		!escaped.contains('>') || escaped.contains("&gt;"),
		"HTML not properly escaped: '>' found in '{}'",
		escaped
	);
	assert!(
		!escaped.contains('&')
			|| escaped.contains("&amp;")
			|| escaped.contains("&lt;")
			|| escaped.contains("&gt;")
			|| escaped.contains("&quot;")
			|| escaped.contains("&#x27;"),
		"HTML not properly escaped: bare '&' found in '{}'",
		escaped
	);

	// Verify that the original input's special characters are escaped
	if input.contains('<') {
		assert!(escaped.contains("&lt;"), "Expected '&lt;' in escaped HTML");
	}
	if input.contains('>') {
		assert!(escaped.contains("&gt;"), "Expected '&gt;' in escaped HTML");
	}
	if input.contains('"') {
		assert!(
			escaped.contains("&quot;"),
			"Expected '&quot;' in escaped HTML"
		);
	}
	if input.contains('\'') {
		assert!(
			escaped.contains("&#x27;"),
			"Expected '&#x27;' in escaped HTML"
		);
	}
}
