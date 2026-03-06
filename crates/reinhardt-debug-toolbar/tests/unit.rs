//! Unit tests for reinhardt-debug-toolbar
//!
//! Comprehensive unit tests covering context, panels, registry, config, errors,
//! sanitization, and toolbar rendering.

use crate::common::{
	builders::{
		CacheOperationBuilder, PerformanceMarkerBuilder, SqlQueryBuilder, TemplateInfoBuilder,
	},
	fixtures::{empty_registry, test_context, test_request_info},
	helpers::{
		assert_html_contains, assert_html_not_contains, assert_stats_has_field, populate_cache_ops,
		populate_markers, populate_sql_queries, populate_templates,
	},
	mock_panel::MockPanel,
};
use chrono::Utc;
use reinhardt_debug_toolbar::{
	DebugToolbarLayer,
	context::{MarkerCategory, RequestInfo, ToolbarContext},
	error::ToolbarError,
	middleware::ToolbarConfig,
	panels::{Panel, PanelStats, registry::PanelRegistry, request::RequestPanel},
	ui::render_toolbar,
	utils::sanitization::sanitize_headers,
};
use rstest::*;
use std::net::IpAddr;
use std::time::Duration;
use tower::Layer;

// ============================================================================
// 1. Happy Path Tests
// ============================================================================

#[rstest]
fn context_record_template_and_retrieve(test_context: ToolbarContext) {
	// Arrange
	let template = TemplateInfoBuilder::new()
		.name("index.html")
		.render_duration(Duration::from_millis(15))
		.context_data(serde_json::json!({"title": "Home"}))
		.build();

	// Act
	test_context.record_template(template);
	let templates = test_context.get_templates();

	// Assert
	assert_eq!(templates.len(), 1);
	assert_eq!(templates[0].name, "index.html");
	assert_eq!(templates[0].render_duration, Duration::from_millis(15));
}

#[rstest]
fn context_record_cache_op_and_retrieve(test_context: ToolbarContext) {
	// Arrange
	let get_op = CacheOperationBuilder::get()
		.key("user:1")
		.hit(true)
		.duration(Duration::from_millis(1))
		.build();
	let set_op = CacheOperationBuilder::set()
		.key("user:2")
		.duration(Duration::from_millis(2))
		.build();
	let del_op = CacheOperationBuilder::delete()
		.key("user:3")
		.duration(Duration::from_millis(1))
		.build();

	// Act
	test_context.record_cache_op(get_op);
	test_context.record_cache_op(set_op);
	test_context.record_cache_op(del_op);
	let ops = test_context.get_cache_ops();

	// Assert
	assert_eq!(ops.len(), 3);
}

#[rstest]
fn context_add_marker_and_retrieve(test_context: ToolbarContext) {
	// Arrange
	let marker = PerformanceMarkerBuilder::new()
		.name("AuthMiddleware")
		.category(MarkerCategory::Middleware)
		.start(Duration::from_millis(0))
		.end(Duration::from_millis(5))
		.build();

	// Act
	test_context.add_marker(marker);
	let markers = test_context.get_performance_markers();

	// Assert
	assert_eq!(markers.len(), 1);
	assert_eq!(markers[0].name, "AuthMiddleware");
	assert_eq!(markers[0].start, Duration::from_millis(0));
	assert_eq!(markers[0].end, Duration::from_millis(5));
}

#[rstest]
#[tokio::test]
async fn context_elapsed_returns_positive_duration(test_context: ToolbarContext) {
	// Arrange
	tokio::time::sleep(Duration::from_millis(5)).await;

	// Act
	let elapsed = test_context.elapsed();

	// Assert
	assert!(elapsed >= Duration::from_millis(5));
}

#[rstest]
#[tokio::test]
async fn request_panel_render_produces_valid_html_structure(test_context: ToolbarContext) {
	// Arrange
	let panel = RequestPanel::new();

	// Act
	let stats = panel.generate_stats(&test_context).await.unwrap();
	let html = panel.render(&stats).unwrap();

	// Assert
	assert_html_contains(&html, "djdt-panel-content");
	assert_html_contains(&html, "Request Information");
	assert_html_contains(&html, "djdt-table");
}

#[rstest]
#[tokio::test]
async fn request_panel_summary_format() {
	// Arrange
	let request_info = RequestInfo {
		method: "POST".to_string(),
		path: "/api/users".to_string(),
		query: None,
		headers: vec![],
		client_ip: "127.0.0.1".to_string(),
		timestamp: Utc::now(),
	};
	let ctx = ToolbarContext::new(request_info);
	let panel = RequestPanel::new();

	// Act
	let stats = panel.generate_stats(&ctx).await.unwrap();

	// Assert
	assert_eq!(stats.summary, "POST /api/users");
}

#[rstest]
fn render_toolbar_with_single_panel() {
	// Arrange
	let stats = vec![PanelStats {
		panel_id: "request".to_string(),
		panel_name: "Request".to_string(),
		data: serde_json::json!({}),
		summary: "GET /test".to_string(),
		rendered_html: Some("<div>Request Info</div>".to_string()),
	}];

	// Act
	let html = render_toolbar(&stats).unwrap();

	// Assert
	assert_html_contains(&html, "djDebug");
	assert_html_contains(&html, "djdt-handle");
	assert_html_contains(&html, "djdt-panel-content");
	assert_html_contains(&html, "<style>");
	assert_html_contains(&html, "<script>");
}

#[rstest]
fn render_toolbar_with_multiple_panels() {
	// Arrange
	let stats = vec![
		PanelStats {
			panel_id: "request".to_string(),
			panel_name: "Request".to_string(),
			data: serde_json::json!({}),
			summary: "GET /".to_string(),
			rendered_html: Some("<div>Request</div>".to_string()),
		},
		PanelStats {
			panel_id: "sql".to_string(),
			panel_name: "SQL".to_string(),
			data: serde_json::json!({}),
			summary: "5 queries".to_string(),
			rendered_html: Some("<div>SQL</div>".to_string()),
		},
		PanelStats {
			panel_id: "cache".to_string(),
			panel_name: "Cache".to_string(),
			data: serde_json::json!({}),
			summary: "10 ops".to_string(),
			rendered_html: Some("<div>Cache</div>".to_string()),
		},
	];

	// Act
	let html = render_toolbar(&stats).unwrap();

	// Assert
	assert_html_contains(&html, "djdt-panel-request");
	assert_html_contains(&html, "djdt-panel-sql");
	assert_html_contains(&html, "djdt-panel-cache");
	assert_html_contains(&html, "data-panel=\"request\"");
	assert_html_contains(&html, "data-panel=\"sql\"");
	assert_html_contains(&html, "data-panel=\"cache\"");
}

#[rstest]
fn render_toolbar_uses_json_fallback_when_no_rendered_html() {
	// Arrange
	let stats = vec![PanelStats {
		panel_id: "test".to_string(),
		panel_name: "Test".to_string(),
		data: serde_json::json!({"key": "value"}),
		summary: "test".to_string(),
		rendered_html: None,
	}];

	// Act
	let html = render_toolbar(&stats).unwrap();

	// Assert
	assert_html_contains(&html, "<pre>");
}

#[rstest]
fn registry_default_impl_is_empty() {
	// Arrange / Act
	let registry = PanelRegistry::default();

	// Assert
	assert_eq!(registry.len(), 0);
	assert!(registry.is_empty());
}

// ============================================================================
// 2. Error Path Tests
// ============================================================================

#[rstest]
fn toolbar_error_display_serialization_error() {
	// Arrange
	let json_err: Result<serde_json::Value, _> = serde_json::from_str("invalid json");
	let err: ToolbarError = json_err.unwrap_err().into();

	// Act
	let display = format!("{}", err);

	// Assert
	assert!(
		display.starts_with("Serialization error:"),
		"Got: {}",
		display
	);
}

#[rstest]
fn toolbar_error_display_injection_error() {
	// Arrange
	let err = ToolbarError::InjectionError("test".to_string());

	// Act
	let display = format!("{}", err);

	// Assert
	assert_eq!(display, "HTML injection error: test");
}

#[rstest]
fn toolbar_error_display_panel_not_found() {
	// Arrange
	let err = ToolbarError::PanelNotFound("missing".to_string());

	// Act
	let display = format!("{}", err);

	// Assert
	assert_eq!(display, "Panel not found: missing");
}

#[rstest]
fn toolbar_error_display_render_error() {
	// Arrange
	let err = ToolbarError::RenderError("fail".to_string());

	// Act
	let display = format!("{}", err);

	// Assert
	assert_eq!(display, "Panel rendering error: fail");
}

#[rstest]
fn toolbar_error_display_context_not_available() {
	// Arrange
	let err = ToolbarError::ContextNotAvailable;

	// Act
	let display = format!("{}", err);

	// Assert
	assert_eq!(display, "Toolbar context not available");
}

#[rstest]
fn toolbar_error_from_serde_json_error() {
	// Arrange
	let json_err = serde_json::from_str::<serde_json::Value>("not valid json").unwrap_err();

	// Act
	let err: ToolbarError = json_err.into();

	// Assert
	assert!(matches!(err, ToolbarError::SerializationError(_)));
}

#[rstest]
fn toolbar_error_from_io_error() {
	// Arrange
	let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");

	// Act
	let err: ToolbarError = io_err.into();

	// Assert
	assert!(matches!(err, ToolbarError::IoError(_)));
}

#[rstest]
fn registry_get_nonexistent_panel_returns_none(empty_registry: PanelRegistry) {
	// Arrange (empty_registry from fixture)

	// Act
	let result = empty_registry.get("nonexistent");

	// Assert
	assert!(result.is_none());
}

#[rstest]
#[tokio::test]
async fn mock_panel_generate_stats_failure_returns_error(test_context: ToolbarContext) {
	// Arrange
	let panel = MockPanel::new("fail_panel", "Failing Panel").with_generate_stats_failure();

	// Act
	let result = panel.generate_stats(&test_context).await;

	// Assert
	assert!(result.is_err());
	let err_msg = format!("{}", result.unwrap_err());
	assert!(err_msg.contains("fail_panel"), "Got: {}", err_msg);
}

#[rstest]
fn mock_panel_render_failure_returns_error() {
	// Arrange
	let panel = MockPanel::new("fail_render", "Failing Render").with_render_failure();
	let stats = PanelStats {
		panel_id: "fail_render".to_string(),
		panel_name: "Failing Render".to_string(),
		data: serde_json::json!({}),
		summary: "test".to_string(),
		rendered_html: None,
	};

	// Act
	let result = panel.render(&stats);

	// Assert
	assert!(result.is_err());
	assert!(matches!(result.unwrap_err(), ToolbarError::RenderError(_)));
}

// ============================================================================
// 3. Edge Cases Tests
// ============================================================================

#[rstest]
fn context_record_template_at_max_boundary(test_context: ToolbarContext) {
	// Arrange / Act
	populate_templates(&test_context, 101, "template");

	// Assert
	let templates = test_context.get_templates();
	assert_eq!(templates.len(), 100);
	// Oldest (template_0) should be evicted; first remaining is template_1
	assert_eq!(templates[0].name, "template_1.html");
}

#[rstest]
fn context_record_cache_op_at_max_boundary(test_context: ToolbarContext) {
	// Arrange / Act
	populate_cache_ops(&test_context, 1001);

	// Assert
	assert_eq!(test_context.get_cache_ops().len(), 1000);
}

#[rstest]
fn context_add_marker_at_max_boundary(test_context: ToolbarContext) {
	// Arrange / Act
	populate_markers(&test_context, 501);

	// Assert
	let markers = test_context.get_performance_markers();
	assert_eq!(markers.len(), 500);
	// Oldest (Marker_0) should be evicted
	assert_eq!(markers[0].name, "Marker_1");
}

#[rstest]
#[tokio::test]
async fn request_info_with_empty_headers() {
	// Arrange
	let request_info = RequestInfo {
		method: "GET".to_string(),
		path: "/empty".to_string(),
		query: None,
		headers: vec![],
		client_ip: "127.0.0.1".to_string(),
		timestamp: Utc::now(),
	};
	let ctx = ToolbarContext::new(request_info);
	let panel = RequestPanel::new();

	// Act
	let stats = panel.generate_stats(&ctx).await.unwrap();
	let html = panel.render(&stats).unwrap();

	// Assert
	assert_stats_has_field(&stats, "headers");
	assert_html_contains(&html, "Request Information");
}

#[rstest]
#[tokio::test]
async fn request_info_with_no_query_string() {
	// Arrange
	let request_info = RequestInfo {
		method: "GET".to_string(),
		path: "/no-query".to_string(),
		query: None,
		headers: vec![],
		client_ip: "127.0.0.1".to_string(),
		timestamp: Utc::now(),
	};
	let ctx = ToolbarContext::new(request_info);
	let panel = RequestPanel::new();

	// Act
	let stats = panel.generate_stats(&ctx).await.unwrap();
	let html = panel.render(&stats).unwrap();

	// Assert
	assert_html_not_contains(&html, "Query String");
}

#[rstest]
fn render_toolbar_with_empty_panel_stats() {
	// Arrange
	let stats: Vec<PanelStats> = vec![];

	// Act
	let html = render_toolbar(&stats).unwrap();

	// Assert
	assert_html_contains(&html, "djDebug");
	// No panel-specific elements should exist (id="djdt-panel-..." is only generated per panel)
	assert_html_not_contains(&html, "id=\"djdt-panel-");
}

#[rstest]
fn sanitize_headers_empty_key_and_value() {
	// Arrange / Act
	let (key, value) = sanitize_headers("", "");

	// Assert
	assert_eq!(key, "");
	assert_eq!(value, "");
}

#[rstest]
fn sanitize_headers_key_is_exactly_sensitive_keyword() {
	// Arrange / Act
	let (key, value) = sanitize_headers("password", "secret123");

	// Assert
	assert_eq!(key, "password");
	assert_eq!(value, "***REDACTED***");
}

#[rstest]
#[tokio::test]
async fn request_panel_render_with_special_chars_in_path() {
	// Arrange
	let request_info = RequestInfo {
		method: "GET".to_string(),
		path: "/search?q=<script>alert('xss')</script>".to_string(),
		query: Some("<script>alert('xss')</script>".to_string()),
		headers: vec![],
		client_ip: "127.0.0.1".to_string(),
		timestamp: Utc::now(),
	};
	let ctx = ToolbarContext::new(request_info);
	let panel = RequestPanel::new();

	// Act
	let stats = panel.generate_stats(&ctx).await.unwrap();
	let html = panel.render(&stats).unwrap();

	// Assert
	assert_html_contains(&html, "&lt;script&gt;");
	assert_html_not_contains(&html, "<script>alert");
}

// ============================================================================
// 4. State Transitions Tests
// ============================================================================

#[rstest]
fn context_accumulates_sql_queries_sequentially(test_context: ToolbarContext) {
	// Arrange / Act / Assert - incremental
	assert_eq!(test_context.get_sql_queries().len(), 0);

	populate_sql_queries(&test_context, 1, "SELECT 1");
	assert_eq!(test_context.get_sql_queries().len(), 1);

	populate_sql_queries(&test_context, 1, "SELECT 2");
	assert_eq!(test_context.get_sql_queries().len(), 2);

	populate_sql_queries(&test_context, 1, "SELECT 3");
	assert_eq!(test_context.get_sql_queries().len(), 3);
}

#[rstest]
fn context_accumulates_mixed_data_types(test_context: ToolbarContext) {
	// Arrange / Act
	populate_sql_queries(&test_context, 3, "SELECT 1");
	populate_templates(&test_context, 2, "tmpl");
	populate_cache_ops(&test_context, 4);
	populate_markers(&test_context, 1);

	// Assert
	assert_eq!(test_context.get_sql_queries().len(), 3);
	assert_eq!(test_context.get_templates().len(), 2);
	assert_eq!(test_context.get_cache_ops().len(), 4);
	assert_eq!(test_context.get_performance_markers().len(), 1);
}

#[rstest]
fn context_bounded_buffer_evicts_oldest_first(test_context: ToolbarContext) {
	// Arrange / Act
	populate_sql_queries(&test_context, 1001, "SELECT * FROM t");

	// Assert
	let queries = test_context.get_sql_queries();
	assert_eq!(queries.len(), 1000);
	// First entry should be query_1, not query_0 (oldest evicted)
	assert!(
		queries[0].sql.contains("-- 1"),
		"Expected first query to contain '-- 1', got: {}",
		queries[0].sql
	);
}

#[rstest]
fn registry_register_replaces_panel_with_same_id() {
	// Arrange
	let mut registry = PanelRegistry::new();
	let panel1 = MockPanel::new("same_id", "Panel V1").with_priority(10);
	let panel2 = MockPanel::new("same_id", "Panel V2").with_priority(99);

	// Act
	registry.register(Box::new(panel1));
	registry.register(Box::new(panel2));

	// Assert
	assert_eq!(registry.len(), 1);
	let panel = registry.get("same_id").unwrap();
	assert_eq!(panel.name(), "Panel V2");
	assert_eq!(panel.priority(), 99);
}

#[rstest]
#[tokio::test]
async fn mock_panel_call_counters_increment_correctly(test_context: ToolbarContext) {
	// Arrange
	let panel = MockPanel::new("counter", "Counter Panel");

	// Act
	panel.generate_stats(&test_context).await.unwrap();
	panel.generate_stats(&test_context).await.unwrap();
	panel.generate_stats(&test_context).await.unwrap();

	// Assert
	assert_eq!(panel.generate_stats_count(), 3);

	// Act - reset
	panel.reset_counters();

	// Assert
	assert_eq!(panel.generate_stats_count(), 0);
}

// ============================================================================
// 5. Fuzz Tests
// ============================================================================

#[rstest]
fn sanitize_headers_with_varied_ascii_keys() {
	// Arrange
	let keys = [
		"",
		"a",
		"X-Very-Long-Header-Name-That-Goes-On-And-On-And-On-Forever",
		"header-with-unicode-\u{00e9}\u{00e8}\u{00ea}",
		"!@#$%^&*()",
		"ALLCAPS",
		"  spaces  ",
		"password-embedded",
	];

	// Act / Assert - no panics
	for key in &keys {
		let (_k, _v) = sanitize_headers(key, "test_value");
	}
}

#[rstest]
fn context_record_sql_with_varied_sql_strings(test_context: ToolbarContext) {
	// Arrange
	let sqls = [
		"",
		"SELECT 1",
		"SELECT * FROM \u{1f4a9}",
		&"A".repeat(10000),
		"SELECT '\\'--; DROP TABLE users;",
		"   \n\t\r   ",
	];

	// Act / Assert - no panics
	for sql in &sqls {
		let query = SqlQueryBuilder::new()
			.sql(*sql)
			.duration(Duration::from_millis(1))
			.build();
		test_context.record_sql_query(query);
	}
	assert_eq!(test_context.get_sql_queries().len(), sqls.len());
}

// ============================================================================
// 6. Property-Based Tests
// ============================================================================

#[rstest]
fn context_get_snapshots_are_independent_of_mutations(test_context: ToolbarContext) {
	// Arrange
	populate_sql_queries(&test_context, 3, "SELECT 1");

	// Act
	let snapshot_before = test_context.get_sql_queries();
	populate_sql_queries(&test_context, 2, "SELECT 2");
	let snapshot_after = test_context.get_sql_queries();

	// Assert - old snapshot is unaffected
	assert_eq!(snapshot_before.len(), 3);
	assert_eq!(snapshot_after.len(), 5);
}

#[rstest]
fn sanitize_headers_preserves_key_identity() {
	// Arrange
	let keys = ["Content-Type", "Authorization", "X-Custom", "password", ""];

	// Act / Assert - property: returned key always == input key
	for key in &keys {
		let (returned_key, _) = sanitize_headers(key, "value");
		assert_eq!(&returned_key, key);
	}
}

#[rstest]
fn sanitize_headers_redacted_value_is_constant() {
	// Arrange
	let sensitive_keys = ["Authorization", "X-API-Key", "session-token", "password"];

	// Act / Assert - property: all sensitive keys produce the same redacted value
	for key in &sensitive_keys {
		let (_, value) = sanitize_headers(key, "anything");
		assert_eq!(value, "***REDACTED***");
	}
}

#[rstest]
fn render_toolbar_always_contains_required_structure() {
	// Arrange / Act / Assert - for 0..=5 panels
	for panel_count in 0..=5 {
		let stats: Vec<PanelStats> = (0..panel_count)
			.map(|i| PanelStats {
				panel_id: format!("panel_{}", i),
				panel_name: format!("Panel {}", i),
				data: serde_json::json!({}),
				summary: "summary".to_string(),
				rendered_html: Some("<div>content</div>".to_string()),
			})
			.collect();

		let html = render_toolbar(&stats).unwrap();
		assert_html_contains(&html, "djDebug");
		assert_html_contains(&html, "<style>");
		assert_html_contains(&html, "<script>");
	}
}

// ============================================================================
// 7. Combination Tests
// ============================================================================

#[rstest]
fn context_mixed_recording_of_all_data_types(test_context: ToolbarContext) {
	// Arrange / Act - interleave 10 of each type
	for i in 0..10 {
		let query = SqlQueryBuilder::new().sql(format!("SELECT {}", i)).build();
		test_context.record_sql_query(query);

		let template = TemplateInfoBuilder::new()
			.name(format!("tmpl_{}", i))
			.build();
		test_context.record_template(template);

		let cache_op = CacheOperationBuilder::get()
			.key(format!("key_{}", i))
			.build();
		test_context.record_cache_op(cache_op);

		let marker = PerformanceMarkerBuilder::new()
			.name(format!("marker_{}", i))
			.build();
		test_context.add_marker(marker);
	}

	// Assert
	assert_eq!(test_context.get_sql_queries().len(), 10);
	assert_eq!(test_context.get_templates().len(), 10);
	assert_eq!(test_context.get_cache_ops().len(), 10);
	assert_eq!(test_context.get_performance_markers().len(), 10);
}

#[rstest]
fn config_combinations_enabled_and_ip_and_panels() {
	// Arrange
	let localhost: IpAddr = "127.0.0.1".parse().unwrap();
	let external: IpAddr = "8.8.8.8".parse().unwrap();

	// Act / Assert - (enabled=true, ip=localhost) -> true
	let mut config = ToolbarConfig {
		enabled: true,
		..Default::default()
	};
	assert!(config.should_show(&localhost));

	// (enabled=true, ip=external) -> false
	assert!(!config.should_show(&external));

	// (enabled=false, ip=localhost) -> false
	config.enabled = false;
	assert!(!config.should_show(&localhost));

	// (enabled=false, ip=external) -> false
	assert!(!config.should_show(&external));
}

// ============================================================================
// 8. Sanity Tests
// ============================================================================

#[rstest]
fn lib_quick_start_toolbar_config_construction() {
	// Arrange / Act
	let config = ToolbarConfig {
		enabled: true,
		internal_ips: vec!["127.0.0.1".parse().unwrap()],
		..Default::default()
	};
	let mut registry = PanelRegistry::new();
	registry.register(Box::new(RequestPanel::new()));
	let layer = DebugToolbarLayer::new(config, registry);

	// Assert - creates valid service via Layer trait
	let _service = layer.layer(tower::service_fn(
		|_req: http::Request<axum::body::Body>| async {
			Ok::<_, std::convert::Infallible>(axum::response::Response::new(
				axum::body::Body::empty(),
			))
		},
	));
}

#[cfg(feature = "sql-panel")]
#[rstest]
fn sql_normalization_doc_example_equivalence() {
	// Arrange
	use reinhardt_debug_toolbar::utils::sql_normalization::normalize_sql;
	let sql1 = "SELECT * FROM users WHERE id = 123";
	let sql2 = "SELECT * FROM users WHERE id = 456";

	// Act
	let norm1 = normalize_sql(sql1);
	let norm2 = normalize_sql(sql2);

	// Assert
	assert_eq!(norm1, norm2);
}

#[rstest]
fn mock_panel_builder_doc_patterns() {
	// Arrange / Act
	let panel = MockPanel::new("test", "Test Panel")
		.with_priority(50)
		.with_custom_html("<div>Custom</div>");

	// Assert
	assert_eq!(panel.id(), "test");
	assert_eq!(panel.name(), "Test Panel");
	assert_eq!(panel.priority(), 50);
}

#[rstest]
fn panel_stats_serialization_roundtrip() {
	// Arrange
	let stats = PanelStats {
		panel_id: "test".to_string(),
		panel_name: "Test".to_string(),
		data: serde_json::json!({"key": "value"}),
		summary: "summary".to_string(),
		rendered_html: Some("html content".to_string()),
	};

	// Act
	let json = serde_json::to_string(&stats).unwrap();
	let deserialized: PanelStats = serde_json::from_str(&json).unwrap();

	// Assert
	assert_eq!(deserialized.panel_id, "test");
	assert_eq!(deserialized.panel_name, "Test");
	assert_eq!(deserialized.summary, "summary");
	// rendered_html has #[serde(skip_serializing)], so it should be None after roundtrip
	assert!(deserialized.rendered_html.is_none());
}

// ============================================================================
// 9. Equivalence Partitioning Tests
// ============================================================================

#[rstest]
#[case("password", "secret", true)]
#[case("token", "abc123", true)]
#[case("secret", "s3cret", true)]
#[case("api_key", "key123", true)]
#[case("apikey", "key123", true)]
#[case("api-key", "key123", true)]
#[case("authorization", "Bearer x", true)]
#[case("auth", "basic x", true)]
#[case("session", "sess_id", true)]
#[case("cookie", "val", true)]
#[case("csrf", "token", true)]
#[case("X-Custom-Token", "tok", true)]
#[case("Content-Type", "text/html", false)]
#[case("Accept", "application/json", false)]
fn sanitize_headers_sensitive_key_partitions(
	#[case] key: &str,
	#[case] value: &str,
	#[case] should_redact: bool,
) {
	// Arrange / Act
	let (_, sanitized_value) = sanitize_headers(key, value);

	// Assert
	if should_redact {
		assert_eq!(
			sanitized_value, "***REDACTED***",
			"Key '{}' should be redacted",
			key
		);
	} else {
		assert_eq!(
			sanitized_value, value,
			"Key '{}' should NOT be redacted",
			key
		);
	}
}

#[rstest]
#[case("Authorization", true)]
#[case("AUTHORIZATION", true)]
#[case("authorization", true)]
#[case("X-Custom-Authorization-Header", true)]
#[case("Content-Type", false)]
fn sanitize_headers_case_insensitivity_partitions(#[case] key: &str, #[case] should_redact: bool) {
	// Arrange / Act
	let (_, value) = sanitize_headers(key, "test_value");

	// Assert
	if should_redact {
		assert_eq!(value, "***REDACTED***");
	} else {
		assert_eq!(value, "test_value");
	}
}

#[rstest]
#[case("127.0.0.1", true)]
#[case("::1", true)]
#[case("192.168.1.1", false)]
#[case("10.0.0.1", false)]
#[case("8.8.8.8", false)]
#[case("fe80::1", false)]
fn toolbar_config_should_show_ip_partitions(#[case] ip_str: &str, #[case] expected: bool) {
	// Arrange
	let config = ToolbarConfig {
		enabled: true,
		..Default::default()
	};
	let ip: IpAddr = ip_str.parse().unwrap();

	// Act
	let result = config.should_show(&ip);

	// Assert
	assert_eq!(result, expected, "IP {} should_show = {}", ip_str, expected);
}

// ============================================================================
// 10. Boundary Value Analysis Tests
// ============================================================================

#[rstest]
#[case(999, 999)]
#[case(1000, 1000)]
#[case(1001, 1000)]
#[case(1002, 1000)]
fn context_sql_bounded_buffer_boundary_values(
	#[case] insert_count: usize,
	#[case] expected_len: usize,
) {
	// Arrange
	let ctx = ToolbarContext::new(test_request_info());

	// Act
	populate_sql_queries(&ctx, insert_count, "SELECT 1");

	// Assert
	assert_eq!(ctx.get_sql_queries().len(), expected_len);
}

#[rstest]
#[case(99, 99)]
#[case(100, 100)]
#[case(101, 100)]
fn context_template_bounded_buffer_boundary_values(
	#[case] insert_count: usize,
	#[case] expected_len: usize,
) {
	// Arrange
	let ctx = ToolbarContext::new(test_request_info());

	// Act
	populate_templates(&ctx, insert_count, "template");

	// Assert
	assert_eq!(ctx.get_templates().len(), expected_len);
}

#[rstest]
#[case(999, 999)]
#[case(1000, 1000)]
#[case(1001, 1000)]
fn context_cache_bounded_buffer_boundary_values(
	#[case] insert_count: usize,
	#[case] expected_len: usize,
) {
	// Arrange
	let ctx = ToolbarContext::new(test_request_info());

	// Act
	populate_cache_ops(&ctx, insert_count);

	// Assert
	assert_eq!(ctx.get_cache_ops().len(), expected_len);
}

#[rstest]
#[case(499, 499)]
#[case(500, 500)]
#[case(501, 500)]
fn context_marker_bounded_buffer_boundary_values(
	#[case] insert_count: usize,
	#[case] expected_len: usize,
) {
	// Arrange
	let ctx = ToolbarContext::new(test_request_info());

	// Act
	populate_markers(&ctx, insert_count);

	// Assert
	assert_eq!(ctx.get_performance_markers().len(), expected_len);
}

// ============================================================================
// 11. Decision Table Tests
// ============================================================================

#[rstest]
#[case(true, "127.0.0.1", true)]
#[case(true, "8.8.8.8", false)]
#[case(false, "127.0.0.1", false)]
#[case(false, "8.8.8.8", false)]
fn config_should_show_decision_table(
	#[case] enabled: bool,
	#[case] ip_str: &str,
	#[case] expected: bool,
) {
	// Arrange
	let config = ToolbarConfig {
		enabled,
		..Default::default()
	};
	let ip: IpAddr = ip_str.parse().unwrap();

	// Act
	let result = config.should_show(&ip);

	// Assert
	assert_eq!(result, expected);
}

#[rstest]
#[case(vec![], "sql", true)]
#[case(vec![], "request", true)]
#[case(vec!["sql".to_string()], "sql", true)]
#[case(vec!["sql".to_string()], "request", false)]
#[case(vec!["sql".to_string(), "request".to_string()], "sql", true)]
#[case(vec!["sql".to_string(), "request".to_string()], "cache", false)]
fn config_is_panel_enabled_decision_table(
	#[case] enabled_panels: Vec<String>,
	#[case] panel_id: &str,
	#[case] expected: bool,
) {
	// Arrange
	let config = ToolbarConfig {
		enabled: true,
		enabled_panels,
		..Default::default()
	};

	// Act
	let result = config.is_panel_enabled(panel_id);

	// Assert
	assert_eq!(result, expected);
}
