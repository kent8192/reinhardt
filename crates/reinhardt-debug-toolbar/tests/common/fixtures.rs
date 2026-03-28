//! Common test fixtures for reinhardt-debug-toolbar tests
//!
//! This module provides reusable test fixtures using rstest framework.

use chrono::Utc;
#[cfg(feature = "sql-panel")]
use reinhardt_debug_toolbar::panels::sql::SqlPanel;
use reinhardt_debug_toolbar::{
	context::{RequestInfo, ToolbarContext},
	middleware::ToolbarConfig,
	panels::{registry::PanelRegistry, request::RequestPanel},
};
use rstest::*;

/// Default toolbar configuration fixture
///
/// Creates a ToolbarConfig with standard defaults:
/// - enabled: true (for testing)
/// - internal_ips: ["127.0.0.1", "::1"]
/// - enabled_panels: [] (all panels enabled)
/// - sql_warning_threshold_ms: 100
#[fixture]
pub fn default_config() -> ToolbarConfig {
	ToolbarConfig {
		enabled: true, // Override debug_assertions for testing
		..Default::default()
	}
}

/// Localhost-only configuration fixture
///
/// Creates a ToolbarConfig configured for localhost access only.
#[fixture]
pub fn localhost_config() -> ToolbarConfig {
	ToolbarConfig {
		enabled: true,
		internal_ips: vec!["127.0.0.1".parse().unwrap(), "::1".parse().unwrap()],
		..Default::default()
	}
}

/// Test request information fixture
///
/// Creates a basic RequestInfo with test values.
#[fixture]
pub fn test_request_info() -> RequestInfo {
	RequestInfo {
		method: "GET".to_string(),
		path: "/test".to_string(),
		query: Some("foo=bar".to_string()),
		headers: vec![
			("Content-Type".to_string(), "application/json".to_string()),
			("User-Agent".to_string(), "Test Agent".to_string()),
		],
		client_ip: "127.0.0.1".to_string(),
		timestamp: Utc::now(),
	}
}

/// Test toolbar context fixture
///
/// Creates a ToolbarContext with default test RequestInfo.
#[fixture]
pub fn test_context() -> ToolbarContext {
	let request_info = test_request_info();
	ToolbarContext::new(request_info)
}

/// Default panel registry fixture
///
/// Creates a PanelRegistry with common panels registered:
/// - RequestPanel (priority: 100)
/// - SqlPanel (priority: 90, threshold: 100ms) (when sql-panel feature is enabled)
#[fixture]
pub fn default_registry() -> PanelRegistry {
	let mut registry = PanelRegistry::new();
	registry.register(Box::new(RequestPanel::new()));
	#[cfg(feature = "sql-panel")]
	registry.register(Box::new(SqlPanel::new()));
	registry
}

/// Empty panel registry fixture
///
/// Creates an empty PanelRegistry for testing edge cases.
#[fixture]
pub fn empty_registry() -> PanelRegistry {
	PanelRegistry::new()
}
