//! Integration tests for reinhardt-debug-toolbar
//!
//! Tests that verify cross-module interactions: inject_toolbar, middleware layer,
//! full pipeline, and use-case scenarios.

use crate::common::{
	fixtures::localhost_config,
	helpers::{
		assert_html_contains, assert_html_not_contains, html_response, json_response,
		response_with_content_type,
	},
};
#[cfg(feature = "sql-panel")]
use crate::common::{
	fixtures::test_context,
	helpers::{create_n_plus_one_pattern, create_slow_queries},
	mock_panel::MockPanel,
};
use axum::body::Body;
use chrono::Utc;
use http_body_util::BodyExt;
#[cfg(feature = "sql-panel")]
use reinhardt_debug_toolbar::panels::registry::PanelRegistry;
#[cfg(feature = "sql-panel")]
use reinhardt_debug_toolbar::panels::sql::SqlPanel;
use reinhardt_debug_toolbar::{
	DebugToolbarLayer,
	context::{RequestInfo, ToolbarContext},
	middleware::ToolbarConfig,
	panels::{Panel, PanelStats, request::RequestPanel},
	ui::inject_toolbar,
};
use rstest::*;
use std::net::IpAddr;
use tower::Layer;

// ============================================================================
// 1. Happy Path Tests
// ============================================================================

#[rstest]
#[tokio::test]
async fn inject_toolbar_into_html_response() {
	// Arrange
	let original_body = "<html><body><h1>Hello</h1></body></html>";
	let response = html_response(original_body);
	let stats = vec![PanelStats {
		panel_id: "request".to_string(),
		panel_name: "Request".to_string(),
		data: serde_json::json!({}),
		summary: "GET /".to_string(),
		rendered_html: Some("<div>Request Info</div>".to_string()),
	}];

	// Act
	let result = inject_toolbar(response, &stats).await.unwrap();
	let body_bytes = result.into_body().collect().await.unwrap().to_bytes();
	let html = String::from_utf8_lossy(&body_bytes);

	// Assert
	assert_html_contains(&html, "<h1>Hello</h1>");
	assert_html_contains(&html, "djDebug");
	assert_html_contains(&html, "</body>");
}

#[rstest]
#[tokio::test]
async fn inject_toolbar_skips_non_html_response() {
	// Arrange
	let original_body = r#"{"key":"value"}"#;
	let response = json_response(original_body);
	let stats = vec![PanelStats {
		panel_id: "request".to_string(),
		panel_name: "Request".to_string(),
		data: serde_json::json!({}),
		summary: "GET /api".to_string(),
		rendered_html: None,
	}];

	// Act
	let result = inject_toolbar(response, &stats).await.unwrap();
	let body_bytes = result.into_body().collect().await.unwrap().to_bytes();
	let body = String::from_utf8_lossy(&body_bytes);

	// Assert
	assert_eq!(body, original_body);
}

#[rstest]
fn toolbar_layer_creates_service_with_default_panels() {
	// Arrange
	let layer = DebugToolbarLayer::with_default();

	// Act
	let _service = layer.layer(tower::service_fn(|_req: http::Request<Body>| async {
		Ok::<_, std::convert::Infallible>(axum::response::Response::new(Body::empty()))
	}));

	// Assert - no panic, service is created successfully
}

#[rstest]
#[tokio::test]
async fn full_pipeline_request_panel_to_injection() {
	// Arrange
	let request_info = RequestInfo {
		method: "GET".to_string(),
		path: "/dashboard".to_string(),
		query: Some("page=1".to_string()),
		headers: vec![("Content-Type".to_string(), "text/html".to_string())],
		client_ip: "127.0.0.1".to_string(),
		timestamp: Utc::now(),
	};
	let ctx = ToolbarContext::new(request_info);
	let panel = RequestPanel::new();

	// Act - generate stats
	let mut stats = panel.generate_stats(&ctx).await.unwrap();
	let rendered = panel.render(&stats).unwrap();
	stats.rendered_html = Some(rendered);

	// Act - render toolbar and inject
	let response = html_response("<html><body><p>Dashboard</p></body></html>");
	let result = inject_toolbar(response, &[stats]).await.unwrap();
	let body_bytes = result.into_body().collect().await.unwrap().to_bytes();
	let html = String::from_utf8_lossy(&body_bytes);

	// Assert
	assert_html_contains(&html, "Dashboard");
	assert_html_contains(&html, "Request Information");
	assert_html_contains(&html, "djDebug");
}

// ============================================================================
// 2. Error Path Tests
// ============================================================================

#[rstest]
#[tokio::test]
async fn inject_toolbar_no_content_type_header() {
	// Arrange
	let response = axum::response::Response::builder()
		.body(Body::from("plain text"))
		.unwrap();
	let stats = vec![];

	// Act
	let result = inject_toolbar(response, &stats).await.unwrap();
	let body_bytes = result.into_body().collect().await.unwrap().to_bytes();
	let body = String::from_utf8_lossy(&body_bytes);

	// Assert - passes through unchanged
	assert_eq!(body, "plain text");
}

#[rstest]
#[tokio::test]
async fn with_toolbar_context_outside_scope_returns_none() {
	// Arrange / Act
	// TOOLBAR_CONTEXT is task-local; outside a scope, try_with returns Err
	let result = reinhardt_debug_toolbar::context::TOOLBAR_CONTEXT
		.try_with(|ctx| ctx.get_sql_queries().len())
		.ok();

	// Assert
	assert!(result.is_none());
}

// ============================================================================
// 3. Edge Cases Tests
// ============================================================================

#[rstest]
#[tokio::test]
async fn inject_toolbar_response_without_body_tag() {
	// Arrange
	let response = html_response("<html><div>No body tag</div></html>");
	let stats = vec![PanelStats {
		panel_id: "test".to_string(),
		panel_name: "Test".to_string(),
		data: serde_json::json!({}),
		summary: "test".to_string(),
		rendered_html: Some("<div>Toolbar</div>".to_string()),
	}];

	// Act
	let result = inject_toolbar(response, &stats).await.unwrap();
	let body_bytes = result.into_body().collect().await.unwrap().to_bytes();
	let html = String::from_utf8_lossy(&body_bytes);

	// Assert - toolbar appended to end since no </body>
	assert_html_contains(&html, "No body tag");
	assert_html_contains(&html, "djDebug");
}

#[rstest]
#[tokio::test]
async fn inject_toolbar_empty_html_body() {
	// Arrange
	let response = html_response("");
	let stats = vec![PanelStats {
		panel_id: "test".to_string(),
		panel_name: "Test".to_string(),
		data: serde_json::json!({}),
		summary: "test".to_string(),
		rendered_html: Some("<div>Toolbar</div>".to_string()),
	}];

	// Act
	let result = inject_toolbar(response, &stats).await.unwrap();
	let body_bytes = result.into_body().collect().await.unwrap().to_bytes();
	let html = String::from_utf8_lossy(&body_bytes);

	// Assert - toolbar injected even into empty body
	assert_html_contains(&html, "djDebug");
}

// ============================================================================
// 4. Use Case Tests
// ============================================================================

#[cfg(feature = "sql-panel")]
#[rstest]
#[tokio::test]
async fn web_developer_debugs_slow_sql_queries(test_context: ToolbarContext) {
	// Arrange
	create_slow_queries(&test_context, 2, 100);
	let panel = SqlPanel::new();

	// Act
	let stats = panel.generate_stats(&test_context).await.unwrap();
	let html = panel.render(&stats).unwrap();

	let toolbar_stats = vec![PanelStats {
		rendered_html: Some(html.clone()),
		..stats
	}];
	let response = html_response("<html><body></body></html>");
	let result = inject_toolbar(response, &toolbar_stats).await.unwrap();
	let body_bytes = result.into_body().collect().await.unwrap().to_bytes();
	let final_html = String::from_utf8_lossy(&body_bytes);

	// Assert
	assert_html_contains(&final_html, "SLOW");
	assert_html_contains(&final_html, "djdt-warning");
}

#[cfg(feature = "sql-panel")]
#[rstest]
#[tokio::test]
async fn web_developer_detects_n_plus_one_pattern(test_context: ToolbarContext) {
	// Arrange
	create_n_plus_one_pattern(&test_context, 5);
	let panel = SqlPanel::new();

	// Act
	let stats = panel.generate_stats(&test_context).await.unwrap();
	let html = panel.render(&stats).unwrap();

	// Assert
	assert_html_contains(&html, "N+1");
	assert_html_contains(&html, "djdt-warning");
	assert_eq!(stats.data["n_plus_one_count"].as_u64().unwrap(), 1);
}

#[rstest]
#[tokio::test]
async fn web_developer_views_request_with_sanitized_headers() {
	// Arrange
	let request_info = RequestInfo {
		method: "GET".to_string(),
		path: "/admin".to_string(),
		query: None,
		headers: vec![
			(
				"Authorization".to_string(),
				"Bearer super-secret-token".to_string(),
			),
			("Content-Type".to_string(), "text/html".to_string()),
		],
		client_ip: "127.0.0.1".to_string(),
		timestamp: Utc::now(),
	};
	let ctx = ToolbarContext::new(request_info);
	let panel = RequestPanel::new();

	// Act
	let stats = panel.generate_stats(&ctx).await.unwrap();
	let html = panel.render(&stats).unwrap();

	let response = html_response("<html><body></body></html>");
	let toolbar_stats = vec![PanelStats {
		rendered_html: Some(html.clone()),
		..stats
	}];
	let result = inject_toolbar(response, &toolbar_stats).await.unwrap();
	let body_bytes = result.into_body().collect().await.unwrap().to_bytes();
	let final_html = String::from_utf8_lossy(&body_bytes);

	// Assert
	assert_html_contains(&final_html, "***REDACTED***");
	assert_html_not_contains(&final_html, "super-secret-token");
	assert_html_contains(&final_html, "text/html");
}

#[rstest]
#[tokio::test]
async fn api_response_not_modified_by_toolbar() {
	// Arrange
	let json_body = r#"{"users":[{"id":1,"name":"Alice"}]}"#;
	let response = json_response(json_body);
	let stats = vec![PanelStats {
		panel_id: "request".to_string(),
		panel_name: "Request".to_string(),
		data: serde_json::json!({}),
		summary: "GET /api/users".to_string(),
		rendered_html: None,
	}];

	// Act
	let result = inject_toolbar(response, &stats).await.unwrap();
	let body_bytes = result.into_body().collect().await.unwrap().to_bytes();
	let body = String::from_utf8_lossy(&body_bytes);

	// Assert
	assert_eq!(body, json_body);
}

#[rstest]
fn toolbar_config_restricts_to_internal_ips(localhost_config: ToolbarConfig) {
	// Arrange
	let localhost_v4: IpAddr = "127.0.0.1".parse().unwrap();
	let localhost_v6: IpAddr = "::1".parse().unwrap();
	let external: IpAddr = "203.0.113.1".parse().unwrap();
	let private: IpAddr = "192.168.1.100".parse().unwrap();

	// Act / Assert
	assert!(localhost_config.should_show(&localhost_v4));
	assert!(localhost_config.should_show(&localhost_v6));
	assert!(!localhost_config.should_show(&external));
	assert!(!localhost_config.should_show(&private));
}

#[cfg(feature = "sql-panel")]
#[rstest]
#[tokio::test]
async fn panel_registry_with_custom_and_builtin_panels(test_context: ToolbarContext) {
	// Arrange
	let mut registry = PanelRegistry::new();
	registry.register(Box::new(RequestPanel::new()));
	registry.register(Box::new(SqlPanel::new()));
	registry.register(Box::new(MockPanel::new("mock", "Mock").with_priority(50)));

	// Act
	let panels = registry.all();

	// Assert - sorted by priority: request(100) > sql(90) > mock(50)
	assert_eq!(panels.len(), 3);
	assert_eq!(panels[0].id(), "request");
	assert_eq!(panels[1].id(), "sql");
	assert_eq!(panels[2].id(), "mock");

	// All panels can generate stats
	for panel in &panels {
		let stats = panel.generate_stats(&test_context).await.unwrap();
		assert!(!stats.panel_id.is_empty());
		assert!(!stats.summary.is_empty());
	}
}

// ============================================================================
// 5. Combination Tests
// ============================================================================

#[rstest]
#[tokio::test]
async fn inject_toolbar_with_panels_of_mixed_rendered_html() {
	// Arrange
	let stats = vec![
		PanelStats {
			panel_id: "with_html".to_string(),
			panel_name: "With HTML".to_string(),
			data: serde_json::json!({}),
			summary: "has html".to_string(),
			rendered_html: Some("<div>Pre-rendered</div>".to_string()),
		},
		PanelStats {
			panel_id: "without_html".to_string(),
			panel_name: "Without HTML".to_string(),
			data: serde_json::json!({"fallback": true}),
			summary: "no html".to_string(),
			rendered_html: None,
		},
	];
	let response = html_response("<html><body></body></html>");

	// Act
	let result = inject_toolbar(response, &stats).await.unwrap();
	let body_bytes = result.into_body().collect().await.unwrap().to_bytes();
	let html = String::from_utf8_lossy(&body_bytes);

	// Assert
	assert_html_contains(&html, "Pre-rendered");
	assert_html_contains(&html, "<pre>"); // JSON fallback
}

#[rstest]
#[tokio::test]
async fn request_panel_with_all_http_methods() {
	// Arrange
	let methods = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"];

	for method in &methods {
		let request_info = RequestInfo {
			method: method.to_string(),
			path: "/resource".to_string(),
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
		assert_eq!(stats.summary, format!("{} /resource", method));
	}
}

#[cfg(feature = "sql-panel")]
#[rstest]
#[tokio::test]
async fn registry_with_all_panel_types_generates_stats_for_each(test_context: ToolbarContext) {
	// Arrange
	let mut registry = PanelRegistry::new();
	registry.register(Box::new(RequestPanel::new()));
	registry.register(Box::new(SqlPanel::new()));
	registry.register(Box::new(
		MockPanel::new("custom", "Custom").with_priority(50),
	));

	// Act
	let mut all_stats = Vec::new();
	for panel in registry.all() {
		let stats = panel.generate_stats(&test_context).await.unwrap();
		all_stats.push(stats);
	}

	// Assert
	assert_eq!(all_stats.len(), 3);
	for stats in &all_stats {
		assert!(!stats.panel_id.is_empty());
		assert!(!stats.panel_name.is_empty());
		assert!(!stats.summary.is_empty());
	}
}

// ============================================================================
// 6. Decision Table Tests
// ============================================================================

#[rstest]
#[case("text/html", "<body></body>", true, true)]
#[case("text/html", "<div>no body tag</div>", true, true)]
#[case("text/html; charset=utf-8", "<body></body>", true, true)]
#[case("application/json", "{}", false, false)]
#[case("text/plain", "hello", false, false)]
#[case("image/png", "binary", false, false)]
#[tokio::test]
async fn inject_toolbar_content_type_decision_table(
	#[case] content_type: &str,
	#[case] body: &str,
	#[case] _has_body_tag: bool,
	#[case] should_inject: bool,
) {
	// Arrange
	let response = response_with_content_type(content_type, body);
	let stats = vec![PanelStats {
		panel_id: "test".to_string(),
		panel_name: "Test".to_string(),
		data: serde_json::json!({}),
		summary: "test".to_string(),
		rendered_html: Some("<div>Toolbar</div>".to_string()),
	}];

	// Act
	let result = inject_toolbar(response, &stats).await.unwrap();
	let body_bytes = result.into_body().collect().await.unwrap().to_bytes();
	let html = String::from_utf8_lossy(&body_bytes);

	// Assert
	if should_inject {
		assert_html_contains(&html, "djDebug");
	} else {
		assert_html_not_contains(&html, "djDebug");
	}
}
